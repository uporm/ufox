/// OpenAI 图像输入保真度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFidelity {
    Auto,
    Low,
    High,
}

/// 音频编解码格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Mp3,
    Wav,
    Flac,
    Opus,
    Aac,
    Pcm,
}

/// 视频输出格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoFormat {
    Mp4,
    Webm,
    Avi,
    Mov,
}

/// 多模态内容来源。
///
/// `File` 变体由 adapter 层在组装请求体时异步读取并转为 `Base64`，
/// 调用方无需手动编码。读取失败返回 `LlmError::MediaInput`。
///
/// 对不支持 `Url` 变体的 provider，adapter 内部自动下载并转为 `Base64`；
/// 若下载失败同样返回 `LlmError::MediaInput`。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaSource {
    /// 内联 base64 编码内容。
    Base64 { data: String, mime_type: String },
    /// 远程 URL。
    Url { url: String },
    /// 本地文件路径。
    File { path: std::path::PathBuf },
}

/// 文本内容。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Text {
    pub text: String,
}

/// 图像内容。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Image {
    pub source: MediaSource,
    /// OpenAI 图像输入保真度参数，其他 provider 忽略此字段。
    pub fidelity: Option<ImageFidelity>,
}

/// 音频内容。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Audio {
    pub source: MediaSource,
    pub format: AudioFormat,
}

/// 视频内容。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Video {
    pub source: MediaSource,
    pub format: VideoFormat,
    /// 采样帧数，`None` 表示由 provider 决定。
    pub sample_frames: Option<u32>,
}
