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
                outcome: "succeeded".to_owned(),
                input_tokens: Some(12),
                output_tokens: Some(3),
                total_tokens: Some(15),
                cost_usd: Some("0.125".to_owned()),
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
    assert_eq!(
        value["data"],
        serde_json::json!({
            "items": [{
                "id": "req_usage",
                "startedAt": "2023-11-14T22:13:20+00:00",
                "model": "gpt-5.5",
                "outcome": "succeeded",
                "inputTokens": 12,
                "outputTokens": 3,
                "totalTokens": 15,
                "costUsd": "0.125",
                "keyId": "key_usage",
                "keyPrefix": "sk_safe",
                "keyName": "主要密钥"
            }],
            "nextCursor": "next-cursor"
        })
    );
    assert!(!String::from_utf8_lossy(&body).contains("plaintext"));
}
