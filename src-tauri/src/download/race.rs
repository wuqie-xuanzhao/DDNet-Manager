//! 多源竞速测速模块：HEAD 淘汰 + Range 5 MiB 测吞吐。
//!
//! 设计目标：把 GitHub 直连、公共反代、官方源放在一起实测吞吐，谁快用谁。
//! 关键取舍——GitHub 直连必须是平等候选而非降级兜底，因为挂了本地代理/VPN
//! 的用户经自己代理直连 GitHub 往往最快最稳。
//!
//! 两段式流程：
//! 1. 阶段1 并发 HEAD 淘汰：不可达 / 不支持 Range / size 不符的候选直接出局。
//! 2. 阶段2 并发 Range 下载前 5 MiB 测真实吞吐，按完成耗时选最快。
//!
//! 测速字节复用为 `.partial` 首段（零浪费）：竞速胜出源返回的 `head_start`
//! 字节由下载层写入 `.part`，正式下载从该 offset 用 Range 续传。

use crate::error::ManagerError;
use tokio_util::sync::CancellationToken;

/// 竞速测速下载前 N 字节用于测吞吐，复用为 `.partial` 首段。
const RACE_PROBE_BYTES: usize = 5 * 1024 * 1024; // 5 MiB
/// HEAD 阶段超时：5s 内不可达即淘汰。
const RACE_HEAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
/// Range 测吞吐阶段硬超时：10s 内未完成 5 MiB 视为太慢，淘汰。
const RACE_RANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// 竞速胜出源：选中 URL + 已下载的首段字节（用于复用为 `.partial`）。
#[derive(Debug)]
pub struct RaceWinner {
    /// 选中的候选 URL。
    pub url: String,
    /// 已下载的首段字节（长度 <= `RACE_PROBE_BYTES`），下载层写入 `.part` 后从该 offset 续传。
    pub head_start: Vec<u8>,
}

/// 对候选源池并发竞速：阶段1 HEAD 淘汰 → 阶段2 Range 测吞吐 → 选最快。
///
/// - 候选 URL 至少 1 个；空列表返回错误。
/// - 所有候选 HEAD 不可达也返回错误。
/// - **SSRF 校验由调用方在竞速前完成**（见 [`crate::download::net::download_asset_to_file`]）：
///   调用方过滤掉私网/不可信 host 后再把安全候选传入。竞速 client 应配置
///   `follow_redirects=false`，避免已校验 host 经 302 重定向到内网；反代通常
///   不重定向，GitHub 原始 URL 若被 302 淘汰由调用方 fallback 到单源直连。
/// - `cancel` 触发时立即中止所有探测任务并返回 `"download canceled"`。
pub async fn select_best_source(
    client: &reqwest::Client,
    candidate_urls: &[String],
    expected_size: u64,
    cancel: Option<&CancellationToken>,
) -> Result<RaceWinner, ManagerError> {
    if candidate_urls.is_empty() {
        return Err(ManagerError::Internal(
            "no candidate urls provided for racing".to_string(),
        ));
    }
    if let Some(token) = cancel {
        if token.is_cancelled() {
            return Err(ManagerError::Internal("download canceled".to_string()));
        }
    }

    let alive = stage1_head_probe(client, candidate_urls, expected_size, cancel).await?;
    if alive.is_empty() {
        return Err(ManagerError::Internal(
            "all candidate sources unreachable or rejected by head probe".to_string(),
        ));
    }

    stage2_range_race(client, &alive, cancel).await
}

/// 阶段1：并发 HEAD 淘汰不可达 / 不支持 Range / size 不符的候选。
///
/// 返回存活候选 URL 列表（按任务完成顺序入列，非原始顺序；stage2 同样按完成
/// 顺序选最快，故顺序不影响竞速结果）。P0 候选数通常 < 8（1 原始 + 4 反代），
/// 不额外加 Semaphore 限流；若未来候选数增长再引入。
async fn stage1_head_probe(
    client: &reqwest::Client,
    candidate_urls: &[String],
    expected_size: u64,
    cancel: Option<&CancellationToken>,
) -> Result<Vec<String>, ManagerError> {
    let mut set = tokio::task::JoinSet::new();
    for url in candidate_urls {
        let url = url.clone();
        let client = client.clone();
        set.spawn(async move {
            let ok = head_probe_once(&client, &url, expected_size).await;
            (url, ok)
        });
    }

    let mut alive = Vec::new();
    loop {
        let join_result = match cancel {
            Some(token) => {
                // review C3：cancel 在 chunk 边界之间也能即时生效，不必等下一个任务完成
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        set.abort_all();
                        return Err(ManagerError::Internal("download canceled".to_string()));
                    }
                    result = set.join_next() => result,
                }
            }
            None => set.join_next().await,
        };
        let Some(join_result) = join_result else {
            break;
        };
        let (url, ok) = join_result
            .map_err(|error| ManagerError::Internal(format!("head probe task failed: {error}")))?;
        if ok {
            alive.push(url);
        }
    }
    Ok(alive)
}

