//! Portal 领域模型。

pub mod auth;
pub mod keys;
pub mod plans;
pub mod subscriptions;
pub mod usage;
pub mod users;

/// Portal 用例错误分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalErrorKind {
    Invalid,
    Unauthorized,
    NotFound,
    Conflict,
    RateLimited,
    Unavailable,
    Internal,
}

/// 可安全返回给用户或管理员的 Portal 错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct PortalError {
    kind: PortalErrorKind,
    message: String,
}

impl PortalError {
    #[must_use]
    pub fn new(kind: PortalErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> PortalErrorKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(PortalErrorKind::Invalid, message)
    }

    #[must_use]
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(PortalErrorKind::Unauthorized, message)
    }

    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(PortalErrorKind::NotFound, message)
    }

    #[must_use]
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(PortalErrorKind::Conflict, message)
    }

    #[must_use]
    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(PortalErrorKind::RateLimited, message)
    }

    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(PortalErrorKind::Unavailable, message)
    }

    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(PortalErrorKind::Internal, message)
    }
}

/// 可审计写操作发起者。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationActor {
    PortalSession { user_id: String },
    AdminSession { admin_user_id: String },
    AdminApiKey,
    System,
}

/// 写操作审计上下文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationContext {
    pub actor: MutationActor,
    pub request_id: String,
}
