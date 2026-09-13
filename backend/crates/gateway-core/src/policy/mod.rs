//! 下游 Client API Key 的准入策略。
//!
//! Client API Key 冻结账号分组权限；模型名称不参与权限判断。

mod client_version;

pub use client_version::{
    ClientVersionRejection, CodexClientKind, CodexClientMinVersions, CodexClientVersion,
};

use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;

use crate::account::scope::FrozenAccountScope;
use crate::validation::{IdentifierError, PolicyError, validate_text};

/// `client_api_keys.id` 的核心值对象。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientApiKeyId(String);

impl ClientApiKeyId {
    /// 校验并创建 Key ID。
    ///
    /// # Errors
    ///
    /// ID 为空、过长或包含控制字符时返回错误。
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        validate_text(&value, 128, false, None)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClientApiKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// 数据面冻结的用户范围 ID；Core 不解释 Portal 领域含义。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnerScopeId(String);

impl OwnerScopeId {
    /// 校验并创建用户范围 ID。
    ///
    /// # Errors
    ///
    /// ID 为空、过长或包含控制字符时返回错误。
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        validate_text(&value, 128, false, None)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OwnerScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// 随 Client Key 冻结的用户级并发/RPM 与订阅截止时间。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientOwnerScope {
    id: OwnerScopeId,
    limits: RateLimits,
    unscoped_means_deny: bool,
    subscription_starts_at: Option<SystemTime>,
    subscription_ends_at: Option<SystemTime>,
}

impl ClientOwnerScope {
    #[must_use]
    pub const fn new(
        id: OwnerScopeId,
        limits: RateLimits,
        unscoped_means_deny: bool,
        subscription_starts_at: Option<SystemTime>,
        subscription_ends_at: Option<SystemTime>,
    ) -> Self {
        Self {
            id,
            limits,
            unscoped_means_deny,
            subscription_starts_at,
            subscription_ends_at,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &OwnerScopeId {
        &self.id
    }

    #[must_use]
    pub const fn limits(&self) -> RateLimits {
        self.limits
    }

    #[must_use]
    pub const fn unscoped_means_deny(&self) -> bool {
        self.unscoped_means_deny
    }

    #[must_use]
    pub const fn subscription_starts_at(&self) -> Option<SystemTime> {
        self.subscription_starts_at
    }

    #[must_use]
    pub const fn subscription_ends_at(&self) -> Option<SystemTime> {
        self.subscription_ends_at
    }
}

/// RuntimeSnapshot 中用于同步认证的明文 Client API Key。
///
/// 数据库按产品约束明文保存；该值对象只负责阻止 `Debug`/日志意外输出。
#[derive(Clone, PartialEq, Eq)]
pub struct PlaintextClientApiKey(String);

impl PlaintextClientApiKey {
    /// 校验并创建明文 Key。
    ///
    /// # Errors
    ///
    /// Key 为空或无法作为 HTTP Bearer 值发送时返回错误。
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        Self::validate(&value)?;
        Ok(Self(value))
    }

    /// 迁入的 Key 不限定前缀或长度；保持原值，仅校验 HTTP 可传输的非空可见 ASCII。
    ///
    /// # Errors
    ///
    /// Key 为空或包含空白、控制字符、非 ASCII 字符时返回错误。
    pub fn validate(value: &str) -> Result<(), IdentifierError> {
        if value.is_empty() {
            return Err(IdentifierError::Empty);
        }
        if !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(IdentifierError::InvalidFormat);
        }
        Ok(())
    }

    /// 仅借给同步认证器做常量时间比较。
    #[must_use]
    pub fn expose_for_auth(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PlaintextClientApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PlaintextClientApiKey(<redacted>)")
    }
}

/// 零表示对应维度不额外限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RateLimits {
    pub max_concurrency: u64,
    pub requests_per_minute: u64,
}

impl RateLimits {
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            max_concurrency: 0,
            requests_per_minute: 0,
        }
    }
}

/// 从 `client_api_keys` 冻结的公开准入事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientPolicy {
    key_id: ClientApiKeyId,
    plaintext_key: PlaintextClientApiKey,
    account_scope: Arc<FrozenAccountScope>,
    enabled: bool,
    limits: RateLimits,
    owner: Option<ClientOwnerScope>,
}

impl ClientPolicy {
    #[must_use]
    pub const fn new(
        key_id: ClientApiKeyId,
        plaintext_key: PlaintextClientApiKey,
        account_scope: Arc<FrozenAccountScope>,
        enabled: bool,
        limits: RateLimits,
    ) -> Self {
        Self {
            key_id,
            plaintext_key,
            account_scope,
            enabled,
            limits,
            owner: None,
        }
    }

    /// 附加用户级并发/RPM 与订阅截止时间。
    #[must_use]
    pub fn with_owner_scope(mut self, owner: ClientOwnerScope) -> Self {
        self.owner = Some(owner);
        self
    }

    #[must_use]
    pub const fn key_id(&self) -> &ClientApiKeyId {
        &self.key_id
    }

    #[must_use]
    pub const fn plaintext_key(&self) -> &PlaintextClientApiKey {
        &self.plaintext_key
    }

    #[must_use]
    pub const fn account_scope(&self) -> &Arc<FrozenAccountScope> {
        &self.account_scope
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn limits(&self) -> RateLimits {
        self.limits
    }

    #[must_use]
    pub const fn owner(&self) -> Option<&ClientOwnerScope> {
        self.owner.as_ref()
    }

    /// 禁用的 Key 不接受新请求。
    ///
    /// # Errors
    ///
    /// Key 已禁用时返回稳定拒绝原因。
    pub fn authorize(&self) -> Result<(), PolicyError> {
        if !self.enabled {
            return Err(PolicyError::Denied {
                reason: "client API key is disabled",
            });
        }
        if let Some(owner) = &self.owner {
            let now = SystemTime::now();
            if owner
                .subscription_starts_at
                .is_some_and(|starts_at| now < starts_at)
                || owner
                    .subscription_ends_at
                    .is_some_and(|ends_at| now >= ends_at)
            {
                return Err(PolicyError::Denied {
                    reason: "subscription is not active",
                });
            }
        }
        Ok(())
    }
}
