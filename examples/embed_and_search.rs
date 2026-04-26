use ufox_llm::{Client, EmbeddingRequest, Provider};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("embed_and_search 示例执行失败：{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), ufox_llm::LlmError> {
    let client = Client::builder()
        .provider(Provider::OpenAI)
        .api_key("sk-xxx")
        .model("text-embedding-3-small")
        .build()?;

    let resp = client
        .embed(EmbeddingRequest {
            inputs: vec!["Rust trait object".into(), "async stream".into()],
            dimensions: None,
            extensions: Default::default(),
        })
        .await?;

    println!("embeddings: {}", resp.embeddings.len());
    Ok(())
}
