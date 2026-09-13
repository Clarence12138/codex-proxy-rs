//! Portal 用例。

pub mod auth;
pub mod keys;
pub mod plans;
pub mod subscriptions;
pub mod usage;
pub mod users;

use gateway_core::routing::ConfigRevision;
use gateway_core::runtime::SnapshotControl;

use crate::model::PortalError;
use crate::ports::store::{PortalStoreError, map_store_error};

pub(crate) fn store_error(error: PortalStoreError) -> PortalError {
    map_store_error(error)
}

pub(crate) async fn publish_committed(
    snapshot: &dyn SnapshotControl,
    revision: u64,
) -> Result<(), PortalError> {
    let revision =
        ConfigRevision::new(revision).map_err(|_| PortalError::internal("配置版本不合法"))?;
    snapshot.publish_committed(revision).await;
    Ok(())
}
