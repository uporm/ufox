use ufox_llm::{Client, EmbeddingRequest};

const PREVIEW_DIMS: usize = 8;

#[tokio::main]
async fn main() -> Result<(), ufox_llm::LlmError> {
    // 默认走环境变量，避免示例里硬编码密钥和模型配置。
    let client = Client::from_env()?;
    let inputs = vec![
        "Rust trait object".to_string(),
        "async stream".to_string(),
    ];

    let resp = client
        .embed(EmbeddingRequest {
            inputs: inputs.clone(),
            dimensions: None,
            extensions: Default::default(),
        })
        .await?;

    println!("model: {}", resp.model);
    println!("embeddings: {}", resp.embeddings.len());

    for (index, embedding) in resp.embeddings.iter().enumerate() {
        let input = inputs.get(index).map(String::as_str).unwrap_or("<unknown>");
        let preview_len = embedding.len().min(PREVIEW_DIMS);
        println!("--- embedding #{index} ---");
        println!("input: {input}");
        println!("dimensions: {}", embedding.len());
        println!("first {preview_len} dims: {:?}", &embedding[..preview_len]);
    }

    Ok(())
}
