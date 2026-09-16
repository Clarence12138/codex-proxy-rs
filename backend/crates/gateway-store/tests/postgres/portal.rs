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
        usage::PortalUsageQuery,
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
async fn self_password_change_is_atomic_fenced_and_audited_without_secret_values() {
    let Some(database) = TestDatabase::create("portal_self_password").await else {
        return;
    };
    let store = PgPortalStore::new(database.pool.clone());
    let ctx = context();
    let user = store
        .create_user(
            CreatePortalUser {
                username: "change-user".to_owned(),
                password: "unused".to_owned(),
            },
            "old-hash",
            &ctx,
        )
        .await
        .unwrap();
    let other = store
        .create_user(
            CreatePortalUser {
                username: "other-user".to_owned(),
                password: "unused".to_owned(),
            },
            "other-hash",
            &ctx,
        )
        .await
        .unwrap();
    for (id, owner, hash) in [
        ("first", &user.id, "old-hash"),
        ("second", &user.id, "old-hash"),
        ("other", &other.id, "other-hash"),
    ] {
        store
            .store_session(
                &PortalSession {
                    id: id.to_owned(),
                    user_id: owner.clone(),
                    token_hash: id.to_owned(),
                    expires_at: Utc::now() + chrono::Duration::days(1),
                },
                hash,
            )
            .await
            .unwrap();
    }
    let actor = MutationContext {
        actor: MutationActor::PortalSession {
            user_id: user.id.clone(),
        },
        request_id: "self-password-test".to_owned(),
    };
    // 审计失败必须回滚密码更新和会话撤销，不能留下部分成功。
    sqlx::query("alter table portal_audit_events add constraint reject_password_test check (action <> 'change_password')").execute(&database.pool).await.unwrap();
    assert!(
        store
            .change_password(&user.id, "old-hash", "failed-hash", &actor)
            .await
            .is_err()
    );
    assert_eq!(
        store
            .load_password_hash("change-user")
            .await
            .unwrap()
            .unwrap()
            .2,
        "old-hash"
    );
    assert!(
        store
            .load_session_by_token_hash("first", Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    sqlx::query("alter table portal_audit_events drop constraint reject_password_test")
        .execute(&database.pool)
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        store.change_password(&user.id, "old-hash", "new-hash-a", &actor),
        store.change_password(&user.id, "old-hash", "new-hash-b", &actor),
    );
    assert_ne!(first.is_ok(), second.is_ok());
    assert_eq!(
        first.err().or_else(|| second.err()).unwrap().kind(),
        PortalStoreErrorKind::Conflict
    );
    for token in ["first", "second"] {
        assert!(
            store
                .load_session_by_token_hash(token, Utc::now())
                .await
                .unwrap()
                .is_none()
        );
    }
    assert!(
        store
            .load_session_by_token_hash("other", Utc::now())
            .await
            .unwrap()
            .is_some()
    );
    let audit: Vec<String> = sqlx::query_scalar(
        "select row_to_json(a)::text from portal_audit_events a where action = 'change_password'",
    )
    .fetch_all(&database.pool)
    .await
    .unwrap();
    assert_eq!(audit.len(), 1);
    for secret in ["old-hash", "new-hash-a", "new-hash-b", "failed-hash"] {
        assert!(!audit[0].contains(secret));
    }
    let stale = PortalSession {
        id: "stale".to_owned(),
        user_id: user.id.clone(),
        token_hash: "stale".to_owned(),
        expires_at: Utc::now() + chrono::Duration::days(1),
    };
    assert_eq!(
        store
            .store_session(&stale, "old-hash")
            .await
            .unwrap_err()
            .kind(),
        PortalStoreErrorKind::Conflict
    );
    let verified = store
        .load_password_hash("change-user")
        .await
        .unwrap()
        .unwrap()
        .2;
    store
        .reset_password(
            ResetPortalPassword {
                user_id: user.id.clone(),
                password: "unused".to_owned(),
            },
            "admin-hash",
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .change_password(&user.id, &verified, "stale-change", &actor)
            .await
            .unwrap_err()
            .kind(),
        PortalStoreErrorKind::Conflict
    );
    PortalUserStore::set_enabled(
        &store,
        SetPortalUserEnabled {
            user_id: user.id.clone(),
            enabled: false,
        },
        &ctx,
    )
    .await
    .unwrap();
    assert_eq!(
        store
            .change_password(&user.id, "admin-hash", "disabled-change", &actor)
            .await
            .unwrap_err()
            .kind(),
        PortalStoreErrorKind::Conflict
    );
    assert_eq!(
        store
            .load_password_hash("change-user")
            .await
            .unwrap()
            .unwrap()
            .2,
        "admin-hash"
    );
    database.close().await;
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

    // 续期仍提交绝对期限；替换订阅不得清空跨 Key 归集的用户账本。
    let previous = store.load_current(&user.id).await.unwrap().unwrap();
    let ends_at = Utc::now() + chrono::Duration::days(30);
    let renewed = store
        .assign(
            AssignSubscription {
                user_id: user.id.clone(),
                plan_id: previous.plan_id,
                starts_at: previous.starts_at,
                ends_at: Some(ends_at),
            },
            &context(),
        )
        .await
        .expect("renew subscription")
        .1;
    assert_eq!(renewed.starts_at, previous.starts_at);
    assert_eq!(
        renewed.ends_at.unwrap().timestamp_micros(),
        ends_at.timestamp_micros()
    );
    let active: i64 = sqlx::query_scalar(
        "select count(*) from user_subscriptions where user_id = $1 and status = 'active'",
    )
    .bind(&user.id)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(active, 1);
    let denied = budgets
        .admit(ClientApiKeyId::new(&second.id).unwrap())
        .await
        .expect_err("renewal must not reset user budget");
    assert_eq!(
        denied.client_error_code(),
        Some("user_daily_budget_exceeded")
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
                started_at, deadline_at, completed_at, routing_scope, owner_user_id,
                downstream_committed_at, client_status_code, provider_kind, request_kind
             ) values (
                $1, 'key_ref', 1, 'openai', 'responses', '/v1/responses',
                'http_json', 'succeeded', $3, 'calculated', 1.25, 'USD',
                now(), now() + interval '1 minute', now(), 'empty', $2, now(), 200, 'openai', 'inference'
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

    // 每种终态都保留原始审计，只有真实交付且非预热的成功用量进入汇总。
    for (id, transport, status, committed, outcome, kind, tokens) in [
        (
            "req_ws",
            "websocket",
            None,
            true,
            "succeeded",
            "inference",
            Some(20_i64),
        ),
        (
            "req_prewarm",
            "http",
            Some(200),
            true,
            "succeeded",
            "prewarm",
            Some(100),
        ),
        (
            "req_undelivered",
            "http",
            Some(200),
            false,
            "succeeded",
            "inference",
            Some(100),
        ),
        (
            "req_failed",
            "http",
            Some(500),
            true,
            "failed",
            "inference",
            Some(100),
        ),
        (
            "req_bad_status",
            "http",
            Some(500),
            true,
            "succeeded",
            "inference",
            Some(100),
        ),
        (
            "req_no_evidence",
            "http",
            Some(200),
            true,
            "succeeded",
            "inference",
            None,
        ),
    ] {
        sqlx::query(
            "insert into model_requests (
                id, client_api_key_ref, config_revision, protocol, operation, endpoint,
                client_transport, outcome, total_tokens, started_at, deadline_at, completed_at,
                routing_scope, owner_user_id, provider_kind, request_kind,
                downstream_committed_at, client_status_code
             ) values (
                $1, 'key_ref', 1, 'openai', 'responses', '/v1/responses',
                $2, $3, $4, now(), now() + interval '1 minute', now(),
                'empty', $5, 'openai', $6, case when $7 then now() end, $8
             )",
        )
        .bind(id)
        .bind(transport)
        .bind(outcome)
        .bind(tokens)
        .bind(&one.id)
        .bind(kind)
        .bind(committed)
        .bind(status)
        .execute(&database.pool)
        .await
        .expect("seed usage boundary");
    }
    let summary = store.summary(&one.id, None, None).await.expect("summary");
    assert_eq!(summary.request_count, 2);
    assert_eq!(summary.total_tokens, 30);
    assert_eq!(summary.total_usd, "1.2500000000");
    // 概览沿用成功交付口径；即使当前 Key 不存在，也按请求冻结 owner 聚合。
    let now = Utc::now();
    let overview_query = gateway_portal::model::usage::PortalOverviewQuery {
        start: now - chrono::Duration::days(1),
        end: now + chrono::Duration::seconds(1),
        model: None,
    };
    let overview = store
        .overview(&one.id, overview_query.clone(), now)
        .await
        .expect("owner overview");
    assert_eq!(overview.summary.requests, 2);
    assert_eq!(overview.summary.total_tokens, 30);
    assert_eq!(
        overview
            .trend
            .iter()
            .map(|point| point.metrics.total_tokens)
            .sum::<u64>(),
        30
    );
    assert!(overview.summary.cost_incomplete);
    assert!(!overview.me.subscription_effective);
    sqlx::query(
        "update model_requests set requested_model_id = 'model-one' where id = 'req_usage_one'",
    )
    .execute(&database.pool)
    .await
    .unwrap();
    let filtered = store
        .overview(
            &one.id,
            gateway_portal::model::usage::PortalOverviewQuery {
                model: Some("model-one".to_owned()),
                ..overview_query
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(filtered.summary.requests, 1);
    assert_eq!(filtered.summary.total_tokens, 10);
    assert_eq!(
        filtered
            .health
            .iter()
            .map(|point| point.success)
            .sum::<u64>(),
        overview
            .health
            .iter()
            .map(|point| point.success)
            .sum::<u64>()
    );
    let future = Utc::now() + chrono::Duration::days(1);
    assert_eq!(
        store
            .summary(&one.id, Some(future), None)
            .await
            .expect("start filter")
            .request_count,
        0
    );
    let past = Utc::now() - chrono::Duration::days(1);
    assert_eq!(
        store
            .summary(&one.id, None, Some(past))
            .await
            .expect("end filter")
            .request_count,
        0
    );
    database.close().await;
}

#[tokio::test]
async fn usage_records_are_owner_scoped_and_keep_a_stable_key_reference() {
    let Some(database) = TestDatabase::create("portal_usage_records").await else {
        return;
    };
    let store = PgPortalStore::new(database.pool.clone());
    let one = store
        .create_user(
            CreatePortalUser {
                username: "records-one".to_owned(),
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
                username: "records-two".to_owned(),
                password: "unused-password".to_owned(),
            },
            "hash",
            &context(),
        )
        .await
        .expect("user two");

    for (key_id, key_name, owner, plaintext) in [
        (
            "key_usage_records_one",
            "主要密钥",
            one.id.as_str(),
            format!("sk_{}", "a".repeat(43)),
        ),
        (
            "key_usage_records_two",
            "其他密钥",
            two.id.as_str(),
            format!("sk_{}", "b".repeat(43)),
        ),
    ] {
        sqlx::query(
            "insert into client_api_keys
               (id, name, key, enabled, owner_user_id, created_at, updated_at)
             values ($1, $2, $3, true, $4, now(), now())",
        )
        .bind(key_id)
        .bind(key_name)
        .bind(plaintext)
        .bind(owner)
        .execute(&database.pool)
        .await
        .expect("seed portal usage key");
        sqlx::query(
            "insert into model_requests (
                id, client_api_key_id, client_api_key_ref, config_revision, protocol, operation,
                endpoint, client_transport, requested_model_id, outcome, input_tokens,
                output_tokens, total_tokens, cost_source, cost_amount, cost_currency, started_at,
                deadline_at, completed_at, routing_scope, owner_user_id
             ) values (
                $1, $2, $2, 1, 'openai', 'responses', '/v1/responses', 'http_json',
                'gpt-5.5', 'succeeded', 10, 5, 15, 'calculated', 0.25, 'USD', now(),
                now() + interval '1 minute', now(), 'empty', $3
             )",
        )
        .bind(format!("req_{key_id}"))
        .bind(key_id)
        .bind(owner)
        .execute(&database.pool)
        .await
        .expect("seed portal usage record");
    }
    sqlx::query(
        "update model_requests set provider_kind = 'openai',
        provider_account_authentication_kind_snapshot = 'oauth', upstream_model_id = 'gpt-5.5',
        reasoning_effort = 'high', upstream_transport = 'websocket', cached_tokens = 4,
        reasoning_tokens = 2, first_token_ms = 120, first_event_ms = 80, latency_ms = 200
        where id = 'req_key_usage_records_one'",
    )
    .execute(&database.pool)
    .await
    .expect("seed safe observation fields");

    let page = store
        .list_records(
            &one.id,
            PortalUsageQuery {
                model: None,
                start: None,
                end: None,
                cursor: None,
                page_size: 50,
            },
        )
        .await
        .expect("list owner usage records");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "req_key_usage_records_one");
    assert_eq!(page.items[0].key_id, "key_usage_records_one");
    assert_eq!(page.items[0].key_name.as_deref(), Some("主要密钥"));
    assert_eq!(page.items[0].key_prefix.as_deref(), Some("sk_aaaaaaa"));
    let record = &page.items[0];
    assert_eq!(record.provider.as_deref(), Some("openai"));
    assert_eq!(record.authentication_kind.as_deref(), Some("oauth"));
    assert_eq!(record.upstream_model.as_deref(), Some("gpt-5.5"));
    assert_eq!(record.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(record.client_transport, "http_json");
    assert_eq!(record.upstream_transport.as_deref(), Some("websocket"));
    assert_eq!(record.cached_tokens, Some(4));
    assert_eq!(record.reasoning_tokens, Some(2));
    assert_eq!(record.first_token_ms, Some(120));
    assert_eq!(record.first_event_ms, Some(80));
    assert_eq!(record.latency_ms, Some(200));
    assert!(record.cache_write_tokens.is_none());
    let other = store
        .list_records(
            &two.id,
            PortalUsageQuery {
                model: None,
                start: None,
                end: None,
                cursor: None,
                page_size: 1,
            },
        )
        .await
        .expect("other owner");
    assert_eq!(other.items.len(), 1);
    assert_eq!(other.items[0].id, "req_key_usage_records_two");
    assert!(other.items[0].provider.is_none());
    assert!(other.items[0].first_token_ms.is_none());

    sqlx::query("delete from client_api_keys where id = 'key_usage_records_one'")
        .execute(&database.pool)
        .await
        .expect("delete portal usage key");
    let deleted = store
        .list_records(
            &one.id,
            PortalUsageQuery {
                model: None,
                start: None,
                end: None,
                cursor: None,
                page_size: 50,
            },
        )
        .await
        .expect("list usage records after key deletion");
    assert_eq!(deleted.items[0].key_id, "key_usage_records_one");
    assert!(deleted.items[0].key_name.is_none());
    assert!(deleted.items[0].key_prefix.is_none());

    database.close().await;
}

#[tokio::test]
async fn portal_me_requires_enabled_plan_and_effective_subscription() {
    let Some(database) = TestDatabase::create("portal_me").await else {
        return;
    };
    let store = PgPortalStore::new(database.pool.clone());
    let group = group_id("dddddddddddddddddddddddddddddddd");
    seed_group(&database, &group, "Portal Me").await;
    let user = store
        .create_user(
            CreatePortalUser {
                username: "portal-me".to_owned(),
                password: "unused-password".to_owned(),
            },
            "hash",
            &context(),
        )
        .await
        .expect("user");
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros()).unwrap();
    assert!(
        !store
            .load_me(&user.id, now)
            .await
            .expect("no subscription")
            .subscription_effective
    );
    let plan = store
        .create_plan(
            CreatePlan {
                name: "portal-me".to_owned(),
                budget: Default::default(),
                limits: Default::default(),
                max_keys: 1,
                group_ids: vec![group],
            },
            &context(),
        )
        .await
        .expect("plan")
        .1;
    store
        .assign(
            AssignSubscription {
                user_id: user.id.clone(),
                plan_id: plan.id.clone(),
                starts_at: now,
                ends_at: Some(now + chrono::Duration::hours(1)),
            },
            &context(),
        )
        .await
        .expect("subscription");
    assert!(
        store
            .load_me(&user.id, now)
            .await
            .expect("effective at start")
            .subscription_effective
    );
    assert!(
        !store
            .load_me(&user.id, now - chrono::Duration::seconds(1))
            .await
            .expect("not started")
            .subscription_effective
    );
    assert!(
        !store
            .load_me(&user.id, now + chrono::Duration::hours(1))
            .await
            .expect("expired at end")
            .subscription_effective
    );
    sqlx::query("update subscription_plans set enabled = false where id = $1")
        .bind(&plan.id)
        .execute(&database.pool)
        .await
        .expect("disable plan");
    assert!(
        !store
            .load_me(&user.id, now)
            .await
            .expect("disabled plan")
            .subscription_effective
    );
    sqlx::query("update subscription_plans set enabled = true where id = $1")
        .bind(&plan.id)
        .execute(&database.pool)
        .await
        .expect("enable plan");
    store
        .disable(&user.id, &context())
        .await
        .expect("disable subscription");
    assert!(
        !store
            .load_me(&user.id, now)
            .await
            .expect("disabled subscription")
            .subscription_effective
    );
    database.close().await;
}
