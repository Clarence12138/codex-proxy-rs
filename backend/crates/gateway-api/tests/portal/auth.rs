use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::json;
use tower::ServiceExt;

use super::portal_router;

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
