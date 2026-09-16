//! Portal 端口的 PostgreSQL 实现。

use std::str::FromStr as _;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_core::{engine::budget::ClientBudgetLimits, metering::Decimal, policy::RateLimits};
use gateway_portal::{
    model::{
        MutationActor, MutationContext,
        auth::{PortalPrincipal, PortalSession},
        keys::{CreatePortalKey, PortalKeyRecord, UpdatePortalKey},
        plans::{CreatePlan, PlanListQuery, PlanPage, SubscriptionPlan, UpdatePlan},
        subscriptions::{AssignSubscription, SubscriptionStatus, UserSubscription},
        usage::{
            PortalMe, PortalUsagePage, PortalUsageQuery, PortalUsageRecord, PortalUsageSummary,
        },
        users::{
            CreatePortalUser, PortalUser, PortalUserListQuery, PortalUserPage, ResetPortalPassword,
            SetPortalUserEnabled, UserStatus,
        },
    },
    ports::store::{
        PortalAuthStore, PortalKeyStore, PortalPlanStore, PortalStoreError, PortalStoreErrorKind,
        PortalStoreResult, PortalSubscriptionStore, PortalUsageStore, PortalUserStore,
    },
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::require_nonempty;

use super::{
    NewClientApiKey, bump_config_revision_in_transaction, insert_client_api_key_in_transaction,
    replace_client_api_key_groups_in_transaction,
};

#[derive(Clone)]
pub struct PgPortalStore {
    pool: PgPool,
    observations: super::PgObservabilityRepository,
}

impl PgPortalStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self::with_query_budget(
            pool,
            super::ObservabilityQueryBudget::try_new(1, std::time::Duration::from_secs(3))
                .expect("positive query budget"),
        )
    }

    #[must_use]
    pub fn with_query_budget(pool: PgPool, budget: super::ObservabilityQueryBudget) -> Self {
        Self {
            observations: super::PgObservabilityRepository::new(pool.clone(), None, budget),
            pool,
        }
    }
}

fn store_error(kind: PortalStoreErrorKind, message: impl Into<String>) -> PortalStoreError {
    PortalStoreError::new(kind, "portal", message)
}

fn unavailable(message: &'static str) -> PortalStoreError {
    store_error(PortalStoreErrorKind::Unavailable, message)
}

fn map_sqlx(error: sqlx::Error, message: &'static str) -> PortalStoreError {
    if let Some(database) = error.as_database_error()
        && database.code().as_deref() == Some("23505")
    {
        return store_error(PortalStoreErrorKind::Conflict, "资源已存在");
    }
    unavailable(message)
}

fn actor_kind(actor: &MutationActor) -> &'static str {
    match actor {
        MutationActor::PortalSession { .. } => "portal_session",
        MutationActor::AdminSession { .. } | MutationActor::AdminApiKey => "admin_session",
        MutationActor::System => "system",
    }
}

fn actor_user_id(actor: &MutationActor) -> Option<&str> {
    match actor {
        MutationActor::PortalSession { user_id } => Some(user_id),
        _ => None,
    }
}

fn actor_ref(actor: &MutationActor) -> String {
    match actor {
        MutationActor::PortalSession { user_id } => format!("portal:{user_id}"),
        MutationActor::AdminSession { admin_user_id } => format!("admin:{admin_user_id}"),
        MutationActor::AdminApiKey => "admin-api-key".to_owned(),
        MutationActor::System => "system".to_owned(),
    }
}

async fn append_audit(
    tx: &mut Transaction<'_, Postgres>,
    context: &MutationContext,
    action: &str,
    entity_kind: &str,
    entity_ref: &str,
    revision: Option<i64>,
    changed_fields: &[&str],
) -> PortalStoreResult<()> {
    sqlx::query(
        "insert into portal_audit_events (
            id, actor_kind, actor_user_id, actor_ref, request_id, action, entity_kind, entity_ref,
            config_revision, changed_fields, created_at
         ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now())",
    )
    .bind(format!("audit_{}", Uuid::now_v7().simple()))
    .bind(actor_kind(&context.actor))
    .bind(actor_user_id(&context.actor))
    .bind(actor_ref(&context.actor))
    .bind(&context.request_id)
    .bind(action)
    .bind(entity_kind)
    .bind(entity_ref)
    .bind(revision)
    .bind(
        changed_fields
            .iter()
            .map(|field| (*field).to_owned())
            .collect::<Vec<_>>(),
    )
    .execute(&mut **tx)
    .await
    .map_err(|error| map_sqlx(error, "write portal audit"))?;
    Ok(())
}

fn parse_decimal(value: &str) -> PortalStoreResult<Decimal> {
    Decimal::from_str(value).map_err(|_| unavailable("decode decimal"))
}

fn budget_from_text(daily: String, weekly: String) -> PortalStoreResult<ClientBudgetLimits> {
    Ok(ClientBudgetLimits {
        daily_usd: parse_decimal(&daily)?,
        weekly_usd: parse_decimal(&weekly)?,
    })
}

fn to_u64(value: i64) -> PortalStoreResult<u64> {
    u64::try_from(value).map_err(|_| unavailable("negative limit"))
}

fn to_i64(value: u64) -> PortalStoreResult<i64> {
    i64::try_from(value).map_err(|_| store_error(PortalStoreErrorKind::Invalid, "limit overflow"))
}

fn exceeds_plan(value: u64, plan: u64) -> bool {
    plan != 0 && value != 0 && value > plan
}

fn exceeds_plan_decimal(value: Decimal, plan: Decimal) -> bool {
    plan != Decimal::ZERO && value != Decimal::ZERO && value > plan
}

