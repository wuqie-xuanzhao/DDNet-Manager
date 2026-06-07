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
