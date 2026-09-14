//! Portal 用户控制面：用户、套餐、订阅与用户 Key/用量用例。
//!
//! 不依赖 `gateway-admin`，也不直接调用管理员级 Key/观测服务。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use gateway_core::runtime::SnapshotControl;
use serde::Deserialize;

pub mod model;
pub mod ports;
mod use_case;

pub use use_case::{
    auth::PortalAuthService, keys::PortalKeyService, plans::PortalPlanService,
    subscriptions::PortalSubscriptionService, usage::PortalUsageService, users::PortalUserService,
};

use model::PortalError;
use ports::store::PortalStorePorts;
use use_case::{
    auth::DefaultPortalAuthService, keys::DefaultPortalKeyService, plans::DefaultPortalPlanService,
    subscriptions::DefaultPortalSubscriptionService, usage::DefaultPortalUsageService,
    users::DefaultPortalUserService,
};

/// Portal 启动配置。
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PortalConfig {
    pub session_ttl_minutes: u64,
}

impl Default for PortalConfig {
    fn default() -> Self {
        Self {
            session_ttl_minutes: 1440,
        }
    }
}

impl PortalConfig {
    /// 校验 Portal 会话有效期。
    ///
    /// # Errors
    ///
    /// 会话 TTL 为零时返回错误。
    pub fn resolve_and_validate(&mut self, _source_dir: &Path) -> Result<(), PortalConfigError> {
        if self.session_ttl_minutes == 0 || i64::try_from(self.session_ttl_minutes).is_err() {
            return Err(PortalConfigError::InvalidField(
                "portal.session_ttl_minutes",
            ));
        }
        Ok(())
    }
}

/// Portal 配置非法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PortalConfigError {
    #[error("配置字段 `{0}` 不合法")]
    InvalidField(&'static str),
}

/// API 持有的 Portal 能力集合。
#[derive(Clone)]
pub struct PortalServices {
    auth: Arc<dyn PortalAuthService>,
    users: Arc<dyn PortalUserService>,
    plans: Arc<dyn PortalPlanService>,
    subscriptions: Arc<dyn PortalSubscriptionService>,
    keys: Arc<dyn PortalKeyService>,
    usage: Arc<dyn PortalUsageService>,
}

impl PortalServices {
    #[must_use]
    pub fn auth(&self) -> &dyn PortalAuthService {
        self.auth.as_ref()
    }

    #[must_use]
    pub fn users(&self) -> &dyn PortalUserService {
        self.users.as_ref()
    }

    #[must_use]
    pub fn plans(&self) -> &dyn PortalPlanService {
        self.plans.as_ref()
    }

    #[must_use]
    pub fn subscriptions(&self) -> &dyn PortalSubscriptionService {
        self.subscriptions.as_ref()
    }

    #[must_use]
    pub fn keys(&self) -> &dyn PortalKeyService {
        self.keys.as_ref()
    }

    #[must_use]
    pub fn usage(&self) -> &dyn PortalUsageService {
        self.usage.as_ref()
    }
}

/// Portal 初始化完成后的封闭能力包。
pub struct PortalBundle {
    services: PortalServices,
}

impl PortalBundle {
    #[must_use]
    pub fn services(&self) -> PortalServices {
        self.services.clone()
    }
}

/// 完成 Portal 用例组装。
///
/// # Errors
///
/// 配置非法时返回错误。
pub fn initialize(
    mut config: PortalConfig,
    store: PortalStorePorts,
    snapshot: Arc<dyn SnapshotControl>,
    billing: Arc<dyn gateway_core::metering::BillingResolver>,
) -> Result<PortalBundle, PortalError> {
    config
        .resolve_and_validate(Path::new("."))
        .map_err(|error| PortalError::invalid(error.to_string()))?;
    let ttl = Duration::from_secs(config.session_ttl_minutes.saturating_mul(60));
    Ok(PortalBundle {
        services: PortalServices {
            auth: Arc::new(DefaultPortalAuthService::new(ttl, store.auth())),
            users: Arc::new(DefaultPortalUserService::new(
                store.users(),
                snapshot.clone(),
            )),
            plans: Arc::new(DefaultPortalPlanService::new(
                store.plans(),
                snapshot.clone(),
            )),
            subscriptions: Arc::new(DefaultPortalSubscriptionService::new(
                store.subscriptions(),
                snapshot.clone(),
            )),
            keys: Arc::new(DefaultPortalKeyService::new(store.keys(), snapshot)),
            usage: Arc::new(DefaultPortalUsageService::new(store.usage(), billing)),
        },
    })
}
