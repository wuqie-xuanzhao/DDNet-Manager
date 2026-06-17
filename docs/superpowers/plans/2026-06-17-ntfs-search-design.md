# ntfs-search crate 设计稿

- **日期**：2026-06-17
- **状态**：已通过 brainstorming，待启动实施
- **作者**：鸢天 + Proma Agent
- **影响范围**：新增独立 crate `crates/ntfs-search/`；改造 `src-tauri/src/client_scan.rs`；前端新增扫描进度展示
- **预估代码量**：~3450 行（含测试）

---

## 1. 背景与动机

### 1.1 现状

`src-tauri/src/client_scan.rs`（658 行）目前用碎片化混合策略扫客户端：

| 子策略 | 实现 | 局限 |
| --- | --- | --- |
| `find_candidate_dirs` | `VecDeque` BFS 递归，max_depth 3-5 | 速度慢；深层客户端扫不到 |
| `default_scan_roots` | 硬编码"C:/Games、D:/Games、Steam 默认路径"等 | 用户自定义路径完全覆盖不到 |
| `find_steam_library_ddnet_roots` | 解析 Steam `libraryfolders.vdf` | 只覆盖 Steam，不覆盖 QmClient/手动解压 |
| `find_everything_candidate_dirs` | 外部调 `es.exe` | 强外部依赖；非 Windows 不可用 |

### 1.2 目标

把扫描能力演进成像 **Everything / Listary** 那样"直接读 NTFS $MFT"的实现，**完全取代** BFS：

- 全盘扫描从十几秒→几秒（admin）/ 几十秒（普通用户）
- 覆盖任意自定义路径（不再依赖硬编码根）
- 移除外部 Everything 依赖
- 把扫描能力沉淀为独立 crate `ntfs-search`，与 DDNet 业务解耦

### 1.3 非目标

- **不**做 Listary 风格的智能排序、上下文感知、拼音模糊匹配（业务层职责，不进 crate）
- **不**做"启动时全量 + 后台 USN 增量缓存"（v0.1 不做；v0.2 视需求再考虑）
- **不**做 GUI（crate 是纯库）
- **不**做服务化（不装为 Windows Service）
- **不**做 Lucene 风格的全文索引

---

## 2. 核心决策摘要

brainstorming 阶段确认的 7 条关键决策：

| # | 决策点 | 选择 | 理由 |
| --- | --- | --- | --- |
| 1 | crate 边界与定位 | **独立 crate**（`crates/ntfs-search/`） | 职责干净；可测试；未来可发布到 crates.io |
| 2 | 跨平台策略 | **跨平台 + 自动 fallback** | Windows MFT + 其他平台 walkdir，统一 API |
| 3 | Windows 实现路径 | **混合**：admin 走 $MFT raw record / 普通 USN 枚举 | 零摩擦体验；自动选最快可用路径 |
| 4 | 智能排序 | **crate 不做**，调用方自行实现 | YAGNI；业务语义不同强行抽象反扭曲 |
| 5 | API 形态 | **async + ProgressSink trait + CancellationToken** | 与 Tauri 现有 download/install 模式对称 |
| 6 | 文件名匹配 | **谓词闭包** `Box<dyn Fn(&str) -> bool>` | 零依赖耦合；零样板 |
| 7 | 底层依赖 | **windows-rs + walkdir + futures + bitflags + tracing + thiserror** | 官方、成熟、与 Tauri 一致 |

---

## 3. 架构总览

### 3.1 工作区结构

DDNet-Manager 升级为 Cargo workspace：

```text
DDNet-Manager/
├── Cargo.toml                       # 新增 workspace 根 manifest
├── crates/
│   └── ntfs-search/                 # 新 crate
│       ├── Cargo.toml
│       ├── README.md
│       ├── LICENSE-MIT              # 双协议预留 crates.io
│       ├── LICENSE-APACHE
│       ├── examples/
│       │   └── dump_fixture.rs      # 本地用：dump MFT record fixture
│       ├── tests/
│       │   ├── fixtures/
│       │   │   ├── mft_records/
│       │   │   ├── usn_records/
│       │   │   └── trees/
│       │   ├── parser_mft.rs
│       │   ├── parser_usn.rs
│       │   ├── rebuild_paths.rs
│       │   └── walkdir_backend.rs
│       └── src/
│           ├── lib.rs               # 公开 re-export
│           ├── error.rs             # ScanError
│           ├── options.rs           # NtfsScanOptions / FileEntry / InspectFields / ProgressEvent
│           ├── matcher.rs           # Matcher 包装
│           ├── find.rs              # find_files 入口 + 多盘并行 + cancel/timeout
│           ├── inspect.rs           # inspect / inspect_many
│           ├── rebuild_paths.rs     # 路径重建 util（MFT + USN 共用）
│           ├── backend.rs           # Backend trait
│           ├── backend/
│           │   ├── windows.rs       # 入口文件
│           │   ├── windows/
│           │   │   ├── volume.rs    # CreateFileW + IOCTL 封装
│           │   │   ├── mft.rs       # $MFT raw record 解析（admin）
│           │   │   ├── usn.rs       # FSCTL_ENUM_USN_DATA（普通用户）
│           │   │   ├── probe.rs     # 权限/能力探测
│           │   │   └── winapi.rs    # windows-rs 类型 alias
│           │   └── walkdir.rs       # 跨平台 fallback
│           └── pe.rs                # VS_VERSION_INFO 解析（inspect 用）
└── src-tauri/                       # 主 crate
    ├── Cargo.toml                   # 加 path 依赖
    └── src/
        ├── client_scan.rs           # 改造为 ntfs-search 调用方
        └── ...
```

> 按 CLAUDE.md 禁用 `mod.rs`，子目录的入口文件用同名 `.rs`（如 `backend.rs` + `backend/`、`windows.rs` + `windows/`）。

### 3.2 Workspace 根 Cargo.toml

```toml
[workspace]
members = ["crates/ntfs-search", "src-tauri"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
rust-version = "1.75"

[workspace.dependencies]
# 共享依赖版本对齐
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
thiserror = "1"
```

### 3.3 ntfs-search Cargo.toml

```toml
[package]
name = "ntfs-search"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Cross-platform file search via NTFS MFT raw reads (Windows) and walkdir fallback (Unix)"
repository = "https://github.com/.../ddnet-manager"

[dependencies]
windows = { version = "0.x", features = [
    "Win32_Foundation",
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
    "Win32_System_Ioctl",
    "Win32_Security",         # SID 解析
] }
walkdir = "2"
futures = "0.3"
tokio = { workspace = true }
tokio-util = "0.7"             # CancellationToken
bitflags = "2"
tracing = { workspace = true }
thiserror = { workspace = true }
async-trait = "0.1"
pelite = "0.10"                # PE 资源解析（VS_VERSION_INFO），避免手写 ~300 行

[dev-dependencies]
tempfile = "3"
proptest = "1"
```

> v0.1 不含 `sha2` / `hex` —— `FileHash` 字段延后到 v0.2（DDNet 业务用不到 SHA-256，YAGNI）。

---

## 4. 公开 API 设计

### 4.1 公开类型清单

```rust
// crates/ntfs-search/src/lib.rs
mod backend;
mod error;
mod find;
mod inspect;
mod matcher;
mod options;
mod rebuild_paths;

pub use crate::error::ScanError;
pub use crate::find::find_files;
pub use crate::inspect::{inspect, inspect_many, InspectOutcome};
pub use crate::options::{
    BackendKind, FileAttributes, FileEntry, InspectFields, InspectedEntry,
    NtfsScanOptions, ProgressEvent, ScanLimitKind, VersionInfo,
};

pub trait ProgressSink: Send + Sync {
    fn emit(&self, event: ProgressEvent);
}

/// 默认空实现，调用方不需要进度时用
pub struct NoopSink;
impl ProgressSink for NoopSink {
    fn emit(&self, _: ProgressEvent) {}
}
```

> **命名约定**：crate 内部统一用 `NtfsScanOptions` 前缀（不复用裸 `ScanOptions`），避免与 DDNet-Manager 业务层的 `ScanOptions`（含 `excluded_paths`、`include_saved_paths` 等业务字段）混淆。

### 4.2 FileEntry —— 基础层（MFT 原生字段）

```rust
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub accessed: SystemTime,
    pub attributes: FileAttributes,
    pub is_directory: bool,
    pub backend: BackendKind,    // 这条记录来自 Mft/Usn/Walkdir
    pub file_reference: Option<u64>,   // NTFS file ref（Mft/Usn 有，Walkdir 无）
}
```

**故意不放的字段**（按需 inspect）：
- PE 版本资源（CompanyName/ProductName 等）
- NTFS 所有者 SID
- 备用数据流（ADS）

**USN 路径的 `created` / `accessed`**：USN_RECORD V2 只存 modified，`created` / `accessed` 设为 `SystemTime::UNIX_EPOCH` 作为 "unknown" 信号，前端需特判显示"未知"。

