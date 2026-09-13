use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use gateway_portal::{
    PortalConfig, initialize,
    model::{
        MutationActor, MutationContext, PortalErrorKind,
        plans::{CreatePlan, PlanListQuery, PlanPage, SubscriptionPlan, UpdatePlan},
    },
    ports::store::{PortalPlanStore, PortalStorePorts, PortalStoreResult},
};

use super::auth::{MemoryAuth, NoopSnapshot};

#[derive(Default)]
struct PlanSpy(Mutex<Vec<String>>);

impl PlanSpy {
    fn record(&self, name: String) -> PortalStoreResult<(u64, SubscriptionPlan)> {
        self.0.lock().unwrap().push(name.clone());
        Ok((
            1,
            SubscriptionPlan {
                id: "plan_test".to_owned(),
                name,
                budget: Default::default(),
                limits: Default::default(),
                max_keys: 0,
                group_ids: vec!["group_test".to_owned()],
                enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ))
    }
}

#[async_trait]
impl PortalPlanStore for PlanSpy {
    async fn create_plan(
        &self,
        command: CreatePlan,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)> {
        self.record(command.name)
    }
    async fn update_plan(
        &self,
        command: UpdatePlan,
        _: &MutationContext,
    ) -> PortalStoreResult<(u64, SubscriptionPlan)> {
        self.record(command.name)
    }
    async fn list_plans(&self, _: PlanListQuery) -> PortalStoreResult<PlanPage> {
        unreachable!()
    }
    async fn load_plan(&self, _: &str) -> PortalStoreResult<Option<SubscriptionPlan>> {
        unreachable!()
    }
}

#[tokio::test]
async fn create_and_update_validate_plan_name_before_store() {
    let other = Arc::new(MemoryAuth::default());
    let store = Arc::new(PlanSpy::default());
    let services = initialize(
        PortalConfig::default(),
        PortalStorePorts::new(
            other.clone(),
            other.clone(),
            store.clone(),
            other.clone(),
            other.clone(),
            other,
        ),
        Arc::new(NoopSnapshot),
    )
    .unwrap()
    .services();
    let context = MutationContext {
        actor: MutationActor::System,
        request_id: "name-validation".to_owned(),
    };
    let cases = [
        ("".to_owned(), Some("请输入套餐名称")),
        (" \t\n　 ".to_owned(), Some("请输入套餐名称")),
        ("a".repeat(65), Some("套餐名称不能超过 64 个 UTF-8 字节")),
        ("中".repeat(22), Some("套餐名称不能超过 64 个 UTF-8 字节")),
        ("a\nb".to_owned(), Some("套餐名称不能包含控制字符")),
        ("a\u{0085}b".to_owned(), Some("套餐名称不能包含控制字符")),
        ("a\0b".to_owned(), Some("套餐名称不能包含控制字符")),
        ("a".repeat(64), None),
        (format!("{}a", "中".repeat(21)), None),
        (format!("　{}a　", "中".repeat(21)), None),
    ];
    for (name, expected) in cases {
        let before = store.0.lock().unwrap().len();
        let create = services
            .plans()
            .create(
                &context,
                CreatePlan {
                    name: name.clone(),
                    budget: Default::default(),
                    limits: Default::default(),
                    max_keys: 0,
                    group_ids: vec!["group_test".to_owned()],
                },
            )
            .await;
        let update = services
            .plans()
            .update(
                &context,
                UpdatePlan {
                    id: "plan_test".to_owned(),
                    name,
                    budget: Default::default(),
                    limits: Default::default(),
                    max_keys: 0,
                    group_ids: vec!["group_test".to_owned()],
                    enabled: true,
                },
            )
            .await;
        for result in [create, update] {
            if let Some(message) = expected {
                let error = result.expect_err("invalid name rejected");
                assert_eq!(error.kind(), PortalErrorKind::Invalid);
                assert_eq!(error.message(), message);
            } else {
                result.expect("valid byte boundary accepted");
            }
        }
        assert_eq!(
            store.0.lock().unwrap().len(),
            before + if expected.is_none() { 2 } else { 0 }
        );
    }
}
