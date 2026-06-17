//! crate 公开类型：扫描选项、文件条目、进度事件、扩展信息字段。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// 全量扫描的入口选项。
///
/// 命名前缀 `Ntfs` 是为了避免与业务层（如 DDNet-Manager 的 `ClientScanQuery` / `ScanOptions`）
/// 同名混淆——crate 内部用 `NtfsScanOptions`，业务层用各自的语义命名。
#[derive(Clone)]
pub struct NtfsScanOptions {
    /// 待扫描的根路径列表。Windows 上一般是 `["C:\\", "D:\\"]`，Unix 上是 `["/", "~"]` 等。
    pub roots: Vec<PathBuf>,

    /// 文件名匹配闭包。对每个候选文件的 `file_name()` 调用一次，返回 `true` 留下。
    ///
    /// 内部用 `Arc<dyn Fn>` 而非 `Box<dyn Fn>` —— 后者无法 `Clone`，会让
    /// `NtfsScanOptions` 失去 `Clone` 能力，多盘并行场景下无法复制。
    pub matcher: Arc<dyn Fn(&str) -> bool + Send + Sync + 'static>,

    /// 累计找到 `max_results` 个匹配 entry 后立即结束扫描（软限制，不报错）。
    pub max_results: Option<usize>,

    /// 累计解析 `max_records_scanned` 条 record 后停止（内存保护，软限制）。
    ///
    /// 默认 `Some(2_000_000)`：按 NTFS record 平均 ~200B 估算，2M ≈ 400MB 内存上限。
    /// 调用方可显式 `.with_max_records_scanned(usize::MAX)` 关闭保护（仅在受控环境）。
    pub max_records_scanned: Option<usize>,

    /// 软超时。触发后返回已收集的部分结果，**不**报错。
    pub timeout: Duration,

    /// `inspect_many` 的并发度。默认 16。
    pub inspect_concurrency: usize,
}

impl std::fmt::Debug for NtfsScanOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NtfsScanOptions")
            .field("roots", &self.roots)
            .field("matcher", &"<closure>")
            .field("max_results", &self.max_results)
            .field("max_records_scanned", &self.max_records_scanned)
            .field("timeout", &self.timeout)
            .field("inspect_concurrency", &self.inspect_concurrency)
            .finish()
    }
}

impl NtfsScanOptions {
    /// 用一个文件名匹配闭包构造默认选项。
    ///
    /// 默认值：
    /// - `roots = []`
    /// - `max_results = None`
    /// - `max_records_scanned = Some(2_000_000)`
    /// - `timeout = 60s`
    /// - `inspect_concurrency = 16`
    pub fn new(matcher: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        Self {
            roots: Vec::new(),
            matcher: Arc::new(matcher),
            max_results: None,
            max_records_scanned: Some(2_000_000),
            timeout: Duration::from_secs(60),
            inspect_concurrency: 16,
        }
    }

    pub fn with_root(mut self, root: PathBuf) -> Self {
        self.roots.push(root);
        self
    }

    pub fn with_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.roots.extend(roots);
        self
    }

    pub fn with_max_results(mut self, n: usize) -> Self {
        self.max_results = Some(n);
        self
    }

    pub fn with_max_records_scanned(mut self, n: usize) -> Self {
        self.max_records_scanned = Some(n);
        self
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    pub fn with_inspect_concurrency(mut self, n: usize) -> Self {
        self.inspect_concurrency = n;
        self
    }
}

/// 一条扫描结果。基础层字段，全部来自 MFT 原生属性或文件系统 metadata。
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub created: SystemTime,
    pub modified: SystemTime,
    pub accessed: SystemTime,
    pub attributes: FileAttributes,
    pub is_directory: bool,
    pub backend: BackendKind,
    /// NTFS FileReferenceNumber（低 48 bit 为 record 序号、高 16 bit 为 sequence）。
    /// Mft / Usn backend 填充；Walkdir backend 填 `None`。
    pub file_reference: Option<u64>,
}

impl FileEntry {
    /// 从 `std::fs::Metadata` 构造一条 Walkdir backend 的 entry。
    ///
    /// 主要供业务层在不进入扫描流程时，单独 fetch 一条 entry 做扩展查询用
    /// （如 DDNet-Manager 的 `infer_client_identity_with_pe`）。
    pub fn from_metadata(path: PathBuf, meta: &std::fs::Metadata) -> Self {
        let attributes = FileAttributes::from_metadata(meta);
        Self {
            path,
            size: meta.len(),
            created: meta.created().unwrap_or(SystemTime::UNIX_EPOCH),
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            accessed: meta.accessed().unwrap_or(SystemTime::UNIX_EPOCH),
            attributes,
            is_directory: meta.is_dir(),
            backend: BackendKind::Walkdir,
            file_reference: None,
        }
    }
}

bitflags::bitflags! {
    /// NTFS / 通用文件属性 bitflags。对应 Windows `dwFileAttributes`。
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

impl FileAttributes {
    /// 从 `std::fs::Metadata` 提取常见属性位（跨平台兼容，Unix 上只填 DIRECTORY/NORMAL）。
    pub fn from_metadata(meta: &std::fs::Metadata) -> Self {
        let mut attrs = Self::empty();
        if meta.is_dir() {
            attrs |= Self::DIRECTORY;
        } else {
            attrs |= Self::NORMAL;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o444 == 0 {
                // 完全无读权限的文件视作 HIDDEN
                attrs |= Self::HIDDEN;
            }
        }
        attrs
    }
}

/// 扫描使用的底层后端类型。每条 `FileEntry` 都会标注来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Windows 管理员：直接读 NTFS $MFT raw record。
    Mft,
    /// Windows 普通用户：FSCTL_ENUM_USN_DATA 枚举 USN journal。
    Usn,
    /// 跨平台 fallback：walkdir 递归。
    Walkdir,
}

