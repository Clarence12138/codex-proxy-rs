use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use serde_json::json;
use tower::ServiceExt;

use super::portal_router;

#[tokio::test]
async fn invalid_plan_names_return_input_errors_instead_of_dependency_failures() {
    let app = portal_router().await;
    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"username":"admin_1","password":"strong-admin-password"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    for (name, message) in [
        ("".to_owned(), "请输入套餐名称"),
        ("   ".to_owned(), "请输入套餐名称"),
        ("中".repeat(22), "套餐名称不能超过 64 个 UTF-8 字节"),
        ("a\nb".to_owned(), "套餐名称不能包含控制字符"),
    ] {
        for action in ["create", "update"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/api/admin/portal/plans/{action}"))
                        .header(header::COOKIE, &cookie)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({"id":"plan_test","name":name,"groupIds":["group_test"]})
                                .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap())
                    .unwrap();
            assert_eq!(body["code"], 40001);
            assert_eq!(body["message"], message);
        }
    }
}
