//! 客户端扫描子命令与辅助逻辑。
//!
//! 把原 commands.rs 里扫描相关代码（scan_clients_via_mft / cancel_scan_clients /
//! ScanCancelState / TauriScanSink / priority_roots 等）拆出来独立成模块，
//! 同时把原 run_scan 的 8 参数封装为 [`ScanConfig`] 结构体，符合 CLAUDE.md
//! "函数参数超过 4 个优先封装为结构体" 规约。
//!
//! lint-WARN(C1): 文件约 641 行 > 600，存量；按项目约定 C1 仅在 > 800 行时强制
//! 拆分，当前规模未触发阈值。扫描策略 + 进度 sink + 优先根目录列表高度内聚，
//! 进一步拆分需要在多个文件间传递 ScanConfig 与 sink 引用。

use crate::error::IpcError;
use crate::models::{ClientHealth, ClientInstallation, ScanClientInstallationsOptions};
use crate::registry::ClientRegistry;
use std::path::PathBuf;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use super::RegistryState;

/// `scan_clients_via_mft` 默认最多收集多少条候选。NTFS 全盘扫很容易超过这个数
/// （多版本/多客户端机器），命中后扫描提前停止；如需放宽，把它做成 settings 字段。
const DEFAULT_SCAN_MAX_RESULTS: usize = 50;

/// `scan_clients_via_mft` 软超时（秒）。普通用户无 Mft/Usn 权限时整盘 Walkdir
/// 较慢，ntfs-search 默认 60s 对 C 盘不够；放宽到 180s 给业务足够时间。
const DEFAULT_SCAN_TIMEOUT_SECS: u64 = 180;

/// 扫描阶段标签，配合 [`ScanPhaseEvent`] 通过 `scan-progress` 通道发到前端，
/// 让 UI 在 ntfs-search 第一条 `drive_started` 到来前就能显示"扫描中"，避免黑屏。
///
/// 业务层事件而非 ntfs_search::ProgressEvent 变体——ntfs-search crate 关注 per-drive
/// 细节，跨阶段的 priority/fallback 切换由 DDNet-Manager 业务层 emit 才合理。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    /// `scan_clients_via_mft` 入口：扫描任务已接收，正在准备 roots（spawn_blocking 阶段）
    Started,
    /// Priority 阶段：扫 Steam / Program Files / 用户目录等常见安装位置
    Priority,
    /// Fallback 阶段：priority 未命中，扩展到全盘
    Fallback,
}

/// 业务层扫描阶段事件包装。和 ntfs_search::ProgressEvent 共用 `scan-progress` 通道，
/// 通过 `kind` 字段做 discriminated union，前端 ScanProgressEvent 类型对应扩展。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScanPhaseEvent {
    /// 进入新扫描阶段。前端可按 phase 区分提示文案（"正在扫描常见位置…" /
    /// "扩展到全盘扫描…"）。
    PhaseStarted { phase: ScanPhase },
}

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

    // 入口立即 emit Started：collect_priority_roots 是同步 spawn_blocking，可能耗
    // 100-300ms；这期间没有 ntfs-search 事件可发，前端会黑屏。先发 PhaseStarted
    // { phase: Started } 让 UI 立刻显示"扫描中"。
    emit_scan_phase(&app, ScanPhase::Started);

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

    let result = run_two_phase_scan(TwoPhaseScanContext {
        registry: &registry,
        app,
        options: &options,
        excluded: &excluded,
        max_results,
        total_timeout,
        priority_roots,
        master_cancel,
    })
    .await;

    cancel_state.clear();
    result
}

/// 两阶段扫描的运行时上下文，封装 [`scan_clients_via_mft`] 内部 async 块的所有参数。
///
/// 把 8 个紧耦合的扫描参数封装为结构体，符合 CLAUDE.md "函数参数超过 4 个优先
/// 封装为结构体" 规约，同时让 [`run_two_phase_scan`] 签名保持单参数。
struct TwoPhaseScanContext<'a> {
    registry: &'a ClientRegistry,
    app: AppHandle,
    options: &'a ScanClientInstallationsOptions,
    excluded: &'a [PathBuf],
    max_results: usize,
    total_timeout: u64,
    priority_roots: Vec<PathBuf>,
    master_cancel: CancellationToken,
}