/// 进度事件聚合。调用方按 `kind` 字段做 discriminated union 处理。
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// 某个 root 开始扫描，附 backend 类型。
    DriveStarted { root: PathBuf, backend: BackendKind },

    /// 周期性上报累计匹配数（"已找到 N 个候选"）。
    EntriesFound { found: usize },

    /// 某个 root 扫描完成。`scanned` 是解析过的 record 总数，`found` 是匹配的 entry 数。
    DriveCompleted {
        root: PathBuf,
        scanned: usize,
        found: usize,
    },

    /// 自动降级。`from → to`，附降级原因。
    BackendDowngraded {
        root: PathBuf,
        from: BackendKind,
        to: BackendKind,
        reason: String,
    },

    /// 触发上限保护。
    ScanLimitHit { kind: ScanLimitKind, limit: usize },

    /// 该 root 跳过（所有 backend 都失败）。继续其他 root。
    DriveSkipped { root: PathBuf, reasons: Vec<String> },

    /// 单条 entry 处理失败（如 metadata 读失败、路径重建遇到 stale ref）。
    /// `path = None` 表示无法定位路径（如 NTFS 路径重建时的悬空 parent reference）。
    EntryError {
        path: Option<PathBuf>,
        error: String,
    },
}

/// 软上限类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanLimitKind {
    /// 累计匹配数达到 `max_results`。
    Results,
    /// 累计解析 record 数达到 `max_records_scanned`。
    RecordsScanned,
}

/// inspect 扩展信息位。v0.1 不含 FILE_HASH（移到 v0.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectFields(u8);

bitflags::bitflags! {
    impl InspectFields: u8 {
        /// PE 资源段 RT_VERSION 解析得到 CompanyName / ProductName / 等。
        const VERSION_INFO = 0b0000_0001;
        /// NTFS 所有者 SID（解析 $SDS）。
        const OWNER_SID    = 0b0000_0010;
        /// 备用数据流列表（ADS）。
        const ADS          = 0b0000_0100;
        // FILE_HASH 延后到 v0.2：拖 sha2/hex 依赖，DDNet 业务用不到。
    }
}

/// 扩展查询返回的结构。
#[derive(Debug, Clone, Default)]
pub struct InspectedEntry {
    pub version_info: Option<VersionInfo>,
    pub owner_sid: Option<String>,
    pub alt_data_streams: Vec<String>,
}

/// PE VS_VERSION_INFO 资源解析结果。
#[derive(Debug, Clone, Default)]
pub struct VersionInfo {
    pub company_name: Option<String>,
    pub product_name: Option<String>,
    pub file_description: Option<String>,
    pub file_version: Option<String>,
    pub product_version: Option<String>,
    pub original_filename: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sets_defaults() {
        let opts = NtfsScanOptions::new(|_| true);
        assert_eq!(opts.max_results, None);
        assert_eq!(opts.max_records_scanned, Some(2_000_000));
        assert_eq!(opts.timeout, Duration::from_secs(60));
        assert_eq!(opts.inspect_concurrency, 16);
    }

    #[test]
    fn builder_chains_overrides() {
        let opts = NtfsScanOptions::new(|_| true)
            .with_root(PathBuf::from("C:\\"))
            .with_root(PathBuf::from("D:\\"))
            .with_max_results(10)
            .with_max_records_scanned(100)
            .with_timeout(Duration::from_secs(10))
            .with_inspect_concurrency(4);
        assert_eq!(opts.roots.len(), 2);
        assert_eq!(opts.max_results, Some(10));
        assert_eq!(opts.max_records_scanned, Some(100));
        assert_eq!(opts.timeout, Duration::from_secs(10));
        assert_eq!(opts.inspect_concurrency, 4);
    }

    #[test]
    fn matcher_is_invoked_via_deref() {
        let opts = NtfsScanOptions::new(|name| name.eq_ignore_ascii_case("DDNet.exe"));
        assert!((opts.matcher)("DDNet.exe"));
        assert!((opts.matcher)("ddnet.exe"));
        assert!(!(opts.matcher)("other.exe"));
    }

    #[test]
    fn debug_does_not_panic_on_closure() {
        let opts = NtfsScanOptions::new(|_| true);
        let s = format!("{:?}", opts);
        assert!(s.contains("NtfsScanOptions"));
        assert!(s.contains("<closure>"));
    }

    #[test]
    fn file_attributes_directory_roundtrip() {
        let mut attrs = FileAttributes::empty();
        attrs |= FileAttributes::DIRECTORY;
        assert!(attrs.contains(FileAttributes::DIRECTORY));
        assert!(!attrs.contains(FileAttributes::HIDDEN));
    }

    #[test]
    fn inspect_fields_bitflags_combine() {
        let fields = InspectFields::VERSION_INFO | InspectFields::ADS;
        assert!(fields.contains(InspectFields::VERSION_INFO));
        assert!(fields.contains(InspectFields::ADS));
        assert!(!fields.contains(InspectFields::OWNER_SID));
    }

    #[test]
    fn progress_event_drive_completed_has_scanned_and_found() {
        let ev = ProgressEvent::DriveCompleted {
            root: PathBuf::from("C:\\"),
            scanned: 312_548,
            found: 7,
        };
        let s = format!("{:?}", ev);
        assert!(s.contains("scanned: 312548"));
        assert!(s.contains("found: 7"));
    }
}
