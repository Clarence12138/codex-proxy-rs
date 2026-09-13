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
    async fn change_password(
        &self,
        user_id: &str,
        expected_hash: &str,
        new_hash: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<()> {
        let mut users = self.users.lock().expect("auth map");
        let row = users
            .values_mut()
            .find(|(id, _, _)| id == user_id)
            .filter(|(_, status, hash)| status == "active" && hash == expected_hash)
            .ok_or_else(|| {
                PortalStoreError::new(PortalStoreErrorKind::Conflict, "portal", "stale")
            })?;
        row.2 = new_hash.to_owned();
        self.sessions
            .lock()
            .expect("session map")
            .retain(|_, session| session.user_id != user_id);
        Ok(())
    }

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
        command: CreatePortalUser,
        password_hash: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<PortalUser> {
        let id = format!("usr_{}", command.username);
        self.users.lock().unwrap().insert(
            command.username.to_ascii_lowercase(),
            (id.clone(), "active".to_owned(), password_hash.to_owned()),
        );
        Ok(PortalUser {
            id,
            username: command.username,
            status: gateway_portal::model::users::UserStatus::Active,
            plan_id: None,
            plan_name: None,
            subscription_starts_at: None,
            subscription_ends_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }
    async fn list_users(&self, _: PortalUserListQuery) -> PortalStoreResult<PortalUserPage> {
        unavailable()
    }
    async fn load_user(&self, _: &str) -> PortalStoreResult<Option<PortalUser>> {
        Ok(None)
    }
    async fn reset_password(
        &self,
        command: ResetPortalPassword,
        password_hash: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<()> {
        let mut users = self.users.lock().unwrap();
        let row = users
            .values_mut()
            .find(|(id, _, _)| id == &command.user_id)
            .unwrap();
        row.2 = password_hash.to_owned();
        self.sessions
            .lock()
            .unwrap()
            .retain(|_, session| session.user_id != command.user_id);
        Ok(())
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

pub(super) fn services(store: MemoryAuth) -> gateway_portal::PortalServices {
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

fn change_command(current: &str, new: &str) -> gateway_portal::model::auth::ChangePasswordCommand {
    gateway_portal::model::auth::ChangePasswordCommand {
        principal: PortalPrincipal {
            user_id: "usr_known".to_owned(),
            username: "known".to_owned(),
        },
        current_password: SecretString::from(current),
        new_password: SecretString::from(new),
        client_ip: "127.0.0.1".to_owned(),
    }
}

fn change_context() -> MutationContext {
    MutationContext {
        actor: gateway_portal::model::MutationActor::PortalSession {
            user_id: "usr_known".to_owned(),
        },
        request_id: "password-test".to_owned(),
    }
}

#[test]
fn change_command_debug_redacts_both_passwords() {
    let debug = format!("{:?}", change_command("old-secret", "new-secret"));
    assert!(!debug.contains("old-secret"));
    assert!(!debug.contains("new-secret"));
}

#[tokio::test]
async fn password_change_checks_current_password_and_length_then_revokes_only_owner_sessions() {
    use gateway_portal::model::PortalErrorKind;
    let store = MemoryAuth::default();
    let old_hash = hash("old-password");
    store.users.lock().unwrap().insert(
        "known".to_owned(),
        (
            "usr_known".to_owned(),
            "active".to_owned(),
            old_hash.clone(),
        ),
    );
    let services = services(store.clone());
    let auth = services.auth();
    for token in ["one", "two", "other"] {
        store.sessions.lock().unwrap().insert(
            token.to_owned(),
            PortalSession {
                id: token.to_owned(),
                user_id: if token == "other" {
                    "usr_other"
                } else {
                    "usr_known"
                }
                .to_owned(),
                token_hash: token.to_owned(),
                expires_at: Utc::now() + chrono::Duration::days(1),
            },
        );
    }
    for new in [
        "12345".to_owned(),
        "字".repeat(5),
        "a".repeat(257),
        "字".repeat(86),
    ] {
        assert_eq!(
            auth.change_password(&change_context(), change_command("old-password", &new))
                .await
                .unwrap_err()
                .kind(),
            PortalErrorKind::Invalid
        );
    }
    assert_eq!(
        auth.change_password(
            &change_context(),
            change_command("wrong-password", "123456")
        )
        .await
        .unwrap_err()
        .kind(),
        PortalErrorKind::Invalid
    );
    assert_eq!(store.users.lock().unwrap()["known"].2, old_hash);
    assert_eq!(store.sessions.lock().unwrap().len(), 3);
    let mut current = "old-password".to_owned();
    for new in [
        "123456".to_owned(),
        "$$$$$$".to_owned(),
        "abcdef".to_owned(),
        "字".repeat(6),
        "a".repeat(256),
        " 1234 ".to_owned(),
    ] {
        auth.change_password(&change_context(), change_command(&current, &new))
            .await
            .unwrap();
        assert_eq!(
            auth.login(LoginCommand {
                username: "known".to_owned(),
                password: SecretString::from(current),
                client_ip: "127.0.0.1".to_owned()
            })
            .await
            .unwrap_err(),
            LoginError::InvalidCredentials
        );
        auth.login(LoginCommand {
            username: "known".to_owned(),
            password: SecretString::from(new.clone()),
            client_ip: "127.0.0.1".to_owned(),
        })
        .await
        .unwrap();
        current = new;
    }
    let sessions = store.sessions.lock().unwrap();
    assert!(!sessions.contains_key("one"));
    assert!(!sessions.contains_key("two"));
    assert!(sessions.contains_key("other"));
}

#[tokio::test]
async fn wrong_current_password_shares_login_throttle() {
    let store = MemoryAuth::default();
    store.users.lock().unwrap().insert(
        "known".to_owned(),
        (
            "usr_known".to_owned(),
            "active".to_owned(),
            hash("old-password"),
        ),
    );
    let services = services(store);
    for _ in 0..8 {
        let error = services
            .auth()
            .change_password(
                &change_context(),
                change_command("wrong-password", "123456"),
            )
            .await
            .unwrap_err();
        assert_eq!(
            error.kind(),
            gateway_portal::model::PortalErrorKind::Invalid
        );
    }
    assert_eq!(
        services
            .auth()
            .login(LoginCommand {
                username: "known".to_owned(),
                password: SecretString::from("old-password"),
                client_ip: "127.0.0.1".to_owned()
            })
            .await
            .unwrap_err(),
        LoginError::RateLimited
    );
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
