//! 用户用量查询。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use gateway_core::{
    identity::ProviderKind,
    metering::{BillingResolver, CurrencyCost, ProviderBillingInput, UsageBilling},
};

use crate::{
    model::{
        PortalError,
        usage::{PortalMe, PortalUsagePage, PortalUsageQuery, PortalUsageSummary},
    },
    ports::store::PortalUsageStore,
};

/// 用户用量服务。
#[async_trait]
pub trait PortalUsageService: Send + Sync {
    async fn me(&self, user_id: &str) -> Result<PortalMe, PortalError>;
    async fn records(
        &self,
        user_id: &str,
        query: PortalUsageQuery,
    ) -> Result<PortalUsagePage, PortalError>;
    async fn summary(
        &self,
        user_id: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<PortalUsageSummary, PortalError>;
}

pub(crate) struct DefaultPortalUsageService {
    store: Arc<dyn PortalUsageStore>,
    billing: Arc<dyn BillingResolver>,
}

impl DefaultPortalUsageService {
    #[must_use]
    pub(crate) fn new(store: Arc<dyn PortalUsageStore>, billing: Arc<dyn BillingResolver>) -> Self {
        Self { store, billing }
    }
}

#[async_trait]
impl PortalUsageService for DefaultPortalUsageService {
    async fn me(&self, user_id: &str) -> Result<PortalMe, PortalError> {
        self.store
            .load_me(user_id, Utc::now())
            .await
            .map_err(super::store_error)
    }

    async fn records(
        &self,
        user_id: &str,
        query: PortalUsageQuery,
    ) -> Result<PortalUsagePage, PortalError> {
        let mut page = self
            .store
            .list_records(user_id, query)
            .await
            .map_err(super::store_error)?;
        for record in &mut page.items {
            let Some(amount) = record
                .cost_usd
                .as_deref()
                .and_then(|value| value.parse().ok())
            else {
                continue;
            };
            let total = CurrencyCost {
                currency: "USD".to_owned(),
                amount,
            };
            record.billing = Some(UsageBilling::Total {
                source: record.cost_source.clone(),
                total: total.clone(),
            });
            if !matches!(
                record.cost_source.as_str(),
                "calculated" | "provider_reported"
            ) {
                continue;
            }
            let Some(provider) = record
                .provider
                .as_ref()
                .and_then(|value| ProviderKind::new(value.clone()).ok())
            else {
                continue;
            };
            let Some(model) = record.upstream_model.clone() else {
                continue;
            };
            let input = ProviderBillingInput {
                upstream_model_id: model,
                service_tier: record.service_tier.clone(),
                input_tokens: record.input_tokens.and_then(|value| value.try_into().ok()),
                output_tokens: record.output_tokens.and_then(|value| value.try_into().ok()),
                cached_tokens: record.cached_tokens.and_then(|value| value.try_into().ok()),
                cache_write_tokens: record
                    .cache_write_tokens
                    .and_then(|value| value.try_into().ok()),
                total,
            };
            if let Some(breakdown) = self.billing.resolve(&provider, &input) {
                record.billing = Some(UsageBilling::Calculated(Box::new(breakdown)));
            }
        }
        Ok(page)
    }

    async fn summary(
        &self,
        user_id: &str,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<PortalUsageSummary, PortalError> {
        self.store
            .summary(user_id, start, end)
            .await
            .map_err(super::store_error)
    }
}
