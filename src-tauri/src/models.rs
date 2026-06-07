use serde::{Deserialize, Serialize};

/// 表示本地客户端安装状态的健康检查结果。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientHealth {
    /// 客户端安装可用。
    Ok,
    /// 客户端可执行文件缺失。
    MissingExecutable,
    /// 客户端 storage.cfg 缺失。
    MissingStorageCfg,
    /// 客户端数据目录缺失。
    MissingDataDir,
}

/// 表示客户端安装记录的来源。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientInstallSource {
    /// 来自客户端官方网站下载。
    OfficialDownload,
    /// 来自 Steam 安装目录。
    Steam,
    /// 用户手动添加的外部安装。
    #[default]
    Manual,
    /// DDNet Manager 管理目录中的安装。
    Manager,
}

/// 表示客户端识别结果的可信度。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientConfidence {
    /// 结构、名称和 catalog 规则均匹配。
    Verified,
    /// 结构符合 DDNet 兼容客户端，但发行方未知。
    Compatible,
    /// 只命中部分文件或标记。
    #[default]
    Partial,
    /// 已知客户端在当前环境不支持。
    Unsupported,
}

/// 表示客户端在当前机器上的启动兼容性状态。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    /// 当前机器满足已知基本运行条件。
    Supported,
    /// 当前机器不满足已知最低运行条件。
    Unsupported,
    /// 可能可启动，但存在依赖、权限或系统版本风险。
    Risky,
    /// 缺少足够证据判断兼容性。
    #[default]
    Unknown,
    /// Manager 曾经成功启动并观察到进程。
    Verified,
}

/// 表示兼容性判断的一条原因。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompatibilityReason {
    /// 稳定的原因代码，供前端映射文案。
    pub code: String,
    /// 可直接展示给用户的中文原因。
    pub message: String,
}

/// 表示客户端在当前机器上的启动兼容性诊断。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClientCompatibility {
    /// 综合兼容性状态。
    pub status: CompatibilityStatus,
    /// 当前状态是否允许默认启动。
    pub can_launch: bool,
    /// 是否已通过 Manager 受控启动验证。
    pub launch_verified: bool,
    /// 影响兼容性判断的原因列表。
    #[serde(default)]
    pub reasons: Vec<CompatibilityReason>,
    /// 最近一次启动结果摘要。
    pub last_launch_result: Option<String>,
}

/// 表示更新来源的类型。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSourceKind {
    /// GitHub Release API。
    GithubRelease,
    /// 客户端官方网站。
    Website,
    /// DDNet 官方下载页与 sha256sums.txt。
    DdnetOfficial,
    /// 项目自维护 manifest。
    Manifest,
    /// 当前客户端没有自动更新来源。
    #[default]
    None,
}

/// 表示更新检查后建议前端执行的动作。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    /// 可以进入自动下载和安装流程。
    Download,
    /// 打开官方网站或管理页面，由用户手动处理。
    OpenUrl,
    /// 当前没有可执行动作。
    #[default]
    None,
}

/// 表示 DDNet 兼容客户端在本机的一条安装记录。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClientInstallation {
    /// 安装记录的唯一标识。
    pub id: String,
    /// 客户端类型标识，例如 qmclient。
    pub client_id: String,
    /// 展示给用户的客户端名称。
    pub display_name: String,
    /// 客户端安装目录。
    pub install_dir: String,
    /// 客户端可执行文件路径。
    pub executable_path: String,
    /// 客户端 storage.cfg 路径。
    pub storage_cfg_path: String,
    /// 客户端资源数据目录。
    pub data_dir: String,
    /// 客户端用户数据目录，未发现时为空。
    pub user_data_dir: Option<String>,
    /// 客户端版本号，未识别时为空。
    pub version: Option<String>,
    /// 是否为默认启动客户端。
    pub is_default: bool,
    /// 当前安装记录的健康状态。
    pub health: ClientHealth,
    /// 当前安装缺失的具体项目列表。
    #[serde(default)]
    pub missing_items: Vec<String>,
    /// 当前安装的来源。
    #[serde(default)]
    pub install_source: ClientInstallSource,
    /// 当前记录的识别可信度。
    #[serde(default)]
    pub confidence: ClientConfidence,
    /// 是否由 DDNet Manager 管理安装目录。
    #[serde(default)]
    pub manager_owned: bool,
    /// 当前机器对该客户端的启动兼容性诊断。
    #[serde(default)]
    pub compatibility: ClientCompatibility,
    /// 官方下载、GitHub release 或 Steam 管理入口。
    #[serde(default)]
    pub upstream_url: Option<String>,
    /// 最后一次扫描或验证该安装记录的 UTC 时间。
    pub last_scanned_at: Option<String>,
}

