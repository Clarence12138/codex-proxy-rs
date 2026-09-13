//! 用户用量查询。

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

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
}

impl DefaultPortalUsageService {
    #[must_use]
    pub(crate) fn new(store: Arc<dyn PortalUsageStore>) -> Self {
        Self { store }
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
        self.store
            .list_records(user_id, query)
            .await
            .map_err(super::store_error)
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