fn user_from_row(row: sqlx::postgres::PgRow) -> PortalStoreResult<PortalUser> {
    Ok(PortalUser {
        id: row.get("id"),
        username: row.get("username"),
        status: UserStatus::parse(row.get("status"))
            .map_err(|_| unavailable("decode user status"))?,
        plan_id: row.get("plan_id"),
        plan_name: row.get("plan_name"),
        subscription_starts_at: row.get("starts_at"),
        subscription_ends_at: row.get("ends_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

async fn load_effective_plan(
    tx: &mut Transaction<'_, Postgres>,
    user_id: &str,
    now: DateTime<Utc>,
) -> PortalStoreResult<SubscriptionPlan> {
    let row = sqlx::query(
        "select p.id, p.name, p.daily_limit_usd::text, p.weekly_limit_usd::text,
                p.max_concurrency, p.requests_per_minute, p.max_keys, p.enabled,
                p.created_at, p.updated_at, s.starts_at, s.ends_at, s.status
         from user_subscriptions s
         join subscription_plans p on p.id = s.plan_id
         where s.user_id = $1 and s.status = 'active'
         for update of s, p",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| map_sqlx(error, "load subscription"))?
    .ok_or_else(|| store_error(PortalStoreErrorKind::Conflict, "用户没有有效订阅"))?;
    let starts_at: DateTime<Utc> = row.get("starts_at");
    let ends_at: Option<DateTime<Utc>> = row.get("ends_at");
    let status: String = row.get("status");
    let enabled: bool = row.get("enabled");
    if status != "active" || !enabled || starts_at > now || ends_at.is_some_and(|ends| now >= ends)
    {
        return Err(store_error(
            PortalStoreErrorKind::Conflict,
            "用户没有有效订阅",
        ));
    }
    let plan_id: String = row.get("id");
    let group_ids = sqlx::query_scalar::<_, String>(
        "select account_group_id from plan_account_groups where plan_id = $1 order by account_group_id",
    )
    .bind(&plan_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|error| map_sqlx(error, "load plan groups"))?;
    if group_ids.is_empty() {
        return Err(store_error(
            PortalStoreErrorKind::Conflict,
            "套餐未绑定账号分组",
        ));
    }
    Ok(SubscriptionPlan {
        id: plan_id,
        name: row.get("name"),
        budget: budget_from_text(row.get("daily_limit_usd"), row.get("weekly_limit_usd"))?,
        limits: RateLimits {
            max_concurrency: to_u64(row.get("max_concurrency"))?,
            requests_per_minute: to_u64(row.get("requests_per_minute"))?,
        },
        max_keys: to_u64(row.get("max_keys"))?,
        group_ids,
        enabled: row.get("enabled"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[async_trait]
impl PortalAuthStore for PgPortalStore {
    async fn change_password(
        &self,
        user_id: &str,
        expected_hash: &str,
        new_hash: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin change password"))?;
        let row = sqlx::query_as::<_, (String, String)>(
            "select status, password_hash from portal_users where id = $1 for update",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "lock portal user for password change"))?;
        if !row.is_some_and(|(status, hash)| status == "active" && hash == expected_hash) {
            return Err(store_error(
                PortalStoreErrorKind::Conflict,
                "用户凭据已变化，请重新登录后重试",
            ));
        }
        sqlx::query("update portal_users set password_hash = $2, updated_at = now() where id = $1")
            .bind(user_id)
            .bind(new_hash)
            .execute(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "change portal password"))?;
        sqlx::query("delete from portal_sessions where user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "revoke sessions after password change"))?;
        append_audit(
            &mut tx,
            context,
            "change_password",
            "portal_user",
            user_id,
            None,
            &["password_hash"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit password change"))?;
        Ok(())
    }

    async fn load_password_hash(
        &self,
        username: &str,
    ) -> PortalStoreResult<Option<(String, String, String)>> {
        sqlx::query_as::<_, (String, String, String)>(
            "select id, status, password_hash from portal_users where lower(username) = lower($1)",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "load portal password"))
    }

    async fn store_session(
        &self,
        session: &PortalSession,
        password_hash: &str,
    ) -> PortalStoreResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin store portal session"))?;
        sqlx::query_scalar::<_, String>("select id from portal_users where id = $1 for update")
            .bind(&session.user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "lock portal user for session"))?
            .ok_or_else(|| store_error(PortalStoreErrorKind::Conflict, "用户不可登录"))?;
        let inserted = sqlx::query(
            "insert into portal_sessions (id, user_id, token_hash, expires_at, created_at)
             select $1, $2, $3, $4, now()
             from portal_users
             where id = $2 and status = 'active' and password_hash = $5",
        )
        .bind(&session.id)
        .bind(&session.user_id)
        .bind(&session.token_hash)
        .bind(session.expires_at)
        .bind(password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "store portal session"))?;
        if inserted.rows_affected() == 0 {
            return Err(store_error(
                PortalStoreErrorKind::Conflict,
                "用户凭据已失效",
            ));
        }
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit portal session"))?;
        Ok(())
    }

    async fn load_session_by_token_hash(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> PortalStoreResult<Option<PortalPrincipal>> {
        sqlx::query_as::<_, (String, String)>(
            "select u.id, u.username
             from portal_sessions s
             join portal_users u on u.id = s.user_id
             where s.token_hash = $1 and s.expires_at > $2 and u.status = 'active'",
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "load portal session"))
        .map(|row| row.map(|(user_id, username)| PortalPrincipal { user_id, username }))
    }

    async fn delete_session_by_token_hash(&self, token_hash: &str) -> PortalStoreResult<()> {
        sqlx::query("delete from portal_sessions where token_hash = $1")
            .bind(token_hash)
            .execute(&self.pool)
            .await
            .map_err(|error| map_sqlx(error, "delete portal session"))?;
        Ok(())
    }

    async fn delete_sessions_for_user(&self, user_id: &str) -> PortalStoreResult<()> {
        sqlx::query("delete from portal_sessions where user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|error| map_sqlx(error, "revoke portal sessions"))?;
        Ok(())
    }
}

#[async_trait]
impl PortalUserStore for PgPortalStore {
    async fn create_user(
        &self,
        command: CreatePortalUser,
        password_hash: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<PortalUser> {
        let id = format!("usr_{}", Uuid::now_v7().simple());
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin create user"))?;
        sqlx::query(
            "insert into portal_users (id, username, password_hash, status, created_at, updated_at)
             values ($1, $2, $3, 'active', now(), now())",
        )
        .bind(&id)
        .bind(command.username.trim())
        .bind(password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "insert portal user"))?;
        append_audit(
            &mut tx,
            context,
            "create",
            "portal_user",
            &id,
            None,
            &["username", "status"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit create user"))?;
        self.load_user(&id)
            .await?
            .ok_or_else(|| unavailable("reload portal user"))
    }

    async fn list_users(&self, query: PortalUserListQuery) -> PortalStoreResult<PortalUserPage> {
        let page = i64::from(query.page.max(1));
        let page_size = i64::from(query.page_size.clamp(1, 200));
        let offset = (page - 1) * page_size;
        let search = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let total = sqlx::query_scalar::<_, i64>(
            "select count(*) from portal_users
             where $1::text is null or lower(username) like lower($1)",
        )
        .bind(search.map(|value| format!("%{value}%")))
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "count portal users"))?;
        let rows = sqlx::query(
            "select u.id, u.username, u.status, u.created_at, u.updated_at,
                    s.plan_id, p.name as plan_name, s.starts_at, s.ends_at
             from portal_users u
             left join user_subscriptions s on s.user_id = u.id and s.status = 'active'
             left join subscription_plans p on p.id = s.plan_id
             where $1::text is null or lower(u.username) like lower($1)
             order by u.created_at desc, u.id desc
             limit $2 offset $3",
        )
        .bind(search.map(|value| format!("%{value}%")))
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "list portal users"))?;
        let items = rows
            .into_iter()
            .map(user_from_row)
            .collect::<PortalStoreResult<Vec<_>>>()?;
        Ok(PortalUserPage {
            items,
            total: to_u64(total)?,
        })
    }

    async fn load_user(&self, user_id: &str) -> PortalStoreResult<Option<PortalUser>> {
        let row = sqlx::query(
            "select u.id, u.username, u.status, u.created_at, u.updated_at,
                    s.plan_id, p.name as plan_name, s.starts_at, s.ends_at
             from portal_users u
             left join user_subscriptions s on s.user_id = u.id and s.status = 'active'
             left join subscription_plans p on p.id = s.plan_id
             where u.id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "load portal user"))?;
        row.map(user_from_row).transpose()
    }

    async fn reset_password(
        &self,
        command: ResetPortalPassword,
        password_hash: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin reset password"))?;
        sqlx::query_scalar::<_, String>("select id from portal_users where id = $1 for update")
            .bind(&command.user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "lock portal user for reset"))?
            .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "用户不存在"))?;
        let result = sqlx::query(
            "update portal_users set password_hash = $2, updated_at = now() where id = $1",
        )
        .bind(&command.user_id)
        .bind(password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "reset portal password"))?;
        if result.rows_affected() == 0 {
            return Err(store_error(PortalStoreErrorKind::NotFound, "用户不存在"));
        }
        sqlx::query("delete from portal_sessions where user_id = $1")
            .bind(&command.user_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "revoke sessions after reset"))?;
        append_audit(
            &mut tx,
            context,
            "reset_password",
            "portal_user",
            &command.user_id,
            None,
            &["password_hash"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit reset password"))?;
        Ok(())
    }

    async fn set_enabled(
        &self,
        command: SetPortalUserEnabled,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalUser)> {
        let status = if command.enabled {
            "active"
        } else {
            "disabled"
        };
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin set user enabled"))?;
        sqlx::query_scalar::<_, String>("select id from portal_users where id = $1 for update")
            .bind(&command.user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "lock portal user for status"))?
            .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "用户不存在"))?;
        sqlx::query("update portal_users set status = $2, updated_at = now() where id = $1")
            .bind(&command.user_id)
            .bind(status)
            .execute(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "set portal user status"))?;
        if !command.enabled {
            sqlx::query("delete from portal_sessions where user_id = $1")
                .bind(&command.user_id)
                .execute(&mut *tx)
                .await
                .map_err(|error| map_sqlx(error, "revoke sessions after disable"))?;
        }
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            if command.enabled { "enable" } else { "disable" },
            "portal_user",
            &command.user_id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["status"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit set user enabled"))?;
        let user = self
            .load_user(&command.user_id)
            .await?
            .ok_or_else(|| unavailable("reload portal user"))?;
        Ok((revision.get(), user))
    }
}

