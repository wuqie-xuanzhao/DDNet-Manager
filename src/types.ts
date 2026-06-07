export type LauncherState =
  | "unconfigured"
  | "validating"
  | "ready"
  | "launching"
  | "running"
  | "error";

export type ClientHealth =
  | "ok"
  | "missing_executable"
  | "missing_storage_cfg"
  | "missing_data_dir";

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
  last_scanned_at: string | null;
};

export type ScanClientInstallationsOptions = {
  roots?: string[];
  include_saved_paths?: boolean;
  deep?: boolean;
};

export type UpsertClientInstallationRequest = {
  install_dir: string;
  is_default?: boolean;
};

export type NetworkRouteMode = "direct" | "proxy_prefix" | "mirror_template";

export type NetworkRouteConfig = {
  mode: NetworkRouteMode;
  proxy_prefix_url?: string | null;
  mirror_template?: string | null;
  enabled_hosts?: string[];
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

export type ClientUpdateCheck = {
  client_id: string;
  channel: string;
  current_version: string | null;
  latest_version: string;
  asset: UpdateAsset;
  needs_update: boolean;
};

export type CheckClientUpdateRequest = {
  client_id: string;
  channel: string;
  /** 项目自维护 manifest 地址；业务调用必须显式传入。 */
  manifest_url?: string | null;
  platform?: string | null;
  network_route?: NetworkRouteConfig | null;
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

export type StartUpdateDownloadRequest = {
  client_installation_id: string;
  channel: string;
  /** 项目自维护 manifest 地址；业务调用必须显式传入。 */
  manifest_url?: string | null;
  platform?: string | null;
  network_route?: NetworkRouteConfig | null;
};

export type BindRecord = {
  key: string;
  command: string;
  source_file: string;
  line: number;
  managed_by_manager: boolean;
  matched_workshop_id: string | null;
};

export type CfgExecRecord = {
  target: string;
  source_file: string;
  line: number;
  resolved_path: string | null;
  missing: boolean;
};

export type CfgUnbindRecord = {
  key: string;
  source_file: string;
  line: number;
};

export type BindConflict = {
  key: string;
  records: BindRecord[];
};

export type CfgAnalysis = {
  binds: BindRecord[];
  unbinds: CfgUnbindRecord[];
  execs: CfgExecRecord[];
  conflicts: BindConflict[];
  missing_exec_targets: CfgExecRecord[];
};

export type WorkshopBind = {
  id: string;
  category: string;
  title: string;
  command: string;
  description: string;
  command_variants: string[];
  variant_labels: string[];
  is_bindable: boolean;
};
