mod admin;
mod auth;

use std::sync::Arc;

use futures::future::BoxFuture;
use gateway_core::{
    engine::execution::{
        AuthenticatedClient, ClientAuthenticationError, ExecutionService, StartExecution,
        StartProviderExecution, StartedExecution,
    },
    error::GatewayError,
    routing::PublicModelId,
};

use crate::openai::api_router;

pub(super) async fn portal_router() -> axum::Router {
    api_router(Arc::new(UnusedExecution)).await
}

struct UnusedExecution;

impl ExecutionService for UnusedExecution {
    fn authenticate(&self, _: &str) -> Result<AuthenticatedClient, ClientAuthenticationError> {
        unreachable!("portal auth tests do not authenticate clients")
    }

    fn public_models(&self, _: &AuthenticatedClient) -> Vec<PublicModelId> {
        unreachable!("portal auth tests do not list models")
    }

    fn contains_public_model(&self, _: &AuthenticatedClient, _: &PublicModelId) -> bool {
        unreachable!("portal auth tests do not inspect models")
    }

    fn start(&self, _: StartExecution) -> BoxFuture<'_, Result<StartedExecution, GatewayError>> {
        Box::pin(async { unreachable!("portal auth tests do not execute") })
    }

    fn start_provider_endpoint(
        &self,
        _: StartProviderExecution,
    ) -> BoxFuture<'_, Result<StartedExecution, GatewayError>> {
        Box::pin(async { unreachable!("portal auth tests do not execute") })
    }
}
