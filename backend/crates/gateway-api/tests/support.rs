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
    async fn overview(
        &self,
        _: &str,
        _: gateway_portal::model::usage::PortalOverviewQuery,
        _: chrono::DateTime<chrono::Utc>,
    ) -> gateway_portal::ports::store::PortalStoreResult<
        gateway_portal::model::usage::PortalUsageOverview,
    > {
        Err(gateway_portal::ports::store::PortalStoreError::new(
            gateway_portal::ports::store::PortalStoreErrorKind::Unavailable,
            "portal",
            "overview fixture not configured",
        ))
    }

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

impl gateway_core::metering::BillingResolver for NoopSnapshot {
    fn resolve(
        &self,
        _: &gateway_core::identity::ProviderKind,
        _: &gateway_core::metering::ProviderBillingInput,
    ) -> Option<gateway_core::metering::CalculatedBillingBreakdown> {
        None
    }
}

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
    services_with_auth_usage_and_billing(auth, usage, Arc::new(NoopSnapshot))
}

pub fn services_with_auth_usage_and_billing(
    auth: Arc<dyn PortalAuthStore>,
    usage: Arc<dyn PortalUsageStore>,
    billing: Arc<dyn gateway_core::metering::BillingResolver>,
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
        billing,
    )
    .expect("placeholder portal services")
    .services()
}

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::atomic::Ordering,
};

use axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{Method, Request, header},
};
use gateway_admin::{
    model::system::{
        SystemOperationAccepted, SystemUpdateDetail, SystemUpdateStatus, SystemVersion,
    },
    ports::system::{SystemOperationError, SystemOperations, SystemUpdateEventStream},
};
use gateway_core::{
    engine::execution::{ClientAuthenticationError, ClientKeyVerifier},
    policy::ClientApiKeyId,
};
use serde_json::Value;

pub(crate) const RAW_KEY: &str = "cpr_live_candidate_must_not_leak";

pub(crate) async fn auth_app() -> (axum::Router, Arc<crate::admin::MemoryAuthStore>) {
    let fixture = key_fixture().await;
    (
        crate::openai::api_router_with_admin(fixture.services),
        fixture.auth,
    )
}

pub(crate) async fn key_fixture() -> crate::admin::AdminTestFixture {
    let fixture = crate::admin::AdminTestFixture::with_key_verifier(
        Arc::new(AcceptingVerifier {
            key_id: ClientApiKeyId::new("key-42").expect("key ID"),
        }),
        Arc::new(VersionSystem),
    )
    .await;
    fixture.auth.enabled.store(true, Ordering::SeqCst);
    fixture
}

pub(crate) fn json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, "https://console.example.test")
        .body(Body::from(body.to_string()))
        .expect("client JSON request");
    request.extensions_mut().insert(ConnectInfo(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10)),
        41_000,
    )));
    request
}

pub(crate) fn empty_request(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("client empty request")
}

pub(crate) fn cookie_request(method: Method, uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .expect("client cookie request")
}

pub(crate) async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read client response body");
    serde_json::from_slice(&body).expect("parse client response JSON")
}

struct VersionSystem;

#[async_trait]
impl SystemOperations for VersionSystem {
    async fn version(&self) -> Result<SystemVersion, SystemOperationError> {
        Ok(SystemVersion {
            version: "3.7.0".to_owned(),
            git_sha: "internal-revision".to_owned(),
            build_time: "internal-build-time".to_owned(),
            deployment_mode: "binary".to_owned(),
            update_channel: "release".to_owned(),
            latest_version: "3.8.0".to_owned(),
            has_update: true,
            update_cached: true,
            update_warning: Some("internal-update-warning".to_owned()),
        })
    }

    async fn update_detail(&self, _: bool) -> Result<SystemUpdateDetail, SystemOperationError> {
        unreachable!("client route must not request update details")
    }

    fn update_events(&self) -> SystemUpdateEventStream {
        unreachable!("client route must not subscribe to updates")
    }

    async fn perform_update(
        &self,
        _: Option<String>,
    ) -> Result<SystemOperationAccepted, SystemOperationError> {
        unreachable!("client route must not perform updates")
    }

    async fn update_status(&self) -> Result<SystemUpdateStatus, SystemOperationError> {
        unreachable!("client route must not request update status")
    }

    async fn rollback(&self) -> Result<SystemOperationAccepted, SystemOperationError> {
        unreachable!("client route must not roll back")
    }

    async fn restart(&self) -> Result<SystemOperationAccepted, SystemOperationError> {
        unreachable!("client route must not restart")
    }
}

struct AcceptingVerifier {
    key_id: ClientApiKeyId,
}

impl ClientKeyVerifier for AcceptingVerifier {
    fn verify_client_key(
        &self,
        candidate: &str,
    ) -> Result<ClientApiKeyId, ClientAuthenticationError> {
        if candidate == RAW_KEY {
            Ok(self.key_id.clone())
        } else {
            Err(ClientAuthenticationError::InvalidKey)
        }
    }
}
