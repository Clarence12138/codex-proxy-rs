//! 用户 Client Key 命令。密钥实体仍是 `client_api_keys`。

use chrono::{DateTime, Utc};
use gateway_core::{engine::budget::ClientBudgetLimits, policy::RateLimits};

/// 用户可见的 Key 投影。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalKeyRecord {
    pub id: String,
    pub name: String,
    pub label: Option<String>,
    pub prefix: String,
    pub enabled: bool,
    pub limits: RateLimits,
    pub budget: ClientBudgetLimits,
    pub daily_used_usd: String,
    pub weekly_used_usd: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 创建用户 Key；分组由服务端按套餐写入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePortalKey {
    pub name: String,
    pub label: Option<String>,
    pub limits: RateLimits,
    pub budget: ClientBudgetLimits,
}

/// 更新用户 Key 的安全字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePortalKey {
    pub id: String,
    pub name: String,
    pub label: Option<String>,
    pub limits: RateLimits,
    pub budget: ClientBudgetLimits,
}

/// 创建结果；明文只出现一次。
pub struct CreatedPortalKey {
    pub record: PortalKeyRecord,
    pub plaintext: String,
}

/// 揭示结果。
pub struct RevealedPortalKey {
    pub record: PortalKeyRecord,
    pub plaintext: String,
}
