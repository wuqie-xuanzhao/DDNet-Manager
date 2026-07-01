import type { IpcError, UpdateCheckReason } from "../types";

const fallbackErrorMessage = "操作失败，请稍后重试。";
const MAX_RAW_ERROR_SUMMARY_LENGTH = 180;

/** 按 IPC 错误码映射的用户文案，与后端 `models.rs` 的 `IPC_ERROR_*` 常量对齐。 */
const IPC_ERROR_MESSAGES: Readonly<Record<string, string>> = {
  network_host_not_trusted: "当前下载地址未被允许，请检查更新源或网络设置。",
  network_https_required: "请使用 HTTPS 地址。",
  checksum_mismatch: "下载文件校验失败，请重新下载。",
  not_found: "没有找到对应的客户端或更新任务。",
  client_running: "请先关闭正在运行的客户端，再安装更新。",
  manifest_unreachable: "更新源读取失败，请检查地址后重试。",
  sha256_missing: "更新资产缺少校验信息，已禁用自动安装，请打开 Release 页面手动下载。",
  unknown: fallbackErrorMessage
};

/** 判断捕获值是否为后端结构化 IpcError。 */
function isIpcError(value: unknown): value is IpcError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { code?: unknown }).code === "string" &&
    typeof (value as { message?: unknown }).message === "string"
  );
}

export function getErrorMessage(error: unknown): string {
  // IpcError 是后端 Tauri command 的标准错误返回形态，优先取 message 让用户看到
  // 原始诊断信息（code 走 getUpdateErrorMessage 映射文案）。
  if (isIpcError(error)) {
    return error.message;
  }

  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return String(error);
}

export function getUpdateErrorMessage(error: unknown): string {
  // 优先按后端结构化 IpcError 的稳定 code 映射文案
  if (isIpcError(error)) {
    const mapped = IPC_ERROR_MESSAGES[error.code];
    if (mapped) {
      return mapped;
    }
    const detail = error.message.trim();
    return detail
      ? `${fallbackErrorMessage}${detail.slice(0, MAX_RAW_ERROR_SUMMARY_LENGTH)}`
      : fallbackErrorMessage;
  }

  // 兼容仍返回 String 的 command（客户端管理、启动、设置等），文案引用同一张表
  const raw = getErrorMessage(error);
  if (raw.includes("host is not trusted") || raw.includes("host is not enabled")) {
    return IPC_ERROR_MESSAGES.network_host_not_trusted;
  }
  if (raw.includes("must use https")) {
    return IPC_ERROR_MESSAGES.network_https_required;
  }
  if (raw.includes("checksum") || raw.includes("sha256")) {
    return IPC_ERROR_MESSAGES.checksum_mismatch;
  }
  if (raw.includes("not found")) {
    return IPC_ERROR_MESSAGES.not_found;
  }
  if (raw.includes("running")) {
    return IPC_ERROR_MESSAGES.client_running;
  }
  if (raw.includes("manifest")) {
    return IPC_ERROR_MESSAGES.manifest_unreachable;
  }

  if ((error instanceof Error || typeof error === "string") && raw.trim()) {
    return `${fallbackErrorMessage}${raw.trim().slice(0, MAX_RAW_ERROR_SUMMARY_LENGTH)}`;
  }

  return fallbackErrorMessage;
}

/** 把更新检查不可用原因映射为小白友好的中文文案。
 * 后端可能已附带详细 message，优先使用；否则按 reason 类型给出默认白话说明。 */
export function getUpdateCheckReasonMessage(reason: UpdateCheckReason, backendMessage?: string | null): string {
  if (backendMessage && backendMessage.trim()) {
    return backendMessage.trim();
  }

  switch (reason) {
    case "client_not_in_catalog":
      return "该客户端暂不支持自动更新。";
    case "no_release_for_channel":
      return "当前渠道暂无可用版本。";
    case "no_asset_for_platform":
      return "当前平台暂无可用下载。";
    case "auto_update_disabled":
      return "该客户端已禁用自动更新。";
    case "manifest_entry_missing":
      return "更新清单缺少该客户端信息。";
    case "none":
      return "已是最新版本。";
    default:
      return "无可用更新。";
  }
}
