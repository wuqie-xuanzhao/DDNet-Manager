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
