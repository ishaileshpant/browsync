use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::detect::DetectedBrowser;
use crate::models::{Bookmark, Browser, HistoryEntry};

use super::BrowserParser;

/// Chrome/Chromium bookmark JSON structures
#[derive(Debug, Deserialize)]
struct ChromeBookmarkFile {
    roots: ChromeRoots,
}

#[derive(Debug, Deserialize)]
struct ChromeRoots {
    bookmark_bar: ChromeNode,
    other: ChromeNode,
    synced: Option<ChromeNode>,
}

#[derive(Debug, Deserialize)]
struct ChromeNode {
    #[serde(default)]
    children: Vec<ChromeNode>,
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    url: Option<String>,
    date_added: Option<String>,
    date_last_used: Option<String>,
    id: Option<String>,
}

pub struct ChromeParser {
    bookmarks_path: Option<PathBuf>,
    history_path: Option<PathBuf>,
    browser: Browser,
}

impl ChromeParser {
    pub fn new(detected: &DetectedBrowser) -> Result<Self> {
        Ok(Self {
            bookmarks_path: detected.bookmarks_path.clone(),
            history_path: detected.history_path.clone(),
            browser: detected.browser,
        })
    }
}

/// Convert Chrome's WebKit timestamp (microseconds since 1601-01-01) to DateTime<Utc>
fn chrome_timestamp_to_utc(ts: &str) -> Option<DateTime<Utc>> {
    let micros: i64 = ts.parse().ok()?;
    if micros == 0 {
        return None;
    }
    // Chrome epoch is 1601-01-01. Unix epoch is 1970-01-01.
    // Difference: 11644473600 seconds
    let unix_micros = micros - 11_644_473_600_000_000;
    DateTime::from_timestamp_micros(unix_micros)
}

/// Convert Chrome's History SQLite timestamp to DateTime<Utc>
/// History uses the same WebKit timestamp format
fn chrome_history_timestamp(micros: i64) -> Option<DateTime<Utc>> {
    if micros == 0 {
        return None;
    }
    let unix_micros = micros - 11_644_473_600_000_000;
    DateTime::from_timestamp_micros(unix_micros)
}

fn flatten_bookmarks(
    node: &ChromeNode,
    folder_path: &[String],
    browser: Browser,
    out: &mut Vec<Bookmark>,
) {
    if node.node_type == "url" {
        if let Some(url) = &node.url {
            // Skip internal chrome:// URLs
            if url.starts_with("chrome://") || url.starts_with("chrome-extension://") {
                return;
            }

            let created_at = node
                .date_added
                .as_deref()
                .and_then(chrome_timestamp_to_utc)
                .unwrap_or_else(Utc::now);

            let modified_at = node
                .date_last_used
                .as_deref()
                .and_then(chrome_timestamp_to_utc)
                .unwrap_or(created_at);

            out.push(Bookmark {
                id: Uuid::new_v4(),
                url: url.clone(),
                title: node.name.clone(),
                folder_path: folder_path.to_vec(),
                tags: Vec::new(),
                favicon_url: None,
                source_browser: browser,
                source_id: node.id.clone().unwrap_or_default(),
                created_at,
                modified_at,
                synced_at: Utc::now(),
            });
        }
    } else if node.node_type == "folder" {
        let mut path = folder_path.to_vec();
        if !node.name.is_empty() {
            path.push(node.name.clone());
        }
        for child in &node.children {
            flatten_bookmarks(child, &path, browser, out);
        }
    }
}

impl BrowserParser for ChromeParser {
    fn parse_bookmarks(&self) -> Result<Vec<Bookmark>> {
        let path = self
            .bookmarks_path
            .as_ref()
            .context("No bookmarks file found")?;

        let content =
            std::fs::read_to_string(path).with_context(|| format!("Reading {}", path.display()))?;

        let file: ChromeBookmarkFile =
            serde_json::from_str(&content).context("Parsing Chrome bookmarks JSON")?;

        let mut bookmarks = Vec::new();
        flatten_bookmarks(
            &file.roots.bookmark_bar,
            &["Bookmark Bar".to_string()],
            self.browser,
            &mut bookmarks,
        );
        flatten_bookmarks(
            &file.roots.other,
            &["Other Bookmarks".to_string()],
            self.browser,
            &mut bookmarks,
        );
        if let Some(synced) = &file.roots.synced {
            flatten_bookmarks(
                synced,
                &["Synced".to_string()],
                self.browser,
                &mut bookmarks,
            );
        }

        Ok(bookmarks)
    }