async fn replace_plan_groups(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: &str,
    group_ids: &[String],
) -> PortalStoreResult<()> {
    sqlx::query("delete from plan_account_groups where plan_id = $1")
        .bind(plan_id)
        .execute(&mut **tx)
        .await
        .map_err(|error| map_sqlx(error, "clear plan groups"))?;
    for group_id in group_ids {
        sqlx::query(
            "insert into plan_account_groups (plan_id, account_group_id, created_at)
             values ($1, $2, now())",
        )
        .bind(plan_id)
        .bind(group_id)
        .execute(&mut **tx)
        .await
        .map_err(|error| map_sqlx(error, "insert plan group"))?;
    }
    Ok(())
}

async fn sync_owned_key_groups(
    tx: &mut Transaction<'_, Postgres>,
    plan_id: &str,
    group_ids: &[String],
) -> PortalStoreResult<()> {
    let key_ids = sqlx::query_scalar::<_, String>(
        "select k.id from client_api_keys k
         join user_subscriptions s on s.user_id = k.owner_user_id
         where s.plan_id = $1 and s.status = 'active' and k.owner_user_id is not null",
    )
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|error| map_sqlx(error, "list plan keys"))?;
    for key_id in key_ids {
        replace_client_api_key_groups_in_transaction(tx, &key_id, group_ids)
            .await
            .map_err(|_| unavailable("replace owned key groups"))?;
    }
    Ok(())
}

