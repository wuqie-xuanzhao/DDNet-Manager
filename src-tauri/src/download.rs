//! 下载任务管理器、安装包类型识别与下载入口。
//!
//! 模块组织：
//! - 当前文件：[`DownloadManager`] 状态容器、[`PackageKind`] 枚举、下载任务构造
//!   与自动安装前置守卫。
//! - [`verify`]：缓存文件 SHA-256 校验、恢复摘要构建。
//! - [`extract`]：zip / tar.xz / dmg 安全解压到 staging。
//! - [`net`]：下载 URL 校验、HTTP 重定向跟随、流式写入与缓存清理。
//! - [`install`]：客户端目录替换、回滚与残留清理。

use crate::models::{ClientUpdateCheck, DownloadJob, DownloadJobStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;

/// 压缩包安全解压到 staging 目录。
pub mod extract;
/// 客户端安装、回滚和残留目录清理。
pub mod install;
/// 下载 URL 校验、HTTP 请求与流式写入。
pub mod net;
/// 多源竞速测速模块（HEAD 淘汰 + Range 测吞吐）。
pub mod race;
/// 缓存文件校验与恢复摘要构建。
pub mod verify;

// 重导出公共入口，让旧调用路径 `crate::download::verify_downloaded_file` 等保持有效。
// 标记为 doc(hidden)，因为这些项的实现与文档已在子模块中提供，rustdoc 不必重复列出。
/// 解压相关入口（zip / tar.xz / dmg）。
#[doc(hidden)]
pub use extract::{extract_package_to_staging, extract_zip_to_staging, find_staged_client_dir};
/// 网络下载入口（流式写入、缓存清理）。
#[doc(hidden)]
pub use net::{cleanup_expired_cache_files, download_asset_to_file};
/// 缓存校验与恢复入口。
#[doc(hidden)]
pub use verify::{
    build_download_job_recovery, sha256_hex, verify_downloaded_file, DownloadJobRecoveryDecision,
};

/// 测试辅助：暴露 net 内部 URL 校验，让旧测试路径继续可用。
#[cfg(test)]
pub(crate) use net::validate_download_url;

/// 表示更新资产的安装包类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    /// zip 压缩包，当前支持自动解压安装。
    Zip,
    /// Linux tar.xz 压缩包，支持进入 Manager-owned 解包安装闭环。
    TarXz,
    /// macOS dmg 镜像包，仅在 macOS hdiutil 可用时支持自动安装。
    Dmg,
    /// 未识别后缀，保守禁止自动安装。
    Unknown,
}

impl PackageKind {
    /// 返回安装包类型用于持久化和诊断的稳定名称。
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Zip => "zip",
            PackageKind::TarXz => "tar.xz",
            PackageKind::Dmg => "dmg",
            PackageKind::Unknown => "unknown",
        }
    }

    fn cache_suffix(self) -> &'static str {
        match self {
            PackageKind::Zip => ".zip",
            PackageKind::TarXz => ".tar.xz",
            PackageKind::Dmg => ".dmg",
            PackageKind::Unknown => ".download",
        }
    }
}

/// 表示一次下载文件写入请求。
pub struct DownloadFileRequest<'a> {
    /// 资产下载地址（权威原始 GitHub URL，由更新发现层产出）。
    pub asset_url: &'a str,
    /// 缓存文件路径。
    pub cache_path: &'a std::path::Path,
    /// manifest 中声明的期望大小。
    pub expected_size: u64,
    /// 用户配置的网络路由（本地代理）；为空表示直连。
    pub route: Option<&'a crate::models::NetworkRouteConfig>,
    /// 竞速候选 URL 列表（含原始 URL + 反代 URL）。空或仅含原始 URL 时退化为单源直连。
    pub candidate_urls: &'a [String],
    /// 用户显式信任的额外下载 host（如公共反代域名），补充基线白名单。
    pub extra_hosts: &'a [String],
}

/// 管理当前进程内的下载任务状态。
///
/// `jobs` 维护活跃任务快照，`cancellers` 为每个任务附带一个
/// [`CancellationToken`]，让 [`net::download_asset_to_file`] 能在 chunk
/// 之间即时感知取消，而不必等到下一个 chunk 边界。
#[derive(Clone, Default)]
pub struct DownloadManager {
    jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
    cancellers: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

/// 同时活跃下载任务的上限，防止带宽或连接资源耗尽。
const MAX_CONCURRENT_DOWNLOADS: usize = 3;

impl DownloadManager {
    /// 插入新的下载任务，若活跃下载数已达上限则拒绝。
    pub fn insert(&self, job: DownloadJob) -> Result<(), String> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?;
        let active_count = jobs
            .values()
            .filter(|job| {
                matches!(
                    job.status,
                    DownloadJobStatus::Downloading | DownloadJobStatus::Pending
                )
            })
            .count();
        if active_count >= MAX_CONCURRENT_DOWNLOADS {
            return Err(format!(
                "concurrent download limit reached ({MAX_CONCURRENT_DOWNLOADS}); wait for an active download to finish"
            ));
        }
        self.cancellers
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?
            .insert(job.id.clone(), CancellationToken::new());
        jobs.insert(job.id.clone(), job);
        Ok(())
    }

