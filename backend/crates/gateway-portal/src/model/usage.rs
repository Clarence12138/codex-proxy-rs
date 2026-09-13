//! 用户用量白名单投影。

use chrono::{DateTime, Utc};

/// 用户请求列表查询。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUsageQuery {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub cursor: Option<String>,
    pub page_size: u32,
}

/// 白名单字段的请求记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUsageRecord {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub model: Option<String>,
    pub outcome: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cost_usd: Option<String>,
    pub key_id: Option<String>,
    pub key_prefix: Option<String>,
    pub key_name: Option<String>,
}

/// 用户用量页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUsagePage {
    pub items: Vec<PortalUsageRecord>,
    pub next_cursor: Option<String>,
}

/// 用户用量汇总。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUsageSummary {
    pub request_count: u64,
    pub total_tokens: i64,
    pub total_usd: String,
}

/// 当前用户资料与套餐窗口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalMe {
    pub user_id: String,
    pub username: String,
    pub plan_name: Option<String>,
    pub subscription_ends_at: Option<DateTime<Utc>>,
    pub subscription_effective: bool,
    pub daily_limit_usd: String,
    pub weekly_limit_usd: String,
    pub daily_used_usd: String,
    pub weekly_used_usd: String,
    pub max_concurrency: u64,
    pub requests_per_minute: u64,
    pub max_keys: u64,
    pub key_count: u64,
}
