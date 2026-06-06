use std::path::Path;
use std::sync::Mutex;
use rusqlite::Connection;

use crate::routes::sync_profile::ProfileDto;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(path)?;
        let conn = Connection::open(path.join("clipsync.db"))?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS devices (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                device_id   TEXT NOT NULL UNIQUE,
                name        TEXT NOT NULL DEFAULT '',
                last_seen   TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS clipboard_history (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                hash        TEXT NOT NULL,
                content_type TEXT NOT NULL,
                text        TEXT NOT NULL DEFAULT '',
                data_name   TEXT,
                size        INTEGER NOT NULL DEFAULT 0,
                device_id   TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_history_hash ON clipboard_history(hash);
            CREATE INDEX IF NOT EXISTS idx_history_created ON clipboard_history(created_at);"
        )?;
        conn.execute_batch("ALTER TABLE clipboard_history ADD COLUMN has_data INTEGER NOT NULL DEFAULT 0")
            .ok();
        conn.execute_batch("ALTER TABLE clipboard_history ADD COLUMN text TEXT NOT NULL DEFAULT ''")
            .ok();
        Ok(())
    }

    // ── 设备注册 ──

    pub fn register_device(&self, device_id: &str, name: &str) {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO devices (device_id, name, last_seen)
                 VALUES (?1, ?2, datetime('now'))
                 ON CONFLICT(device_id) DO UPDATE SET
                     name = COALESCE(NULLIF(?2, ''), name),
                     last_seen = datetime('now')",
                rusqlite::params![device_id, name],
            )
            .ok();
    }

    pub fn get_device_name(&self, device_id: &str) -> Option<String> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT name FROM devices WHERE device_id = ?1",
                rusqlite::params![device_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .filter(|n| !n.is_empty())
    }

    // ── 剪贴板历史 ──

    pub fn get_latest_profile_with_source(&self) -> Option<(ProfileDto, String, String)> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT hash, content_type, text, data_name, size, has_data, device_id, created_at
                 FROM clipboard_history
                 ORDER BY id DESC LIMIT 1",
                [],
                |row| {
                    Ok((
                        ProfileDto {
                            hash: row.get(0)?,
                            content_type: row.get(1)?,
                            text: row.get::<_, String>(2).unwrap_or_default(),
                            has_data: row.get::<_, bool>(5).unwrap_or(false),
                            data_name: row.get::<_, String>(3).unwrap_or_default(),
                            size: row.get(4)?,
                        },
                        row.get::<_, String>(6).unwrap_or_default(),
                        row.get::<_, String>(7).unwrap_or_default(),
                    ))
                },
            )
            .ok()
    }

    pub fn get_latest_profile(&self) -> Option<ProfileDto> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT hash, content_type, text, data_name, size, has_data
                 FROM clipboard_history
                 ORDER BY id DESC LIMIT 1",
                [],
                |row| {
                    Ok(ProfileDto {
                        hash: row.get(0)?,
                        content_type: row.get(1)?,
                        text: row.get::<_, String>(2).unwrap_or_default(),
                        has_data: row.get::<_, bool>(5).unwrap_or(false),
                        data_name: row.get::<_, String>(3).unwrap_or_default(),
                        size: row.get(4)?,
                    })
                },
            )
            .ok()
    }

    pub fn save_profile(&self, profile: &ProfileDto, device_id: &str) {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO clipboard_history (hash, content_type, text, data_name, size, device_id, has_data)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    profile.hash,
                    profile.content_type,
                    profile.text,
                    profile.data_name,
                    profile.size,
                    device_id,
                    profile.has_data,
                ],
            )
            .ok();
    }

    // ── 离线消息 Backlog ──

    pub fn get_backlog(&self, device_id: &str) -> Vec<ProfileDto> {
        let conn = self.conn.lock().unwrap();
        let last_seen: Option<String> = conn
            .query_row(
                "SELECT last_seen FROM devices WHERE device_id = ?1",
                rusqlite::params![device_id],
                |row| row.get(0),
            )
            .ok();

        let Some(last_seen) = last_seen else { return vec![] };

        let mut stmt = conn
            .prepare(
                "SELECT hash, content_type, text, data_name, size, has_data
                 FROM clipboard_history
                 WHERE created_at > ?1 AND device_id != ?2
                 ORDER BY id ASC",
            )
            .unwrap();

        let rows = stmt
            .query_map(rusqlite::params![last_seen, device_id], |row| {
                Ok(ProfileDto {
                    hash: row.get(0)?,
                    content_type: row.get(1)?,
                    text: row.get::<_, String>(2).unwrap_or_default(),
                    has_data: row.get::<_, bool>(5).unwrap_or(false),
                    data_name: row.get::<_, String>(3).unwrap_or_default(),
                    size: row.get(4)?,
                })
            })
            .unwrap();

        rows.filter_map(|r| r.ok()).collect()
    }
}
