import { describe, expect, it } from "vitest";
import { getErrorMessage, getUpdateErrorMessage } from "./errors";

describe("getErrorMessage", () => {
  it("returns Error.message for Error objects", () => {
    expect(getErrorMessage(new Error("boom"))).toBe("boom");
  });

  it("keeps string errors unchanged", () => {
    expect(getErrorMessage("plain failure")).toBe("plain failure");
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
});
