use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use argon2::{Argon2, PasswordHasher};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use gateway_core::routing::ConfigRevision;
use gateway_core::runtime::SnapshotControl;
use gateway_portal::{
    PortalConfig, initialize,
    model::{
        MutationContext,
        auth::{LoginCommand, LoginError, PortalPrincipal, PortalSession},
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
use secrecy::SecretString;

type PasswordRow = (String, String, String);

#[derive(Clone, Default)]
pub(super) struct MemoryAuth {
    users: Arc<Mutex<HashMap<String, PasswordRow>>>,
    sessions: Arc<Mutex<HashMap<String, PortalSession>>>,
}

fn unavailable<T>() -> PortalStoreResult<T> {
    Err(PortalStoreError::new(
        PortalStoreErrorKind::Unavailable,
        "portal",
        "unused",
    ))
}

#[async_trait]
impl PortalAuthStore for MemoryAuth {
    async fn load_password_hash(
        &self,
        username: &str,
    ) -> PortalStoreResult<Option<(String, String, String)>> {
        Ok(self
            .users
            .lock()
            .expect("auth map")
            .get(&username.to_ascii_lowercase())
            .cloned())
    }

    async fn store_session(
        &self,
        session: &PortalSession,
        password_hash: &str,
    ) -> PortalStoreResult<()> {
        let users = self.users.lock().expect("auth map");
        let Some((_, status, hash)) = users.values().find(|(id, _, _)| id == &session.user_id)
        else {
            return Err(PortalStoreError::new(
                PortalStoreErrorKind::Conflict,
                "portal",
                "missing",
            ));
        };
        if status != "active" || hash != password_hash {
            return Err(PortalStoreError::new(
                PortalStoreErrorKind::Conflict,
                "portal",
                "stale",
            ));
        }
        drop(users);
        self.sessions
            .lock()
            .expect("session map")
            .insert(session.token_hash.clone(), session.clone());
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
impl PortalUserStore for MemoryAuth {
    async fn create_user(
        &self,
        _: CreatePortalUser,
        _: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<PortalUser> {
        unavailable()
    }
    async fn list_users(&self, _: PortalUserListQuery) -> PortalStoreResult<PortalUserPage> {
        unavailable()
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
impl PortalPlanStore for MemoryAuth {
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
        unavailable()
    }
    async fn load_plan(&self, _: &str) -> PortalStoreResult<Option<SubscriptionPlan>> {
        Ok(None)
    }
}

#[async_trait]
impl PortalSubscriptionStore for MemoryAuth {
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
        unavailable()
    }
    async fn load_current(&self, _: &str) -> PortalStoreResult<Option<UserSubscription>> {
        Ok(None)
    }
}

#[async_trait]
impl PortalKeyStore for MemoryAuth {
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
impl PortalUsageStore for MemoryAuth {
    async fn load_me(&self, _: &str, _: DateTime<Utc>) -> PortalStoreResult<PortalMe> {
        unavailable()
    }
    async fn list_records(
        &self,
        _: &str,
        _: PortalUsageQuery,
    ) -> PortalStoreResult<PortalUsagePage> {
        unavailable()
    }
    async fn summary(
        &self,
        _: &str,
        _: Option<DateTime<Utc>>,
        _: Option<DateTime<Utc>>,
    ) -> PortalStoreResult<PortalUsageSummary> {
        unavailable()
    }
}

pub(super) struct NoopSnapshot;

impl SnapshotControl for NoopSnapshot {
    fn publish_committed(&self, _: ConfigRevision) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
}

fn hash(password: &str) -> String {
    Argon2::default()
        .hash_password(password.as_bytes())
        .expect("hash")
        .to_string()
}

fn services(store: MemoryAuth) -> gateway_portal::PortalServices {
    let ports = PortalStorePorts::new(
        Arc::new(store.clone()),
        Arc::new(store.clone()),
        Arc::new(store.clone()),
        Arc::new(store.clone()),
        Arc::new(store.clone()),
        Arc::new(store),
    );
    initialize(PortalConfig::default(), ports, Arc::new(NoopSnapshot))
        .expect("portal")
        .services()
}

#[test]
fn login_error_kinds_are_stable() {
    assert_ne!(LoginError::InvalidCredentials, LoginError::RateLimited);
}

#[tokio::test]
async fn login_rate_limit_is_atomic_and_applies_to_unknown_users() {
    let store = MemoryAuth::default();
    store.users.lock().expect("users").insert(
        "known".to_owned(),
        (
            "usr_known".to_owned(),
            "active".to_owned(),
            hash("correct-password-12"),
        ),
    );
    let services = services(store);
    let auth = services.auth();
    for _ in 0..8 {
        let error = auth
            .login(LoginCommand {
                username: "missing".to_owned(),
                password: SecretString::from("wrong-password-12"),
                client_ip: "127.0.0.1".to_owned(),
            })
            .await
            .expect_err("unknown user still counts");
        assert_eq!(error, LoginError::InvalidCredentials);
    }
    let limited = auth
        .login(LoginCommand {
            username: "missing".to_owned(),
            password: SecretString::from("wrong-password-12"),
            client_ip: "127.0.0.1".to_owned(),
        })
        .await
        .expect_err("ninth attempt");
    assert_eq!(limited, LoginError::RateLimited);
}

#[tokio::test]
async fn successful_login_clears_throttle_window() {
    let store = MemoryAuth::default();
    store.users.lock().expect("users").insert(
        "known".to_owned(),
        (
            "usr_known".to_owned(),
            "active".to_owned(),
            hash("correct-password-12"),
        ),
    );
    let services = services(store);
    let auth = services.auth();
    for _ in 0..2 {
        let _ = auth
            .login(LoginCommand {
                username: "known".to_owned(),
                password: SecretString::from("wrong-password-12"),
                client_ip: "10.0.0.1".to_owned(),
            })
            .await;
    }
    auth.login(LoginCommand {
        username: "known".to_owned(),
        password: SecretString::from("correct-password-12"),
        client_ip: "10.0.0.1".to_owned(),
    })
    .await
    .expect("success clears throttle");
    let error = auth
        .login(LoginCommand {
            username: "known".to_owned(),
            password: SecretString::from("wrong-password-12"),
            client_ip: "10.0.0.1".to_owned(),
        })
        .await
        .expect_err("window reset");
    assert_eq!(error, LoginError::InvalidCredentials);
}

#[tokio::test]
async fn login_rate_limit_is_shared_across_concurrent_unknown_users() {
    let store = MemoryAuth::default();
    let services = std::sync::Arc::new(services(store));
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(20));
    let mut tasks = Vec::new();
    for _ in 0..20 {
        let services = services.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            services
                .auth()
                .login(LoginCommand {
                    username: "ghost".to_owned(),
                    password: SecretString::from("wrong-password-12"),
                    client_ip: "192.0.2.10".to_owned(),
                })
                .await
        }));
    }
    let mut invalid = 0;
    let mut limited = 0;
    for task in tasks {
        match task.await.expect("join") {
            Err(LoginError::InvalidCredentials) => invalid += 1,
            Err(LoginError::RateLimited) => limited += 1,
            other => panic!("unexpected login result: {other:?}"),
        }
    }
    assert_eq!(invalid + limited, 20);
    assert!(invalid <= 8, "concurrent reserve leaked {invalid} attempts");
    assert!(limited >= 12);
}

#[tokio::test]
async fn disabled_user_still_verifies_password_before_rejecting() {
    let store = MemoryAuth::default();
    store.users.lock().expect("users").insert(
        "disabled".to_owned(),
        (
            "usr_disabled".to_owned(),
            "disabled".to_owned(),
            hash("correct-password-12"),
        ),
    );
    let services = services(store);
    let error = services
        .auth()
        .login(LoginCommand {
            username: "disabled".to_owned(),
            password: SecretString::from("correct-password-12"),
            client_ip: "10.0.0.8".to_owned(),
        })
        .await
        .expect_err("disabled");
    assert_eq!(error, LoginError::InvalidCredentials);
}
