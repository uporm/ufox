use std::time::Duration;

use ufox_llm::{Client, LlmError, TaskStatus, VideoFormat, VideoGenRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 统一走 `.env` / 环境变量，便于后续直接切换到支持视频生成的 provider。
    let client = Client::from_env()?;
    let requested_duration_secs = 8;
    let output_path = std::path::PathBuf::from("output/generated_video.mp4");

    let response = match client
        .generate_video(VideoGenRequest {
            prompt: "朝鲜战争时期的战场清晨，寒冷冬季，雪地与焦黑的山地交错，天空灰蒙。远处炮火闪烁，硝烟弥漫。\
            中国人民志愿军士兵身穿厚重棉衣，背着步枪与弹药，在炮火掩护下从战壕中跃出，发起冲锋。\
            镜头采用电影级写实风格，手持摄影机视角，轻微晃动增强临场感。士兵表情坚毅，动作迅速，雪地上留下深深脚印。\
            爆炸火光在远处不断闪现，尘土与雪雾被冲击波掀起。背景音乐紧张低沉，节奏逐渐增强。\
            整体画面强调历史战争的真实感与肃穆氛围，色调冷灰偏蓝，强对比光影，电影级8K画质，浅景深，慢动作与实时镜头交替。".into(),
            // OpenAI 当前公开视频接口只接受 4 / 8 / 12 秒。
            duration_secs: Some(requested_duration_secs),
            output_format: Some(VideoFormat::Mp4),
            extensions: Default::default(),
        })
        .await
    {
        Ok(response) => response,
        Err(LlmError::UnsupportedCapability { capability, .. })
            if capability == "generate_video" =>
        {
            eprintln!("当前 provider / model 还未接入视频生成能力。");
            eprintln!("请改用已实现该能力的 provider 后重试。");
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    println!(
        "视频任务已提交：task_id={}，初始状态={:?}",
        response.task_id, response.status
    );

    let final_response = wait_until_finished(&client, response).await?;

    match final_response.status {
        TaskStatus::Succeeded => {
            if let Some(url) = final_response.url {
                println!("视频生成完成，内容下载端点：{url}");
                println!("注意：该地址通常仍需携带 API Key 才能成功下载。");
                client
                    .download_video_to_file(&final_response.task_id, &output_path)
                    .await?;
                println!("视频已保存到本地：{}", output_path.display());
            } else {
                println!("视频生成完成，但 provider 未返回下载地址。");
            }
        }
        TaskStatus::Failed => {
            println!("视频生成失败，task_id={}", final_response.task_id);
        }
        TaskStatus::Pending | TaskStatus::Processing => {
            println!("任务仍未结束，当前状态={:?}", final_response.status);
        }
    }

    Ok(())
}

async fn wait_until_finished(
    client: &Client,
    mut response: ufox_llm::VideoGenResponse,
) -> Result<ufox_llm::VideoGenResponse, LlmError> {
    loop {
        match response.status {
            TaskStatus::Succeeded | TaskStatus::Failed => return Ok(response),
            TaskStatus::Pending | TaskStatus::Processing => {
                // 轮询间隔保持保守，避免在长任务上触发不必要的 provider 限流。
                tokio::time::sleep(Duration::from_secs(3)).await;
                response = client.poll_video_task(&response.task_id).await?;
                println!("轮询状态：task_id={}，status={:?}", response.task_id, response.status);
            }
        }
    }
}
