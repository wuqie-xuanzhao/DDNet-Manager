export type LauncherState =
  | "unconfigured"
  | "validating"
  | "ready"
  | "launching"
  | "running"
  | "error";

/// catalog 中按平台分组的可执行文件候选。镜像 Rust `PlatformExecutableCandidates`。
export type PlatformExecutableCandidates = {
  windows: string[];
  macos: string[];
  linux: string[];
};

/// 客户端更新来源。镜像 Rust `UpdateSourceDescriptor`（serde tag = "kind"）。
export type UpdateSourceDescriptor =
  | { kind: "github_release"; owner: string; repo: string; windows_assets: string[]; macos_assets: string[]; linux_assets: string[] }
  | { kind: "ddnet_official" }
  | { kind: "website"; url: string }
  | { kind: "none" };

/// 内置客户端 catalog 条目。镜像 Rust `ClientCatalogEntry`。
export type ClientCatalogEntry = {
  client_id: string;
  display_name: string;
  aliases: string[];
  executable_candidates: PlatformExecutableCandidates;
  required_markers: string[];
  pe_company_names: string[];
  pe_product_names: string[];
  /// (version, sha256_hex) 元组数组。初始为空，等用户填实际 release hash。
  known_hashes: [string, string][];
  update_source: UpdateSourceDescriptor;
  upstream_url: string | null;
};

/// 磁盘探测结果。镜像 Rust `DiskProbe`。
export type DiskProbe = {
  free_bytes: number;
  total_bytes: number;
  /** 是否 SSD。null 表示平台不支持判断或 sysinfo 未识别（NAS / 网络盘等）。 */
  is_ssd: boolean | null;
  mount_point: string;
};

/// 快捷方式创建请求。镜像 Rust `CreateShortcutsRequest`。
export type CreateShortcutsRequest = {
  executable_path: string;
  working_dir: string;
  display_name: string;
  desktop: boolean;
  start_menu: boolean;
};

export type ClientTypeId =
  | "qmclient"
  | "ddnet"
  | "qmclient-nightly"
  | "taterclient"
  | "bestclient"
  | "cactusclient";

export type ClientHealth =
  | "ok"
  | "missing_executable"
  | "missing_storage_cfg"
  | "missing_data_dir";

export type ClientInstallSource = "official_download" | "steam" | "manual" | "manager";

export type ClientConfidence = "verified" | "compatible" | "partial" | "unsupported";

export type CompatibilityStatus = "supported" | "unsupported" | "risky" | "unknown" | "verified";

export type CompatibilityReason = {
  code: string;
  message: string;
};

export type ClientCompatibility = {
  status: CompatibilityStatus;
  can_launch: boolean;
  launch_verified: boolean;
  reasons: CompatibilityReason[];
  last_launch_result: string | null;
};

export type ClientInstallation = {
  id: string;
  client_id: string;
  display_name: string;
  install_dir: string;
  executable_path: string;
  storage_cfg_path: string;
  data_dir: string;
  user_data_dir: string | null;
  version: string | null;
  is_default: boolean;
  health: ClientHealth;
  missing_items: string[];
  install_source: ClientInstallSource;
  confidence: ClientConfidence;
  manager_owned: boolean;
  compatibility: ClientCompatibility;
  upstream_url: string | null;
  /** PE VS_VERSION_INFO 的 CompanyName（识别发行方）。非 PE / 解析失败为 null。 */
  pe_company_name: string | null;
  /** PE VS_VERSION_INFO 的 ProductName。 */
  pe_product_name: string | null;
  /** PE VS_VERSION_INFO 的 FileVersion。 */
  pe_file_version: string | null;
  /** 可执行文件 SHA-256（小写十六进制）。仅 ≤ 1 GB 文件才计算。 */
  exe_sha256: string | null;
  last_scanned_at: string | null;
};

export type LaunchReadiness = {
  client: ClientInstallation | null;
  can_launch: boolean;
  running: boolean;
  status_label: string;
  user_message: string;
  blocking_reasons: string[];
  checked_at: string | null;
};

export type ScanClientInstallationsOptions = {
  roots?: string[];
  include_saved_paths?: boolean;
  /** 是否包含 health != Ok 的残缺客户端（缺 data 目录、storage.cfg 等）。默认 false。 */
  include_unhealthy?: boolean;
};

export type UpsertClientInstallationRequest = {
  install_dir: string;
  is_default?: boolean;
};

export type NetworkRouteMode = "direct" | "auto_detect" | "local_proxy";

export type NetworkRouteConfig = {
  mode: NetworkRouteMode;
  local_proxy_url?: string | null;
};