**`file_reference` 字段**：
- Mft / Usn backend 填充（NTFS FileReferenceNumber，低 48 bit 为 record 序号、高 16 bit 为 sequence number）
- Walkdir backend 填 `None`
- 调用方按需用（比如二次 `FSCTL_GET_NTFS_FILE_RECORD` 单条查询某 entry 的扩展属性）

### 4.3 FileAttributes —— 完整 NTFS 属性集

```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FileAttributes: u32 {
        const READONLY            = 0x0001;
        const HIDDEN              = 0x0002;
        const SYSTEM              = 0x0004;
        const DIRECTORY           = 0x0010;
        const ARCHIVE             = 0x0020;
        const DEVICE              = 0x0040;
        const NORMAL              = 0x0080;
        const TEMPORARY           = 0x0100;
        const SPARSE_FILE         = 0x0200;
        const REPARSE_POINT       = 0x0400;
        const COMPRESSED          = 0x0800;
        const OFFLINE             = 0x1000;
        const NOT_CONTENT_INDEXED = 0x2000;
        const ENCRYPTED           = 0x4000;
        const INTEGRITY_STREAM    = 0x8000;
    }
}
```

### 4.4 NtfsScanOptions

```rust
#[derive(Clone)]
pub struct NtfsScanOptions {
    pub roots: Vec<PathBuf>,
    pub matcher: Box<dyn Fn(&str) -> bool + Send + Sync + 'static>,
    pub max_results: Option<usize>,
    pub max_records_scanned: Option<usize>,
    pub timeout: Duration,
    pub inspect_concurrency: usize,
}

impl NtfsScanOptions {
    pub fn new(matcher: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        Self {
            roots: Vec::new(),
            matcher: Box::new(matcher),
            max_results: None,
            max_records_scanned: Some(2_000_000),  // 2M 上限，控制 HashMap 内存峰值 < 400MB
            timeout: Duration::from_secs(60),
            inspect_concurrency: 16,
        }
    }

    pub fn with_root(mut self, root: PathBuf) -> Self { self.roots.push(root); self }
    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self { self.roots.extend(roots); self }
    pub fn with_max_results(mut self, n: usize) -> Self { self.max_results = Some(n); self }
    pub fn with_max_records_scanned(mut self, n: usize) -> Self { self.max_records_scanned = Some(n); self }
    pub fn with_timeout(mut self, d: Duration) -> Self { self.timeout = d; self }
    pub fn with_inspect_concurrency(mut self, n: usize) -> Self { self.inspect_concurrency = n; self }
}
```

**关键约束**：
- `matcher` 必须加 `'static`，否则 `NtfsScanOptions` 无法跨 `tokio::spawn` 边界（编译报错）
- `max_records_scanned` 默认 **2,000,000**（原稿 5M）—— NTFS record 平均占 ~200B（短/长文件名双 `$FILE_NAME`），2M ≈ 400MB 内存上限；百万级文件的 C 盘也只触达一半额度
- 调用方可显式 `.with_max_records_scanned(usize::MAX)` 关闭保护（仅在受控环境使用）

### 4.5 ProgressEvent —— 一个枚举聚合所有回调

```rust
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    DriveStarted {
        root: PathBuf,
        backend: BackendKind,
    },
    EntriesFound {
        found: usize,           // 截至当前累计匹配数
    },
    DriveCompleted {
        root: PathBuf,
        scanned: usize,         // 该盘解析过的 record 总数（≠ found）
        found: usize,           // 该盘匹配的 entry 数
    },
    BackendDowngraded {
        root: PathBuf,
        from: BackendKind,
        to: BackendKind,
        reason: String,
    },
    ScanLimitHit {
        kind: ScanLimitKind,
        limit: usize,
    },
    DriveSkipped {
        root: PathBuf,
        reasons: Vec<String>,
    },
    EntryError {
        path: Option<PathBuf>,  // None 表示无法定位路径（如 stale parent ref）
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanLimitKind {
    Results,
    RecordsScanned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Mft,
    Usn,
    Walkdir,
}
```

**关键澄清**：
- `EntriesFound.found` = 当前累计的匹配数（"已找到 N 个候选"）
- `DriveCompleted.scanned` = 解析过的 record 总数（"扫描了 N 万条记录"）
- `DriveCompleted.found` = 匹配的 entry 数
- 前端展示两个不同语义字段，不能混用
- `EntryError.path = None` 用于 NTFS 路径重建时遇到 stale parent ref（杀软实时改 MFT 导致快照不一致），entry 的真实路径无法定位

### 4.6 find_files 入口

```rust
pub async fn find_files(
    opts: NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
) -> Result<Vec<FileEntry>, ScanError>;
```

**设计要点**：
- `progress: Arc<dyn ProgressSink>` —— 内部多盘 backend 共享
- `cancel: CancellationToken` —— 与现有 download/install 取消模式对称；用户取消时返回 `Err(ScanError::Cancelled)`，**调用方**决定是转为 `Ok(vec![])` 还是 propagate
- `max_results` 触发即返回，**软限制**不报错
- `timeout` 软超时，返回已找到的部分结果（不报错）

---

## 5. 扩展层 inspect / inspect_many

### 5.1 InspectFields —— 调用方按需声明要 fetch 哪些字段

```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InspectFields: u8 {
        const VERSION_INFO = 0b0000_0001;   // PE VS_VERSION_INFO
        const OWNER_SID    = 0b0000_0010;   // NTFS 所有者
        const ADS          = 0b0000_0100;   // 备用数据流列表
        // FILE_HASH 延后到 v0.2（DDNet 业务用不到 SHA-256，避免拖 sha2/hex 依赖）
    }
}
```

### 5.2 InspectedEntry —— 扩展层结构

```rust
#[derive(Debug, Clone)]
pub struct InspectedEntry {
    pub version_info: Option<VersionInfo>,
    pub owner_sid: Option<String>,
    pub alt_data_streams: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub company_name: Option<String>,
    pub product_name: Option<String>,
    pub file_description: Option<String>,
    pub file_version: Option<String>,
    pub product_version: Option<String>,
    pub original_filename: Option<String>,
}
```

> **FileHash 移到 v0.2**：原稿包含 `FileHash { algorithm, digest }` 字段 + `HashAlgorithm` 枚举。审查认为 DDNet 业务用不到 SHA-256，且会拖 `sha2` 依赖、拖 inspect 并发池、拖测试。v0.1 砍掉，v0.2 视需求补回。

### 5.3 inspect / inspect_many API

```rust
pub async fn inspect(
    entry: &FileEntry,
    fields: InspectFields,
) -> Result<InspectedEntry, ScanError>;

pub async fn inspect_many(
    entries: &[FileEntry],
    fields: InspectFields,
    progress: Arc<dyn ProgressSink>,
    concurrency: usize,    // 来自 NtfsScanOptions.inspect_concurrency
) -> Result<Vec<InspectOutcome>, ScanError>;

#[derive(Debug, Clone)]
pub enum InspectOutcome {
    Success(InspectedEntry),
    Failed { path: PathBuf, error: ScanError },
}
```

**设计要点**：
- 内置 `tokio::sync::Semaphore` 控制并发，默认 16 路
- 单条 inspect 失败不阻断 `inspect_many`，按 `InspectOutcome` 分流
- 调用方传入 `InspectFields::VERSION_INFO | InspectFields::ADS` 即可一次取多字段
- 单条 `inspect` 不接受 `progress` / `concurrency`（保持 API 简洁，单次 fetch 不需要并发）

### 5.4 调用方使用示例

```rust
use ntfs_search::*;

// 步骤 1：全盘扫 DDNet.exe（几秒）
let opts = NtfsScanOptions::new(|n| {
    ["DDNet.exe", "ddnet.exe"].iter().any(|e| n.eq_ignore_ascii_case(e))
})
.with_roots([PathBuf::from("C:\\"), PathBuf::from("D:\\")])
.with_max_results(50);

let entries = find_files(opts, Arc::new(NoopSink), CancellationToken::new()).await?;

// 步骤 2：批量 inspect（只取 VERSION_INFO，跳过 owner/ADS）
let inspected = inspect_many(
    &entries,
    InspectFields::VERSION_INFO,
    Arc::new(NoopSink),
    16,
).await?;

// 步骤 3：调用方按 PE 元数据判定身份
for (entry, outcome) in entries.iter().zip(inspected.iter()) {
    if let InspectOutcome::Success(info) = outcome {
        if let Some(vi) = &info.version_info {
            if vi.company_name.as_deref() == Some("DDNet Team") {
                // 标记为官方 DDNet
            }
        }
    }
}
```

---

## 6. Windows 实现分层

### 6.1 Backend 抽象

```rust
// crates/ntfs-search/src/backend.rs
use async_trait::async_trait;

#[async_trait]
pub(super) trait Backend: Send + Sync {
    fn kind(&self) -> BackendKind;

    async fn scan_root(
        &self,
        root: &Path,
        opts: &NtfsScanOptions,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<Vec<FileEntry>, ScanError>;
}
```

