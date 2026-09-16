use gateway_core::operation::{GenerateRequest, ProtocolPayload};
use provider_openai::encode_generate_request;
use serde_json::{Value, json};

#[test]
fn encoder_should_adapt_pi_responses_parameters_without_losing_codex_fields() {
    // 对照本机 Pi 0.79.0 普通 Responses 适配在 onPayload 阶段生成的正文；
    // maxTokens、temperature 和长缓存选项最终会产生下面三个顶层字段。
    let body = json!({
        "model": "client-model",
        "input": [{"role": "user", "content": [{"type": "input_text", "text": "hello"}]}],
        "stream": true,
        "store": false,
        "prompt_cache_key": "client-session",
        "prompt_cache_retention": "24h",
        "max_output_tokens": 512,
        "temperature": 0.2,
        "reasoning": {"effort": "high", "summary": "auto"},
        "include": ["reasoning.encrypted_content"]
    });
    let payload =
        ProtocolPayload::json_object("openai", body.as_object().expect("request object").clone())
            .expect("OpenAI payload");
    let encoded = encode_generate_request(
        &GenerateRequest::from_protocol_payload(payload),
        "gpt-test",
        None,
    )
    .expect("encode Pi request");

    assert_eq!(
        Value::Object(encoded.body().clone()),
        json!({
            "model": "gpt-test",
            "input": body["input"],
            "stream": true,
            "store": false,
            "prompt_cache_key": "client-session",
            "reasoning": {"effort": "high", "summary": "auto"},
            "include": ["reasoning.encrypted_content"]
        })
    );
}

#[test]
fn encoder_should_preserve_unknown_fields_and_nested_business_parameters() {
    let body = json!({
        "model": "client-model",
        "input": "temperature and max_output_tokens are tool parameter names",
        "max_tokens": 256,
        "future_options": {"temperature": 0.7},
        "client_metadata": {"prompt_cache_retention": "business-value"},
        "tools": [{
            "type": "function", "name": "configure_sampler",
            "parameters": {
                "type": "object",
                "properties": {
                    "temperature": {"type": "number"},
                    "max_output_tokens": {"type": "integer"},
                    "prompt_cache_retention": {"type": "string"}
                }
            }
        }]
    });
    let payload =
        ProtocolPayload::json_object("openai", body.as_object().expect("request object").clone())
            .expect("OpenAI payload");
    let encoded = encode_generate_request(
        &GenerateRequest::from_protocol_payload(payload),
        "client-model",
        None,
    )
    .expect("encode business parameters");

    assert_eq!(Value::Object(encoded.body().clone()), body);
}
