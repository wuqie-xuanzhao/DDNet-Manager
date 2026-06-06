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
mod tests {
    use crate::models::WorkshopBind;

    use super::parse_workshop_binds;

    #[test]
    fn parses_workshop_binds_payload_with_camel_case_fields() {
        let payload = r#"{
            "binds": [{
                "id": "bind-防自杀-fb77af69c13c",
                "category": "基础",
                "title": "防自杀",
                "command": "bind mouse3 \"echo Kill; kill\"",
                "description": "鼠标中键 - 快速自杀重开",
                "commandVariants": [
                    "bind mouse3 \"kill\""
                ],
                "variantLabels": [
                    "纯自杀"
                ],
                "isBindable": true
            }]
        }"#;

        let binds = parse_workshop_binds(payload).expect("测试输入应成功解析");
        let bind: &WorkshopBind = &binds[0];

        assert_eq!(binds.len(), 1);
        assert_eq!(bind.id, "bind-防自杀-fb77af69c13c");
        assert_eq!(bind.category, "基础");
        assert!(bind.is_bindable);
        assert_eq!(bind.command_variants, vec!["bind mouse3 \"kill\""]);
        assert_eq!(bind.variant_labels, vec!["纯自杀"]);
    }

    #[test]
    fn defaults_is_bindable_to_false_when_field_is_missing() {
        let payload = r#"{
            "binds": [{
                "id": "bind-基础-1",
                "category": "基础",
                "title": "无开关字段",
                "command": "bind f1 \"say hi\"",
                "description": "省略 isBindable 仍应兼容",
                "commandVariants": [
                    "bind f1 \"echo hi\""
                ],
                "variantLabels": [
                    "提示版"
                ]
            }]
        }"#;

        let binds = parse_workshop_binds(payload).expect("缺失 isBindable 也应成功解析");

        assert_eq!(binds.len(), 1);
        assert!(!binds[0].is_bindable);
        assert_eq!(binds[0].command_variants, vec!["bind f1 \"echo hi\""]);
        assert_eq!(binds[0].variant_labels, vec!["提示版"]);
    }

    #[test]
    fn rejects_workshop_response_when_chunks_exceed_limit() {
        let error = super::workshop_response_text_from_chunks(
            Some(8),
            8,
            vec![
                Ok::<Vec<u8>, std::convert::Infallible>(vec![b'a'; 4]),
                Ok::<Vec<u8>, std::convert::Infallible>(vec![b'b'; 5]),
            ],
        )
        .expect_err("超过上限的响应体应被拒绝");

        assert_eq!(error, "workshop response exceeds 8 bytes");
    }

    #[test]
    fn rejects_workshop_response_when_content_length_exceeds_limit() {
        let error = super::workshop_response_text_from_chunks(
            Some(9),
            8,
            Vec::<Result<Vec<u8>, std::convert::Infallible>>::new(),
        )
        .expect_err("超过上限的 content-length 应被拒绝");

        assert_eq!(error, "workshop response exceeds 8 bytes");
    }
}
