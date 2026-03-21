use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::detect::DetectedBrowser;
use crate::models::{Bookmark, Browser, HistoryEntry};

use super::BrowserParser;

pub struct FirefoxParser {
    places_path: Option<PathBuf>,
}

impl FirefoxParser {
    pub fn new(detected: &DetectedBrowser) -> Result<Self> {
        Ok(Self {
            places_path: detected.bookmarks_path.clone(),
        })
    }

    /// Open places.sqlite (copy first since Firefox locks it)
    fn open_places(&self) -> Result<rusqlite::Connection> {
        let path = self
            .places_path
            .as_ref()
            .context("No places.sqlite found")?;

        let temp_path = std::env::temp_dir().join("browsync_firefox_places.sqlite");
        std::fs::copy(path, &temp_path)
            .with_context(|| "Copying Firefox places.sqlite (browser may be locked)".to_string())?;

        let conn = rusqlite::Connection::open_with_flags(
            &temp_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .context("Opening Firefox places database")?;

        Ok(conn)
    }
}

/// Convert Firefox timestamp (microseconds since Unix epoch) to DateTime<Utc>
fn firefox_timestamp(micros: i64) -> Option<DateTime<Utc>> {
    if micros == 0 {
        return None;
    }
    DateTime::from_timestamp_micros(micros)
}

impl BrowserParser for FirefoxParser {
    fn parse_bookmarks(&self) -> Result<Vec<Bookmark>> {
        let conn = self.open_places()?;

        // Firefox stores bookmarks in moz_bookmarks with URLs in moz_places
        // Folders have type=2, bookmarks have type=1
        let mut stmt = conn.prepare(
            "SELECT b.id, b.title, p.url, b.dateAdded, b.lastModified, b.parent,
                    (SELECT GROUP_CONCAT(pb.title, '/')
                     FROM moz_bookmarks pb
                     WHERE pb.id IN (
                         WITH RECURSIVE ancestors(id, parent) AS (
                             SELECT b2.id, b2.parent FROM moz_bookmarks b2 WHERE b2.id = b.parent
                             UNION ALL
                             SELECT a.id, b3.parent FROM ancestors a
                             JOIN moz_bookmarks b3 ON b3.id = a.parent
                             WHERE b3.parent IS NOT NULL AND b3.parent != 0
                         )
                         SELECT id FROM ancestors
                     )
                     AND pb.title IS NOT NULL
                    ) as folder_path
             FROM moz_bookmarks b
             JOIN moz_places p ON b.fk = p.id
             WHERE b.type = 1
             AND p.url NOT LIKE 'place:%'",
        )?;

        let bookmarks = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let title: Option<String> = row.get(1)?;
                let url: String = row.get(2)?;
                let date_added: i64 = row.get(3)?;
                let last_modified: i64 = row.get(4)?;
                let folder_path: Option<String> = row.get(6)?;
                Ok((id, title, url, date_added, last_modified, folder_path))
            })?
            .filter_map(|r| r.ok())
            .map(|(id, title, url, date_added, last_modified, folder_path)| {
                let created_at = firefox_timestamp(date_added).unwrap_or_else(Utc::now);
                let modified_at = firefox_timestamp(last_modified).unwrap_or(created_at);
                let path: Vec<String> = folder_path
                    .map(|p| p.split('/').rev().map(String::from).collect())
                    .unwrap_or_default();

                Bookmark {
                    id: Uuid::new_v4(),
                    url,
                    title: title.unwrap_or_default(),
                    folder_path: path,
                    tags: Vec::new(),
                    favicon_url: None,
                    source_browser: Browser::Firefox,
                    source_id: id.to_string(),
                    created_at,
                    modified_at,
                    synced_at: Utc::now(),
                }
            })
            .collect();

        Ok(bookmarks)
    }

    fn parse_history(&self) -> Result<Vec<HistoryEntry>> {
        let conn = self.open_places()?;

        let mut stmt = conn.prepare(
            "SELECT url, title, visit_count, last_visit_date
             FROM moz_places
             WHERE visit_count > 0
             AND url NOT LIKE 'place:%'
             ORDER BY last_visit_date DESC
             LIMIT 10000",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let url: String = row.get(0)?;
                let title: Option<String> = row.get(1)?;
                let visit_count: u32 = row.get(2)?;
                let last_visit: i64 = row.get(3)?;
                Ok((url, title, visit_count, last_visit))
            })?
            .filter_map(|r| r.ok())
            .map(|(url, title, visit_count, last_visit)| {
                let last_visited = firefox_timestamp(last_visit).unwrap_or_else(Utc::now);
                HistoryEntry {
                    id: Uuid::new_v4(),
                    url,
                    title: title.unwrap_or_default(),
                    visit_count,
                    last_visited,
                    source_browser: Browser::Firefox,
                    duration_secs: None,
                }
            })
            .collect();

        Ok(entries)
    }
}
