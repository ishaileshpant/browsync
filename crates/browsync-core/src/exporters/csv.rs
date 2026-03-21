use crate::models::{Bookmark, HistoryEntry};

/// Export bookmarks as CSV with proper escaping
pub fn export_bookmarks(bookmarks: &[Bookmark]) -> String {
    let mut csv = String::from("url,title,folder,browser,tags,created\n");
    for bm in bookmarks {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_escape(&bm.url),
            csv_escape(&bm.title),
            csv_escape(&bm.folder_path.join("/")),
            bm.source_browser,
            csv_escape(&bm.tags.join(";")),
            bm.created_at.format("%Y-%m-%d"),
        ));
    }
    csv
}

/// Export history as CSV
pub fn export_history(entries: &[HistoryEntry]) -> String {
    let mut csv = String::from("url,title,visit_count,last_visited,browser\n");
    for entry in entries {
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_escape(&entry.url),
            csv_escape(&entry.title),
            entry.visit_count,
            entry.last_visited.format("%Y-%m-%dT%H:%M:%S"),
            entry.source_browser,
        ));
    }
    csv
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_csv_escape_commas() {
        assert_eq!(csv_escape("hello, world"), "\"hello, world\"");
        assert_eq!(csv_escape("simple"), "simple");
    }

    #[test]
    fn test_csv_escape_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_export_bookmarks_csv() {
        let bm = Bookmark {
            id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
            title: "Example, Inc".to_string(),
            folder_path: vec!["Dev".to_string()],
            tags: vec!["test".to_string()],
            favicon_url: None,
            source_browser: crate::models::Browser::Chrome,
            source_id: String::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            synced_at: Utc::now(),
        };

        let csv = export_bookmarks(&[bm]);
        assert!(csv.starts_with("url,title,"));
        // Title with comma should be quoted
        assert!(csv.contains("\"Example, Inc\""));
    }

    #[test]
    fn test_export_history_csv() {
        let entry = HistoryEntry {
            id: Uuid::new_v4(),
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            visit_count: 5,
            last_visited: Utc::now(),
            source_browser: crate::models::Browser::Firefox,
            duration_secs: None,
        };

        let csv = export_history(&[entry]);
        assert!(csv.starts_with("url,title,visit_count,"));
        assert!(csv.contains("Firefox"));
    }
}
