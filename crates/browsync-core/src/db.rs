use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::params;

use crate::models::{Bookmark, Browser, HistoryEntry};

pub struct Database {
    conn: rusqlite::Connection,
}

impl Database {
    /// Open or create the browsync database at ~/.browsync/browsync.db
    pub fn open_default() -> Result<Self> {
        let dir = Self::data_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("browsync.db");
        Self::open(&path)
    }

    pub fn data_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".browsync"))
    }

    pub fn open(path: &PathBuf) -> Result<Self> {
        let conn = rusqlite::Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn open_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                folder_path TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '',
                favicon_url TEXT,
                source_browser TEXT NOT NULL,
                source_id TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                synced_at TEXT NOT NULL
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_bookmarks_url_browser ON bookmarks(url, source_browser);
            CREATE INDEX IF NOT EXISTS idx_bookmarks_browser ON bookmarks(source_browser);

            CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                visit_count INTEGER NOT NULL DEFAULT 1,
                last_visited TEXT NOT NULL,
                source_browser TEXT NOT NULL,
                duration_secs INTEGER
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_history_url_browser ON history(url, source_browser);
            CREATE INDEX IF NOT EXISTS idx_history_visited ON history(last_visited);
            CREATE INDEX IF NOT EXISTS idx_history_browser ON history(source_browser);

            CREATE TABLE IF NOT EXISTS auth_entries (
                id TEXT PRIMARY KEY,
                domain TEXT NOT NULL,
                username TEXT NOT NULL DEFAULT '',
                source_browser TEXT NOT NULL,
                last_used TEXT,
                password_manager TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_auth_domain ON auth_entries(domain);

            -- FTS5 virtual tables for full-text search
            CREATE VIRTUAL TABLE IF NOT EXISTS bookmarks_fts USING fts5(
                url, title, folder_path, tags,
                content='bookmarks',
                content_rowid='rowid'
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS history_fts USING fts5(
                url, title,
                content='history',
                content_rowid='rowid'
            );

            -- Triggers to keep FTS in sync
            CREATE TRIGGER IF NOT EXISTS bookmarks_ai AFTER INSERT ON bookmarks BEGIN
                INSERT INTO bookmarks_fts(rowid, url, title, folder_path, tags)
                VALUES (new.rowid, new.url, new.title, new.folder_path, new.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS bookmarks_ad AFTER DELETE ON bookmarks BEGIN
                INSERT INTO bookmarks_fts(bookmarks_fts, rowid, url, title, folder_path, tags)
                VALUES ('delete', old.rowid, old.url, old.title, old.folder_path, old.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS history_ai AFTER INSERT ON history BEGIN
                INSERT INTO history_fts(rowid, url, title)
                VALUES (new.rowid, new.url, new.title);
            END;

            CREATE TRIGGER IF NOT EXISTS history_ad AFTER DELETE ON history BEGIN
                INSERT INTO history_fts(history_fts, rowid, url, title)
                VALUES ('delete', old.rowid, old.url, old.title);
            END;

            -- Sync metadata
            CREATE TABLE IF NOT EXISTS sync_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                browser TEXT NOT NULL,
                sync_type TEXT NOT NULL,
                items_synced INTEGER NOT NULL DEFAULT 0,
                synced_at TEXT NOT NULL
            );

            -- Bookmark summaries (AI-generated, cached)
            CREATE TABLE IF NOT EXISTS summaries (
                url TEXT PRIMARY KEY,
                summary TEXT NOT NULL,
                engine TEXT NOT NULL DEFAULT 'extractive',
                created_at TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    /// Insert or update a bookmark (upsert by URL + source_browser)
    pub fn upsert_bookmark(&self, bookmark: &Bookmark) -> Result<()> {
        let folder_path = bookmark.folder_path.join("/");
        let tags = bookmark.tags.join(",");
        let browser = serde_json::to_string(&bookmark.source_browser)?;

        self.conn.execute(
            "INSERT INTO bookmarks (id, url, title, folder_path, tags, favicon_url,
                                    source_browser, source_id, created_at, modified_at, synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(url, source_browser) DO UPDATE SET
                title = excluded.title,
                folder_path = excluded.folder_path,
                tags = excluded.tags,
                modified_at = excluded.modified_at,
                synced_at = excluded.synced_at",
            params![
                bookmark.id.to_string(),
                bookmark.url,
                bookmark.title,
                folder_path,
                tags,
                bookmark.favicon_url,
                browser.trim_matches('"'),
                bookmark.source_id,
                bookmark.created_at.to_rfc3339(),
                bookmark.modified_at.to_rfc3339(),
                bookmark.synced_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Bulk insert bookmarks
    pub fn insert_bookmarks(&self, bookmarks: &[Bookmark]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;
        for b in bookmarks {
            self.upsert_bookmark(b)?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    /// Insert a history entry
    pub fn upsert_history(&self, entry: &HistoryEntry) -> Result<()> {
        let browser = serde_json::to_string(&entry.source_browser)?;

        self.conn.execute(
            "INSERT INTO history (id, url, title, visit_count, last_visited,
                                  source_browser, duration_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(url, source_browser) DO UPDATE SET
                title = excluded.title,
                visit_count = MAX(history.visit_count, excluded.visit_count),
                last_visited = MAX(history.last_visited, excluded.last_visited)",
            params![
                entry.id.to_string(),
                entry.url,
                entry.title,
                entry.visit_count,
                entry.last_visited.to_rfc3339(),
                browser.trim_matches('"'),
                entry.duration_secs,
            ],
        )?;
        Ok(())
    }

    /// Bulk insert history entries
    pub fn insert_history(&self, entries: &[HistoryEntry]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut count = 0;
        for e in entries {
            self.upsert_history(e)?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    /// Search bookmarks using FTS5
    pub fn search_bookmarks(&self, query: &str) -> Result<Vec<Bookmark>> {
        // Use FTS5 match syntax - prefix search with *
        let fts_query = query
            .split_whitespace()
            .map(|w| format!("{w}*"))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = self.conn.prepare(
            "SELECT b.id, b.url, b.title, b.folder_path, b.tags, b.favicon_url,
                    b.source_browser, b.source_id, b.created_at, b.modified_at, b.synced_at
             FROM bookmarks b
             JOIN bookmarks_fts f ON b.rowid = f.rowid
             WHERE bookmarks_fts MATCH ?1
             ORDER BY rank
             LIMIT 100",
        )?;

        let results = stmt
            .query_map(params![fts_query], |row| Ok(row_to_bookmark(row)))?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        Ok(results)
    }

    /// Search history using FTS5
    pub fn search_history(&self, query: &str) -> Result<Vec<HistoryEntry>> {
        let fts_query = query
            .split_whitespace()
            .map(|w| format!("{w}*"))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = self.conn.prepare(
            "SELECT h.id, h.url, h.title, h.visit_count, h.last_visited,
                    h.source_browser, h.duration_secs
             FROM history h
             JOIN history_fts f ON h.rowid = f.rowid
             WHERE history_fts MATCH ?1
             ORDER BY rank
             LIMIT 100",
        )?;

        let results = stmt
            .query_map(params![fts_query], |row| Ok(row_to_history(row)))?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();

        Ok(results)
    }

    /// Get all bookmarks, optionally filtered by browser
    pub fn get_bookmarks(&self, browser: Option<Browser>) -> Result<Vec<Bookmark>> {
        let (sql, browser_filter) = match browser {
            Some(b) => {
                let name = serde_json::to_string(&b)?;
                (
                    "SELECT id, url, title, folder_path, tags, favicon_url,
                            source_browser, source_id, created_at, modified_at, synced_at
                     FROM bookmarks WHERE source_browser = ?1
                     ORDER BY folder_path, title",
                    Some(name.trim_matches('"').to_string()),
                )
            }
            None => (
                "SELECT id, url, title, folder_path, tags, favicon_url,
                        source_browser, source_id, created_at, modified_at, synced_at
                 FROM bookmarks
                 ORDER BY folder_path, title",
                None,
            ),
        };

        let mut stmt = self.conn.prepare(sql)?;

        let results = if let Some(filter) = &browser_filter {
            stmt.query_map(params![filter], |row| Ok(row_to_bookmark(row)))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map([], |row| Ok(row_to_bookmark(row)))?
                .filter_map(|r| r.ok())
                .collect()
        };

        Ok(results)
    }

    /// Get all history entries, optionally filtered by browser
    pub fn get_history(&self, browser: Option<Browser>, limit: usize) -> Result<Vec<HistoryEntry>> {
        let (sql, browser_filter) = match browser {
            Some(b) => {
                let name = serde_json::to_string(&b)?;
                (
                    format!(
                        "SELECT id, url, title, visit_count, last_visited,
                                source_browser, duration_secs
                         FROM history WHERE source_browser = ?1
                         ORDER BY last_visited DESC LIMIT {limit}"
                    ),
                    Some(name.trim_matches('"').to_string()),
                )
            }
            None => (
                format!(
                    "SELECT id, url, title, visit_count, last_visited,
                            source_browser, duration_secs
                     FROM history
                     ORDER BY last_visited DESC LIMIT {limit}"
                ),
                None,
            ),
        };

        let mut stmt = self.conn.prepare(&sql)?;

        let results = if let Some(filter) = &browser_filter {
            stmt.query_map(params![filter], |row| Ok(row_to_history(row)))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map([], |row| Ok(row_to_history(row)))?
                .filter_map(|r| r.ok())
                .collect()
        };

        Ok(results)
    }

    /// Record a sync operation
    pub fn log_sync(&self, browser: Browser, sync_type: &str, items: usize) -> Result<()> {
        let browser_name = serde_json::to_string(&browser)?;
        self.conn.execute(
            "INSERT INTO sync_log (browser, sync_type, items_synced, synced_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                browser_name.trim_matches('"'),
                sync_type,
                items as i64,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Get sync status summary
    pub fn sync_status(&self) -> Result<Vec<(String, String, i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT browser, sync_type, items_synced, synced_at
             FROM sync_log
             ORDER BY synced_at DESC
             LIMIT 20",
        )?;

        let results = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Count bookmarks and history entries
    pub fn counts(&self) -> Result<(usize, usize)> {
        let bookmarks: usize =
            self.conn
                .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))?;
        let history: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;
        Ok((bookmarks, history))
    }

    /// Delete all data from a specific browser
    pub fn clear_browser(&self, browser: Browser) -> Result<()> {
        let name = serde_json::to_string(&browser)?;
        let name = name.trim_matches('"');
        self.conn.execute(
            "DELETE FROM bookmarks WHERE source_browser = ?1",
            params![name],
        )?;
        self.conn.execute(
            "DELETE FROM history WHERE source_browser = ?1",
            params![name],
        )?;
        Ok(())
    }

    /// Save a URL summary
    pub fn save_summary(&self, url: &str, summary: &str, engine: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO summaries (url, summary, engine, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(url) DO UPDATE SET
                summary = excluded.summary,
                engine = excluded.engine,
                created_at = excluded.created_at",
            params![url, summary, engine, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Get summary for a URL
    pub fn get_summary(&self, url: &str) -> Result<Option<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT summary, engine FROM summaries WHERE url = ?1")?;
        let result = stmt
            .query_row(params![url], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .ok();
        Ok(result)
    }

    /// Get all summaries
    pub fn get_all_summaries(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT url, summary FROM summaries")?;
        let map = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(map)
    }

    /// Get URLs that have no summary yet
    pub fn get_unsummarized_urls(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT b.url, b.title FROM bookmarks b
             LEFT JOIN summaries s ON b.url = s.url
             WHERE s.url IS NULL
             AND b.url LIKE 'http%'
             LIMIT {limit}"
        ))?;
        let results = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(results)
    }
}

fn row_to_bookmark(row: &rusqlite::Row) -> Bookmark {
    let id: String = row.get(0).unwrap_or_default();
    let folder_path_str: String = row.get(3).unwrap_or_default();
    let tags_str: String = row.get(4).unwrap_or_default();
    let browser_str: String = row.get(6).unwrap_or_default();
    let created_str: String = row.get(8).unwrap_or_default();
    let modified_str: String = row.get(9).unwrap_or_default();
    let synced_str: String = row.get(10).unwrap_or_default();

    Bookmark {
        id: uuid::Uuid::parse_str(&id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
        url: row.get(1).unwrap_or_default(),
        title: row.get(2).unwrap_or_default(),
        folder_path: if folder_path_str.is_empty() {
            Vec::new()
        } else {
            folder_path_str.split('/').map(String::from).collect()
        },
        tags: if tags_str.is_empty() {
            Vec::new()
        } else {
            tags_str.split(',').map(String::from).collect()
        },
        favicon_url: row.get(5).unwrap_or(None),
        source_browser: serde_json::from_str(&format!("\"{browser_str}\""))
            .unwrap_or(Browser::Chrome),
        source_id: row.get(7).unwrap_or_default(),
        created_at: chrono::DateTime::parse_from_rfc3339(&created_str)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        modified_at: chrono::DateTime::parse_from_rfc3339(&modified_str)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        synced_at: chrono::DateTime::parse_from_rfc3339(&synced_str)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
    }
}

fn row_to_history(row: &rusqlite::Row) -> HistoryEntry {
    let id: String = row.get(0).unwrap_or_default();
    let browser_str: String = row.get(5).unwrap_or_default();
    let visited_str: String = row.get(4).unwrap_or_default();

    HistoryEntry {
        id: uuid::Uuid::parse_str(&id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
        url: row.get(1).unwrap_or_default(),
        title: row.get(2).unwrap_or_default(),
        visit_count: row.get(3).unwrap_or(1),
        last_visited: chrono::DateTime::parse_from_rfc3339(&visited_str)
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now()),
        source_browser: serde_json::from_str(&format!("\"{browser_str}\""))
            .unwrap_or(Browser::Chrome),
        duration_secs: row.get(6).unwrap_or(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn test_bookmark() -> Bookmark {
        Bookmark {
            id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            folder_path: vec!["Dev".to_string(), "Rust".to_string()],
            tags: vec!["rust".to_string(), "web".to_string()],
            favicon_url: None,
            source_browser: Browser::Chrome,
            source_id: "1".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            synced_at: Utc::now(),
        }
    }

    fn test_history() -> HistoryEntry {
        HistoryEntry {
            id: Uuid::new_v4(),
            url: "https://docs.rs".to_string(),
            title: "Docs.rs".to_string(),
            visit_count: 42,
            last_visited: Utc::now(),
            source_browser: Browser::Chrome,
            duration_secs: None,
        }
    }

    #[test]
    fn test_create_db() {
        let db = Database::open_memory().unwrap();
        let (bk, hist) = db.counts().unwrap();
        assert_eq!(bk, 0);
        assert_eq!(hist, 0);
    }

    #[test]
    fn test_insert_and_get_bookmark() {
        let db = Database::open_memory().unwrap();
        let bm = test_bookmark();
        db.upsert_bookmark(&bm).unwrap();

        let results = db.get_bookmarks(None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[0].title, "Example");
    }

    #[test]
    fn test_insert_and_get_history() {
        let db = Database::open_memory().unwrap();
        let entry = test_history();
        db.upsert_history(&entry).unwrap();

        let results = db.get_history(None, 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://docs.rs");
        assert_eq!(results[0].visit_count, 42);
    }

    #[test]
    fn test_bulk_insert() {
        let db = Database::open_memory().unwrap();
        let bookmarks: Vec<Bookmark> = (0..100)
            .map(|i| {
                let mut b = test_bookmark();
                b.id = Uuid::new_v4();
                b.url = format!("https://example.com/{i}");
                b.title = format!("Page {i}");
                b
            })
            .collect();

        let count = db.insert_bookmarks(&bookmarks).unwrap();
        assert_eq!(count, 100);

        let (bk, _) = db.counts().unwrap();
        assert_eq!(bk, 100);
    }

    #[test]
    fn test_search_bookmarks() {
        let db = Database::open_memory().unwrap();

        let mut b1 = test_bookmark();
        b1.title = "Rust Programming Language".to_string();
        b1.url = "https://rust-lang.org".to_string();
        db.upsert_bookmark(&b1).unwrap();

        let mut b2 = test_bookmark();
        b2.id = Uuid::new_v4();
        b2.title = "Python Tutorial".to_string();
        b2.url = "https://python.org".to_string();
        b2.tags = vec!["python".to_string()];
        b2.folder_path = vec!["Dev".to_string(), "Python".to_string()];
        db.upsert_bookmark(&b2).unwrap();

        let results = db.search_bookmarks("rust").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming Language");
    }

    #[test]
    fn test_filter_by_browser() {
        let db = Database::open_memory().unwrap();

        let mut b1 = test_bookmark();
        b1.source_browser = Browser::Chrome;
        db.upsert_bookmark(&b1).unwrap();

        let mut b2 = test_bookmark();
        b2.id = Uuid::new_v4();
        b2.source_browser = Browser::Firefox;
        b2.url = "https://firefox.com".to_string();
        db.upsert_bookmark(&b2).unwrap();

        let chrome = db.get_bookmarks(Some(Browser::Chrome)).unwrap();
        assert_eq!(chrome.len(), 1);

        let all = db.get_bookmarks(None).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_sync_log() {
        let db = Database::open_memory().unwrap();
        db.log_sync(Browser::Chrome, "bookmarks", 42).unwrap();
        db.log_sync(Browser::Chrome, "history", 100).unwrap();

        let status = db.sync_status().unwrap();
        assert_eq!(status.len(), 2);
    }
}