export type AppSettings = {
  network_route: NetworkRouteConfig | null;
  scan_excluded_paths: string[];
  scan_max_results: number | null;
  scan_timeout_secs: number | null;
  close_panel_after_launch: boolean;
  auto_check_updates: boolean;
  autostart: boolean;
  exit_game_show_launcher: boolean;
  close_behavior: string;
  allow_silent_update: boolean;
  /** 用户显式信任的额外下载 host（公共反代域名），对应后端 SSRF 白名单动态放行。 */
  extra_trusted_hosts: string[];
  /** 反代前缀列表；空时后端用 DEFAULT_MIRROR_PREFIXES 兜底。 */
  mirror_prefixes: string[];
};

export type LocalSmokeResultStatus = "succeeded" | "failed";

export type LocalSmokeResultReport = {
  status: LocalSmokeResultStatus;
  stage: string;
  message?: string | null;
};

export type LocalSmokeAutomationConfig = {
  clientInstallDir: string;
  manifestUrl: string;
  closeWindowOnFinish: boolean;
};

export type InstallHistoryStatus = "completed" | "failed" | "rolled_back";

export type InstallHistoryRecord = {
  id: string;
  job_id: string;
  client_installation_id: string;
  client_id: string;
  version: string;
  asset_url: string;
  package_kind: string;
  status: InstallHistoryStatus;
  rollback_path: string | null;
  error: string | null;
  completed_at: string | null;
};

export type UpdateAsset = {
  platform: string;
  asset_url: string;
  sha256: string;
  size: number;
};

export type ManifestClient = {
  client_id: string;
  channel: string;
  version: string;
  release_notes: string;
  assets: UpdateAsset[];
};

export type UpdateManifest = {
  schema_version: number;
  clients: ManifestClient[];
};

export type UpdateCheckReason =
  | "client_not_in_catalog"
  | "no_release_for_channel"
  | "no_asset_for_platform"
  | "auto_update_disabled"
  | "manifest_entry_missing"
  | "none";

export type ClientUpdateCheck = {
  client_id: string;
  channel: string;
  current_version: string | null;
  latest_version: string;
  asset: UpdateAsset;
  needs_update: boolean;
  source_kind:
    | "github_release"
    | "website"
    | "ddnet_official"
    | "manifest"
    | "none";
  action: "download" | "open_url" | "none";
  action_url: string | null;
  message: string | null;
  /**
   * 更新检查无法提供自动更新动作的具体原因。
   * `action === "none"` 时：`reason === "none"` 表示确为已是最新版，
   * 其他值表示检查不可用（catalog 无条目/无 release/无资产/不支持自动更新/manifest 无条目）。
   * 前端据此区分"已是最新版"与"无法检查"，避免误判。
   */
  reason: UpdateCheckReason;
};

export type CheckClientUpdateRequest = {
  client_id: string;
  channel: string;
  /** 项目自维护 manifest 地址；业务调用必须显式传入。 */
  manifest_url?: string | null;
  platform?: string | null;
  network_route?: NetworkRouteConfig | null;
  use_manifest_source?: boolean;
};

export type DownloadJobStatus =
  | "pending"
  | "downloading"
  | "verified"
  | "installing"
  | "completed"
  | "canceled"
  | "failed";

export type DownloadJob = {
  id: string;
  client_installation_id: string;
  client_id: string;
  channel: string;
  version: string;
  asset_url: string;
  sha256: string;
  size: number;
  status: DownloadJobStatus;
  downloaded_bytes: number;
  cache_path: string;
  error: string | null;
};

export type DownloadCacheState = "missing" | "present" | "verified" | "corrupted";

export type DownloadJobRecovery = {
  job: DownloadJob;
  cache_state: DownloadCacheState;
  can_install: boolean;
  can_retry: boolean;
  user_message: string;
};

export type StartUpdateDownloadRequest = {
  client_installation_id: string;
  channel: string;
  /** 项目自维护 manifest 地址；业务调用必须显式传入。 */
  manifest_url?: string | null;
  platform?: string | null;
  network_route?: NetworkRouteConfig | null;
  use_manifest_source?: boolean;
};

/** 后端 IPC 结构化错误契约，携带稳定错误码与可读文案。 */
export type IpcError = {
  /** 稳定错误码，与后端 `models.rs` 的 `IPC_ERROR_*` 常量对齐。 */
  code: string;
  /** 面向调试的原始信息，前端可兜底展示。 */
  message: string;
};

export type AppUpdateCheck = {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  release_url: string;
  release_notes: string | null;
};