#[async_trait]
impl PortalPlanStore for PgPortalStore {
    async fn create_plan(
        &self,
        command: CreatePlan,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)> {
        if command.group_ids.is_empty() {
            return Err(store_error(
                PortalStoreErrorKind::Invalid,
                "套餐必须绑定至少一个账号分组",
            ));
        }
        let id = format!("plan_{}", Uuid::now_v7().simple());
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin create plan"))?;
        sqlx::query(
            "insert into subscription_plans (
                id, name, daily_limit_usd, weekly_limit_usd, max_concurrency, requests_per_minute,
                max_keys, enabled, created_at, updated_at
             ) values ($1, $2, $3::text::numeric, $4::text::numeric, $5, $6, $7, true, now(), now())",
        )
        .bind(&id)
        .bind(command.name.trim())
        .bind(command.budget.daily_usd.canonical())
        .bind(command.budget.weekly_usd.canonical())
        .bind(to_i64(command.limits.max_concurrency)?)
        .bind(to_i64(command.limits.requests_per_minute)?)
        .bind(to_i64(command.max_keys)?)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "insert plan"))?;
        replace_plan_groups(&mut tx, &id, &command.group_ids).await?;
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            "create",
            "subscription_plan",
            &id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["name", "group_ids"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit create plan"))?;
        let plan = self
            .load_plan(&id)
            .await?
            .ok_or_else(|| unavailable("reload plan"))?;
        Ok((revision.get(), plan))
    }

    async fn update_plan(
        &self,
        command: UpdatePlan,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)> {
        if command.group_ids.is_empty() {
            return Err(store_error(
                PortalStoreErrorKind::Invalid,
                "套餐必须绑定至少一个账号分组",
            ));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin update plan"))?;
        let user_ids = sqlx::query_scalar::<_, String>(
            "select user_id from user_subscriptions
             where plan_id = $1 and status = 'active'
             order by user_id",
        )
        .bind(&command.id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "list plan subscribers"))?;
        for user_id in user_ids {
            sqlx::query_scalar::<_, String>("select id from portal_users where id = $1 for update")
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|error| map_sqlx(error, "lock plan user"))?;
        }
        sqlx::query_scalar::<_, String>(
            "select id from subscription_plans where id = $1 for update",
        )
        .bind(&command.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "lock plan"))?
        .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "套餐不存在"))?;
        let result = sqlx::query(
            "update subscription_plans set
                name = $2, daily_limit_usd = $3::text::numeric, weekly_limit_usd = $4::text::numeric,
                max_concurrency = $5, requests_per_minute = $6, max_keys = $7, enabled = $8,
                updated_at = now()
             where id = $1",
        )
        .bind(&command.id)
        .bind(command.name.trim())
        .bind(command.budget.daily_usd.canonical())
        .bind(command.budget.weekly_usd.canonical())
        .bind(to_i64(command.limits.max_concurrency)?)
        .bind(to_i64(command.limits.requests_per_minute)?)
        .bind(to_i64(command.max_keys)?)
        .bind(command.enabled)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "update plan"))?;
        if result.rows_affected() == 0 {
            return Err(store_error(PortalStoreErrorKind::NotFound, "套餐不存在"));
        }
        replace_plan_groups(&mut tx, &command.id, &command.group_ids).await?;
        sync_owned_key_groups(&mut tx, &command.id, &command.group_ids).await?;
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            "update",
            "subscription_plan",
            &command.id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["name", "group_ids", "limits"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit update plan"))?;
        let plan = self
            .load_plan(&command.id)
            .await?
            .ok_or_else(|| unavailable("reload plan"))?;
        Ok((revision.get(), plan))
    }

    async fn list_plans(&self, query: PlanListQuery) -> PortalStoreResult<PlanPage> {
        let page = i64::from(query.page.max(1));
        let page_size = i64::from(query.page_size.clamp(1, 200));
        let offset = (page - 1) * page_size;
        let total = sqlx::query_scalar::<_, i64>("select count(*) from subscription_plans")
            .fetch_one(&self.pool)
            .await
            .map_err(|error| map_sqlx(error, "count plans"))?;
        let ids = sqlx::query_scalar::<_, String>(
            "select id from subscription_plans order by created_at desc, id desc limit $1 offset $2",
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "list plans"))?;
        let mut items = Vec::new();
        for id in ids {
            if let Some(plan) = self.load_plan(&id).await? {
                items.push(plan);
            }
        }
        Ok(PlanPage {
            items,
            total: to_u64(total)?,
        })
    }

    async fn load_plan(&self, plan_id: &str) -> PortalStoreResult<Option<SubscriptionPlan>> {
        let row = sqlx::query(
            "select id, name, daily_limit_usd::text, weekly_limit_usd::text, max_concurrency,
                    requests_per_minute, max_keys, enabled, created_at, updated_at
             from subscription_plans where id = $1",
        )
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "load plan"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let group_ids = sqlx::query_scalar::<_, String>(
            "select account_group_id from plan_account_groups where plan_id = $1 order by account_group_id",
        )
        .bind(plan_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "load plan groups"))?;
        Ok(Some(SubscriptionPlan {
            id: row.get("id"),
            name: row.get("name"),
            budget: budget_from_text(row.get("daily_limit_usd"), row.get("weekly_limit_usd"))?,
            limits: RateLimits {
                max_concurrency: to_u64(row.get("max_concurrency"))?,
                requests_per_minute: to_u64(row.get("requests_per_minute"))?,
            },
            max_keys: to_u64(row.get("max_keys"))?,
            group_ids,
            enabled: row.get("enabled"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }
}

#[async_trait]
impl PortalSubscriptionStore for PgPortalStore {
    async fn assign(
        &self,
        command: AssignSubscription,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, UserSubscription)> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin assign subscription"))?;
        sqlx::query_scalar::<_, String>("select id from portal_users where id = $1 for update")
            .bind(&command.user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "lock user for assign"))?
            .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "用户不存在"))?;
        sqlx::query_scalar::<_, String>(
            "select id from subscription_plans where id = $1 for update",
        )
        .bind(&command.plan_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "lock plan for assign"))?
        .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "套餐不存在"))?;
        sqlx::query(
            "update user_subscriptions set status = 'disabled', updated_at = now()
             where user_id = $1 and status = 'active'",
        )
        .bind(&command.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "disable previous subscription"))?;
        let id = format!("sub_{}", Uuid::now_v7().simple());
        sqlx::query(
            "insert into user_subscriptions (
                id, user_id, plan_id, status, starts_at, ends_at, created_at, updated_at
             ) values ($1, $2, $3, 'active', $4, $5, now(), now())",
        )
        .bind(&id)
        .bind(&command.user_id)
        .bind(&command.plan_id)
        .bind(command.starts_at)
        .bind(command.ends_at)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "insert subscription"))?;
        let groups = sqlx::query_scalar::<_, String>(
            "select account_group_id from plan_account_groups where plan_id = $1",
        )
        .bind(&command.plan_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "load assigned plan groups"))?;
        if groups.is_empty() {
            return Err(store_error(
                PortalStoreErrorKind::Invalid,
                "套餐必须绑定至少一个账号分组",
            ));
        }
        let key_ids = sqlx::query_scalar::<_, String>(
            "select id from client_api_keys where owner_user_id = $1",
        )
        .bind(&command.user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "list user keys"))?;
        for key_id in key_ids {
            replace_client_api_key_groups_in_transaction(&mut tx, &key_id, &groups)
                .await
                .map_err(|_| unavailable("replace key groups on assign"))?;
        }
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            "assign",
            "user_subscription",
            &id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["plan_id", "starts_at", "ends_at"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit assign subscription"))?;
        let subscription = self
            .load_current(&command.user_id)
            .await?
            .ok_or_else(|| unavailable("reload subscription"))?;
        Ok((revision.get(), subscription))
    }

    async fn disable(
        &self,
        user_id: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, Option<UserSubscription>)> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin disable subscription"))?;
        sqlx::query(
            "update user_subscriptions set status = 'disabled', updated_at = now()
             where user_id = $1 and status = 'active'",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "disable subscription"))?;
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            "disable",
            "user_subscription",
            user_id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["status"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit disable subscription"))?;
        Ok((revision.get(), self.load_current(user_id).await?))
    }

    async fn load_current(&self, user_id: &str) -> PortalStoreResult<Option<UserSubscription>> {
        let row = sqlx::query(
            "select id, user_id, plan_id, status, starts_at, ends_at, created_at, updated_at
             from user_subscriptions where user_id = $1
             order by created_at desc, id desc limit 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "load current subscription"))?;
        row.map(|row| {
            let status = match row.get::<String, _>("status").as_str() {
                "active" => SubscriptionStatus::Active,
                _ => SubscriptionStatus::Disabled,
            };
            Ok(UserSubscription {
                id: row.get("id"),
                user_id: row.get("user_id"),
                plan_id: row.get("plan_id"),
                status,
                starts_at: row.get("starts_at"),
                ends_at: row.get("ends_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
        })
        .transpose()
    }
}

async fn load_owned_key_row(
    pool: &PgPool,
    key_id: &str,
    user_id: &str,
) -> PortalStoreResult<sqlx::postgres::PgRow> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "{KEY_SELECT} where k.id = $1 and k.owner_user_id = $2"
    )))
    .bind(key_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| map_sqlx(error, "load owned key"))
}

