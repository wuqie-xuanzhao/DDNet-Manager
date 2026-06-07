import { invoke } from "@tauri-apps/api/core";
import type {
  CfgAnalysis,
  CheckClientUpdateRequest,
  ClientInstallation,
  ClientUpdateCheck,
  DownloadJob,
  NetworkRouteConfig,
  ScanClientInstallationsOptions,
  StartUpdateDownloadRequest,
  UpdateManifest,
  UpsertClientInstallationRequest,
  WorkshopBind
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

export function analyzeCfgFile(path: string): Promise<CfgAnalysis> {
  return invoke<CfgAnalysis>("analyze_cfg_file", { path });
}

export function loadWorkshopBinds(url: string): Promise<WorkshopBind[]> {
  return invoke<WorkshopBind[]>("load_workshop_binds", { url });
}
