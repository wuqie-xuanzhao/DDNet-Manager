#[cfg(test)]
mod tests {
    use crate::commands::required_manifest_url;

    #[test]
    fn required_manifest_url_rejects_missing_or_blank_input() {
        assert_eq!(
            required_manifest_url(None).expect_err("缺少 manifest 地址应被拒绝"),
            "manifest url is not configured"
        );
        assert_eq!(
            required_manifest_url(Some("   ")).expect_err("空 manifest 地址应被拒绝"),
            "manifest url is not configured"
        );
    }

    #[test]
    fn required_manifest_url_trims_configured_input() {
        let url = required_manifest_url(Some(
            "  https://raw.githubusercontent.com/example/manifest/main/manifest.json  ",
        ))
        .expect("非空 manifest 地址应被接受");

        assert_eq!(
            url,
            "https://raw.githubusercontent.com/example/manifest/main/manifest.json"
        );
    }

    #[test]
    fn render_manager_bind_cfg_rejects_multiline_command() {
        let error = crate::commands::render_manager_bind_cfg(vec![
            "bind f1 \"say hi\"\nbind f2 \"say bye\"".to_string(),
        ])
        .expect_err("multiline should fail");

        assert!(error.contains("must not contain newline"));
    }

    #[test]
    fn render_manager_bind_cfg_rejects_manager_marker_injection() {
        let error = crate::commands::render_manager_bind_cfg(vec![format!(
            "bind f1 \"echo {}\"",
            crate::file_tx::MANAGER_BEGIN
        )])
        .expect_err("marker should fail");

        assert!(error.contains("must not contain manager markers"));
    }

    #[test]
    fn render_manager_bind_cfg_returns_rendered_block_for_valid_commands() {
        let output = crate::commands::render_manager_bind_cfg(vec![
            " bind f1 \"say hi\" ".to_string(),
            "bind mouse5 \"toggle cl_showfps 0 1\"".to_string(),
        ])
        .expect("valid commands");

        assert_eq!(
                output,
                "# DDNET_MANAGER_BEGIN\n# This block is managed by DDNet Manager.\nbind f1 \"say hi\"\nbind mouse5 \"toggle cl_showfps 0 1\"\n# DDNET_MANAGER_END\n"
            );
    }
}
