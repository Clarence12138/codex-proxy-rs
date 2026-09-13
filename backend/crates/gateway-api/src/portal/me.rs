//! Portal 当前用户。

use axum::{Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

use super::{
    PortalAuth, PortalSessionState,
    wire::{PortalEnvelope, PortalError, PortalResponse, map_portal_error},
};

pub fn router<S>() -> Router<S>
where
    S: PortalSessionState + Clone + Send + Sync + 'static,
{
    Router::new().route("/api/portal/me", get(me::<S>))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeView {
    user_id: String,
    username: String,
    plan_name: Option<String>,
    subscription_ends_at: Option<String>,
    subscription_effective: bool,
    daily_limit_usd: String,
    weekly_limit_usd: String,
    daily_used_usd: String,
    weekly_used_usd: String,
    max_concurrency: u64,
    requests_per_minute: u64,
    max_keys: u64,
    key_count: u64,
}

async fn me<S>(
    auth: PortalAuth,
    State(state): State<S>,
) -> Result<PortalResponse<MeView>, PortalError>
where
    S: PortalSessionState + Send + Sync,
{
    let me = state
        .portal_services()
        .usage()
        .me(&auth.principal.user_id)
        .await
        .map_err(map_portal_error)?;
    Ok(PortalResponse::new(
        StatusCode::OK,
        PortalEnvelope::ok(MeView {
            user_id: me.user_id,
            username: me.username,
            plan_name: me.plan_name,
            subscription_ends_at: me.subscription_ends_at.map(|value| value.to_rfc3339()),
            subscription_effective: me.subscription_effective,
            daily_limit_usd: me.daily_limit_usd,
            weekly_limit_usd: me.weekly_limit_usd,
            daily_used_usd: me.daily_used_usd,
            weekly_used_usd: me.weekly_used_usd,
            max_concurrency: me.max_concurrency,
            requests_per_minute: me.requests_per_minute,
            max_keys: me.max_keys,
            key_count: me.key_count,
        }),
    ))
}
