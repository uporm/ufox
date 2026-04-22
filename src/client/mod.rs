//! 客户端模块。
//!
//! 提供 `Client`、请求构建器和请求发送主流程。

use std::{pin::Pin, sync::Arc, time::Duration};

use eventsource_stream::Eventsource;
use futures_util::{Stream, StreamExt, stream};
use reqwest::{Response, StatusCode, header::RETRY_AFTER};

use crate::{
    ChatResponse, LlmError, Message, Provider, ProviderAdapter, StreamChunk, Tool,
    types::{ChatRequest, RequestOptions},
};

mod builder;
mod debug;

use self::{
    builder::ClientSettings,
    debug::{
        debug_chat_response, debug_ignored_request_option, debug_request, debug_request_failure,
        debug_request_success, debug_stream_event,
    },
};

pub use builder::ClientBuilder;

/// 聊天流式响应类型。
///
/// 该类型是对异步流返回值的统一封装。流中的每一项都是一次增量输出或终止片段。
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<StreamChunk, LlmError>> + Send>>;

#[derive(Clone)]
pub struct Client {
    client_settings: ClientSettings,
    http: reqwest::Client,
    adapter: Arc<dyn ProviderAdapter>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    fn from_builder(client_settings: ClientSettings) -> Self {
        let adapter = client_settings.provider.make_adapter();

        Self {
            client_settings,
            http: reqwest::Client::new(),
            adapter,
        }
    }

    pub const fn provider(&self) -> Provider {
        self.client_settings.provider
    }

