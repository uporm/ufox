mod audio;
mod chat;
mod embedding;
mod image;
mod media;
mod stream;

#[cfg(test)]
mod tests;

/// OpenAI Chat Completions 接口路径。
const CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use crate::{
    error::LlmError,
    middleware::Transport,
    types::{
        request::{
            ChatRequest, EmbeddingRequest, ImageGenRequest, SpeechToTextRequest,
            TextToSpeechRequest, VideoGenRequest,
        },
        response::{
            ChatChunk, ChatResponse, EmbeddingResponse, FinishReason, ImageGenResponse,
            SpeechToTextResponse, TextToSpeechResponse, Usage, VideoGenResponse,
        },
    },
};

use super::ProviderAdapter;

pub(super) type ChatChunkStream = Pin<Box<dyn Stream<Item = Result<ChatChunk, LlmError>> + Send>>;

/// OpenAI 兼容协议的适配器实现。
pub(crate) struct OpenAiAdapter {
    transport: Transport,
    api_key: String,
    base_url: String,
    provider_name: &'static str,
}

impl OpenAiAdapter {
    fn new(
        provider_name: &'static str,
        api_key: &str,
        base_url: &str,
        transport: Transport,
    ) -> Self {
        Self {
            transport,
            api_key: api_key.to_owned(),
            base_url: base_url.trim_end_matches('/').to_owned(),
            provider_name,
        }
    }

    fn name(&self) -> &'static str {
        self.provider_name
    }

    fn request_json(&self, path: &str) -> reqwest::RequestBuilder {
        self.transport
            .client()
            .post(format!("{}/{}", self.base_url, path.trim_start_matches('/')))
            .bearer_auth(&self.api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
    }

    fn request_multipart(&self, path: &str) -> reqwest::RequestBuilder {
        self.transport
            .client()
            .post(format!("{}/{}", self.base_url, path.trim_start_matches('/')))
            .bearer_auth(&self.api_key)
    }

    fn map_error_response(&self, status: u16, body_text: &str) -> LlmError {
        match status {
            401 | 403 => LlmError::Authentication {
                message: format!("[{}] {}", self.name(), body_text),
            },
            429 => LlmError::RateLimit {
                retry_after_secs: None,
            },
            _ => LlmError::HttpStatus {
                provider: self.name().into(),
                status,
                body: body_text.to_owned(),
            },
        }
    }

    fn parse_finish_reason(raw: Option<&str>) -> Option<FinishReason> {
        match raw {
            Some("stop") => Some(FinishReason::Stop),
            Some("length") => Some(FinishReason::Length),
            Some("tool_calls") => Some(FinishReason::ToolCalls),
            Some("content_filter") => Some(FinishReason::ContentFilter),
            Some(_) => Some(FinishReason::Other),
            None => None,
        }
    }

    fn parse_usage(raw: Option<&serde_json::Value>) -> Option<Usage> {
        let raw = raw?;
        Some(Usage {
            prompt_tokens: Self::parse_usage_tokens(raw.get("prompt_tokens"))?,
            completion_tokens: Self::parse_usage_tokens(raw.get("completion_tokens"))?,
            total_tokens: Self::parse_usage_tokens(raw.get("total_tokens"))?,
        })
    }

    fn parse_usage_tokens(raw: Option<&serde_json::Value>) -> Option<u32> {
        raw?.as_u64()?.try_into().ok()
    }

    fn stream_read_timeout_error(read_timeout_ms: u64) -> LlmError {
        LlmError::request_timeout(
            "读取流式响应",
            read_timeout_ms,
            "流式连接已建立，但在读取超时窗口内未收到新数据；可尝试增大 read_timeout_secs，或检查 provider 是否持续输出分片",
        )
    }

    fn map_stream_read_error(read_timeout_ms: u64, err: reqwest::Error) -> LlmError {
        if err.is_timeout() {
            Self::stream_read_timeout_error(read_timeout_ms)
        } else {
            LlmError::transport("读取流式响应", err)
        }
    }
}

#[async_trait]
impl ProviderAdapter for OpenAiAdapter {
    fn name(&self) -> &'static str {
        self.provider_name
    }

    async fn chat(&self, model: &str, req: ChatRequest) -> Result<ChatResponse, LlmError> {
        self.execute_chat(model, req).await
    }

    async fn chat_stream(
        &self,
        model: &str,
        req: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, LlmError>> + Send>>, LlmError> {
        self.execute_chat_stream(model, req).await
    }

    async fn embed(
        &self,
        model: &str,
        req: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, LlmError> {
        self.execute_embed(model, req).await
    }

    async fn speech_to_text(
        &self,
        model: &str,
        req: SpeechToTextRequest,
    ) -> Result<SpeechToTextResponse, LlmError> {
        self.execute_speech_to_text(model, req).await
    }

    async fn text_to_speech(
        &self,
        model: &str,
        req: TextToSpeechRequest,
    ) -> Result<TextToSpeechResponse, LlmError> {
        self.execute_text_to_speech(model, req).await
    }

    async fn generate_image(
        &self,
        model: &str,
        req: ImageGenRequest,
    ) -> Result<ImageGenResponse, LlmError> {
        self.execute_generate_image(model, req).await
    }

    async fn generate_video(
        &self,
        model: &str,
        req: VideoGenRequest,
    ) -> Result<VideoGenResponse, LlmError> {
        self.execute_generate_video(model, req).await
    }
}

/// 构造 OpenAI 兼容 provider adapter。
pub(crate) fn build(
    provider_name: &'static str,
    api_key: &str,
    base_url: &str,
    transport: &Transport,
) -> Result<Box<dyn ProviderAdapter>, LlmError> {
    Ok(Box::new(OpenAiAdapter::new(
        provider_name,
        api_key,
        base_url,
        transport.clone(),
    )))
}

#[cfg(test)]
mod unit_tests {
    use super::OpenAiAdapter;

    #[test]
    fn parse_usage_accepts_u32_range_values() {
        let usage = OpenAiAdapter::parse_usage(Some(&serde_json::json!({
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30,
            })))
        .unwrap();

        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn parse_usage_rejects_values_larger_than_u32() {
        assert!(
            OpenAiAdapter::parse_usage(Some(&serde_json::json!({
                "prompt_tokens": u64::from(u32::MAX) + 1,
                "completion_tokens": 20,
                "total_tokens": 30,
            })))
            .is_none()
        );
    }
}
