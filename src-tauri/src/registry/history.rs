//! Manager-owned 安装历史记录的持久化。

use super::ClientRegistry;
use crate::error::ManagerError;
use crate::models::InstallHistoryRecord;
use rusqlite::params;

impl ClientRegistry {
    /// 记录一次已完成或失败的 Manager-owned 安装事务。
    pub fn record_install_history(
        &self,
        record: &InstallHistoryRecord,
    ) -> Result<(), ManagerError> {
        let status = serde_json::to_string(&record.status).map_err(|error| {
            ManagerError::Internal(format!("failed to serialize install status: {error}"))
        })?;
        let conn = self.lock_conn()?;
        conn.execute(
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
                    ManagerError::Internal(format!("failed to serialize install history: {error}"))
                })?
            ],
        )
        .map(|_| ())
        .map_err(|error| {
            ManagerError::Internal(format!("failed to record install history: {error}"))
        })
    }

    /// 返回指定客户端的安装历史，最新记录排在前面。
    pub fn list_install_history(
        &self,
        client_installation_id: &str,
    ) -> Result<Vec<InstallHistoryRecord>, ManagerError> {
        let conn = self.lock_conn()?;
        let mut statement = conn
            .prepare(
                "SELECT record_json
                 FROM install_history
                 WHERE client_installation_id = ?1
                 ORDER BY completed_at DESC, id DESC",
            )
            .map_err(|error| {
                ManagerError::Internal(format!("failed to query install history: {error}"))
            })?;
        let rows = statement
            .query_map(params![client_installation_id], |row| row.get::<_, String>(0))
            .map_err(|error| {
                ManagerError::Internal(format!("failed to read install history: {error}"))
            })?;
        let mut history = Vec::new();
        for row in rows {
            let record_json = row.map_err(|error| {
                ManagerError::Internal(format!("failed to read install history row: {error}"))
            })?;
            history.push(serde_json::from_str(&record_json).map_err(|error| {
                ManagerError::Internal(format!("failed to parse install history: {error}"))
            })?);
        }
        Ok(history)
    }
}
