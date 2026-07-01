//! 公共反代前缀拼装与候选 URL 组装。
//!
//! 仅在下载执行层使用：更新发现层产出的 `asset_url` 始终是权威原始 GitHub
//! 地址，本模块负责把原始 URL 与反代前缀拼成多候选 URL 列表，交给竞速模块
//! 实测吞吐择优。URL 重写不污染数据层。

/// P0 代码内 fallback 反代前缀列表。
///
/// P1 外部化热更新上线后，此常量作为兜底：当 `AppSettings.mirror_prefixes`
/// 为空时使用。反代不是"主力"，是裸连用户的降级路径；竞速会自然让挂代理
/// 用户选 GitHub、裸连用户选可达的反代。
pub const DEFAULT_MIRROR_PREFIXES: &[&str] = &[
    "https://gh-proxy.com/",
    "https://mirror.ghproxy.com/",
    "https://gh.api.99988866.xyz/",
    "https://g.ioiox.com/",
];

/// 把原始资产 URL 与单个反代前缀拼成反代 URL（rustup 风格字符串拼接）。
///
/// `proxy_prefix` 为 `None` 或空串时直接返回原始 URL。前缀末尾的 `/` 会被
/// 规范化为恰好一个，避免拼出双斜杠或无斜杠。
pub fn resolve_asset_url(original: &str, proxy_prefix: Option<&str>) -> String {
    match proxy_prefix {
        Some(prefix) => {
            let trimmed = prefix.trim();
            if trimmed.is_empty() {
                return original.to_string();
            }
            format!("{}/{}", trimmed.trim_end_matches('/'), original)
        }
        None => original.to_string(),
    }
}

/// 由原始 URL + 多个反代前缀组装候选 URL 列表。
///
/// 顺序：原始 URL 始终首位（保证 GitHub 直连作为平等候选参与竞速，而非
/// 降级兜底），后接各反代 URL。前缀去重并去空，避免重复打爆反代。返回
/// 列表至少含原始 URL（即使 `prefixes` 为空）。
pub fn build_candidate_urls(original: &str, prefixes: &[String]) -> Vec<String> {
    let mut candidates = Vec::with_capacity(prefixes.len() + 1);
    candidates.push(original.to_string());
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for prefix in prefixes {
        let trimmed = prefix.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !seen.insert(trimmed) {
            continue;
        }
        candidates.push(resolve_asset_url(original, Some(trimmed)));
    }
    candidates
}

/// 返回有效反代前缀列表（String 形式）：`configured` 含任意非空白项时用
/// `configured`（去空白去重），否则用 [`DEFAULT_MIRROR_PREFIXES`] 兜底。
/// 调用方传入 `AppSettings.mirror_prefixes`，把返回值交给 [`build_candidate_urls`]。
pub fn resolve_prefixes(configured: &[String]) -> Vec<String> {
    // review C6：用 HashSet 去重（替代 Vec::dedup 只去连续重复），保留首次出现顺序。
    let mut seen = std::collections::HashSet::new();
    let filtered: Vec<String> = configured
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect();
    if filtered.is_empty() {
        DEFAULT_MIRROR_PREFIXES
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        filtered
    }
}

/// 从反代前缀列表提取所有 host（小写），用于自动合并进 SSRF 白名单。
///
/// review C1：默认 `extra_trusted_hosts` 为空，但 `mirror_prefixes` fallback 到
/// `DEFAULT_MIRROR_PREFIXES`（gh-proxy.com 等）。若不把反代 host 加入白名单，
/// 竞速胜出的反代源会被 `validate_download_url` 拒绝，导致裸连用户无法下载。
/// 解析失败的前缀静默跳过（不阻断下载流程）。
pub fn extract_mirror_hosts(prefixes: &[String]) -> Vec<String> {
    let mut hosts = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for prefix in prefixes {
        let trimmed = prefix.trim().trim_end_matches('/');
        if let Ok(url) = reqwest::Url::parse(trimmed) {
            if let Some(host) = url.host_str() {
                let host = host.to_ascii_lowercase();
                if seen.insert(host.clone()) {
                    hosts.push(host);
                }
            }
        }
    }
    hosts
}

#[cfg(test)]
#[path = "test/mirror.rs"]
mod tests;
