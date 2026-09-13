//! Portal 用户。

use chrono::{DateTime, Utc};

use super::PortalError;

/// 用户状态。第一期只禁用，不物理删除。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Disabled,
}

impl UserStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Result<Self, PortalError> {
        match value {
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            _ => Err(PortalError::internal("用户状态不合法")),
        }
    }
}

/// 用户记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUser {
    pub id: String,
    pub username: String,
    pub status: UserStatus,
    pub plan_id: Option<String>,
    pub plan_name: Option<String>,
    pub subscription_starts_at: Option<DateTime<Utc>>,
    pub subscription_ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 管理员创建用户。
#[derive(Clone, PartialEq, Eq)]
pub struct CreatePortalUser {
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for CreatePortalUser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CreatePortalUser")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// 重置密码。
#[derive(Clone, PartialEq, Eq)]
pub struct ResetPortalPassword {
    pub user_id: String,
    pub password: String,
}

impl std::fmt::Debug for ResetPortalPassword {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResetPortalPassword")
            .field("user_id", &self.user_id)
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// 启停用户。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPortalUserEnabled {
    pub user_id: String,
    pub enabled: bool,
}

/// 用户列表查询。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUserListQuery {
    pub search: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

/// 用户列表页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUserPage {
    pub items: Vec<PortalUser>,
    pub total: u64,
}
