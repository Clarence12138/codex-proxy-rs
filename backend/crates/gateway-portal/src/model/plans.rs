//! 订阅套餐。

use chrono::{DateTime, Utc};
use gateway_core::{engine::budget::ClientBudgetLimits, policy::RateLimits};

/// 套餐。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionPlan {
    pub id: String,
    pub name: String,
    pub budget: ClientBudgetLimits,
    pub limits: RateLimits,
    pub max_keys: u64,
    pub group_ids: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 创建套餐；必须至少绑定一组账号分组。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePlan {
    pub name: String,
    pub budget: ClientBudgetLimits,
    pub limits: RateLimits,
    pub max_keys: u64,
    pub group_ids: Vec<String>,
}

/// 更新套餐。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePlan {
    pub id: String,
    pub name: String,
    pub budget: ClientBudgetLimits,
    pub limits: RateLimits,
    pub max_keys: u64,
    pub group_ids: Vec<String>,
    pub enabled: bool,
}

/// 套餐列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanListQuery {
    pub page: u32,
    pub page_size: u32,
}

/// 套餐页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPage {
    pub items: Vec<SubscriptionPlan>,
    pub total: u64,
}