/// 两阶段扫描核心逻辑：priority 先扫常见安装位置，命中数 < max_results 时 fallback 全盘。
///
/// priority 命中且 health=ok 时自动 upsert 落 registry（B4），让前端 triggerScan
/// 不再需要二次 IPC 调用。fallback 命中同样落库（review issue H1），行为与 priority 一致。
async fn run_two_phase_scan(
    ctx: TwoPhaseScanContext<'_>,
) -> Result<Vec<ClientInstallation>, IpcError> {
    let mut all_installations: Vec<ClientInstallation> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // priority 阶段：先扫常见安装位置，命中秒级返回
    if !ctx.priority_roots.is_empty() {
        let priority_installations = run_scan(
            ScanConfig {
                roots: ctx.priority_roots.clone(),
                excluded: ctx.excluded.to_vec(),
                max_results: ctx.max_results,
                timeout_secs: ctx.total_timeout / 3, // priority 阶段给 1/3 时间预算
                include_unhealthy: ctx.options.include_unhealthy,
                phase: ScanPhase::Priority,
            },
            ctx.registry,
            ctx.app.clone(),
            ctx.master_cancel.clone(),
        )
        .await?;
        for inst in priority_installations {
            if seen_ids.insert(inst.id.clone()) {
                // upsert_scan_hit 内部 health check + 保留已存 is_default（review issue M1）。
                upsert_scan_hit(ctx.registry, &inst, "priority");
                all_installations.push(inst);
            }
        }
        // priority 已找到 max_results 个，提前返回；否则继续 fallback
        if all_installations.len() >= ctx.max_results {
            return Ok(all_installations);
        }
    }

    // fallback 阶段 / 用户显式指定 roots：全盘扫描
    let mut roots: Vec<PathBuf> = if ctx.options.roots.is_empty() {
        collect_default_drive_roots()
    } else {
        ctx.options.roots.iter().map(PathBuf::from).collect()
    };

    if ctx.options.include_saved_paths {
        roots.extend(
            ctx.registry
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
            excluded: ctx.excluded.to_vec(),
            max_results: ctx.max_results,
            timeout_secs: ctx.total_timeout,
            include_unhealthy: ctx.options.include_unhealthy,
            phase: ScanPhase::Fallback,
        },
        ctx.registry,
        ctx.app.clone(),
        ctx.master_cancel.clone(),
    )
    .await?;
    for inst in fallback_installations {
        if seen_ids.insert(inst.id.clone()) {
            upsert_scan_hit(ctx.registry, &inst, "fallback");
            all_installations.push(inst);
        }
    }

    all_installations.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
    Ok(all_installations)
}

/// 单次扫描的用户可配置参数。封装成结构体让 [`run_scan`] 参数数量 ≤ 4
/// （符合 CLAUDE.md "函数参数超过 4 个优先封装为结构体" 规约）。
struct ScanConfig {
    roots: Vec<PathBuf>,
    excluded: Vec<PathBuf>,
    max_results: usize,
    timeout_secs: u64,
    include_unhealthy: bool,
    /// 当前阶段标签，run_scan 入口 emit PhaseStarted 时透传给前端做 UI 区分。
    phase: ScanPhase,
}

