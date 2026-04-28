use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use crate::{
    error::LlmError,
    middleware::Transport,
    types::{request::ChatRequest, response::ChatChunk, response::ChatResponse},
};

use super::ProviderAdapter;

pub(crate) struct GeminiAdapter {
    _transport: Transport,
    _api_key: String,
    _base_url: String,
}

impl GeminiAdapter {
    fn new(api_key: &str, base_url: &str, transport: Transport) -> Self {
        Self {
            _transport: transport,
            _api_key: api_key.to_owned(),
            _base_url: base_url.to_owned(),
        }
    }
}

#[async_trait]
impl ProviderAdapter for GeminiAdapter {
    fn name(&self) -> &'static str {
        "gemini"
    }

    async fn chat(&self, _model: &str, _req: ChatRequest) -> Result<ChatResponse, LlmError> {
        Err(LlmError::UnsupportedCapability {
            provider: Some(self.name().into()),
            capability: "chat".into(),
        })
    }

    async fn chat_stream(
        &self,
        _model: &str,
        _req: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, LlmError>> + Send>>, LlmError> {
        Err(LlmError::UnsupportedCapability {
            provider: Some(self.name().into()),
            capability: "chat_stream".into(),
        })
    }
}

pub(crate) fn build(
    api_key: &str,
    base_url: &str,
    transport: &Transport,
) -> Result<Box<dyn ProviderAdapter>, LlmError> {
    Ok(Box::new(GeminiAdapter::new(
        api_key,
        base_url,
        transport.clone(),
    )))
}
