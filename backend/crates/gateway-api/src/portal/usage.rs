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
    outcome: String,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    cost_usd: Option<String>,
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
            outcome: record.outcome,
            input_tokens: record.input_tokens,
            output_tokens: record.output_tokens,
            total_tokens: record.total_tokens,
            cost_usd: record.cost_usd,
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
