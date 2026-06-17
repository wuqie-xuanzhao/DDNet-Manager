import { describe, expect, it } from "vitest";
import type { IpcError } from "../types";
import { getErrorMessage, getUpdateErrorMessage } from "./errors";

describe("getErrorMessage", () => {
  it("returns Error.message for Error objects", () => {
    expect(getErrorMessage(new Error("boom"))).toBe("boom");
  });

  it("keeps string errors unchanged", () => {
    expect(getErrorMessage("plain failure")).toBe("plain failure");
  });

  it("returns IpcError.message so callers can show backend diagnostics", () => {
    const error: IpcError = {
      code: "unknown",
      message: "client executable is not a file: D:/Games/QmClient/DDNet.exe",
    };
    expect(getErrorMessage(error)).toBe(
      "client executable is not a file: D:/Games/QmClient/DDNet.exe"
    );
  });

  it("returns IpcError.message regardless of code, even for known codes", () => {
    const error: IpcError = {
      code: "not_found",
      message: "client installation not found: qmclient-main",
    };
    expect(getErrorMessage(error)).toBe("client installation not found: qmclient-main");
  });
});

describe("getUpdateErrorMessage", () => {
  it("maps checksum errors to a product message", () => {
    expect(getUpdateErrorMessage("download sha256 mismatch")).toBe("下载文件校验失败，请重新下载。");
  });

  it("falls back for unknown errors", () => {
    expect(getUpdateErrorMessage("failed to create rollback point: access denied")).toBe(
      "操作失败，请稍后重试。failed to create rollback point: access denied"
    );
  });

  it("maps structured IpcError by stable code", () => {
    const error: IpcError = { code: "checksum_mismatch", message: "sha256 mismatch detail" };
    expect(getUpdateErrorMessage(error)).toBe("下载文件校验失败，请重新下载。");
  });

  it("maps sha256_missing IpcError to manual-download hint", () => {
    const error: IpcError = { code: "sha256_missing", message: "更新资产缺少 sha256" };
    expect(getUpdateErrorMessage(error)).toContain("手动下载");
  });

  it("falls back to message for unknown IpcError code", () => {
    const error: IpcError = { code: "something_new", message: "未知错误细节" };
    expect(getUpdateErrorMessage(error)).toBe("操作失败，请稍后重试。未知错误细节");
  });
});
