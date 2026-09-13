//! Portal 登录会话。

use std::net::{IpAddr, SocketAddr};

use axum::{
    Router,
    extract::{FromRequestParts, State, connect_info::ConnectInfo},
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use gateway_portal::{
    PortalServices,
    model::{
        MutationActor, MutationContext,
        auth::{ChangePasswordCommand, LoginCommand, LoginError, PortalPrincipal},
    },
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tower_http::request_id::RequestId;

use super::{
    PortalError,
    extract::PortalJson,
    wire::{PortalEnvelope, PortalResponse, map_portal_error},
};

const SESSION_COOKIE: &str = "cpr_portal_session";
const COOKIE_ATTRS: &str = "Path=/; Secure; HttpOnly; SameSite=Lax";

pub trait PortalSessionState {
    fn portal_services(&self) -> &PortalServices;
    fn trusted_proxy_ips(&self) -> &[IpAddr];
}

pub struct PortalAuth {
    pub principal: PortalPrincipal,
    pub request_id: String,
}

impl<S> FromRequestParts<S> for PortalAuth
where
    S: PortalSessionState + Send + Sync,
{
    type Rejection = PortalError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        reject_cross_site(&parts.headers)?;
        let token = session_cookie(&parts.headers);
        let principal = state
            .portal_services()
            .auth()
            .validate_session(token.as_deref())
            .await
            .map_err(map_portal_error)?
            .ok_or_else(PortalError::session_required)?;
        let request_id = parts
            .extensions
            .get::<RequestId>()
            .map(RequestId::header_value)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("portal-request")
            .to_owned();
        Ok(Self {
            principal,
            request_id,
        })
    }
}

pub fn router<S>() -> Router<S>
where
    S: PortalSessionState + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/portal/auth/login", post(login::<S>))
        .route("/api/portal/auth/status", get(status::<S>))
        .route("/api/portal/auth/logout", post(logout::<S>))
        .route("/api/portal/auth/password", post(change_password::<S>))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

async fn change_password<S>(
    State(state): State<S>,
    auth: PortalAuth,
    headers: HeaderMap,
    connect_info: Option<axum::extract::Extension<ConnectInfo<SocketAddr>>>,
    PortalJson(payload): PortalJson<ChangePasswordRequest>,
) -> Result<Response, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let context = MutationContext {
        actor: MutationActor::PortalSession {
            user_id: auth.principal.user_id.clone(),
        },
        request_id: auth.request_id,
    };
    state
        .portal_services()
        .auth()
        .change_password(
            &context,
            ChangePasswordCommand {
                principal: auth.principal,
                current_password: SecretString::from(payload.current_password),
                new_password: SecretString::from(payload.new_password),
                client_ip: client_ip(
                    &headers,
                    connect_info.map(|axum::extract::Extension(ConnectInfo(address))| address),
                    state.trusted_proxy_ips(),
                ),
            },
        )
        .await
        .map_err(map_portal_error)?;
    let mut response =
        PortalResponse::new(StatusCode::OK, PortalEnvelope::ok(serde_json::json!({})))
            .into_response();
    let cookie = format!("{SESSION_COOKIE}=; {COOKIE_ATTRS}; Max-Age=0");
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| PortalError::internal())?,
    );
    Ok(response)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginData {
    expires_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusData {
    authenticated: bool,
}

async fn login<S>(
    State(state): State<S>,
    headers: HeaderMap,
    connect_info: Option<axum::extract::Extension<ConnectInfo<SocketAddr>>>,
    PortalJson(payload): PortalJson<LoginRequest>,
) -> Result<Response, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    reject_cross_site(&headers)?;
    let result = state
        .portal_services()
        .auth()
        .login(LoginCommand {
            username: payload.username,
            password: SecretString::from(payload.password),
            client_ip: client_ip(
                &headers,
                connect_info.map(|axum::extract::Extension(ConnectInfo(address))| address),
                state.trusted_proxy_ips(),
            ),
        })
        .await
        .map_err(map_login_error)?;
    let mut response = PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(LoginData {
            expires_at: result.expires_at.to_rfc3339(),
        }),
    )
    .into_response();
    let cookie = format!("{SESSION_COOKIE}={}; {COOKIE_ATTRS}", result.session_token);
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| PortalError::internal())?,
    );
    Ok(response)
}