fn key_record_from_row(row: sqlx::postgres::PgRow) -> PortalStoreResult<PortalKeyRecord> {
    Ok(PortalKeyRecord {
        id: row.get("id"),
        name: row.get("name"),
        label: row.get("label"),
        prefix: row.get("prefix"),
        enabled: row.get("enabled"),
        limits: RateLimits {
            max_concurrency: to_u64(row.get("max_concurrency"))?,
            requests_per_minute: to_u64(row.get("requests_per_minute"))?,
        },
        budget: budget_from_text(row.get("daily_limit_usd"), row.get("weekly_limit_usd"))?,
        daily_used_usd: row.get("daily_used"),
        weekly_used_usd: row.get("weekly_used"),
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
    })
}

const KEY_SELECT: &str = "select k.id, k.name, k.label, left(k.key, 10) as prefix, k.enabled,
        k.max_concurrency, k.requests_per_minute, k.daily_limit_usd::text, k.weekly_limit_usd::text,
        coalesce((case when w.daily_end > now() then w.daily_used_usd else 0 end)::text, '0') as daily_used,
        coalesce((case when w.weekly_end > now() then w.weekly_used_usd else 0 end)::text, '0') as weekly_used,
        k.last_used_at, k.created_at
 from client_api_keys k
 left join client_key_budget_windows w on w.client_api_key_id = k.id";

