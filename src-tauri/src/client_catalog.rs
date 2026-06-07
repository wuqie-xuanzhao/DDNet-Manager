/// 表示 catalog 中按平台分组的可执行文件候选。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformExecutableCandidates {
    /// Windows 可执行文件候选。
    pub windows: &'static [&'static str],
    /// macOS bundle 内可执行文件候选。
    pub macos: &'static [&'static str],
    /// Linux 可执行文件候选。
    pub linux: &'static [&'static str],
}

/// 表示客户端更新来源配置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateSourceDescriptor {
    /// GitHub Release 更新源。
    GithubRelease {
        /// GitHub owner。
        owner: &'static str,
        /// GitHub repo。
        repo: &'static str,
        /// Windows 资产名模式。
        windows_assets: &'static [&'static str],
        /// macOS 资产名模式。
        macos_assets: &'static [&'static str],
        /// Linux 资产名模式。
        linux_assets: &'static [&'static str],
    },
    /// DDNet 官方下载页更新源。
    DdnetOfficial,
    /// 仅提供官网入口的网站型来源。
    Website {
        /// 官方网站 URL。
        url: &'static str,
    },
    /// 不支持自动更新。
    None,
}

/// 表示 DDNet Manager 内置客户端目录中的一个客户端定义。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientCatalogEntry {
    /// 客户端类型标识。
    pub client_id: &'static str,
    /// 展示名称。
    pub display_name: &'static str,
    /// 用于路径和目录名匹配的别名。
    pub aliases: &'static [&'static str],
    /// 各平台可执行文件候选。
    pub executable_candidates: PlatformExecutableCandidates,
    /// 必须存在的客户端结构标记。
    pub required_markers: &'static [&'static str],
    /// 更新来源。
    pub update_source: UpdateSourceDescriptor,
    /// 上游主页、Release 页或管理入口。
    pub upstream_url: Option<&'static str>,
}

const DDNET_EXECUTABLES_WINDOWS: &[&str] = &["DDNet.exe", "ddnet.exe"];
const DDNET_EXECUTABLES_MACOS: &[&str] = &["DDNet", "ddnet"];
const DDNET_EXECUTABLES_LINUX: &[&str] = &["DDNet", "ddnet"];

const COMMON_EXECUTABLES: PlatformExecutableCandidates = PlatformExecutableCandidates {
    windows: DDNET_EXECUTABLES_WINDOWS,
    macos: DDNET_EXECUTABLES_MACOS,
    linux: DDNET_EXECUTABLES_LINUX,
};

const REQUIRED_MARKERS: &[&str] = &["storage.cfg", "data"];

const CATALOG: &[ClientCatalogEntry] = &[
    ClientCatalogEntry {
        client_id: "qmclient",
        display_name: "QmClient",
        aliases: &["qmclient", "qm-client"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        update_source: UpdateSourceDescriptor::GithubRelease {
            owner: "wxj881027",
            repo: "QmClient",
            windows_assets: &["QmClient-windows.zip", "QmClient-windows.7z"],
            macos_assets: &["QmClient-macOS.dmg"],
            linux_assets: &["QmClient-ubuntu.tar.xz"],
        },
        upstream_url: Some("https://github.com/wxj881027/QmClient/releases"),
    },
    ClientCatalogEntry {
        client_id: "taterclient",
        display_name: "TaterClient",
        aliases: &["taterclient", "tclient", "t-client"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        update_source: UpdateSourceDescriptor::GithubRelease {
            owner: "TaterClient",
            repo: "TClient",
            windows_assets: &["TClient-windows.zip"],
            macos_assets: &["TClient-macOS.dmg"],
            linux_assets: &["TClient-ubuntu.tar.xz"],
        },
        upstream_url: Some("https://github.com/TaterClient/TClient/releases"),
    },
    ClientCatalogEntry {
        client_id: "bestclient",
        display_name: "BestClient",
        aliases: &["bestclient", "best-client"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        update_source: UpdateSourceDescriptor::GithubRelease {
            owner: "BestProjectTeam",
            repo: "BestClient",
            windows_assets: &["BestClient-windows.zip"],
            macos_assets: &[],
            linux_assets: &["BestClient-linux.tar.xz"],
        },
        upstream_url: Some("https://github.com/BestProjectTeam/BestClient/releases"),
    },
    ClientCatalogEntry {
        client_id: "cactusclient",
        display_name: "Cactus Client",
        aliases: &["cactusclient", "cactus-client", "cactus"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        update_source: UpdateSourceDescriptor::Website {
            url: "https://cactuss.top/",
        },
        upstream_url: Some("https://cactuss.top/"),
    },
    ClientCatalogEntry {
        client_id: "ddnet",
        display_name: "DDNet",
        aliases: &["ddnet", "ddnet vanilla", "ddnet-vanilla"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        update_source: UpdateSourceDescriptor::DdnetOfficial,
        upstream_url: Some("https://ddnet.org/downloads/"),
    },
    ClientCatalogEntry {
        client_id: "third_party",
        display_name: "DDNet 兼容客户端",
        aliases: &["third_party"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        update_source: UpdateSourceDescriptor::None,
        upstream_url: None,
    },
];

/// 返回内置客户端 catalog。
pub fn catalog_entries() -> &'static [ClientCatalogEntry] {
    CATALOG
}

/// 返回第三方兼容客户端 fallback entry。
pub fn third_party_entry() -> &'static ClientCatalogEntry {
    &CATALOG[5]
}

/// 按客户端 ID 查找 catalog entry。
pub fn catalog_entry_by_id(client_id: &str) -> Option<&'static ClientCatalogEntry> {
    let normalized = normalize_client_id(client_id);
    CATALOG.iter().find(|entry| entry.client_id == normalized)
}

/// 根据路径文本匹配最可能的 catalog entry。
pub fn match_catalog_entry(path_text: &str) -> Option<&'static ClientCatalogEntry> {
    let haystack = path_text.to_ascii_lowercase();
    CATALOG
        .iter()
        .filter(|entry| entry.client_id != "third_party")
        .find(|entry| {
            entry
                .aliases
                .iter()
                .any(|alias| haystack.contains(&alias.to_ascii_lowercase()))
        })
}

/// 将历史客户端 ID 归一化为 MVP 使用的 ID。
pub fn normalize_client_id(client_id: &str) -> &str {
    if client_id == "ddnet_vanilla" {
        "ddnet"
    } else {
        client_id
    }
}

/// 返回 Steam DDNet 管理入口 URL。
pub fn ddnet_steam_url() -> &'static str {
    "https://store.steampowered.com/app/412220/DDNet/"
}
