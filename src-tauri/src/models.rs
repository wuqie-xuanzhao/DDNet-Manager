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
mod tests {
    use super::{
        BindConflict, CfgAnalysis, CfgExecRecord, CfgUnbindRecord, ClientHealth,
        ClientInstallation, WorkshopBind,
    };

    #[test]
    fn serializes_client_installation_with_snake_case_fields_and_ok_health() {
        let installation = ClientInstallation {
            id: "qmclient-main".to_string(),
            client_id: "qmclient".to_string(),
            display_name: "QmClient".to_string(),
            install_dir: "D:/Games/QmClient".to_string(),
            executable_path: "D:/Games/QmClient/DDNet.exe".to_string(),
            storage_cfg_path: "D:/Games/QmClient/storage.cfg".to_string(),
            data_dir: "D:/Games/QmClient/data".to_string(),
            user_data_dir: Some("C:/Users/Player/AppData/Roaming/DDNet".to_string()),
            version: Some("18.9.1".to_string()),
            is_default: true,
            health: ClientHealth::Ok,
            last_scanned_at: Some("2026-06-06T12:00:00Z".to_string()),
        };

        let serialized = serde_json::to_value(installation).expect("测试序列化应成功");

        assert_eq!(serialized["client_id"], "qmclient");
        assert_eq!(serialized["display_name"], "QmClient");
        assert_eq!(serialized["install_dir"], "D:/Games/QmClient");
        assert_eq!(serialized["executable_path"], "D:/Games/QmClient/DDNet.exe");
        assert_eq!(
            serialized["storage_cfg_path"],
            "D:/Games/QmClient/storage.cfg"
        );
        assert_eq!(serialized["data_dir"], "D:/Games/QmClient/data");
        assert_eq!(
            serialized["user_data_dir"],
            "C:/Users/Player/AppData/Roaming/DDNet"
        );
        assert_eq!(serialized["is_default"], true);
        assert_eq!(serialized["health"], "ok");
    }

    #[test]
    fn serializes_workshop_bind_with_snake_case_fields() {
        let bind = WorkshopBind {
            id: "bind-1".to_string(),
            category: "基础".to_string(),
            title: "防自杀".to_string(),
            command: "bind mouse3 \"kill\"".to_string(),
            description: "测试".to_string(),
            command_variants: vec!["bind mouse3 \"echo Kill; kill\"".to_string()],
            variant_labels: vec!["带提示".to_string()],
            is_bindable: true,
        };

        let serialized = serde_json::to_value(bind).expect("测试序列化应成功");

        assert_eq!(
            serialized["command_variants"][0],
            "bind mouse3 \"echo Kill; kill\""
        );
        assert_eq!(serialized["variant_labels"][0], "带提示");
        assert_eq!(serialized["is_bindable"], true);
        assert!(serialized.get("commandVariants").is_none());
        assert!(serialized.get("variantLabels").is_none());
        assert!(serialized.get("isBindable").is_none());
    }

    #[test]
    fn serializes_cfg_analysis_related_types_with_snake_case_fields() {
        let analysis = CfgAnalysis {
            binds: vec![],
            unbinds: vec![CfgUnbindRecord {
                key: "mouse3".to_string(),
                source_file: "settings_ddnet.cfg".to_string(),
                line: 4,
            }],
            execs: vec![CfgExecRecord {
                target: "extra.cfg".to_string(),
                source_file: "settings_ddnet.cfg".to_string(),
                line: 3,
                resolved_path: Some("C:/DDNet/extra.cfg".to_string()),
                missing: false,
            }],
            conflicts: vec![BindConflict {
                key: "f1".to_string(),
                records: vec![],
            }],
            missing_exec_targets: vec![CfgExecRecord {
                target: "missing.cfg".to_string(),
                source_file: "settings_ddnet.cfg".to_string(),
                line: 8,
                resolved_path: Some("C:/DDNet/missing.cfg".to_string()),
                missing: true,
            }],
        };

        let serialized = serde_json::to_value(analysis).expect("测试序列化应成功");

        assert_eq!(
            serialized["unbinds"][0]["source_file"],
            "settings_ddnet.cfg"
        );
        assert_eq!(
            serialized["execs"][0]["resolved_path"],
            "C:/DDNet/extra.cfg"
        );
        assert_eq!(serialized["execs"][0]["missing"], false);
        assert_eq!(serialized["conflicts"][0]["key"], "f1");
        assert_eq!(
            serialized["missing_exec_targets"][0]["target"],
            "missing.cfg"
        );
    }
}
