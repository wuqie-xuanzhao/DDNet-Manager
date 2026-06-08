import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  CheckClientUpdateRequest,
  ClientInstallation,
  ClientUpdateCheck,
  DownloadJob,
  DownloadJobRecovery,
  InstallHistoryRecord,
  LaunchReadiness,
  LocalSmokeResultReport,
  NetworkRouteConfig,
  ScanClientInstallationsOptions,
  StartUpdateDownloadRequest,
  UpdateManifest,
  UpsertClientInstallationRequest
} from "../types";

export function isTauriRuntime() {
  if (typeof window === "undefined") {
    return false;
  }

  const tauriWindow = window as Window & {
    __TAURI_INTERNALS__?: {
      invoke?: unknown;
    };
  };

  return typeof tauriWindow.__TAURI_INTERNALS__?.invoke === "function";
}

export function validateClientDir(path: string): Promise<ClientInstallation> {
  return invoke<ClientInstallation>("validate_client_dir", { path });
}

export function scanClientInstallations(options?: ScanClientInstallationsOptions): Promise<ClientInstallation[]> {
  return invoke<ClientInstallation[]>("scan_client_installations", { options });
}

export function upsertClientInstallation(request: UpsertClientInstallationRequest): Promise<ClientInstallation> {
  return invoke<ClientInstallation>("upsert_client_installation", { request });
}

export function removeClientInstallation(id: string): Promise<void> {
  return invoke<void>("remove_client_installation", { id });
}

export function setDefaultClient(id: string): Promise<void> {
  return invoke<void>("set_default_client", { id });
}

export function listClientInstallations(): Promise<ClientInstallation[]> {
  return invoke<ClientInstallation[]>("list_client_installations");
}

export function getDefaultClient(): Promise<ClientInstallation | null> {
  return invoke<ClientInstallation | null>("get_default_client");
}

export async function getLaunchReadiness(): Promise<LaunchReadiness> {
  const client = await getDefaultClient();
  const checkedAt = new Date().toISOString();

  if (!client) {
    return {
      client: null,
      can_launch: false,
      running: false,
      status_label: "未设置",
      user_message: "尚未设置默认客户端，请先定位并保存一个客户端。",
      blocking_reasons: ["没有默认客户端记录"],
      checked_at: checkedAt
    };
  }

  const blockingReasons: string[] = [];
  if (client.health !== "ok") {
    blockingReasons.push(...(client.missing_items.length > 0 ? client.missing_items : [client.health]));
  }
  if (!client.compatibility.can_launch) {
    blockingReasons.push(...(client.compatibility.reasons.length > 0 ? client.compatibility.reasons.map((reason) => reason.message) : ["当前机器兼容性未通过"]));
  }

  let running = false;
  if (client.health === "ok") {
    try {
      running = await isClientRunning(client.executable_path);
    } catch (error) {
      blockingReasons.push(error instanceof Error ? error.message : String(error));
    }
  }

  const canLaunch = client.health === "ok" && client.compatibility.can_launch;
  const statusLabel = running ? "正在运行" : canLaunch ? "可启动" : client.health !== "ok" ? "安装不完整" : "不可启动";
  const userMessage = running
    ? `${client.display_name} 正在运行。`
    : canLaunch
      ? `${client.display_name} 已准备就绪，可以启动。`
      : `${client.display_name} 暂不能启动，请处理阻断项。`;

  return {
    client,
    can_launch: canLaunch,
    running,
    status_label: statusLabel,
    user_message: userMessage,
    blocking_reasons: blockingReasons,
    checked_at: checkedAt
  };
}

export function loadAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("load_app_settings");
}

export function saveAppSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("save_app_settings", { settings });
}

export function reportLocalSmokeResult(result: LocalSmokeResultReport): Promise<void> {
  return invoke<void>("report_local_smoke_result", { result });
}

export function listInstallHistory(clientInstallationId: string): Promise<InstallHistoryRecord[]> {
  return invoke<InstallHistoryRecord[]>("list_install_history", { clientInstallationId });
}

export function loadManifest(
  url: string,
  networkRoute?: NetworkRouteConfig
): Promise<UpdateManifest> {
  return invoke<UpdateManifest>("load_manifest", { url, networkRoute });
}

export function checkClientUpdate(request: CheckClientUpdateRequest): Promise<ClientUpdateCheck | null> {
  return invoke<ClientUpdateCheck | null>("check_client_update", { request });
}

export function startUpdateDownload(request: StartUpdateDownloadRequest): Promise<DownloadJob> {
  return invoke<DownloadJob>("start_update_download", { request });
}

export function cancelDownload(jobId: string): Promise<DownloadJob> {
  return invoke<DownloadJob>("cancel_download", { jobId });
}

export function getDownloadJob(jobId: string): Promise<DownloadJob | null> {
  return invoke<DownloadJob | null>("get_download_job", { jobId });
}

export function listDownloadJobRecoveries(
  clientInstallationId?: string
): Promise<DownloadJobRecovery[]> {
  return invoke<DownloadJobRecovery[]>("list_download_job_recoveries", {
    clientInstallationId
  });
}

export function installDownloadedUpdate(jobId: string): Promise<DownloadJob> {
  return invoke<DownloadJob>("install_downloaded_update", { jobId });
}

export function launchClient(path: string): Promise<void> {
  return invoke<void>("launch_client", { path });
}

export function launchDefaultClient(): Promise<void> {
  return invoke<void>("launch_default_client");
}

export function isClientRunning(path: string): Promise<boolean> {
  return invoke<boolean>("is_client_running", { path });
}
