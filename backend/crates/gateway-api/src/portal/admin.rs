//! 管理员用户/套餐/订阅 HTTP。走现有 Admin 鉴权。

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use gateway_core::{engine::budget::ClientBudgetLimits, metering::Decimal, policy::RateLimits};
use gateway_portal::model::{
    MutationActor, MutationContext,
    plans::{CreatePlan, PlanListQuery, UpdatePlan},
    subscriptions::AssignSubscription,
    users::{CreatePortalUser, PortalUserListQuery, ResetPortalPassword, SetPortalUserEnabled},
};
use serde::Deserialize;

use crate::admin::{AdminAuth, AdminJson, AdminQuery};
use gateway_admin::model::auth::AdminPrincipal;

use super::wire::{PortalEnvelope, PortalError, PortalResponse, map_portal_error};

pub fn router<S>() -> Router<S>
where
    S: crate::portal::PortalSessionState
        + crate::auth::SessionState
        + Clone
        + Send
        + Sync
        + 'static,
{
    Router::new()
        .route("/api/admin/portal/users", get(list_users::<S>))
        .route("/api/admin/portal/users/create", post(create_user::<S>))
        .route(
            "/api/admin/portal/users/reset-password",
            post(reset_password::<S>),
        )
        .route("/api/admin/portal/users/enable", post(enable_user::<S>))
        .route("/api/admin/portal/users/disable", post(disable_user::<S>))
        .route("/api/admin/portal/plans", get(list_plans::<S>))
        .route("/api/admin/portal/plans/create", post(create_plan::<S>))
        .route("/api/admin/portal/plans/update", post(update_plan::<S>))
        .route(
            "/api/admin/portal/subscriptions/assign",
            post(assign_subscription::<S>),
        )
        .route(
            "/api/admin/portal/subscriptions/disable",
            post(disable_subscription::<S>),
        )
}

fn admin_context(auth: &AdminAuth) -> MutationContext {
    let context = auth.context();
    MutationContext {
        actor: match &context.principal {
            AdminPrincipal::Session { admin_user_id } => MutationActor::AdminSession {
                admin_user_id: admin_user_id.clone(),
            },
            AdminPrincipal::ApiKey => MutationActor::AdminApiKey,
        },
        request_id: context.request_id.clone(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UserListQuery {
    search: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateUserBody {
    username: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResetPasswordBody {
    user_id: String,
    password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UserIdBody {
    user_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanBody {
    id: Option<String>,
    name: String,
    daily_limit_usd: Option<String>,
    weekly_limit_usd: Option<String>,
    max_concurrency: Option<u64>,
    requests_per_minute: Option<u64>,
    max_keys: Option<u64>,
    group_ids: Vec<String>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssignBody {
    user_id: String,
    plan_id: String,
    starts_at: String,
    ends_at: Option<String>,
}

async fn list_users<S>(
    _auth: AdminAuth,
    State(state): State<S>,
    AdminQuery(query): AdminQuery<UserListQuery>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    let page = state
        .portal_services()
        .users()
        .list(PortalUserListQuery {
            search: query.search,
            page: query.page.unwrap_or(1),
            page_size: query.page_size.unwrap_or(50),
        })
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({
            "items": page.items.iter().map(|user| serde_json::json!({
                "id": user.id,
                "username": user.username,
                "status": user.status.as_str(),
                "planId": user.plan_id,
                "planName": user.plan_name,
                "subscriptionStartsAt": user.subscription_starts_at.map(|value| value.to_rfc3339()),
                "subscriptionEndsAt": user.subscription_ends_at.map(|value| value.to_rfc3339()),
                "createdAt": user.created_at.to_rfc3339(),
            })).collect::<Vec<_>>(),
            "total": page.total,
        })),
    ))
}

async fn create_user<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<CreateUserBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    let user = state
        .portal_services()
        .users()
        .create(
            &admin_context(&auth),
            CreatePortalUser {
                username: body.username,
                password: body.password,
            },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({ "id": user.id })),
    ))
}

async fn reset_password<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<ResetPasswordBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    state
        .portal_services()
        .users()
        .reset_password(
            &admin_context(&auth),
            ResetPortalPassword {
                user_id: body.user_id,
                password: body.password,
            },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({})),
    ))
}

async fn enable_user<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<UserIdBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    set_user(auth, state, body.user_id, true).await
}

async fn disable_user<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<UserIdBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    set_user(auth, state, body.user_id, false).await
}