### 6.2 路径 A：管理员 / $MFT raw record

> **明确机制**：本路径用 **"直接打开 `$MFT` 文件 + ReadFile 按字节偏移读 record"**，**不**用 `FSCTL_GET_NTFS_FILE_RECORD`（后者每次只查一条 record，且输入 `MinFileReference` 输出的 ref number 可能不是同一 ref，不适合全盘扫描）。

**API 调用链**：

```
CreateFileW(r"\\.\C:")                              → volume handle (GENERIC_READ)
  ↓
DeviceIoControl(FSCTL_GET_NTFS_VOLUME_DATA)         → BytesPerFileRecordSegment / MftValidDataLength
                                                      （不需要 MftStartLcn —— 那是给 FSCTL_GET_NTFS_FILE_RECORD 用的）
  ↓
CreateFileW(r"\\.\C:$MFT")                          → $MFT 文件句柄
                                                      （需要 admin 或 SeBackupPrivilege；
                                                        普通用户在这里失败 → 降级到 USN 路径）
  ↓
按 BytesPerFileRecordSegment 分块 ReadFile（默认每 record 1024 字节，1 MB buffer）
  ↓
每条 FILE record 解析：
  - $FILE_NAME 属性                 → 文件名 + parent reference
  - $STANDARD_INFORMATION 属性       → timestamps + attributes
  - $DATA 属性（resident/non-resident） → size
  ↓
filter 早：matcher(name) ? 留 : 弃
  ↓
全部 record 解析完成后，用 HashMap<FileRef, (name, parent_ref)> 反向重建路径
```

**关键技术点**：
- **流式读取**：MFT 可能几百 MB，按 1 MB buffer 分块 ReadFile，避免一次性 mmap 内存峰值。读取按 `BytesPerFileRecordSegment` 偏移推进。
- **属性 walk**：FILE record 由 multi-attribute header 链表组成，按 type code 顺序遍历；遇到 $ATTRIBUTE_LIST 时需递归 fetch 额外 record（罕见但要处理）。
- **路径重建**：扫完后建 `HashMap<FileReference, Record>`，对每个匹配 record 递归向上找根（NTFS 根目录 reference = 5）。最坏 O(N·D)，D 是平均目录深度（< 10）。
- **stale ref 容忍**：杀软实时改 MFT 会导致我们读到"半新半旧"的快照——`rebuild_paths` 遇到悬空 parent reference 时**不 panic**，emit `ProgressEvent::EntryError { path: None, error: "stale parent ref" }`，跳过该条继续。
- **matcher 早过滤**：解析 $FILE_NAME 拿到文件名立即调 matcher，不匹配直接丢弃 record，避免内存浪费。

### 6.3 路径 B：普通用户 / USN 枚举

**API 调用链**：

```
CreateFileW(r"\\.\C:")                              → volume handle（不需要 admin）
  ↓
DeviceIoControl(FSCTL_QUERY_USN_JOURNAL)           → UsnJournalID / FirstUsn / NextUsn
  ↓
循环 DeviceIoControl(FSCTL_ENUM_USN_DATA, &USN_ENUM_DATA_V2 {
        StartFileReference: 0x0000000000000000,
        LowUsn:  0,
        HighUsn: NextUsn,
    })
  ↓
每块返回 buffer，前 8 字节是下次 StartFileReference，后面是变长 USN_RECORD V2 链
  ↓
每条 USN_RECORD V2 解析：
  - FileReferenceNumber         (64-bit)
  - ParentFileReferenceNumber   (64-bit)
  - TimeStamp                   (modified only)
  - Reason                      (忽略)
  - FileAttributes
  - FileLength
  - FileName (UTF-16，变长)
  ↓
filter: matcher(file_name) ? 留 : 弃
  ↓
路径重建（同 A 路径，复用 rebuild_paths 模块）
```

**与 A 路径的关键差异**：
- USN 路径**只有 modified 时间**，`created` / `accessed` 设为 `SystemTime::UNIX_EPOCH` 表示 unknown
- USN V2 仅 NTFS；ReFS 用 V3/V4（128-bit ref），非 NTFS 卷直接降级 walkdir

### 6.4 权限探测（不靠 TokenElevation）

```rust
// crates/ntfs-search/src/backend/windows/probe.rs
pub(super) fn probe_backend_for_root(root: &Path) -> BackendKind {
    let drive = root_to_drive_char(root);    // 'C'
    let mft_path = format!(r"\\.\{}:$MFT", drive);

    // 尝试以 GENERIC_READ 打开 $MFT
    if let Ok(handle) = CreateFileW(
        &pcwstr(mft_path),
        GENERIC_READ.0,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        None,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        None,
    ) {
        let _ = CloseHandle(handle);
        return BackendKind::Mft;
    }

    // 尝试 USN
    if usn_journal_available(drive) {
        return BackendKind::Usn;
    }

    // 退到 walkdir
    BackendKind::Walkdir
}
```

**不靠 `TokenElevation`**：用户可能用 `runas /trustlevel:0x20000` 这种半权限模式，TokenElevation=false 但实际能读 $MFT。**直接探测目标能力**更稳。

### 6.5 多盘独立 backend

`find_files` 为每个 root **独立**探测 backend，不强制全盘统一。C 盘 admin + D 盘 non-admin 完全正常。

降级流程：

```
find_files(roots=[C:\, D:\])
  ├─ probe C → Mft，spawn MftBackend.scan_root(C:\)
  │      └─ 若 MftBackend 失败 → Usn → Walkdir，emit BackendDowngraded
  ├─ probe D → Usn，spawn UsnBackend.scan_root(D:\)
  │      └─ 若 UsnBackend 失败 → Walkdir
  └─ join_all → merge results → 返回
```

每盘失败不阻断其他盘；所有盘都失败才报 `NoBackendAvailable`。

### 6.6 实现代码量

| 模块 | 行数 | 难点 |
| --- | --- | --- |
| `windows/volume.rs` CreateFileW + IOCTL 封装 | ~150 | windows-rs 类型映射 |
| `windows/mft.rs` FILE record 字节解析 + 属性 walk | ~500 | $ATTRIBUTE_LIST 递归展开 |
| `windows/mft.rs` $MFT 流式读取 + 路径重建 | ~250 | HashMap 内存控制（Arc<str> 共享） |
| `windows/usn.rs` USN_ENUM_DATA_V2 + record 解析 | ~200 | USN_RECORD V2/V3/V4 变长对齐 |
| `windows/probe.rs` 探测降级 | ~80 | 简单 |
| **Windows 端合计** | **~1180** | |

---

## 7. 跨平台 fallback（macOS/Linux walkdir）

### 7.1 设计要点

| 维度 | 决策 |
| --- | --- |
| **递归实现** | `walkdir` crate（单线程） |
| **符号链接** | 默认不跟随（`follow_links(false)`），避免循环 |
| **元数据** | walkdir 内部已经 `metadata()` 一次，零额外 IO |
| **默认 roots** | macOS：`[/Applications, ~]`；Linux：`[~]`（聚焦实际放客户端的位置） |
| **进度频率** | 每 1000 entry emit 一次 `EntriesFound` |
| **取消检查** | 每个目录切换时检查 `cancel.is_cancelled()` |
| **错误隔离** | 单个 entry metadata 失败不阻断，emit `EntryError` 继续 |
| **mdfind / locate** | **不做**——Spotlight 用户可能关闭；locate 依赖 cron，都不可靠 |

### 7.2 Walkdir backend 实现概要

```rust
// crates/ntfs-search/src/backend/walkdir.rs
pub(super) struct WalkdirBackend;

#[async_trait]
impl Backend for WalkdirBackend {
    fn kind(&self) -> BackendKind { BackendKind::Walkdir }

    async fn scan_root(
        &self,
        root: &Path,
        opts: &NtfsScanOptions,
        progress: &dyn ProgressSink,
        cancel: &CancellationToken,
    ) -> Result<Vec<FileEntry>, ScanError> {
        let root = root.to_owned();
        let cancel = cancel.clone();
        let max_results = opts.max_results;
        let max_records = opts.max_records_scanned.unwrap_or(usize::MAX);

        tokio::task::spawn_blocking(move || {
            let mut found = Vec::new();
            let mut scanned = 0usize;
            let mut last_emit = Instant::now();

            for entry in walkdir::WalkDir::new(&root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !is_symlink_to_avoid(e))
            {
                if cancel.is_cancelled() { break; }

                let entry = match entry { Ok(e) => e, Err(e) => {
                    // emit EntryError 但不阻断
                    continue;
                }};
                let meta = match entry.metadata() { Ok(m) => m, Err(_) => continue };
                scanned += 1;

                if let Some(name) = entry.file_name().to_str() {
                    if (opts.matcher)(name) {
                        found.push(file_entry_from_meta(&entry.path(), &meta, BackendKind::Walkdir));
                    }
                }

                // 周期性 emit
                if scanned % 1000 == 0 && last_emit.elapsed() > Duration::from_millis(100) {
                    progress.emit(ProgressEvent::EntriesFound { count: found.len() });
                    last_emit = Instant::now();
                }

                // 上限保护
                if scanned >= max_records {
                    progress.emit(ProgressEvent::ScanLimitHit {
                        kind: ScanLimitKind::RecordsScanned,
                        limit: max_records,
                    });
                    break;
                }
                if let Some(max) = max_results {
                    if found.len() >= max {
                        progress.emit(ProgressEvent::ScanLimitHit {
                            kind: ScanLimitKind::Results,
                            limit: max,
                        });
                        break;
                    }
                }
            }

            Ok(found)
        })
        .await
        .map_err(|e| ScanError::Internal(format!("join error: {e}")))?
    }
}
```

