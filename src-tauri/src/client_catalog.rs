/// 表示 catalog 中按平台分组的可执行文件候选。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PlatformExecutableCandidates {
    /// Windows 可执行文件候选。
    pub windows: &'static [&'static str],
    /// macOS bundle 内可执行文件候选。
    pub macos: &'static [&'static str],
    /// Linux 可执行文件候选。
    pub linux: &'static [&'static str],
}

/// 表示客户端更新来源配置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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
    /// PE VS_VERSION_INFO 中 CompanyName 字段的候选值（忽略大小写匹配）。
    ///
    /// 来源：客户端上游 GitHub repo owner / DDNet fork 元数据。**推断值**，
    /// 实际请用 `pelite` / ResourceHacker 检查真实 PE 文件后修正。
    /// 空 `&[]` 表示该客户端 PE 元信息未知，扫描时只能依赖路径匹配。
    pub pe_company_names: &'static [&'static str],
    /// PE VS_VERSION_INFO 中 ProductName 字段的候选值（忽略大小写匹配）。
    pub pe_product_names: &'static [&'static str],
    /// 已知 release sha256 列表：(version, sha256_hex) 元组。
    ///
    /// 用于扫描时识别"用户自己装过的"或"内置 known-good"客户端。
    /// 比路径匹配和 PE 元信息更可靠（不可伪造）。初始留空，后续随 release 补充。
    pub known_hashes: &'static [(&'static str, &'static str)],
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
        client_id: "ddnet",
        display_name: "DDNet",
        aliases: &["ddnet", "ddnet vanilla", "ddnet-vanilla"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        // DDNet 官方上游 build 脚本里硬编码的 VS_VERSION_INFO。
        // 注意：所有衍生客户端（QmClient/TaterClient/BestClient/Cactus）都继承这组值，
        // match_catalog_by_pe 在多客户端共享同一 PE 时返回 None，识别走路径 + sha256。
        pe_company_names: &["DDNet Team", "ddnet-team", "ddnet team"],
        pe_product_names: &["DDNet", "DDNet Client"],
        known_hashes: &[],
        update_source: UpdateSourceDescriptor::DdnetOfficial,
        upstream_url: Some("https://ddnet.org/downloads/"),
    },
    ClientCatalogEntry {
        client_id: "qmclient",
        display_name: "QmClient",
        aliases: &["qmclient", "qm-client"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        // 实测值（E:\个人资料\Game\DDnet\客户端\QmClient\DDNet.exe）：
        // CompanyName="DDNet Team", ProductName="DDNet"。
        // DDNet 上游 CMakeLists.txt 硬编码 VS_VERSION_INFO，所有衍生客户端都继承，
        // PE 元信息无法区分衍生客户端，仅用于识别"DDNet 家族"。
        // 具体客户端识别靠路径 + sha256 指纹。
        pe_company_names: &["DDNet Team"],
        pe_product_names: &["DDNet"],
        known_hashes: &[],
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
        // 实测值（E:\个人资料\Game\DDnet\客户端\TClient\DDNet.exe）：
        // CompanyName="DDNet Team", ProductName="DDNet"。继承 DDNet 上游硬编码。
        // PE 元信息无法区分衍生客户端，仅用于识别"DDNet 家族"。
        pe_company_names: &["DDNet Team"],
        pe_product_names: &["DDNet"],
        known_hashes: &[],
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
        // 实测值（E:\个人资料\Game\DDnet\客户端\bestclient\DDNet.exe）：
        // CompanyName="DDNet Team", ProductName="DDNet"。继承 DDNet 上游硬编码。
        // PE 元信息无法区分衍生客户端，仅用于识别"DDNet 家族"。
        pe_company_names: &["DDNet Team"],
        pe_product_names: &["DDNet"],
        known_hashes: &[],
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
        // 实测值（E:\个人资料\Game\DDnet\客户端\Cactus\DDNet.exe）：
        // CompanyName="DDNet Team", ProductName="DDNet"。继承 DDNet 上游硬编码。
        // PE 元信息无法区分衍生客户端，仅用于识别"DDNet 家族"。
        pe_company_names: &["DDNet Team"],
        pe_product_names: &["DDNet"],
        known_hashes: &[],
        update_source: UpdateSourceDescriptor::Website {
            url: "https://cactusss.vercel.app/",
        },
        upstream_url: Some("https://cactusss.vercel.app/"),
    },
];

/// 返回内置客户端 catalog。
pub fn catalog_entries() -> &'static [ClientCatalogEntry] {
    CATALOG
}

