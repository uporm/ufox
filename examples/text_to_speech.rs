use ufox_llm::{AudioFormat, Client, Provider, TextToSpeechRequest};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("text_to_speech 示例执行失败：{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .provider(Provider::OpenAI)
        .api_key("sk-xxx")
        .model("gpt-4o-mini-tts")
        .build()?;

    let output = client
        .text_to_speech(TextToSpeechRequest {
            text: "你好，欢迎使用 ufox-llm。".into(),
            voice: Some("alloy".into()),
            output_format: AudioFormat::Mp3,
            extensions: Default::default(),
        })
        .await?;

    std::fs::write("speech.mp3", output.audio_data)?;
    Ok(())
}
