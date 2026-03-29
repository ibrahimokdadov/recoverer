use crate::error::Result;
use crate::events::ScanSession;
use rusqlite::{Connection, params};

pub struct SessionsStore {
    conn: Connection,
}

impl SessionsStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS sessions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT    NOT NULL,
                drive       TEXT    NOT NULL,
                db_path     TEXT    NOT NULL UNIQUE,
                created_at  INTEGER NOT NULL,
                total_files INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS recovered_clusters (
                drive         TEXT    NOT NULL,
                first_cluster INTEGER NOT NULL,
                PRIMARY KEY (drive, first_cluster)
            );
        ")?;
        Ok(Self { conn })
    }

    /// Record clusters that were successfully recovered for a drive.
    /// Called after each recovery operation so future scans of the same drive
    /// can automatically mark matching files as already-recovered.
    pub fn record_recovered_clusters(&self, drive: &str, clusters: &[u64]) -> Result<()> {
        if clusters.is_empty() { return Ok(()); }
        let mut stmt = self.conn.prepare_cached(
            "INSERT OR IGNORE INTO recovered_clusters (drive, first_cluster) VALUES (?1, ?2)"
        )?;
        for &c in clusters {
            stmt.execute(params![drive, c as i64])?;
        }
        Ok(())
    }

    /// Return all clusters previously recovered on a drive, for cross-referencing a new scan.
    pub fn get_recovered_clusters(&self, drive: &str) -> Result<Vec<u64>> {
        let mut stmt = self.conn.prepare(
            "SELECT first_cluster FROM recovered_clusters WHERE drive = ?1"
        )?;
        let rows = stmt.query_map(params![drive], |row| {
            row.get::<_, i64>(0).map(|v| v as u64)
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn register(&self, name: &str, drive: &str, db_path: &str, created_at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO sessions (name, drive, db_path, created_at) VALUES (?1,?2,?3,?4)",
            params![name, drive, db_path, created_at],
        )?;
        Ok(())
    }

    /// List all sessions, refreshing live file counts from each session DB.
    pub fn list(&self) -> Result<Vec<ScanSession>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, drive, db_path, created_at, total_files
             FROM sessions ORDER BY created_at DESC"
        )?;
        let mut sessions: Vec<ScanSession> = stmt
            .query_map([], |row| Ok(ScanSession {
                id:          row.get(0)?,
                name:        row.get(1)?,
                drive:       row.get(2)?,
                db_path:     row.get(3)?,
                created_at:  row.get(4)?,
                total_files: row.get(5)?,
            }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for s in &mut sessions {
            if let Ok(conn) = Connection::open(&s.db_path) {
                if let Ok(n) = conn.query_row(
                    "SELECT COUNT(*) FROM files", [], |r| r.get::<_, i64>(0)
                ) {
                    s.total_files = n;
                    let _ = self.conn.execute(
                        "UPDATE sessions SET total_files = ?1 WHERE id = ?2",
                        params![n, s.id],
                    );
                }
            }
        }
        Ok(sessions)
    }

    pub fn get_db_path_by_id(&self, id: i64) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT db_path FROM sessions WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(p)                                        => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows)    => Ok(None),
            Err(e)                                       => Err(e.into()),
        }
    }
}

pub fn new_session_db_path(sessions_dir: &std::path::Path, drive: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let letter = drive.chars().next().unwrap_or('X').to_ascii_uppercase();
    sessions_dir.join(format!("{}_{}.db", letter, ts))
        .to_string_lossy()
        .to_string()
}
