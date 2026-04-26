use std::error::Error;

use ufox_llm::{
    ChatRequest, Client, ContentPart, Message, Provider, Role, Tool, ToolChoice, ToolResult,
    ToolResultPayload,
};

fn run_local_tool(
    name: &str,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn Error>> {
    match name {
        "get_weather" => Ok(serde_json::json!({
            "city": arguments.get("city").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "weather": "cloudy",
            "temperature_c": 24
        })),
        _ => Err(format!("unknown tool: {name}").into()),
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("tool_calling 示例执行失败：{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let client = Client::builder()
        .provider(Provider::OpenAI)
        .api_key("sk-xxx")
        .model("gpt-4o")
        .build()?;

    let weather_tool = Tool::function(
        "get_weather",
        "查询指定城市的实时天气",
        serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        }),
    );

    let mut messages = vec![Message {
        role: Role::User,
        content: vec![ContentPart::text("帮我查询杭州天气，并给出穿衣建议")],
        name: None,
    }];

    loop {
        let output = client
            .chat(
                ChatRequest::builder()
                    .messages(messages.clone())
                    .tools(vec![weather_tool.clone()])
                    .tool_choice(ToolChoice::Auto)
                    .build(),
            )
            .await?;

        let tool_calls = output.tool_calls.clone();
        messages.push(output.into_message());

        if tool_calls.is_empty() {
            if let Some(last) = messages.last() {
                println!("{}", last.text());
            }
            break;
        }

        for call in &tool_calls {
            let result = run_local_tool(&call.tool_name, &call.arguments)?;
            messages.push(Message {
                role: Role::Tool,
                content: vec![ContentPart::ToolResult(ToolResult {
                    tool_call_id: call.id.clone(),
                    tool_name: Some(call.tool_name.clone()),
                    payload: ToolResultPayload::json(result),
                    is_error: false,
                })],
                name: None,
            });
        }
    }

    Ok(())
}