### 7.3 spawn_blocking 必要性

walkdir 是**同步迭代器**，在 async 上下文里直接用会阻塞 runtime worker。包到 `spawn_blocking` 里挪到阻塞线程池——Tokio 官方推荐模式。

**Windows MFT/USN backend 同理**：底层都是同步 `DeviceIoControl`，统一在 `spawn_blocking` 内做。`find_files` 整体是 async 只是为了**多盘并行 + cancel/timeout 的语义**。

### 7.4 多盘并行

```rust
// crates/ntfs-search/src/find.rs
pub async fn find_files(
    opts: NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
) -> Result<Vec<FileEntry>, ScanError> {
    let backends: Vec<(PathBuf, Box<dyn Backend>)> = opts.roots.iter()
        .map(|root| (root.clone(), select_backend_for_root(root)))
        .collect();

    let tasks = backends.into_iter().map(|(root, backend)| {
        let progress = Arc::clone(&progress);
        let cancel = cancel.clone();
        let opts = opts.clone();
        tokio::spawn(async move {
            progress.emit(ProgressEvent::DriveStarted {
                root: root.clone(),
                backend: backend.kind(),
            });
            let result = backend.scan_root(&root, &opts, &*progress, &cancel).await;
            (root, result)
        })
    }).collect::<Vec<_>>();

    let mut all = Vec::new();
    let mut skipped = Vec::new();
    for handle in tasks {
        match handle.await {
            Ok((root, Ok(entries))) => {
                progress.emit(ProgressEvent::DriveCompleted {
                    root: root.clone(),
                    scanned: entries.len(),  // 简化；实际应记每盘 scanned count
                    found: entries.len(),
                });
                all.extend(entries);
            }
            Ok((root, Err(e))) => {
                skipped.push((root, e));
            }
            Err(join_err) => return Err(ScanError::Internal(join_err.to_string())),
        }
    }

    // 全部盘都失败才报错
    if all.is_empty() && !skipped.is_empty() {
        return Err(ScanError::NoBackendAvailable {
            root: "all".to_string(),
        });
    }

    for (root, e) in skipped {
        progress.emit(ProgressEvent::DriveSkipped {
            root,
            reasons: vec![e.to_string()],
        });
    }

    Ok(all)
}
```

### 7.5 完整代码量估算（更新）

| 模块 | 行数 |
| --- | --- |
| Windows `$MFT` 直接读 + 属性解析 | ~750 |
| Windows USN 枚举 + record 解析 | ~250 |
| Windows volume / probe | ~230 |
| 路径重建 util（MFT + USN 共用） | ~150 |
| Unix walkdir backend | ~180 |
| `find.rs` 总入口 + 多盘并行 + cancel/timeout | ~120 |
| `inspect.rs` / `inspect_many` 并发 fetch | ~250 |
| `pe.rs` VS_VERSION_INFO 解析 | ~150 |
| options / error / matcher / lib 公开类型 | ~200 |
| **总计** | **~2280 行**（不含测试） |

测试再加 ~600 行 → **~2900 行**。

---

## 8. 错误处理

### 8.1 ScanError 枚举（thiserror derive）

```rust
// crates/ntfs-search/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("invalid root path: {0}")]
    InvalidRoot(String),

    #[error("failed to open volume {root}: {source}")]
    VolumeOpenFailed {
        root: String,
        #[source]
        source: std::io::Error,
    },

    #[error("MFT raw read failed on {root}: {detail}")]
    MftReadFailed { root: String, detail: String },

    #[error("USN enumeration failed on {root}: {detail}")]
    UsnEnumFailed { root: String, detail: String },

    #[error("no backend available for {root} (MFT/USN/walkdir all failed)")]
    NoBackendAvailable { root: String },

    #[error("scan cancelled by user")]
    Cancelled,

    #[error("inspect failed for {path}: {detail}")]
    InspectFailed { path: PathBuf, detail: String },

    #[error("internal error: {0}")]
    Internal(String),
}
```

### 8.2 错误分类与处理策略

| 错误 | 类别 | 调用方应如何处理 |
| --- | --- | --- |
| `InvalidRoot` | **Fatal** 用户输入错 | 显示错误，不重试 |
| `VolumeOpenFailed` | **Recoverable** 该盘扫不了 | crate 内部自动降级；若降级链全失败，单盘跳过 |
| `MftReadFailed` | **Recoverable** | crate 内部自动降级到 USN/walkdir |
| `UsnEnumFailed` | **Recoverable** | crate 内部自动降级到 walkdir |
| `NoBackendAvailable` | **Recoverable** 单盘失败但其他盘成功 | crate 内部聚合结果；仅当全部盘失败才返回 |
| `Cancelled` | **Expected** 用户主动取消 | crate **返回 `Err(ScanError::Cancelled)`**；调用方自行决定是转 `Ok(vec![])` 还是 propagate 到 `ManagerError::Cancelled` |
| `InspectFailed` | **Single-entry** | inspect_many 内部跳过，`InspectOutcome::Failed` 标记 |
| `Internal` | **Bug** | 显示错误 + 日志 |

> **取消语义已统一**：crate 内部不在 `find_files` 里"静默吞掉"取消信号转 `Ok(vec![])`——明确返回 `Err(ScanError::Cancelled)`，让调用方有清晰的判断点。DDNet-Manager 这边的桥接 `From<ScanError> for ManagerError` 把它映射到 `ManagerError::Cancelled`，前端按 `code = "cancelled"` 显示"用户取消"。

### 8.3 降级不报错，失败聚合

`find_files` **不**因为单个盘失败就返回 `Err`——它会：

1. 探测该盘的可用 backend 链：`Mft → Usn → Walkdir`
2. 链上任一 backend 成功就用它
3. 全部失败 → emit `ProgressEvent::DriveSkipped { root, reasons }`，**跳过该盘继续其他盘**
4. 全部盘都失败 → 返回 `Err(ScanError::NoBackendAvailable { root: "all" })`

### 8.4 与 ManagerError 的桥接

ntfs-search 是独立 crate，**不感知** ManagerError 存在。桥接由 DDNet-Manager 的调用方做：

```rust
// src-tauri/src/client_scan.rs
impl From<ScanError> for ManagerError {
    fn from(e: ScanError) -> Self {
        match e {
            ScanError::Cancelled => ManagerError::Cancelled,
            ScanError::InvalidRoot(msg) => ManagerError::NotFound(msg),
            ScanError::NoBackendAvailable { root } => {
                ManagerError::NotFound(format!("no scan backend for {root}"))
            }
            other => ManagerError::Internal(other.to_string()),
        }
    }
}
```

### 8.5 日志策略（tracing）

crate 内部用 `tracing`：

```rust
use tracing::{warn, info, debug};

info!(root = %root.display(), backend = ?backend.kind(), "starting scan");
warn!(error = %e, "MFT backend failed, falling back to USN");
debug!(scanned, found, "scan progress");
```

DDNet-Manager 的 `main.rs` 已经有 tracing subscriber，自动捕获。

---

## 9. 测试策略

### 9.1 测试金字塔

| 层 | 工具 | 覆盖范围 | 在 CI 跑 |
| --- | --- | --- | --- |
| **单元测试**（inline `#[cfg(test)]`） | std `assert_eq!` / `assert_matches!` | MFT record 字节解析、USN record 解析、路径重建算法、FileAttributes bitflags、matcher 包装、NtfsScanOptions builder | ✅ 全平台 |
| **属性测试** | proptest | MFT record 反序列化对随机字节流不 panic；路径重建对随机 parent map 必终止 | ✅ 全平台 |
| **fixture 集成测试** | tempfile + walkdir | Walkdir backend 行为；inspect_many 并发；取消语义；max_results/max_records_scanned 限制 | ✅ 全平台 |
| **真盘集成测试** | `#[ignore]` 标记 | MFT/USN backend 在真 C 盘上的端到端扫描；admin 探测；降级流程 | ❌ 仅开发者本地手动跑 |

### 9.2 Fixture 设计

