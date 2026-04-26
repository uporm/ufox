use ufox_llm::{ChatRequest, Client, ContentPart, Provider};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("multimodal_image 示例执行失败：{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), ufox_llm::LlmError> {
    let client = Client::builder()
        .provider(Provider::OpenAI)
        .api_key("sk-xxx")
        .model("gpt-4o")
        .build()?;

    let req = ChatRequest::builder()
        .user(vec![
            ContentPart::image_url("https://example.com/chart.png"),
            ContentPart::text("这张图表说明了什么趋势？"),
        ])
        .max_tokens(512)
        .build();

    let output = client.chat(req).await?;
    println!("{}", output.text);
    Ok(())
}
