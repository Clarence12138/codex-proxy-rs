//! 下游客户端发送的非 Codex 模型请求参数兼容。

use serde_json::{Map, Value};

/// 仅移除已确认不适用于 Codex Responses 的顶层参数，不递归清洗业务正文。
///
/// 兼容基准是 Codex Core/Desktop 的模型请求，不是公开 OpenAI Responses API。
/// 未知字段继续透传，不能因官方请求结构中没有某个字段就将其列入过滤规则。
pub(in crate::transport) fn strip_non_codex_request_fields(body: &mut Map<String, Value>) {
    for field in [
        // Pi 普通 Responses 适配将 maxTokens 映射为 max_output_tokens，
        // temperature 则原样写入；Pi 的 Codex 适配也可能发送 temperature。
        "max_output_tokens",
        "temperature",
        // Pi 开启长缓存时发送 24h；保留有效的 prompt_cache_key，
        // 只剥离 Codex Responses 明确拒绝的缓存保留时长参数。
        "prompt_cache_retention",
    ] {
        body.remove(field);
    }
}
