//! Portal HTTP adapter。

use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
    response::Response,
    routing::any,
};

pub mod admin;
mod auth;
mod extract;
mod keys;
mod me;
mod usage;
mod wire;

pub use auth::{PortalAuth, PortalSessionState};
pub use wire::PortalError;

/// 构造 `/api/portal` 与管理员用户/套餐路由。
pub fn router<S>() -> Router<S>
where
    S: PortalSessionState + crate::auth::SessionState + Clone + Send + Sync + 'static,
{
    Router::new()
        .merge(auth::router::<S>())
        .merge(me::router::<S>())
        .merge(keys::router::<S>())
        .merge(usage::router::<S>())
        .merge(admin::router::<S>())
        .method_not_allowed_fallback(method_not_allowed)
        .route("/api/portal", any(portal_not_found))
        .route("/api/portal/{*path}", any(portal_not_found))
        .layer(middleware::map_response(no_store))
}

async fn method_not_allowed() -> PortalError {
    PortalError::method_not_allowed()
}

async fn portal_not_found() -> PortalError {
    PortalError::route_not_found()
}

async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    response
}
