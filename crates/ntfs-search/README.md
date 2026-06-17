# ntfs-search

跨平台文件搜索 crate。Windows 上读 NTFS $MFT / USN，其他平台走 walkdir。

## 设计目标

- **只读**：所有句柄 `GENERIC_READ`，与 Listary / Everything / chkdsk 完美共存
- **跨平台统一 API**：`find_files` 单一入口，crate 自动按平台/权限选 backend
- **谓词闭包匹配**：调用方决定匹配规则，crate 零依赖匹配库
- **可取消 + 进度回调**：`CancellationToken` + `ProgressSink` trait

## 当前状态

**v0.1 仅骨架**——本 commit 只定义公开类型与 trait，未实现 backend / `find_files`。
后续 commit 逐步补：
- walkdir backend + `find_files` 入口
- USN backend（Windows 普通用户）
- $MFT raw record backend（Windows admin）
- inspect / inspect_many

详见设计稿：`docs/superpowers/plans/2026-06-17-ntfs-search-design.md`

## 磁盘安全

参见设计稿 §10。三条铁律：

1. 所有句柄 `GENERIC_READ`，绝不请求 `WRITE`
2. 只调用只读 NTFS IOCTL（5 个白名单）
3. `FILE_SHARE_READ | FILE_SHARE_WRITE`，允许多进程并发访问

## 协议

MIT OR Apache-2.0。

## 设计稿引用

如需了解 API 设计、错误处理、测试策略等完整内容，请阅读项目根的
`docs/superpowers/plans/2026-06-17-ntfs-search-design.md`。
