# DDNet Manager 0.1.0 MVP 发布验收清单

> 依据：`docs/superpowers/specs/2026-06-07-ddnet-manager-next-mvp-spec.md` 的“完成定义”和“默认验收矩阵”。

## 当前 Windows 验收

- [x] `make check-lint` 已通过：PASS 14 / WARN 4 / FAIL 0。
- [x] `make build` 已通过。
- [x] `make tauri-dev-smoke` 已完成 Windows 桌面烟测：Vite ready、Tauri dev build 完成，并输出 `DDNet Manager shell initialized.`。
- [x] 烟测日志已规范写入 `tmp/tauri-dev-smoke.out.log` 和 `tmp/tauri-dev-smoke.err.log`，`tmp/` 已加入 `.gitignore`。
- [x] Rust 测试统一位于 `src-tauri/src/test/`。
- [x] 普通更新流程不要求用户手填 manifest URL；高级 ManifestSource 仅作为调试入口。
- [x] 设置页支持持久化网络路由、扫描排除路径、Everything 开关、GitHub token 和高级 manifest URL。
- [x] 安装事务会持久化完成或失败历史。
- [x] 默认客户端启动前会重新验证健康状态，并写回受控启动探测结果。
- [x] 资源、Binds、Workshop、cfg 写入未作为 0.1.0 可用入口发布。

## macOS 补验边界

当前 Windows 机器不能完成 macOS 实机验收。0.1.0 发布前必须在 macOS 机器补验：

- [ ] 手动添加或扫描 `.app` bundle。
- [ ] 验证 QmClient、TaterClient、Cactus Client、DDNet 原版和 DDNet Steam 的可管理路径。
- [ ] 验证 BestClient 在 macOS 上显示平台不支持或未知，而不是伪装为可自动安装。
- [ ] 验证多个不同路径或不同 bundle 名的 DDNet 兼容 `.app` 可以共存。
- [ ] 验证 Manager-owned `.dmg` 安装闭环：下载、sha256 校验、挂载、复制 `.app` 到 Manager 管理目录、版本记录、备份和失败回滚。
- [ ] 验证不会默认覆盖 `/Applications/DDNet.app`；只有用户显式接管后才允许进入版本控制、备份和回滚链路。

## Linux 补验边界

当前 Windows 机器不能完成 Linux 实机验收。0.1.0 发布前必须在 Linux 机器补验：

- [ ] 手动添加或扫描 `.tar.xz` 解压目录。
- [ ] 验证 QmClient、TaterClient、BestClient、Cactus Client、DDNet 原版和 DDNet Steam 的可管理路径。
- [ ] 验证 Linux 可执行权限缺失时显示风险或不可用原因。
- [ ] 验证 Manager-owned `.tar.xz` 安装闭环：下载、sha256 校验、安全解包、安装到 Manager 管理目录、版本记录、备份和失败回滚。
- [ ] 验证 Steam DDNet 只走发现、注册、启动和打开 Steam 管理入口，不覆盖 Steam 安装目录。

## 发布判断

当前提交具备 Windows 自动化和桌面烟测证据；macOS/Linux 仍必须由对应平台完成上述补验后，才能做跨平台发布签核。
