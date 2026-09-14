//! 两个控制面共用计费展示，输入只包含中立费用事实。

use super::format_decimal_currency;
use gateway_core::metering::{CurrencyCost, UsageBilling};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingView {
    pub input_amount_display: String,
    pub output_amount_display: String,
    pub cache_read_amount_display: String,
    pub cache_write_amount_display: String,
    pub standard_amount_display: String,
    pub total_amount_display: String,
    pub input_price_display: String,
    pub output_price_display: String,
    pub cache_read_price_display: String,
    pub cache_write_price_display: String,
    pub service_tier_display: String,
    pub multiplier_display: String,
}

pub(crate) fn format_money(cost: &CurrencyCost) -> String {
    format_decimal_currency(cost.amount.as_str(), &cost.currency)
}

pub(crate) fn format_token_price(cost: &CurrencyCost) -> String {
    if cost.currency != "USD" {
        return format!("{} {} / 1M Token", cost.currency, cost.amount.as_str());
    }
    format!("${} / 1M Token", cost.amount.as_str())
}

pub(crate) fn format_service_tier(service_tier: Option<&str>) -> String {
    match service_tier {
        Some("priority" | "fast") => "Fast".to_owned(),
        Some("flex") => "Flex".to_owned(),
        Some("default" | "standard") | None => "Standard".to_owned(),
        Some(other) => capitalize_first(other),
    }
}

fn capitalize_first(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

pub(crate) fn billing_view(billing: Option<&UsageBilling>) -> Option<BillingView> {
    match billing? {
        UsageBilling::Total { source, total } => Some(BillingView {
            input_amount_display: "—".to_owned(),
            output_amount_display: "—".to_owned(),
            cache_read_amount_display: "—".to_owned(),
            cache_write_amount_display: "—".to_owned(),
            standard_amount_display: "—".to_owned(),
            total_amount_display: if source == "calculated" {
                format!("≈ {}", format_money(total))
            } else {
                format_money(total)
            },
            input_price_display: "—".to_owned(),
            output_price_display: "—".to_owned(),
            cache_read_price_display: "—".to_owned(),
            cache_write_price_display: "—".to_owned(),
            service_tier_display: "—".to_owned(),
            multiplier_display: "—".to_owned(),
        }),
        UsageBilling::Calculated(value) => Some(BillingView {
            input_amount_display: format_money(&value.input_amount),
            output_amount_display: format_money(&value.output_amount),
            cache_read_amount_display: format_money(&value.cache_read_amount),
            cache_write_amount_display: format_money(&value.cache_write_amount),
            standard_amount_display: format_money(&value.standard_amount),
            total_amount_display: format_money(&value.total_amount),
            input_price_display: format_token_price(&value.input_price_per_million),
            output_price_display: format_token_price(&value.output_price_per_million),
            cache_read_price_display: format_token_price(&value.cache_read_price_per_million),
            cache_write_price_display: format_token_price(&value.cache_write_price_per_million),
            service_tier_display: format_service_tier(value.service_tier.as_deref()),
            multiplier_display: format!("{:.2}x", f64::from(value.multiplier_percent) / 100.0),
        }),
    }
}
