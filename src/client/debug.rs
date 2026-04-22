use std::time::Duration;

fn debug_body(body: &[u8]) -> String {
    String::from_utf8_lossy(body).into_owned()
}

pub(super) fn debug_request(
    provider_name: &str,
    model: &str,
    stream: bool,
    url: &str,
    body_json: &str,
) {
    tracing::debug!(
        provider = provider_name,
        model,
        stream,
        request_url = %url,
        request_body = %body_json,
        "LLM 请求"
    );
}

pub(super) fn debug_request_success(provider_name: &str, status: u16) {
    tracing::debug!(
        provider = provider_name,
        status,
        "LLM 请求成功"
    );
}

pub(super) fn debug_chat_response(provider_name: &str, body: &[u8]) {
    tracing::debug!(
        provider = provider_name,
        response_body = %debug_body(body),
        "LLM 非流式响应"
    );
}

pub(super) fn debug_stream_event(provider_name: &str, data: &str) {
    tracing::debug!(
        provider = provider_name,
        stream_event = %data,
        "LLM 流式响应事件"
    );
}

pub(super) fn debug_request_failure(
    provider_name: &str,
    status: u16,
    retry_after: Option<Duration>,
    body: &[u8],
) {
    tracing::debug!(
        provider = provider_name,
        status,
        retry_after_secs = retry_after.map(|duration| duration.as_secs()),
        response_body = %debug_body(body),
        "LLM 请求失败"
    );
}

pub(super) fn debug_ignored_request_option(
    provider_name: &str,
    model: &str,
    option: &'static str,
    value: Option<&str>,
    reason: &'static str,
) {
    let option_value = value.unwrap_or("");
    tracing::debug!(
        provider = provider_name,
        model,
        option,
        option_value = %option_value,
        reason,
        "LLM 请求参数已忽略"
    );
}
