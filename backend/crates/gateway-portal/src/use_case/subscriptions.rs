//! 订阅开通、停用与续期。

use std::sync::Arc;

use async_trait::async_trait;
use gateway_core::runtime::SnapshotControl;

use crate::{
    model::{
        MutationContext, PortalError,
        subscriptions::{AssignSubscription, UserSubscription},
    },
    ports::store::PortalSubscriptionStore,
};

/// 订阅服务。
#[async_trait]
pub trait PortalSubscriptionService: Send + Sync {
    async fn assign(
        &self,
        context: &MutationContext,
        command: AssignSubscription,
    ) -> Result<UserSubscription, PortalError>;
    async fn disable(
        &self,
        context: &MutationContext,
        user_id: &str,
    ) -> Result<Option<UserSubscription>, PortalError>;
}

pub(crate) struct DefaultPortalSubscriptionService {
    store: Arc<dyn PortalSubscriptionStore>,
    snapshot: Arc<dyn SnapshotControl>,
}

impl DefaultPortalSubscriptionService {
    #[must_use]
    pub(crate) fn new(
        store: Arc<dyn PortalSubscriptionStore>,
        snapshot: Arc<dyn SnapshotControl>,
    ) -> Self {
        Self { store, snapshot }
    }
}

#[async_trait]
impl PortalSubscriptionService for DefaultPortalSubscriptionService {
    async fn assign(
        &self,
        context: &MutationContext,
        command: AssignSubscription,
    ) -> Result<UserSubscription, PortalError> {
        if command
            .ends_at
            .is_some_and(|ends_at| ends_at <= command.starts_at)
        {
            return Err(PortalError::invalid("订阅结束时间必须晚于开始时间"));
        }
        let (revision, subscription) = self
            .store
            .assign(command, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(subscription)
    }

    async fn disable(
        &self,
        context: &MutationContext,
        user_id: &str,
    ) -> Result<Option<UserSubscription>, PortalError> {
        let (revision, subscription) = self
            .store
            .disable(user_id, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(subscription)
    }
}
