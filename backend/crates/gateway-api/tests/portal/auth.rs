use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::json;
use tower::ServiceExt;

use super::portal_router;

#[derive(Clone)]
struct AuthenticatedState(gateway_portal::PortalServices);

impl gateway_api::portal::PortalSessionState for AuthenticatedState {
    fn trusted_proxy_ips(&self) -> &[std::net::IpAddr] {
        &[]
    }

    fn portal_services(&self) -> &gateway_portal::PortalServices {
        &self.0
    }
}

impl gateway_api::auth::SessionState for AuthenticatedState {
    fn admin_services(&self) -> &gateway_admin::AdminServices {
        unreachable!("password tests never access admin services")
    }
}

struct SessionOnlyStore;

#[async_trait::async_trait]
impl gateway_portal::ports::store::PortalAuthStore for SessionOnlyStore {
    async fn load_password_hash(
        &self,
        _: &str,
    ) -> gateway_portal::ports::store::PortalStoreResult<Option<(String, String, String)>> {
        unreachable!("invalid inputs must be rejected before credential lookup")
    }
    async fn store_session(
        &self,
        _: &gateway_portal::model::auth::PortalSession,
        _: &str,
    ) -> gateway_portal::ports::store::PortalStoreResult<()> {
        unreachable!()
    }
    async fn load_session_by_token_hash(
        &self,
        _: &str,
        _: chrono::DateTime<chrono::Utc>,
    ) -> gateway_portal::ports::store::PortalStoreResult<
        Option<gateway_portal::model::auth::PortalPrincipal>,
    > {
        Ok(Some(gateway_portal::model::auth::PortalPrincipal {
            user_id: "usr_self".to_owned(),
            username: "self".to_owned(),
        }))
    }
    async fn delete_session_by_token_hash(
        &self,
        _: &str,
    ) -> gateway_portal::ports::store::PortalStoreResult<()> {
        unreachable!()
    }
    async fn delete_sessions_for_user(
        &self,
        _: &str,
    ) -> gateway_portal::ports::store::PortalStoreResult<()> {
        unreachable!()
    }
    async fn change_password(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &gateway_portal::model::MutationContext,
    ) -> gateway_portal::ports::store::PortalStoreResult<()> {
        unreachable!()
    }
}

