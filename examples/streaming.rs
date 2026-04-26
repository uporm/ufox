use futures::StreamExt;
use ufox_llm::{ChatRequest, Client};

#[tokio::main]
async fn main() -> Result<(), ufox_llm::LlmError> {
    // 默认走环境变量，避免维护两份仅初始化方式不同的示例。
    let client = Client::from_env()?;
    // 也可以显式构建客户端：
    // let client = Client::builder()
    //     .provider(Provider::OpenAI)
    //     .api_key("sk-xxx")
    //     .model("gpt-4o")
    //     .build()?;

    let mut stream = client
        .chat_stream(
            ChatRequest::builder()
                .user_text("写一首关于 Rust 的诗")
                .build(),
        )
        .await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if let Some(text) = &chunk.text_delta {
            print!("{text}");
        }
        if let Some(thinking) = &chunk.thinking_delta {
            eprint!("[thinking]{thinking}");
        }
        if chunk.is_finished() {
            break;
        }
    }

    Ok(())
}
