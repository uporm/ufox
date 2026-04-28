use std::pin::Pin;

use bytes::Bytes;
use futures::{Stream, StreamExt};

use crate::{
    error::LlmError,
    types::{
        request::VideoGenRequest,
        response::{TaskStatus, VideoGenResponse},
    },
};

use super::http::{OpenAiRequestBuilder, send_json_request, send_request};

fn parse_task_status(provider_name: &str, raw: &serde_json::Value) -> Result<TaskStatus, LlmError> {
    match raw.get("status").and_then(|value| value.as_str()) {
        Some("queued") => Ok(TaskStatus::Pending),
        Some("in_progress") | Some("processing") => Ok(TaskStatus::Processing),
        Some("completed") => Ok(TaskStatus::Succeeded),
        Some("failed") => Ok(TaskStatus::Failed),
        Some(status) => Err(LlmError::ProviderResponse {
            provider: provider_name.into(),
            code: None,
            message: format!("视频任务状态不受支持: {status}"),
        }),
        None => Err(LlmError::ProviderResponse {
            provider: provider_name.into(),
            code: None,
            message: "视频任务响应缺少 status".into(),
        }),
    }
}

fn parse_video_response<A: OpenAiRequestBuilder>(
    adapter: &A,
    raw: serde_json::Value,
) -> Result<VideoGenResponse, LlmError> {
    let task_id = raw
        .get("id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| LlmError::ProviderResponse {
            provider: adapter.provider_name().into(),
            code: None,
            message: "视频任务响应缺少 id".into(),
        })?
        .to_owned();
    let status = parse_task_status(adapter.provider_name(), &raw)?;
    let url = match status {
        // OpenAI 完成后通过 `/videos/{id}/content` 下载二进制内容，这里返回可轮询后使用的资源地址。
        TaskStatus::Succeeded => Some(format!(
            "{}/videos/{}/content",
            adapter.base_url(),
            task_id
        )),
        TaskStatus::Pending | TaskStatus::Processing | TaskStatus::Failed => None,
    };

    Ok(VideoGenResponse {
        task_id,
        status,
        url,
    })
}

pub(super) async fn execute_generate_video<A: OpenAiRequestBuilder>(
    adapter: &A,
    model: &str,
    req: VideoGenRequest,
) -> Result<VideoGenResponse, LlmError> {
    let mut body = serde_json::Map::new();
    body.insert("model".into(), serde_json::Value::String(model.to_owned()));
    body.insert("prompt".into(), serde_json::Value::String(req.prompt));
    if let Some(duration_secs) = req.duration_secs {
        body.insert("seconds".into(), serde_json::Value::String(duration_secs.to_string()));
    }
    if let Some(output_format) = req.output_format {
        body.insert(
            "format".into(),
            serde_json::to_value(output_format).map_err(|err| LlmError::ProviderResponse {
                provider: adapter.provider_name().into(),
                code: None,
                message: format!("序列化视频输出格式失败: {err}"),
            })?,
        );
    }
    for (key, value) in req.extensions {
        body.insert(key, value);
    }

    let raw = send_json_request(
        adapter,
        adapter.post_json("/videos").json(&serde_json::Value::Object(body)),
    )
    .await?;
    parse_video_response(adapter, raw)
}

pub(super) async fn execute_poll_video_task<A: OpenAiRequestBuilder>(
    adapter: &A,
    task_id: &str,
) -> Result<VideoGenResponse, LlmError> {
    let raw = send_json_request(adapter, adapter.get(&format!("/videos/{task_id}"))).await?;
    parse_video_response(adapter, raw)
}

pub(super) async fn execute_download_video_stream<A: OpenAiRequestBuilder>(
    adapter: &A,
    task_id: &str,
) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, LlmError>> + Send>>, LlmError> {
    let response = send_request(adapter, adapter.get(&format!("/videos/{task_id}/content"))).await?;
    Ok(Box::pin(response.bytes_stream().map(|chunk| {
        chunk.map_err(|err| LlmError::transport("读取视频下载流", err))
    })))
}
