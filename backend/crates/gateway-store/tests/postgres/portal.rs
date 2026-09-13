use chrono::Utc;
use gateway_admin::{
    model::{
        MutationActor as AdminActor, MutationContext as AdminContext, client_keys::UpdateClientKey,
    },
    ports::store::ClientKeyStore as _,
};
use gateway_core::routing::AccountGroupId;
use gateway_core::{
    engine::{
        ModelRequestId,
        budget::{ClientBudgetCharge, ClientBudgetPort},
    },
    error::GatewayErrorKind,
    policy::{ClientApiKeyId, OwnerScopeId},
};
use gateway_portal::{
    model::{
        MutationActor, MutationContext,
        auth::PortalSession,
        keys::CreatePortalKey,
        plans::CreatePlan,
        subscriptions::AssignSubscription,
        users::{CreatePortalUser, ResetPortalPassword, SetPortalUserEnabled},
    },
    ports::store::{
        PortalAuthStore, PortalKeyStore, PortalPlanStore, PortalStoreErrorKind,
        PortalSubscriptionStore, PortalUsageStore, PortalUserStore,
    },
};
use gateway_store::postgres::{PgAdminClientKeyStore, PgClientBudgetStore, PgPortalStore};
use uuid::Uuid;

use super::TestDatabase;

fn context() -> MutationContext {
    MutationContext {
        actor: MutationActor::System,
        request_id: "portal-test".to_owned(),
    }
}

fn group_id(label: &str) -> String {
    format!("grp_{label:0<32}")
}

async fn seed_group(database: &TestDatabase, id: &str, name: &str) {
    sqlx::query(
        "insert into account_groups (id, name, color, enabled, created_at, updated_at)
         values ($1, $2, '#2563EBFF', true, now(), now())",
    )
    .bind(id)
    .bind(name)
    .execute(&database.pool)
    .await
    .expect("seed account group");
}

fn plaintext(index: u8) -> String {
    format!("sk_{}", (b'a' + index) as char)
        .chars()
        .chain(std::iter::repeat('a'))
        .take(46)
        .collect()
}

#[tokio::test]
async fn portal_migrations_create_user_ledger_tables() {
    let Some(database) = TestDatabase::create("portal_schema").await else {
        return;
    };
    let tables = sqlx::query_scalar::<_, String>(
        "select table_name from information_schema.tables
         where table_schema = current_schema()
           and table_name in (
             'portal_users', 'subscription_plans', 'user_subscriptions',
             'portal_user_budget_windows', 'portal_user_charge_events'
           )
         order by table_name",
    )
    .fetch_all(&database.pool)
    .await
    .expect("list portal tables");
    assert_eq!(
        tables,
        [
            "portal_user_budget_windows",
            "portal_user_charge_events",
            "portal_users",
            "subscription_plans",
            "user_subscriptions",
        ]
    );
    database.close().await;
}