/// 表示扫描客户端安装目录时使用的选项。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScanClientInstallationsOptions {
    /// 用户显式要求扫描的根目录。
    #[serde(default)]
    pub roots: Vec<String>,
    /// 是否把注册表中已保存的历史路径也纳入扫描。
    #[serde(default)]
    pub include_saved_paths: bool,
    /// 是否启用更深层级扫描。默认扫描保持轻量，不做全盘扫描。
    #[serde(default)]
    pub deep: bool,
}

/// 表示 manifest 或下载链路的网络路由模式。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkRouteMode {
    /// 不使用代理或镜像，直接访问原始地址。
    #[default]
    Direct,
    /// 使用代理前缀拼接原始 URL。
    ProxyPrefix,
    /// 使用包含 `{url}` 占位符的镜像模板构造访问 URL。
    MirrorTemplate,
}

/// 表示用户显式启用的网络代理或镜像路由配置。
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct NetworkRouteConfig {
    /// 网络路由模式。
    pub mode: NetworkRouteMode,
    /// 代理前缀 URL，仅 `proxy_prefix` 模式使用。
    pub proxy_prefix_url: Option<String>,
    /// 镜像模板 URL，仅 `mirror_template` 模式使用。
    pub mirror_template: Option<String>,
    /// 显式启用的代理或镜像 host 列表。
    #[serde(default)]
    pub enabled_hosts: Vec<String>,
}

impl NetworkRouteConfig {
    /// 创建直连网络路由配置。
    pub fn direct() -> Self {
        Self {
            mode: NetworkRouteMode::Direct,
            proxy_prefix_url: None,
            mirror_template: None,
            enabled_hosts: Vec::new(),
        }
    }
}

/// 表示保存客户端安装记录的请求。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UpsertClientInstallationRequest {
    /// 客户端安装目录。
    pub install_dir: String,
    /// 是否保存为默认客户端。
    #[serde(default)]
    pub is_default: bool,
}

/// 表示后端为某个客户端选出的更新资产。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClientUpdateCheck {
    /// 客户端类型标识。
    pub client_id: String,
    /// 发布渠道标识。
    pub channel: String,
    /// 本地版本号，未知时为空。
    pub current_version: Option<String>,
    /// manifest 中匹配到的最新版本号。
    pub latest_version: String,
    /// 当前平台匹配到的更新资产。
    pub asset: UpdateAsset,
    /// 是否需要更新。版本未知时按需要更新处理。
    pub needs_update: bool,
    /// 本次更新检查使用的来源类型。
    #[serde(default)]
    pub source_kind: UpdateSourceKind,
    /// 建议前端执行的下一步动作。
    #[serde(default)]
    pub action: UpdateAction,
    /// 手动下载或管理入口 URL。
    #[serde(default)]
    pub action_url: Option<String>,
    /// 更新检查诊断或不可自动安装原因。
    #[serde(default)]
    pub message: Option<String>,
}

/// 表示检查客户端更新的请求。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CheckClientUpdateRequest {
    /// 客户端类型标识。
    pub client_id: String,
    /// 发布渠道标识。
    pub channel: String,
    /// 项目自维护 manifest 地址；为空时后端会拒绝请求。
    pub manifest_url: Option<String>,
    /// 平台标识。为空时由后端根据当前平台推断。
    pub platform: Option<String>,
    /// 可选网络路由策略，用于代理或镜像 manifest 访问。
    pub network_route: Option<NetworkRouteConfig>,
    /// 是否强制使用高级 manifest 来源。
    #[serde(default)]
    pub use_manifest_source: bool,
}

/// 表示从 manifest 中选择更新资产的条件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientUpdateSelector {
    /// 客户端类型标识。
    pub client_id: String,
    /// 发布渠道标识。
    pub channel: String,
    /// 平台标识。
    pub platform: String,
}

/// 表示下载任务当前状态。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadJobStatus {
    /// 下载任务已创建但尚未开始。
    Pending,
    /// 正在下载远程资产。
    Downloading,
    /// 下载文件已通过 size 和 sha256 校验。
    Verified,
    /// 正在安装已下载资产。
    Installing,
    /// 安装已完成。
    Completed,
    /// 用户取消了下载任务。
    Canceled,
    /// 下载或安装失败。
    Failed,
}

/// 表示一个更新下载任务。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct DownloadJob {
    /// 下载任务 ID。
    pub id: String,
    /// 目标客户端安装记录 ID。
    pub client_installation_id: String,
    /// 客户端类型标识。
    pub client_id: String,
    /// 发布渠道标识。
    pub channel: String,
    /// 目标版本。
    pub version: String,
    /// 资产下载地址。
    pub asset_url: String,
    /// 期望 sha256。
    pub sha256: String,
    /// 期望文件大小。
    pub size: u64,
    /// 当前任务状态。
    pub status: DownloadJobStatus,
    /// 已下载字节数。
    pub downloaded_bytes: u64,
    /// 缓存文件路径。
    pub cache_path: String,
    /// 最近一次错误信息。
    pub error: Option<String>,
}

