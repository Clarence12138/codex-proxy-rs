//! 共用 Token 事实格式化，不根据分项推导总量。
use super::{format_compact_number, format_number};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenDetailsView {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub image_input_tokens: Option<u64>,
    pub image_output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub input_tokens_display: String,
    pub output_tokens_display: String,
    pub cached_tokens_display: String,
    pub cache_write_tokens_display: String,
    pub reasoning_tokens_display: String,
    pub image_input_tokens_display: String,
    pub image_output_tokens_display: String,
    pub total_tokens_display: String,
}

pub(crate) fn token_details(record: &gateway_core::metering::Usage) -> TokenDetailsView {
    TokenDetailsView {
        input_tokens: record.input_tokens,
        output_tokens: record.output_tokens,
        cached_tokens: record.cached_tokens,
        cache_write_tokens: record.cache_write_tokens,
        reasoning_tokens: record.reasoning_tokens,
        image_input_tokens: record.image_input_tokens,
        image_output_tokens: record.image_output_tokens,
        total_tokens: record.total_tokens,
        input_tokens_display: record
            .input_tokens
            .map_or_else(|| "-".to_owned(), format_number),
        output_tokens_display: record
            .output_tokens
            .map_or_else(|| "-".to_owned(), format_number),
        cached_tokens_display: record
            .cached_tokens
            .map_or_else(|| "-".to_owned(), format_compact_number),
        cache_write_tokens_display: record
            .cache_write_tokens
            .map_or_else(|| "-".to_owned(), format_compact_number),
        reasoning_tokens_display: record
            .reasoning_tokens
            .map_or_else(|| "-".to_owned(), format_number),
        image_input_tokens_display: record
            .image_input_tokens
            .map_or_else(|| "-".to_owned(), format_number),
        image_output_tokens_display: record
            .image_output_tokens
            .map_or_else(|| "-".to_owned(), format_number),
        total_tokens_display: record
            .total_tokens
            .map_or_else(|| "-".to_owned(), format_number),
    }
}
