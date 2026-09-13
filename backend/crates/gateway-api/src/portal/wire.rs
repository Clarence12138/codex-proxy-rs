//! Portal HTTP 信封。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use gateway_portal::model::{PortalError as DomainError, PortalErrorKind};
use serde::{Deserialize, Serialize};

pub const PORTAL_OK_CODE: u32 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalEnvelope<T> {
    pub code: u32,
    pub message: String,
    pub data: T,
}

impl<T> PortalEnvelope<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: PORTAL_OK_CODE,
            message: "OK".to_owned(),
            data,
        }
    }
}

pub struct PortalResponse<T> {
    status: StatusCode,
    body: PortalEnvelope<T>,
}

impl<T> PortalResponse<T> {
    pub fn new(status: StatusCode, body: PortalEnvelope<T>) -> Self {
        Self { status, body }
    }
}

impl<T: Serialize> IntoResponse for PortalResponse<T> {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

pub struct PortalError {
    status: StatusCode,
    code: u32,
    message: String,
}

impl PortalError {
    fn new(status: StatusCode, code: u32, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn malformed_json() -> Self {
        Self::new(StatusCode::BAD_REQUEST, 40000, "请求体不是合法 JSON")
    }

    pub fn invalid_request(status: StatusCode, message: &'static str) -> Self {
        Self::new(status, 40001, message)
    }

    pub fn session_required() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, 40101, "需要登录")
    }

    pub fn invalid_credentials() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, 40102, "用户名或密码错误")
    }

    pub fn too_many_logins() -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, 42901, "登录尝试过多")
    }

    pub fn route_not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, 40401, "接口不存在")
    }

    pub fn method_not_allowed() -> Self {
        Self::new(StatusCode::METHOD_NOT_ALLOWED, 40001, "请求方法不允许")
    }

    pub fn csrf_rejected() -> Self {
        Self::new(StatusCode::FORBIDDEN, 40301, "跨站请求被拒绝")
    }

    pub fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, 50001, "服务内部错误")
    }
}

impl IntoResponse for PortalError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "code": self.code,
                "message": self.message,
                "data": null
            })),
        )
            .into_response()
    }
}

pub fn map_portal_error(error: DomainError) -> PortalError {
    let (status, code) = match error.kind() {
        PortalErrorKind::Invalid => (StatusCode::BAD_REQUEST, 40001),
        PortalErrorKind::Unauthorized => (StatusCode::UNAUTHORIZED, 40101),
        PortalErrorKind::NotFound => (StatusCode::NOT_FOUND, 40401),
        PortalErrorKind::Conflict => (StatusCode::CONFLICT, 40901),
        PortalErrorKind::RateLimited => (StatusCode::TOO_MANY_REQUESTS, 42901),
        PortalErrorKind::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, 50301),
        PortalErrorKind::Internal => (StatusCode::INTERNAL_SERVER_ERROR, 50001),
    };
    PortalError::new(status, code, error.message().to_owned())
}
