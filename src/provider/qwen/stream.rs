//! `Qwen` 流式解析。
//!
//! 复用 Qwen OpenAI-compatible 的 `SSE` 事件解析逻辑。

use crate::{LlmError, StreamChunk};

use crate::provider::openai::OpenAiStreamParser;

/// `Qwen` OpenAI-compatible 流式事件解析器。
#[derive(Debug, Default)]
pub struct QwenStreamParser {
    inner: OpenAiStreamParser,
}

impl QwenStreamParser {
    /// 创建流式事件解析器。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 重置内部累积状态。
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// 解析单条 `SSE` 事件的 `data:` 文本。
    /// # Errors
    /// - [`LlmError::ParseError`]：当事件数据不是合法 `JSON` 时触发
    /// - [`LlmError::StreamError`]：当事件缺少必要字段或工具调用碎片不完整时触发
    pub fn parse_event(&mut self, event_data: &str) -> Result<Option<StreamChunk>, LlmError> {
        self.inner
            .parse_event(event_data)
            .map_err(rewrite_provider_in_stream_error)
    }

    /// 解析单条 `SSE` 事件并返回其中全部片段。
    /// # Errors
    /// - [`LlmError::ParseError`]：当事件数据不是合法 `JSON` 时触发
    /// - [`LlmError::StreamError`]：当事件缺少必要字段或工具调用碎片不完整时触发
    pub fn parse_event_chunks(&mut self, event_data: &str) -> Result<Vec<StreamChunk>, LlmError> {
        self.inner
            .parse_event_chunks(event_data)
            .map_err(rewrite_provider_in_stream_error)
    }
}

fn rewrite_provider_in_stream_error(error: LlmError) -> LlmError {
    match error {
        LlmError::StreamError(message) => LlmError::StreamError(message.replace("OpenAI", "Qwen")),
        other => other,
    }
}

/// 判断事件是否为 `[DONE]` 终止标记。
#[cfg(test)]
#[must_use]
pub fn is_done_event(event_data: &str) -> bool {
    event_data.trim() == "[DONE]"
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{QwenStreamParser, is_done_event};
    use crate::FinishReason;

    #[test]
    fn qwen_openai_compatible_done_chunk() {
        let mut parser = QwenStreamParser::new();

        let chunk = parser.parse_event("[DONE]").expect("事件应解析成功");

        assert!(chunk.is_none());
        assert!(is_done_event("[DONE]"));
    }

    #[test]
    fn qwen_openai_compatible_text_stream() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "content": "你"
                    },
                    "finish_reason": null
                }
            ]
        })
        .to_string();
        let mut parser = QwenStreamParser::new();

        let chunk = parser
            .parse_event(&body)
            .expect("事件应解析成功")
            .expect("应产出增量");

        assert_eq!(chunk.delta, "你");
        assert!(!chunk.is_terminal());
    }

    #[test]
    fn qwen_openai_compatible_stop_chunk() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "content": ""
                    },
                    "finish_reason": "stop"
                }
            ]
        })
        .to_string();
        let mut parser = QwenStreamParser::new();

        let chunk = parser
            .parse_event(&body)
            .expect("事件应解析成功")
            .expect("应产出尾片段");

        assert_eq!(chunk.finish_reason.as_ref(), Some(&FinishReason::Stop));
        assert!(chunk.is_terminal());
    }

    #[test]
    fn qwen_openai_compatible_tool_call_stream() {
        let mut parser = QwenStreamParser::new();
        let first = json!({
            "choices": [
                {
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": "call_1",
                                "function": {
                                    "name": "get_weather",
                                    "arguments": "{\"city\":"
                                }
                            }
                        ]
                    },
                    "finish_reason": null
                }
            ]
        })
        .to_string();
        let second = json!({
            "choices": [
                {
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "function": {
                                    "arguments": "\"杭州\"}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }
            ]
        })
        .to_string();

        assert!(
            parser
                .parse_event(&first)
                .expect("第一段应解析成功")
                .is_none()
        );

        let chunk = parser
            .parse_event(&second)
            .expect("第二段应解析成功")
            .expect("应输出完整工具调用");

        assert_eq!(chunk.finish_reason.as_ref(), Some(&FinishReason::ToolCalls));
        assert_eq!(
            chunk.tool_calls.as_ref().expect("应包含工具调用")[0].arguments,
            "{\"city\":\"杭州\"}"
        );
    }

    #[test]
    fn qwen_openai_compatible_usage_chunk() {
        let body = json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 8,
                "total_tokens": 20
            }
        })
        .to_string();
        let mut parser = QwenStreamParser::new();

        let chunk = parser
            .parse_event(&body)
            .expect("事件应解析成功")
            .expect("应产出 usage 尾片段");

        assert_eq!(chunk.usage.as_ref().expect("应包含 usage").total, 20);
    }

    #[test]
    fn qwen_openai_compatible_reasoning_chunk() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "reasoning_content": "先分析"
                    },
                    "finish_reason": null
                }
            ]
        })
        .to_string();
        let mut parser = QwenStreamParser::new();

        let chunk = parser
            .parse_event(&body)
            .expect("事件应解析成功")
            .expect("应产出思考增量");

        assert!(chunk.is_thinking());
        assert_eq!(chunk.delta, "先分析");
    }

    #[test]
    fn qwen_openai_compatible_reasoning_and_text_chunks() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "reasoning_content": "先分析",
                        "content": "最终答案"
                    },
                    "finish_reason": null
                }
            ]
        })
        .to_string();
        let mut parser = QwenStreamParser::new();

        let chunks = parser.parse_event_chunks(&body).expect("事件应解析成功");

        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].is_thinking());
        assert_eq!(chunks[0].delta, "先分析");
        assert_eq!(chunks[1].delta, "最终答案");
        assert!(!chunks[1].is_thinking());
    }

    #[test]
    fn qwen_openai_compatible_parse_event_returns_last_chunk() {
        let body = json!({
            "choices": [
                {
                    "delta": {
                        "reasoning_content": "先分析",
                        "content": "最终答案"
                    },
                    "finish_reason": null
                }
            ]
        })
        .to_string();
        let mut parser = QwenStreamParser::new();

        let chunk = parser
            .parse_event(&body)
            .expect("事件应解析成功")
            .expect("应返回最后片段");

        assert!(!chunk.is_thinking());
        assert_eq!(chunk.delta, "最终答案");
    }
}