/// 表示创建下载任务的请求。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct StartUpdateDownloadRequest {
    /// 目标客户端安装记录 ID。
    pub client_installation_id: String,
    /// 发布渠道标识。
    pub channel: String,
    /// 项目自维护 manifest 地址；为空时后端会拒绝请求。
    pub manifest_url: Option<String>,
    /// 平台标识。为空时由后端根据当前平台推断。
    pub platform: Option<String>,
    /// 可选网络路由策略，用于代理或镜像 manifest 访问。
    pub network_route: Option<NetworkRouteConfig>,
    /// 是否强制使用高级 manifest 来源。
    #[serde(default)]
    pub use_manifest_source: bool,
}

/// 表示 manifest 中一个可下载更新资产。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UpdateAsset {
    /// 资产适用的平台标识。
    pub platform: String,
    /// 资产下载地址。
    pub asset_url: String,
    /// 资产 sha256 校验值。
    pub sha256: String,
    /// 资产字节大小。
    pub size: u64,
}

/// 表示 manifest 中某个客户端渠道的发布信息。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ManifestClient {
    /// 客户端类型标识。
    pub client_id: String,
    /// 发布渠道标识。
    pub channel: String,
    /// 发布版本号。
    pub version: String,
    /// 发布说明文本。
    pub release_notes: String,
    /// 当前发布包含的下载资产列表。
    pub assets: Vec<UpdateAsset>,
}

/// 表示更新服务返回的客户端更新清单。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct UpdateManifest {
    /// manifest 结构版本号。
    pub schema_version: u32,
    /// manifest 中包含的客户端发布列表。
    pub clients: Vec<ManifestClient>,
}

/// 表示从 DDNet / QmClient 配置中解析出的一条 bind 记录。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct BindRecord {
    /// 绑定按键。
    pub key: String,
    /// 绑定命令内容。
    pub command: String,
    /// 记录来源配置文件路径。
    pub source_file: String,
    /// 记录所在源码行号。
    pub line: usize,
    /// 是否由 DDNet Manager 管理。
    pub managed_by_manager: bool,
    /// 匹配到的 Workshop 条目 ID。
    pub matched_workshop_id: Option<String>,
}

/// 表示 cfg 中一条 exec 记录及其解析结果。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CfgExecRecord {
    /// exec 指令声明的目标文件。
    pub target: String,
    /// 记录来源配置文件路径。
    pub source_file: String,
    /// 记录所在源码行号。
    pub line: usize,
    /// 解析得到的目标文件路径，无法解析时为空。
    pub resolved_path: Option<String>,
    /// 目标文件是否缺失。
    pub missing: bool,
}

/// 表示 cfg 中一条 unbind 记录。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CfgUnbindRecord {
    /// 解绑按键。
    pub key: String,
    /// 记录来源配置文件路径。
    pub source_file: String,
    /// 记录所在源码行号。
    pub line: usize,
}

/// 表示同一个按键存在多条 bind 记录时的冲突集合。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct BindConflict {
    /// 冲突按键。
    pub key: String,
    /// 参与冲突的 bind 记录列表。
    pub records: Vec<BindRecord>,
}

/// 表示 cfg 深度分析结果。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CfgAnalysis {
    /// 解析到的 bind 记录。
    pub binds: Vec<BindRecord>,
    /// 解析到的 unbind 记录。
    pub unbinds: Vec<CfgUnbindRecord>,
    /// 解析到的 exec 记录。
    pub execs: Vec<CfgExecRecord>,
    /// 按键冲突记录。
    pub conflicts: Vec<BindConflict>,
    /// 缺失的 exec 目标记录。
    pub missing_exec_targets: Vec<CfgExecRecord>,
}

/// 表示从 Workshop 公开 JSON 映射出的一条 bind 条目。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct WorkshopBind {
    /// Workshop 条目的唯一标识。
    pub id: String,
    /// Workshop 条目的分类名称。
    pub category: String,
    /// Workshop 条目的标题。
    pub title: String,
    /// Workshop 条目的默认命令文本。
    pub command: String,
    /// Workshop 条目的说明文本。
    pub description: String,
    /// Workshop 条目的命令变体列表。
    #[serde(default, alias = "commandVariants")]
    pub command_variants: Vec<String>,
    /// Workshop 条目的变体标签列表。
    #[serde(default, alias = "variantLabels")]
    pub variant_labels: Vec<String>,
    /// Workshop 条目是否允许直接绑定。
    #[serde(default, alias = "isBindable")]
    pub is_bindable: bool,
}

#[cfg(test)]
#[path = "test/models.rs"]
mod tests;