    /// 发送非流式聊天请求。
    ///
    /// 这是以 [`ChatRequest`] 为中心的主入口；当需要配置请求选项或工具调用时，优先使用该方法。
    /// # Errors
    /// - [`LlmError::ApiError`]：当 Provider 返回业务失败或本地关键配置缺失时触发
    /// - [`LlmError::AuthError`]：当接口返回 `401` 时触发
    /// - [`LlmError::RateLimitError`]：当接口返回 `429` 时触发
    /// - [`LlmError::NetworkError`]：当网络请求失败时触发
    /// - [`LlmError::ParseError`]：当响应体解析失败时触发
    pub async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let provider_name = self.adapter.provider_name().to_string();
        let response = self
            .send_request(
                &request.messages,
                request.tools.as_deref(),
                false,
                &request.options,
            )
            .await?;
        let body = response.bytes().await.map_err(LlmError::from)?;
        debug_chat_response(provider_name.as_str(), body.as_ref());
        self.adapter.parse_chat_response(body.as_ref())
    }

    /// 发送流式聊天请求。
    ///
    /// 这是以 [`ChatRequest`] 为中心的流式主入口；当需要配置请求选项或工具调用时，优先使用该方法。
    /// # Errors
    /// - [`LlmError::ApiError`]：当 Provider 返回业务失败或本地关键配置缺失时触发
    /// - [`LlmError::AuthError`]：当接口返回 `401` 时触发
    /// - [`LlmError::RateLimitError`]：当接口返回 `429` 时触发
    /// - [`LlmError::NetworkError`]：当网络请求失败时触发
    pub async fn chat_stream(&self, request: &ChatRequest) -> Result<ChatStream, LlmError> {
        let provider_name = self.adapter.provider_name().to_string();
        let response = self
            .send_request(
                &request.messages,
                request.tools.as_deref(),
                true,
                &request.options,
            )
            .await?;
        let adapter = Arc::clone(&self.adapter);
        let stream = response
            .bytes_stream()
            .eventsource()
            .map(move |event_result| match event_result {
                Ok(event) => {
                    debug_stream_event(provider_name.as_str(), &event.data);
                    match adapter.parse_stream_chunks(&event.data) {
                        Ok(chunks) => chunks.into_iter().map(Ok).collect::<Vec<_>>(),
                        Err(error) => vec![Err(error)],
                    }
                }
                Err(error) => vec![Err(LlmError::StreamError(format!(
                    "读取 SSE 事件失败：{error}"
                )))],
            })
            .flat_map(stream::iter);

        Ok(Box::pin(stream))
    }

    /// 使用默认请求选项发送非流式聊天请求。
    ///
    /// 当调用方无需额外配置 `thinking`、`tool_choice` 等参数时，可以直接使用该快捷方法。
    /// 该方法等价于先构造 `ChatRequest::new(messages)`，再调用 [`Self::chat`]。
    /// 若后续需要增加请求级配置，可无缝切换到 `ChatRequest` 入口。
    pub async fn chat_messages(
        &self,
        messages: impl AsRef<[Message]>,
    ) -> Result<ChatResponse, LlmError> {
        let request = ChatRequest::new(messages);
        self.chat(&request).await
    }

    /// 使用默认请求选项发送流式聊天请求。
    ///
    /// 当调用方无需额外配置请求选项时，可以直接使用该快捷方法。
    /// 该方法等价于先构造 `ChatRequest::new(messages)`，再调用 [`Self::chat_stream`]。
    /// 若后续需要增加请求级配置，可无缝切换到 `ChatRequest` 入口。
    pub async fn chat_stream_messages(
        &self,
        messages: impl AsRef<[Message]>,
    ) -> Result<ChatStream, LlmError> {
        let request = ChatRequest::new(messages);
        self.chat_stream(&request).await
    }

    async fn send_request(
        &self,
        messages: &[Message],
        tools: Option<&[Tool]>,
        stream: bool,
        options: &RequestOptions,
    ) -> Result<Response, LlmError> {
        let url = self.request_url()?;
        let model = self.required_model()?;
        let options = self.resolve_request_options(model, tools, options);
        let body = self
            .adapter
            .build_chat_request(model, messages, tools, stream, &options)?;
        let provider_name = self.adapter.provider_name().to_string();
        let request_body_json = serde_json::to_string(&body).unwrap_or_else(|error| {
            serde_json::json!({
                "serialization_error": error.to_string()
            })
            .to_string()
        });

        debug_request(
            provider_name.as_str(),
            model,
            stream,
            &url,
            &request_body_json,
        );

        let mut request = self.http.post(url).json(&body);
        request = request.header(
            "Authorization",
            format!("Bearer {}", self.client_settings.api_key),
        );

        request = apply_stream_headers(request, self.provider(), stream);

        if let Some(timeout_secs) = self.client_settings.timeout_secs {
            request = request.timeout(Duration::from_secs(timeout_secs));
        }

        if self.provider() == Provider::OpenAI
            && let Some(organization) = self.client_settings.organization.as_deref()
        {
            request = request.header("OpenAI-Organization", organization);
        }

        for (key, value) in &self.client_settings.extra_headers {
            request = request.header(key, value);
        }

        let response = request.send().await.map_err(LlmError::from)?;
        if response.status().is_success() {
            debug_request_success(provider_name.as_str(), response.status().as_u16());
            Ok(response)
        } else {
            let status = response.status();
            let retry_after = parse_retry_after_header(response.headers());
            let body = response.bytes().await.map_err(LlmError::from)?;
            debug_request_failure(
                provider_name.as_str(),
                status.as_u16(),
                retry_after,
                body.as_ref(),
            );
            Err(map_http_error(
                status,
                retry_after,
                body.as_ref(),
                self.adapter.provider_name(),
            ))
        }
    }

    fn request_url(&self) -> Result<String, LlmError> {
        let base_url = self
            .client_settings
            .base_url
            .as_deref()
            .or_else(|| self.adapter.default_base_url())
            .ok_or_else(|| LlmError::ApiError {
                status_code: 0,
                message: "当前 Provider 需要显式设置 base_url".to_string(),
                provider: self.provider().display_name().to_string(),
            })?;

        Ok(join_url(base_url, self.adapter.chat_path()))
    }

    fn required_model(&self) -> Result<&str, LlmError> {
        self.client_settings
            .default_model
            .as_deref()
            .ok_or_else(|| LlmError::ApiError {
                status_code: 0,
                message: "尚未设置默认模型，请在构建器中调用 .model(...)".to_string(),
                provider: self.provider().display_name().to_string(),
            })
    }

    fn resolve_request_options(
        &self,
        model: &str,
        tools: Option<&[Tool]>,
        options: &RequestOptions,
    ) -> RequestOptions {
        let provider_name = self.provider().display_name();
        let capability = self.adapter.thinking_capability(model);
        let mut resolved = RequestOptions {
            temperature: options.temperature,
            top_p: options.top_p,
            max_tokens: options.max_tokens,
            presence_penalty: options.presence_penalty,
            frequency_penalty: options.frequency_penalty,
            provider_options: options.provider_options.clone(),
            ..RequestOptions::default()
        };

        if options.thinking {
            if capability.supports_thinking {
                resolved.thinking = true;
            } else {
                debug_ignored_request_option(
                    provider_name,
                    model,
                    "thinking",
                    Some("true"),
                    "provider / model 不支持思考模式",
                );
            }
        }

        if let Some(thinking_budget) = options.thinking_budget {
            if capability.supports_thinking_budget {
                resolved.thinking = true;
                resolved.thinking_budget = Some(thinking_budget);
            } else {
                let thinking_budget_value = thinking_budget.to_string();
                debug_ignored_request_option(
                    provider_name,
                    model,
                    "thinking_budget",
                    Some(thinking_budget_value.as_str()),
                    "provider / model 不支持 thinking_budget",
                );
            }
        }

        if let Some(reasoning_effort) = options.reasoning_effort {
            if capability.supports_reasoning_effort {
                resolved.reasoning_effort = Some(reasoning_effort);
            } else {
                debug_ignored_request_option(
                    provider_name,
                    model,
                    "reasoning_effort",
                    Some(reasoning_effort.as_str()),
                    "provider / model 不支持 reasoning_effort",
                );
            }
        }

        if let Some(tool_choice) = options.tool_choice.clone() {
            if has_tools(tools) {
                resolved.tool_choice = Some(tool_choice);
            } else {
                debug_ignored_request_option(
                    provider_name,
                    model,
                    "tool_choice",
                    None,
                    "当前请求未传入 tools",
                );
            }
        }

        if let Some(parallel_tool_calls) = options.parallel_tool_calls {
            if has_tools(tools) {
                resolved.parallel_tool_calls = Some(parallel_tool_calls);
            } else {
                let parallel_tool_calls_value = parallel_tool_calls.to_string();
                debug_ignored_request_option(
                    provider_name,
                    model,
                    "parallel_tool_calls",
                    Some(parallel_tool_calls_value.as_str()),
                    "当前请求未传入 tools",
                );
            }
        }

        resolved
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("provider", &self.client_settings.provider)
            .finish_non_exhaustive()
    }
}
fn has_tools(tools: Option<&[Tool]>) -> bool {
    matches!(tools, Some(items) if !items.is_empty())
}