async fn status<S>(
    State(state): State<S>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let authenticated = state
        .portal_services()
        .auth()
        .validate_session(session_cookie(&headers).as_deref())
        .await
        .map_err(map_portal_error)?
        .is_some();
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(StatusData { authenticated }),
    ))
}

async fn logout<S>(State(state): State<S>, headers: HeaderMap) -> Result<Response, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    reject_cross_site(&headers)?;
    if let Some(token) = session_cookie(&headers) {
        let _ = state.portal_services().auth().logout(&token).await;
    }
    let mut response =
        PortalResponse::new(StatusCode::OK, PortalEnvelope::ok(serde_json::json!({})))
            .into_response();
    let cookie = format!("{SESSION_COOKIE}=; {COOKIE_ATTRS}; Max-Age=0");
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| PortalError::internal())?,
    );
    Ok(response)
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get("cookie")?.to_str().ok()?;
    cookie.split(';').find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        (name == SESSION_COOKIE).then(|| value.to_owned())
    })
}

fn client_ip(headers: &HeaderMap, peer: Option<SocketAddr>, trusted: &[IpAddr]) -> String {
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

fn reject_cross_site(headers: &HeaderMap) -> Result<(), PortalError> {
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("cross-site"))
    {
        return Err(PortalError::csrf_rejected());
    }
    let Some(origin) = headers.get(axum::http::header::ORIGIN) else {
        return Ok(());
    };
    let origin = origin.to_str().map_err(|_| PortalError::csrf_rejected())?;
    let Some(host) = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(PortalError::csrf_rejected());
    };
    if !origin_matches_host(origin, host) {
        return Err(PortalError::csrf_rejected());
    }
    Ok(())
}

fn origin_matches_host(origin: &str, host: &str) -> bool {
    let Ok(url) = url::Url::parse(origin) else {
        return false;
    };
    if url.scheme() != "http" && url.scheme() != "https" {
        return false;
    }
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return false;
    }
    let Some(origin_host) = url.host_str() else {
        return false;
    };
    let Some(origin_port) = url.port_or_known_default() else {
        return false;
    };
    let Some((host_name, host_port)) = parse_host_header(host) else {
        return false;
    };
    if !origin_host.eq_ignore_ascii_case(&host_name) {
        return false;
    }
    match host_port {
        Some(port) => origin_port == port,
        None => origin_port == 80 || origin_port == 443,
    }
}

fn parse_host_header(host: &str) -> Option<(String, Option<u16>)> {
    let host = host.trim();
    if let Some(rest) = host.strip_prefix('[') {
        let (name, remainder) = rest.split_once(']')?;
        if name.is_empty() {
            return None;
        }
        if remainder.is_empty() {
            return Some((name.to_owned(), None));
        }
        let port = remainder.strip_prefix(':')?.parse().ok()?;
        return Some((name.to_owned(), Some(port)));
    }
    if let Some((name, port)) = host.rsplit_once(':')
        && !name.is_empty()
        && port.chars().all(|ch| ch.is_ascii_digit())
    {
        return Some((name.to_owned(), port.parse().ok()));
    }
    if host.is_empty() {
        return None;
    }
    Some((host.to_owned(), None))
}

fn map_login_error(error: LoginError) -> PortalError {
    match error {
        LoginError::InvalidCredentials => PortalError::invalid_credentials(),
        LoginError::RateLimited => PortalError::too_many_logins(),
        LoginError::Unavailable => PortalError::internal(),
    }
}
