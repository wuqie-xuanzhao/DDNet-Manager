//! 客户端扫描子命令与辅助逻辑。
//!
//! 把原 commands.rs 里扫描相关代码（scan_clients_via_mft / cancel_scan_clients /
//! ScanCancelState / TauriScanSink / priority_roots 等）拆出来独立成模块，
//! 同时把原 run_scan 的 8 参数封装为 [`ScanConfig`] 结构体，符合 CLAUDE.md
//! "函数参数超过 4 个优先封装为结构体" 规约。

use crate::error::IpcError;
use crate::models::{ClientInstallation, ScanClientInstallationsOptions};
use crate::registry::ClientRegistry;
use std::path::PathBuf;
use tauri::AppHandle;

use super::RegistryState;

/// `scan_clients_via_mft` 默认最多收集多少条候选。NTFS 全盘扫很容易超过这个数
/// （多版本/多客户端机器），命中后扫描提前停止；如需放宽，把它做成 settings 字段。
const DEFAULT_SCAN_MAX_RESULTS: usize = 50;

/// `scan_clients_via_mft` 软超时（秒）。普通用户无 Mft/Usn 权限时整盘 Walkdir
/// 较慢，ntfs-search 默认 60s 对 C 盘不够；放宽到 180s 给业务足够时间。
const DEFAULT_SCAN_TIMEOUT_SECS: u64 = 180;

/// 扫描取消 token 的全局共享状态。
///
/// `scan_clients_via_mft` 开始时存入 master token，结束时清理；
/// `cancel_scan_clients` command 拿 token 调 cancel() 让正在跑的扫描尽快返回。
#[derive(Default)]
pub struct ScanCancelState(pub std::sync::Mutex<Option<tokio_util::sync::CancellationToken>>);

impl ScanCancelState {
    fn set(&self, token: tokio_util::sync::CancellationToken) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(token);
        }
    }

    fn clear(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
        }
    }

    /// 触发当前扫描的取消。返回 false 表示当前没有扫描在跑。
    fn cancel(&self) -> bool {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(token) = guard.take() {
                token.cancel();
                return true;
            }
        }
        false
    }
}

/// 取消正在进行的 scan_clients_via_mft 扫描。返回是否成功取消。
#[tauri::command]
pub fn cancel_scan_clients(state: tauri::State<'_, ScanCancelState>) -> Result<bool, IpcError> {
    Ok(state.cancel())
}

/// 使用 ntfs-search crate 全量扫盘找 DDNet.exe 兼容客户端。
///
/// 后端自动按平台/权限选 Mft / Usn / Walkdir（admin > 普通 > fallback），失败自动降级。
/// 扫描期间实时 emit `scan-progress` 事件（[`ntfs_search::ProgressEvent`]），
/// 前端按 `kind` discriminated union 渲染进度。
///
/// **两阶段扫描**：用户未显式指定 roots 时，先扫 priority（Steam / Program Files /
/// 用户目录），命中秒级返回；未命中再 fallback 全盘。这样典型用户场景（Steam
/// 安装的 DDNet）只需扫少数子树，避免大盘（HDD 几 T）长时间扫描。
#[tauri::command]
pub async fn scan_clients_via_mft(
    registry: RegistryState<'_>,
    cancel_state: tauri::State<'_, ScanCancelState>,
    options: Option<ScanClientInstallationsOptions>,
    app: AppHandle,
) -> Result<Vec<ClientInstallation>, IpcError> {
    let options = options.unwrap_or_default();
    let settings = registry.load_app_settings()?;

    let excluded: Vec<PathBuf> = settings
        .scan_excluded_paths
        .iter()
        .map(PathBuf::from)
        .collect();

    let max_results = settings
        .scan_max_results
        .unwrap_or(DEFAULT_SCAN_MAX_RESULTS);
    let total_timeout = settings
        .scan_timeout_secs
        .unwrap_or(DEFAULT_SCAN_TIMEOUT_SECS);

    // master cancel token：priority 和 fallback 阶段共享；存到全局 state 让
    // cancel_scan_clients command 能从外部触发取消。
    let master_cancel = tokio_util::sync::CancellationToken::new();
    cancel_state.set(master_cancel.clone());

    // collect_priority_roots 含 30-50 次同步 is_dir，移到 spawn_blocking 不阻塞 executor
    let priority_roots = if options.roots.is_empty() {
        tokio::task::spawn_blocking(collect_priority_roots)
            .await
            .map_err(|e| {
                crate::error::ManagerError::Internal(format!("priority_roots join: {e}"))
            })?
    } else {
        Vec::new()
    };

    let result = async {
        let mut all_installations: Vec<ClientInstallation> = Vec::new();
        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        // 两阶段扫描：priority 先扫常见安装位置，命中数 < max_results 时继续 fallback 找全
        if !priority_roots.is_empty() {
            let priority_installations = run_scan(
                ScanConfig {
                    roots: priority_roots,
                    excluded: excluded.clone(),
                    max_results,
                    timeout_secs: total_timeout / 3, // priority 阶段给 1/3 时间预算
                    include_unhealthy: options.include_unhealthy,
                },
                &registry,
                app.clone(),
                master_cancel.clone(),
            )
            .await?;
            for inst in priority_installations {
                if seen_ids.insert(inst.id.clone()) {
                    all_installations.push(inst);
                }
            }
            // priority 已找到 max_results 个，提前返回；否则继续 fallback
            if all_installations.len() >= max_results {
                return Ok(all_installations);
            }
        }

        // Fallback / 用户显式指定 roots：全盘扫描
        let mut roots: Vec<PathBuf> = if options.roots.is_empty() {
            collect_default_drive_roots()
        } else {
            options.roots.iter().map(PathBuf::from).collect()
        };

        if options.include_saved_paths {
            roots.extend(
                registry
                    .list_client_installations()?
                    .into_iter()
                    .filter_map(|c| {
                        PathBuf::from(c.install_dir)
                            .parent()
                            .map(std::path::Path::to_path_buf)
                    }),
            );
        }

        let fallback_installations = run_scan(
            ScanConfig {
                roots,
                excluded: excluded.clone(),
                max_results,
                timeout_secs: total_timeout,
                include_unhealthy: options.include_unhealthy,
            },
            &registry,
            app,
            master_cancel.clone(),
        )
        .await?;
        for inst in fallback_installations {
            if seen_ids.insert(inst.id.clone()) {
                all_installations.push(inst);
            }
        }

        all_installations.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
        Ok(all_installations)
    }
    .await;

    cancel_state.clear();
    result
}

