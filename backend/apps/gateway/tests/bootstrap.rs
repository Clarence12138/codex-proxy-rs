use std::{fs, process::Command};

use codex_proxy_rs::bootstrap::GatewayConfig;
use gateway_host::LoadableConfig;

const CONFIG_EXAMPLE: &str = include_str!("../../../../deploy/config.example.yaml");
const POSTGRES_PASSWORD: &str = "111111111111111111111111111111111111111111111111";
const REDIS_PASSWORD: &str = "222222222222222222222222222222222222222222222222";
const ADMIN_PASSWORD: &str = "test-admin-password";
const TOPOLOGY_CHILD_ENV: &str = "CPR_TEST_TOPOLOGY_CHILD";

#[tokio::test]
async fn proxy_probe_should_use_provider_custom_ca_for_https_proxies() {
    use gateway_admin::ports::proxy::ProxyProbe;
    use gateway_core::account::OutboundProxy;
    use gateway_host::proxy_probe::HttpProxyProbe;
    use std::{sync::Arc, time::Duration};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio_rustls::{
        TlsAcceptor,
        rustls::{
            ServerConfig,
            pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject as _},
        },
    };

    const CHILD_ENV: &str = "CPR_TEST_PROXY_TLS_DIRECTORY";
    let Ok(directory) = std::env::var(CHILD_ENV) else {
        let directory = tempfile::tempdir().unwrap();
        generate_proxy_test_certificates(directory.path());
        // 使用子进程隔离环境变量，避免并行测试读取到临时 CA 配置。
        for ca_env in ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"] {
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "bootstrap::proxy_probe_should_use_provider_custom_ca_for_https_proxies",
                    "--nocapture",
                ])
                .env(CHILD_ENV, directory.path())
                .env_remove("CODEX_CA_CERTIFICATE")
                .env_remove("SSL_CERT_FILE")
                .env(ca_env, directory.path().join("ca.pem"));
            if ca_env == "CODEX_CA_CERTIFICATE" {
                child.env(
                    "SSL_CERT_FILE",
                    directory.path().join("missing-fallback.pem"),
                );
            }
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "{ca_env}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    provider_openai::ensure_rustls_provider();
    let directory = std::path::Path::new(&directory);
    let certificate = CertificateDer::from_pem_file(directory.join("server.pem")).unwrap();
    let key = PrivateKeyDer::from_pem_file(directory.join("server.key")).unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate], key)
            .unwrap(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy =
        OutboundProxy::parse(&format!("https://{}", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(socket).await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            assert!(request.len() < 8192);
            request.push(stream.read_u8().await.unwrap());
        }
        assert!(request.starts_with(b"GET http://unresolvable.invalid/ip "));
        let body = "{\"ip\":\"203.0.113.8\"}";
        stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap();
    });
    let probe = HttpProxyProbe::new("http://unresolvable.invalid/ip")
        .with_client_builder(provider_openai::build_reqwest_client_with_custom_ca);
    let result = probe.test(&proxy).await;
    assert!(result.success, "{}", result.message);
    assert_eq!(result.exit_ip.unwrap().to_string(), "203.0.113.8");
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}

