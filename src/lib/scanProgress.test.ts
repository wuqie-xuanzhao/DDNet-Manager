import { describe, expect, it } from "vitest";
import {
  describeBackend,
  describeScanEvent,
  describeScanPhase,
  type ScanProgressEvent
} from "./scanProgress";

describe("describeBackend", () => {
  it("MFT 标签含管理员标识", () => {
    expect(describeBackend("mft")).toBe("MFT（管理员）");
  });

  it("USN 标签含普通用户标识", () => {
    expect(describeBackend("usn")).toBe("USN（普通用户）");
  });

  it("Walkdir 标签标注 fallback", () => {
    expect(describeBackend("walkdir")).toBe("Walkdir（fallback）");
  });
});

describe("describeScanEvent", () => {
  it("drive_started 含 root 与 backend 中文标签", () => {
    const event: ScanProgressEvent = {
      kind: "drive_started",
      root: "C:\\",
      backend: "mft"
    };
    expect(describeScanEvent(event)).toBe("扫描 C:\\（MFT（管理员））");
  });

  it("entries_found 显示候选数", () => {
    const event: ScanProgressEvent = { kind: "entries_found", found: 3 };
    expect(describeScanEvent(event)).toBe("已找到 3 个候选");
  });

  it("drive_completed 含 scanned + found", () => {
    const event: ScanProgressEvent = {
      kind: "drive_completed",
      root: "C:\\",
      scanned: 1000,
      found: 2
    };
    expect(describeScanEvent(event)).toContain("扫描 1000 条记录");
    expect(describeScanEvent(event)).toContain("找到 2 个");
  });

  it("phase_started started 委托给 describeScanPhase", () => {
    const event: ScanProgressEvent = { kind: "phase_started", phase: "started" };
    expect(describeScanEvent(event)).toBe(describeScanPhase("started"));
  });

  it("phase_started priority 含 Steam 等位置提示", () => {
    const event: ScanProgressEvent = { kind: "phase_started", phase: "priority" };
    expect(describeScanEvent(event)).toContain("常见安装位置");
  });

  it("phase_started fallback 含全盘扫描提示", () => {
    const event: ScanProgressEvent = { kind: "phase_started", phase: "fallback" };
    expect(describeScanEvent(event)).toContain("全盘扫描");
  });
});

describe("describeScanPhase", () => {
  it("started 显示扫描已启动提示", () => {
    expect(describeScanPhase("started")).toContain("扫描已启动");
  });

  it("priority 含常见位置关键字", () => {
    expect(describeScanPhase("priority")).toContain("Steam");
    expect(describeScanPhase("priority")).toContain("Program Files");
  });

  it("fallback 含扩展到全盘关键字", () => {
    expect(describeScanPhase("fallback")).toContain("全盘扫描");
  });
});