#[async_trait]
impl PortalKeyStore for PgPortalStore {
    async fn list_keys(&self, user_id: &str) -> PortalStoreResult<Vec<PortalKeyRecord>> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{KEY_SELECT} where k.owner_user_id = $1 order by k.created_at desc, k.id desc"
        )))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "list portal keys"))?;
        rows.into_iter().map(key_record_from_row).collect()
    }

    async fn create_key(
        &self,
        user_id: &str,
        command: CreatePortalKey,
        plaintext: &str,
        key_id: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)> {
        require_nonempty("portal key", "name", &command.name)
            .map_err(|_| store_error(PortalStoreErrorKind::Invalid, "名称不合法"))?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin create key"))?;
        sqlx::query_scalar::<_, String>("select id from portal_users where id = $1 for update")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "lock portal user"))?
            .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "用户不存在"))?;
        let plan = load_effective_plan(&mut tx, user_id, Utc::now()).await?;
        if exceeds_plan(command.limits.max_concurrency, plan.limits.max_concurrency)
            || exceeds_plan(
                command.limits.requests_per_minute,
                plan.limits.requests_per_minute,
            )
            || exceeds_plan_decimal(command.budget.daily_usd, plan.budget.daily_usd)
            || exceeds_plan_decimal(command.budget.weekly_usd, plan.budget.weekly_usd)
        {
            return Err(store_error(
                PortalStoreErrorKind::Invalid,
                "Key 限额不能高于套餐",
            ));
        }
        let key_count = sqlx::query_scalar::<_, i64>(
            "select count(*) from client_api_keys where owner_user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "count user keys"))?;
        if plan.max_keys != 0 && to_u64(key_count)? >= plan.max_keys {
            return Err(store_error(
                PortalStoreErrorKind::Conflict,
                "已达到套餐允许的 Key 数量",
            ));
        }
        insert_client_api_key_in_transaction(
            &mut tx,
            &NewClientApiKey {
                id: key_id.to_owned(),
                name: command.name,
                label: command.label,
                group_ids: plan.group_ids.clone(),
                key: plaintext.to_owned(),
                max_concurrency: command.limits.max_concurrency,
                requests_per_minute: command.limits.requests_per_minute,
                budget: command.budget,
                owner_user_id: Some(user_id.to_owned()),
            },
        )
        .await
        .map_err(|_| unavailable("insert owned client key"))?;
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            "create",
            "client_api_key",
            key_id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["name", "owner_user_id", "group_ids"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit create key"))?;
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{KEY_SELECT} where k.id = $1 and k.owner_user_id = $2"
        )))
        .bind(key_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "reload created key"))?;
        Ok((revision.get(), key_record_from_row(row)?))
    }

    async fn update_key(
        &self,
        user_id: &str,
        command: UpdatePortalKey,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin update key"))?;
        sqlx::query_scalar::<_, String>("select id from portal_users where id = $1 for update")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| map_sqlx(error, "lock portal user"))?
            .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "用户不存在"))?;
        let plan = load_effective_plan(&mut tx, user_id, Utc::now()).await?;
        if exceeds_plan(command.limits.max_concurrency, plan.limits.max_concurrency)
            || exceeds_plan(
                command.limits.requests_per_minute,
                plan.limits.requests_per_minute,
            )
            || exceeds_plan_decimal(command.budget.daily_usd, plan.budget.daily_usd)
            || exceeds_plan_decimal(command.budget.weekly_usd, plan.budget.weekly_usd)
        {
            return Err(store_error(
                PortalStoreErrorKind::Invalid,
                "Key 限额不能高于套餐",
            ));
        }
        let result = sqlx::query(
            "update client_api_keys set name = $3, label = $4, max_concurrency = $5,
                    requests_per_minute = $6, daily_limit_usd = $7::text::numeric,
                    weekly_limit_usd = $8::text::numeric, updated_at = now()
             where id = $1 and owner_user_id = $2",
        )
        .bind(&command.id)
        .bind(user_id)
        .bind(&command.name)
        .bind(&command.label)
        .bind(to_i64(command.limits.max_concurrency)?)
        .bind(to_i64(command.limits.requests_per_minute)?)
        .bind(command.budget.daily_usd.canonical())
        .bind(command.budget.weekly_usd.canonical())
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "update owned key"))?;
        if result.rows_affected() == 0 {
            return Err(store_error(PortalStoreErrorKind::NotFound, "密钥不存在"));
        }
        replace_client_api_key_groups_in_transaction(&mut tx, &command.id, &plan.group_ids)
            .await
            .map_err(|_| unavailable("refresh owned key groups"))?;
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            "update",
            "client_api_key",
            &command.id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["name", "limits"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit update key"))?;
        let row = load_owned_key_row(&self.pool, &command.id, user_id).await?;
        Ok((revision.get(), key_record_from_row(row)?))
    }

    async fn set_enabled(
        &self,
        user_id: &str,
        key_id: &str,
        enabled: bool,
        context: &MutationContext,
    ) -> PortalStoreResult<(u64, PortalKeyRecord)> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin set key enabled"))?;
        let result = sqlx::query(
            "update client_api_keys set enabled = $3, updated_at = now()
             where id = $1 and owner_user_id = $2",
        )
        .bind(key_id)
        .bind(user_id)
        .bind(enabled)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_sqlx(error, "set owned key enabled"))?;
        if result.rows_affected() == 0 {
            return Err(store_error(PortalStoreErrorKind::NotFound, "密钥不存在"));
        }
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            if enabled { "enable" } else { "disable" },
            "client_api_key",
            key_id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["enabled"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit set key enabled"))?;
        let row = load_owned_key_row(&self.pool, key_id, user_id).await?;
        Ok((revision.get(), key_record_from_row(row)?))
    }

    async fn delete_key(
        &self,
        user_id: &str,
        key_id: &str,
        context: &MutationContext,
    ) -> PortalStoreResult<u64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| map_sqlx(error, "begin delete key"))?;
        let result =
            sqlx::query("delete from client_api_keys where id = $1 and owner_user_id = $2")
                .bind(key_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await
                .map_err(|error| map_sqlx(error, "delete owned key"))?;
        if result.rows_affected() == 0 {
            return Err(store_error(PortalStoreErrorKind::NotFound, "密钥不存在"));
        }
        let revision = bump_config_revision_in_transaction(&mut tx)
            .await
            .map_err(|_| unavailable("bump revision"))?;
        append_audit(
            &mut tx,
            context,
            "delete",
            "client_api_key",
            key_id,
            Some(i64::try_from(revision.get()).unwrap_or(1)),
            &["id"],
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| map_sqlx(error, "commit delete key"))?;
        Ok(revision.get())
    }

    async fn reveal_key(
        &self,
        user_id: &str,
        key_id: &str,
    ) -> PortalStoreResult<Option<(PortalKeyRecord, String)>> {
        let plaintext = sqlx::query_scalar::<_, String>(
            "select key from client_api_keys where id = $1 and owner_user_id = $2",
        )
        .bind(key_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "reveal owned key"))?;
        let Some(plaintext) = plaintext else {
            return Ok(None);
        };
        let row = load_owned_key_row(&self.pool, key_id, user_id).await?;
        Ok(Some((key_record_from_row(row)?, plaintext)))
    }
}

