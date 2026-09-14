use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use chrono::{DateTime, TimeZone as _, Utc};
use gateway_portal::{
    PortalServices,
    model::{
        MutationContext,
        auth::{PortalPrincipal, PortalSession},
        usage::{
            PortalMe, PortalUsagePage, PortalUsageQuery, PortalUsageRecord, PortalUsageSummary,
        },
    },
    ports::store::{PortalAuthStore, PortalStoreResult, PortalUsageStore},
};
use tower::ServiceExt as _;

#[derive(Clone)]
struct UsageState(PortalServices);

impl gateway_api::portal::PortalSessionState for UsageState {
    fn trusted_proxy_ips(&self) -> &[std::net::IpAddr] {
        &[]
    }

    fn portal_services(&self) -> &PortalServices {
        &self.0
    }
}

impl gateway_api::admin::AdminSessionState for UsageState {
    fn admin_services(&self) -> &gateway_admin::AdminServices {
        unreachable!("usage tests never access admin services")
    }
}

struct UsageStore;

#[async_trait]
impl PortalAuthStore for UsageStore {
    async fn change_password(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &MutationContext,
    ) -> PortalStoreResult<()> {
        unreachable!()
    }

    async fn load_password_hash(
        &self,
        _: &str,
    ) -> PortalStoreResult<Option<(String, String, String)>> {
        unreachable!()
    }

    async fn store_session(&self, _: &PortalSession, _: &str) -> PortalStoreResult<()> {
        unreachable!()
    }

    async fn load_session_by_token_hash(
        &self,
        _: &str,
        _: DateTime<Utc>,
    ) -> PortalStoreResult<Option<PortalPrincipal>> {
        Ok(Some(PortalPrincipal {
            user_id: "usr_usage".to_owned(),
            username: "usage-user".to_owned(),
        }))
    }

    async fn delete_session_by_token_hash(&self, _: &str) -> PortalStoreResult<()> {
        unreachable!()
    }

    async fn delete_sessions_for_user(&self, _: &str) -> PortalStoreResult<()> {
        unreachable!()
    }
}

#[async_trait]
impl PortalUsageStore for UsageStore {
    async fn load_me(&self, _: &str, _: DateTime<Utc>) -> PortalStoreResult<PortalMe> {
        unreachable!()
    }

    async fn list_records(
        &self,
        user_id: &str,
        _: PortalUsageQuery,
    ) -> PortalStoreResult<PortalUsagePage> {
        assert_eq!(user_id, "usr_usage");
        Ok(PortalUsagePage {
            items: vec![PortalUsageRecord {
                id: "req_usage".to_owned(),
                started_at: Utc.timestamp_opt(1_700_000_000, 0).single().unwrap(),
                model: Some("gpt-5.5".to_owned()),
                upstream_model: Some("gpt-5.5".to_owned()),
                provider: Some("openai".to_owned()),
                authentication_kind: Some("oauth".to_owned()),
                reasoning_effort: Some("high".to_owned()),
                reasoning_preset: None,
                subagent_kind: None,
                service_tier: None,
                client_transport: "http_sse".to_owned(),
                upstream_transport: Some("websocket".to_owned()),
                cached_tokens: Some(4),
                cache_write_tokens: None,
                reasoning_tokens: Some(2),
                image_input_tokens: None,
                image_output_tokens: None,
                first_token_ms: Some(120),
                first_event_ms: Some(80),
                first_reasoning_ms: Some(120),
                first_text_ms: Some(150),
                latency_ms: Some(200),
                outcome: "succeeded".to_owned(),
                input_tokens: Some(12),
                output_tokens: Some(3),
                total_tokens: Some(15),
                cost_usd: Some("0.125".to_owned()),
                cost_source: "provider_reported".to_owned(),
                billing: None,
                key_id: "key_usage".to_owned(),
                key_prefix: Some("sk_safe".to_owned()),
                key_name: Some("主要密钥".to_owned()),
            }],
            next_cursor: Some("next-cursor".to_owned()),
        })
    }

    async fn summary(
        &self,
        _: &str,
        _: Option<DateTime<Utc>>,
        _: Option<DateTime<Utc>>,
    ) -> PortalStoreResult<PortalUsageSummary> {
        unreachable!()
    }
}

