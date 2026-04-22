//! 请求模型。
//!
//! 定义与 Provider 无关的公共聊天请求结构，以及单次请求可配置的附加选项。
//!
//! 设计上分成两层：
//! - `ChatRequest` 作为对外推荐的请求入口，负责承载消息、工具和链式配置；
//! - `RequestOptions` 作为请求级附加选项的底层数据结构，便于 `client` 与 `provider`
//!   之间共享同一份公共参数表示。

use serde_json::{Map, Value};

use crate::{Message, ReasoningEffort, Tool, ToolChoice};

/// 单次聊天请求的附加选项。
///
/// 该类型承载 `ChatRequest` 的请求级配置，例如采样参数、思考模式、思考预算与
/// 推理强度。普通调用方通常无需直接构造它，而是通过 `ChatRequest::new(...)`
/// 返回的请求对象链式设置。
///
/// 该类型主要用于公共请求参数在内部模块之间传递；如果只是发起一次聊天请求，优先使用
/// `ChatRequest` 的链式方法。
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    /// 采样温度。
    pub temperature: Option<f32>,
    /// nucleus sampling 参数。
    pub top_p: Option<f32>,
    /// 允许生成的最大 token 数。
    pub max_tokens: Option<u32>,
    /// presence penalty 参数。
    pub presence_penalty: Option<f32>,
    /// frequency penalty 参数。
    pub frequency_penalty: Option<f32>,
    /// 透传给特定 provider 的原生扩展参数。
    pub provider_options: Map<String, Value>,
    /// 是否启用 thinking 模式。
    pub thinking: bool,
    /// thinking 模式下允许使用的预算。
    pub thinking_budget: Option<u32>,
    /// 推理强度等级。
    pub reasoning_effort: Option<ReasoningEffort>,
    /// 工具调用选择策略。
    pub tool_choice: Option<ToolChoice>,
    /// 是否允许并行发起多个工具调用。
    pub parallel_tool_calls: Option<bool>,
}

/// 可复用的聊天请求。
///
/// 该类型可直接链式组装，并分别交给 `Client::chat` 与 `Client::chat_stream` 执行。
/// 当调用方需要配置 `tools`、`thinking` 或其他请求选项时，优先使用该入口。
///
/// 相比直接操作 `RequestOptions`，该类型还能同时统一表达消息列表、工具定义和请求级配置，
/// 更适合作为调用方长期持有或复用的请求对象。
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub tools: Option<Vec<Tool>>,
    pub options: RequestOptions,
}

impl ChatRequest {
    pub fn new(messages: impl AsRef<[Message]>) -> Self {
        Self {
            messages: messages.as_ref().to_vec(),
            tools: None,
            options: RequestOptions::default(),
        }
    }

    pub fn tools(mut self, tools: impl AsRef<[Tool]>) -> Self {
        self.tools = Some(tools.as_ref().to_vec());
        self
    }

    /// 添加供应商原生请求参数。
    ///
    /// 当键名与库内置字段冲突时，内置字段优先，透传值会被忽略。
    pub fn provider_option(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.options
            .provider_options
            .insert(key.into(), value.into());
        self
    }

    /// 批量添加供应商原生请求参数。
    ///
    /// 参数落点与 `Self::provider_option` 一致；若同一键重复出现，后写入的值会覆盖先前值。
    pub fn provider_options<I, K, V>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<serde_json::Value>,
    {
        self.options.provider_options.extend(
            options
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.options.temperature = Some(temperature);
        self
    }

    pub fn top_p(mut self, top_p: f32) -> Self {
        self.options.top_p = Some(top_p);
        self
    }

    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.options.max_tokens = Some(max_tokens);
        self
    }

    pub fn presence_penalty(mut self, presence_penalty: f32) -> Self {
        self.options.presence_penalty = Some(presence_penalty);
        self
    }

