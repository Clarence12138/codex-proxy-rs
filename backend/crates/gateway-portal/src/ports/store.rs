//! Portal 持久化能力。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::model::{
    MutationContext, PortalError,
    auth::{PortalPrincipal, PortalSession},
    keys::{CreatePortalKey, PortalKeyRecord, UpdatePortalKey},
    plans::{CreatePlan, PlanListQuery, PlanPage, SubscriptionPlan, UpdatePlan},
    subscriptions::{AssignSubscription, UserSubscription},
    usage::{PortalMe, PortalUsagePage, PortalUsageQuery, PortalUsageSummary},
    users::{
        CreatePortalUser, PortalUser, PortalUserListQuery, PortalUserPage, ResetPortalPassword,
        SetPortalUserEnabled,
    },
};

/// Portal 持久化错误分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalStoreErrorKind {
    Invalid,
    NotFound,
    Conflict,
    Unavailable,
}

/// 隐藏数据库细节的持久化错误。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{resource} store operation failed: {message}")]
pub struct PortalStoreError {
    kind: PortalStoreErrorKind,
    resource: &'static str,
    message: String,
}

impl PortalStoreError {
    #[must_use]
    pub fn new(
        kind: PortalStoreErrorKind,
        resource: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            resource,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> PortalStoreErrorKind {
        self.kind
    }
}

pub type PortalStoreResult<T> = Result<T, PortalStoreError>;

/// 认证存储。
#[async_trait]
pub trait PortalAuthStore: Send + Sync {
    /// 在用户行锁内核对已验证凭据，原子改密、撤销会话并记录审计。
    async fn change_password(
        &self,
        user_id: &str,
        expected_hash: &str,
        new_hash: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<()>;
    async fn load_password_hash(
        &self,
        username: &str,
    ) -> PortalStoreResult<Option<(String, String, String)>>;
    async fn store_session(
        &self,
        session: &PortalSession,
        password_hash: &str,
    ) -> PortalStoreResult<()>;
    async fn load_session_by_token_hash(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> PortalStoreResult<Option<PortalPrincipal>>;
    async fn delete_session_by_token_hash(&self, token_hash: &str) -> PortalStoreResult<()>;
    async fn delete_sessions_for_user(&self, user_id: &str) -> PortalStoreResult<()>;
}

/// 用户存储。
#[async_trait]
pub trait PortalUserStore: Send + Sync {
    async fn create_user(
        &self,
        command: CreatePortalUser,
        password_hash: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<PortalUser>;
    async fn list_users(&self, query: PortalUserListQuery) -> PortalStoreResult<PortalUserPage>;
    async fn load_user(&self, user_id: &str) -> PortalStoreResult<Option<PortalUser>>;
    async fn reset_password(
        &self,
        command: ResetPortalPassword,
        password_hash: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<()>;
    async fn set_enabled(
        &self,
        command: SetPortalUserEnabled,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalUser)>;
}

/// 套餐存储。
#[async_trait]
pub trait PortalPlanStore: Send + Sync {
    async fn create_plan(
        &self,
        command: CreatePlan,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)>;
    async fn update_plan(
        &self,
        command: UpdatePlan,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)>;
    async fn list_plans(&self, query: PlanListQuery) -> PortalStoreResult<PlanPage>;
    async fn load_plan(&self, plan_id: &str) -> PortalStoreResult<Option<SubscriptionPlan>>;
}

/// 订阅存储。
#[async_trait]
pub trait PortalSubscriptionStore: Send + Sync {
    async fn assign(
        &self,
        command: AssignSubscription,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, UserSubscription)>;
    async fn disable(
        &self,
        user_id: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, Option<UserSubscription>)>;
    async fn load_current(&self, user_id: &str) -> PortalStoreResult<Option<UserSubscription>>;
}

/// 用户 Key 存储。
#[async_trait]
pub trait PortalKeyStore: Send + Sync {
    async fn list_keys(&self, user_id: &str) -> PortalStoreResult<Vec<PortalKeyRecord>>;
    async fn create_key(
        &self,
        user_id: &str,
        command: CreatePortalKey,
        plaintext: &str,
        key_id: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)>;
    async fn update_key(
        &self,
        user_id: &str,
        command: UpdatePortalKey,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)>;
    async fn set_enabled(
        &self,
        user_id: &str,
        key_id: &str,
        enabled: bool,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)>;
    async fn delete_key(
        &self,
        user_id: &str,
        key_id: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<u64>;
    async fn reveal_key(
        &self,
        user_id: &str,
        key_id: &str,
    ) -> PortalStoreResult<Option<(PortalKeyRecord, String)>>;
}

/// 用户用量存储。
#[async_trait]
pub trait PortalUsageStore: Send + Sync {
    async fn overview(
        &self,
        user_id: &str,
        query: crate::model::usage::PortalOverviewQuery,
        now: DateTime<Utc>,
    ) -> PortalStoreResult<crate::model::usage::PortalUsageOverview>;
    async fn load_me(&self, user_id: &str, now: DateTime<Utc>) -> PortalStoreResult<PortalMe>;
    async fn list_records(
        &self,
        user_id: &str,
        query: PortalUsageQuery,
    ) -> PortalStoreResult<PortalUsagePage>;
    async fn summary(
        &self,
        user_id: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> PortalStoreResult<PortalUsageSummary>;
}

/// Portal 持久化能力集合。
#[derive(Clone)]
pub struct PortalStorePorts {
    auth: Arc<dyn PortalAuthStore>,
    users: Arc<dyn PortalUserStore>,
    plans: Arc<dyn PortalPlanStore>,
    subscriptions: Arc<dyn PortalSubscriptionStore>,
    keys: Arc<dyn PortalKeyStore>,
    usage: Arc<dyn PortalUsageStore>,
}

impl PortalStorePorts {
    #[must_use]
    pub fn new(
        auth: Arc<dyn PortalAuthStore>,
        users: Arc<dyn PortalUserStore>,
        plans: Arc<dyn PortalPlanStore>,
        subscriptions: Arc<dyn PortalSubscriptionStore>,
        keys: Arc<dyn PortalKeyStore>,
        usage: Arc<dyn PortalUsageStore>,
    ) -> Self {
        Self {
            auth,
            users,
            plans,
            subscriptions,
            keys,
            usage,
        }
    }

    #[must_use]
    pub fn auth(&self) -> Arc<dyn PortalAuthStore> {
        Arc::clone(&self.auth)
    }

    #[must_use]
    pub fn users(&self) -> Arc<dyn PortalUserStore> {
        Arc::clone(&self.users)
    }

    #[must_use]
    pub fn plans(&self) -> Arc<dyn PortalPlanStore> {
        Arc::clone(&self.plans)
    }

    #[must_use]
    pub fn subscriptions(&self) -> Arc<dyn PortalSubscriptionStore> {
        Arc::clone(&self.subscriptions)
    }

    #[must_use]
    pub fn keys(&self) -> Arc<dyn PortalKeyStore> {
        Arc::clone(&self.keys)
    }

    #[must_use]
    pub fn usage(&self) -> Arc<dyn PortalUsageStore> {
        Arc::clone(&self.usage)
    }
}

pub fn map_store_error(error: PortalStoreError) -> PortalError {
    let kind = match error.kind() {
        PortalStoreErrorKind::Invalid => crate::model::PortalErrorKind::Invalid,
        PortalStoreErrorKind::NotFound => crate::model::PortalErrorKind::NotFound,
        PortalStoreErrorKind::Conflict => crate::model::PortalErrorKind::Conflict,
        PortalStoreErrorKind::Unavailable => crate::model::PortalErrorKind::Unavailable,
    };
    let message = match kind {
        crate::model::PortalErrorKind::Invalid => "请求参数不合法",
        crate::model::PortalErrorKind::NotFound => "请求的资源不存在",
        crate::model::PortalErrorKind::Conflict => "当前资源状态冲突，请刷新后重试",
        crate::model::PortalErrorKind::Unavailable => "依赖服务暂不可用",
        _ => "服务内部错误",
    };
    PortalError::new(kind, message)
}
