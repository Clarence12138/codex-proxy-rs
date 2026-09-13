//! Portal 登录与会话。

use chrono::{DateTime, Utc};
use secrecy::SecretString;

/// 登录命令。
pub struct LoginCommand {
    pub username: String,
    pub password: SecretString,
    pub client_ip: String,
}

/// 登录结果；明文会话令牌只存在于此。
pub struct LoginResult {
    pub session_token: String,
    pub expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for LoginResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoginResult")
            .field("session_token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// 已认证的 Portal 用户。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalPrincipal {
    pub user_id: String,
    pub username: String,
}

/// 持久化会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalSession {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

/// 登录失败分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginError {
    InvalidCredentials,
    RateLimited,
    Unavailable,
}
