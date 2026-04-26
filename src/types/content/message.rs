use super::{Audio, Image, MediaSource, Text, ToolCall, ToolResult, ToolResultPayload, Video};

/// 消息内容的最小单元，覆盖所有模态和工具交互。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text(Text),
    Image(Image),
    Audio(Audio),
    Video(Video),
    /// assistant 发出的工具调用。
    ToolCall(ToolCall),
    /// tool role 回传的工具执行结果。
    ToolResult(ToolResult),
}

impl ContentPart {
    pub fn text(s: impl Into<String>) -> Self {
        ContentPart::Text(Text { text: s.into() })
    }

    pub fn image_url(url: impl Into<String>) -> Self {
        ContentPart::Image(Image {
            source: MediaSource::Url { url: url.into() },
            fidelity: None,
        })
    }

    pub fn image_file(path: impl Into<std::path::PathBuf>) -> Self {
        ContentPart::Image(Image {
            source: MediaSource::File { path: path.into() },
            fidelity: None,
        })
    }

    pub fn tool_call(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        ContentPart::ToolCall(ToolCall {
            id: id.into(),
            tool_name: name.into(),
            arguments,
        })
    }

    /// 构造文本型工具结果。
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ContentPart::ToolResult(ToolResult {
            tool_call_id: tool_call_id.into(),
            tool_name: None,
            payload: ToolResultPayload::text(content),
            is_error: false,
        })
    }
}

/// 对话角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// 对话历史中的单条消息。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentPart>,
    /// provider-specific 元数据；适配器会在目标协议支持时透传，不解析其语义。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// 按顺序拼接消息中所有文本片段。
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentPart, Message, Role};

    #[test]
    fn message_text_concatenates_only_text_parts() {
        let message = Message {
            role: Role::Assistant,
            content: vec![
                ContentPart::text("hello"),
                ContentPart::image_url("https://example.com/image.png"),
                ContentPart::text(" world"),
            ],
            name: None,
        };

        assert_eq!(message.text(), "hello world");
    }
}
