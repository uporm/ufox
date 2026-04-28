/// 注册给模型的可用工具。
///
/// `input_schema` 字段名与 Anthropic 协议对齐；OpenAI 系 adapter 在序列化时
/// 将其映射为 `parameters` 字段，调用方无需感知这一差异。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// 工具输入的 JSON Schema；公共接口直接使用 `serde_json::Value`。
    pub input_schema: serde_json::Value,
}

impl Tool {
    /// 用 JSON Schema 描述参数构造工具定义。
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: parameters,
        }
    }
}

/// 工具选择策略。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// 模型自行决定是否调用工具。
    #[default]
    Auto,
    /// 强制不调用任何工具。
    None,
    /// 强制必须调用至少一个工具，由模型自行选择。
    Required,
    /// 强制调用指定名称的工具。
    Specific(String),
}

/// assistant 发出的单次工具调用。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// provider 分配的唯一 ID，用于匹配后续 `ToolResult`。
    pub id: String,
    pub tool_name: String,
    /// 始终为已解析的 JSON 值，禁止以字符串形式存储。
    pub arguments: serde_json::Value,
}

/// tool role 回传的工具执行结果。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    /// 对应 `ToolCall.id`。
    pub tool_call_id: String,
    /// 部分 provider 在回传结果时需要重复携带工具名。
    pub tool_name: Option<String>,
    pub payload: ToolResultPayload,
    /// 标记执行是否出错。
    pub is_error: bool,
}

/// 工具执行结果载荷。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ToolResultPayload {
    Text(String),
    Json(serde_json::Value),
}

impl ToolResultPayload {
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    pub fn json(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}