/// 单次扫描的封装：构建 opts + 调 find_files + 转 ClientInstallation。
async fn run_scan(
    config: ScanConfig,
    registry: &ClientRegistry,
    app: AppHandle,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<Vec<ClientInstallation>, IpcError> {
    // 入口 emit PhaseStarted：标记进入新阶段。priority → fallback 切换时让前端
    // 看到提示变化（"扩展到全盘扫描…"），同时填补 ntfs-search 启动到第一条
    // drive_started 之间的静默期。
    emit_scan_phase(&app, config.phase.clone());

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

/// 发业务层扫描阶段事件。和 TauriScanSink 共用 `scan-progress` 通道：前端按
/// `kind` 做 discriminated union 区分 ntfs_search 事件 vs 业务层 phase 事件。
///
/// 失败记录到 stderr（与其他 eprintln! 风格一致）：emit 失败一般是前端已卸载
/// 监听，但也可能是序列化错误或 event bus 未初始化，记录后便于排查"黑屏回归"。
fn emit_scan_phase(app: &AppHandle, phase: ScanPhase) {
    use tauri::Emitter;
    if let Err(e) = app.emit("scan-progress", &ScanPhaseEvent::PhaseStarted { phase }) {
        eprintln!("[scan] emit scan-progress PhaseStarted failed: {e}");
    }
}

/// 把扫描命中落 registry（health=ok 时）。priority 和 fallback 共用：
/// - health != ok 跳过（plan B4 风险点 #2：避免残留目录污染）
/// - 已存在的记录保留 is_default（review issue M1：validate_client_dir 总是
///   返回 is_default=false，直接 upsert 会覆盖用户的默认客户端设置）
/// - upsert 失败记录到 stderr 但不阻断扫描
fn upsert_scan_hit(registry: &ClientRegistry, inst: &ClientInstallation, phase_label: &str) {
    if inst.health != ClientHealth::Ok {
        return;
    }
    let to_upsert = match registry.client_installation_by_id(&inst.id) {
        Ok(Some(existing)) if existing.is_default => {
            let mut cloned = inst.clone();
            cloned.is_default = true;
            cloned
        }
        _ => inst.clone(),
    };
    if let Err(e) = registry.upsert_client_installation(&to_upsert) {
        eprintln!(
            "[scan] {} upsert failed for {}: {}",
            phase_label, inst.install_dir, e
        );
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

    /// 测试用 ClientInstallation 构造器：默认 health=ok、is_default=false。
    fn scan_hit_client(id: &str, health: ClientHealth, is_default: bool) -> ClientInstallation {
        use crate::models::{ClientCompatibility, ClientConfidence, ClientInstallSource};
        ClientInstallation {
            id: id.to_string(),
            client_id: "qmclient".to_string(),
            display_name: "QmClient".to_string(),
            install_dir: format!("C:/Games/{id}"),
            executable_path: format!("C:/Games/{id}/DDNet.exe"),
            storage_cfg_path: format!("C:/Games/{id}/storage.cfg"),
            data_dir: format!("C:/Games/{id}/data"),
            user_data_dir: None,
            version: None,
            is_default,
            health,
            missing_items: Vec::new(),
            install_source: ClientInstallSource::Manual,
            confidence: ClientConfidence::Compatible,
            manager_owned: false,
            compatibility: ClientCompatibility::default(),
            upstream_url: None,
            pe_company_name: None,
            pe_product_name: None,
            pe_file_version: None,
            exe_sha256: None,
            last_scanned_at: None,
        }
    }

    /// 测试用临时 ClientRegistry：用 tempfile 创建独立 sqlite db 避免污染。
    fn temp_registry() -> ClientRegistry {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        // temp_dir 在函数返回时 drop 会清理目录，但 sqlite 文件需保持存活到测试结束。
        // 用 Box::leak 让 temp_dir 永久存活，单测内存开销可接受。
        let leaked = Box::leak(Box::new(temp_dir));
        crate::registry::ClientRegistry::open(&leaked.path().join("scan-test.sqlite"))
            .expect("注册表应打开成功")
    }

    #[test]
    fn upsert_scan_hit_skips_unhealthy_to_avoid_registry_pollution() {
        let registry = temp_registry();
        // health 非 ok 的扫描命中不应落库（plan B4 风险点 #2：残留目录污染）
        let broken = scan_hit_client("broken-1", ClientHealth::MissingExecutable, false);
        upsert_scan_hit(&registry, &broken, "priority");
        let clients = registry.list_client_installations().expect("读取应成功");
        assert!(clients.is_empty(), "health 非 ok 的命中不应落库");
    }

    #[test]
    fn upsert_scan_hit_first_time_writes_is_default_false() {
        let registry = temp_registry();
        let hit = scan_hit_client("new-1", ClientHealth::Ok, false);
        upsert_scan_hit(&registry, &hit, "priority");
        let stored = registry
            .client_installation_by_id("new-1")
            .expect("查询应成功")
            .expect("新命中应已落库");
        assert!(!stored.is_default, "首次落库 is_default 应为 false");
    }

    #[test]
    fn upsert_scan_hit_preserves_existing_is_default() {
        // review issue M1：validate_client_dir 总是返回 is_default=false，
        // upsert_scan_hit 必须先查现有记录继承 is_default，否则用户的默认
        // 客户端设置会被 priority 扫描无声覆盖。
        let registry = temp_registry();

        // 先把 new-1 设为 default
        let mut initial = scan_hit_client("preserved-1", ClientHealth::Ok, false);
        registry
            .upsert_client_installation(&initial)
            .expect("初始写入应成功");
        registry
            .set_default_client("preserved-1")
            .expect("设置默认应成功");
        let stored = registry
            .client_installation_by_id("preserved-1")
            .expect("查询应成功")
            .expect("应存在");
        assert!(stored.is_default, "前置：设置默认应生效");

        // 模拟再次扫描命中（validate_client_dir 返回 is_default=false），
        // upsert_scan_hit 应保留 is_default=true 不覆盖。
        initial.version = Some("1.2.3".to_string()); // 模拟扫描后字段更新
        upsert_scan_hit(&registry, &initial, "priority");

        let after = registry
            .client_installation_by_id("preserved-1")
            .expect("查询应成功")
            .expect("应存在");
        assert!(after.is_default, "is_default 应保留为 true 不被覆盖");
        assert_eq!(
            after.version.as_deref(),
            Some("1.2.3"),
            "其他字段应正常更新"
        );
    }
}
