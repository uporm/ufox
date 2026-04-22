//! `ufox-llm` 库入口。
//!
//! 统一导出根级公共 API，并将具体实现模块保留在 crate 内部。
//!
//! 推荐用法：
//!
//! - 默认请求选项：优先使用 `Client::chat_messages(...)` 或 `Client::chat_stream_messages(...)`
//! - 需要配置 `tools`、`thinking` 或其他请求选项：使用 `ChatRequest::new(...)` 链式组装后，再调用
//!   `Client::chat(...)` 或 `Client::chat_stream(...)`
//! - `RequestOptions` 作为公共请求选项载体对外暴露；普通调用方通常无需直接构造它
//!
//! 基础聊天：
//!
//! ```no_run
//! use ufox_llm::{Client, Message, Provider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), ufox_llm::LlmError> {
//!     let client = Client::builder()
//!         .provider(Provider::OpenAI)
//!         .api_key("sk-xxx")
//!         .model("gpt-4o")
//!         .build()?;
//!
//!     let messages = vec![Message::user("请用一句话介绍 Rust。")];
//!     let response = client.chat_messages(&messages).await?;
//!     println!("{}", response.content);
//!     Ok(())
//! }
//! ```
//!
//! 请求级配置：
//!
//! `Qwen` Provider 使用 OpenAI-compatible `Chat Completions` 协议；当模型支持思考模式时，
//! 可通过 `thinking(true)` 与 `thinking_budget(...)` 传递对应扩展参数。
//!
//! ```no_run
//! use ufox_llm::{ChatRequest, Client, Message, Provider};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), ufox_llm::LlmError> {
//!     let client = Client::builder()
//!         .provider(Provider::Qwen)
//!         .api_key("sk-xxx")
//!         .model("qwen3-max")
//!         .build()?;
//!
//!     let messages = vec![Message::user("请分析这道题并给出结论。")];
//!     let request = ChatRequest::new(&messages).thinking(true).thinking_budget(8_000);
//!     let response = client.chat(&request).await?;
//!     println!("{}", response.content);
//!     Ok(())
//! }
//! ```

mod client;
mod error;
mod provider;
mod types;

pub use client::{ChatStream, Client, ClientBuilder};
pub use error::LlmError;
pub use provider::compatible::{CompatibleAdapter, CompatibleStreamParser};
pub use provider::openai::{OpenAiAdapter, OpenAiStreamParser};
pub use provider::qwen::{QwenAdapter, QwenStreamParser};
pub use provider::{Provider, ProviderAdapter};
pub use types::{
    AudioFile, AudioSource, ChatRequest, ChatResponse, Content, ContentPart, DeltaKind, DeltaType,
    FinishReason, ImageFile, ImageSource, JsonType, Message, MessageBuilder, ReasoningEffort,
    RequestOptions, Role, StreamChunk, Tool, ToolBuilder, ToolCall, ToolChoice, ToolParameter,
    ToolResult, Usage, VideoFile, VideoSource,
};
