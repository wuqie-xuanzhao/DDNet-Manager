//! 客户端指纹（sha256 → client_id/version）持久化。
//!
//! 用户用本工具下载客户端后，把 sha256 + PE 元信息存到 `client_fingerprints` 表。
//! 后续扫描硬盘找本地客户端时，若 exe sha256 命中本表，识别为"用户自己装过的"，
//! 升级 confidence 到 Verified，覆盖路径/PE 匹配的结果（不可伪造）。

use super::ClientRegistry;
use crate::error::ManagerError;
use rusqlite::{params, OptionalExtension};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// 一条客户端指纹记录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientFingerprint {
    pub sha256: String,
    pub client_id: String,
    pub display_name: String,
    pub version: Option<String>,
    pub company_name: Option<String>,
    pub product_name: Option<String>,
    pub recorded_at: String,
}

/// 写入指纹时的请求参数。`recorded_at` 由 [`ClientRegistry::record_client_fingerprint`]
/// 内部填入当前 UTC 时间，调用方不必提供。
pub struct FingerprintRecord<'a> {
    pub sha256: &'a str,
    pub client_id: &'a str,
    pub display_name: &'a str,
    pub version: Option<&'a str>,
    pub company_name: Option<&'a str>,
    pub product_name: Option<&'a str>,
}

impl ClientRegistry {
    /// 写入或更新一条指纹。同 sha256 覆盖（INSERT OR REPLACE）。
    /// sha256 内部统一转小写存储，查询时也按小写比较。
    pub fn record_client_fingerprint(
        &self,
        record: FingerprintRecord<'_>,
    ) -> Result<(), ManagerError> {
        let conn = self.lock_conn()?;
        let recorded_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
        conn.execute(
            "INSERT OR REPLACE INTO client_fingerprints (
                sha256, client_id, display_name, version, company_name, product_name, recorded_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.sha256.to_ascii_lowercase(),
                record.client_id,
                record.display_name,
                record.version,
                record.company_name,
                record.product_name,
                recorded_at,
            ],
        )
        .map_err(|e| ManagerError::Internal(format!("failed to record fingerprint: {e}")))?;
        Ok(())
    }

    /// 按 sha256 查询指纹（大小写不敏感）。
    pub fn lookup_fingerprint_by_hash(
        &self,
        sha256: &str,
    ) -> Result<Option<ClientFingerprint>, ManagerError> {
        let conn = self.lock_conn()?;
        let row = conn
            .query_row(
                "SELECT sha256, client_id, display_name, version, company_name, product_name, recorded_at
                 FROM client_fingerprints WHERE sha256 = ?1",
                params![sha256.to_ascii_lowercase()],
                |row| {
                    Ok(ClientFingerprint {
                        sha256: row.get(0)?,
                        client_id: row.get(1)?,
                        display_name: row.get(2)?,
                        version: row.get(3)?,
                        company_name: row.get(4)?,
                        product_name: row.get(5)?,
                        recorded_at: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(|e| ManagerError::Internal(format!("failed to lookup fingerprint: {e}")))?;
        Ok(row)
    }

    /// 列出所有指纹（调试 / 设置页展示用）。
    pub fn list_client_fingerprints(&self) -> Result<Vec<ClientFingerprint>, ManagerError> {
        let conn = self.lock_conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT sha256, client_id, display_name, version, company_name, product_name, recorded_at
                 FROM client_fingerprints ORDER BY recorded_at DESC",
            )
            .map_err(|e| ManagerError::Internal(format!("failed to prepare fingerprints: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ClientFingerprint {
                    sha256: row.get(0)?,
                    client_id: row.get(1)?,
                    display_name: row.get(2)?,
                    version: row.get(3)?,
                    company_name: row.get(4)?,
                    product_name: row.get(5)?,
                    recorded_at: row.get(6)?,
                })
            })
            .map_err(|e| ManagerError::Internal(format!("failed to query fingerprints: {e}")))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| {
                ManagerError::Internal(format!("failed to read fingerprint row: {e}"))
            })?);
        }
        Ok(out)
    }
}
