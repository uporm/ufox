use ufox_llm::{AudioFormat, Client, MediaSource, Provider, SpeechToTextRequest};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("speech_to_text 示例执行失败：{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), ufox_llm::LlmError> {
    let client = Client::builder()
        .provider(Provider::OpenAI)
        .api_key("sk-xxx")
        .model("gpt-4o-mini-transcribe")
        .build()?;

    let output = client
        .speech_to_text(SpeechToTextRequest {
            source: MediaSource::File {
                path: "sample.wav".into(),
            },
            format: AudioFormat::Wav,
            language: Some("zh".into()),
            extensions: Default::default(),
        })
        .await?;

    println!("{}", output.text);
    Ok(())
}
