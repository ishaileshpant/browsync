use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::detect::DetectedBrowser;
use crate::models::{Bookmark, Browser, HistoryEntry};

use super::BrowserParser;

pub struct SafariParser {
    bookmarks_path: Option<PathBuf>,
    history_path: Option<PathBuf>,
}

impl SafariParser {
    pub fn new(detected: &DetectedBrowser) -> Result<Self> {
        Ok(Self {
            bookmarks_path: detected.bookmarks_path.clone(),
            history_path: detected.history_path.clone(),
        })
    }
}

/// Safari plist bookmark structures
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SafariBookmarkRoot {
    children: Option<Vec<SafariBookmarkNode>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SafariBookmarkNode {
    #[serde(rename = "WebBookmarkType")]
    web_bookmark_type: String,
    #[serde(rename = "URLString")]
    url_string: Option<String>,
    title: Option<String>,
    children: Option<Vec<SafariBookmarkNode>>,
    #[serde(rename = "URIDictionary")]
    uri_dictionary: Option<SafariUriDict>,
}

#[derive(Debug, serde::Deserialize)]
struct SafariUriDict {
    title: Option<String>,
}

fn flatten_safari_bookmarks(
    node: &SafariBookmarkNode,
    folder_path: &[String],
    out: &mut Vec<Bookmark>,
) {
    match node.web_bookmark_type.as_str() {
        "WebBookmarkTypeLeaf" => {
            if let Some(url) = &node.url_string {
                let title = node
                    .uri_dictionary
                    .as_ref()
                    .and_then(|d| d.title.clone())
                    .or_else(|| node.title.clone())
                    .unwrap_or_default();

                out.push(Bookmark {
                    id: Uuid::new_v4(),
                    url: url.clone(),
                    title,
                    folder_path: folder_path.to_vec(),
                    tags: Vec::new(),
                    favicon_url: None,
                    source_browser: Browser::Safari,
                    source_id: String::new(),
                    created_at: Utc::now(),
                    modified_at: Utc::now(),
                    synced_at: Utc::now(),
                });
            }
        }
        "WebBookmarkTypeList" => {
            let mut path = folder_path.to_vec();
            if let Some(title) = &node.title
                && !title.is_empty() {
                    path.push(title.clone());
                }
            if let Some(children) = &node.children {
                for child in children {
                    flatten_safari_bookmarks(child, &path, out);
                }
            }
        }
        _ => {}
    }
}

/// Convert Safari/Core Data timestamp (seconds since 2001-01-01) to DateTime<Utc>
fn safari_timestamp(secs: f64) -> Option<DateTime<Utc>> {
    if secs == 0.0 {
        return None;
    }
    // Core Data epoch is 2001-01-01 00:00:00 UTC
    let unix_secs = secs + 978_307_200.0;
    DateTime::from_timestamp(unix_secs as i64, 0)
}

impl BrowserParser for SafariParser {
    fn parse_bookmarks(&self) -> Result<Vec<Bookmark>> {
        let path = self
            .bookmarks_path
            .as_ref()
            .context("No Safari Bookmarks.plist found")?;

        let file = std::fs::File::open(path)
            .with_context(|| format!("Opening Safari bookmarks at {}", path.display()))?;

        let root: SafariBookmarkRoot =
            plist::from_reader(file).context("Parsing Safari Bookmarks.plist")?;

        let mut bookmarks = Vec::new();
        if let Some(children) = &root.children {
            for child in children {
                flatten_safari_bookmarks(child, &[], &mut bookmarks);
            }
        }

        Ok(bookmarks)
    }

    fn parse_history(&self) -> Result<Vec<HistoryEntry>> {
        let path = self
            .history_path
            .as_ref()
            .context("No Safari History.db found")?;

        // Copy since Safari may lock it
        let temp_path = std::env::temp_dir().join("browsync_safari_history.db");
        std::fs::copy(path, &temp_path)
            .with_context(|| "Copying Safari History.db (may need Full Disk Access)")?;

        let conn = rusqlite::Connection::open_with_flags(
            &temp_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .context("Opening Safari history database")?;

        let mut stmt = conn.prepare(
            "SELECT hi.url, hv.title, hi.visit_count,
                    hv.visit_time
             FROM history_items hi
             LEFT JOIN history_visits hv ON hi.id = hv.history_item
             WHERE hi.url IS NOT NULL
             ORDER BY hv.visit_time DESC
             LIMIT 10000",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let url: String = row.get(0)?;
                let title: Option<String> = row.get(1)?;
                let visit_count: u32 = row.get(2)?;
                let visit_time: f64 = row.get(3)?;
                Ok((url, title, visit_count, visit_time))
            })?
            .filter_map(|r| r.ok())
            .map(|(url, title, visit_count, visit_time)| {
                let last_visited = safari_timestamp(visit_time).unwrap_or_else(Utc::now);
                HistoryEntry {
                    id: Uuid::new_v4(),
                    url,
                    title: title.unwrap_or_default(),
                    visit_count,
                    last_visited,
                    source_browser: Browser::Safari,
                    duration_secs: None,
                }
            })
            .collect();

        let _ = std::fs::remove_file(&temp_path);
        Ok(entries)
    }
}