/// 单个候选的 HEAD 探测：可达 + 支持 Range + size 符合。
///
/// 返回 `true` 表示存活。任何错误（连接超时 / TLS 失败 / 非 2xx / 不支持
/// Range / size 不符）都返回 `false`，由调用方统一淘汰。
async fn head_probe_once(client: &reqwest::Client, url: &str, expected_size: u64) -> bool {
    let request = match client
        .request(reqwest::Method::HEAD, url)
        .timeout(RACE_HEAD_TIMEOUT)
        .build()
    {
        Ok(req) => req,
        Err(_) => return false,
    };
    let response = match client.execute(request).await {
        Ok(resp) => resp,
        Err(_) => return false,
    };
    if !response.status().is_success() {
        return false;
    }
    // 必须支持 Range（Accept-Ranges: bytes），否则无法精确测速和续传
    let supports_range = response
        .headers()
        .get(reqwest::header::ACCEPT_RANGES)
        .map(|v| v.to_str().unwrap_or("").eq_ignore_ascii_case("bytes"))
        .unwrap_or(false);
    if !supports_range {
        return false;
    }
    // Content-Length 与基准 size 比对：调用纯函数判定，逻辑见 [`content_length_matches`]。
    if !content_length_matches(response.content_length(), expected_size) {
        return false;
    }
    true
}

/// 判断 HEAD 响应的 Content-Length 是否与期望 size 一致。
///
/// - `None` 或 `0` 视为无法判定（HEAD 响应在某些链路含 mockito 会返回 content-length=0
///   的无 body 传输约定；反代可能不返回该头），放行不淘汰。
/// - `>0` 且与 `expected_size` 不符则判定为不符（淘汰）。
///
/// 抽成纯函数便于直接单测 size 比对逻辑，不依赖 mockito HEAD 行为。
fn content_length_matches(actual: Option<u64>, expected_size: u64) -> bool {
    match actual {
        Some(length) if length > 0 => length == expected_size,
        _ => true,
    }
}

/// 阶段2：对存活候选并发 Range 下载前 5 MiB，选最快完成的。
///
/// JoinSet 按完成顺序返回任务结果，第一个成功的就是最快的。失败任务跳过，
/// 继续等待其他任务；全部失败则返回错误。
async fn stage2_range_race(
    client: &reqwest::Client,
    alive_urls: &[String],
    cancel: Option<&CancellationToken>,
) -> Result<RaceWinner, ManagerError> {
    let mut set = tokio::task::JoinSet::new();
    for url in alive_urls {
        let url = url.clone();
        let client = client.clone();
        set.spawn(async move {
            let result = range_probe_once(&client, &url).await;
            (url, result)
        });
    }

    let mut winner: Option<RaceWinner> = None;
    loop {
        let join_result = match cancel {
            Some(token) => {
                // review C3：cancel 在探测任务 pending 期间也能即时生效
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        set.abort_all();
                        return Err(ManagerError::Internal("download canceled".to_string()));
                    }
                    result = set.join_next() => result,
                }
            }
            None => set.join_next().await,
        };
        let Some(join_result) = join_result else {
            break;
        };
        let (url, result) = join_result
            .map_err(|error| ManagerError::Internal(format!("range probe task failed: {error}")))?;
        match result {
            Ok(bytes) => {
                // 第一个成功的即为胜出（JoinSet 按完成顺序返回），中止其余探测
                set.abort_all();
                winner = Some(RaceWinner {
                    url,
                    head_start: bytes,
                });
                break;
            }
            Err(_) => continue,
        }
    }

    winner.ok_or_else(|| {
        ManagerError::Internal("all candidate sources failed range probe".to_string())
    })
}

/// 单个候选的 Range 测速：下载前 5 MiB（或整个文件，若更小）。
///
/// 用 `response.chunk()` 流式读取并限制字节数，防止服务器忽略 Range 返回整个
/// 文件时下载几十 MB。硬超时 `RACE_RANGE_TIMEOUT` 由请求级 timeout 保证。
async fn range_probe_once(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, ManagerError> {
    let request = client
        .get(url)
        .timeout(RACE_RANGE_TIMEOUT)
        .header(
            reqwest::header::RANGE,
            format!("bytes=0-{}", RACE_PROBE_BYTES - 1),
        )
        .build()
        .map_err(|error| format!("range probe build failed: {error}"))?;
    let response = client
        .execute(request)
        .await
        .map_err(|error| format!("range probe request failed: {error}"))?;
    let mut response = response
        .error_for_status()
        .map_err(|error| format!("range probe response error: {error}"))?;

    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("range probe body read failed: {error}"))?
    {
        buf.extend_from_slice(&chunk);
        if buf.len() >= RACE_PROBE_BYTES {
            buf.truncate(RACE_PROBE_BYTES);
            break;
        }
    }
    Ok(buf)
}

#[cfg(test)]
#[path = "../test/download/race.rs"]
mod tests;
