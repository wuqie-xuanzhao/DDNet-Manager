//! 客户端注册表持久化能力（SQLite）。
//!
//! 模块组织：
//! - 当前文件：[`ClientRegistry`] 状态容器、[`LaunchProbeStatus`] /
//!   [`LaunchProbeRecord`] 数据类型、`open` / `lock_conn` / `init_schema` 基础设施。
//! - [`clients`]：客户端安装记录与启动探测读写。
//! - [`settings`]：应用设置读写。
//! - [`jobs`]：下载任务快照读写。
//! - [`history`]：Manager-owned 安装历史读写。
//! - [`fingerprints`]：用户下载时记录的客户端 sha256 指纹（用于扫描时升级识别）。

use crate::error::ManagerError;
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// 客户端安装记录相关表的读写。
mod clients;
/// 客户端指纹（sha256 → client_id/version）读写。
pub mod fingerprints;
/// Manager-owned 安装历史记录的持久化。
mod history;
/// 下载任务快照的持久化。
mod jobs;
/// 应用设置的持久化。
mod settings;

/// 表示一次启动探测写回请求。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchProbeStatus {
    /// 已观察到目标客户端进程。
    Verified,
    /// 启动命令已发出，但限定时间内未观察到进程，不能据此判定失败。
    Unobserved,
    /// 启动后进程提前退出。
    Exited,
}

/// 表示一次启动探测写回请求。
pub struct LaunchProbeRecord<'a> {
    /// 客户端安装记录 ID。
    pub client_installation_id: &'a str,
    /// 受控启动探测状态。
    pub status: LaunchProbeStatus,
    /// 启动探测结果摘要。
    pub message: &'a str,
}

/// 管理 DDNet 兼容客户端安装记录的 SQLite 注册表。
///
/// 内部使用 `Mutex<Connection>` 包装 SQLite 连接，使 `ClientRegistry` 满足
/// `Send + Sync`，可作为 Tauri managed 状态跨 IPC command 复用同一连接。
pub struct ClientRegistry {
    conn: Arc<Mutex<Connection>>,
}

impl Clone for ClientRegistry {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}

impl ClientRegistry {
    /// 打开或创建客户端注册表，并初始化最小 schema。
    pub fn open(path: &Path) -> Result<Self, ManagerError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                ManagerError::Internal(format!("failed to create registry dir: {error}"))
            })?;
        }

        let conn = Connection::open(path)
            .map_err(|error| ManagerError::Internal(format!("failed to open registry: {error}")))?;
        let registry = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        registry.init_schema()?;
        Ok(registry)
    }

    /// 获取互斥锁保护的 SQLite 连接，供内部方法使用。
    pub(crate) fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, ManagerError> {
        self.conn
            .lock()
            .map_err(|_| ManagerError::Internal("registry connection is poisoned".to_string()))
    }

    fn init_schema(&self) -> Result<(), ManagerError> {
        let conn = self.lock_conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS client_installations (
                id TEXT PRIMARY KEY NOT NULL,
                client_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                install_dir TEXT NOT NULL,
                executable_path TEXT NOT NULL,
                storage_cfg_path TEXT NOT NULL,
                data_dir TEXT NOT NULL,
                user_data_dir TEXT,
                version TEXT,
                health TEXT NOT NULL,
                last_scanned_at TEXT,
                is_default INTEGER NOT NULL DEFAULT 0,
                client_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS download_jobs (
                id TEXT PRIMARY KEY NOT NULL,
                client_installation_id TEXT NOT NULL,
                client_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                version TEXT NOT NULL,
                asset_url TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                size INTEGER NOT NULL,
                status TEXT NOT NULL,
                downloaded_bytes INTEGER NOT NULL,
                cache_path TEXT NOT NULL,
                error TEXT,
                job_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS install_history (
                id TEXT PRIMARY KEY NOT NULL,
                job_id TEXT NOT NULL,
                client_installation_id TEXT NOT NULL,
                client_id TEXT NOT NULL,
                version TEXT NOT NULL,
                asset_url TEXT NOT NULL,
                package_kind TEXT NOT NULL,
                status TEXT NOT NULL,
                rollback_path TEXT,
                error TEXT,
                completed_at TEXT,
                record_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS client_fingerprints (
                sha256 TEXT PRIMARY KEY NOT NULL,
                client_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                version TEXT,
                company_name TEXT,
                product_name TEXT,
                recorded_at TEXT NOT NULL
            );",
        )
        .map_err(|error| {
            ManagerError::Internal(format!("failed to initialize registry schema: {error}"))
        })
    }
}

#[cfg(test)]
#[path = "test/registry.rs"]
mod tests;
