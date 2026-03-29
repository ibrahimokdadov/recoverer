use crate::error::Result;
use crate::events::{FileRecord, RecoveryStatus};
use rusqlite::{Connection, params, types::ToSql};
use std::sync::Mutex;

pub struct Store {
    conn: Mutex<Connection>,
}

pub struct NewFile {
    pub filename: Option<String>,
    pub original_path: Option<String>,
    pub mime_type: String,
    pub category: String,
    pub size_bytes: u64,
    pub first_cluster: Option<u64>,
    pub confidence: u8,
    pub source: String,
    pub mft_record_number: Option<u64>,
    pub created_at: Option<i64>,
    pub modified_at: Option<i64>,
    pub deleted_at: Option<i64>,
}

impl Store {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                filename TEXT,
                original_path TEXT,
                mime_type TEXT NOT NULL,
                category TEXT NOT NULL,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                first_cluster INTEGER,
                confidence INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL,
                recovery_status TEXT NOT NULL DEFAULT 'pending',
                mft_record_number INTEGER,
                created_at INTEGER,
                modified_at INTEGER,
                deleted_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_category ON files(category);
            CREATE INDEX IF NOT EXISTS idx_confidence ON files(confidence);
            CREATE INDEX IF NOT EXISTS idx_filename ON files(filename COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_cluster ON files(first_cluster);
            CREATE TABLE IF NOT EXISTS scan_checkpoint (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        ")?;
        // Migrations — ignore errors if columns already exist
        conn.execute(
            "ALTER TABLE files ADD COLUMN fragment_group_id INTEGER NOT NULL DEFAULT 0", [],
        ).ok();
        conn.execute(
            "ALTER TABLE files ADD COLUMN fragment_chain_lead INTEGER NOT NULL DEFAULT 0", [],
        ).ok();
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn insert_file(&self, f: &NewFile) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO files (filename, original_path, mime_type, category, size_bytes,
             first_cluster, confidence, source, mft_record_number, created_at, modified_at, deleted_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                f.filename, f.original_path, f.mime_type, f.category, f.size_bytes as i64,
                f.first_cluster.map(|c| c as i64), f.confidence as i64, f.source,
                f.mft_record_number.map(|n| n as i64),
                f.created_at, f.modified_at, f.deleted_at
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn query_files(
        &self,
        category: Option<&str>,
        min_confidence: Option<i32>,
        name_contains: Option<&str>,
        exclude_recovered: bool,
        collapse_fragments: bool,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<FileRecord>> {
        let like_pattern = name_contains.map(|n| {
            let escaped = n.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            format!("%{}%", escaped)
        });

        let mut conditions: Vec<String> = Vec::new();
        let mut p: Vec<Box<dyn ToSql>> = Vec::new();

        if let Some(cat) = category {
            p.push(Box::new(cat.to_string()));
            conditions.push(format!("category = ?{}", p.len()));
        }
        if let Some(mc) = min_confidence {
            p.push(Box::new(mc));
            conditions.push(format!("confidence >= ?{}", p.len()));
        }
        if let Some(ref lp) = like_pattern {
            p.push(Box::new(lp.clone()));
            conditions.push(format!("filename LIKE ?{} ESCAPE '\\' COLLATE NOCASE", p.len()));
        }
        if exclude_recovered {
            conditions.push("recovery_status != 'recovered'".to_string());
        }
        if collapse_fragments {
            conditions.push("(fragment_group_id = 0 OR fragment_chain_lead = 1)".to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        p.push(Box::new(limit));
        let limit_idx = p.len();
        p.push(Box::new(offset));
        let offset_idx = p.len();

        let sql = format!(
            "SELECT id, filename, original_path, mime_type, category, size_bytes, confidence, source, recovery_status, modified_at, fragment_group_id
             FROM files {} ORDER BY id DESC LIMIT ?{} OFFSET ?{}",
            where_clause, limit_idx, offset_idx
        );

        let p_refs: Vec<&dyn ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(p_refs.as_slice(), |row| {
            let status_str: String = row.get(8)?;
            let recovery_status = match status_str.as_str() {
                "recovered" => RecoveryStatus::Recovered,
                "failed"    => RecoveryStatus::Failed,
                "skipped"   => RecoveryStatus::Skipped,
                _           => RecoveryStatus::Pending,
            };
            Ok(FileRecord {
                id:                row.get(0)?,
                filename:          row.get(1)?,
                original_path:     row.get(2)?,
                mime_type:         row.get(3)?,
                category:          row.get(4)?,
                size_bytes:        row.get::<_, i64>(5)? as u64,
                confidence:        row.get::<_, i64>(6)? as u8,
                source:            row.get(7)?,
                recovery_status,
                modified_at:       row.get(9)?,
                fragment_group_id: row.get::<_, i64>(10).unwrap_or(0),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn total_count(
        &self,
        category: Option<&str>,
        min_confidence: Option<i32>,
        name_contains: Option<&str>,
        exclude_recovered: bool,
        collapse_fragments: bool,
    ) -> Result<i64> {
        let like_pattern = name_contains.map(|n| {
            let escaped = n.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            format!("%{}%", escaped)
        });

        let mut conditions: Vec<String> = Vec::new();
        let mut p: Vec<Box<dyn ToSql>> = Vec::new();

        if let Some(cat) = category {
            p.push(Box::new(cat.to_string()));
            conditions.push(format!("category = ?{}", p.len()));
        }
        if let Some(mc) = min_confidence {
            p.push(Box::new(mc));
            conditions.push(format!("confidence >= ?{}", p.len()));
        }
        if let Some(ref lp) = like_pattern {
            p.push(Box::new(lp.clone()));
            conditions.push(format!("filename LIKE ?{} ESCAPE '\\' COLLATE NOCASE", p.len()));
        }
        if exclude_recovered {
            conditions.push("recovery_status != 'recovered'".to_string());
        }
        if collapse_fragments {
            conditions.push("(fragment_group_id = 0 OR fragment_chain_lead = 1)".to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) FROM files {}", where_clause);
        let p_refs: Vec<&dyn ToSql> = p.iter().map(|b| b.as_ref()).collect();

        Ok(self.conn.lock().unwrap().query_row(&sql, p_refs.as_slice(), |row| row.get(0))?)
    }

    pub fn update_recovery_status(&self, id: i64, status: RecoveryStatus) -> Result<()> {
        let status_str = match status {
            RecoveryStatus::Pending => "pending",
            RecoveryStatus::Recovered => "recovered",
            RecoveryStatus::Failed => "failed",
            RecoveryStatus::Skipped => "skipped",
        };
        self.conn.lock().unwrap().execute(
            "UPDATE files SET recovery_status = ?1 WHERE id = ?2",
            params![status_str, id],
        )?;
        Ok(())
    }

    pub fn get_file_by_id(&self, id: i64) -> Result<Option<FileRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, original_path, mime_type, category, size_bytes, confidence, source, recovery_status, modified_at, fragment_group_id
             FROM files WHERE id = ?1"
        )?;
        let result = stmt.query_row(rusqlite::params![id], |row| {
            Ok(FileRecord {
                id:                row.get(0)?,
                filename:          row.get(1)?,
                original_path:     row.get(2)?,
                mime_type:         row.get(3)?,
                category:          row.get(4)?,
                size_bytes:        row.get::<_, i64>(5)? as u64,
                confidence:        row.get::<_, i64>(6)? as u8,
                source:            row.get(7)?,
                recovery_status: {
                    let s: String = row.get(8)?;
                    match s.as_str() {
                        "recovered" => crate::events::RecoveryStatus::Recovered,
                        "failed"    => crate::events::RecoveryStatus::Failed,
                        "skipped"   => crate::events::RecoveryStatus::Skipped,
                        _           => crate::events::RecoveryStatus::Pending,
                    }
                },
                modified_at:       row.get(9)?,
                fragment_group_id: row.get::<_, i64>(10).unwrap_or(0),
            })
        });
        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(crate::error::EngineError::Database(e)),
        }
    }

    pub fn clear_files(&self) -> Result<()> {
        self.conn.lock().unwrap().execute("DELETE FROM files", [])?;
        Ok(())
    }

    /// Returns (first_cluster, size_bytes, source) for the given file id, or None if not found.
    pub fn get_file_cluster_and_size(&self, id: i64) -> Result<Option<(u64, u64, String)>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT first_cluster, size_bytes, source FROM files WHERE id = ?1",
            rusqlite::params![id],
            |row| Ok((
                row.get::<_, Option<i64>>(0)?.map(|v| v as u64),
                row.get::<_, i64>(1)? as u64,
                row.get::<_, String>(2)?,
            )),
        );
        match result {
            Ok((cluster, size, source)) => Ok(cluster.map(|c| (c, size, source))),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(crate::error::EngineError::Database(e)),
        }
    }

    pub fn save_checkpoint(&self, key: &str, value: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO scan_checkpoint (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Returns (id, first_cluster, size_bytes, category, mime_type) for all carved files, sorted by sector.
    pub fn get_carved_for_fragment_detection(&self) -> Result<Vec<(i64, u64, u64, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, first_cluster, size_bytes, category, mime_type FROM files
             WHERE source = 'carved' AND first_cluster IS NOT NULL
             ORDER BY first_cluster ASC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)? as u64,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Assign a fragment_group_id to a batch of file IDs.
    /// `ids` must be in sector order — ids[0] is the chain lead.
    pub fn set_fragment_group(&self, ids: &[i64], group_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        for (i, id) in ids.iter().enumerate() {
            let is_lead = if i == 0 { 1i64 } else { 0i64 };
            conn.execute(
                "UPDATE files SET fragment_group_id = ?1, fragment_chain_lead = ?2 WHERE id = ?3",
                params![group_id, is_lead, id],
            )?;
        }
        Ok(())
    }

    /// Returns (id, first_cluster) for all files in a fragment group, sorted by sector.
    pub fn get_fragment_siblings(&self, group_id: i64) -> Result<Vec<(i64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, first_cluster FROM files
             WHERE fragment_group_id = ?1 AND first_cluster IS NOT NULL
             ORDER BY first_cluster ASC"
        )?;
        let rows = stmt.query_map(params![group_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)? as u64))
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Returns (id, first_cluster/sector, size_bytes) for all files in a fragment group, sorted by sector.
    pub fn get_fragment_siblings_with_size(&self, group_id: i64) -> Result<Vec<(i64, u64, u64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, first_cluster, size_bytes FROM files
             WHERE fragment_group_id = ?1 AND first_cluster IS NOT NULL
             ORDER BY first_cluster ASC"
        )?;
        let rows = stmt.query_map(params![group_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)? as u64,
            ))
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Set the chain lead's size_bytes to the total span of the chain.
    pub fn update_chain_lead_size(&self, group_id: i64, size_bytes: u64) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE files SET size_bytes = ?1 WHERE fragment_group_id = ?2 AND fragment_chain_lead = 1",
            params![size_bytes as i64, group_id],
        )?;
        Ok(())
    }

    /// Mark all files whose first_cluster is in the given set as 'recovered'.
    /// Returns the number of files updated.
    pub fn bulk_mark_recovered_by_clusters(&self, clusters: &[u64]) -> Result<u64> {
        if clusters.is_empty() { return Ok(0); }
        let conn = self.conn.lock().unwrap();
        let mut updated = 0u64;
        let mut stmt = conn.prepare_cached(
            "UPDATE files SET recovery_status = 'recovered'
             WHERE first_cluster = ?1 AND recovery_status != 'recovered'"
        )?;
        for &c in clusters {
            updated += stmt.execute(params![c as i64])? as u64;
        }
        Ok(updated)
    }

    /// Return (first_cluster) for all recovered files in this session.
    pub fn get_recovered_clusters(&self) -> Result<Vec<u64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT first_cluster FROM files
             WHERE recovery_status = 'recovered' AND first_cluster IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |row| {
            row.get::<_, i64>(0).map(|v| v as u64)
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn load_checkpoint(&self, key: &str) -> Result<Option<String>> {
        let result = self.conn.lock().unwrap().query_row(
            "SELECT value FROM scan_checkpoint WHERE key = ?1",
            params![key],
            |row| row.get(0),
        );
        match result {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
