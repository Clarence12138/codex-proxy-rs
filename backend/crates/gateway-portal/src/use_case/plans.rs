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
        validate_plan_name(&command.name)?;
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
        validate_plan_name(&command.name)?;
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

fn validate_plan_name(name: &str) -> Result<(), PortalError> {
    // 与存储的 trim 和 UTF-8 字节上限一致，在写库前返回可操作的输入错误。
    let name = name.trim();
    if name.is_empty() {
        return Err(PortalError::invalid("请输入套餐名称"));
    }
    if name.len() > 64 {
        return Err(PortalError::invalid("套餐名称不能超过 64 个 UTF-8 字节"));
    }
    if name.chars().any(char::is_control) {
        return Err(PortalError::invalid("套餐名称不能包含控制字符"));
    }
    Ok(())
}

fn validate_plan_groups(group_ids: &[String]) -> Result<(), PortalError> {
    if group_ids.is_empty() {
        return Err(PortalError::invalid("套餐必须绑定至少一个账号分组"));
    }
    Ok(())
}