    pub fn frequency_penalty(mut self, frequency_penalty: f32) -> Self {
        self.options.frequency_penalty = Some(frequency_penalty);
        self
    }

    pub fn thinking(mut self, enabled: bool) -> Self {
        self.options.thinking = enabled;
        self
    }

    pub fn thinking_budget(mut self, budget: u32) -> Self {
        self.options.thinking_budget = Some(budget);
        self
    }

    pub fn reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.options.reasoning_effort = Some(effort);
        self
    }

    pub fn tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.options.tool_choice = Some(tool_choice);
        self
    }

    pub fn parallel_tool_calls(mut self, enabled: bool) -> Self {
        self.options.parallel_tool_calls = Some(enabled);
        self
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ChatRequest, RequestOptions};
    use crate::{JsonType, Message, ReasoningEffort, Tool, ToolChoice};

    #[test]
    fn request_options_default_is_empty() {
        let options = RequestOptions::default();

        assert_eq!(options.temperature, None);
        assert_eq!(options.top_p, None);
        assert_eq!(options.max_tokens, None);
        assert_eq!(options.presence_penalty, None);
        assert_eq!(options.frequency_penalty, None);
        assert!(options.provider_options.is_empty());
        assert!(!options.thinking);
        assert_eq!(options.thinking_budget, None);
        assert_eq!(options.reasoning_effort, None);
        assert_eq!(options.tool_choice, None);
        assert_eq!(options.parallel_tool_calls, None);
    }

    #[test]
    fn chat_request_owns_messages_tools_and_options() {
        let messages = vec![Message::user("hello")];
        let tools = [Tool::function("get_weather")
            .param("city", JsonType::String, "城市名称", true)
            .build()];

        let request = ChatRequest::new(&messages)
            .tools(&tools)
            .provider_option("max_completion_tokens", 4096)
            .provider_option("metadata", json!({ "tier": "pro" }))
            .temperature(0.7)
            .top_p(0.9)
            .max_tokens(2048)
            .presence_penalty(0.3)
            .frequency_penalty(0.1)
            .thinking(true)
            .thinking_budget(8_000)
            .reasoning_effort(ReasoningEffort::High)
            .tool_choice(ToolChoice::function("get_weather"))
            .parallel_tool_calls(true);

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.tools.as_ref().map(Vec::len), Some(1));
        assert_eq!(
            request.options.provider_options.get("max_completion_tokens"),
            Some(&json!(4096))
        );
        assert_eq!(
            request.options.provider_options.get("metadata"),
            Some(&json!({ "tier": "pro" }))
        );
        assert_eq!(request.options.temperature, Some(0.7));
        assert_eq!(request.options.top_p, Some(0.9));
        assert_eq!(request.options.max_tokens, Some(2048));
        assert_eq!(request.options.presence_penalty, Some(0.3));
        assert_eq!(request.options.frequency_penalty, Some(0.1));
        assert!(request.options.thinking);
        assert_eq!(request.options.thinking_budget, Some(8_000));
        assert_eq!(request.options.reasoning_effort, Some(ReasoningEffort::High));
        assert_eq!(request.options.parallel_tool_calls, Some(true));
        assert_eq!(
            request
                .options
                .tool_choice
                .as_ref()
                .and_then(ToolChoice::function_name),
            Some("get_weather")
        );
    }

    #[test]
    fn chat_request_accepts_provider_options_batch() {
        let request = ChatRequest::new([Message::user("hello")])
            .provider_option("seed", 7)
            .provider_options([
                ("seed", json!(8)),
                ("metadata", json!({ "tier": "pro" })),
                ("max_completion_tokens", json!(2048)),
            ]);

        assert_eq!(request.options.provider_options.get("seed"), Some(&json!(8)));
        assert_eq!(
            request.options.provider_options.get("metadata"),
            Some(&json!({ "tier": "pro" }))
        );
        assert_eq!(
            request.options.provider_options.get("max_completion_tokens"),
            Some(&json!(2048))
        );
    }
}