#[tokio::test]
async fn owned_keys_share_user_budget_and_survive_key_deletion() {
    let Some(database) = TestDatabase::create("portal_budget").await else {
        return;
    };
    let store = PgPortalStore::new(database.pool.clone());
    let group = group_id("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    seed_group(&database, &group, "Portal Budget").await;
    let user = store
        .create_user(
            CreatePortalUser {
                username: "budget-user".to_owned(),
                password: "unused-password".to_owned(),
            },
            "hash",
            &context(),
        )
        .await
        .expect("create user");
    let plan = store
        .create_plan(
            CreatePlan {
                name: "shared".to_owned(),
                budget: gateway_core::engine::budget::ClientBudgetLimits {
                    daily_usd: "1".parse().unwrap(),
                    weekly_usd: "5".parse().unwrap(),
                },
                limits: Default::default(),
                max_keys: 4,
                group_ids: vec![group.clone()],
            },
            &context(),
        )
        .await
        .expect("create plan")
        .1;
    store
        .assign(
            AssignSubscription {
                user_id: user.id.clone(),
                plan_id: plan.id,
                starts_at: Utc::now() - chrono::Duration::hours(1),
                ends_at: None,
            },
            &context(),
        )
        .await
        .expect("assign");
    let first = store
        .create_key(
            &user.id,
            CreatePortalKey {
                name: "one".to_owned(),
                label: None,
                limits: Default::default(),
                budget: Default::default(),
            },
            &plaintext(0),
            "key_portal_one",
            &context(),
        )
        .await
        .expect("create first key")
        .1;
    let second = store
        .create_key(
            &user.id,
            CreatePortalKey {
                name: "two".to_owned(),
                label: None,
                limits: Default::default(),
                budget: Default::default(),
            },
            &plaintext(1),
            "key_portal_two",
            &context(),
        )
        .await
        .expect("create second key")
        .1;

    let budgets = PgClientBudgetStore::new(database.pool.clone());
    let owner = OwnerScopeId::new(user.id.clone()).expect("owner");
    for (key, request, amount) in [
        (first.id.as_str(), "a", "0.6"),
        (second.id.as_str(), "b", "0.6"),
    ] {
        budgets
            .admit(ClientApiKeyId::new(key).unwrap())
            .await
            .expect("admit owned key");
        budgets
            .settle(ClientBudgetCharge {
                key_id: ClientApiKeyId::new(key).unwrap(),
                request_id: ModelRequestId::new(format!("req_{request}")).unwrap(),
                owner_scope_id: Some(owner.clone()),
                amount_usd: amount.parse().unwrap(),
                completed_at: Utc::now().into(),
            })
            .await
            .expect("settle owned key");
    }
    let denied = budgets
        .admit(ClientApiKeyId::new(&first.id).unwrap())
        .await
        .expect_err("user daily budget exhausted");
    assert_eq!(denied.kind(), GatewayErrorKind::RateLimited);
    assert_eq!(
        denied.client_error_code(),
        Some("user_daily_budget_exceeded")
    );

    store
        .delete_key(&user.id, &first.id, &context())
        .await
        .expect("delete first key");
    budgets
        .settle(ClientBudgetCharge {
            key_id: ClientApiKeyId::new(&first.id).unwrap(),
            request_id: ModelRequestId::new("req_after_delete").unwrap(),
            owner_scope_id: Some(owner),
            amount_usd: "0.1".parse().unwrap(),
            completed_at: Utc::now().into(),
        })
        .await
        .expect("settle deleted key against frozen owner");
    let used: String = sqlx::query_scalar(
        "select daily_used_usd::text from portal_user_budget_windows where user_id = $1",
    )
    .bind(&user.id)
    .fetch_one(&database.pool)
    .await
    .expect("user window");
    assert_eq!(
        used.parse::<gateway_core::metering::Decimal>()
            .expect("user used")
            .canonical(),
        "1.3",
    );
    database.close().await;
}

#[tokio::test]
async fn portal_store_rejects_cross_user_key_and_empty_plan_scope() {
    let Some(database) = TestDatabase::create("portal_acl").await else {
        return;
    };
    let store = PgPortalStore::new(database.pool.clone());
    let group = group_id("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    seed_group(&database, &group, "Portal Acl").await;
    let owner = store
        .create_user(
            CreatePortalUser {
                username: "owner".to_owned(),
                password: "unused-password".to_owned(),
            },
            "hash",
            &context(),
        )
        .await
        .expect("owner");
    let stranger = store
        .create_user(
            CreatePortalUser {
                username: "stranger".to_owned(),
                password: "unused-password".to_owned(),
            },
            "hash",
            &context(),
        )
        .await
        .expect("stranger");
    let plan = store
        .create_plan(
            CreatePlan {
                name: "acl".to_owned(),
                budget: Default::default(),
                limits: Default::default(),
                max_keys: 2,
                group_ids: vec![group.clone()],
            },
            &context(),
        )
        .await
        .expect("plan")
        .1;
    store
        .assign(
            AssignSubscription {
                user_id: owner.id.clone(),
                plan_id: plan.id,
                starts_at: Utc::now() - chrono::Duration::minutes(1),
                ends_at: None,
            },
            &context(),
        )
        .await
        .expect("assign");
    let key = store
        .create_key(
            &owner.id,
            CreatePortalKey {
                name: "owned".to_owned(),
                label: None,
                limits: Default::default(),
                budget: Default::default(),
            },
            &plaintext(2),
            "key_portal_acl",
            &context(),
        )
        .await
        .expect("key")
        .1;
    let error = store
        .delete_key(&stranger.id, &key.id, &context())
        .await
        .expect_err("cross-user delete");
    assert_eq!(error.kind(), PortalStoreErrorKind::NotFound);
    let empty = store
        .create_plan(
            CreatePlan {
                name: "empty".to_owned(),
                budget: Default::default(),
                limits: Default::default(),
                max_keys: 1,
                group_ids: vec![],
            },
            &context(),
        )
        .await
        .expect_err("empty plan groups");
    assert_eq!(empty.kind(), PortalStoreErrorKind::Invalid);
    let other = group_id("cccccccccccccccccccccccccccccccc");
    seed_group(&database, &other, "Other Group").await;
    let admin = PgAdminClientKeyStore::new(database.pool.clone());
    admin
        .update_client_key(
            UpdateClientKey {
                id: ClientApiKeyId::new(&key.id).unwrap(),
                name: key.name.clone(),
                label: None,
                group_ids: vec![AccountGroupId::new(other).unwrap()],
                limits: Default::default(),
                daily_limit_usd: None,
                weekly_limit_usd: None,
            },
            &AdminContext {
                actor: AdminActor::System,
                request_id: "admin-rescope".to_owned(),
            },
        )
        .await
        .expect_err("admin cannot re-scope owned key");
    let kept: Vec<String> = sqlx::query_scalar(
        "select account_group_id from client_api_key_groups
         where client_api_key_id = $1 order by account_group_id",
    )
    .bind(&key.id)
    .fetch_all(&database.pool)
    .await
    .expect("kept groups");
    assert_eq!(kept, vec![group.clone()]);
    admin
        .update_client_key(
            UpdateClientKey {
                id: ClientApiKeyId::new(&key.id).unwrap(),
                name: "owned-renamed".to_owned(),
                label: None,
                group_ids: vec![AccountGroupId::new(group.clone()).unwrap()],
                limits: Default::default(),
                daily_limit_usd: None,
                weekly_limit_usd: None,
            },
            &AdminContext {
                actor: AdminActor::System,
                request_id: "admin-same-scope".to_owned(),
            },
        )
        .await
        .expect("same-scope admin update");
    database.close().await;
}

#[tokio::test]
async fn reset_password_and_disable_revoke_sessions_in_the_same_transaction() {
    let Some(database) = TestDatabase::create("portal_session").await else {
        return;
    };
    let store = PgPortalStore::new(database.pool.clone());
    let user = store
        .create_user(
            CreatePortalUser {
                username: "session-user".to_owned(),
                password: "unused-password".to_owned(),
            },
            "old-hash",
            &context(),
        )
        .await
        .expect("user");
    store
        .store_session(
            &PortalSession {
                id: format!("psess_{}", Uuid::now_v7().simple()),
                user_id: user.id.clone(),
                token_hash: "aa".repeat(32),
                expires_at: Utc::now() + chrono::Duration::hours(1),
            },
            "old-hash",
        )
        .await
        .expect("session");
    store
        .reset_password(
            ResetPortalPassword {
                user_id: user.id.clone(),
                password: "unused-password".to_owned(),
            },
            "new-hash",
            &context(),
        )
        .await
        .expect("reset");
    let remaining: i64 =
        sqlx::query_scalar("select count(*) from portal_sessions where user_id = $1")
            .bind(&user.id)
            .fetch_one(&database.pool)
            .await
            .expect("count sessions");
    assert_eq!(remaining, 0);
    store
        .store_session(
            &PortalSession {
                id: format!("psess_{}", Uuid::now_v7().simple()),
                user_id: user.id.clone(),
                token_hash: "bb".repeat(32),
                expires_at: Utc::now() + chrono::Duration::hours(1),
            },
            "new-hash",
        )
        .await
        .expect("new session");
    PortalUserStore::set_enabled(
        &store,
        SetPortalUserEnabled {
            user_id: user.id.clone(),
            enabled: false,
        },
        &context(),
    )
    .await
    .expect("disable");
    let after_disable: i64 =
        sqlx::query_scalar("select count(*) from portal_sessions where user_id = $1")
            .bind(&user.id)
            .fetch_one(&database.pool)
            .await
            .expect("count after disable");
    assert_eq!(after_disable, 0);
    let stale = store
        .store_session(
            &PortalSession {
                id: format!("psess_{}", Uuid::now_v7().simple()),
                user_id: user.id.clone(),
                token_hash: "cc".repeat(32),
                expires_at: Utc::now() + chrono::Duration::hours(1),
            },
            "new-hash",
        )
        .await
        .expect_err("disabled user cannot store session");
    assert_eq!(stale.kind(), PortalStoreErrorKind::Conflict);
    database.close().await;
}

#[tokio::test]
async fn usage_summary_is_owner_scoped() {
    let Some(database) = TestDatabase::create("portal_usage").await else {
        return;
    };
    let store = PgPortalStore::new(database.pool.clone());
    let one = store
        .create_user(
            CreatePortalUser {
                username: "usage-one".to_owned(),
                password: "unused-password".to_owned(),
            },
            "hash",
            &context(),
        )
        .await
        .expect("user one");
    let two = store
        .create_user(
            CreatePortalUser {
                username: "usage-two".to_owned(),
                password: "unused-password".to_owned(),
            },
            "hash",
            &context(),
        )
        .await
        .expect("user two");
    for (id, owner, tokens) in [
        ("req_usage_one", one.id.as_str(), 10_i64),
        ("req_usage_two", two.id.as_str(), 99_i64),
    ] {
        sqlx::query(
            "insert into model_requests (
                id, client_api_key_ref, config_revision, protocol, operation, endpoint,
                client_transport, outcome, total_tokens, cost_source, cost_amount, cost_currency,
                started_at, deadline_at, completed_at, routing_scope, owner_user_id
             ) values (
                $1, 'key_ref', 1, 'openai', 'responses', '/v1/responses',
                'http', 'succeeded', $3, 'calculated', 1.25, 'USD',
                now(), now() + interval '1 minute', now(), 'empty', $2
             )",
        )
        .bind(id)
        .bind(owner)
        .bind(tokens)
        .execute(&database.pool)
        .await
        .expect("seed usage");
    }
    let summary = store.summary(&one.id, None, None).await.expect("summary");
    assert_eq!(summary.request_count, 1);
    assert_eq!(summary.total_tokens, 10);
    assert_eq!(summary.total_usd, "1.2500000000");
    database.close().await;
}
