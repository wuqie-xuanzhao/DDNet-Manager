# 实现计划：下载加速收尾 —— race 测试修复与门禁验收

基于 `.trae/documents/plan-下载加速收尾-前端同步与测试修复.md` 的延续。前序会话已完成 A1（network_route env 测试串行化）、B-E（前端类型/UI/测试同步），仅剩 A2 的 race 测试修复和 F1 门禁验收未完成。

## 现状分析

### 已完成（无需再动）

经核对，以下均已落地并通过：
- 后端实现：[mirror.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/mirror.rs)、[download/race.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/race.rs)（含已提取的 `content_length_matches` 纯函数）、[download/net.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/net.rs)、[models.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/models.rs)、[network_route.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/network_route.rs)、[commands/download.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/commands/download.rs)
- 后端测试：[test/mirror.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/mirror.rs)（11 测试）、[test/network_route.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/network_route.rs)（ENV_MUTEX 串行化已加）、[test/download/net.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/download/net.rs)（extra_hosts/candidate_urls 已同步）
- 前端：[types.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/types.ts)、[settings.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/settings.ts)、[updateLogic.ts](file:///e:/Coding/DDNet/DDNet-Manager/src/lib/updateLogic.ts)、[UpdatePanel.tsx](file:///e:/Coding/DDNet/DDNet-Manager/src/components/update/UpdatePanel.tsx)
- 前端测试：UpdatePanel.test.tsx、useClientInstaller.test.tsx、useAppUpdater.test.tsx、settings.test.ts、updateLogic.test.ts

### 唯一剩余的测试失败

[test/download/race.rs:71-87](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/download/race.rs#L71-L87) 的 `select_best_source_rejects_candidate_with_wrong_content_length` 失败。

**根因**：mockito 的 HEAD 响应强制发送 `content-length: 0`（忽略 `with_header("content-length", "999")`）。前序会话为修此问题已：
1. 在 [race.rs:145-150](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/race.rs#L145-L150) 提取纯函数 `content_length_matches(actual: Option<u64>, expected_size: u64) -> bool`，逻辑为 `Some(length) if length > 0 => length == expected_size, _ => true`（0 视为无法判定，放行）。
2. 在 [race.rs:132](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/race.rs#L132) 的 `head_probe_once` 中改为调用该纯函数。

但测试文件未同步更新——失败的集成测试仍在用 mockito HEAD 验证 size 拒绝，而 mockito 无法发送非零 content-length，故测试断言 `"unreachable"` 不再成立。

## 改动清单

### A. 替换 race 测试中的 wrong-content-length 集成测试为纯函数单元测试

#### A1. [test/download/race.rs](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/test/download/race.rs)

**改动 1**：顶部 import 增加 `content_length_matches`。

当前 L1：
```rust
use crate::download::race::select_best_source;
```

改为：
```rust
use crate::download::race::select_best_source;
use super::content_length_matches;
```

**为什么用 `super::` 而非 `crate::download::race::`**：`content_length_matches` 是 `race` 模块的私有 `fn`（非 `pub`），子模块（`mod tests`）可通过 `super::` 访问父模块私有项；`crate::download::race::` 路径只能访问 `pub` 项。测试模块由 [race.rs:228-230](file:///e:/Coding/DDNet/DDNet-Manager/src-tauri/src/download/race.rs#L228-L230) 的 `#[cfg(test)] #[path = "../test/download/race.rs"] mod tests;` 声明为 `race` 的子模块，故 `super::` 指向 `race`。

**改动 2**：删除 L71-87 的失败集成测试 `select_best_source_rejects_candidate_with_wrong_content_length`，替换为 `content_length_matches` 的 4 个直接单元测试：

```rust
#[test]
fn content_length_matches_rejects_mismatched_size() {
    // size 不符应淘汰（这是原集成测试想验证的逻辑）
    assert!(!content_length_matches(Some(999), 100));
    assert!(!content_length_matches(Some(1025), 1024));
}

#[test]
fn content_length_matches_accepts_zero_as_indeterminate() {
    // mockito HEAD 强制 content-length=0，视为无法判定，放行不淘汰
    assert!(content_length_matches(Some(0), 1024));
    assert!(content_length_matches(Some(0), 0));
}

#[test]
fn content_length_matches_accepts_none_as_indeterminate() {
    // 反代可能不返回 Content-Length 头
    assert!(content_length_matches(None, 1024));
    assert!(content_length_matches(None, 0));
}

#[test]
fn content_length_matches_accepts_exact_size() {
    assert!(content_length_matches(Some(1024), 1024));
    assert!(content_length_matches(Some(100), 100));
}
```

**为什么替换而非修复集成测试**：mockito 的 HEAD 响应在底层 hyper 强制 `content-length: 0`（HEAD 请求无 body 的 HTTP 约定），无法通过 `with_header` 覆盖。集成测试无法模拟"HEAD 返回非零 content-length 且与 expected_size 不符"的场景，故 size 拒绝逻辑只能通过纯函数单元测试覆盖。这是测试粒度的合理下沉——纯函数已抽离正是为此。

### B. 门禁验收

#### B1. 运行 `cargo test` 确认全绿

```bash
cargo test --manifest-path src-tauri/Cargo.toml 2>&1 | Select-Object -Last 40
```

预期：所有测试通过（前序会话最近一次结果为 226 passed; 1 failed，A1 修后应 227 passed; 0 failed）。

**若仍有失败**：按报错就地修复。可能场景：
- race 测试并发起 mockito server 端口冲突 → 给 race 测试模块加 `static RACE_MUTEX: Mutex<()> = Mutex::new(())` 串行化（兜底，仅确认并发冲突时启用）。
- 其他意外失败 → 就地修复并记录。

#### B2. 运行 `make check-lint` 完整门禁

```bash
make check-lint
```

预期全绿。重点关注：
- `cargo fmt --check`：A1 新增代码已 fmt（4 个测试函数 + import）。
- `cargo clippy -- -D warnings`：无告警。
- `cargo test`：mirror / race / network_route / net / registry 全过。
- `bun install --frozen-lockfile` + `bun run check`：TS 编译通过（前序会话已同步前端类型，应无问题）。
- 结构扫描：单文件/函数规模、`unwrap` 扫描（测试代码允许）、`mod.rs` 禁用、`super::super::` 告警、公共 API 文档注释、`unsafe` SAFETY、TODO/FIXME 扫描。

**若 `make check-lint` 因非本次改动原因失败**：明确汇报失败项与是否为存量问题，不擅自修无关代码。

## 假设与决策

### 决策

1. **替换集成测试为纯函数单元测试**而非"修复 mockito 行为"：mockito HEAD 的 content-length=0 是底层 HTTP 约定，不可绕过。纯函数已抽离，测试粒度下沉到纯函数是正确做法，覆盖 wrong-size / zero / none / correct 四种场景比原集成测试更全面。
2. **不删除 `head_probe_once` 中的 size 检查**：纯函数单测覆盖逻辑正确性，集成测试覆盖"HEAD 流程能跑通"，两者互补。原集成测试的错误在于用 mockito 验证 mockito 无法模拟的场景，而非 size 检查本身有问题。
3. **不补 `serial_test` crate**：race 测试当前 9 个，前序会话已修 8 个通过，仅 1 个因 mockito 限制失败，与并发无关。若 B1 发现并发端口冲突再加串行化（兜底方案）。

### 假设

- `content_length_matches` 的 `fn`（非 `pub fn`）可见性足够被子模块 `mod tests` 通过 `super::` 访问——这是 Rust 标准可见性规则。
- 前序会话的 226 passed 在 A1 修后变为 227 passed（+4 新测试 -1 删除测试 = +3，但原 1 failed 转 passed = +1，共 +4？实际：原 226 passed 含 1 failed 即 225+1，修后 225+4=229 passed；具体数以 B1 实跑为准）。
- 前端 `bun run check` 已在前序会话同步类型后通过，本次不改前端，应仍通过。

## 验证步骤

1. `cargo test --manifest-path src-tauri/Cargo.toml` —— 确认 race 测试全过（含 4 个新 `content_length_matches` 单测），无 failed。
2. `make check-lint` —— 全绿（fmt + clippy + cargo test + bun install + bun run check + 结构扫描）。
3. （可选，手动）`make tauri-dev` —— 设置页确认网络路由三选一按钮（直连 / 自动检测 / 手动填写）正常，自动检测时地址输入框隐藏。

## 不在本次范围

- 任何前端改动（前序会话已完成）。
- 任何后端实现改动（前序会话已完成；本次只动测试文件）。
- P1/P2 项（反代列表外部化热更新、测速缓存、前端竞速状态展示、`.partial` 完整断点续传）。
- 技术债（`Result<_, String>` → `thiserror`）。
