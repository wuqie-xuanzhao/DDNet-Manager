use crate::models::NetworkRouteConfig;
use std::time::Instant;
use tokio::task::JoinSet;

/// 表示一个可探测的网络路由候选。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkRouteCandidate {
    /// 候选名称，用于错误聚合和前端诊断。
    pub name: String,
    /// 具体路由配置；为空表示直连。
    pub route: Option<NetworkRouteConfig>,
}

impl NetworkRouteCandidate {
    /// 创建直连候选。
    pub fn direct() -> Self {
        Self {
            name: "direct".to_string(),
            route: None,
        }
    }

    /// 创建带名称的显式路由候选。
    pub fn named(name: impl Into<String>, route: NetworkRouteConfig) -> Self {
        Self {
            name: name.into(),
            route: Some(route),
        }
    }

    /// 构造用于探测 manifest 的最终 URL，并复用 manifest 层安全校验。
    pub fn manifest_probe_url(&self, manifest_url: &str) -> Result<reqwest::Url, String> {
        crate::manifest::build_manifest_url_with_route(manifest_url, self.route.as_ref())
    }
}

/// 表示一次路由候选探测结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkRouteProbeOutcome {
    candidate: NetworkRouteCandidate,
    elapsed_ms: Option<u64>,
    error: Option<String>,
}

impl NetworkRouteProbeOutcome {
    /// 创建成功的路由探测结果。
    pub fn success(candidate: NetworkRouteCandidate, elapsed_ms: u64) -> Self {
        Self {
            candidate,
            elapsed_ms: Some(elapsed_ms),
            error: None,
        }
    }

    /// 创建失败的路由探测结果。
    pub fn failure(candidate: NetworkRouteCandidate, error: impl Into<String>) -> Self {
        Self {
            candidate,
            elapsed_ms: None,
            error: Some(error.into()),
        }
    }
}

/// 表示从候选探测结果中选出的网络路由。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedNetworkRoute {
    /// 候选名称。
    pub name: String,
    /// 实际使用的路由；为空表示直连。
    pub route: Option<NetworkRouteConfig>,
    /// 探测耗时，单位毫秒。
    pub elapsed_ms: u64,
}

/// 从探测结果中选择最快的成功路由，全部失败时返回聚合错误。
pub fn select_fastest_successful_route(
    outcomes: Vec<NetworkRouteProbeOutcome>,
) -> Result<SelectedNetworkRoute, String> {
    let mut failures = Vec::new();
    let mut selected: Option<SelectedNetworkRoute> = None;

    for outcome in outcomes {
        match (outcome.elapsed_ms, outcome.error) {
            (Some(elapsed_ms), None) => {
                let candidate = outcome.candidate;
                let next = SelectedNetworkRoute {
                    name: candidate.name,
                    route: candidate.route,
                    elapsed_ms,
                };
                if selected
                    .as_ref()
                    .is_none_or(|current| next.elapsed_ms < current.elapsed_ms)
                {
                    selected = Some(next);
                }
            }
            (_, Some(error)) => {
                failures.push(format!("{}: {error}", outcome.candidate.name));
            }
            (None, None) => {
                failures.push(format!("{}: unknown error", outcome.candidate.name));
            }
        }
    }

    selected.ok_or_else(|| {
        if failures.is_empty() {
            "no network route probe outcomes".to_string()
        } else {
            failures.join("; ")
        }
    })
}

/// 使用候选路由真实拉取并解析 manifest，返回可参与选择的探测结果。
pub async fn probe_manifest_route(
    manifest_url: &str,
    candidate: NetworkRouteCandidate,
) -> NetworkRouteProbeOutcome {
    let start = Instant::now();
    match crate::manifest::fetch_manifest_with_route(manifest_url, candidate.route.as_ref()).await {
        Ok(_) => NetworkRouteProbeOutcome::success(candidate, elapsed_millis(start)),
        Err(error) => NetworkRouteProbeOutcome::failure(candidate, error),
    }
}

/// 并行探测一组 manifest 路由候选，并返回最先完成的成功路由。
pub async fn select_manifest_route(
    manifest_url: &str,
    candidates: Vec<NetworkRouteCandidate>,
) -> Result<SelectedNetworkRoute, String> {
    let mut tasks = JoinSet::new();
    for candidate in candidates {
        let manifest_url = manifest_url.to_string();
        tasks.spawn(async move { probe_manifest_route(&manifest_url, candidate).await });
    }

    let mut failures = Vec::new();
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(outcome) => match (outcome.elapsed_ms, outcome.error) {
                (Some(elapsed_ms), None) => {
                    let candidate = outcome.candidate;
                    tasks.abort_all();
                    return Ok(SelectedNetworkRoute {
                        name: candidate.name,
                        route: candidate.route,
                        elapsed_ms,
                    });
                }
                (_, Some(error)) => {
                    failures.push(format!("{}: {error}", outcome.candidate.name));
                }
                (None, None) => {
                    failures.push(format!("{}: unknown error", outcome.candidate.name));
                }
            },
            Err(error) => failures.push(format!("probe-task: route probe task failed: {error}")),
        }
    }

    if failures.is_empty() {
        Err("no network route probe outcomes".to_string())
    } else {
        Err(failures.join("; "))
    }
}

fn elapsed_millis(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "test/network_route.rs"]
mod tests;
