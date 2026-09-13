use gateway_portal::model::subscriptions::{SubscriptionStatus, UserSubscription};

use chrono::{Duration, Utc};

#[test]
fn subscription_effective_window_excludes_end_instant() {
    let now = Utc::now();
    let subscription = UserSubscription {
        id: "sub_1".to_owned(),
        user_id: "usr_1".to_owned(),
        plan_id: "plan_1".to_owned(),
        status: SubscriptionStatus::Active,
        starts_at: now - Duration::hours(1),
        ends_at: Some(now),
        created_at: now,
        updated_at: now,
    };
    assert!(!subscription.is_effective(now));
    assert!(subscription.is_effective(now - Duration::seconds(1)));
}