fn portal_metrics(
    metrics: &super::RequestMetrics,
    costs: &[super::CurrencyCostTotal],
    coverage: &super::CostCoverage,
) -> gateway_portal::model::usage::PortalUsageMetrics {
    gateway_portal::model::usage::PortalUsageMetrics {
        requests: metrics.request_count,
        input_tokens: metrics.input_tokens,
        output_tokens: metrics.output_tokens,
        cached_tokens: metrics.cached_tokens,
        cache_write_tokens: metrics.cache_write_tokens,
        reasoning_tokens: metrics.reasoning_tokens,
        total_tokens: metrics.total_tokens,
        cost_usd: costs
            .iter()
            .find(|cost| cost.currency.eq_ignore_ascii_case("USD"))
            .map(|cost| cost.amount.as_str().to_owned())
            .or_else(|| (metrics.request_count == 0).then(|| "0".to_owned())),
        cost_incomplete: coverage.unavailable_count > 0,
    }
}

#[async_trait]
impl PortalUsageStore for PgPortalStore {
    async fn overview(
        &self,
        user_id: &str,
        query: gateway_portal::model::usage::PortalOverviewQuery,
        now: DateTime<Utc>,
    ) -> PortalStoreResult<gateway_portal::model::usage::PortalUsageOverview> {
        use super::{ObservabilityRange, ObservabilityRepository as _, UsageRecordFilter};
        use gateway_portal::model::usage::{
            PortalHealthPoint, PortalTrendPoint, PortalUsageOverview,
        };
        let range = ObservabilityRange::new(query.start, query.end)
            .map_err(|_| store_error(PortalStoreErrorKind::Invalid, "时间范围不合法"))?;
        let filter = UsageRecordFilter {
            owner_user_id: Some(user_id.to_owned()),
            model: query.model.clone(),
            completed_only: true,
            ..UsageRecordFilter::default()
        };
        let summary = self
            .observations
            .usage_summary(range, filter.clone())
            .await
            .map_err(|_| unavailable("portal overview summary"))?;
        let trend = self
            .observations
            .usage_trend(range, filter)
            .await
            .map_err(|_| unavailable("portal overview trend"))?;
        let day_start = DateTime::from_timestamp(
            (now.timestamp() + 8 * 3600).div_euclid(86400) * 86400 - 8 * 3600,
            0,
        )
        .ok_or_else(|| unavailable("portal day range"))?;
        let health = self
            .observations
            .usage_trend(
                ObservabilityRange {
                    start: day_start,
                    end: now,
                },
                UsageRecordFilter {
                    owner_user_id: Some(user_id.to_owned()),
                    ..UsageRecordFilter::default()
                },
            )
            .await
            .map_err(|_| unavailable("portal health"))?;
        let resets = sqlx::query(
            "select daily_end, weekly_end from portal_user_budget_windows where user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "portal budget resets"))?;
        Ok(PortalUsageOverview {
            as_of: now,
            query,
            me: self.load_me(user_id, now).await?,
            daily_resets_at: resets
                .as_ref()
                .and_then(|row| row.get::<Option<DateTime<Utc>>, _>("daily_end"))
                .filter(|end| *end > now),
            weekly_resets_at: resets
                .as_ref()
                .and_then(|row| row.get::<Option<DateTime<Utc>>, _>("weekly_end"))
                .filter(|end| *end > now),
            summary: portal_metrics(
                &summary.requests,
                &summary.attempts.costs,
                &summary.attempts.cost_coverage,
            ),
            trend: trend
                .into_iter()
                .map(|point| PortalTrendPoint {
                    time: point.bucket_start,
                    bucket_seconds: point.granularity.seconds(),
                    metrics: portal_metrics(&point.metrics, &point.costs, &point.cost_coverage),
                })
                .collect(),
            health: health
                .into_iter()
                .map(|point| PortalHealthPoint {
                    time: point.bucket_start,
                    success: point.metrics.success_count,
                    failed: point.metrics.failure_count,
                    cancelled: point.metrics.cancelled_count,
                    incomplete: point.metrics.incomplete_count,
                    caller_error: point.metrics.caller_error_count,
                })
                .collect(),
        })
    }
    async fn load_me(&self, user_id: &str, now: DateTime<Utc>) -> PortalStoreResult<PortalMe> {
        let user = sqlx::query("select id, username from portal_users where id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| map_sqlx(error, "load me"))?
            .ok_or_else(|| store_error(PortalStoreErrorKind::NotFound, "用户不存在"))?;
        let subscription = self.load_current(user_id).await?;
        let plan = if let Some(subscription) = subscription.as_ref() {
            self.load_plan(&subscription.plan_id).await?
        } else {
            None
        };
        let window = sqlx::query(
            "select coalesce((case when daily_end > now() then daily_used_usd else 0 end)::text, '0'),
                    coalesce((case when weekly_end > now() then weekly_used_usd else 0 end)::text, '0')
             from portal_user_budget_windows where user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "load user budget"))?;
        let (daily_used, weekly_used) = window
            .map(|row| (row.get::<String, _>(0), row.get::<String, _>(1)))
            .unwrap_or_else(|| ("0".to_owned(), "0".to_owned()));
        let key_count = sqlx::query_scalar::<_, i64>(
            "select count(*) from client_api_keys where owner_user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "count keys"))?;
        Ok(PortalMe {
            user_id: user.get("id"),
            username: user.get("username"),
            plan_name: plan.as_ref().map(|plan| plan.name.clone()),
            subscription_ends_at: subscription.as_ref().and_then(|item| item.ends_at),
            subscription_effective: plan.as_ref().is_some_and(|plan| plan.enabled)
                && subscription
                    .as_ref()
                    .is_some_and(|item| item.is_effective(now)),
            daily_limit_usd: plan
                .as_ref()
                .map(|plan| plan.budget.daily_usd.canonical())
                .unwrap_or_else(|| "0".to_owned()),
            weekly_limit_usd: plan
                .as_ref()
                .map(|plan| plan.budget.weekly_usd.canonical())
                .unwrap_or_else(|| "0".to_owned()),
            daily_used_usd: daily_used,
            weekly_used_usd: weekly_used,
            max_concurrency: plan
                .as_ref()
                .map(|plan| plan.limits.max_concurrency)
                .unwrap_or(0),
            requests_per_minute: plan
                .as_ref()
                .map(|plan| plan.limits.requests_per_minute)
                .unwrap_or(0),
            max_keys: plan.as_ref().map(|plan| plan.max_keys).unwrap_or(0),
            key_count: to_u64(key_count)?,
        })
    }

    async fn list_records(
        &self,
        user_id: &str,
        query: PortalUsageQuery,
    ) -> PortalStoreResult<PortalUsagePage> {
        let page_size = i64::from(query.page_size.clamp(1, 100));
        let (cursor_started_at, cursor_id) = match query.cursor.as_deref() {
            Some(cursor) => {
                let (started_at, id) = cursor
                    .split_once('|')
                    .ok_or_else(|| store_error(PortalStoreErrorKind::Invalid, "游标不合法"))?;
                let started_at = DateTime::parse_from_rfc3339(started_at)
                    .map_err(|_| store_error(PortalStoreErrorKind::Invalid, "游标不合法"))?
                    .with_timezone(&Utc);
                (Some(started_at), Some(id.to_owned()))
            }
            None => (None, None),
        };
        let rows = sqlx::query(
            "select mr.id, mr.started_at, mr.requested_model_id, mr.outcome, mr.input_tokens,
                    mr.output_tokens, mr.total_tokens,
                    mr.upstream_model_id, mr.provider_kind,
                    mr.provider_account_authentication_kind_snapshot,
                    mr.reasoning_effort, mr.reasoning_preset, mr.subagent_kind, mr.service_tier,
                    mr.client_transport, mr.upstream_transport,
                    mr.cached_tokens, mr.cache_write_tokens, mr.reasoning_tokens,
                    mr.image_input_tokens, mr.image_output_tokens,
                    mr.first_token_ms, mr.first_event_ms, mr.first_reasoning_ms,
                    mr.first_text_ms, mr.latency_ms, mr.cost_source,
                    case when mr.cost_currency = 'USD' then mr.cost_amount::text end as cost_usd,
                    mr.client_api_key_ref,
                    left(k.key, least(10, length(k.key) / 2)) as key_prefix, k.name as key_name
             from model_requests mr
             left join client_api_keys k on k.id = mr.client_api_key_id
             where mr.owner_user_id = $1
               and ($2::timestamptz is null or mr.started_at >= $2)
               and ($3::timestamptz is null or mr.started_at < $3)
               and ($7::text is null or mr.requested_model_id = $7)
               and (
                 $4::timestamptz is null
                 or mr.started_at < $4
                 or (mr.started_at = $4 and mr.id < $5)
               )
             order by mr.started_at desc, mr.id desc
             limit $6",
        )
        .bind(user_id)
        .bind(query.start)
        .bind(query.end)
        .bind(cursor_started_at)
        .bind(cursor_id)
        .bind(page_size + 1)
        .bind(query.model)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "list portal usage"))?;
        let mut items = rows
            .into_iter()
            .map(|row| PortalUsageRecord {
                id: row.get("id"),
                started_at: row.get("started_at"),
                model: row.get("requested_model_id"),
                upstream_model: row.get("upstream_model_id"),
                provider: row.get("provider_kind"),
                authentication_kind: row.get("provider_account_authentication_kind_snapshot"),
                reasoning_effort: row.get("reasoning_effort"),
                reasoning_preset: row.get("reasoning_preset"),
                subagent_kind: row.get("subagent_kind"),
                service_tier: row.get("service_tier"),
                client_transport: row.get("client_transport"),
                upstream_transport: row.get("upstream_transport"),
                cached_tokens: row.get("cached_tokens"),
                cache_write_tokens: row.get("cache_write_tokens"),
                reasoning_tokens: row.get("reasoning_tokens"),
                image_input_tokens: row.get("image_input_tokens"),
                image_output_tokens: row.get("image_output_tokens"),
                first_token_ms: row.get("first_token_ms"),
                first_event_ms: row.get("first_event_ms"),
                first_reasoning_ms: row.get("first_reasoning_ms"),
                first_text_ms: row.get("first_text_ms"),
                latency_ms: row.get("latency_ms"),
                outcome: row.get("outcome"),
                input_tokens: row.get("input_tokens"),
                output_tokens: row.get("output_tokens"),
                total_tokens: row.get("total_tokens"),
                cost_usd: row.get("cost_usd"),
                cost_source: row.get("cost_source"),
                billing: None,
                key_id: row.get("client_api_key_ref"),
                key_prefix: row.get("key_prefix"),
                key_name: row.get("key_name"),
            })
            .collect::<Vec<_>>();
        let next_cursor = if i64::try_from(items.len()).unwrap_or(i64::MAX) > page_size {
            items.pop();
            items
                .last()
                .map(|item| format!("{}|{}", item.started_at.to_rfc3339(), item.id))
        } else {
            None
        };
        Ok(PortalUsagePage { items, next_cursor })
    }

    async fn summary(
        &self,
        user_id: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> PortalStoreResult<PortalUsageSummary> {
        // 成功用量沿用官方交付口径；费用账本仍独立记录各次尝试的实际费用。
        let completed = super::usage_facts::completed_usage_fact_predicate("mr");
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "select count(*)::bigint,
                    coalesce(sum(mr.total_tokens), 0)::text,
                    coalesce(sum(mr.cost_amount) filter (where mr.cost_currency = 'USD'), 0)::text
             from model_requests mr
             where mr.owner_user_id = $1
               and ({completed})
               and ($2::timestamptz is null or mr.started_at >= $2)
               and ($3::timestamptz is null or mr.started_at < $3)"
        )))
        .bind(user_id)
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| map_sqlx(error, "summarize portal usage"))?;
        Ok(PortalUsageSummary {
            request_count: to_u64(row.get(0))?,
            total_tokens: row.get::<String, _>(1).parse::<i64>().unwrap_or(i64::MAX),
            total_usd: row.get(2),
        })
    }
}
