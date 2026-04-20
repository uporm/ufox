//! `ufox-llm` 库入口。
//!
//! 统一导出 `client`、`provider`、`types` 与 `error` 的公共 API。

pub mod error;
pub mod client;
pub mod provider;
pub mod types;

pub use client::{ChatRequestBuilder, ChatStream, ChatStreamRequestBuilder, Client};
pub use client::builder::{
    ApiKeySet, ApiKeyUnset, ClientBuilder, ClientConfig, CompatibleConfig, OpenAiConfig,
    ProviderConfig, ProviderSet, ProviderUnset, QwenConfig,
};
pub use error::LlmError;
pub use provider::{Provider, ProviderAdapter};
pub use provider::compatible::{CompatibleAdapter, CompatibleStreamParser};
pub use provider::openai::{OpenAiAdapter, OpenAiStreamParser};
pub use provider::qwen::{QwenAdapter, QwenStreamParser};
pub use types::{
    ChatResponse, Content, ContentPart, DeltaType, FinishReason, ImageFile, ImageSource,
    JsonType, Message, MessageBuilder, ReasoningEffort, Role, StreamChunk, Tool, ToolBuilder,
    ToolCall, ToolChoice, ToolKind, ToolParameter, ToolResult, Usage,
};
