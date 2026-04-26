use base64::Engine as _;
use futures::StreamExt;
use wiremock::{
    matchers::{body_partial_json, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::{ChatRequest, Client, Provider};

#[tokio::test]
async fn compatible_chat_parses_text_and_tool_calls() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("authorization", "Bearer test-key"))
        .and(body_partial_json(serde_json::json!({
            "model": "gpt-4o-mini",
            "stream": false,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl_123",
            "model": "gpt-4o-mini",
            "choices": [{
                "message": {
                    "content": "杭州今天多云。",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\":\"Hangzhou\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 12,
                "total_tokens": 22
            }
        })))
        .mount(&server)
        .await;

    let client = Client::builder()
        .provider(Provider::Compatible)
        .base_url(server.uri())
        .api_key("test-key")
        .model("gpt-4o-mini")
        .build()
        .unwrap();

    let response = client
        .chat(ChatRequest::builder().user_text("杭州天气").build())
        .await
        .unwrap();

    assert_eq!(response.text, "杭州今天多云。");
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].tool_name, "get_weather");
    assert_eq!(
        response.tool_calls[0].arguments,
        serde_json::json!({ "city": "Hangzhou" })
    );
}

#[tokio::test]
async fn compatible_embed_maps_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid api key"))
        .mount(&server)
        .await;

    let client = Client::builder()
        .provider(Provider::Compatible)
        .base_url(server.uri())
        .api_key("test-key")
        .model("text-embedding-3-small")
        .build()
        .unwrap();

    let error = client
        .embed(crate::EmbeddingRequest {
            inputs: vec!["hello".into()],
            dimensions: None,
            extensions: Default::default(),
        })
        .await
        .unwrap_err();

    match error {
        crate::LlmError::Authentication { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn compatible_chat_stream_aggregates_tool_calls() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(serde_json::json!({
            "model": "gpt-4o-mini",
            "stream": true,
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("content-type", "text/event-stream")
                .set_body_string(
                    "data: {\"id\":\"chatcmpl_1\",\"choices\":[{\"delta\":{\"content\":\"你好\"},\"finish_reason\":null}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"{\\\"city\\\":\\\"杭\"}}]},\"finish_reason\":null}]}\n\n\
data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"州\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":8,\"total_tokens\":18}}\n\n\
data: [DONE]\n\n",
                ),
        )
        .mount(&server)
        .await;

    let client = Client::builder()
        .provider(Provider::Compatible)
        .base_url(server.uri())
        .api_key("test-key")
        .model("gpt-4o-mini")
        .build()
        .unwrap();

    let mut stream = client
        .chat_stream(ChatRequest::builder().user_text("杭州天气").build())
        .await
        .unwrap();

    let first = stream.next().await.unwrap().unwrap();
    assert_eq!(first.text_delta.as_deref(), Some("你好"));
    assert!(!first.is_finished());

    let second = stream.next().await.unwrap().unwrap();
    assert_eq!(second.tool_calls.len(), 1);
    assert_eq!(second.tool_calls[0].tool_name, "get_weather");
    assert_eq!(
        second.tool_calls[0].arguments,
        serde_json::json!({ "city": "杭州" })
    );
    assert!(matches!(
        second.finish_reason,
        Some(crate::FinishReason::ToolCalls)
    ));
    assert_eq!(second.usage.unwrap().total_tokens, 18);
}

#[tokio::test]
async fn openai_image_generation_parses_images() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{
                "url": "https://example.com/image.png",
                "revised_prompt": "an improved prompt"
            }]
        })))
        .mount(&server)
        .await;

    let client = Client::builder()
        .provider(Provider::Compatible)
        .base_url(server.uri())
        .api_key("test-key")
        .model("gpt-image-1")
        .build()
        .unwrap();

    let response = client
        .generate_image(crate::ImageGenRequest {
            prompt: "一只狐狸".into(),
            n: Some(1),
            size: Some("1024x1024".into()),
            extensions: Default::default(),
        })
        .await
        .unwrap();

    assert_eq!(response.images.len(), 1);
    assert_eq!(
        response.images[0].url.as_deref(),
        Some("https://example.com/image.png")
    );
    assert_eq!(
        response.images[0].revised_prompt.as_deref(),
        Some("an improved prompt")
    );
}

#[tokio::test]
async fn openai_text_to_speech_returns_audio_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/audio/speech"))
        .and(body_partial_json(serde_json::json!({
            "model": "gpt-4o-mini-tts",
            "input": "你好",
            "voice": "alloy",
            "response_format": "mp3"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("content-type", "audio/mpeg")
                .set_body_bytes(b"fake-mp3".to_vec()),
        )
        .mount(&server)
        .await;

    let client = Client::builder()
        .provider(Provider::Compatible)
        .base_url(server.uri())
        .api_key("test-key")
        .model("gpt-4o-mini-tts")
        .build()
        .unwrap();

    let response = client
        .text_to_speech(crate::TextToSpeechRequest {
            text: "你好".into(),
            voice: Some("alloy".into()),
            output_format: crate::AudioFormat::Mp3,
            extensions: Default::default(),
        })
        .await
        .unwrap();

    assert_eq!(response.audio_data.as_ref(), b"fake-mp3");
    assert!(matches!(response.format, crate::AudioFormat::Mp3));
}

#[tokio::test]
async fn openai_speech_to_text_parses_transcript() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "text": "你好，世界",
            "language": "zh",
            "duration": 1.25,
            "usage": {
                "prompt_tokens": 0,
                "completion_tokens": 8,
                "total_tokens": 8
            }
        })))
        .mount(&server)
        .await;

    let client = Client::builder()
        .provider(Provider::Compatible)
        .base_url(server.uri())
        .api_key("test-key")
        .model("gpt-4o-mini-transcribe")
        .build()
        .unwrap();

    let response = client
        .speech_to_text(crate::SpeechToTextRequest {
            source: crate::MediaSource::Base64 {
                data: base64::engine::general_purpose::STANDARD.encode(b"fake wav"),
                mime_type: "audio/wav".into(),
            },
            format: crate::AudioFormat::Wav,
            language: Some("zh".into()),
            extensions: Default::default(),
        })
        .await
        .unwrap();

    assert_eq!(response.text, "你好，世界");
    assert_eq!(response.language.as_deref(), Some("zh"));
    assert_eq!(response.duration_secs, Some(1.25));
    assert_eq!(response.usage.unwrap().total_tokens, 8);
}