    /// 读取指定下载任务。
    pub fn get(&self, job_id: &str) -> Result<Option<DownloadJob>, String> {
        Ok(self
            .jobs
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?
            .get(job_id)
            .cloned())
    }

    /// 更新指定下载任务。
    pub fn update<F>(&self, job_id: &str, update: F) -> Result<DownloadJob, String>
    where
        F: FnOnce(&mut DownloadJob),
    {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?;
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| format!("download job not found: {job_id}"))?;
        update(job);
        Ok(job.clone())
    }

    /// 标记下载任务已取消，并触发对应的 [`CancellationToken`]。
    ///
    /// 取消信号通过 `select!` 立即唤醒下载循环，无需等待下一个 chunk 边界。
    pub fn cancel(&self, job_id: &str) -> Result<DownloadJob, String> {
        if let Ok(cancellers) = self.cancellers.lock() {
            if let Some(token) = cancellers.get(job_id) {
                token.cancel();
            }
        }
        self.update(job_id, |job| {
            job.status = DownloadJobStatus::Canceled;
        })
    }

    /// 克隆指定下载任务的取消令牌，供后台下载任务在 `select!` 中监听。
    pub fn cancel_token_clone(&self, job_id: &str) -> Option<CancellationToken> {
        self.cancellers
            .lock()
            .ok()
            .and_then(|cancellers| cancellers.get(job_id).cloned())
    }

    /// 从注册表恢复上次退出时未完成的下载任务。
    ///
    /// 将 `Downloading` 和 `Installing` 状态转为 `Failed`（进程崩溃后这些状态不可信），
    /// 将恢复后的任务全部载入内存管理器，使前端能感知并展示可重试/可安装的恢复项。
    ///
    /// 恢复任务绕过并发下载数上限，因为恢复的任务不会立即开始下载。
    pub fn recover_from_registry(
        &self,
        registry: &crate::registry::ClientRegistry,
    ) -> Result<Vec<DownloadJob>, crate::error::ManagerError> {
        let persisted = registry.list_download_jobs(None)?;
        let mut recovered = Vec::new();
        for mut job in persisted {
            let needs_recovery = matches!(
                job.status,
                DownloadJobStatus::Downloading
                    | DownloadJobStatus::Installing
                    | DownloadJobStatus::Pending
            );
            if needs_recovery {
                job.status = DownloadJobStatus::Failed;
                job.error =
                    Some("application exited before download or install completed".to_string());
                registry.upsert_download_job(&job)?;
            }
            self.insert_bypass_limit(job.clone())
                .map_err(crate::error::ManagerError::Internal)?;
            recovered.push(job);
        }
        Ok(recovered)
    }

    /// 插入任务到内存状态，绕过并发下载数上限。
    ///
    /// 仅用于从注册表恢复已持久化的任务或测试场景；新发起的下载必须通过
    /// [`insert`](Self::insert) 方法接受并发限制检查。同时为恢复的任务附带一个
    /// 新的 [`CancellationToken`]，避免后续 cancel 时找不到令牌。
    pub fn insert_bypass_limit(&self, job: DownloadJob) -> Result<(), String> {
        self.cancellers
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?
            .entry(job.id.clone())
            .or_insert_with(CancellationToken::new);
        self.jobs
            .lock()
            .map_err(|_| "download job state is poisoned".to_string())?
            .insert(job.id.clone(), job);
        Ok(())
    }
}

/// 基于更新请求和下载目录创建下载任务模型。
pub fn create_download_job(
    client_installation_id: &str,
    update: &ClientUpdateCheck,
    downloads_dir: &std::path::Path,
) -> DownloadJob {
    let now = OffsetDateTime::now_utc().unix_timestamp_nanos();
    let id = format!("download-{now}");
    let cache_path = downloads_dir.join(format!(
        "{id}{}",
        package_kind_for_asset_url(&update.asset.asset_url).cache_suffix()
    ));
    DownloadJob {
        id,
        client_installation_id: client_installation_id.to_string(),
        client_id: update.client_id.clone(),
        channel: update.channel.clone(),
        version: update.latest_version.clone(),
        asset_url: update.asset.asset_url.clone(),
        sha256: update.asset.sha256.clone(),
        size: update.asset.size,
        status: DownloadJobStatus::Pending,
        downloaded_bytes: 0,
        cache_path: net::normalize_path(&cache_path),
        error: None,
    }
}

/// 根据下载资产 URL 推断安装包类型。
pub fn package_kind_for_asset_url(asset_url: &str) -> PackageKind {
    let asset_url = asset_url.to_ascii_lowercase();
    if asset_url.ends_with(".tar.xz") {
        PackageKind::TarXz
    } else if asset_url.ends_with(".dmg") {
        PackageKind::Dmg
    } else if asset_url.ends_with(".zip") {
        PackageKind::Zip
    } else {
        PackageKind::Unknown
    }
}

/// 校验当前自动安装链路是否支持该安装包类型。
pub fn auto_install_guard(package_kind: PackageKind) -> Result<(), String> {
    match package_kind {
        PackageKind::Zip | PackageKind::TarXz | PackageKind::Dmg => Ok(()),
        PackageKind::Unknown => Err(
            "automatic install only supports .zip, .tar.xz, and .dmg packages; unknown package type requires manual install"
                .to_string(),
        ),
    }
}

#[cfg(test)]
#[path = "test/download.rs"]
mod tests;
