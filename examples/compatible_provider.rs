use ufox_llm::{Client, Provider};

fn main() {
    if let Err(err) = run() {
        eprintln!("compatible_provider 示例执行失败：{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), ufox_llm::LlmError> {
    let client = Client::builder()
        .provider(Provider::Compatible)
        .base_url("https://api.deepseek.com/v1")
        .api_key("sk-xxx")
        .model("deepseek-chat")
        .build()?;

    assert_eq!(client.base_url(), "https://api.deepseek.com/v1");
    Ok(())
}
