//! 用户 Key HTTP。

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use gateway_core::{engine::budget::ClientBudgetLimits, metering::Decimal, policy::RateLimits};
use gateway_portal::model::{
    MutationActor, MutationContext,
    keys::{CreatePortalKey, UpdatePortalKey},
};
use serde::{Deserialize, Serialize};

use super::{
    PortalAuth, PortalSessionState,
    extract::PortalJson,
    wire::{PortalEnvelope, PortalError, PortalResponse, map_portal_error},
};

pub fn router<S>() -> Router<S>
where
    S: PortalSessionState + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/portal/keys", get(list::<S>).post(create::<S>))
        .route("/api/portal/keys/update", post(update::<S>))
        .route("/api/portal/keys/enable", post(enable::<S>))
        .route("/api/portal/keys/disable", post(disable::<S>))
        .route("/api/portal/keys/delete", post(delete::<S>))
        .route("/api/portal/keys/reveal", get(reveal::<S>))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyBody {
    name: String,
    label: Option<String>,
    max_concurrency: Option<u64>,
    requests_per_minute: Option<u64>,
    daily_limit_usd: Option<String>,
    weekly_limit_usd: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyIdBody {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyIdQuery {
    id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct KeyView {
    id: String,
    name: String,
    label: Option<String>,
    prefix: String,
    enabled: bool,
    max_concurrency: u64,
    requests_per_minute: u64,
    daily_limit_usd: String,
    weekly_limit_usd: String,
    daily_used_usd: String,
    weekly_used_usd: String,
    last_used_at: Option<String>,
    created_at: String,
    plaintext: Option<String>,
}

fn context(user_id: &str, request_id: &str) -> MutationContext {
    MutationContext {
        actor: MutationActor::PortalSession {
            user_id: user_id.to_owned(),
        },
        request_id: request_id.to_owned(),
    }
}

fn parse_budget(
    daily: Option<String>,
    weekly: Option<String>,
) -> Result<ClientBudgetLimits, PortalError> {
    Ok(ClientBudgetLimits {
        daily_usd: daily
            .unwrap_or_else(|| "0".to_owned())
            .parse::<Decimal>()
            .map_err(|_| {
                PortalError::invalid_request(StatusCode::UNPROCESSABLE_ENTITY, "日限额不合法")
            })?,
        weekly_usd: weekly
            .unwrap_or_else(|| "0".to_owned())
            .parse::<Decimal>()
            .map_err(|_| {
                PortalError::invalid_request(StatusCode::UNPROCESSABLE_ENTITY, "周限额不合法")
            })?,
    })
}

async fn list<S>(
    auth: PortalAuth,
    State(state): State<S>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let items = state
        .portal_services()
        .keys()
        .list(&auth.principal.user_id)
        .await
        .map_err(map_portal_error)?
        .into_iter()
        .map(|record| KeyView {
            id: record.id,
            name: record.name,
            label: record.label,
            prefix: record.prefix,
            enabled: record.enabled,
            max_concurrency: record.limits.max_concurrency,
            requests_per_minute: record.limits.requests_per_minute,
            daily_limit_usd: record.budget.daily_usd.canonical(),
            weekly_limit_usd: record.budget.weekly_usd.canonical(),
            daily_used_usd: record.daily_used_usd,
            weekly_used_usd: record.weekly_used_usd,
            last_used_at: record.last_used_at.map(|value| value.to_rfc3339()),
            created_at: record.created_at.to_rfc3339(),
            plaintext: None,
        })
        .collect::<Vec<_>>();
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(items),
    ))
}

async fn create<S>(
    auth: PortalAuth,
    State(state): State<S>,
    PortalJson(body): PortalJson<KeyBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let created = state
        .portal_services()
        .keys()
        .create(
            &context(&auth.principal.user_id, &auth.request_id),
            &auth.principal.user_id,
            CreatePortalKey {
                name: body.name,
                label: body.label,
                limits: RateLimits {
                    max_concurrency: body.max_concurrency.unwrap_or(0),
                    requests_per_minute: body.requests_per_minute.unwrap_or(0),
                },
                budget: parse_budget(body.daily_limit_usd, body.weekly_limit_usd)?,
            },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(KeyView {
            id: created.record.id,
            name: created.record.name,
            label: created.record.label,
            prefix: created.record.prefix,
            enabled: created.record.enabled,
            max_concurrency: created.record.limits.max_concurrency,
            requests_per_minute: created.record.limits.requests_per_minute,
            daily_limit_usd: created.record.budget.daily_usd.canonical(),
            weekly_limit_usd: created.record.budget.weekly_usd.canonical(),
            daily_used_usd: created.record.daily_used_usd,
            weekly_used_usd: created.record.weekly_used_usd,
            last_used_at: None,
            created_at: created.record.created_at.to_rfc3339(),
            plaintext: Some(created.plaintext),
        }),
    ))
}

async fn update<S>(
    auth: PortalAuth,
    State(state): State<S>,
    PortalJson(body): PortalJson<KeyUpdateBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let record = state
        .portal_services()
        .keys()
        .update(
            &context(&auth.principal.user_id, &auth.request_id),
            &auth.principal.user_id,
            UpdatePortalKey {
                id: body.id,
                name: body.name,
                label: body.label,
                limits: RateLimits {
                    max_concurrency: body.max_concurrency.unwrap_or(0),
                    requests_per_minute: body.requests_per_minute.unwrap_or(0),
                },
                budget: parse_budget(body.daily_limit_usd, body.weekly_limit_usd)?,
            },
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({ "id": record.id })),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KeyUpdateBody {
    id: String,
    name: String,
    label: Option<String>,
    max_concurrency: Option<u64>,
    requests_per_minute: Option<u64>,
    daily_limit_usd: Option<String>,
    weekly_limit_usd: Option<String>,
}

async fn enable<S>(
    auth: PortalAuth,
    State(state): State<S>,
    PortalJson(body): PortalJson<KeyIdBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    set_enabled(auth, state, body.id, true).await
}

async fn disable<S>(
    auth: PortalAuth,
    State(state): State<S>,
    PortalJson(body): PortalJson<KeyIdBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    set_enabled(auth, state, body.id, false).await
}

async fn set_enabled<S>(
    auth: PortalAuth,
    state: S,
    id: String,
    enabled: bool,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    state
        .portal_services()
        .keys()
        .set_enabled(
            &context(&auth.principal.user_id, &auth.request_id),
            &auth.principal.user_id,
            &id,
            enabled,
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({ "id": id })),
    ))
}

async fn delete<S>(
    auth: PortalAuth,
    State(state): State<S>,
    PortalJson(body): PortalJson<KeyIdBody>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    state
        .portal_services()
        .keys()
        .delete(
            &context(&auth.principal.user_id, &auth.request_id),
            &auth.principal.user_id,
            &body.id,
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({ "id": body.id })),
    ))
}

async fn reveal<S>(
    auth: PortalAuth,
    State(state): State<S>,
    Query(query): Query<KeyIdQuery>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let revealed = state
        .portal_services()
        .keys()
        .reveal(&auth.principal.user_id, &query.id)
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({ "plaintext": revealed.plaintext })),
    ))
}
