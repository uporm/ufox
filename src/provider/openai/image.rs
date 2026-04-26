use crate::{
    error::LlmError,
    types::{
        request::{ImageGenRequest, VideoGenRequest},
        response::{ImageGenResponse, VideoGenResponse},
    },
};

use super::OpenAiAdapter;

impl OpenAiAdapter {
    pub(super) async fn execute_generate_image(
        &self,
        model: &str,
        req: ImageGenRequest,
    ) -> Result<ImageGenResponse, LlmError> {
        let mut body = serde_json::Map::new();
        body.insert("model".into(), serde_json::Value::String(model.to_owned()));
        body.insert("prompt".into(), serde_json::Value::String(req.prompt));
        if let Some(n) = req.n {
            body.insert("n".into(), serde_json::json!(n));
        }
        if let Some(size) = req.size {
            body.insert("size".into(), serde_json::Value::String(size));
        }
        for (key, value) in req.extensions {
            body.insert(key, value);
        }

        let response = self
            .transport
            .send(
                self.request_json("/images/generations")
                    .json(&serde_json::Value::Object(body)),
            )
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await?;
            return Err(self.map_error_response(status, &body_text));
        }

        let raw: serde_json::Value = response.json().await?;
        let images = raw
            .get("data")
            .and_then(|value| value.as_array())
            .ok_or_else(|| LlmError::ProviderResponse {
                provider: self.name().into(),
                code: None,
                message: "图片生成响应缺少 data".into(),
            })?
            .iter()
            .map(|item| crate::types::response::GeneratedImage {
                url: item.get("url").and_then(|value| value.as_str()).map(str::to_owned),
                base64: item
                    .get("b64_json")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                revised_prompt: item
                    .get("revised_prompt")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
            })
            .collect();

        Ok(ImageGenResponse {
            images,
            usage: Self::parse_usage(raw.get("usage")),
        })
    }

    pub(super) async fn execute_generate_video(
        &self,
        _model: &str,
        _req: VideoGenRequest,
    ) -> Result<VideoGenResponse, LlmError> {
        Err(LlmError::UnsupportedCapability {
            provider: Some(self.name().into()),
            capability: "generate_video".into(),
        })
    }
}