    fn parse_history(&self) -> Result<Vec<HistoryEntry>> {
        let path = self
            .history_path
            .as_ref()
            .context("No history file found")?;

        // Chrome locks the History file while running. Copy it first.
        let temp_path = std::env::temp_dir().join(format!("browsync_history_{}", self.browser));
        std::fs::copy(path, &temp_path)
            .with_context(|| format!("Copying {} history (browser may be locked)", self.browser))?;

        let conn = rusqlite::Connection::open_with_flags(
            &temp_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .context("Opening history database")?;

        let mut stmt = conn.prepare(
            "SELECT url, title, visit_count, last_visit_time
             FROM urls
             ORDER BY last_visit_time DESC
             LIMIT 10000",
        )?;

        let entries = stmt
            .query_map([], |row| {
                let url: String = row.get(0)?;
                let title: String = row.get(1)?;
                let visit_count: u32 = row.get(2)?;
                let last_visit_micros: i64 = row.get(3)?;
                Ok((url, title, visit_count, last_visit_micros))
            })?
            .filter_map(|r| r.ok())
            .filter(|(url, _, _, _)| {
                !url.starts_with("chrome://") && !url.starts_with("chrome-extension://")
            })
            .map(|(url, title, visit_count, last_visit_micros)| {
                let last_visited =
                    chrome_history_timestamp(last_visit_micros).unwrap_or_else(Utc::now);
                HistoryEntry {
                    id: Uuid::new_v4(),
                    url,
                    title,
                    visit_count,
                    last_visited,
                    source_browser: self.browser,
                    duration_secs: None,
                }
            })
            .collect();

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_timestamp_conversion() {
        // 2024-01-01 00:00:00 UTC in Chrome format
        // Chrome epoch: 1601-01-01, offset = 11644473600 seconds = 11644473600000000 microseconds
        // 2024-01-01 00:00:00 UTC = 1704067200 unix seconds = 1704067200000000 unix micros
        // Chrome micros = 1704067200000000 + 11644473600000000 = 13348540800000000
        let ts = "13348540800000000";
        let dt = chrome_timestamp_to_utc(ts).unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-01");
    }

    #[test]
    fn test_chrome_timestamp_zero() {
        assert!(chrome_timestamp_to_utc("0").is_none());
    }

    #[test]
    fn test_parse_bookmarks_json() {
        let json = r#"{
            "checksum": "abc",
            "roots": {
                "bookmark_bar": {
                    "children": [
                        {
                            "date_added": "13349961600000000",
                            "id": "1",
                            "name": "Test Bookmark",
                            "type": "url",
                            "url": "https://example.com"
                        },
                        {
                            "children": [
                                {
                                    "date_added": "13349961600000000",
                                    "id": "2",
                                    "name": "Nested",
                                    "type": "url",
                                    "url": "https://nested.example.com"
                                }
                            ],
                            "name": "Folder",
                            "type": "folder"
                        }
                    ],
                    "name": "Bookmark Bar",
                    "type": "folder"
                },
                "other": {
                    "children": [],
                    "name": "Other",
                    "type": "folder"
                }
            }
        }"#;

        let file: ChromeBookmarkFile = serde_json::from_str(json).unwrap();
        let mut bookmarks = Vec::new();
        flatten_bookmarks(
            &file.roots.bookmark_bar,
            &["Bookmark Bar".to_string()],
            Browser::Chrome,
            &mut bookmarks,
        );

        assert_eq!(bookmarks.len(), 2);
        assert_eq!(bookmarks[0].title, "Test Bookmark");
        assert_eq!(bookmarks[0].url, "https://example.com");
        assert_eq!(
            bookmarks[0].folder_path,
            vec!["Bookmark Bar".to_string(), "Bookmark Bar".to_string()]
        );

        assert_eq!(bookmarks[1].title, "Nested");
        assert_eq!(
            bookmarks[1].folder_path,
            vec![
                "Bookmark Bar".to_string(),
                "Bookmark Bar".to_string(),
                "Folder".to_string()
            ]
        );
    }
}
