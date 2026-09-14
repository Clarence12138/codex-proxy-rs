//! 测试与 API 装配用的空实现，不持久化任何状态。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use gateway_core::routing::ConfigRevision;
use gateway_core::runtime::SnapshotControl;

use gateway_portal::{
    PortalConfig, PortalServices, initialize,
    model::{
        MutationContext,
        auth::{PortalPrincipal, PortalSession},
        keys::{CreatePortalKey, PortalKeyRecord, UpdatePortalKey},
        plans::{CreatePlan, PlanListQuery, PlanPage, SubscriptionPlan, UpdatePlan},
        subscriptions::{AssignSubscription, UserSubscription},
        usage::{PortalMe, PortalUsagePage, PortalUsageQuery, PortalUsageSummary},
        users::{
            CreatePortalUser, PortalUser, PortalUserListQuery, PortalUserPage, ResetPortalPassword,
            SetPortalUserEnabled,
        },
    },
    ports::store::{
        PortalAuthStore, PortalKeyStore, PortalPlanStore, PortalStoreError, PortalStoreErrorKind,
        PortalStorePorts, PortalStoreResult, PortalSubscriptionStore, PortalUsageStore,
        PortalUserStore,
    },
};

#[derive(Clone, Default)]
struct EmptyStore;

fn unavailable<T>() -> PortalStoreResult<T> {
    Err(PortalStoreError::new(
        PortalStoreErrorKind::Unavailable,
        "portal",
        "placeholder store",
    ))
}

#[async_trait]
impl PortalAuthStore for EmptyStore {
    async fn change_password(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<()> {
        unavailable()
    }

    async fn load_password_hash(
        &self,
        _: &str,
    ) -> PortalStoreResult<Option<(String, String, String)>> {
        Ok(None)
    }
    async fn store_session(&self, _: &PortalSession, _: &str) -> PortalStoreResult<()> {
        Ok(())
    }
    async fn load_session_by_token_hash(
        &self,
        _: &str,
        _: DateTime<Utc>,
    ) -> PortalStoreResult<Option<PortalPrincipal>> {
        Ok(None)
    }
    async fn delete_session_by_token_hash(&self, _: &str) -> PortalStoreResult<()> {
        Ok(())
    }
    async fn delete_sessions_for_user(&self, _: &str) -> PortalStoreResult<()> {
        Ok(())
    }
}

#[async_trait]
impl PortalUserStore for EmptyStore {
    async fn create_user(
        &self,
        _: CreatePortalUser,
        _: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<PortalUser> {
        unavailable()
    }
    async fn list_users(&self, _: PortalUserListQuery) -> PortalStoreResult<PortalUserPage> {
        Ok(PortalUserPage {
            items: Vec::new(),
            total: 0,
        })
    }
    async fn load_user(&self, _: &str) -> PortalStoreResult<Option<PortalUser>> {
        Ok(None)
    }
    async fn reset_password(
        &self,
        _: ResetPortalPassword,
        _: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<()> {
        unavailable()
    }
    async fn set_enabled(
        &self,
        _: SetPortalUserEnabled,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalUser)> {
        unavailable()
    }
}

#[async_trait]
impl PortalPlanStore for EmptyStore {
    async fn create_plan(
        &self,
        _: CreatePlan,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)> {
        unavailable()
    }
    async fn update_plan(
        &self,
        _: UpdatePlan,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)> {
        unavailable()
    }
    async fn list_plans(&self, _: PlanListQuery) -> PortalStoreResult<PlanPage> {
        Ok(PlanPage {
            items: Vec::new(),
            total: 0,
        })
    }
    async fn load_plan(&self, _: &str) -> PortalStoreResult<Option<SubscriptionPlan>> {
        Ok(None)
    }
}

#[async_trait]
impl PortalSubscriptionStore for EmptyStore {
    async fn assign(
        &self,
        _: AssignSubscription,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, UserSubscription)> {
        unavailable()
    }
    async fn disable(
        &self,
        _: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, Option<UserSubscription>)> {
        Ok((1, None))
    }
    async fn load_current(&self, _: &str) -> PortalStoreResult<Option<UserSubscription>> {
        Ok(None)
    }
}

#[async_trait]
impl PortalKeyStore for EmptyStore {
    async fn list_keys(&self, _: &str) -> PortalStoreResult<Vec<PortalKeyRecord>> {
        Ok(Vec::new())
    }
    async fn create_key(
        &self,
        _: &str,
        _: CreatePortalKey,
        _: &str,
        _: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)> {
        unavailable()
    }
    async fn update_key(
        &self,
        _: &str,
        _: UpdatePortalKey,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)> {
        unavailable()
    }
    async fn set_enabled(
        &self,
        _: &str,
        _: &str,
        _: bool,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)> {
        unavailable()
    }
    async fn delete_key(&self, _: &str, _: &str, _: &MutationContext) -> PortalStoreResult<u64> {
        unavailable()
    }
    async fn reveal_key(
        &self,
        _: &str,
        _: &str,
    ) -> PortalStoreResult<Option<(PortalKeyRecord, String)>> {
        Ok(None)
    }
}

#[async_trait]
impl PortalUsageStore for EmptyStore {
    async fn load_me(&self, _: &str, _: DateTime<Utc>) -> PortalStoreResult<PortalMe> {
        unavailable()
    }
    async fn list_records(
        &self,
        _: &str,
        _: PortalUsageQuery,
    ) -> PortalStoreResult<PortalUsagePage> {
        Ok(PortalUsagePage {
            items: Vec::new(),
            next_cursor: None,
        })
    }
    async fn summary(
        &self,
        _: &str,
        _: Option<DateTime<Utc>>,
        _: Option<DateTime<Utc>>,
    ) -> PortalStoreResult<PortalUsageSummary> {
        Ok(PortalUsageSummary {
            request_count: 0,
            total_tokens: 0,
            total_usd: "0".to_owned(),
        })
    }
}

struct NoopSnapshot;

impl SnapshotControl for NoopSnapshot {
    fn publish_committed(&self, _: ConfigRevision) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
}

/// 构造不访问数据库的 Portal 服务，供 API 装配测试使用。
#[must_use]
pub fn services() -> PortalServices {
    services_with_auth(Arc::new(EmptyStore))
}

pub fn services_with_auth(auth: Arc<dyn PortalAuthStore>) -> PortalServices {
    services_with_auth_and_usage(auth, Arc::new(EmptyStore))
}

pub fn services_with_auth_and_usage(
    auth: Arc<dyn PortalAuthStore>,
    usage: Arc<dyn PortalUsageStore>,
) -> PortalServices {
    let store = Arc::new(EmptyStore);
    initialize(
        PortalConfig::default(),
        PortalStorePorts::new(
            auth,
            store.clone(),
            store.clone(),
            store.clone(),
            store,
            usage,
        ),
        Arc::new(NoopSnapshot),
    )
    .expect("placeholder portal services")
    .services()
}