/// 单次扫描的用户可配置参数。封装成结构体让 [`run_scan`] 参数数量 ≤ 4
/// （符合 CLAUDE.md "函数参数超过 4 个优先封装为结构体" 规约）。
struct ScanConfig {
    roots: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
    max_results: usize,
    timeout_secs: u64,
    include_unhealthy: bool,
}

/// 单次扫描的封装：构建 opts + 调 find_files + 转 ClientInstallation。
async fn run_scan(
    config: ScanConfig,
    registry: &ClientRegistry,
    app: AppHandle,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<Vec<ClientInstallation>, IpcError> {
    let opts = ntfs_search::NtfsScanOptions::new(|name| {
        ["DDNet.exe", "ddnet.exe"]
            .iter()
            .any(|expected| name.eq_ignore_ascii_case(expected))
    })
    .with_roots(config.roots)
    .with_max_results(config.max_results)
    .with_timeout(std::time::Duration::from_secs(config.timeout_secs));

    let progress = std::sync::Arc::new(TauriScanSink::new(app));

    let entries = ntfs_search::find_files(opts, progress, cancel)
        .await
        .map_err(crate::error::ManagerError::from)?;

    let mut installations: Vec<ClientInstallation> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in entries {
        let Some(parent) = entry.path.parent() else {
            continue;
        };
        if crate::client_scan::is_local_smoke_tmp_path(parent) {
            continue;
        }
        if config.excluded.iter().any(|ex| {
            crate::client_scan::normalize_for_compare(ex)
                == crate::client_scan::normalize_for_compare(parent)
        }) {
            continue;
        }
        let mut installation = crate::client_scan::validate_client_dir(parent)?;
        // 默认过滤残缺客户端（缺 data 目录、storage.cfg 等）。这类候选通常是
        // QQ 文件夹里的 ddnet.exe 单文件、开发仓库里的 build 产物等噪音。
        // 前端"显示残缺客户端"开关透传到 include_unhealthy。
        if !config.include_unhealthy && installation.health != crate::models::ClientHealth::Ok {
            continue;
        }
        // 用 registry 指纹库升级识别：exe_sha256 命中用户下载记录时覆盖路径/PE
        // 匹配结果，置信度升到 Verified（不可伪造）。exe_sha256 为 None 时跳过。
        upgrade_identity_with_registry_fingerprint(&mut installation, registry)?;
        if seen_ids.insert(installation.id.clone()) {
            installations.push(installation);
        }
    }

    installations.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
    Ok(installations)
}

/// 用 registry 中的用户下载指纹升级识别结果。命中时覆盖 client_id/display_name/
/// version/upstream_url/confidence。失败不阻断扫描（指纹缺失只让识别降级）。
fn upgrade_identity_with_registry_fingerprint(
    client: &mut ClientInstallation,
    registry: &ClientRegistry,
) -> Result<(), IpcError> {
    let Some(hash) = client.exe_sha256.as_ref() else {
        return Ok(());
    };
    let fp = registry
        .lookup_fingerprint_by_hash(hash)
        .map_err(IpcError::from)?;
    if let Some(fp) = fp {
        client.client_id = fp.client_id;
        client.display_name = fp.display_name;
        if let Some(version) = fp.version {
            client.version = Some(version);
        }
        client.confidence = crate::models::ClientConfidence::Verified;
        if let Some(entry) = crate::client_catalog::catalog_entry_by_id(&client.client_id) {
            client.upstream_url = entry.upstream_url.map(str::to_string);
        }
    }
    Ok(())
}

/// Priority roots：DDNet 客户端最可能安装的位置。典型用户场景命中即返回，
/// 避免大盘（HDD 几 T）长时间扫描。
///
/// 包含：
/// - Steam library（默认 Program Files + 各盘符根下 \Steam）
/// - Program Files / Program Files (x86)
/// - 用户目录（Downloads / Desktop / Documents / Games）
/// - LOCALAPPDATA（部分客户端装这里）
/// - 各盘 \Games 子目录（玩家常用）
fn collect_priority_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let push = |roots: &mut Vec<PathBuf>, p: PathBuf| {
        if p.is_dir() && !roots.contains(&p) {
            roots.push(p);
        }
    };

    // Steam libraries：默认安装位置 + 各盘符根下 \Steam
    push(
        &mut roots,
        PathBuf::from(r"C:\Program Files (x86)\Steam")
            .join("steamapps")
            .join("common"),
    );
    push(
        &mut roots,
        PathBuf::from(r"C:\Program Files\Steam")
            .join("steamapps")
            .join("common"),
    );
    for letter in b'C'..=b'Z' {
        push(
            &mut roots,
            PathBuf::from(format!("{}:\\Steam", letter as char))
                .join("steamapps")
                .join("common"),
        );
    }

    // Program Files
    for env in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(p) = std::env::var_os(env) {
            push(&mut roots, PathBuf::from(p));
        }
    }

    // User dirs
    if let Some(p) = std::env::var_os("USERPROFILE") {
        let user = PathBuf::from(p);
        for sub in ["Downloads", "Desktop", "Documents", "Games"] {
            push(&mut roots, user.join(sub));
        }
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        push(&mut roots, PathBuf::from(p));
    }

    // 各盘 \Games 子目录（玩家常用）
    for letter in b'C'..=b'Z' {
        push(
            &mut roots,
            PathBuf::from(format!("{}:\\Games", letter as char)),
        );
    }

    roots
}

