use std::{
    collections::{BTreeMap, VecDeque},
    pin::Pin,
};

use futures::{stream, Stream, StreamExt};

use crate::{
    error::LlmError,
    types::{
        content::ToolCall,
        request::ChatRequest,
        response::{ChatChunk, FinishReason},
    },
};

use super::{ChatChunkStream, OpenAiAdapter, CHAT_COMPLETIONS_PATH};

#[derive(Default)]
struct PartialToolCall {
    id: Option<String>,
    tool_name: Option<String>,
    arguments: String,
    arguments_seen: bool,
}

impl PartialToolCall {
    fn finalize(self) -> Result<ToolCall, LlmError> {
        let id = self.id.ok_or_else(|| LlmError::ToolProtocol {
            message: "stream tool call 缺少 id".into(),
        })?;
        let tool_name = self.tool_name.ok_or_else(|| LlmError::ToolProtocol {
            message: "stream tool call 缺少 name".into(),
        })?;
        let arguments_raw = if self.arguments_seen {
            self.arguments.as_str()
        } else {
            return Err(LlmError::ToolProtocol {
                message: "stream tool call 缺少 arguments".into(),
            });
        };
        let arguments = serde_json::from_str(arguments_raw).map_err(|err| LlmError::ToolProtocol {
            message: format!("stream tool arguments 解析失败: {err}"),
        })?;

        Ok(ToolCall {
            id,
            tool_name,
            arguments,
        })
    }
}

struct StreamState {
    source: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: Vec<u8>,
    pending: VecDeque<Result<ChatChunk, LlmError>>,
    tool_calls: BTreeMap<usize, PartialToolCall>,
    done: bool,
}

impl OpenAiAdapter {
    fn take_sse_event(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
        for index in 0..buffer.len() {
            if buffer[index..].starts_with(b"\r\n\r\n") {
                return Some(buffer.drain(..index + 4).collect());
            }
            if buffer[index..].starts_with(b"\n\n") {
                return Some(buffer.drain(..index + 2).collect());
            }
        }
        None
    }