```
crates/ntfs-search/tests/fixtures/
├── mft_records/
│   ├── minimal_file.bin         # 一个最小的 FILE record
│   ├── large_with_dataruns.bin  # 大文件（$DATA 非常驻）
│   ├── directory.bin            # 目录 record
│   └── corrupted.bin            # 损坏数据（确保解析 fail-fast 不 panic）
├── usn_records/
│   ├── v2_simple.bin
│   └── v4_128bit_ref.bin
└── trees/
    ├── linear.txt               # parent map: A←B←C←file
    ├── cyclic.txt               # 故意构造循环（路径重建必须检测并终止）
    └── orphan.txt               # parent ref 指向不存在的 record
```

### 9.3 Fixture 制作流程（dump_fixture.rs）

**关键安全保证**：所有 fixture **只 dump 我们自己临时创建的文件**，零隐私风险。

**关键一致性问题**：fixture dump 必须与生产 mft.rs 用**同一机制**——"打开 `$MFT::$DATA` + 按 `BytesPerFileRecordSegment` 偏移读对应 record"，**不能**用 `FSCTL_GET_NTFS_FILE_RECORD`（后者内核会按需读 MFT 区段且返回的 ref 可能错位，dump 出的字节流跟生产 ReadFile 读到的格式可能不一致）。

```rust
// crates/ntfs-search/examples/dump_fixture.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 临时创建文件，文件名天生无害
    let temp_dir = tempfile::tempdir()?;
    let test_file = temp_dir.path().join("ntfs_search_test_ddnet.exe");
    std::fs::write(&test_file, b"fixture content")?;

    // 2. 通过路径查到该文件在 MFT 中的 record 序号
    let file_ref = windows::get_file_reference_number(&test_file)?;
    let record_index = file_ref & 0x0000_FFFF_FFFF_FFFF;   // 低 48 bit

    // 3. 打开 $MFT::$DATA 句柄（与生产 mft.rs 同一函数）
    let volume = windows::open_volume_readonly('C')?;
    let mft_handle = windows::open_mft_file('C')?;
    let bytes_per_record = windows::query_bytes_per_file_record_segment(&volume)?;

    // 4. 按 offset 读对应 record 字节流
    let mut buf = vec![0u8; bytes_per_record as usize];
    windows::read_mft_record(&mft_handle, record_index, &mut buf)?;

    // 5. 写入 fixture（项目相对路径）
    let out = std::path::Path::new("tests/fixtures/mft_records/minimal_file.bin");
    std::fs::write(out, &buf)?;

    // 6. 临时文件自动清理（temp_dir drop）
    Ok(())
}
```

dump 多种典型 record：
- `minimal_file.bin` —— 普通 1KB 文件
- `large_with_dataruns.bin` —— 大文件（dump 临时创建的 100MB sparse 文件）
- `directory.bin` —— 目录（dump 临时创建的目录）
- `with_attribute_list.bin` —— 属性多到要 $ATTRIBUTE_LIST（人为制造大量 $EA 等触发）

**验证 fixture 可被 mft.rs parser 正确解析**：dump 后立即在 dump_fixture.rs 末尾调一次 `parse_file_record(&buf)`，确保不 panic 且字段对齐。

### 9.4 单元测试样例

```rust
// src/backend/windows/mft.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_file_record() {
        let bytes = include_bytes!("../../../tests/fixtures/mft_records/minimal_file.bin");
        let record = parse_file_record(bytes).expect("parse");
        assert_eq!(record.file_name(), Some("ntfs_search_test_ddnet.exe"));
        assert!(record.attributes().contains(FileAttributes::ARCHIVE));
    }

    #[test]
    fn rejects_corrupted_record_without_panic() {
        let bytes = include_bytes!("../../../tests/fixtures/mft_records/corrupted.bin");
        let result = parse_file_record(bytes);
        assert!(result.is_err());
    }

    proptest! {
        #[test]
        fn parser_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
            let _ = parse_file_record(&bytes);  // 只要不 panic 就 OK
        }
    }
}
```

### 9.5 路径重建测试

```rust
#[test]
fn rebuilds_path_from_parent_chain() {
    let map = build_test_map_from("tests/fixtures/trees/linear.txt");
    let path = rebuild_path(&map, file_ref("file")).unwrap();
    assert_eq!(path, PathBuf::from(r"C:\A\B\C\file"));
}

#[test]
fn detects_cyclic_parent_chain() {
    let map = build_test_map_from("tests/fixtures/trees/cyclic.txt");
    let result = rebuild_path(&map, file_ref("file"));
    assert!(matches!(result.err(), Some(RebuildError::CycleDetected)));
}

#[test]
fn handles_orphan_parent_gracefully() {
    let map = build_test_map_from("tests/fixtures/trees/orphan.txt");
    let path = rebuild_path(&map, file_ref("file")).unwrap_or_default();
    assert!(path.starts_with("<orphan>"));
}
```

### 9.6 Walkdir backend 集成测试（CI 跑得起来）

```rust
#[tokio::test]
async fn walkdir_backend_finds_by_name() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("DDNet.exe"), b"fake").unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub").join("DDNet.exe"), b"fake").unwrap();
    std::fs::write(tmp.path().join("other.txt"), b"noise").unwrap();

    let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("DDNet.exe"))
        .with_root(tmp.path().to_owned());
    let entries = find_files(opts, Arc::new(NoopSink), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(entries.len(), 2);
}

#[tokio::test]
async fn walkdir_backend_respects_max_results() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..100 {
        std::fs::write(tmp.path().join(format!("f{i}.exe")), b"x").unwrap();
    }
    let opts = NtfsScanOptions::new(|n| n.ends_with(".exe"))
        .with_root(tmp.path().to_owned())
        .with_max_results(5);
    let entries = find_files(opts, Arc::new(NoopSink), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(entries.len(), 5);
}

#[tokio::test]
async fn walkdir_backend_respects_cancellation() {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..100_000 {
        std::fs::write(tmp.path().join(format!("f{i}.exe")), b"x").unwrap();
    }
    let opts = NtfsScanOptions::new(|_| true).with_root(tmp.path().to_owned());
    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel2.cancel();
    });
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        find_files(opts, Arc::new(NoopSink), cancel),
    ).await;
    assert!(result.is_ok(), "should not hang after cancel");
}
```

### 9.7 真盘集成测试（手动）

```rust
#[cfg(windows)]
#[tokio::test]
#[ignore = "needs real C: drive; run with --ignored"]
async fn real_mft_backend_scans_c_drive() {
    let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("notepad.exe"))
        .with_root(PathBuf::from("C:\\"));
    let progress = Arc::new(CountingSink::default());
    let entries = find_files(opts, progress.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert!(entries.iter().any(|e| e.path.to_string_lossy().contains("System32")));
    assert!(progress.backends_seen().contains(&BackendKind::Mft));
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "needs admin elevation to test MFT path"]
async fn real_mft_admin_path_takes_precedence() {
    let opts = NtfsScanOptions::new(|_| false)
        .with_root(PathBuf::from("C:\\"));
    let progress = Arc::new(CountingSink::default());
    let _ = find_files(opts, progress.clone(), CancellationToken::new()).await;
    assert!(progress.backends_seen().contains(&BackendKind::Mft));
}
```

### 9.8 CI 配置

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Test ntfs-search
        run: cargo test -p ntfs-search --verbose

  test-ignored:
    if: github.event_name == 'workflow_dispatch'
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run ignored real-disk tests
        run: cargo test -p ntfs-search -- --ignored --nocapture