#[tokio::test]
async fn usage_records_expose_only_safe_key_metadata() {
    let store = Arc::new(UsageStore);
    let state = UsageState(crate::support::services_with_auth_and_usage(
        store.clone(),
        store,
    ));
    let response = gateway_api::portal::router()
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/api/portal/usage/records")
                .header(header::COOKIE, "cpr_portal_session=test-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tokens = serde_json::json!({
        "inputTokens": 12, "outputTokens": 3, "cachedTokens": 4,
        "cacheWriteTokens": null, "reasoningTokens": 2,
        "imageInputTokens": null, "imageOutputTokens": null, "totalTokens": 15,
        "inputTokensDisplay": "12", "outputTokensDisplay": "3", "cachedTokensDisplay": "4",
        "cacheWriteTokensDisplay": "-", "reasoningTokensDisplay": "2",
        "imageInputTokensDisplay": "-", "imageOutputTokensDisplay": "-", "totalTokensDisplay": "15"
    });
    let billing = serde_json::json!({
        "inputAmountDisplay": "—", "outputAmountDisplay": "—",
        "cacheReadAmountDisplay": "—", "cacheWriteAmountDisplay": "—",
        "standardAmountDisplay": "—", "totalAmountDisplay": "$0.125",
        "inputPriceDisplay": "—", "outputPriceDisplay": "—",
        "cacheReadPriceDisplay": "—", "cacheWritePriceDisplay": "—",
        "serviceTierDisplay": "—", "multiplierDisplay": "—"
    });
    assert_eq!(
        value["data"],
        serde_json::json!({
            "items": [{
                "id": "req_usage",
                "startedAt": "2023-11-14T22:13:20+00:00",
                "model": "gpt-5.5",
                "upstreamModel": "gpt-5.5",
                "provider": "openai",
                "authenticationKind": "oauth",
                "reasoningEffort": "high",
                "reasoningPreset": null,
                "subagentKind": null,
                "serviceTier": null,
                "clientTransport": "http_sse",
                "upstreamTransport": "websocket",
                "cachedTokens": 4,
                "cacheWriteTokens": null,
                "reasoningTokens": 2,
                "imageInputTokens": null,
                "imageOutputTokens": null,
                "firstTokenLatencyMs": 120,
                "firstEventMs": 80,
                "firstReasoningMs": 120,
                "firstTextMs": 150,
                "latencyMs": 200,
                "outcome": "succeeded",
                "inputTokens": 12,
                "outputTokens": 3,
                "totalTokens": 15,
                "costUsd": "0.125",
                "billing": billing,
                "tokenDetails": tokens,
                "keyId": "key_usage",
                "keyPrefix": "sk_safe",
                "keyName": "主要密钥"
            }],
            "nextCursor": "next-cursor"
        })
    );
    assert!(!String::from_utf8_lossy(&body).contains("plaintext"));
}

struct BillingSpy;

impl gateway_core::metering::BillingResolver for BillingSpy {
    fn resolve(
        &self,
        provider: &gateway_core::identity::ProviderKind,
        input: &gateway_core::metering::ProviderBillingInput,
    ) -> Option<gateway_core::metering::CalculatedBillingBreakdown> {
        use gateway_core::metering::{CalculatedBillingBreakdown, CurrencyCost};
        assert_eq!(provider.as_str(), "openai");
        assert_eq!(input.upstream_model_id, "gpt-5.5");
        assert_eq!(input.input_tokens, Some(12));
        assert_eq!(input.output_tokens, Some(3));
        assert_eq!(input.cached_tokens, Some(4));
        assert_eq!(input.total.amount.as_str(), "0.125");
        let money = |value: &str| CurrencyCost {
            currency: "USD".to_owned(),
            amount: value.parse().unwrap(),
        };
        Some(CalculatedBillingBreakdown {
            input_amount: money("0.08"),
            output_amount: money("0.02"),
            cache_read_amount: money("0.025"),
            cache_write_amount: money("0"),
            standard_amount: money("0.05"),
            total_amount: input.total.clone(),
            input_price_per_million: money("3"),
            output_price_per_million: money("10"),
            cache_read_price_per_million: money("0.5"),
            cache_write_price_per_million: money("0"),
            service_tier: Some("priority".to_owned()),
            multiplier_percent: 250,
        })
    }
}

#[tokio::test]
async fn portal_usage_passes_safe_facts_to_billing_and_presents_verified_breakdown() {
    let store = Arc::new(UsageStore);
    let state = UsageState(crate::support::services_with_auth_usage_and_billing(
        store.clone(),
        store,
        Arc::new(BillingSpy),
    ));
    let response = gateway_api::portal::router()
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/api/portal/usage/records")
                .header(header::COOKIE, "cpr_portal_session=test-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let record = &value["data"]["items"][0];
    assert_eq!(record["billing"]["totalAmountDisplay"], "$0.125");
    assert_eq!(record["billing"]["inputAmountDisplay"], "$0.08");
    assert_eq!(record["billing"]["serviceTierDisplay"], "Fast");
    assert_eq!(record["billing"]["multiplierDisplay"], "2.50x");
    for forbidden in [
        "accountId",
        "accountEmail",
        "accountName",
        "metadata",
        "costSource",
        "capacityUsedSlots",
        "requestBody",
        "responseBody",
    ] {
        assert!(record.get(forbidden).is_none(), "{forbidden}");
    }
}
