use crate::models::ClientInstallation;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

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
                );",
            )
            .map_err(|error| format!("failed to initialize registry schema: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{ClientHealth, ClientInstallation};

    fn test_client(id: &str, is_default: bool) -> ClientInstallation {
        ClientInstallation {
            id: id.to_string(),
            client_id: "qmclient".to_string(),
            display_name: "QmClient".to_string(),
            install_dir: format!("C:/Games/{id}"),
            executable_path: format!("C:/Games/{id}/DDNet.exe"),
            storage_cfg_path: format!("C:/Games/{id}/storage.cfg"),
            data_dir: format!("C:/Games/{id}/data"),
            user_data_dir: Some("C:/Users/Player/AppData/Roaming/DDNet".to_string()),
            version: None,
            is_default,
            health: ClientHealth::Ok,
            last_scanned_at: Some("2026-06-06T12:00:00Z".to_string()),
        }
    }

    #[test]
    fn registry_upserts_lists_and_sets_single_default_client() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let db_path = temp_dir.path().join("ddnet-manager.sqlite");
        let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

        registry
            .upsert_client_installation(&test_client("qmclient-main", true))
            .expect("第一条客户端应保存成功");
        registry
            .upsert_client_installation(&test_client("ddnet-vanilla", false))
            .expect("第二条客户端应保存成功");
        registry
            .set_default_client("ddnet-vanilla")
            .expect("默认客户端应设置成功");

        let clients = registry
            .list_client_installations()
            .expect("客户端列表应读取成功");
        let default_client = registry
            .get_default_client()
            .expect("默认客户端查询应成功")
            .expect("应存在默认客户端");

        assert_eq!(clients.len(), 2);
        assert_eq!(default_client.id, "ddnet-vanilla");
        assert_eq!(clients.iter().filter(|client| client.is_default).count(), 1);
    }

    #[test]
    fn registry_remove_does_not_delete_files_and_clears_default() {
        let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
        let db_path = temp_dir.path().join("ddnet-manager.sqlite");
        let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

        registry
            .upsert_client_installation(&test_client("qmclient-main", true))
            .expect("客户端应保存成功");
        registry
            .remove_client_installation("qmclient-main")
            .expect("客户端记录应移除成功");

        assert!(registry
            .get_default_client()
            .expect("默认客户端查询应成功")
            .is_none());
        assert!(registry
            .list_client_installations()
            .expect("客户端列表应读取成功")
            .is_empty());
    }
}