/// 返回第三方兼容客户端 fallback entry。
/// 不在 CATALOG 里（不在 gallery 显示），仅作为 infer_client_identity 兜底用。
pub fn third_party_entry() -> ClientCatalogEntry {
    ClientCatalogEntry {
        client_id: "third_party",
        display_name: "DDNet 兼容客户端",
        aliases: &["third_party"],
        executable_candidates: COMMON_EXECUTABLES,
        required_markers: REQUIRED_MARKERS,
        pe_company_names: &[],
        pe_product_names: &[],
        known_hashes: &[],
        update_source: UpdateSourceDescriptor::None,
        upstream_url: None,
    }
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

/// PE 元信息匹配强度。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeMatchStrength {
    /// CompanyName + ProductName 都匹配（最高置信度）。
    ///
    /// 注意：在当前 catalog 下永远不会返回 Strong —— 因为所有 DDNet 衍生客户端
    /// 共享同一组 PE 值（"DDNet Team" / "DDNet"），任何"双字段匹配"都会让
    /// 多个 entry 同时命中，`match_catalog_by_pe` 会直接返回 None。保留此变体
    /// 是为了未来若出现 PE 唯一可识别的客户端时使用。
    Strong,
    /// 只匹配 CompanyName 或 ProductName 之一，或当前 catalog 下唯一匹配的 entry。
    Weak,
}

/// 根据 PE VS_VERSION_INFO 字段匹配 catalog entry。
///
/// **关键设计**：当 ≥ 2 个 catalog entry 都匹配同一组 PE 值时（即 PE 元信息无法
/// 区分这些客户端，典型场景：所有 DDNet 衍生客户端 PE 都是 "DDNet Team"/"DDNet"），
/// 返回 `None`，让识别走到路径匹配 + sha256 指纹。这是为了避免把 QmClient /
/// TaterClient / BestClient / Cactus 等衍生客户端误识别为 ddnet 原版。
///
/// 只有当 PE 唯一匹配单个 entry 时（其他 entry 都不匹配），才返回 `(entry, Weak)`。
/// `None` 表示 PE 元信息不可用或无法区分，应 fallback 到路径匹配。
pub fn match_catalog_by_pe(
    company_name: Option<&str>,
    product_name: Option<&str>,
) -> Option<(&'static ClientCatalogEntry, PeMatchStrength)> {
    let company_lower = company_name.map(|s| s.to_ascii_lowercase());
    let product_lower = product_name.map(|s| s.to_ascii_lowercase());

    let mut matches: Vec<&'static ClientCatalogEntry> = Vec::new();
    for entry in CATALOG.iter().filter(|e| e.client_id != "third_party") {
        // 跳过没有 PE 元信息规则的客户端（避免空候选误匹配）
        if entry.pe_company_names.is_empty() && entry.pe_product_names.is_empty() {
            continue;
        }
        let company_match = company_lower.as_ref().is_some_and(|c| {
            entry
                .pe_company_names
                .iter()
                .any(|n| n.to_ascii_lowercase() == *c)
        });
        let product_match = product_lower.as_ref().is_some_and(|p| {
            entry
                .pe_product_names
                .iter()
                .any(|n| n.to_ascii_lowercase() == *p)
        });
        if company_match || product_match {
            matches.push(entry);
        }
    }

    // 关键：≥2 个 entry 匹配意味着 PE 元信息无法区分这些客户端（如所有 DDNet 衍生
    // 客户端 PE 都是 "DDNet Team"/"DDNet"），返回 None 让识别走到路径匹配 + sha256。
    if matches.len() == 1 {
        Some((matches[0], PeMatchStrength::Weak))
    } else {
        None
    }
}

/// 根据 exe 的 sha256 匹配 catalog entry（known_hashes 内置指纹库）。
///
/// 返回 (entry, version)。`None` 表示 catalog 里没有这个 hash（应查 registry
/// 的用户下载指纹库）。
pub fn match_catalog_by_hash(
    sha256_hex: &str,
) -> Option<(&'static ClientCatalogEntry, &'static str)> {
    let lower = sha256_hex.to_ascii_lowercase();
    for entry in CATALOG.iter().filter(|e| e.client_id != "third_party") {
        if let Some((version, _hash)) = entry
            .known_hashes
            .iter()
            .find(|(_, hash)| hash.to_ascii_lowercase() == lower)
        {
            return Some((entry, *version));
        }
    }
    None
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

#[cfg(test)]
#[path = "test/client_catalog.rs"]
mod tests;