#[tokio::test]
async fn authenticated_password_change_rejects_target_user_and_invalid_length() {
    let state = AuthenticatedState(crate::support::services_with_auth(std::sync::Arc::new(
        SessionOnlyStore,
    )));
    let router = gateway_api::portal::router().with_state(state);
    for (payload, expected) in [
        // 未知字段沿用 PortalJson 的 422 合同；业务长度错误为 400。
        (
            json!({"currentPassword":"old-password", "newPassword":"123456", "userId":"usr_other"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({"currentPassword":"old-password", "newPassword":"12345"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"currentPassword":"old-password", "newPassword":"a".repeat(257)}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/portal/auth/password")
                    .header(header::HOST, "localhost:8080")
                    .header(header::ORIGIN, "http://localhost:8080")
                    .header(header::COOKIE, "cpr_portal_session=test-session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }
}

#[tokio::test]
async fn password_change_requires_portal_session_and_same_origin() {
    for (origin, cookie, expected) in [
        ("http://localhost:8080", "", StatusCode::UNAUTHORIZED),
        (
            "http://localhost:8080",
            "cpr_admin_session=not-portal",
            StatusCode::UNAUTHORIZED,
        ),
        (
            "http://localhost:8080",
            "cpr_portal_session=expired",
            StatusCode::UNAUTHORIZED,
        ),
        ("https://evil.example", "", StatusCode::FORBIDDEN),
    ] {
        let response = portal_router()
            .await
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/portal/auth/password")
                    .header(header::HOST, "localhost:8080")
                    .header(header::ORIGIN, origin)
                    .header(header::COOKIE, cookie)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"currentPassword":"old-password", "newPassword":"123456"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
    }
}

async fn proxy_router(trusted: &[&str]) -> axum::Router {
    crate::openai::api_router_with_trusted_proxies(
        std::sync::Arc::new(super::UnusedExecution),
        trusted.iter().map(|ip| ip.parse().unwrap()).collect(),
    )
    .await
}

fn proxy_login(peer: Option<&str>, forwarded: &[&str], username: &str) -> Request<Body> {
    let mut request = login_request(Some("https://portal.example"), "portal.example");
    *request.body_mut() =
        Body::from(json!({ "username": username, "password": "wrong-password-12" }).to_string());
    if let Some(peer) = peer {
        request.extensions_mut().insert(axum::extract::ConnectInfo(
            peer.parse::<std::net::SocketAddr>().unwrap(),
        ));
    }
    for value in forwarded {
        request
            .headers_mut()
            .append("x-forwarded-for", value.parse().unwrap());
    }
    request
}

async fn exhaust_ip(app: &axum::Router, peer: Option<&str>, forwarded: &[&str]) {
    for i in 0..8 {
        let response = app
            .clone()
            .oneshot(proxy_login(peer, forwarded, &format!("unknown-{i}")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn trusted_proxy_separates_clients_but_preserves_ip_and_username_limits() {
    let app = proxy_router(&["127.0.0.1"]).await;
    let peer = Some("127.0.0.1:1000");
    exhaust_ip(&app, peer, &["192.0.2.1"]).await;
    let denied = app
        .clone()
        .oneshot(proxy_login(peer, &["192.0.2.1"], "another-user"))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::TOO_MANY_REQUESTS);
    let independent = app
        .clone()
        .oneshot(proxy_login(peer, &["192.0.2.2"], "another-user"))
        .await
        .unwrap();
    assert_eq!(independent.status(), StatusCode::UNAUTHORIZED);
    for i in 1..8 {
        let response = app
            .clone()
            .oneshot(proxy_login(
                peer,
                &[&format!("192.0.2.{}", i + 2)],
                "another-user",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let denied = app
        .oneshot(proxy_login(peer, &["192.0.2.100"], "another-user"))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn untrusted_peer_cannot_rotate_forwarded_headers_to_evade_throttle() {
    let app = proxy_router(&["127.0.0.1"]).await;
    let peer = Some("192.0.2.10:1000");
    exhaust_ip(&app, peer, &["198.51.100.1"]).await;
    let response = app
        .oneshot(proxy_login(peer, &["198.51.100.2"], "new-user"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn forwarded_chains_are_bounded_validated_and_walked_from_the_trusted_right_edge() {
    let too_many = vec!["192.0.2.1"; 33].join(",");
    let too_long = format!("{}192.0.2.1", " ".repeat(8192));
    let cases: Vec<(Option<&str>, Vec<&str>, &str)> = vec![
        (
            Some("127.0.0.1:1000"),
            vec!["198.51.100.99, 192.0.2.1", "10.0.0.1"],
            "192.0.2.1",
        ),
        (
            Some("[::ffff:127.0.0.1]:1000"),
            vec!["::ffff:192.0.2.1"],
            "192.0.2.1",
        ),
        (Some("[::1]:1000"), vec!["2001:db8::1"], "2001:db8::1"),
        (Some("127.0.0.1:1000"), vec!["bad, 192.0.2.1"], "127.0.0.1"),
        (Some("127.0.0.1:1000"), vec!["192.0.2.1,"], "127.0.0.1"),
        (Some("127.0.0.1:1000"), vec![&too_many], "127.0.0.1"),
        (Some("127.0.0.1:1000"), vec![&too_long], "127.0.0.1"),
        (Some("127.0.0.1:1000"), vec![], "127.0.0.1"),
    ];
    for (peer, headers, expected) in cases {
        let app = proxy_router(&["127.0.0.1", "10.0.0.1", "::1"]).await;
        exhaust_ip(&app, peer, &headers).await;
        let response = app
            .oneshot(proxy_login(
                Some("127.0.0.1:2000"),
                &[expected],
                "fresh-user",
            ))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::TOO_MANY_REQUESTS,
            "{headers:?}"
        );
    }
    let app = proxy_router(&["127.0.0.1"]).await;
    exhaust_ip(&app, None, &["192.0.2.1"]).await;
    let response = app
        .oneshot(proxy_login(None, &["192.0.2.2"], "fresh-user"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[test]
fn trusted_proxy_configuration_is_opt_in_and_requires_ip_literals() {
    let mut config = json!({
        "asset_directory": "/tmp/web", "cors_allowed_origins": [],
        "request_timeout_seconds": null, "request_id_header": "x-request-id"
    });
    let legacy: gateway_api::ApiConfig = serde_json::from_value(config.clone()).unwrap();
    assert!(legacy.trusted_proxy_ips.is_empty());
    config["trusted_proxy_ips"] = json!(["127.0.0.1", "::1", "::ffff:127.0.0.1"]);
    assert!(serde_json::from_value::<gateway_api::ApiConfig>(config.clone()).is_ok());
    for invalid in ["localhost", "10.0.0.0/8", "bad"] {
        config["trusted_proxy_ips"] = json!([invalid]);
        assert!(serde_json::from_value::<gateway_api::ApiConfig>(config.clone()).is_err());
    }
}

fn login_request(origin: Option<&str>, host: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/portal/auth/login")
        .header(header::HOST, host)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    builder
        .body(Body::from(
            json!({ "username": "nobody", "password": "wrong-password-12" }).to_string(),
        ))
        .expect("login request")
}

#[tokio::test]
async fn portal_login_rejects_cross_origin_and_non_default_port() {
    let response = portal_router()
        .await
        .oneshot(login_request(
            Some("https://evil.example"),
            "localhost:8080",
        ))
        .await
        .expect("cross origin");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = portal_router()
        .await
        .oneshot(login_request(Some("https://localhost:444"), "localhost"))
        .await
        .expect("wrong port");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = portal_router()
        .await
        .oneshot(login_request(
            Some("https://user@localhost:8080"),
            "localhost:8080",
        ))
        .await
        .expect("userinfo");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn portal_login_accepts_same_origin_then_rejects_credentials() {
    let response = portal_router()
        .await
        .oneshot(login_request(
            Some("http://localhost:8080"),
            "localhost:8080",
        ))
        .await
        .expect("same origin");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn portal_logout_rejects_cross_origin() {
    let response = portal_router()
        .await
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/portal/auth/logout")
                .header(header::HOST, "localhost:8080")
                .header(header::ORIGIN, "https://evil.example")
                .body(Body::empty())
                .expect("logout"),
        )
        .await
        .expect("logout response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn portal_me_requires_session_and_ignores_admin_cookie() {
    let unauthorized = portal_router()
        .await
        .oneshot(
            Request::builder()
                .uri("/api/portal/me")
                .header(header::HOST, "localhost:8080")
                .body(Body::empty())
                .expect("me"),
        )
        .await
        .expect("me response");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let admin_cookie = portal_router()
        .await
        .oneshot(
            Request::builder()
                .uri("/api/portal/me")
                .header(header::HOST, "localhost:8080")
                .header(header::COOKIE, "cpr_admin_session=not-a-portal-session")
                .body(Body::empty())
                .expect("me with admin cookie"),
        )
        .await
        .expect("admin cookie response");
    assert_eq!(admin_cookie.status(), StatusCode::UNAUTHORIZED);
}