fn apply_stream_headers(
    request: reqwest::RequestBuilder,
    _provider: Provider,
    stream: bool,
) -> reqwest::RequestBuilder {
    if !stream {
        return request;
    }

    request.header("Accept", "text/event-stream")
}

fn join_url(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

fn parse_retry_after_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let raw = headers.get(RETRY_AFTER)?.to_str().ok()?;
    let secs = raw.parse::<u64>().ok()?;
    Some(Duration::from_secs(secs))
}

fn map_http_error(
    status: StatusCode,
    retry_after: Option<Duration>,
    body: &[u8],
    provider_name: &str,
) -> LlmError {
    match status {
        StatusCode::UNAUTHORIZED => LlmError::AuthError,
        StatusCode::TOO_MANY_REQUESTS => LlmError::RateLimitError { retry_after },
        _ => LlmError::ApiError {
            status_code: status.as_u16(),
            message: extract_error_message(body, status),
            provider: provider_name.to_string(),
        },
    }
}

fn extract_error_message(body: &[u8], status: StatusCode) -> String {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
        {
            return message.to_string();
        }

        if let Some(message) = value.get("message").and_then(serde_json::Value::as_str) {
            return message.to_string();
        }
    }

    let text = String::from_utf8_lossy(body).trim().to_string();
    if text.is_empty() {
        format!("请求失败，HTTP 状态码：{}", status.as_u16())
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use reqwest::StatusCode;
    use serde_json::json;

    use super::{
        Client, apply_stream_headers, extract_error_message, join_url, map_http_error,
        parse_retry_after_header,
    };
    use crate::{LlmError, Provider, ReasoningEffort, ToolChoice, types::RequestOptions};

    #[test]
    fn client_provider() {
        let client = Client::builder()
            .provider(Provider::Compatible)
            .base_url("https://api.deepseek.com/v1")
            .api_key("sk-demo")
            .model("deepseek-chat")
            .build()
            .expect("应构建成功");

        assert_eq!(client.provider(), Provider::Compatible);
    }

    #[test]
    fn join_url_base_url() {
        assert_eq!(
            join_url("https://api.openai.com/v1/", "/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn retry_after_duration() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("12"),
        );

        assert_eq!(
            parse_retry_after_header(&headers),
            Some(Duration::from_secs(12))
        );
    }

    #[test]
    fn map_http_error_returns_rate_limit_with_retry_after() {
        let error = map_http_error(
            StatusCode::TOO_MANY_REQUESTS,
            Some(Duration::from_secs(5)),
            r#"{"message":"请求过于频繁"}"#.as_bytes(),
            "openai",
        );

        match error {
            LlmError::RateLimitError { retry_after } => {
                assert_eq!(retry_after, Some(Duration::from_secs(5)));
            }
            other => panic!("错误类型不符合预期：{other:?}"),
        }
    }

    #[test]
    fn extract_error_message_reads_nested_error_message() {
        let message = extract_error_message(
            r#"{"error":{"message":"无效请求"}}"#.as_bytes(),
            StatusCode::BAD_REQUEST,
        );

        assert_eq!(message, "无效请求");
    }

    #[test]
    fn apply_stream_headers_sets_sse_accept_header_for_qwen_streaming() {
        let client = reqwest::Client::new();
        let request = apply_stream_headers(
            client.post("https://example.com/chat"),
            Provider::Qwen,
            true,
        )
        .build()
        .expect("请求应构建成功");

        assert_eq!(
            request.headers().get("Accept"),
            Some(&reqwest::header::HeaderValue::from_static(
                "text/event-stream"
            ))
        );
        assert!(request.headers().get("X-DashScope-SSE").is_none());
    }

    #[test]
    fn apply_stream_headers_sets_same_headers_for_non_qwen_streaming() {
        let client = reqwest::Client::new();
        let request = apply_stream_headers(
            client.post("https://example.com/chat"),
            Provider::OpenAI,
            true,
        )
        .build()
        .expect("请求应构建成功");

        assert_eq!(
            request.headers().get("Accept"),
            Some(&reqwest::header::HeaderValue::from_static(
                "text/event-stream"
            ))
        );
        assert!(request.headers().get("X-DashScope-SSE").is_none());
    }

    #[test]
    fn resolve_request_options_ignores_reasoning_effort_for_qwen3() {
        let client = Client::builder()
            .provider(Provider::Qwen)
            .api_key("sk-demo")
            .model("qwen3-max")
            .build()
            .expect("应构建成功");

        let options = client.resolve_request_options(
            "qwen3-max",
            None,
            &RequestOptions {
                thinking: true,
                thinking_budget: Some(8000),
                reasoning_effort: Some(ReasoningEffort::High),
                ..RequestOptions::default()
            },
        );

        assert!(options.thinking);
        assert_eq!(options.thinking_budget, Some(8000));
        assert_eq!(options.reasoning_effort, None);
    }

    #[test]
    fn resolve_request_options_ignores_thinking_settings_for_non_reasoning_openai_model() {
        let client = Client::builder()
            .provider(Provider::OpenAI)
            .api_key("sk-demo")
            .model("gpt-4o")
            .build()
            .expect("应构建成功");

        let options = client.resolve_request_options(
            "gpt-4o",
            None,
            &RequestOptions {
                thinking: true,
                thinking_budget: Some(4000),
                reasoning_effort: Some(ReasoningEffort::High),
                ..RequestOptions::default()
            },
        );

        assert!(!options.thinking);
        assert_eq!(options.thinking_budget, None);
        assert_eq!(options.reasoning_effort, None);
    }

    #[test]
    fn resolve_request_options_keeps_tool_settings_when_tools_are_provided() {
        let client = Client::builder()
            .provider(Provider::OpenAI)
            .api_key("sk-demo")
            .model("gpt-4o")
            .build()
            .expect("应构建成功");
        let tools = [crate::Tool::function("get_weather")
            .param("city", crate::JsonType::String, "城市名称", true)
            .build()];

        let options = client.resolve_request_options(
            "gpt-4o",
            Some(&tools),
            &RequestOptions {
                tool_choice: Some(ToolChoice::function("get_weather")),
                parallel_tool_calls: Some(true),
                ..RequestOptions::default()
            },
        );

        assert_eq!(options.parallel_tool_calls, Some(true));
        assert_eq!(
            options
                .tool_choice
                .as_ref()
                .and_then(ToolChoice::function_name),
            Some("get_weather")
        );
    }

    #[test]
    fn resolve_request_options_keeps_sampling_settings() {
        let client = Client::builder()
            .provider(Provider::OpenAI)
            .api_key("sk-demo")
            .model("gpt-4o")
            .build()
            .expect("应构建成功");

        let options = client.resolve_request_options(
            "gpt-4o",
            None,
            &RequestOptions {
                temperature: Some(0.4),
                top_p: Some(0.8),
                max_tokens: Some(1024),
                presence_penalty: Some(0.2),
                frequency_penalty: Some(0.1),
                provider_options: serde_json::Map::from_iter([(
                    "max_completion_tokens".to_string(),
                    json!(1536),
                )]),
                ..RequestOptions::default()
            },
        );

        assert_eq!(options.temperature, Some(0.4));
        assert_eq!(options.top_p, Some(0.8));
        assert_eq!(options.max_tokens, Some(1024));
        assert_eq!(options.presence_penalty, Some(0.2));
        assert_eq!(options.frequency_penalty, Some(0.1));
        assert_eq!(
            options.provider_options.get("max_completion_tokens"),
            Some(&json!(1536))
        );
    }

    #[test]
    fn resolve_request_options_drops_tool_settings_when_tools_are_missing() {
        let client = Client::builder()
            .provider(Provider::OpenAI)
            .api_key("sk-demo")
            .model("gpt-4o")
            .build()
            .expect("应构建成功");

        let options = client.resolve_request_options(
            "gpt-4o",
            None,
            &RequestOptions {
                tool_choice: Some(ToolChoice::Required),
                parallel_tool_calls: Some(true),
                ..RequestOptions::default()
            },
        );

        assert_eq!(options.tool_choice, None);
        assert_eq!(options.parallel_tool_calls, None);
    }

    #[test]
    fn compatible_deepseek_reasoner() {
        let client = Client::builder()
            .provider(Provider::Compatible)
            .base_url("https://example.com/v1")
            .api_key("sk-demo")
            .model("vendor/deepseek-reasoner")
            .build()
            .expect("应构建成功");

        let options = client.resolve_request_options(
            "vendor/deepseek-reasoner",
            None,
            &RequestOptions {
                thinking: true,
                ..RequestOptions::default()
            },
        );

        assert!(options.thinking);
    }
}
