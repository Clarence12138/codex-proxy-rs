//! 下游请求头的透传规则；只作用于请求，不处理上游响应诊断头。

use base64::{Engine as _, engine::general_purpose::STANDARD};
use gateway_protocol::openai::is_transport_managed_request_header;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Map, Value};

const PASSTHROUGH_HEADERS_CONTEXT_KEY: &str = "opaque_request_headers";

// 协议上下文只在这里解码为可透传头，HTTP/SSE 与 WebSocket 共用同一边界。
pub(in crate::transport) fn decode_passthrough_headers(context: &Map<String, Value>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let Some(entries) = context
        .get(PASSTHROUGH_HEADERS_CONTEXT_KEY)
        .and_then(Value::as_array)
    else {
        return headers;
    };

    for entry in entries {
        let Some(entry) = entry.as_array().filter(|entry| entry.len() == 2) else {
            continue;
        };
        let Some(name) = entry.first().and_then(Value::as_str) else {
            continue;
        };
        let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        if is_filtered_header(name.as_str()) {
            continue;
        }
        let Some(encoded) = entry.get(1).and_then(Value::as_str) else {
            continue;
        };
        let Ok(bytes) = STANDARD.decode(encoded) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_bytes(&bytes) else {
            continue;
        };
        headers.append(name, value);
    }
    headers
}

fn is_filtered_header(name: &str) -> bool {
    is_transport_managed_request_header(name)
        // 反代元数据只描述下游链路，未知命名空间扩展也不能继承到上游。
        || name.starts_with("cf-")
        || name.starts_with("x-forwarded-")
        // SDK、浏览器与其他 Provider 的私有环境不属于 OpenAI 请求事实。
        || name.starts_with("x-stainless-")
        || name.starts_with("sec-ch-ua")
        || name.starts_with("sec-fetch-")
        || name.starts_with("x-grok-")
        || name.starts_with("x-xai-")
        || matches!(
            name,
            "forwarded"
                | "via"
                | "cdn-loop"
                | "x-real-ip"
                | "true-client-ip"
                | "origin"
                | "referer"
                // API 已提取会话语义；别名不再成为第二份上游会话头。
                | "session_id"
                // 即使绕过 API 直接提供协议上下文，也不能覆盖服务端账号身份。
                | "authorization"
                | "x-api-key"
                | "x-openai-actor-authorization"
                | "cookie"
                | "cookie2"
                | "chatgpt-account-id"
                | "chatgpt-organization-id"
                | "chatgpt-org-id"
                | "chatgpt-project-id"
                | "openai-organization"
                | "openai-project"
                | "x-openai-organization"
                | "x-openai-project"
                | "x-codex-installation-id"
        )
}
