pub use error::LlmError;
pub use types::content::{
    Audio,
    AudioFormat,
    ContentPart,
    Image,
    ImageFidelity,
    MediaSource,
    Message,
    Role,
    Text,
    Tool,
    ToolCall,
    ToolChoice,
    ToolResult,
    ToolResultPayload,
    Video,
    VideoFormat,
};
pub use types::request::{
    ChatRequest,
    ChatRequestBuilder,
    EmbeddingRequest,
    ImageGenRequest,
    SpeechToTextRequest,
    TextToSpeechRequest,
    VideoGenRequest,
};
pub use types::response::{
    ChatChunk,
    ChatResponse,
    EmbeddingResponse,
    FinishReason,
    GeneratedImage,
    ImageGenResponse,
    SpeechToTextResponse,
    TaskStatus,
    TextToSpeechResponse,
    Usage,
    VideoGenResponse,
};
pub use client::{Client, ClientBuilder};
pub use provider::Provider;

mod error;
mod types;
mod provider;
mod middleware;
mod client;