async fn set_user<S>(
    auth: AdminAuth,
    state: S,
    user_id: String,
    enabled: bool,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    state
        .portal_services()
        .users()
        .set_enabled(
            &admin_context(&auth),
            SetPortalUserEnabled { user_id, enabled },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({})),
    ))
}

async fn list_plans<S>(
    _auth: AdminAuth,
    State(state): State<S>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    let page = state
        .portal_services()
        .plans()
        .list(PlanListQuery {
            page: 1,
            page_size: 200,
        })
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({
            "items": page.items.iter().map(|plan| serde_json::json!({
                "id": plan.id,
                "name": plan.name,
                "dailyLimitUsd": plan.budget.daily_usd.canonical(),
                "weeklyLimitUsd": plan.budget.weekly_usd.canonical(),
                "maxConcurrency": plan.limits.max_concurrency,
                "requestsPerMinute": plan.limits.requests_per_minute,
                "maxKeys": plan.max_keys,
                "groupIds": plan.group_ids,
                "enabled": plan.enabled,
            })).collect::<Vec<_>>(),
            "total": page.total,
        })),
    ))
}

fn parse_plan_budget(body: &PlanBody) -> Result<ClientBudgetLimits, PortalError> {
    Ok(ClientBudgetLimits {
        daily_usd: body
            .daily_limit_usd
            .clone()
            .unwrap_or_else(|| "0".to_owned())
            .parse::<Decimal>()
            .map_err(|_| {
                PortalError::invalid_request(StatusCode::UNPROCESSABLE_ENTITY, "日限额不合法")
            })?,
        weekly_usd: body
            .weekly_limit_usd
            .clone()
            .unwrap_or_else(|| "0".to_owned())
            .parse::<Decimal>()
            .map_err(|_| {
                PortalError::invalid_request(StatusCode::UNPROCESSABLE_ENTITY, "周限额不合法")
            })?,
    })
}

async fn create_plan<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<PlanBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    let plan = state
        .portal_services()
        .plans()
        .create(
            &admin_context(&auth),
            CreatePlan {
                budget: parse_plan_budget(&body)?,
                name: body.name,
                limits: RateLimits {
                    max_concurrency: body.max_concurrency.unwrap_or(0),
                    requests_per_minute: body.requests_per_minute.unwrap_or(0),
                },
                max_keys: body.max_keys.unwrap_or(0),
                group_ids: body.group_ids,
            },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({ "id": plan.id })),
    ))
}

async fn update_plan<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<PlanBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    let id = body.id.clone().ok_or_else(|| {
        PortalError::invalid_request(StatusCode::UNPROCESSABLE_ENTITY, "缺少套餐 ID")
    })?;
    state
        .portal_services()
        .plans()
        .update(
            &admin_context(&auth),
            UpdatePlan {
                id,
                budget: parse_plan_budget(&body)?,
                name: body.name,
                limits: RateLimits {
                    max_concurrency: body.max_concurrency.unwrap_or(0),
                    requests_per_minute: body.requests_per_minute.unwrap_or(0),
                },
                max_keys: body.max_keys.unwrap_or(0),
                group_ids: body.group_ids,
                enabled: body.enabled.unwrap_or(true),
            },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({})),
    ))
}

async fn assign_subscription<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<AssignBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    let starts_at = DateTime::parse_from_rfc3339(&body.starts_at)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| PortalError::invalid_request(StatusCode::BAD_REQUEST, "开始时间不合法"))?;
    let ends_at = body
        .ends_at
        .as_deref()
        .map(|value| {
            DateTime::parse_from_rfc3339(value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|_| {
                    PortalError::invalid_request(StatusCode::BAD_REQUEST, "结束时间不合法")
                })
        })
        .transpose()?;
    let subscription = state
        .portal_services()
        .subscriptions()
        .assign(
            &admin_context(&auth),
            AssignSubscription {
                user_id: body.user_id,
                plan_id: body.plan_id,
                starts_at,
                ends_at,
            },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({ "id": subscription.id })),
    ))
}

async fn disable_subscription<S>(
    auth: AdminAuth,
    State(state): State<S>,
    AdminJson(body): AdminJson<UserIdBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: crate::portal::PortalSessionState + Send + Sync,
{
    state
        .portal_services()
        .subscriptions()
        .disable(&admin_context(&auth), &body.user_id)
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({})),
    ))
}
