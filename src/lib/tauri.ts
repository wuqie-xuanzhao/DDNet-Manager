import { invoke } from "@tauri-apps/api/core";
import type {
  CfgAnalysis,
  CheckClientUpdateRequest,
  AppSettings,
  ClientInstallation,
  ClientUpdateCheck,
  DownloadJob,
  InstallHistoryRecord,
  NetworkRouteConfig,
  ScanClientInstallationsOptions,
  StartUpdateDownloadRequest,
  UpdateManifest,
  UpsertClientInstallationRequest
} from "../types";

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

export function loadAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("load_app_settings");
}

export function saveAppSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("save_app_settings", { settings });
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
