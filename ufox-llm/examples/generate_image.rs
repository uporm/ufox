use base64::Engine as _;
use std::{fs, io, path::PathBuf};
use ufox_llm::{Client, ImageGenRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 统一走 `.env` / 环境变量，便于在 OpenAI 与兼容接口之间切换。
    let client = Client::from_env()?;

    let mut extensions = serde_json::Map::new();
    // 显式要求返回 base64，优先演示“直接落盘”的最短路径；若服务端仍返回 URL，下面也会兼容处理。
    extensions.insert("response_format".into(), serde_json::json!("b64_json"));

    let response = client
        .generate_image(ImageGenRequest {
            prompt: "一座古老的中国寺庙坐落在青山之间，清晨薄雾缭绕，红墙青瓦，香炉升起袅袅青烟，一位身穿灰色僧袍的和尚在庭院中静坐打坐，阳光透过树叶洒下斑驳光影，超写实，电影级光影，细节丰富，8K，广角镜头，宁静氛围。".into(),
            n: Some(1),
            size: Some("1024x1024".into()),
            extensions,
        })
        .await?;

    let image = response
        .images
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "服务端未返回任何图片"))?;

    if let Some(revised_prompt) = &image.revised_prompt {
        println!("服务端改写后的提示词：{revised_prompt}");
    }

    if let Some(base64) = &image.base64 {
        let bytes = base64::engine::general_purpose::STANDARD.decode(base64)?;
        let output_path = PathBuf::from("generated-image.png");
        fs::write(&output_path, bytes)?;
        println!("图片已保存到：{}", output_path.display());
        return Ok(());
    }

    if let Some(url) = &image.url {
        println!("服务端返回的是图片 URL：{url}");
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "图片响应既没有 base64，也没有 url",
    )
    .into())
}
