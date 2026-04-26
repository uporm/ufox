use reqwest::multipart::{Form, Part};

use crate::{
    error::LlmError,
    types::{
        request::{SpeechToTextRequest, TextToSpeechRequest},
        response::{SpeechToTextResponse, TextToSpeechResponse},
    },
};

use super::OpenAiAdapter;

impl OpenAiAdapter {
    pub(super) async fn execute_speech_to_text(
        &self,
        model: &str,
        req: SpeechToTextRequest,
    ) -> Result<SpeechToTextResponse, LlmError> {
        let fallback_filename = format!("audio.{}", Self::audio_extension(req.format));
        let (bytes, mime_type, filename) = self
            .resolve_media_source_bytes(
                &req.source,
                &fallback_filename,
                Some(Self::audio_mime(req.format)),
            )
            .await?;

        let part = Part::bytes(bytes)
            .file_name(filename)
            .mime_str(&mime_type)
            .map_err(|err| LlmError::MediaInput {
                message: format!("音频 MIME 不合法: {err}"),
            })?;
        let mut form = Form::new().part("file", part).text("model", model.to_owned());
        if let Some(language) = req.language {
            form = form.text("language", language);
        }
        for (key, value) in req.extensions {
            form = form.text(
                key,
                value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()),
            );
        }

        let response = self
            .transport
            .send(self.request_multipart("/audio/transcriptions").multipart(form))
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await?;
            return Err(self.map_error_response(status, &body_text));
        }

        let raw: serde_json::Value = response.json().await?;
        Ok(SpeechToTextResponse {
            text: raw
                .get("text")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_owned(),
            language: raw
                .get("language")
                .and_then(|value| value.as_str())
                .map(str::to_owned),
            duration_secs: raw
                .get("duration")
                .or_else(|| raw.get("duration_secs"))
                .and_then(|value| value.as_f64())
                .map(|value| value as f32),
            usage: Self::parse_usage(raw.get("usage")),
        })
    }

    pub(super) async fn execute_text_to_speech(
        &self,
        model: &str,
        req: TextToSpeechRequest,
    ) -> Result<TextToSpeechResponse, LlmError> {
        let mut body = serde_json::Map::new();
        body.insert("model".into(), serde_json::Value::String(model.to_owned()));
        body.insert("input".into(), serde_json::Value::String(req.text));
        body.insert(
            "voice".into(),
            serde_json::Value::String(req.voice.unwrap_or_else(|| "alloy".into())),
        );
        body.insert(
            "response_format".into(),
            serde_json::Value::String(Self::audio_extension(req.output_format).into()),
        );
        for (key, value) in req.extensions {
            body.insert(key, value);
        }

        let response = self
            .transport
            .send(self.request_json("/audio/speech").json(&serde_json::Value::Object(body)))
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await?;
            return Err(self.map_error_response(status, &body_text));
        }

        Ok(TextToSpeechResponse {
            audio_data: response
                .bytes()
                .await
                .map_err(|err| LlmError::transport("读取音频响应", err))?,
            format: req.output_format,
            duration_secs: None,
        })
    }
}
