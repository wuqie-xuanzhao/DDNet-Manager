//! 应用设置的持久化。

use super::ClientRegistry;
use crate::error::ManagerError;
use crate::models::AppSettings;
use rusqlite::OptionalExtension;

impl ClientRegistry {
    /// 读取应用设置。未保存过设置时返回默认值。
    pub fn load_app_settings(&self) -> Result<AppSettings, ManagerError> {
        let conn = self.lock_conn()?;
        let value = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'settings' LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| {
                ManagerError::Internal(format!("failed to query app settings: {error}"))
            })?;

        let Some(json) = value else {
            return Ok(AppSettings::default());
        };

        let settings = serde_json::from_str(&json).map_err(|error| {
            ManagerError::Internal(format!("failed to parse app settings: {error}"))
        })?;
        if json.contains("\"github_token\"") {
            // 释放锁后再调用 save_app_settings 以避免死锁（save 内部也会获取同一把锁）。
            // 当前单进程场景下不存在竞态；若未来引入多线程并发写入设置，需改用重入锁或合并读写路径。
            drop(conn);
            self.save_app_settings(&settings)?;
            return Ok(settings);
        }
        Ok(settings)
    }

    /// 保存应用设置，并覆盖当前运行时使用的配置快照。
    pub fn save_app_settings(&self, settings: &AppSettings) -> Result<(), ManagerError> {
        let value = serde_json::to_string(settings).map_err(|error| {
            ManagerError::Internal(format!("failed to serialize app settings: {error}"))
        })?;
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![value],
        )
        .map(|_| ())
        .map_err(|error| {
            ManagerError::Internal(format!("failed to save app settings: {error}"))
        })
    }
}
