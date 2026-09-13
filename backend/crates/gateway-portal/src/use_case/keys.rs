//! 用户 Key 用例。分组由存储层按当前套餐写入。

use std::sync::Arc;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gateway_core::policy::ClientApiKeyId;
use gateway_core::runtime::SnapshotControl;
use rand_core::{OsRng, RngCore as _};
use uuid::Uuid;

use crate::{
    model::{
        MutationContext, PortalError,
        keys::{
            CreatePortalKey, CreatedPortalKey, PortalKeyRecord, RevealedPortalKey, UpdatePortalKey,
        },
    },
    ports::store::PortalKeyStore,
};

/// 用户 Key 服务。
#[async_trait]
pub trait PortalKeyService: Send + Sync {
    async fn list(&self, user_id: &str) -> Result<Vec<PortalKeyRecord>, PortalError>;
    async fn create(
        &self,
        context: &MutationContext,
        user_id: &str,
        command: CreatePortalKey,
    ) -> Result<CreatedPortalKey, PortalError>;
    async fn update(
        &self,
        context: &MutationContext,
        user_id: &str,
        command: UpdatePortalKey,
    ) -> Result<PortalKeyRecord, PortalError>;
    async fn set_enabled(
        &self,
        context: &MutationContext,
        user_id: &str,
        key_id: &str,
        enabled: bool,
    ) -> Result<PortalKeyRecord, PortalError>;
    async fn delete(
        &self,
        context: &MutationContext,
        user_id: &str,
        key_id: &str,
    ) -> Result<(), PortalError>;
    async fn reveal(&self, user_id: &str, key_id: &str) -> Result<RevealedPortalKey, PortalError>;
}

pub(crate) struct DefaultPortalKeyService {
    store: Arc<dyn PortalKeyStore>,
    snapshot: Arc<dyn SnapshotControl>,
}

impl DefaultPortalKeyService {
    #[must_use]
    pub(crate) fn new(store: Arc<dyn PortalKeyStore>, snapshot: Arc<dyn SnapshotControl>) -> Self {
        Self { store, snapshot }
    }
}

#[async_trait]
impl PortalKeyService for DefaultPortalKeyService {
    async fn list(&self, user_id: &str) -> Result<Vec<PortalKeyRecord>, PortalError> {
        self.store
            .list_keys(user_id)
            .await
            .map_err(super::store_error)
    }

    async fn create(
        &self,
        context: &MutationContext,
        user_id: &str,
        command: CreatePortalKey,
    ) -> Result<CreatedPortalKey, PortalError> {
        let key_id = format!("key_{}", Uuid::now_v7().simple());
        let _ = ClientApiKeyId::new(&key_id).map_err(|_| PortalError::internal("Key ID 不合法"))?;
        let plaintext = random_client_key();
        let (revision, record) = self
            .store
            .create_key(user_id, command, &plaintext, &key_id, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(CreatedPortalKey { record, plaintext })
    }

    async fn update(
        &self,
        context: &MutationContext,
        user_id: &str,
        command: UpdatePortalKey,
    ) -> Result<PortalKeyRecord, PortalError> {
        let (revision, record) = self
            .store
            .update_key(user_id, command, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(record)
    }

    async fn set_enabled(
        &self,
        context: &MutationContext,
        user_id: &str,
        key_id: &str,
        enabled: bool,
    ) -> Result<PortalKeyRecord, PortalError> {
        let (revision, record) = self
            .store
            .set_enabled(user_id, key_id, enabled, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(record)
    }

    async fn delete(
        &self,
        context: &MutationContext,
        user_id: &str,
        key_id: &str,
    ) -> Result<(), PortalError> {
        let revision = self
            .store
            .delete_key(user_id, key_id, context)
            .await
            .map_err(super::store_error)?;
        super::publish_committed(self.snapshot.as_ref(), revision).await?;
        Ok(())
    }

    async fn reveal(&self, user_id: &str, key_id: &str) -> Result<RevealedPortalKey, PortalError> {
        self.store
            .reveal_key(user_id, key_id)
            .await
            .map_err(super::store_error)?
            .map(|(record, plaintext)| RevealedPortalKey { record, plaintext })
            .ok_or_else(|| PortalError::not_found("密钥不存在"))
    }
}

fn random_client_key() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("sk_{}", URL_SAFE_NO_PAD.encode(bytes))
}
