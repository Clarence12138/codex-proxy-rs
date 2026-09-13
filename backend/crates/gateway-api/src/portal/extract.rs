//! Portal JSON extractor。

use axum::{
    Json,
    extract::{FromRequest, Request, rejection::JsonRejection},
    http::StatusCode,
};
use serde::de::DeserializeOwned;

use super::PortalError;

pub struct PortalJson<T>(pub T);

impl<T, S> FromRequest<S> for PortalJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = PortalError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        <Json<T> as FromRequest<S>>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(map_json_rejection)
    }
}

fn map_json_rejection(rejection: JsonRejection) -> PortalError {
    match rejection {
        JsonRejection::JsonSyntaxError(_) => PortalError::malformed_json(),
        JsonRejection::JsonDataError(_) => {
            PortalError::invalid_request(StatusCode::UNPROCESSABLE_ENTITY, "请求字段不合法")
        }
        JsonRejection::MissingJsonContentType(_) => PortalError::invalid_request(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "请求必须使用 application/json",
        ),
        _ => PortalError::internal(),
    }
}
