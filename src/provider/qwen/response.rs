//! `Qwen` 响应反序列化。
//!
//! 将 Qwen OpenAI-compatible 非流式响应解析为公共响应模型。

use crate::{ChatResponse, LlmError};

/// 将 `Qwen` OpenAI-compatible 非流式响应体解析为公共聊天响应。
/// # Errors
/// - [`LlmError::ParseError`]：当响应体不是合法 `JSON` 时触发
/// - [`LlmError::ApiError`]：当响应体是 `Qwen` 错误对象，或缺少必要字段时触发
pub fn parse_chat_response(body: &[u8]) -> Result<ChatResponse, LlmError> {
    crate::provider::openai::response::parse_chat_response_with_provider(body, "Qwen")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_chat_response;
    use crate::FinishReason;

    #[test]
    fn qwen_openai_compatible_chat_response() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": "你好，我可以帮你分析代码。"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 8,
                "total_tokens": 20
            }
        })
        .to_string();

        let response = parse_chat_response(body.as_bytes()).expect("响应应解析成功");

        assert_eq!(response.content, "你好，我可以帮你分析代码。");
        assert_eq!(response.finish_reason.as_ref(), Some(&FinishReason::Stop));
        assert_eq!(response.usage.as_ref().expect("应包含用量").total, 20);
    }

    #[test]
    fn qwen_openai_compatible_tool_calls() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": null,
                        "tool_calls": [
                            {
                                "id": "call_1",
                                "type": "function",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"city\":\"杭州\"}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }
            ]
        })
        .to_string();

        let response = parse_chat_response(body.as_bytes()).expect("响应应解析成功");

        assert!(response.has_tool_calls());
        assert_eq!(
            response.tool_calls.as_ref().expect("应包含工具调用")[0].name,
            "get_weather"
        );
        assert_eq!(response.finish_reason.as_ref(), Some(&FinishReason::ToolCalls));
    }

    #[test]
    fn qwen_openai_compatible_content_parts() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": [
                            {
                                "type": "text",
                                "text": "第一段。"
                            },
                            {
                                "type": "refusal",
                                "refusal": "第二段。"
                            }
                        ]
                    },
                    "finish_reason": "stop"
                }
            ]
        })
        .to_string();

        let response = parse_chat_response(body.as_bytes()).expect("响应应解析成功");

        assert_eq!(response.content, "第一段。第二段。");
    }

    #[test]
    fn qwen_openai_compatible_api_error() {
        let body = json!({
            "error": {
                "message": "API Key 无效",
                "type": "invalid_request_error"
            }
        })
        .to_string();

        let error = parse_chat_response(body.as_bytes()).expect_err("应返回错误");

        match error {
            crate::LlmError::ApiError {
                status_code,
                message,
                provider,
            } => {
                assert_eq!(status_code, 0);
                assert_eq!(message, "API Key 无效");
                assert_eq!(provider, "Qwen");
            }
            other => panic!("错误类型不符合预期：{other:?}"),
        }
    }

    #[test]
    fn qwen_openai_compatible_reasoning_content() {
        let body = json!({
            "choices": [
                {
                    "message": {
                        "content": "最终答案",
                        "reasoning_content": "先分析题意"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 20,
                "completion_tokens_details": {
                    "reasoning_tokens": 9
                }
            }
        })
        .to_string();

        let response = parse_chat_response(body.as_bytes()).expect("响应应解析成功");

        assert_eq!(response.thinking_content.as_deref(), Some("先分析题意"));
        assert_eq!(response.thinking_tokens, Some(9));
        assert_eq!(response.content, "最终答案");
    }
}
