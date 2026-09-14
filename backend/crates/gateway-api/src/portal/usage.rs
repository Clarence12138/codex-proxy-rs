//! 用户用量 HTTP。

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use chrono::{DateTime, Utc};
use gateway_portal::model::usage::PortalUsageQuery;
use serde::{Deserialize, Serialize};

use super::{
    PortalAuth, PortalSessionState,
    wire::{PortalEnvelope, PortalError, PortalResponse, map_portal_error},
};

pub fn router<S>() -> Router<S>
where
    S: PortalSessionState + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/portal/usage/records", get(records::<S>))
        .route("/api/portal/usage/summary", get(summary::<S>))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UsageQuery {
    start: Option<String>,
    end: Option<String>,
    cursor: Option<String>,
    page_size: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordView {
    id: String,
    started_at: String,
    model: Option<String>,
    upstream_model: Option<String>,
    provider: Option<String>,
    authentication_kind: Option<String>,
    reasoning_effort: Option<String>,
    reasoning_preset: Option<String>,
    subagent_kind: Option<String>,
    service_tier: Option<String>,
    client_transport: String,
    upstream_transport: Option<String>,
    cached_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
    reasoning_tokens: Option<i64>,
    image_input_tokens: Option<i64>,
    image_output_tokens: Option<i64>,
    first_token_latency_ms: Option<i64>,
    first_event_ms: Option<i64>,
    first_reasoning_ms: Option<i64>,
    first_text_ms: Option<i64>,
    latency_ms: Option<i64>,
    outcome: String,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    token_details: crate::usage_presentation::TokenDetailsView,
    cost_usd: Option<String>,
    billing: Option<crate::usage_presentation::BillingView>,
    key_id: String,
    key_prefix: Option<String>,
    key_name: Option<String>,
}

fn parse_time(value: Option<String>) -> Result<Option<DateTime<Utc>>, PortalError> {
    value
        .map(|value| {
            DateTime::parse_from_rfc3339(&value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|_| PortalError::invalid_request(StatusCode::BAD_REQUEST, "时间不合法"))
        })
        .transpose()
}

async fn records<S>(
    auth: PortalAuth,
    State(state): State<S>,
    Query(query): Query<UsageQuery>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let page = state
        .portal_services()
        .usage()
        .records(
            &auth.principal.user_id,
            PortalUsageQuery {
                start: parse_time(query.start)?,
                end: parse_time(query.end)?,
                cursor: query.cursor,
                page_size: query.page_size.unwrap_or(50),
            },
        )
        .await
        .map_err(map_portal_error)?;
    let items = page
        .items
        .into_iter()
        .map(|record| RecordView {
            id: record.id,
            started_at: record.started_at.to_rfc3339(),
            model: record.model,
            upstream_model: record.upstream_model,
            provider: record.provider,
            authentication_kind: record.authentication_kind,
            reasoning_effort: record.reasoning_effort,
            reasoning_preset: record.reasoning_preset,
            subagent_kind: record.subagent_kind,
            service_tier: record.service_tier,
            client_transport: record.client_transport,
            upstream_transport: record.upstream_transport,
            cached_tokens: record.cached_tokens,
            cache_write_tokens: record.cache_write_tokens,
            reasoning_tokens: record.reasoning_tokens,
            image_input_tokens: record.image_input_tokens,
            image_output_tokens: record.image_output_tokens,
            first_token_latency_ms: record.first_token_ms,
            first_event_ms: record.first_event_ms,
            first_reasoning_ms: record.first_reasoning_ms,
            first_text_ms: record.first_text_ms,
            latency_ms: record.latency_ms,
            outcome: record.outcome,
            input_tokens: record.input_tokens,
            output_tokens: record.output_tokens,
            total_tokens: record.total_tokens,
            token_details: crate::usage_presentation::token_details(
                &gateway_core::metering::Usage {
                    input_tokens: record.input_tokens.and_then(|v| v.try_into().ok()),
                    output_tokens: record.output_tokens.and_then(|v| v.try_into().ok()),
                    cached_tokens: record.cached_tokens.and_then(|v| v.try_into().ok()),
                    cache_write_tokens: record.cache_write_tokens.and_then(|v| v.try_into().ok()),
                    reasoning_tokens: record.reasoning_tokens.and_then(|v| v.try_into().ok()),
                    image_input_tokens: record.image_input_tokens.and_then(|v| v.try_into().ok()),
                    image_output_tokens: record.image_output_tokens.and_then(|v| v.try_into().ok()),
                    total_tokens: record.total_tokens.and_then(|v| v.try_into().ok()),
                },
            ),
            cost_usd: record.cost_usd,
            billing: crate::usage_presentation::billing_view(record.billing.as_ref()),
            key_id: record.key_id,
            key_prefix: record.key_prefix,
            key_name: record.key_name,
        })
        .collect::<Vec<_>>();
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({
            "items": items,
            "nextCursor": page.next_cursor,
        })),
    ))
}

async fn summary<S>(
    auth: PortalAuth,
    State(state): State<S>,
    Query(query): Query<UsageQuery>,
) -> Result<impl axum::response::IntoResponse, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let summary = state
        .portal_services()
        .usage()
        .summary(
            &auth.principal.user_id,
            parse_time(query.start)?,
            parse_time(query.end)?,
        )
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(serde_json::json!({
            "requestCount": summary.request_count,
            "totalTokens": summary.total_tokens,
            "totalUsd": summary.total_usd,
        })),
    ))
}
