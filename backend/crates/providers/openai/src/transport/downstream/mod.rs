//! 下游来源适配，与官方 OpenAI/Codex 上游协议实现分开维护。

mod headers;

pub(super) use headers::decode_passthrough_headers;
