//! Portal 登录、会话与登录限流。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use rand_core::{OsRng, RngCore as _};
use secrecy::ExposeSecret as _;
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use crate::{
    model::{
        PortalError,
        auth::{LoginCommand, LoginError, LoginResult, PortalPrincipal, PortalSession},
    },
    ports::store::{PortalAuthStore, PortalStoreErrorKind},
};

const MAX_ATTEMPTS: u32 = 8;
const WINDOW_SECS: i64 = 15 * 60;
const MAX_THROTTLE_ENTRIES: usize = 4096;
const MAX_USERNAME_CHARS: usize = 64;
const MAX_PASSWORD_BYTES: usize = 256;
const MAX_IP_CHARS: usize = 64;

struct ThrottleEntry {
    count: u32,
    start: DateTime<Utc>,
}

struct ThrottleBook {
    by_username: BTreeMap<String, ThrottleEntry>,
    by_ip: BTreeMap<String, ThrottleEntry>,
}

/// Portal 认证服务。
#[async_trait]
pub trait PortalAuthService: Send + Sync {
    async fn login(&self, command: LoginCommand) -> Result<LoginResult, LoginError>;
    async fn validate_session(
        &self,
        token: Option<&str>,
    ) -> Result<Option<PortalPrincipal>, PortalError>;
    async fn logout(&self, token: &str) -> Result<(), PortalError>;
}

pub(crate) struct DefaultPortalAuthService {
    session_ttl: Duration,
    store: Arc<dyn PortalAuthStore>,
    throttle: Mutex<ThrottleBook>,
}

impl DefaultPortalAuthService {
    #[must_use]
    pub(crate) fn new(session_ttl: Duration, store: Arc<dyn PortalAuthStore>) -> Self {
        Self {
            session_ttl,
            store,
            throttle: Mutex::new(ThrottleBook {
                by_username: BTreeMap::new(),
                by_ip: BTreeMap::new(),
            }),
        }
    }

    fn reserve_bucket(
        map: &mut BTreeMap<String, ThrottleEntry>,
        key: &str,
        now: DateTime<Utc>,
    ) -> Result<(), LoginError> {
        map.retain(|_, entry| (now - entry.start).num_seconds() < WINDOW_SECS);
        if let Some(entry) = map.get_mut(key) {
            if entry.count >= MAX_ATTEMPTS {
                return Err(LoginError::RateLimited);
            }
            entry.count = entry.count.saturating_add(1);
            return Ok(());
        }
        if map.len() >= MAX_THROTTLE_ENTRIES {
            return Err(LoginError::RateLimited);
        }
        map.insert(
            key.to_owned(),
            ThrottleEntry {
                count: 1,
                start: now,
            },
        );
        Ok(())
    }

    fn reserve_attempt(
        &self,
        username: &str,
        ip: &str,
        now: DateTime<Utc>,
    ) -> Result<(), LoginError> {
        let mut guard = self.throttle.lock().map_err(|_| LoginError::Unavailable)?;
        Self::reserve_bucket(&mut guard.by_ip, ip, now)?;
        Self::reserve_bucket(&mut guard.by_username, username, now)
    }

    fn clear_throttle(&self, username: &str, ip: &str) {
        if let Ok(mut guard) = self.throttle.lock() {
            guard.by_username.remove(username);
            guard.by_ip.remove(ip);
        }
    }
}

#[async_trait]
impl PortalAuthService for DefaultPortalAuthService {
    async fn login(&self, command: LoginCommand) -> Result<LoginResult, LoginError> {
        let now = Utc::now();
        let username = command.username.trim().to_ascii_lowercase();
        let ip = command.client_ip.trim();
        if username.is_empty()
            || username.chars().count() > MAX_USERNAME_CHARS
            || ip.is_empty()
            || ip.chars().count() > MAX_IP_CHARS
            || command.password.expose_secret().len() > MAX_PASSWORD_BYTES
        {
            return Err(LoginError::InvalidCredentials);
        }
        self.reserve_attempt(&username, ip, now)?;
        let loaded = self
            .store
            .load_password_hash(&username)
            .await
            .map_err(|_| LoginError::Unavailable)?;
        let password = command.password.expose_secret().to_owned();
        let (user_id, status, hash) = match loaded {
            Some(row) => row,
            None => {
                let dummy = dummy_password_hash()?.to_owned();
                let _ = verify_password_blocking(password, dummy).await?;
                return Err(LoginError::InvalidCredentials);
            }
        };
        let verified = verify_password_blocking(password, hash.clone()).await?;
        if !verified || status != "active" {
            return Err(LoginError::InvalidCredentials);
        }
        let session_token = random_session_token();
        let token_hash = hash_session_token(&session_token);
        let expires_at =
            now + chrono::Duration::from_std(self.session_ttl).unwrap_or(chrono::Duration::days(1));
        self.store
            .store_session(
                &PortalSession {
                    id: format!("psess_{}", Uuid::now_v7().simple()),
                    user_id,
                    token_hash,
                    expires_at,
                },
                &hash,
            )
            .await
            .map_err(|error| match error.kind() {
                PortalStoreErrorKind::Conflict | PortalStoreErrorKind::Invalid => {
                    LoginError::InvalidCredentials
                }
                _ => LoginError::Unavailable,
            })?;
        self.clear_throttle(&username, ip);
        Ok(LoginResult {
            session_token,
            expires_at,
        })
    }

    async fn validate_session(
        &self,
        token: Option<&str>,
    ) -> Result<Option<PortalPrincipal>, PortalError> {
        let Some(token) = token.filter(|value| !value.is_empty()) else {
            return Ok(None);
        };
        self.store
            .load_session_by_token_hash(&hash_session_token(token), Utc::now())
            .await
            .map_err(super::store_error)
    }

    async fn logout(&self, token: &str) -> Result<(), PortalError> {
        self.store
            .delete_session_by_token_hash(&hash_session_token(token))
            .await
            .map_err(super::store_error)
    }
}

pub(crate) fn hash_password(password: &str) -> Result<String, PortalError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| PortalError::internal("密码哈希失败"))
}

fn dummy_password_hash() -> Result<&'static str, LoginError> {
    static HASH: OnceLock<String> = OnceLock::new();
    if let Some(value) = HASH.get() {
        return Ok(value.as_str());
    }
    let hashed =
        hash_password("portal-dummy-password-not-used").map_err(|_| LoginError::Unavailable)?;
    Ok(HASH.get_or_init(|| hashed).as_str())
}

async fn verify_password_blocking(password: String, encoded: String) -> Result<bool, LoginError> {
    tokio::task::spawn_blocking(move || verify_password(&password, &encoded))
        .await
        .map_err(|_| LoginError::Unavailable)
}

fn verify_password(password: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded)
        .ok()
        .and_then(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .ok()
        })
        .is_some()
}

fn random_session_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("portal_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn hash_session_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
