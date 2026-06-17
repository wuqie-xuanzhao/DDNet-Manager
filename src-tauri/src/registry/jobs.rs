//! 下载任务快照的持久化。

use super::ClientRegistry;
use crate::error::ManagerError;
use crate::models::DownloadJob;
use rusqlite::{params, OptionalExtension};

impl ClientRegistry {
    /// 保存或更新下载任务快照。
    pub fn upsert_download_job(&self, job: &DownloadJob) -> Result<(), ManagerError> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO download_jobs (
                id, client_installation_id, client_id, channel, version, asset_url,
                sha256, size, status, downloaded_bytes, cache_path, error, job_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                client_installation_id = excluded.client_installation_id,
                client_id = excluded.client_id,
                channel = excluded.channel,
                version = excluded.version,
                asset_url = excluded.asset_url,
                sha256 = excluded.sha256,
                size = excluded.size,
                status = excluded.status,
                downloaded_bytes = excluded.downloaded_bytes,
                cache_path = excluded.cache_path,
                error = excluded.error,
                job_json = excluded.job_json",
            params![
                job.id,
                job.client_installation_id,
                job.client_id,
                job.channel,
                job.version,
                job.asset_url,
                job.sha256,
                job.size,
                serde_json::to_string(&job.status).map_err(|error| {
                    ManagerError::Internal(format!(
                        "failed to serialize download job status: {error}"
                    ))
                })?,
                job.downloaded_bytes,
                job.cache_path,
                job.error,
                serde_json::to_string(job).map_err(|error| {
                    ManagerError::Internal(format!("failed to serialize download job: {error}"))
                })?
            ],
        )
        .map(|_| ())
        .map_err(|error| ManagerError::Internal(format!("failed to upsert download job: {error}")))
    }

    /// 返回指定客户端的下载任务列表；为空时返回全部任务。
    pub fn list_download_jobs(
        &self,
        client_installation_id: Option<&str>,
    ) -> Result<Vec<DownloadJob>, ManagerError> {
        let conn = self.lock_conn()?;
        let sql = if client_installation_id.is_some() {
            "SELECT job_json
             FROM download_jobs
             WHERE client_installation_id = ?1
             ORDER BY id DESC"
        } else {
            "SELECT job_json
             FROM download_jobs
             ORDER BY id DESC"
        };
        let mut statement = conn.prepare(sql).map_err(|error| {
            ManagerError::Internal(format!("failed to query download jobs: {error}"))
        })?;
        let mut jobs = Vec::new();
        if let Some(client_installation_id) = client_installation_id {
            let rows = statement
                .query_map(params![client_installation_id], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|error| {
                    ManagerError::Internal(format!("failed to read download jobs: {error}"))
                })?;
            for row in rows {
                let job_json = row.map_err(|error| {
                    ManagerError::Internal(format!("failed to read download job row: {error}"))
                })?;
                jobs.push(serde_json::from_str(&job_json).map_err(|error| {
                    ManagerError::Internal(format!("failed to parse download job: {error}"))
                })?);
            }
        } else {
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| {
                    ManagerError::Internal(format!("failed to read download jobs: {error}"))
                })?;
            for row in rows {
                let job_json = row.map_err(|error| {
                    ManagerError::Internal(format!("failed to read download job row: {error}"))
                })?;
                jobs.push(serde_json::from_str(&job_json).map_err(|error| {
                    ManagerError::Internal(format!("failed to parse download job: {error}"))
                })?);
            }
        }
        Ok(jobs)
    }

    /// 按下载任务 ID 读取任务快照。
    pub fn download_job_by_id(&self, id: &str) -> Result<Option<DownloadJob>, ManagerError> {
        let conn = self.lock_conn()?;
        let row = conn
            .query_row(
                "SELECT job_json FROM download_jobs WHERE id = ?1 LIMIT 1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| {
                ManagerError::Internal(format!("failed to query download job: {error}"))
            })?;

        row.map(|job_json| {
            serde_json::from_str(&job_json).map_err(|error| {
                ManagerError::Internal(format!("failed to parse download job: {error}"))
            })
        })
        .transpose()
    }

    /// 删除已不再需要的下载任务快照。
    pub fn remove_download_job(&self, id: &str) -> Result<(), ManagerError> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM download_jobs WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(|error| {
                ManagerError::Internal(format!("failed to remove download job: {error}"))
            })
    }
}
