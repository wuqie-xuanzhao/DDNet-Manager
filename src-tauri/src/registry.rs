use crate::models::{AppSettings, ClientInstallation, CompatibilityStatus, InstallHistoryRecord};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

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
pub struct ClientRegistry {
    conn: Connection,
}

impl ClientRegistry {
    /// 打开或创建客户端注册表，并初始化最小 schema。
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create registry dir: {error}"))?;
        }

        let conn =
            Connection::open(path).map_err(|error| format!("failed to open registry: {error}"))?;
        let registry = Self { conn };
        registry.init_schema()?;
        Ok(registry)
    }

    /// 保存或更新客户端安装记录。若记录为默认客户端，会清除其他默认标记。
    pub fn upsert_client_installation(&self, client: &ClientInstallation) -> Result<(), String> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| format!("failed to start registry transaction: {error}"))?;

        if client.is_default {
            tx.execute("UPDATE client_installations SET is_default = 0", [])
                .map_err(|error| format!("failed to clear default client: {error}"))?;
        }

        let client_json = serde_json::to_string(client)
            .map_err(|error| format!("failed to serialize client installation: {error}"))?;
        tx.execute(
            "INSERT INTO client_installations (
                id, client_id, display_name, install_dir, executable_path,
                storage_cfg_path, data_dir, user_data_dir, version, health,
                last_scanned_at, is_default, client_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                client_id = excluded.client_id,
                display_name = excluded.display_name,
                install_dir = excluded.install_dir,
                executable_path = excluded.executable_path,
                storage_cfg_path = excluded.storage_cfg_path,
                data_dir = excluded.data_dir,
                user_data_dir = excluded.user_data_dir,
                version = excluded.version,
                health = excluded.health,
                last_scanned_at = excluded.last_scanned_at,
                is_default = excluded.is_default,
                client_json = excluded.client_json",
            params![
                client.id,
                client.client_id,
                client.display_name,
                client.install_dir,
                client.executable_path,
                client.storage_cfg_path,
                client.data_dir,
                client.user_data_dir,
                client.version,
                serde_json::to_string(&client.health)
                    .map_err(|error| format!("failed to serialize client health: {error}"))?,
                client.last_scanned_at,
                client.is_default,
                client_json
            ],
        )
        .map_err(|error| format!("failed to upsert client installation: {error}"))?;

        tx.commit()
            .map_err(|error| format!("failed to commit registry transaction: {error}"))
    }

    /// 返回已保存的所有客户端安装记录。
    pub fn list_client_installations(&self) -> Result<Vec<ClientInstallation>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT client_json, is_default
                 FROM client_installations
                 ORDER BY is_default DESC, display_name ASC, install_dir ASC",
            )
            .map_err(|error| format!("failed to query client installations: {error}"))?;

        let rows = statement
            .query_map([], |row| {
                let client_json: String = row.get(0)?;
                let is_default: bool = row.get(1)?;
                Ok((client_json, is_default))
            })
            .map_err(|error| format!("failed to read client installations: {error}"))?;

        let mut clients = Vec::new();
        for row in rows {
            let (client_json, is_default) =
                row.map_err(|error| format!("failed to read client installation row: {error}"))?;
            let mut client: ClientInstallation = serde_json::from_str(&client_json)
                .map_err(|error| format!("failed to parse client installation: {error}"))?;
            client.is_default = is_default;
            normalize_client_installation(&mut client);
            clients.push(client);
        }

        Ok(clients)
    }

    /// 设置默认启动客户端。
    pub fn set_default_client(&self, id: &str) -> Result<(), String> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|error| format!("failed to start registry transaction: {error}"))?;
        let changed = tx
            .execute(
                "UPDATE client_installations SET is_default = 1 WHERE id = ?1",
                params![id],
            )
            .map_err(|error| format!("failed to set default client: {error}"))?;
        if changed == 0 {
            return Err(format!("client installation not found: {id}"));
        }
        tx.execute(
            "UPDATE client_installations SET is_default = 0 WHERE id <> ?1",
            params![id],
        )
        .map_err(|error| format!("failed to clear other default clients: {error}"))?;
        tx.commit()
            .map_err(|error| format!("failed to commit registry transaction: {error}"))
    }

    /// 返回当前默认客户端。没有默认客户端时返回空。
    pub fn get_default_client(&self) -> Result<Option<ClientInstallation>, String> {
        let row = self
            .conn
            .query_row(
                "SELECT client_json FROM client_installations WHERE is_default = 1 LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("failed to query default client: {error}"))?;

        row.map(|client_json| {
            let mut client: ClientInstallation = serde_json::from_str(&client_json)
                .map_err(|error| format!("failed to parse default client: {error}"))?;
            client.is_default = true;
            normalize_client_installation(&mut client);
            Ok(client)
        })
        .transpose()
    }

    /// 从注册表移除客户端安装记录，不删除本地文件。
    pub fn remove_client_installation(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM client_installations WHERE id = ?1",
                params![id],
            )
            .map(|_| ())
            .map_err(|error| format!("failed to remove client installation: {error}"))
    }

    /// 读取应用设置。未保存过设置时返回默认值。
    pub fn load_app_settings(&self) -> Result<AppSettings, String> {
        let value = self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'settings' LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("failed to query app settings: {error}"))?;

        value
            .map(|json| {
                serde_json::from_str(&json)
                    .map_err(|error| format!("failed to parse app settings: {error}"))
            })
            .transpose()
            .map(|settings| settings.unwrap_or_default())
    }

    /// 保存应用设置，并覆盖当前运行时使用的配置快照。
    pub fn save_app_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let value = serde_json::to_string(settings)
            .map_err(|error| format!("failed to serialize app settings: {error}"))?;
        self.conn
            .execute(
                "INSERT INTO app_settings (key, value) VALUES ('settings', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![value],
            )
            .map(|_| ())
            .map_err(|error| format!("failed to save app settings: {error}"))
    }

    /// 记录一次已完成或失败的 Manager-owned 安装事务。
    pub fn record_install_history(&self, record: &InstallHistoryRecord) -> Result<(), String> {
        let status = serde_json::to_string(&record.status)
            .map_err(|error| format!("failed to serialize install status: {error}"))?;
        self.conn
            .execute(
                "INSERT INTO install_history (
                    id, job_id, client_installation_id, client_id, version, asset_url,
                    package_kind, status, rollback_path, error, completed_at, record_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(id) DO UPDATE SET
                    job_id = excluded.job_id,
                    client_installation_id = excluded.client_installation_id,
                    client_id = excluded.client_id,
                    version = excluded.version,
                    asset_url = excluded.asset_url,
                    package_kind = excluded.package_kind,
                    status = excluded.status,
                    rollback_path = excluded.rollback_path,
                    error = excluded.error,
                    completed_at = excluded.completed_at,
                    record_json = excluded.record_json",
                params![
                    record.id,
                    record.job_id,
                    record.client_installation_id,
                    record.client_id,
                    record.version,
                    record.asset_url,
                    record.package_kind,
                    status,
                    record.rollback_path,
                    record.error,
                    record.completed_at,
                    serde_json::to_string(record).map_err(|error| {
                        format!("failed to serialize install history: {error}")
                    })?
                ],
            )
            .map(|_| ())
            .map_err(|error| format!("failed to record install history: {error}"))
    }

    /// 返回指定客户端的安装历史，最新记录排在前面。
    pub fn list_install_history(
        &self,
        client_installation_id: &str,
    ) -> Result<Vec<InstallHistoryRecord>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT record_json
                 FROM install_history
                 WHERE client_installation_id = ?1
                 ORDER BY completed_at DESC, id DESC",
            )
            .map_err(|error| format!("failed to query install history: {error}"))?;
        let rows = statement
            .query_map(params![client_installation_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| format!("failed to read install history: {error}"))?;
        let mut history = Vec::new();
        for row in rows {
            let record_json =
                row.map_err(|error| format!("failed to read install history row: {error}"))?;
            history.push(
                serde_json::from_str(&record_json)
                    .map_err(|error| format!("failed to parse install history: {error}"))?,
            );
        }
        Ok(history)
    }

    /// 写回一次受控启动探测结果。
    pub fn record_launch_probe_result(&self, record: LaunchProbeRecord<'_>) -> Result<(), String> {
        let mut client = self
            .client_installation_by_id(record.client_installation_id)?
            .ok_or_else(|| {
                format!(
                    "client installation not found: {}",
                    record.client_installation_id
                )
            })?;
        client.compatibility.launch_verified = record.status == LaunchProbeStatus::Verified;
        client.compatibility.last_launch_result = Some(record.message.to_string());
        match record.status {
            LaunchProbeStatus::Verified => {
                client.compatibility.status = CompatibilityStatus::Verified;
                client.compatibility.can_launch = true;
            }
            LaunchProbeStatus::Unobserved => {}
            LaunchProbeStatus::Exited => {
                client.compatibility.status = CompatibilityStatus::Risky;
            }
        }
        self.upsert_client_installation(&client)
    }

    /// 按安装记录 ID 读取客户端记录。
    pub fn client_installation_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ClientInstallation>, String> {
        let row = self
            .conn
            .query_row(
                "SELECT client_json, is_default FROM client_installations WHERE id = ?1 LIMIT 1",
                params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
            )
            .optional()
            .map_err(|error| format!("failed to query client installation: {error}"))?;

        row.map(|(client_json, is_default)| {
            let mut client: ClientInstallation = serde_json::from_str(&client_json)
                .map_err(|error| format!("failed to parse client installation: {error}"))?;
            client.is_default = is_default;
            normalize_client_installation(&mut client);
            Ok(client)
        })
        .transpose()
    }

    fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
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
                CREATE TABLE IF NOT EXISTS scan_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    scanned_at TEXT NOT NULL,
                    root TEXT NOT NULL,
                    candidate_count INTEGER NOT NULL
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
                );",
            )
            .map_err(|error| format!("failed to initialize registry schema: {error}"))
    }
}

fn normalize_client_installation(client: &mut ClientInstallation) {
    client.client_id = crate::client_catalog::normalize_client_id(&client.client_id).to_string();
}

#[cfg(test)]
#[path = "test/registry.rs"]
mod tests;
