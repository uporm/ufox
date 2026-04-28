use futures::StreamExt;
use ufox_llm::{ChatRequest, Client, ContentPart};

#[tokio::main]
async fn main() -> Result<(), ufox_llm::LlmError> {
    // 默认走环境变量，避免示例里硬编码密钥，也方便切换不同 provider。
    let client = Client::from_env()?;

    let mut stream = client
        .chat_stream(
            ChatRequest::builder()
                .user(vec![
                    ContentPart::image_url("https://fastly.picsum.photos/id/294/800/600.jpg?hmac=X4RiVynizog5zMK1YZqNYt7sT1XJVHx4bRv9ZDCpPwI"),
                    ContentPart::text("这张图表说明了什么趋势？"),
                ])
                .max_tokens(512)
                .build(),
        )
        .await?;

    let mut started_answer = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if let Some(text) = &chunk.text_delta {
            started_answer = true;
            print!("{text}");
        }
        if chunk.is_finished() {
            break;
        }
    }

    if started_answer {
        println!();
    }

    Ok(())
}
