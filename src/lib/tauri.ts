import { invoke } from "@tauri-apps/api/core";
import type { CfgAnalysis, ClientInstallation, UpdateManifest, WorkshopBind } from "../types";

export function validateClientDir(path: string): Promise<ClientInstallation> {
  return invoke<ClientInstallation>("validate_client_dir", { path });
}

export function loadManifest(url: string, proxyBaseUrl?: string): Promise<UpdateManifest> {
  return invoke<UpdateManifest>("load_manifest", { url, proxyBaseUrl });
}

export function launchClient(path: string): Promise<void> {
  return invoke<void>("launch_client", { path });
}

export function analyzeCfgFile(path: string): Promise<CfgAnalysis> {
  return invoke<CfgAnalysis>("analyze_cfg_file", { path });
}

export function loadWorkshopBinds(url: string): Promise<WorkshopBind[]> {
  return invoke<WorkshopBind[]>("load_workshop_binds", { url });
}
