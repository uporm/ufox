use base64::Engine as _;

use crate::{
    error::LlmError,
    types::content::{AudioFormat, MediaSource},
};

use super::OpenAiAdapter;

impl OpenAiAdapter {
    pub(super) async fn resolve_media_source_to_image_url(
        source: &MediaSource,
    ) -> Result<serde_json::Value, LlmError> {
        match source {
            MediaSource::Url { url } => Ok(serde_json::json!({ "url": url })),
            MediaSource::Base64 { data, mime_type } => Ok(serde_json::json!({
                "url": format!("data:{mime_type};base64,{data}")
            })),
            MediaSource::File { path } => {
                let data = tokio::fs::read(path).await.map_err(|err| LlmError::MediaInput {
                    message: format!("读取文件失败 {:?}: {}", path, err),
                })?;
                let mime_type = mime_guess::from_path(path)
                    .first_raw()
                    .unwrap_or("application/octet-stream");
                let data =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
                Ok(serde_json::json!({
                    "url": format!("data:{mime_type};base64,{data}")
                }))
            }
        }
    }

    pub(super) fn audio_mime(format: AudioFormat) -> &'static str {
        match format {
            AudioFormat::Mp3 => "audio/mpeg",
            AudioFormat::Wav => "audio/wav",
            AudioFormat::Flac => "audio/flac",
            AudioFormat::Opus => "audio/ogg",
            AudioFormat::Aac => "audio/aac",
            AudioFormat::Pcm => "audio/pcm",
        }
    }

    pub(super) fn audio_extension(format: AudioFormat) -> &'static str {
        match format {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Wav => "wav",
            AudioFormat::Flac => "flac",
            AudioFormat::Opus => "opus",
            AudioFormat::Aac => "aac",
            AudioFormat::Pcm => "pcm",
        }
    }

    pub(super) async fn resolve_media_source_bytes(
        &self,
        source: &MediaSource,
        fallback_filename: &str,
        default_mime: Option<&str>,
    ) -> Result<(Vec<u8>, String, String), LlmError> {
        match source {
            MediaSource::Base64 { data, mime_type } => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .map_err(|err| LlmError::MediaInput {
                        message: format!("base64 解码失败: {err}"),
                    })?;
                Ok((bytes, mime_type.clone(), fallback_filename.to_owned()))
            }
            MediaSource::Url { url } => {
                let response = self
                    .transport
                    .client()
                    .get(url)
                    .send()
                    .await
                    .map_err(|err| LlmError::transport("下载媒体资源", err))?;
                if !response.status().is_success() {
                    return Err(LlmError::MediaInput {
                        message: format!(
                            "下载媒体资源失败：status={} url={url}",
                            response.status().as_u16()
                        ),
                    });
                }
                let mime_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
                    .or_else(|| default_mime.map(str::to_owned))
                    .unwrap_or_else(|| "application/octet-stream".to_owned());
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|err| LlmError::transport("读取媒体响应", err))?
                    .to_vec();
                let filename = url
                    .split('/')
                    .next_back()
                    .filter(|segment| !segment.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| fallback_filename.to_owned());
                Ok((bytes, mime_type, filename))
            }
            MediaSource::File { path } => {
                let bytes = tokio::fs::read(path).await.map_err(|err| LlmError::MediaInput {
                    message: format!("读取文件失败 {:?}: {}", path, err),
                })?;
                let mime_type = mime_guess::from_path(path)
                    .first_raw()
                    .or(default_mime)
                    .unwrap_or("application/octet-stream")
                    .to_owned();
                let filename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| fallback_filename.to_owned());
                Ok((bytes, mime_type, filename))
            }
        }
    }
}
