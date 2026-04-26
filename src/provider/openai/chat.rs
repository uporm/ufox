use crate::{
    error::LlmError,
    types::{
        content::{ContentPart, Message, Role, Tool, ToolCall, ToolChoice, ToolResultPayload},
        request::ChatRequest,
        response::ChatResponse,
    },
};

use super::{OpenAiAdapter, CHAT_COMPLETIONS_PATH};

impl OpenAiAdapter {
    // 统一走同一个错误出口，避免各角色分支因为复制粘贴而漂移。
    fn unsupported_multimodal_error(&self, role: Role) -> LlmError {
        LlmError::UnsupportedCapability {
            provider: Some(self.name().into()),
            capability: match role {
                Role::User => "user_multimodal_content",
                Role::System => "system_multimodal_content",
                Role::Assistant => "assistant_multimodal_content",
                Role::Tool => "tool_multimodal_content",
            }
            .into(),
        }
    }

    fn to_tool_role_messages(&self, message: &Message) -> Result<Vec<serde_json::Value>, LlmError> {
        let mut out = Vec::with_capacity(message.content.len());
        for part in &message.content {
            // OpenAI 要求每个 tool result 都对应一条独立的 `tool` 消息，不能合并。
            let ContentPart::ToolResult(result) = part else {
                return Err(LlmError::ToolProtocol {
                    message: "tool role 仅允许 ToolResult".into(),
                });
            };

            let content = match &result.payload {
                ToolResultPayload::Text(text) => text.clone(),
                ToolResultPayload::Json(value) => value.to_string(),
            };
            out.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": result.tool_call_id,
                "content": content,
            }));
        }
        Ok(out)
    }

    fn to_assistant_chat_message(
        &self,
        message: &Message,
    ) -> Result<serde_json::Value, LlmError> {
        let mut text = String::new();
        let mut tool_calls = Vec::new();
        for part in &message.content {
            match part {
                ContentPart::Text(value) => text.push_str(&value.text),
                ContentPart::ToolCall(call) => {
                    tool_calls.push(serde_json::json!({
                        "id": call.id,
                        "type": "function",
                        "function": {
                            "name": call.tool_name,
                            "arguments": call.arguments.to_string(),
                        }
                    }));
                }
                _ => return Err(self.unsupported_multimodal_error(message.role)),
            }
        }

        let mut obj = serde_json::Map::new();
        // 这里保持字符串 `content`，避免 assistant 在带 tool_calls 时出现请求形态漂移。
        obj.insert("role".into(), "assistant".into());
        obj.insert("content".into(), text.into());
        if let Some(name) = &message.name {
            obj.insert("name".into(), name.clone().into());
        }
        if !tool_calls.is_empty() {
            obj.insert("tool_calls".into(), tool_calls.into());
        }
        Ok(serde_json::Value::Object(obj))
    }

    async fn to_user_or_system_chat_message(
        &self,
        message: &Message,
    ) -> Result<serde_json::Value, LlmError> {
        let mut parts = Vec::with_capacity(message.content.len());
        for part in &message.content {
            match part {
                ContentPart::Text(value) => parts.push(serde_json::json!({
                    "type": "text",
                    "text": value.text,
                })),
                ContentPart::Image(image) => {
                    let image_url =
                        Self::resolve_media_source_to_image_url(&image.source).await?;
                    parts.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": image_url,
                    }));
                }
                _ => return Err(self.unsupported_multimodal_error(message.role)),
            }
        }

        // user/system 统一用分段数组，便于同时承载文本和图片输入。
        let role = match message.role {
            Role::User => "user",
            Role::System => "system",
            _ => unreachable!(),
        };

        let mut obj = serde_json::Map::new();
        obj.insert("role".into(), role.into());
        obj.insert("content".into(), parts.into());
        if let Some(name) = &message.name {
            obj.insert("name".into(), name.clone().into());
        }
        Ok(serde_json::Value::Object(obj))
    }

    /// 将内部消息序列转换为 OpenAI Chat Completions 所需的 `messages` 数组。
    ///
    /// # Errors
    /// 当消息内容与角色协议约束不匹配，或包含当前适配器不支持的内容时返回错误。
    pub(super) async fn to_chat_messages(
        &self,
        messages: &[Message],
    ) -> Result<Vec<serde_json::Value>, LlmError> {
        let mut out = Vec::with_capacity(messages.len());
        for message in messages {
            // 按角色分发，主流程只保留协议层的路由，具体形态约束收敛到对应转换函数里。
            match message.role {
                Role::Tool => out.extend(self.to_tool_role_messages(message)?),
                Role::Assistant => out.push(self.to_assistant_chat_message(message)?),
                Role::User | Role::System => {
                    out.push(self.to_user_or_system_chat_message(message).await?)
                }
            }
        }
        Ok(out)
    }

    /// 构造 OpenAI `tools` 字段。
    pub(super) fn build_tools_payload(tools: &[Tool]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.input_schema,
                    }
                })
            })
            .collect()
    }

    /// 构造 OpenAI `tool_choice` 字段。
    pub(super) fn build_tool_choice_payload(choice: &ToolChoice) -> serde_json::Value {
        match choice {
            ToolChoice::Auto => "auto".into(),
            ToolChoice::None => "none".into(),
            ToolChoice::Required => "required".into(),
            ToolChoice::Specific(name) => serde_json::json!({
                "type": "function",
                "function": { "name": name }
            }),
        }
    }

    /// 构造 OpenAI Chat Completions 请求体。
    ///
    /// # Errors
    /// 当消息无法转换为兼容的 OpenAI 协议格式时返回错误。
    pub(super) async fn to_request_body(
        &self,
        model: &str,
        req: &ChatRequest,
        stream: bool,
    ) -> Result<serde_json::Value, LlmError> {
        let mut body = serde_json::Map::new();
        body.insert("model".into(), model.to_owned().into());
        body.insert(
            "messages".into(),
            self.to_chat_messages(&req.messages).await?.into(),
        );
        body.insert("stream".into(), stream.into());

        if let Some(max_tokens) = req.max_tokens {
            body.insert("max_tokens".into(), max_tokens.into());
        }
        if let Some(temperature) = req.temperature {
            body.insert("temperature".into(), temperature.into());
        }
        if let Some(top_p) = req.top_p {
            body.insert("top_p".into(), top_p.into());
        }
        if !req.tools.is_empty() {
            body.insert("tools".into(), Self::build_tools_payload(&req.tools).into());
            body.insert(
                "tool_choice".into(),
                Self::build_tool_choice_payload(&req.tool_choice),
            );
        }

        for (key, value) in &req.extensions {
            body.insert(key.clone(), value.clone());
        }

        Ok(serde_json::Value::Object(body))
    }

    /// 将 OpenAI Chat Completions 响应解析为统一的 `ChatResponse`。
    ///
    /// # Errors
    /// 当响应缺少关键字段，或工具调用参数不是合法 JSON 时返回错误。
    pub(super) fn parse_response(
        &self,
        raw: serde_json::Value,
        raw_field: Option<serde_json::Value>,
    ) -> Result<ChatResponse, LlmError> {
        let choice = raw
            .get("choices")
            .and_then(|value| value.as_array())
            .and_then(|choices| choices.first())
            .ok_or_else(|| LlmError::ProviderResponse {
                provider: self.name().into(),
                code: None,
                message: "缺少 choices[0]".into(),
            })?;

        let message = choice
            .get("message")
            .and_then(|value| value.as_object())
            .ok_or_else(|| LlmError::ProviderResponse {
                provider: self.name().into(),
                code: None,
                message: "缺少 message".into(),
            })?;
        let text = match message.get("content") {
            Some(serde_json::Value::String(text)) => text.clone(),
            Some(serde_json::Value::Null) | None => String::new(),
            // 兼容把文本拆成分段数组的实现，统一回收敛成单个字符串。
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.get("text")
                        .and_then(|value| value.as_str())
                        .map(str::to_owned)
                        .ok_or_else(|| LlmError::ProviderResponse {
                            provider: self.name().into(),
                            code: None,
                            message: "message.content 数组项缺少 text".into(),
                        })
                })
                .collect::<Result<Vec<_>, LlmError>>()?
                .join(""),
            Some(_) => {
                return Err(LlmError::ProviderResponse {
                    provider: self.name().into(),
                    code: None,
                    message: "message.content 形态不受支持".into(),
                });
            }
        };

        let tool_calls = message
            .get("tool_calls")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        let id = item.get("id").and_then(|value| value.as_str()).ok_or_else(
                            || LlmError::ToolProtocol {
                                message: "tool call 缺少 id".into(),
                            },
                        )?;
                        let function = item
                            .get("function")
                            .and_then(|value| value.as_object())
                            .ok_or_else(|| LlmError::ToolProtocol {
                                message: "tool call 缺少 function".into(),
                            })?;
                        let tool_name = function.get("name").and_then(|value| value.as_str()).ok_or_else(
                            || LlmError::ToolProtocol {
                                message: "tool call 缺少 name".into(),
                            },
                        )?;
                        let arguments_raw = match function.get("arguments") {
                            Some(serde_json::Value::String(arguments)) => arguments,
                            Some(_) => {
                                return Err(LlmError::ToolProtocol {
                                    message: "tool call arguments 不是字符串".into(),
                                });
                            }
                            None => {
                                return Err(LlmError::ToolProtocol {
                                    message: "tool call 缺少 arguments".into(),
                                });
                            }
                        };
                        // OpenAI 协议中的 arguments 是 JSON 字符串，这里恢复成结构化参数。
                        let arguments = serde_json::from_str(arguments_raw).map_err(|err| {
                            LlmError::ToolProtocol {
                                message: format!("tool arguments 解析失败: {err}"),
                            }
                        })?;

                        Ok(ToolCall {
                            id: id.to_owned(),
                            tool_name: tool_name.to_owned(),
                            arguments,
                        })
                    })
                    .collect::<Result<Vec<_>, LlmError>>()
            })
            .transpose()?
            .unwrap_or_default();

        Ok(ChatResponse {
            id: raw
                .get("id")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned(),
            model: raw
                .get("model")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned(),
            text,
            thinking: None,
            tool_calls,
            finish_reason: Self::parse_finish_reason(
                choice.get("finish_reason").and_then(|value| value.as_str()),
            ),
            usage: Self::parse_usage(raw.get("usage")),
            raw: raw_field,
        })
    }

    /// 执行一次非流式聊天请求。
    ///
    /// # Errors
    /// 当请求构造失败、传输失败，或提供方返回非成功状态码时返回错误。
    pub(super) async fn execute_chat(
        &self,
        model: &str,
        req: ChatRequest,
    ) -> Result<ChatResponse, LlmError> {
        let body = self.to_request_body(model, &req, false).await?;
        let response = self
            .transport
            .send(self.request_json(CHAT_COMPLETIONS_PATH).json(&body))
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await?;
            return Err(self.map_error_response(status, &body_text));
        }

        let raw: serde_json::Value = response.json().await?;
        self.parse_response(raw.clone(), Some(raw))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        middleware::{RetryConfig, Transport, TransportConfig},
        types::content::{ContentPart, Message},
    };

    use super::*;

    fn test_adapter() -> OpenAiAdapter {
        OpenAiAdapter::new(
            "compatible",
            "test-key",
            "https://example.com",
            Transport::new(TransportConfig {
                timeout_secs: 600,
                connect_timeout_secs: 5,
                read_timeout_secs: 60,
                retry: RetryConfig::default(),
                rate_limit: None,
            }),
        )
    }

    #[tokio::test]
    async fn request_body_preserves_extensions_and_message_name() {
        let adapter = test_adapter();
        let req = ChatRequest {
            messages: vec![Message {
                role: Role::User,
                content: vec![ContentPart::text("杭州天气")],
                name: Some("alice".into()),
            }],
            extensions: serde_json::Map::from_iter([(
                "reasoning".into(),
                serde_json::json!({ "effort": "medium" }),
            )]),
            ..ChatRequest::default()
        };

        let body = adapter
            .to_request_body("gpt-4o-mini", &req, false)
            .await
            .unwrap();

        assert_eq!(body.get("reasoning"), Some(&serde_json::json!({ "effort": "medium" })));
        assert_eq!(body["messages"][0]["name"], "alice");
    }

    #[test]
    fn parse_response_rejects_missing_tool_arguments() {
        let adapter = test_adapter();
        let error = adapter
            .parse_response(
                serde_json::json!({
                    "id": "chatcmpl_123",
                    "model": "gpt-4o-mini",
                    "choices": [{
                        "message": {
                            "content": "",
                            "tool_calls": [{
                                "id": "call_1",
                                "type": "function",
                                "function": {
                                    "name": "get_weather"
                                }
                            }]
                        },
                        "finish_reason": "tool_calls"
                    }]
                }),
                None,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            LlmError::ToolProtocol { ref message } if message == "tool call 缺少 arguments"
        ));
    }

    #[test]
    fn parse_response_rejects_missing_message() {
        let adapter = test_adapter();
        let error = adapter
            .parse_response(
                serde_json::json!({
                    "id": "chatcmpl_123",
                    "model": "gpt-4o-mini",
                    "choices": [{
                        "finish_reason": "stop"
                    }]
                }),
                None,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            LlmError::ProviderResponse { ref message, .. } if message == "缺少 message"
        ));
    }

    #[test]
    fn parse_response_rejects_non_text_content_array_items() {
        let adapter = test_adapter();
        let error = adapter
            .parse_response(
                serde_json::json!({
                    "id": "chatcmpl_123",
                    "model": "gpt-4o-mini",
                    "choices": [{
                        "message": {
                            "content": [{
                                "type": "image_url",
                                "image_url": { "url": "https://example.com/image.png" }
                            }]
                        },
                        "finish_reason": "stop"
                    }]
                }),
                None,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            LlmError::ProviderResponse { ref message, .. }
                if message == "message.content 数组项缺少 text"
        ));
    }
}