/// 把 ntfs-search 的 ProgressEvent 转 Tauri event 推到前端。
struct TauriScanSink {
    app: AppHandle,
}

impl TauriScanSink {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ntfs_search::ProgressSink for TauriScanSink {
    fn emit(&self, event: ntfs_search::ProgressEvent) {
        use tauri::Emitter;
        let _ = self.app.emit("scan-progress", &event);
    }
}

/// Windows 默认固定盘符 roots（C: 永远在，D-Z 按存在性添加）。
fn collect_default_drive_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("C:\\")];
    #[cfg(windows)]
    {
        for letter in b'D'..=b'Z' {
            let path = PathBuf::from(format!("{}:\\", letter as char));
            if path.exists() {
                roots.push(path);
            }
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_cancel_state_cancel_returns_false_when_empty() {
        let state = ScanCancelState::default();
        assert!(!state.cancel());
    }

    #[test]
    fn scan_cancel_state_cancel_returns_true_after_set() {
        let state = ScanCancelState::default();
        let token = tokio_util::sync::CancellationToken::new();
        state.set(token);
        assert!(state.cancel());
        // 二次 cancel 应 false（token 已被 take）
        assert!(!state.cancel());
    }

    #[test]
    fn scan_cancel_state_clear_drops_token_without_firing() {
        let state = ScanCancelState::default();
        let token = tokio_util::sync::CancellationToken::new();
        let cloned = token.clone();
        state.set(token);
        state.clear();
        assert!(!state.cancel());
        // token 自身未被 cancel，外部 cloned 仍然 active
        assert!(!cloned.is_cancelled());
    }

    #[test]
    fn collect_priority_roots_does_not_panic() {
        // 不验证具体内容（依赖运行机器环境），只确保不 panic 且返回 Vec
        let _roots = collect_priority_roots();
    }

    #[test]
    fn collect_priority_roots_includes_program_files_when_present() {
        let roots = collect_priority_roots();
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            assert!(
                roots.iter().any(|r| r.as_os_str() == pf),
                "ProgramFiles 应在 priority roots 中"
            );
        }
    }
}