    fn parse_sse_data(event: &[u8], provider_name: &str) -> Result<Option<String>, LlmError> {
        let raw = String::from_utf8(event.to_vec()).map_err(|err| LlmError::StreamProtocol {
            provider: provider_name.to_owned(),
            message: format!("SSE 数据不是合法 UTF-8: {err}"),
        })?;
        let mut data_lines = Vec::new();
        for line in raw.replace("\r\n", "\n").lines() {
            if let Some(data) = line.strip_prefix("data:") {
                data_lines.push(data.trim_start().to_owned());
            }
        }
        if data_lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(data_lines.join("\n")))
    }

    fn drain_partial_tool_calls(
        partials: &mut BTreeMap<usize, PartialToolCall>,
    ) -> Result<Vec<ToolCall>, LlmError> {
        let mut calls = Vec::with_capacity(partials.len());
        for (_, partial) in std::mem::take(partials) {
            calls.push(partial.finalize()?);
        }
        Ok(calls)
    }

    fn parse_stream_event(
        provider_name: &str,
        raw: &serde_json::Value,
        partials: &mut BTreeMap<usize, PartialToolCall>,
    ) -> Result<Vec<ChatChunk>, LlmError> {
        let choice = raw
            .get("choices")
            .and_then(|value| value.as_array())
            .and_then(|choices| choices.first())
            .ok_or_else(|| LlmError::StreamProtocol {
                provider: provider_name.to_owned(),
                message: "缺少 choices[0]".into(),
            })?;

        let mut chunks = Vec::new();
        let delta = choice.get("delta").and_then(|value| value.as_object());
        if let Some(content) = delta
            .and_then(|value| value.get("content"))
            .and_then(|value| value.as_str())
            .filter(|content| !content.is_empty())
        {
            chunks.push(ChatChunk {
                text_delta: Some(content.to_owned()),
                ..ChatChunk::default()
            });
        }

        if let Some(items) = delta
            .and_then(|value| value.get("tool_calls"))
            .and_then(|value| value.as_array())
        {
            for item in items {
                let index = item
                    .get("index")
                    .and_then(|value| value.as_u64())
                    .ok_or_else(|| LlmError::ToolProtocol {
                        message: "stream tool call 缺少 index".into(),
                    })? as usize;
                let entry = partials.entry(index).or_default();
                if let Some(id) = item.get("id").and_then(|value| value.as_str()) {
                    entry.id = Some(id.to_owned());
                }
                if let Some(function) = item.get("function").and_then(|value| value.as_object()) {
                    if let Some(name) = function.get("name").and_then(|value| value.as_str()) {
                        match &mut entry.tool_name {
                            Some(existing) => existing.push_str(name),
                            None => entry.tool_name = Some(name.to_owned()),
                        }
                    }
                    if let Some(arguments) = function.get("arguments") {
                        let arguments = arguments.as_str().ok_or_else(|| LlmError::ToolProtocol {
                            message: "stream tool call arguments 不是字符串".into(),
                        })?;
                        entry.arguments_seen = true;
                        entry.arguments.push_str(arguments);
                    }
                }
            }
        }

        let finish_reason =
            Self::parse_finish_reason(choice.get("finish_reason").and_then(|value| value.as_str()));
        let usage = Self::parse_usage(raw.get("usage"));

        if matches!(finish_reason, Some(FinishReason::ToolCalls)) {
            chunks.push(ChatChunk {
                tool_calls: Self::drain_partial_tool_calls(partials)?,
                finish_reason,
                usage,
                ..ChatChunk::default()
            });
        } else if finish_reason.is_some() || usage.is_some() {
            if let Some(last) = chunks.last_mut() {
                if last.finish_reason.is_none() && last.usage.is_none() {
                    last.finish_reason = finish_reason;
                    last.usage = usage;
                } else {
                    chunks.push(ChatChunk {
                        finish_reason,
                        usage,
                        ..ChatChunk::default()
                    });
                }
            } else {
                chunks.push(ChatChunk {
                    finish_reason,
                    usage,
                    ..ChatChunk::default()
                });
            }
        }

        Ok(chunks)
    }

    pub(super) async fn execute_chat_stream(
        &self,
        model: &str,
        req: ChatRequest,
    ) -> Result<ChatChunkStream, LlmError> {
        let body = self.to_request_body(model, &req, true).await?;
        let response = self
            .transport
            .send(self.request_json(CHAT_COMPLETIONS_PATH).json(&body))
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await?;
            return Err(self.map_error_response(status, &body_text));
        }

        let provider_name = self.name();
        let read_timeout_ms = self.transport.read_timeout_ms();
        let stream = stream::unfold(
            StreamState {
                source: Box::pin(response.bytes_stream()),
                buffer: Vec::new(),
                pending: VecDeque::new(),
                tool_calls: BTreeMap::new(),
                done: false,
            },
            move |mut state| async move {
                loop {
                    if let Some(item) = state.pending.pop_front() {
                        return Some((item, state));
                    }
                    if state.done {
                        return None;
                    }

                    match state.source.next().await {
                        Some(Ok(bytes)) => {
                            state.buffer.extend_from_slice(&bytes);
                            while let Some(event) = OpenAiAdapter::take_sse_event(&mut state.buffer) {
                                match OpenAiAdapter::parse_sse_data(&event, provider_name) {
                                    Ok(Some(data)) if data == "[DONE]" => {
                                        state.done = true;
                                        match OpenAiAdapter::drain_partial_tool_calls(
                                            &mut state.tool_calls,
                                        ) {
                                            Ok(tool_calls) if !tool_calls.is_empty() => {
                                                state.pending.push_back(Ok(ChatChunk {
                                                    tool_calls,
                                                    ..ChatChunk::default()
                                                }));
                                            }
                                            Ok(_) => {}
                                            Err(err) => state.pending.push_back(Err(err)),
                                        }
                                        break;
                                    }
                                    Ok(Some(data)) => {
                                        match serde_json::from_str::<serde_json::Value>(&data) {
                                            Ok(raw) => match OpenAiAdapter::parse_stream_event(
                                                provider_name,
                                                &raw,
                                                &mut state.tool_calls,
                                            ) {
                                                Ok(chunks) => {
                                                    state.pending.extend(chunks.into_iter().map(Ok));
                                                }
                                                Err(err) => {
                                                    state.done = true;
                                                    state.pending.push_back(Err(err));
                                                    break;
                                                }
                                            },
                                            Err(err) => {
                                                state.done = true;
                                                state.pending.push_back(Err(
                                                    LlmError::StreamProtocol {
                                                        provider: provider_name.to_owned(),
                                                        message: format!(
                                                            "stream json 解析失败: {err}"
                                                        ),
                                                    },
                                                ));
                                                break;
                                            }
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(err) => {
                                        state.done = true;
                                        state.pending.push_back(Err(err));
                                        break;
                                    }
                                }
                            }
                        }
                        Some(Err(err)) => {
                            state.done = true;
                            return Some((
                                Err(OpenAiAdapter::map_stream_read_error(read_timeout_ms, err)),
                                state,
                            ));
                        }
                        None => {
                            state.done = true;
                            match OpenAiAdapter::drain_partial_tool_calls(&mut state.tool_calls) {
                                Ok(tool_calls) if !tool_calls.is_empty() => {
                                    return Some((
                                        Ok(ChatChunk {
                                            tool_calls,
                                            ..ChatChunk::default()
                                        }),
                                        state,
                                    ));
                                }
                                Ok(_) => return None,
                                Err(err) => return Some((Err(err), state)),
                            }
                        }
                    }
                }
            },
        );

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{error::LlmError, types::response::FinishReason};

    use super::{OpenAiAdapter, PartialToolCall};

    #[test]
    fn partial_tool_call_rejects_missing_arguments() {
        let error = PartialToolCall {
            id: Some("call_1".into()),
            tool_name: Some("get_weather".into()),
            arguments: String::new(),
            arguments_seen: false,
        }
        .finalize()
        .unwrap_err();

        assert!(matches!(
            error,
            LlmError::ToolProtocol { ref message } if message == "stream tool call 缺少 arguments"
        ));
    }

    #[test]
    fn parse_stream_event_rejects_non_string_arguments() {
        let error = OpenAiAdapter::parse_stream_event(
            "compatible",
            &serde_json::json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "function": {
                                "name": "get_weather",
                                "arguments": { "city": "Hangzhou" }
                            }
                        }]
                    },
                    "finish_reason": null
                }]
            }),
            &mut BTreeMap::new(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            LlmError::ToolProtocol { ref message }
                if message == "stream tool call arguments 不是字符串"
        ));
    }

    #[test]
    fn parse_stream_event_finishes_with_missing_arguments_error() {
        let error = OpenAiAdapter::parse_stream_event(
            "compatible",
            &serde_json::json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "function": {
                                "name": "get_weather"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }]
            }),
            &mut BTreeMap::new(),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            LlmError::ToolProtocol { ref message }
                if message == "stream tool call 缺少 arguments"
        ));
    }

    #[test]
    fn parse_stream_event_keeps_finish_reason_for_valid_tool_call() {
        let chunks = OpenAiAdapter::parse_stream_event(
            "compatible",
            &serde_json::json!({
                "choices": [{
                    "delta": {
                        "tool_calls": [{
                            "index": 0,
                            "id": "call_1",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"city\":\"Hangzhou\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }]
            }),
            &mut BTreeMap::new(),
        )
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(matches!(chunks[0].finish_reason, Some(FinishReason::ToolCalls)));
        assert_eq!(chunks[0].tool_calls.len(), 1);
    }
}
