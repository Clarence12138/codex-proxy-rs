//! 套餐管理。

use std::sync::Arc;

use async_trait::async_trait;
use gateway_core::runtime::SnapshotControl;

use crate::{
    model::{
        MutationContext, PortalError,
        plans::{CreatePlan, PlanListQuery, PlanPage, SubscriptionPlan, UpdatePlan},
    },
    ports::store::PortalPlanStore,
};

/// 套餐服务。
#[async_trait]
pub trait PortalPlanService: Send + Sync {
    async fn create(
        &self,
        context: &MutationContext,
        command: CreatePlan,
    ) -> Result<SubscriptionPlan, PortalError>;
    async fn update(
        &self,
        context: &MutationContext,
        command: UpdatePlan,
    ) -> Result<SubscriptionPlan, PortalError>;
    async fn list(&self, query: PlanListQuery) -> Result<PlanPage, PortalError>;
}

pub(crate) struct DefaultPortalPlanService {
    store: Arc<dyn PortalPlanStore>,
    snapshot: Arc<dyn SnapshotControl>,
}

impl DefaultPortalPlanService {
    #[must_use]
    pub(crate) fn new(store: Arc<dyn PortalPlanStore>, snapshot: Arc<dyn SnapshotControl>) -> Self {
        Self { store, snapshot }
    }
}

#[async_trait]
impl PortalPlanService for DefaultPortalPlanService {
    async fn create(
        &self,
        context: &MutationContext,
        command: CreatePlan,
    ) -> Result<SubscriptionPlan, PortalError> {
        validate_plan_groups(&command.group_ids)?;
        let (revision, plan) = self
            .store
            .create_plan(command, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(plan)
    }

    async fn update(
        &self,
        context: &MutationContext,
        command: UpdatePlan,
    ) -> Result<SubscriptionPlan, PortalError> {
        validate_plan_groups(&command.group_ids)?;
        let (revision, plan) = self
            .store
            .update_plan(command, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(plan)
    }

    async fn list(&self, query: PlanListQuery) -> Result<PlanPage, PortalError> {
        self.store
            .list_plans(query)
            .await
            .map_err(super::store_error)
    }
}

fn validate_plan_groups(group_ids: &[String]) -> Result<(), PortalError> {
    if group_ids.is_empty() {
        return Err(PortalError::invalid("套餐必须绑定至少一个账号分组"));
    }
    Ok(())
}