```

### 9.9 测试覆盖目标

| 模块 | 行覆盖目标 |
| --- | --- |
| `parser_mft.rs` 字节解析 | ≥ 90% |
| `parser_usn.rs` 字节解析 | ≥ 90% |
| `rebuild_paths.rs` | 100% |
| `walkdir_backend.rs` | ≥ 85% |
| `windows/mft.rs`（IO 部分） | 不强制（fixture 已覆盖解析层） |
| `find.rs` 调度层 | ≥ 70% |
| `inspect.rs` | ≥ 75% |
| **整 crate** | ≥ 80% |

---

## 10. 磁盘安全保证

### 10.1 三条铁律

#### 铁律 1：所有句柄都用 `GENERIC_READ`，绝不请求 `WRITE`

```rust
// crates/ntfs-search/src/backend/windows/volume.rs
pub(super) fn open_volume_readonly(drive: char) -> Result<HANDLE, ScanError> {
    let path = format!(r"\\.\{}:", drive);
    let mut wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

    let handle = unsafe {
        CreateFileW(
            PCWSTR::from_raw(wide.as_mut_ptr()),
            GENERIC_READ.0,                          // ← 只读
            FILE_SHARE_READ | FILE_SHARE_WRITE,      // ← 允许其他进程同时读写
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }?;

    Ok(handle)
}
```

**对比危险写法**（坚决不用）：

```rust
// ❌ 反面教材
GENERIC_READ.0 | GENERIC_WRITE.0
FILE_SHARE_NONE          // 独占，会阻塞 Listary/chkdsk
```

#### 铁律 2：只调用只读 NTFS IOCTL

| IOCTL | 类别 | 用 |
| --- | --- | --- |
| `FSCTL_GET_NTFS_FILE_RECORD` | 只读 | ✅ |
| `FSCTL_ENUM_USN_DATA` | 只读 | ✅ |
| `FSCTL_QUERY_USN_JOURNAL` | 只读 | ✅ |
| `FSCTL_READ_USN_JOURNAL` | 只读 | ✅ |
| `FSCTL_GET_NTFS_VOLUME_DATA` | 只读 | ✅ |
| `FSCTL_SET_REPARSE_POINT` | **改文件** | ❌ |
| `FSCTL_DELETE_REPARSE_POINT` | **改文件** | ❌ |
| `FSCTL_WRITE_USN_CLOSE_RECORD` | **写** | ❌ |

#### 铁律 3：和 Listary / Everything 完美共存

NTFS 设计上就是**多读单写**：
- Listary 启动后会持续持有 `$MFT::$DATA` 的 read handle（用于实时 USN 监控）
- Everything 同样持有 read handle
- 我们也持有 read handle
- 三家同时持有 read handle **完全合法**，NTFS 内核层无锁冲突
- 我们用 `FILE_SHARE_READ | FILE_SHARE_WRITE` 让其他进程可以同时打开

唯一会冲突的场景：
- 用户正在运行 `chkdsk /f`（独占锁定卷）—— 我们 emit `VolumeOpenFailed`，调用方提示 "磁盘维护中，请稍后重试"
- 杀软/EDR 拦截 `CreateFileW(r"\\.\C:")` —— emit `BackendDowngraded(Mft → Usn → Walkdir)`，自动降级

### 10.2 跟磁盘健康 0 关系

我们既不写 $MFT，也不写任何用户文件。**读写过的元数据本身**不会因为读而损坏——读是幂等、零副作用的操作。

### 10.3 审计日志

```rust
// 启动 MFT 扫描前明确声明只读
tracing::info!(
    target: "ntfs_search::audit",
    roots = ?opts.roots,
    mode = "READ_ONLY",
    access = "GENERIC_READ | FILE_SHARE_READ | FILE_SHARE_WRITE",
    "starting ntfs-search scan"
);
```

任何对 `\\.\X:` 的非 `GENERIC_READ` 调用都会在 code review 阶段被拒绝（CLAUDE.md 安全规范）。

---

## 11. DDNet-Manager 集成

### 11.1 集成策略：取代 BFS

**业务层完全用 ntfs-search 替换**现有 BFS：

| 移除 | 保留 |
| --- | --- |
| ❌ `find_candidate_dirs` | ✅ `validate_client_dir`（业务校验 + identity 推断，与扫描无关） |
| ❌ `default_scan_roots` | ✅ `infer_client_identity`（保留，但配合 inspect 升级） |
| ❌ `find_everything_candidate_dirs` | ✅ Steam `libraryfolders.vdf` 解析（作为 roots 补充） |
| ❌ `find_steam_library_ddnet_roots`（碎片化版） | ✅ `include_saved_paths`（用户保存路径作为 roots 补充） |

**ntfs-search 内部** walkdir fallback 保留——这是 macOS/Linux/杀软拦截场景的最后退路，但**用户无感**（crate 自动选）。

### 11.2 改造后的 client_scan.rs

**关键修订**（针对 code-reviewer 必修项）：
- 业务层结构体 **重命名为 `ClientScanQuery`**，与 `ntfs_search::NtfsScanOptions` 明确区分
- **删除 sync `scan_client_installations` + `runtime.block_on` 包装层** —— Tauri command 本身已经在 tokio runtime 内，再 `block_on` 必然死锁
- 业务侧**新 async 函数 `scan_installations`**，由 Tauri command 直接 `.await`

```rust
// src-tauri/src/client_scan.rs（改造后）
use ntfs_search::{find_files, NtfsScanOptions, ProgressEvent, ProgressSink, ScanError, NoopSink};
use tokio_util::sync::CancellationToken;

const DDNET_EXECUTABLE_NAMES: &[&str] = &["DDNet.exe", "ddnet.exe"];

/// 业务层扫描参数（保留业务语义字段，与 crate 内部 NtfsScanOptions 解耦）
#[derive(Clone, Debug, Default)]
pub struct ClientScanQuery {
    /// 用户配置的扫描排除路径
    pub excluded_paths: Vec<PathBuf>,
    /// 是否包含已保存路径（registry 中存的客户端路径）
    pub include_saved_paths: bool,
    /// 限制最大结果数（默认 50）
    pub max_results: Option<usize>,
}

/// Tauri command 直接 await 的 async 函数
pub async fn scan_installations(
    query: &ClientScanQuery,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
) -> Result<Vec<ClientInstallation>, ManagerError> {
    // 1. 收集 roots：用户保存路径 + Steam VDF 推导 + 固定盘符
    let roots = collect_scan_roots(query);

    // 2. 构造 ntfs-search 选项
    let opts = NtfsScanOptions::new(|name| {
        DDNET_EXECUTABLE_NAMES.iter().any(|e| name.eq_ignore_ascii_case(e))
    })
    .with_roots(roots)
    .with_max_results(query.max_results.unwrap_or(50));

    // 3. 调 ntfs-search
    let entries = find_files(opts, progress, cancel)
        .await
        .map_err(ManagerError::from)?;

    // 4. 转换：FileEntry.path.parent() → validate_client_dir → ClientInstallation
    let mut installations = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for entry in entries {
        let Some(parent) = entry.path.parent() else { continue };
        if is_local_smoke_tmp_path(parent) { continue; }
        if is_excluded_path(parent, &query.excluded_paths) { continue; }
        let installation = validate_client_dir(parent)?;
        if seen_ids.insert(installation.id.clone()) {
            installations.push(installation);
        }
    }

    installations.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
    Ok(installations)
}

/// 收集扫描 roots：固定盘符 + 用户保存路径 + Steam VDF
fn collect_scan_roots(query: &ClientScanQuery) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    // 固定盘符（C, D 等本地盘）
    for drive in available_fixed_drives() {
        roots.push(PathBuf::from(format!("{}:\\", drive)));
    }

    // Steam VDF 推导（用户自定义 Steam 库）
    for steam_root in steam_libraryfolders_paths() {
        if let Some(parent) = steam_root.parent() {
            roots.push(parent.to_path_buf());
        }
    }

    // 用户保存的客户端路径
    if query.include_saved_paths {
        if let Some(saved) = load_saved_client_paths() {
            for path in saved {
                if let Some(parent) = path.parent() {
                    roots.push(parent.to_path_buf());
                }
            }
        }
    }

    roots.sort();
    roots.dedup();
    roots
}

/// 升级版 identity 推断：先用 PE 元数据，再回退路径模式
pub async fn infer_client_identity_with_pe(
    path: &Path,
) -> Result<ClientIdentity, ManagerError> {
    // 步骤 1：尝试 PE 元数据
    if let Ok(file_entry) = std::fs::metadata(path).map(|m| FileEntry::from_meta(path, &m)) {
        if let Ok(inspected) = ntfs_search::inspect(
            &file_entry,
            ntfs_search::InspectFields::VERSION_INFO,
        ).await {
            if let Some(vi) = inspected.version_info {
                if let Some(id) = identify_from_version_info(&vi) {
                    return Ok(id);
                }
            }
        }
    }

    // 步骤 2：回退路径模式（现有 infer_client_identity 逻辑）
    Ok(infer_client_identity(path))
}
```

**关键点**：
- 业务层 `ScanOptions` 改名 `ClientScanQuery`，`roots: Vec<PathBuf>` 字段不再暴露——roots 由 `collect_scan_roots(query)` 内部推导（固定盘 + Steam + 用户保存路径），调用方只需声明 `excluded_paths` / `include_saved_paths` / `max_results`
- Tauri command 层（§11.3）直接 `scan_installations(&query, sink, cancel).await`，零 `block_on`
- 旧的 sync `scan_client_installations` 函数完全删除（连同 `find_candidate_dirs` / `default_scan_roots` / `find_everything_candidate_dirs` 等碎片化策略）

### 11.3 Tauri command 增加进度事件

```rust
// src-tauri/src/commands/install.rs
#[tauri::command]
pub async fn scan_clients_via_mft(
    state: State<'_, AppState>,
    excluded_paths: Option<Vec<String>>,
    include_saved_paths: Option<bool>,
    max_results: Option<usize>,
    app: AppHandle,
) -> Result<Vec<ClientInstallation>, IpcError> {
    let query = ClientScanQuery {
        excluded_paths: excluded_paths.unwrap_or_default()
            .into_iter().map(PathBuf::from).collect(),
        include_saved_paths: include_saved_paths.unwrap_or(true),
        max_results,
    };

    let progress = std::sync::Arc::new(TauriEmitterSink::new(app));
    let cancel = CancellationToken::new();

    let installations = crate::client_scan::scan_installations(&query, progress, cancel)
        .await
        .map_err(|e| ManagerError::from(e))?;

    Ok(installations)
}

