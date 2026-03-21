use anyhow::Result;
use serde::Serialize;

use crate::models::Bookmark;

/// Chrome-compatible bookmark JSON format
#[derive(Serialize)]
struct ChromeBookmarkFile {
    checksum: String,
    roots: ChromeRoots,
    version: u32,
}

#[derive(Serialize)]
struct ChromeRoots {
    bookmark_bar: ChromeNode,
    other: ChromeNode,
    synced: ChromeNode,
}

#[derive(Serialize)]
struct ChromeNode {
    children: Vec<ChromeNode>,
    date_added: String,
    date_last_used: String,
    id: String,
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

/// Export bookmarks as browsync JSON (full metadata)
pub fn export_browsync(bookmarks: &[Bookmark]) -> Result<String> {
    Ok(serde_json::to_string_pretty(bookmarks)?)
}

/// Export bookmarks as Chrome-compatible JSON
pub fn export_chrome_format(bookmarks: &[Bookmark]) -> Result<String> {
    let mut bar_children = Vec::new();
    let mut other_children = Vec::new();
    let mut id_counter = 1u64;

    for bm in bookmarks {
        let node = ChromeNode {
            children: vec![],
            date_added: datetime_to_chrome(bm.created_at),
            date_last_used: datetime_to_chrome(bm.modified_at),
            id: {
                id_counter += 1;
                id_counter.to_string()
            },
            name: bm.title.clone(),
            node_type: "url".to_string(),
            url: Some(bm.url.clone()),
        };

        let is_bar = bm
            .folder_path
            .first()
            .map(|f| f.contains("Bar") || f.contains("bar") || f.contains("Toolbar"))
            .unwrap_or(false);

        if is_bar {
            bar_children.push(node);
        } else {
            other_children.push(node);
        }
    }

    let file = ChromeBookmarkFile {
        checksum: String::new(),
        roots: ChromeRoots {
            bookmark_bar: ChromeNode {
                children: bar_children,
                date_added: "0".to_string(),
                date_last_used: "0".to_string(),
                id: "1".to_string(),
                name: "Bookmarks bar".to_string(),
                node_type: "folder".to_string(),
                url: None,
            },
            other: ChromeNode {
                children: other_children,
                date_added: "0".to_string(),
                date_last_used: "0".to_string(),
                id: "2".to_string(),
                name: "Other bookmarks".to_string(),
                node_type: "folder".to_string(),
                url: None,
            },
            synced: ChromeNode {
                children: vec![],
                date_added: "0".to_string(),
                date_last_used: "0".to_string(),
                id: "3".to_string(),
                name: "Mobile bookmarks".to_string(),
                node_type: "folder".to_string(),
                url: None,
            },
        },
        version: 1,
    };

    Ok(serde_json::to_string_pretty(&file)?)
}

fn datetime_to_chrome(dt: chrono::DateTime<chrono::Utc>) -> String {
    let unix_micros = dt.timestamp_micros();
    let chrome_micros = unix_micros + 11_644_473_600_000_000;
    chrome_micros.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_bm(url: &str, title: &str) -> Bookmark {
        Bookmark {
            id: Uuid::new_v4(),
            url: url.to_string(),
            title: title.to_string(),
            folder_path: vec!["Bookmark Bar".to_string()],
            tags: vec![],
            favicon_url: None,
            source_browser: crate::models::Browser::Chrome,
            source_id: String::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            synced_at: Utc::now(),
        }
    }

    #[test]
    fn test_export_browsync_json() {
        let bms = vec![make_bm("https://example.com", "Example")];
        let json = export_browsync(&bms).unwrap();
        assert!(json.contains("example.com"));
        // Should be valid JSON
        let _: Vec<Bookmark> = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_export_chrome_format() {
        let bms = vec![
            make_bm("https://example.com", "Example"),
            make_bm("https://rust-lang.org", "Rust"),
        ];
        let json = export_chrome_format(&bms).unwrap();
        assert!(json.contains("bookmark_bar"));
        assert!(json.contains("example.com"));
    }
}