fn generate_proxy_test_certificates(directory: &std::path::Path) {
    let openssl = |args: &[&str]| {
        let output = Command::new("openssl")
            .args(args)
            .current_dir(directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    openssl(&[
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-keyout",
        "ca.key",
        "-out",
        "ca.pem",
        "-days",
        "1",
        "-subj",
        "/CN=Proxy Test CA",
    ]);
    openssl(&[
        "req",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-keyout",
        "server.key",
        "-out",
        "server.csr",
        "-subj",
        "/CN=localhost",
    ]);
    fs::write(directory.join("extensions"), "subjectAltName=IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
    openssl(&[
        "x509",
        "-req",
        "-in",
        "server.csr",
        "-CA",
        "ca.pem",
        "-CAkey",
        "ca.key",
        "-CAcreateserial",
        "-out",
        "server.pem",
        "-days",
        "1",
        "-extfile",
        "extensions",
    ]);
}

#[test]
fn config_loader_should_load_complete_terminal_example() {
    parse_config(&valid_config()).expect("terminal config example");
}

#[test]
fn config_loader_should_resolve_paths_relative_to_config_file() {
    let (config, _directory) = parse_config(&valid_config()).expect("resolved config");
    let debug = format!("{config:?}");
    assert!(debug.contains(".runtime/data"));
    assert!(debug.contains(".runtime/logs"));
    assert!(debug.contains("frontend/dist"));
}

#[test]
fn config_loader_should_reject_missing_runtime_data_dir() {
    let config = valid_config().replace("  runtime_data_dir: '../.runtime/data'\n", "");

    assert!(parse_config(&config).is_err());
}

#[test]
fn config_loader_should_inject_connection_passwords_into_urls() {
    parse_config(&valid_config()).expect("Store validates password injection into both URLs");
}

#[test]
fn config_loader_should_apply_only_explicit_topology_overrides() {
    let invalid = valid_config()
        .replace("host: '127.0.0.1'", "host: ''")
        .replace("port: 8080", "port: 0")
        .replace(
            "url: 'postgres://codex_proxy@127.0.0.1:5432/codex_proxy'",
            "url: 'invalid-postgres-url'",
        )
        .replace("url: 'redis://127.0.0.1:6379/'", "url: 'invalid-redis-url'")
        .replace(POSTGRES_PASSWORD, "invalid-postgres-password")
        .replace(REDIS_PASSWORD, "invalid-redis-password");
    if std::env::var_os(TOPOLOGY_CHILD_ENV).is_some() {
        parse_config(&invalid).expect("explicit package-owned environment overrides");
        return;
    }
    assert!(parse_config(&invalid).is_err());
    let status = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "bootstrap::config_loader_should_apply_only_explicit_topology_overrides",
        ])
        .env(TOPOLOGY_CHILD_ENV, "1")
        .env("CPR_SERVER_HOST", "127.0.0.1")
        .env("CPR_SERVER_PORT", "8080")
        .env(
            "CPR_DATABASE_URL",
            "postgres://codex_proxy@127.0.0.1:5432/codex_proxy",
        )
        .env("CPR_REDIS_URL", "redis://127.0.0.1:6379/")
        .env("CPR_DATABASE_PASSWORD", POSTGRES_PASSWORD)
        .env("CPR_REDIS_PASSWORD", REDIS_PASSWORD)
        .status()
        .expect("run isolated environment override test");
    assert!(status.success());
}

#[test]
fn bootstrap_config_debug_should_redact_all_passwords() {
    let (config, _directory) = parse_config(&valid_config()).expect("config");
    let debug = format!("{config:?}");
    assert!(!debug.contains(POSTGRES_PASSWORD));
    assert!(!debug.contains(REDIS_PASSWORD));
    assert!(!debug.contains(ADMIN_PASSWORD));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn config_loader_should_reject_unknown_fields() {
    assert_rejected(format!(
        "{}\nunknown_terminal_field: true\n",
        valid_config()
    ));
}

#[test]
fn config_loader_should_reject_removed_tls_section() {
    assert_rejected(valid_config().replace("openai:\n", "openai:\n  tls: {}\n"));
}

#[test]
fn config_loader_should_reject_missing_explicit_fields() {
    assert_rejected(valid_config().replace("  request_id_header: 'x-request-id'\n", ""));
}

#[test]
fn config_loader_should_reject_unsupported_schema_version() {
    assert_rejected(valid_config().replace("schema_version: 1", "schema_version: 2"));
}

#[test]
fn config_loader_should_reject_embedded_database_password() {
    assert_rejected(valid_config().replace(
        "postgres://codex_proxy@127.0.0.1:5432/codex_proxy",
        "postgres://codex_proxy:embedded@127.0.0.1:5432/codex_proxy",
    ));
}

#[test]
fn config_loader_should_reject_non_hex_postgres_password() {
    assert_rejected(valid_config().replace(POSTGRES_PASSWORD, &"g".repeat(48)));
}

#[test]
fn config_loader_should_reject_wrong_length_redis_password() {
    assert_rejected(valid_config().replace(REDIS_PASSWORD, "1234"));
}

#[test]
fn config_loader_should_reject_weak_admin_password() {
    assert_rejected(valid_config().replace(ADMIN_PASSWORD, "password"));
}

#[test]
fn config_loader_should_reject_admin_password_with_compose_interpolation() {
    assert_rejected(valid_config().replace(ADMIN_PASSWORD, "unsafe$password"));
}

#[test]
fn config_loader_should_reject_zero_client_session_ttl() {
    let mut config = valid_config_document();
    *config
        .pointer_mut("/client/session_ttl_minutes")
        .expect("example client session TTL") = serde_json::json!(0);
    assert_rejected(config.to_string());
}

#[test]
fn config_loader_should_default_missing_client_section() {
    let mut config = valid_config_document();
    config
        .as_object_mut()
        .expect("example config mapping")
        .remove("client")
        .expect("example client section");
    parse_config(&config.to_string()).expect("client defaults when the section is omitted");
}

#[test]
fn config_loader_should_reject_missing_client_session_ttl() {
    let mut config = valid_config_document();
    config["client"]
        .as_object_mut()
        .expect("example client mapping")
        .remove("session_ttl_minutes")
        .expect("example client session TTL");
    assert_rejected(config.to_string());
}

#[test]
fn config_loader_should_reject_removed_fingerprint_section() {
    assert_rejected(valid_config().replace(
        "openai:\n",
        "openai:\n  fingerprint:\n    browser: removed\n",
    ));
}

#[test]
fn config_loader_should_reject_invalid_codex_cli_version() {
    assert_rejected(valid_config().replace("codex_version: '0.153.4'", "codex_version: 'latest'"));
}

#[test]
fn config_loader_should_reject_disabled_all_log_outputs() {
    assert_rejected(
        valid_config()
            .replace("stdout: true", "stdout: false")
            .replace("enabled: true", "enabled: false"),
    );
}

#[test]
fn config_loader_should_reject_zero_server_port() {
    assert_rejected(valid_config().replace("port: 8080", "port: 0"));
}

#[test]
fn config_loader_should_reject_invalid_desktop_profile_fields() {
    assert_rejected(valid_config().replace("desktop_build: '8109'", "desktop_build: 'build'"));
}

#[test]
fn config_loader_should_accept_empty_and_custom_request_locations() {
    let original = valid_config();
    let omitted = original
        .lines()
        .filter(|line| !line.trim_start().starts_with("location:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(original, omitted);
    parse_config(&omitted).expect("location passthrough when omitted");
    let location_line = original
        .lines()
        .find(|line| line.trim_start().starts_with("location:"))
        .expect("example location");
    for empty in ["    location:", "    location: null", "    location: ~"] {
        parse_config(&original.replace(location_line, empty)).expect("empty YAML location");
    }
    let custom = original.replace(
        "location: null",
        "location: { country: 'NZ', region: 'Auckland', city: 'Auckland', timezone: 'Pacific/Auckland' }",
    );
    assert_ne!(original, custom);
    parse_config(&custom).expect("custom location from YAML");
}

#[test]
fn config_loader_should_reject_invalid_request_location_timezones() {
    let original = valid_config();
    let invalid = original.replace(
        "location: null",
        "location: { country: 'US', region: 'Ohio', city: 'Piketon', timezone: 'Not/A_Timezone' }",
    );
    assert_ne!(original, invalid);
    assert_rejected(invalid);
}

fn assert_rejected(config: String) {
    assert!(parse_config(&config).is_err());
}

fn valid_config() -> String {
    CONFIG_EXAMPLE
        .replacen(
            "password: &postgres_password ''",
            &format!("password: &postgres_password '{POSTGRES_PASSWORD}'"),
            1,
        )
        .replacen(
            "password: &redis_password ''",
            &format!("password: &redis_password '{REDIS_PASSWORD}'"),
            1,
        )
        .replace(
            "default_password: ''",
            &format!("default_password: '{ADMIN_PASSWORD}'"),
        )
}

fn valid_config_document() -> serde_json::Value {
    // 按字段修改样例，避免注释或排版变化让测试输入悄悄失效；JSON 仍可由 YAML 文件入口加载。
    config::Config::builder()
        .add_source(config::File::from_str(
            &valid_config(),
            config::FileFormat::Yaml,
        ))
        .build()
        .and_then(config::Config::try_deserialize)
        .expect("example config document")
}

fn parse_config(config: &str) -> Result<(GatewayConfig, tempfile::TempDir), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let deploy = directory.path().join("deploy");
    fs::create_dir(&deploy).map_err(|error| error.to_string())?;
    let path = deploy.join("config.yaml");
    fs::write(&path, config).map_err(|error| error.to_string())?;
    let mut config = config::Config::builder()
        .add_source(config::File::from(path).required(true))
        .build()
        .and_then(config::Config::try_deserialize::<GatewayConfig>)
        .map_err(|error| error.to_string())?;
    config
        .resolve_and_validate(&deploy)
        .map_err(|error| error.to_string())?;
    Ok((config, directory))
}

// 组合根连接真实 API decoder、OpenAI encoder 与 transport；不把测试所需的
// 具体 Provider 依赖倒灌进 API，也不把此协议验证宣称为存储/调度/计费验收。
const PI_WIRE_FIXTURE: &str = include_str!("fixtures/client_requests/pi-0.85.1.json");
const DESKTOP_WIRE_FIXTURE: &str =
    include_str!("fixtures/client_requests/desktop-26.908.40834.json");

type WireHeaders = tokio_tungstenite::tungstenite::http::HeaderMap;

#[derive(Clone, serde::Deserialize)]
struct ClientWireTurn {
    headers: Vec<(String, String)>,
    body: serde_json::Value,
    response_events: Vec<serde_json::Value>,
}

#[derive(Clone, serde::Deserialize)]
struct ClientWireSample {
    name: String,
    transport: String,
    prompt_kind: String,
    turns: Vec<ClientWireTurn>,
}

#[derive(serde::Deserialize)]
struct ClientWireFixture {
    samples: Vec<ClientWireSample>,
}

fn desktop_capture_profile() -> provider_openai::transport::profile::CodexWireProfile {
    let capture: serde_json::Value = serde_json::from_str(DESKTOP_WIRE_FIXTURE).unwrap();
    let config: provider_openai::config::CodexWireProfileConfig =
        serde_json::from_value(capture["profile"].clone()).unwrap();
    assert_eq!(config.originator, "Codex Desktop");
    assert_eq!(config.codex_version, "0.154.0-alpha.6.2");
    assert_eq!(config.desktop_version, "26.908.40834");
    assert_eq!(config.desktop_build, "8881");
    config.into()
}

#[tokio::test]
async fn pi_wire_replay_should_minimize_headers_and_preserve_tools_and_prompts() {
    let fixture: ClientWireFixture = serde_json::from_str(PI_WIRE_FIXTURE).unwrap();
    assert_eq!(fixture.samples.len(), 6);
    for sample in &fixture.samples {
        assert_eq!(sample.turns.len(), 2);
        let first = &sample.turns[0].body;
        let prompt = first["instructions"]
            .as_str()
            .unwrap_or_else(|| first["input"][0]["content"].as_str().unwrap());
        assert_eq!(
            prompt.contains("operating inside pi"),
            sample.prompt_kind == "default"
        );
        assert_eq!(
            prompt.contains("Pi documentation"),
            sample.prompt_kind == "default"
        );
        let second = sample.turns[1].body.to_string();
        assert!(second.contains("function_call_output"));
        assert!(second.contains("合成文件内容：hello pi"));
        for upstream_ws in [false, true] {
            for add_environment_headers in [false, true] {
                replay_client_wire(sample, upstream_ws, add_environment_headers).await;
            }
        }
    }
}

#[tokio::test]
async fn desktop_bundled_core_wire_replay_should_preserve_its_protocol_fields() {
    let fixture: ClientWireFixture = serde_json::from_str(DESKTOP_WIRE_FIXTURE).unwrap();
    assert_eq!(fixture.samples.len(), 2);
    for sample in &fixture.samples {
        // Desktop WS 样本含 generate=false 预热和 connection-local continuation，
        // 不强迫它走 HTTP；沿真实捕获的传输和帧顺序验证。
        replay_client_wire(sample, sample.transport == "websocket", false).await;
    }
}

async fn replay_client_wire(
    sample: &ClientWireSample,
    upstream_ws: bool,
    add_environment_headers: bool,
) {
    use gateway_api::openai::responses::{
        OpenAiRequestHeaders, decode_request_with_headers, decode_response_create_with_context,
    };
    use gateway_core::operation::Operation;
    use provider_openai::transport::profile::CodexWireProfileState;
    use provider_openai::transport::{
        CodexBackendClient, CodexRequestContext, CodexWebSocketPool, build_reqwest_client,
        encode_generate_request,
    };
    use serde_json::json;
    use std::{sync::Arc, time::Duration};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let turns = sample.turns.clone();
    let capture = tokio::spawn(async move {
        if upstream_ws {
            capture_wire_websocket(listener, turns).await
        } else {
            capture_wire_http(listener, turns).await
        }
    });
    let profile = desktop_capture_profile();
    let backend = CodexBackendClient::new(
        build_reqwest_client().unwrap(),
        format!("http://{address}/backend-api"),
        CodexWireProfileState::new(profile.clone()),
    )
    .with_websocket_pool(Arc::new(CodexWebSocketPool::new(Duration::from_secs(60))));
    let mut expectations = Vec::new();
    for (index, turn) in sample.turns.iter().enumerate() {
        let mut headers = WireHeaders::new();
        for (name, value) in &turn.headers {
            headers.append(
                name.parse::<tokio_tungstenite::tungstenite::http::HeaderName>()
                    .unwrap(),
                value.parse().unwrap(),
            );
        }
        let mut expected_body = turn.body.clone();
        if add_environment_headers {
            for name in [
                "X-Stainless-Runtime",
                "x-stainless-future-field",
                "Origin",
                "Referer",
                "Sec-Ch-Ua",
                "sec-ch-ua-platform",
                "Sec-Fetch-Site",
            ] {
                headers.append(name, "synthetic-environment".parse().unwrap());
                headers.append(name, "duplicate".parse().unwrap());
            }
            headers.append("x-future-business", "first".parse().unwrap());
            headers.append("x-future-business", "second".parse().unwrap());
            headers.insert("traceparent", "synthetic-trace".parse().unwrap());
            headers.insert("tracestate", "synthetic-state".parse().unwrap());
            expected_body["future_business"] = json!({"text": "中文 pi 扩展字段"});
        }
        let decoded = if sample.transport == "websocket" {
            decode_response_create_with_context(
                &expected_body.to_string(),
                &OpenAiRequestHeaders::from_headers(&headers),
            )
            .unwrap()
        } else {
            // fixture 保存解压后的正文；回放时重建原始入站压缩，而非移除压缩头。
            let bytes = serde_json::to_vec(&expected_body).unwrap();
            let bytes = if headers
                .get("content-encoding")
                .is_some_and(|value| value == "zstd")
            {
                zstd::stream::encode_all(bytes.as_slice(), 3).unwrap()
            } else {
                bytes
            };
            decode_request_with_headers(&bytes, &headers).unwrap()
        };
        let Operation::Generate(generate) = decoded.operation() else {
            panic!("Generate operation")
        };
        let mut request = encode_generate_request(generate, "gpt-5.5", None).unwrap();
        request.use_websocket = upstream_ws;
        request.force_http_sse = !upstream_ws;
        request.local_conversation_id = Some(sample.name.clone());
        let context = CodexRequestContext {
            session_id: request.client_session_id.as_deref(),
            thread_id: request.client_thread_id.as_deref(),
            client_request_id: request.client_request_id.as_deref(),
            turn_state: request.turn_state.as_deref(),
            turn_metadata: request.turn_metadata.as_deref(),
            beta_features: request.beta_features.as_deref(),
            codex_window_id: request.codex_window_id.as_deref(),
            parent_thread_id: request.parent_thread_id.as_deref(),
            ..CodexRequestContext::auxiliary(
                "Bearer synthetic-upstream-token",
                Some("synthetic-upstream-account"),
                "synthetic-request",
                None,
            )
        };
        let response_body = tokio::time::timeout(Duration::from_secs(10), async {
            use futures::StreamExt as _;
            let mut response = backend
                .create_response_stream_with_pool_account(
                    &request,
                    context,
                    Some("synthetic-upstream-account"),
                )
                .await
                .unwrap_or_else(|error| {
                    panic!("{} turn {index}, WS={upstream_ws}: {error}", sample.name)
                });
            let mut bytes = Vec::new();
            while let Some(chunk) = response.body.next().await {
                bytes.extend_from_slice(&chunk.unwrap());
            }
            String::from_utf8(bytes).unwrap()
        })
        .await
        .unwrap();
        for event in &turn.response_events {
            assert!(
                response_body.contains(&event.to_string()),
                "lost event in {}",
                sample.name
            );
        }
        expected_body.as_object_mut().unwrap().remove("type");
        expected_body["stream"] = json!(true);
        for name in ["max_output_tokens", "temperature"] {
            expected_body.as_object_mut().unwrap().remove(name);
        }
        expectations.push((
            expected_body,
            request.client_session_id.clone(),
            request.client_thread_id.clone(),
        ));
    }
    let observed = tokio::time::timeout(Duration::from_secs(10), capture)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.len(), expectations.len());
    for ((headers, mut body), (mut expected, session, thread)) in
        observed.into_iter().zip(expectations)
    {
        for name in headers.keys() {
            let name = name.as_str();
            assert!(
                !name.starts_with("x-stainless-")
                    && !name.starts_with("sec-ch-ua")
                    && !name.starts_with("sec-fetch-")
                    && !matches!(name, "origin" | "referer" | "session_id"),
                "leaked {name} in {}",
                sample.name
            );
        }
        assert_eq!(headers["user-agent"], profile.user_agent());
        assert_eq!(headers["originator"], "Codex Desktop");
        assert_eq!(headers["version"], profile.codex_version);
        assert_eq!(headers["authorization"], "Bearer synthetic-upstream-token");
        assert_eq!(headers["chatgpt-account-id"], "synthetic-upstream-account");
        for (name, value) in [("session-id", session), ("thread-id", thread)] {
            assert_eq!(
                headers.get(name).map(|v| v.to_str().unwrap()),
                value.as_deref()
            );
            assert_eq!(
                headers.get_all(name).iter().count(),
                usize::from(value.is_some())
            );
        }
        if add_environment_headers {
            assert_eq!(
                headers
                    .get_all("x-future-business")
                    .iter()
                    .map(|v| v.to_str().unwrap())
                    .collect::<Vec<_>>(),
                ["first", "second"]
            );
            assert_eq!(headers["traceparent"], "synthetic-trace");
            assert_eq!(headers["tracestate"], "synthetic-state");
        }
        if upstream_ws {
            assert_eq!(headers["openai-beta"], "responses_websockets=2026-02-06");
            assert_eq!(body["type"], "response.create");
            // 去掉观测帧的外层标记时保持顺序，避免测试自身的 swap_remove 重排字段。
            body.as_object_mut().unwrap().shift_remove("type");
            // WS transport 独占的发送时间戳每次生成；其余输入/工具/metadata 完整比较。
            let metadata = body["client_metadata"].as_object_mut().unwrap();
            assert!(
                metadata
                    .remove("x-codex-ws-stream-request-start-ms")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .parse::<u128>()
                    .unwrap()
                    > 0
            );
            if let Some(metadata) = expected
                .get_mut("client_metadata")
                .and_then(serde_json::Value::as_object_mut)
            {
                metadata.remove("x-codex-ws-stream-request-start-ms");
            }
            if expected.get("client_metadata").is_none() && body["client_metadata"] == json!({}) {
                body.as_object_mut().unwrap().remove("client_metadata");
            }
        } else {
            assert_eq!(headers["content-encoding"], "zstd");
        }
        assert_eq!(
            body.as_object().unwrap().keys().collect::<Vec<_>>(),
            expected.as_object().unwrap().keys().collect::<Vec<_>>(),
            "field order changed in {}",
            sample.name,
        );
        assert_eq!(body, expected, "body changed in {}", sample.name);
    }
}

async fn capture_wire_http(
    listener: tokio::net::TcpListener,
    turns: Vec<ClientWireTurn>,
) -> Vec<(WireHeaders, serde_json::Value)> {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut captured = Vec::new();
    for turn in turns {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            assert!(head.len() < 64 * 1024);
            head.push(socket.read_u8().await.unwrap());
        }
        let head = String::from_utf8(head).unwrap();
        assert!(head.starts_with("POST /backend-api/codex/responses HTTP/1.1\r\n"));
        let mut headers = WireHeaders::new();
        for line in head.split("\r\n").skip(1).filter(|line| !line.is_empty()) {
            let (name, value) = line.split_once(':').unwrap();
            headers.append(
                name.parse::<tokio_tungstenite::tungstenite::http::HeaderName>()
                    .unwrap(),
                value.trim().parse().unwrap(),
            );
        }
        let length: usize = headers["content-length"].to_str().unwrap().parse().unwrap();
        assert!(length < 4 * 1024 * 1024);
        let mut bytes = vec![0; length];
        socket.read_exact(&mut bytes).await.unwrap();
        let body = zstd::stream::decode_all(bytes.as_slice()).unwrap();
        captured.push((headers, serde_json::from_slice(&body).unwrap()));
        let body: String = turn
            .response_events
            .iter()
            .map(|event| {
                format!(
                    "event: {}\ndata: {event}\n\n",
                    event["type"].as_str().unwrap()
                )
            })
            .collect();
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    }
    captured
}

struct WireOpeningCapture(std::sync::Arc<std::sync::Mutex<WireHeaders>>);

impl tokio_tungstenite::tungstenite::handshake::server::Callback for WireOpeningCapture {
    fn on_request(
        self,
        request: &tokio_tungstenite::tungstenite::handshake::server::Request,
        mut response: tokio_tungstenite::tungstenite::handshake::server::Response,
    ) -> Result<
        tokio_tungstenite::tungstenite::handshake::server::Response,
        tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
    > {
        assert_eq!(request.uri().path(), "/backend-api/codex/responses");
        *self.0.lock().unwrap() = request.headers().clone();
        response.headers_mut().insert(
            "sec-websocket-extensions",
            "permessage-deflate".parse().unwrap(),
        );
        Ok(response)
    }
}

async fn capture_wire_websocket(
    listener: tokio::net::TcpListener,
    turns: Vec<ClientWireTurn>,
) -> Vec<(WireHeaders, serde_json::Value)> {
    use futures::{SinkExt as _, StreamExt as _};
    use std::sync::{Arc, Mutex};
    use tokio_tungstenite::{
        accept_hdr_async_with_config,
        tungstenite::{
            Message,
            extensions::{ExtensionsConfig, compression::deflate::DeflateConfig},
            protocol::WebSocketConfig,
        },
    };
    let (socket, _) = listener.accept().await.unwrap();
    let headers = Arc::new(Mutex::new(WireHeaders::new()));
    let opening = Arc::clone(&headers);
    let mut extensions = ExtensionsConfig::default();
    extensions.permessage_deflate = Some(DeflateConfig::default());
    let mut config = WebSocketConfig::default();
    config.extensions = extensions;
    let mut socket =
        accept_hdr_async_with_config(socket, WireOpeningCapture(opening), Some(config))
            .await
            .unwrap();
    let mut captured = Vec::new();
    for turn in turns {
        let frame = socket.next().await.unwrap().unwrap();
        captured.push((
            headers.lock().unwrap().clone(),
            serde_json::from_str(frame.to_text().unwrap()).unwrap(),
        ));
        for event in turn.response_events {
            socket
                .send(Message::Text(event.to_string().into()))
                .await
                .unwrap();
        }
    }
    captured
}
