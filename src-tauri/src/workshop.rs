use crate::models::WorkshopBind;
use std::time::Duration;

const MAX_WORKSHOP_BYTES: usize = 1_048_576;

#[derive(serde::Deserialize)]
struct WorkshopPayload {
    binds: Vec<WorkshopBind>,
}

/// 解析 Workshop 公开 JSON 文本，并返回其中的 bind 列表。
pub fn parse_workshop_binds(input: &str) -> Result<Vec<WorkshopBind>, String> {
    let payload: WorkshopPayload =
        serde_json::from_str(input).map_err(|error| format!("invalid workshop json: {error}"))?;
    Ok(payload.binds)
}

/// 从远程地址拉取 Workshop 公开 JSON，并解析为 bind 列表。
pub async fn fetch_workshop_binds(url: &str) -> Result<Vec<WorkshopBind>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("failed to build workshop client: {error}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch workshop binds: {error}"))?;
    let text = read_limited_workshop_response(response).await?;

    parse_workshop_binds(&text)
}

async fn read_limited_workshop_response(mut response: reqwest::Response) -> Result<String, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_WORKSHOP_BYTES as u64)
    {
        return Err(format!(
            "workshop response exceeds {MAX_WORKSHOP_BYTES} bytes"
        ));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("failed to read workshop response: {error}"))?
    {
        append_workshop_chunk(&mut bytes, &chunk, MAX_WORKSHOP_BYTES)?;
    }

    String::from_utf8(bytes)
        .map_err(|error| format!("workshop response is not valid UTF-8: {error}"))
}

#[cfg(test)]
fn workshop_response_text_from_chunks<I, E>(
    content_length: Option<u64>,
    max_bytes: usize,
    chunks: I,
) -> Result<String, String>
where
    I: IntoIterator<Item = Result<Vec<u8>, E>>,
    E: std::fmt::Display,
{
    if content_length.is_some_and(|length| length > max_bytes as u64) {
        return Err(format!("workshop response exceeds {max_bytes} bytes"));
    }

    let mut bytes = Vec::new();
    for chunk in chunks {
        let chunk = chunk.map_err(|error| format!("failed to read workshop response: {error}"))?;
        append_workshop_chunk(&mut bytes, &chunk, max_bytes)?;
    }

    String::from_utf8(bytes)
        .map_err(|error| format!("workshop response is not valid UTF-8: {error}"))
}

fn append_workshop_chunk(
    bytes: &mut Vec<u8>,
    chunk: &[u8],
    max_bytes: usize,
) -> Result<(), String> {
    if bytes.len() + chunk.len() > max_bytes {
        return Err(format!("workshop response exceeds {max_bytes} bytes"));
    }

    bytes.extend_from_slice(chunk);
    Ok(())
}

#[cfg(test)]
#[path = "test/workshop.rs"]
mod tests;