struct TauriEmitterSink {
    app: AppHandle,
}

impl ProgressSink for TauriEmitterSink {
    fn emit(&self, event: ProgressEvent) {
        // serde 序列化 + Tauri event
        let _ = self.app.emit("scan-progress", &event);
    }
}
```

**关键修订**：
- Tauri command 直接 `.await` 业务层 `scan_installations`，**不**调 `runtime.block_on`（避免死锁）
- 接收 `excluded_paths` / `include_saved_paths` / `max_results`，构造业务层 `ClientScanQuery`
- 业务层 `scan_installations` 内部构造 `ntfs_search::NtfsScanOptions`，调 `find_files`

### 11.4 前端集成

```typescript
// src/types/ipc.ts
export type ScanProgressEvent =
  | { kind: "drive_started"; root: string; backend: "mft" | "usn" | "walkdir" }
  | { kind: "entries_found"; found: number }
  | { kind: "drive_completed"; root: string; scanned: number; found: number }
  | { kind: "backend_downgraded"; root: string; from: string; to: string; reason: string }
  | { kind: "scan_limit_hit"; kind2: "records_scanned" | "results"; limit: number }
  | { kind: "drive_skipped"; root: string; reasons: string[] }
  | { kind: "entry_error"; path: string | null; error: string };

// src/hooks/useClientScanner.ts
export function useClientScanner() {
  // 多盘并行：聚合每个 root 的最新状态
  const [driveStates, setDriveStates] = useState<Map<string, DriveState>>(new Map());
  const [scanning, setScanning] = useState(false);
  const [results, setResults] = useState<ClientInstallation[]>([]);

  const start = useCallback(async (params?: ScanClientsParams) => {
    setScanning(true);
    setResults([]);
    setDriveStates(new Map());

    const unlisten = await listen<ScanProgressEvent>("scan-progress", (e) => {
      const ev = e.payload;
      setDriveStates(prev => {
        const next = new Map(prev);
        switch (ev.kind) {
          case "drive_started":
            next.set(ev.root, { backend: ev.backend, scanned: 0, found: 0, status: "scanning" });
            break;
          case "entries_found":
            // 找到当前扫描中的盘（最后一个 status=scanning 的），累加 found
            // 实际实现按 root 维度细分（entries_found 也带 root 字段）
            break;
          case "drive_completed":
            next.set(ev.root, { backend: "mft", scanned: ev.scanned, found: ev.found, status: "done" });
            break;
          case "backend_downgraded":
            // 显示降级提示
            break;
          case "drive_skipped":
            next.set(ev.root, { backend: null, scanned: 0, found: 0, status: "skipped", reasons: ev.reasons });
            break;
        }
        return next;
      });
    });

    try {
      const installations = await invoke<ClientInstallation[]>(
        "scan_clients_via_mft",
        params ?? {}
      );
      setResults(installations);
    } finally {
      unlisten();
      setScanning(false);
    }
  }, []);

  return { driveStates, scanning, results, start };
}

// src/components/ScanProgress.tsx —— 多盘聚合展示
export function ScanProgressList({ driveStates }: { driveStates: Map<string, DriveState> }) {
  return (
    <List>
      {Array.from(driveStates.entries()).map(([root, state]) => (
        <ListItem key={root}>
          <Text>扫描 {root}</Text>
          <Text muted>后端: {state.backend ?? "—"}</Text>
          {state.status === "scanning" && <Text>进行中...</Text>}
          {state.status === "done" && (
            <Text>扫描 {state.scanned} 条记录，找到 {state.found} 个候选</Text>
          )}
          {state.status === "skipped" && (
            <Text error>跳过：{state.reasons?.join(", ")}</Text>
          )}
        </ListItem>
      ))}
    </List>
  );
}
```

UI 展示样例（多盘并行）：

```
[扫描客户端]
扫描 C:\   后端: MFT (管理员)    | 已扫描 312,548 条 | 找到 7 个候选
扫描 D:\   后端: USN (普通用户) | 已扫描 89,234 条 | 找到 2 个候选
扫描 E:\   后端: Walkdir (降级) | 已扫描 12 条     | 找到 0 个候选
[完成] 共找到 9 个候选客户端
```

### 11.4.1 USN 路径时间字段前端特判

USN backend 不返回 `created` / `accessed` 时间（`SystemTime::UNIX_EPOCH` 表示 unknown）。前端展示客户端详情时需特判：

```typescript
// src/utils/time.ts
export function formatTimestamp(t: string | null | undefined): string {
  if (!t) return "未知";
  const date = new Date(t);
  // UNIX_EPOCH (1970-01-01) 视为 unknown 信号
  if (date.getFullYear() === 1970) return "未知（USN 模式）";
  return date.toLocaleString();
}

