//! 用户订阅。生效条件由时间推导，不写 expired 状态。

use chrono::{DateTime, Utc};

/// 订阅状态（存储态）。过期由 `ends_at` 推导。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    Active,
    Disabled,
}

impl SubscriptionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

/// 用户当前订阅。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSubscription {
    pub id: String,
    pub user_id: String,
    pub plan_id: String,
    pub status: SubscriptionStatus,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserSubscription {
    /// `starts_at <= now < ends_at`（`ends_at` 为空表示无截止）且状态为 active。
    #[must_use]
    pub fn is_effective(&self, now: DateTime<Utc>) -> bool {
        self.status == SubscriptionStatus::Active
            && self.starts_at <= now
            && self.ends_at.is_none_or(|ends_at| now < ends_at)
    }
}

/// 开通或续期。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignSubscription {
    pub user_id: String,
    pub plan_id: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
}

/// 停用当前订阅。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisableSubscription {
    pub user_id: String,
}
