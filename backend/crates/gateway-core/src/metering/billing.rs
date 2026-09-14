//! 控制面共用的持久费用事实与只读分解能力，不承载 Provider 价格规则。

use super::Decimal;
use crate::identity::ProviderKind;
use crate::validation::MeteringError;
use std::str::FromStr;

/// 与数据库精度一致的规范化金额。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DecimalAmount(String);

impl DecimalAmount {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn checked_add(&self, other: &Self) -> Option<Self> {
        let left: Decimal = self.0.parse().ok()?;
        let right: Decimal = other.0.parse().ok()?;
        Some(Self(left.checked_add(right)?.canonical()))
    }

    #[must_use]
    pub fn checked_div_u64(&self, divisor: u64) -> Option<Self> {
        let amount: Decimal = self.0.parse().ok()?;
        Some(Self(amount.checked_div_u64(divisor)?.canonical()))
    }
}

impl FromStr for DecimalAmount {
    type Err = MeteringError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        // 保留控制面的既有输入合同：不接受末尾小数点。
        if input.ends_with('.') {
            return Err(MeteringError::InvalidDecimal);
        }
        let value: Decimal = input.parse()?;
        Ok(Self(value.canonical()))
    }
}

impl std::fmt::Display for DecimalAmount {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyCost {
    pub currency: String,
    pub amount: DecimalAmount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBillingInput {
    pub upstream_model_id: String,
    pub service_tier: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub total: CurrencyCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculatedBillingBreakdown {
    pub input_amount: CurrencyCost,
    pub output_amount: CurrencyCost,
    pub cache_read_amount: CurrencyCost,
    pub cache_write_amount: CurrencyCost,
    pub standard_amount: CurrencyCost,
    pub total_amount: CurrencyCost,
    pub input_price_per_million: CurrencyCost,
    pub output_price_per_million: CurrencyCost,
    pub cache_read_price_per_million: CurrencyCost,
    pub cache_write_price_per_million: CurrencyCost,
    pub service_tier: Option<String>,
    pub multiplier_percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageBilling {
    Total { source: String, total: CurrencyCost },
    Calculated(Box<CalculatedBillingBreakdown>),
}

/// 失败或无法校验时保留已存总额；接口不暴露管理员能力或原始错误。
pub trait BillingResolver: Send + Sync {
    fn resolve(
        &self,
        provider: &ProviderKind,
        input: &ProviderBillingInput,
    ) -> Option<CalculatedBillingBreakdown>;
}
