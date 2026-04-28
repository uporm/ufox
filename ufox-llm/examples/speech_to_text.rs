use ufox_llm::{AudioFormat, Client, MediaSource, SpeechToTextRequest};

#[tokio::main]
async fn main() -> Result<(), ufox_llm::LlmError> {
    // 默认走环境变量，避免示例里硬编码密钥，也和其他示例保持一致。
    let client = Client::from_env()?;

    let output = client
        .speech_to_text(SpeechToTextRequest {
            source: MediaSource::File {
                path: "examples/sample.mp3".into(),
            },
            format: AudioFormat::Mp3,
            language: Some("zh".into()),
            extensions: Default::default(),
        })
        .await?;

    println!("{}", output.text);
    Ok(())
}
