//! 控制面统一登录、会话恢复与退出；身份由认证用例返回。

use std::{
    fmt,
    net::{IpAddr, SocketAddr},
};

use axum::{
    Router,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use gateway_admin::{
    AdminServices,
    model::auth::{AuthSession, LoginCommand, LoginError, SessionSubject},
};
use serde::{Deserialize, Serialize};

use crate::{
    admin::{AdminEnvelope, AdminError, AdminJson, AdminResponse, wire::map_admin_service_error},
    session_cookie,
};

/// 控制面 HTTP adapter 消费同一组用例；权限由各入口服务端校验。
pub trait SessionState {
    fn admin_services(&self) -> &AdminServices;
    fn trusted_proxy_ips(&self) -> &[IpAddr] {
        &[]
    }
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum LoginRequest {
    Admin {
        username: Option<String>,
        password: String,
    },
    Key {
        #[serde(rename = "apiKey")]
        api_key: String,
    },
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admin { username, .. } => formatter
                .debug_struct("AdminLogin")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
            Self::Key { .. } => formatter
                .debug_struct("KeyLogin")
                .field("api_key", &"[REDACTED]")
                .finish(),
        }
    }
}

impl From<LoginRequest> for LoginCommand {
    fn from(request: LoginRequest) -> Self {
        match request {
            LoginRequest::Admin { username, password } => Self::Admin { username, password },
            LoginRequest::Key { api_key } => Self::Key { api_key },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionData {
    role: &'static str,
    expires_at: String,
}

impl From<&AuthSession> for SessionData {
    fn from(session: &AuthSession) -> Self {
        Self {
            role: match session.subject {
                SessionSubject::Admin { .. } => "admin",
                SessionSubject::Key { .. } => "key",
            },
            expires_at: session.expires_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
struct SessionStatusData {
    authenticated: bool,
    session: Option<SessionData>,
}

#[derive(Debug, Serialize)]
struct LogoutData {
    message: &'static str,
}

pub(crate) fn router<S>() -> Router<S>
where
    S: SessionState + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/auth/login", post(login::<S>))
        .route("/api/auth/status", get(session_status::<S>))
        .route("/api/auth/logout", post(logout::<S>))
        .route("/api/auth", any(not_found))
        .route("/api/auth/{*path}", any(not_found))
        .method_not_allowed_fallback(method_not_allowed)
        .layer(middleware::map_response(no_store))
}

async fn login<S>(
    State(state): State<S>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    AdminJson(payload): AdminJson<LoginRequest>,
) -> Result<Response, AdminError>
where
    S: SessionState + Send + Sync,
{
    let result = state
        .admin_services()
        .auth()
        .login(
            payload.into(),
            client_ip(&headers, Some(peer), state.trusted_proxy_ips())
                .parse()
                .unwrap_or_else(|_| peer.ip()),
            session_cookie::value(&headers).as_deref(),
        )
        .await
        .map_err(map_login_error)?;
    let max_age = result
        .session
        .expires_at
        .signed_duration_since(chrono::Utc::now())
        .num_seconds()
        .max(1);
    let expires = result
        .session
        .expires_at
        .format("%a, %d %b %Y %H:%M:%S GMT");
    let cookie = format!(
        "{}={}; {}; Max-Age={max_age}; Expires={expires}",
        session_cookie::NAME,
        result.session_id,
        session_cookie::attributes(&headers)
    );
    let mut response = AdminResponse::new(
        StatusCode::OK,
        AdminEnvelope::ok(SessionData::from(&result.session)),
    )
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| AdminError::internal())?,
    );
    Ok(response)
}

async fn session_status<S>(
    State(state): State<S>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AdminError>
where
    S: SessionState + Send + Sync,
{
    let session = state
        .admin_services()
        .auth()
        .session(session_cookie::value(&headers).as_deref())
        .await
        .map_err(map_admin_service_error)?;
    Ok(AdminResponse::new(
        StatusCode::OK,
        AdminEnvelope::ok(SessionStatusData {
            authenticated: session.is_some(),
            session: session.as_ref().map(SessionData::from),
        }),
    ))
}

async fn logout<S>(State(state): State<S>, headers: HeaderMap) -> Result<Response, AdminError>
where
    S: SessionState + Send + Sync,
{
    if let Some(session_id) = session_cookie::value(&headers) {
        state
            .admin_services()
            .auth()
            .logout(&session_id)
            .await
            .map_err(map_admin_service_error)?;
    }
    let mut response = AdminResponse::new(
        StatusCode::OK,
        AdminEnvelope::ok(LogoutData {
            message: "Logged out successfully",
        }),
    )
    .into_response();
    let cookie = format!(
        "{}=; {}; Max-Age=0",
        session_cookie::NAME,
        session_cookie::attributes(&headers)
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| AdminError::internal())?,
    );
    Ok(response)
}

fn map_login_error(error: LoginError) -> AdminError {
    match error {
        LoginError::InvalidCredentials => AdminError::invalid_credentials(),
        LoginError::TooManyAttempts {
            retry_after_seconds,
        } => AdminError::too_many_login_attempts().with_retry_after(retry_after_seconds),
        LoginError::Unavailable => AdminError::service_unavailable(),
    }
}

async fn method_not_allowed() -> AdminError {
    AdminError::method_not_allowed()
}
async fn not_found() -> AdminError {
    AdminError::not_found("认证接口不存在")
}
async fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(crate) fn client_ip(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    trusted: &[IpAddr],
) -> String {
    let Some(peer) = peer.map(|address| address.ip().to_canonical()) else {
        return "unknown".to_owned();
    };
    let is_trusted = |ip: IpAddr| trusted.iter().any(|proxy| proxy.to_canonical() == ip);
    if !is_trusted(peer) {
        return peer.to_string();
    }
    // 先完整校验有界链，避免从损坏或截断的头部猜测地址；重复头按到达顺序组合。
    let Some(chain) = forwarded_chain(headers) else {
        return peer.to_string();
    };
    let mut client = peer;
    for address in chain.into_iter().rev() {
        // 与 Nginx real_ip_recursive 一致：只有当前一跳可信，才采纳其左侧地址。
        if !is_trusted(client) {
            break;
        }
        client = address;
    }
    client.to_string()
}

fn forwarded_chain(headers: &HeaderMap) -> Option<Vec<IpAddr>> {
    const MAX_BYTES: usize = 8 * 1024;
    const MAX_HOPS: usize = 32;
    let mut bytes = 0_usize;
    let mut chain = Vec::new();
    for header in headers.get_all("x-forwarded-for") {
        bytes = bytes.checked_add(header.as_bytes().len())?;
        if bytes > MAX_BYTES {
            return None;
        }
        for part in header.to_str().ok()?.split(',') {
            if chain.len() >= MAX_HOPS {
                return None;
            }
            chain.push(part.trim().parse::<IpAddr>().ok()?.to_canonical());
        }
    }
    Some(chain)
}
