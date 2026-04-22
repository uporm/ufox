//! `Qwen` 请求序列化。
//!
//! 将公共消息和工具定义转换为 Qwen OpenAI-compatible 请求体。

use serde_json::{Value, json};

use crate::{LlmError, Message, Tool, types::RequestOptions};

/// 将公共聊天参数转换为 `Qwen` OpenAI-compatible 请求体。
/// # Errors
/// - [`LlmError::UnsupportedFeature`]：当当前公共消息暂时无法映射为 `Qwen` 请求时触发
/// - [`LlmError::StreamError`]：当读取本地多媒体文件失败或构建 `data URL` 失败时触发
pub fn build_chat_request(
    model: &str,
    messages: &[Message],
    tools: Option<&[Tool]>,
    stream: bool,
    options: &RequestOptions,
) -> Result<Value, LlmError> {
    let mut request =
        crate::provider::openai::request::build_chat_request(model, messages, tools, stream, options)?;
    let body = request
        .as_object_mut()
        .ok_or_else(|| LlmError::StreamError("Qwen 请求体不是合法 JSON 对象".to_string()))?;

    if options.thinking {
        body.insert("enable_thinking".to_string(), Value::Bool(true));
    }

    if let Some(thinking_budget) = options.thinking_budget {
        body.insert("thinking_budget".to_string(), json!(thinking_budget));
    }

    Ok(request)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_chat_request;
    use crate::{JsonType, Message, MessageBuilder, Role, Tool, types::RequestOptions};

    #[test]
    fn qwen_openai_compatible_text_request() {
        let request = build_chat_request(
            "qwen3-max",
            &[Message::user("你好")],
            None,
            false,
            &RequestOptions::default(),
        )
        .expect("请求体应构建成功");

        assert_eq!(request["model"], "qwen3-max");
        assert_eq!(request["messages"][0]["role"], "user");
        assert_eq!(request["messages"][0]["content"], "你好");
        assert_eq!(request["stream"], false);
    }

    #[test]
    fn qwen_openai_compatible_image_parts() {
        let message = Message::builder(Role::User)
            .text("描述这张图片")
            .image_url("https://example.com/photo.jpg")
            .build();

        let request = build_chat_request(
            "qwen-vl-max",
            &[message],
            None,
            false,
            &RequestOptions::default(),
        )
        .expect("请求体应构建成功");

        assert_eq!(
            request["messages"][0]["content"][0]["type"],
            "text"
        );
        assert_eq!(
            request["messages"][0]["content"][0]["text"],
            "描述这张图片"
        );
        assert_eq!(
            request["messages"][0]["content"][1]["type"],
            "image_url"
        );
        assert_eq!(
            request["messages"][0]["content"][1]["image_url"]["url"],
            "https://example.com/photo.jpg"
        );
    }

    #[test]
    fn qwen_openai_compatible_tools() {
        let tool = Tool::function("get_weather")
            .description("获取城市实时天气")
            .param("city", JsonType::String, "城市名称", true)
            .param(
                "unit",
                JsonType::Enum(vec!["celsius".to_string(), "fahrenheit".to_string()]),
                "温度单位",
                false,
            )
            .build();

        let request = build_chat_request(
            "qwen3-max",
            &[Message::user("杭州天气")],
            Some(&[tool]),
            true,
            &RequestOptions::default(),
        )
        .expect("请求体应构建成功");

        assert_eq!(request["stream"], true);
        assert_eq!(request["tools"][0]["type"], "function");
        assert_eq!(
            request["tools"][0]["function"]["name"],
            "get_weather"
        );
        assert_eq!(
            request["tools"][0]["function"]["parameters"],
            json!({
                "type": "object",
                "properties": {
                    "city": {
                        "type": "string",
                        "description": "城市名称"
                    },
                    "unit": {
                        "type": "string",
                        "enum": ["celsius", "fahrenheit"],
                        "description": "温度单位"
                    }
                },
                "additionalProperties": false,
                "required": ["city"]
            })
        );
    }

    #[test]
    fn qwen_openai_compatible_data_url() {
        let file_path = temp_png_path();
        std::fs::write(&file_path, [0x89, b'P', b'N', b'G']).expect("应能写入测试图片");
        let message = MessageBuilder::user().image_file(&file_path).build();
        let request = build_chat_request(
            "qwen-vl-max",
            &[message],
            None,
            false,
            &RequestOptions::default(),
        )
        .expect("请求体应构建成功");
        let data_url = request["messages"][0]["content"][0]["image_url"]["url"]
            .as_str()
            .expect("应为字符串 URL");

        assert!(data_url.starts_with("data:image/png;base64,"));

        std::fs::remove_file(file_path).expect("应能清理测试文件");
    }

    #[test]
    fn qwen_openai_compatible_tool_calls() {
        let calls = vec![crate::ToolCall::new(
            "call_1",
            "get_weather",
            r#"{"city":"杭州"}"#,
        )];
        let request = build_chat_request(
            "qwen3-max",
            &[crate::Message::assistant_with_tool_calls(&calls)],
            None,
            false,
            &RequestOptions::default(),
        )
        .expect("请求体应构建成功");

        assert_eq!(request["messages"][0]["role"], "assistant");
        assert_eq!(
            request["messages"][0]["content"],
            serde_json::Value::Null
        );
        assert_eq!(request["messages"][0]["tool_calls"][0]["id"], "call_1");
    }

    #[test]
    fn qwen_openai_compatible_tool_role() {
        let request = build_chat_request(
            "qwen3-max",
            &[crate::Message::tool_result("call_1", r#"{"temp":26}"#)],
            None,
            false,
            &RequestOptions::default(),
        )
        .expect("请求体应构建成功");

        assert_eq!(request["messages"][0]["role"], "tool");
        assert_eq!(request["messages"][0]["tool_call_id"], "call_1");
        assert_eq!(request["messages"][0]["content"], r#"{"temp":26}"#);
    }

    #[test]
    fn qwen_keeps_thinking_extensions_on_top_level() {
        let request = build_chat_request(
            "qwen3-max",
            &[Message::user("分析这段代码")],
            None,
            true,
            &RequestOptions {
                thinking: true,
                thinking_budget: Some(8192),
                ..RequestOptions::default()
            },
        )
        .expect("请求体应构建成功");

        assert_eq!(request["enable_thinking"], true);
        assert_eq!(request["thinking_budget"], 8192);
    }

    #[test]
    fn qwen_openai_compatible_parameters() {
        let tool = Tool::function("get_weather")
            .param("city", JsonType::String, "城市名称", true)
            .build();
        let request = build_chat_request(
            "qwen3-max",
            &[Message::user("杭州天气")],
            Some(&[tool]),
            false,
            &RequestOptions {
                tool_choice: Some(crate::ToolChoice::function("get_weather")),
                parallel_tool_calls: Some(true),
                ..RequestOptions::default()
            },
        )
        .expect("请求体应构建成功");

        assert_eq!(request["parallel_tool_calls"], true);
        assert_eq!(request["tool_choice"]["type"], "function");
        assert_eq!(
            request["tool_choice"]["function"]["name"],
            "get_weather"
        );
    }

    #[test]
    fn qwen_openai_compatible_sampling_parameters() {
        let request = build_chat_request(
            "qwen3-max",
            &[Message::user("讲个故事")],
            None,
            false,
            &RequestOptions {
                temperature: Some(0.7),
                top_p: Some(0.85),
                max_tokens: Some(768),
                presence_penalty: Some(0.3),
                frequency_penalty: Some(0.15),
                ..RequestOptions::default()
            },
        )
        .expect("请求体应构建成功");

        assert!(
            (request["temperature"]
                .as_f64()
                .expect("temperature 应为数字")
                - 0.7)
                .abs()
                < 1e-6
        );
        assert!(
            (request["top_p"]
                .as_f64()
                .expect("top_p 应为数字")
                - 0.85)
                .abs()
                < 1e-6
        );
        assert_eq!(request["max_tokens"], 768);
        assert!(
            (request["presence_penalty"]
                .as_f64()
                .expect("presence_penalty 应为数字")
                - 0.3)
                .abs()
                < 1e-6
        );
        assert!(
            (request["frequency_penalty"]
                .as_f64()
                .expect("frequency_penalty 应为数字")
                - 0.15)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn qwen_provider_options_merge_into_openai_compatible_body() {
        let request = build_chat_request(
            "qwen3-max",
            &[Message::user("讲个故事")],
            None,
            false,
            &RequestOptions {
                top_p: Some(0.85),
                provider_options: serde_json::Map::from_iter([
                    ("seed".to_string(), json!(7)),
                    ("top_p".to_string(), json!(0.2)),
                    ("repetition_penalty".to_string(), json!(1.1)),
                ]),
                ..RequestOptions::default()
            },
        )
        .expect("请求体应构建成功");

        assert_eq!(request["seed"], 7);
        assert_eq!(request["repetition_penalty"], 1.1);
        assert!(
            (request["top_p"]
                .as_f64()
                .expect("top_p 应为数字")
                - 0.85)
                .abs()
                < 1e-6
        );
    }

    fn temp_png_path() -> std::path::PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时间应大于 UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ufox-llm-qwen-request-{timestamp}-{}.png",
            std::process::id()
        ))
    }
}
