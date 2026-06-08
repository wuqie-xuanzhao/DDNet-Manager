const fallbackErrorMessage = "操作失败，请稍后重试。";
const MAX_RAW_ERROR_SUMMARY_LENGTH = 180;

export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return String(error);
}

export function getUpdateErrorMessage(error: unknown): string {
  const raw = getErrorMessage(error);
  if (raw.includes("host is not trusted") || raw.includes("host is not enabled")) {
    return "当前下载地址未被允许，请检查更新源或网络设置。";
  }
  if (raw.includes("must use https")) {
    return "请使用 HTTPS 地址。";
  }
  if (raw.includes("checksum") || raw.includes("sha256")) {
    return "下载文件校验失败，请重新下载。";
  }
  if (raw.includes("not found")) {
    return "没有找到对应的客户端或更新任务。";
  }
  if (raw.includes("running")) {
    return "请先关闭正在运行的客户端，再安装更新。";
  }
  if (raw.includes("manifest")) {
    return "更新源读取失败，请检查地址后重试。";
  }

  if ((error instanceof Error || typeof error === "string") && raw.trim()) {
    return `${fallbackErrorMessage}${raw.trim().slice(0, MAX_RAW_ERROR_SUMMARY_LENGTH)}`;
  }

  return fallbackErrorMessage;
}
