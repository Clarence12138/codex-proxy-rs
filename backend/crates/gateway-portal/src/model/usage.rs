//! 用户用量白名单投影。

use chrono::{DateTime, Utc};

/// 用户请求列表查询。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUsageQuery {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub cursor: Option<String>,
    pub model: Option<String>,
    pub page_size: u32,
}

/// 白名单字段的请求记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalUsageRecord {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub model: Option<String>,
    pub upstream_model: Option<String>,
    pub provider: Option<String>,
    pub authentication_kind: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_preset: Option<String>,
    pub subagent_kind: Option<String>,
    pub service_tier: Option<String>,
    pub client_transport: String,
    pub upstream_transport: Option<String>,
    pub cached_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub image_input_tokens: Option<i64>,
    pub image_output_tokens: Option<i64>,
    pub first_token_ms: Option<i64>,
    pub first_event_ms: Option<i64>,
    pub first_reasoning_ms: Option<i64>,
    pub first_text_ms: Option<i64>,
    pub latency_ms: Option<i64>,
    pub outcome: String,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cost_usd: Option<String>,
    pub cost_source: String,
    pub billing: Option<gateway_core::metering::UsageBilling>,
    pub key_id: String,
    pub key_prefix: Option<String>,
    pub key_name: Option<String>,
}

/// 用户概览查询；用户范围仅由会话提供。
#[derive(Debug, Clone)]
pub struct PortalOverviewQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalUsageMetrics {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: Option<String>,
    pub cost_incomplete: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortalTrendPoint {
    pub time: DateTime<Utc>,
    pub bucket_seconds: i64,
    #[serde(flatten)]
    pub metrics: PortalUsageMetrics,
}

/// 健康计数独立于成功计量，供 API 共用展示规则转换。
#[derive(Debug, Clone)]
pub struct PortalHealthPoint {
    pub time: DateTime<Utc>,
    pub success: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub incomplete: u64,
    pub caller_error: u64,
}

#[derive(Debug, Clone)]
pub struct PortalUsageOverview {
    pub as_of: DateTime<Utc>,
    pub query: PortalOverviewQuery,
    pub me: PortalMe,
    pub daily_resets_at: Option<DateTime<Utc>>,
    pub weekly_resets_at: Option<DateTime<Utc>>,
    pub summary: PortalUsageMetrics,
    pub trend: Vec<PortalTrendPoint>,
    pub health: Vec<PortalHealthPoint>,
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
