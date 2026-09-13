//! 管理员用户管理。

use std::sync::Arc;

use async_trait::async_trait;
use gateway_core::runtime::SnapshotControl;

use crate::{
    model::{
        MutationContext, PortalError,
        users::{
            CreatePortalUser, PortalUser, PortalUserListQuery, PortalUserPage, ResetPortalPassword,
            SetPortalUserEnabled,
        },
    },
    ports::store::PortalUserStore,
};

use super::auth::{hash_password_blocking, validate_password};

/// 用户管理服务。
#[async_trait]
pub trait PortalUserService: Send + Sync {
    async fn create(
        &self,
        context: &MutationContext,
        command: CreatePortalUser,
    ) -> Result<PortalUser, PortalError>;
    async fn list(&self, query: PortalUserListQuery) -> Result<PortalUserPage, PortalError>;
    async fn reset_password(
        &self,
        context: &MutationContext,
        command: ResetPortalPassword,
    ) -> Result<(), PortalError>;
    async fn set_enabled(
        &self,
        context: &MutationContext,
        command: SetPortalUserEnabled,
    ) -> Result<PortalUser, PortalError>;
}

pub(crate) struct DefaultPortalUserService {
    users: Arc<dyn PortalUserStore>,
    snapshot: Arc<dyn SnapshotControl>,
}

impl DefaultPortalUserService {
    #[must_use]
    pub(crate) fn new(users: Arc<dyn PortalUserStore>, snapshot: Arc<dyn SnapshotControl>) -> Self {
        Self { users, snapshot }
    }
}

#[async_trait]
impl PortalUserService for DefaultPortalUserService {
    async fn create(
        &self,
        context: &MutationContext,
        command: CreatePortalUser,
    ) -> Result<PortalUser, PortalError> {
        validate_username(&command.username)?;
        validate_password(&command.password)?;
        let hash = hash_password_blocking(command.password.clone()).await?;
        self.users
            .create_user(command, &hash, context)
            .await
            .map_err(super::store_error)
    }

    async fn list(&self, query: PortalUserListQuery) -> Result<PortalUserPage, PortalError> {
        self.users
            .list_users(query)
            .await
            .map_err(super::store_error)
    }

    async fn reset_password(
        &self,
        context: &MutationContext,
        command: ResetPortalPassword,
    ) -> Result<(), PortalError> {
        validate_password(&command.password)?;
        let hash = hash_password_blocking(command.password.clone()).await?;
        self.users
            .reset_password(command, &hash, context)
            .await
            .map_err(super::store_error)
    }

    async fn set_enabled(
        &self,
        context: &MutationContext,
        command: SetPortalUserEnabled,
    ) -> Result<PortalUser, PortalError> {
        let (revision, user) = self
            .users
            .set_enabled(command, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(user)
    }
}

fn validate_username(username: &str) -> Result<(), PortalError> {
    let username = username.trim();
    if username.is_empty() || username.len() > 64 || username.chars().any(char::is_control) {
        return Err(PortalError::invalid("用户名不合法"));
    }
    Ok(())
}