// 在 ClientInstallation 详情展示组件里：
<Text>创建时间：{formatTimestamp(installation.created)}</Text>
<Text>访问时间：{formatTimestamp(installation.accessed)}</Text>
<Text>修改时间：{formatTimestamp(installation.modified)}</Text>
```

**Mft backend** 返回完整时间；**USN / Walkdir backend** 返回部分或零时间——前端按 `"未知（USN 模式）"` 提示用户当前数据源不完整。

### 11.5 集成变更面汇总

| 文件 | 变更类型 | 内容 |
| --- | --- | --- |
| `Cargo.toml`（根，新建） | 新增 | workspace manifest |
| `src-tauri/Cargo.toml` | 修改 | 改成 workspace member；加 `ntfs-search = { path = "../crates/ntfs-search" }` |
| `src-tauri/src/client_scan.rs` | 重写 | 移除 BFS 系列；改为调 ntfs-search |
| `src-tauri/src/commands/install.rs` | 修改 | 加 `scan_clients_via_mft` command；保留原 `scan_clients` 作为兼容入口（可后续移除） |
| `src-tauri/src/models/error.rs` | 修改 | 加 `From<ScanError> for ManagerError` |
| `src/types/ipc.ts` | 新增 | `ScanProgressEvent` discriminated union |
| `src/hooks/useClientScanner.ts` | 新增 | 监听 `scan-progress` 事件 |
| `src/components/ScanProgress.tsx` | 新增 | 进度展示组件 |
| `src/pages/Installations.tsx` | 修改 | 接入新 hook，展示 backend 类型 |

---

## 12. 实施步骤与里程碑

### 12.1 commit 拆分（按 CLAUDE.md 多主题）

> **commit 4 拆分**：原 commit 4 单体 ~850 行（含 FILE record 解析 + $MFT 流式读 + 路径重建 + 探测降级）违反"单文件 < 1000 行"和"多主题拆分"约束，**拆为 4a / 4b / 4c 三个子 commit**。

| # | Commit | 内容 | 预计行数 | 依赖 |
| --- | --- | --- | --- | --- |
| 1 | `feat(workspace): 升级为 cargo workspace，加 crates/ntfs-search 骨架` | workspace Cargo.toml + crate 骨架 + lib.rs/options.rs/error.rs/matcher.rs 公开类型 | ~350 | 无 |
| 2 | `feat(ntfs-search): 实现 walkdir 跨平台 backend 与 find_files 入口` | backend/walkdir.rs + find.rs + cancel/timeout + 多盘并行 | ~450 | #1 |
| 3 | `feat(ntfs-search): 实现 USN 枚举 backend（普通用户路径）` | backend/windows/{volume,usn,probe}.rs + record 解析 + 路径重建 | ~450 | #2 |
| 4a | `feat(ntfs-search): 实现 $MFT FILE record 字节解析器与 fixture 单测` | parser_mft.rs（FILE record 属性 walk、$ATTRIBUTE_LIST 递归）+ fixture dump_fixture.rs + proptest | ~450 | #3 |
| 4b | `feat(ntfs-search): 实现 $MFT raw record backend（管理员极速路径）` | backend/windows/mft.rs（$MFT::$DATA 流式读 + 调 4a parser）+ 探测降级 | ~400 | #4a |
| 4c | `feat(ntfs-search): 完善 $MFT 路径重建与 stale ref 容忍` | rebuild_paths.rs 升级（cycle 检测、stale ref emit EntryError）+ 集成测试 | ~200 | #4b |
| 5 | `feat(ntfs-search): 实现 inspect/inspect_many 与 PE 版本资源解析` | inspect.rs + pe.rs（pelite）+ 并发信号量 | ~350 | #4c |
| 6 | `test(ntfs-search): 覆盖字节解析、路径重建、walkdir 集成测试` | fixtures + proptest + 单元/集成测试 | ~600 | #5 |
| 7 | `feat(client-scan): 集成 ntfs-search，前端新增扫描进度展示` | client_scan.rs 改造 + Tauri command + 前端 hook/组件 | ~400 | #6 |

**总计**：~3250 行（含测试）。

**commit 4 拆分理由**：
- 4a 纯字节解析 + 单测，零 IO，可独立单元测试
- 4b 接线 backend trait + $MFT IO + 探测降级，依赖 4a 的 parser
- 4c 路径重建 + stale ref 容忍，单独测试算法
- 每个 commit < 600 行，单一主题，独立可 review

### 12.2 里程碑节奏（渐进式交付）

每个 milestone 都能用，永远不会出现"全做完才能跑"的情况：

| 里程碑 | 包含 commit | 验证标准 | 价值 |
| --- | --- | --- | --- |
| **M1** | 1-2 | `cargo test -p ntfs-search` 全绿；能跨平台跑 walkdir | **那时就能完全替换现有 client_scan BFS** |
| **M2** | 3 | Windows 普通用户也能极速扫（USN） | Windows 体验大幅提升 |
| **M3a** | 4a | MFT FILE record 字节解析器单测全过；fixture 可解析 | MFT 解析层稳定 |
| **M3b** | 4b | admin 路径开启 | 达到 Listary 速度（5 秒扫 C 盘） |
| **M3c** | 4c | 路径重建含 stale ref 容忍 | 真实环境稳定性 |
| **M4** | 5-6 | inspect 上线 + 测试覆盖完整 | PE 元数据识别 + 质量保证 |
| **M5** | 7 | DDNet-Manager 集成上线 | 端到端可用 |

### 12.3 单 commit 内部结构

每个 commit 内部按以下顺序：

1. **先写测试**（CLAUDE.md：新增 Rust 代码默认先写测试）
2. 实现代码使测试通过
3. `cargo fmt` + `cargo clippy -- -D warnings` 通过
4. `make check-lint` 16 PASS / 2 WARN（剩余 WARN 同前）
5. Commit message 中文动宾短语，按 CLAUDE.md 规范

---

## 13. 验收清单

### 13.1 功能验收

- [ ] `cargo test -p ntfs-search` 全绿
- [ ] `cargo test -p ntfs-search -- --ignored` 在 Windows + admin 下真盘扫描通过
- [ ] `cargo test -p ntfs-search -- --ignored` 在 Windows + 普通用户下 USN 扫描通过
- [ ] `cargo test -p ntfs-search` 在 macOS / Linux 上 walkdir backend 通过
- [ ] `make check-lint` 16 PASS / 2 WARN（剩余 WARN 同前）
- [ ] 与 Listary 同时运行无冲突（双方都能扫到同样文件）
- [ ] 与 Everything 同时运行无冲突
- [ ] 在杀软环境下自动降级 walkdir（手动模拟：通过组策略禁用 raw volume access）

### 13.2 性能验收

- [ ] Windows admin: C 盘全扫 < 5 秒（百万文件级）
- [ ] Windows 普通用户: C 盘全扫 < 30 秒
- [ ] macOS: 用户目录全扫 < 5 秒（十万文件级）
- [ ] 内存峰值: 扫 C 盘 < 200 MB（HashMap 控制）
- [ ] max_results 提前退出有效（设 5，找到 5 个立即返回）

### 13.3 安全审计

- [ ] `\\.\X:` 句柄审计：全部 `GENERIC_READ`
- [ ] 全部 DeviceIoControl 调用：白名单内（只读 IOCTL）
- [ ] 无任何 `WRITE` / `DELETE` / `FILE_SHARE_NONE` 调用
- [ ] `tracing::audit` 日志在每次扫描前记录只读声明

### 13.4 代码质量

- [ ] `cargo clippy --workspace -- -D warnings` 0 warning
- [ ] 无 `.unwrap()` / `.expect()`（除测试代码和静态不变量）
- [ ] 所有公开 API 有 doc comment
- [ ] README.md 说明使用方法 + 磁盘安全保证

---

## 14. 风险与缓解

| 风险 | 等级 | 缓解 |
| --- | --- | --- |
| **$MFT 字节解析 bug 导致 panic** | 高 | proptest fuzz + fixture 单测覆盖；解析器 fail-fast 返回 `Err` |
| **杀软/EDR 拦截 `\\.\C:` 打开** | 高 | 自动降级 USN → walkdir；emit `BackendDowngraded` 提示用户 |
| **路径重建的循环依赖**（NTFS 损坏） | 中 | `rebuild_paths` 检测 cycle，返回 `RebuildError::CycleDetected` |
| **HashMap 内存峰值超 500 MB** | 中 | `max_records_scanned` 默认 5,000,000；超限 emit `ScanLimitHit` 返回部分结果 |
| **walkdir 在 `/` 全扫卡死** | 中 | 默认 roots 不含 `/`；DDNet-Manager 调用方决定 |
| **`spawn_blocking` 线程池耗尽** | 低 | 单盘扫占用 1 个阻塞线程；8 盘并行占 8 个；默认池 512 个够用 |
| **Windows async IO 跨平台麻烦** | 已规避 | 用 `spawn_blocking` 包装同步 IO，不追求原生 async IO |
| **MFT fixture 字节过时**（新版 NTFS 加字段） | 低 | fixture 注明 NTFS 版本来源；解析器对未知字段 skip 而非 panic |
| **proptest 在 CI 上偶发 fail** | 低 | 固定 `ProptestConfig::failure_persistence`，失败 case 自动写入 `proptest-regressions/` |
| **Windows-only 模块在 Linux CI 上 `cargo test` 报错** | 低 | `#[cfg(windows)]` 严格标注；CI matrix 含 windows + ubuntu |
| **USN V2 vs V4 兼容性** | 低 | 解析器按版本字段分支；V4 用 128-bit FileReference |
| **crate 公开后用户用作恶意用途（勒索软件扫描目标）** | 低 | MIT/Apache 协议明示"使用者自担风险"；README 强调合法用途 |

---

## 15. 参考实现

### 15.1 直接参考的源码

| 项目 | 协议 | 用途 |
| --- | --- | --- |
| [everything-cli](https://www.voidtools.com/) (voidtools) | GPLv2 | $MFT 直接读取的算法骨架（**不抄代码**，仅参考算法） |
| [ntfs-search](https://github.com/ColinFinck/ntfs) (ColinFinck) | MIT | MFT record 解析的字段映射 |
| [Listary](https://www.listary.com/) | 商业 | UI 模式参考；底层技术文档无公开 |
| [windows-rs](https://github.com/microsoft/windows-rs) | MIT | NTFS IOCTL 的 Rust 绑定 |

**协议注意**：本项目 `ntfs-search` crate 用 **MIT OR Apache-2.0** 双协议。参考 everything-cli 时**只能参考算法思路**，不复制代码（GPLv2 会污染）。

### 15.2 微软官方文档

- [NTFS Technical Reference](https://learn.microsoft.com/en-us/windows-server/storage/file-system/ntfs-overview)
- [FSCTL_GET_NTFS_FILE_RECORD](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_get_ntfs_file_record)
- [FSCTL_ENUM_USN_DATA](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_enum_usn_data)
- [USN_RECORD structure](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-usn_record_v2)
- [MFT Record Structure](https://learn.microsoft.com/en-us/windows/win32/devnotes/master-file-table)

### 15.3 关键算法文献

- "Windows NT File System Internals" (Rajeev Nagar, 1997) —— MFT 结构权威书籍
- " Investigating NTFS's $MFT" (NTFS.com 技术文章) —— FILE record 字段详解

---

## 16. 后续演进（v0.2+，非本次范围）

| 演进项 | 价值 | 时机 |
| --- | --- | --- |
| **USN 增量缓存** | 启动时全量扫 + 后台 USN 监控，二次扫描 < 100ms | v0.1 验证用户体量后 |
| **FileHash / InspectFields::FILE_HASH** | SHA-256 文件指纹，用于重复文件检测 | v0.1 砍掉后视需求补回 |
| **正则匹配** | 调用方需要复杂匹配模式 | 有用户请求时 |
| **发布到 crates.io** | 让其他 Rust 项目受益 | v0.2 稳定后 |
| **Service 化（Windows Service）** | 持续后台索引，热键秒级响应 | 与启动器定位不符，不做 |
| **GUI**（独立 CLI 工具） | 调试用 | 视需求 |
| **APFS / ext4 直接读** | macOS/Linux 提速 | 极难，价值低，不做 |

---

## 17. 总结

`ntfs-search` crate 的核心价值：

1. **业务层彻底简化**：`client_scan.rs` 从 658 行的碎片化策略 → ~200 行的 "调 ntfs-search + 转 ClientInstallation"
2. **性能跨越**：Windows 上从十几秒 → admin 5 秒 / 普通 30 秒
3. **覆盖完整**：用户自定义路径、Steam 库、移动硬盘、网络驱动器（NTFS）都能扫
4. **职责干净**：crate 只做"全盘找文件"，业务判断留给上层
5. **演进路径清晰**：v0.1 是 Listary 风格全量；v0.2 加 USN 增量；v0.3 视需求扩展

整个演进过程**渐进式交付**：M1（commit 1-2）跑通就立刻能替换现有 BFS，后续每个 milestone 都是叠加增量价值。
