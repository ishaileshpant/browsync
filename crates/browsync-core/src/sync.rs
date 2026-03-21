use anyhow::Result;
use std::collections::HashMap;

use crate::db::Database;
use crate::models::{Bookmark, Browser, HistoryEntry};

/// Deduplication strategy for merging browser data
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Keep the most recently modified version
    LastWriteWins,
    /// Merge tags and folders from all sources
    UnionMerge,
}

/// Merge result stats
#[derive(Debug, Default)]
pub struct MergeStats {
    pub new_items: usize,
    pub updated_items: usize,
    pub duplicates_merged: usize,
}

impl std::fmt::Display for MergeStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} new, {} updated, {} duplicates merged",
            self.new_items, self.updated_items, self.duplicates_merged
        )
    }
}

/// Deduplicate bookmarks by URL, merging metadata from multiple browsers
pub fn dedup_bookmarks(bookmarks: &[Bookmark], strategy: MergeStrategy) -> Vec<Bookmark> {
    let mut by_url: HashMap<String, Vec<&Bookmark>> = HashMap::new();

    for bm in bookmarks {
        by_url.entry(bm.url.clone()).or_default().push(bm);
    }

    by_url
        .into_values()
        .map(|group| merge_bookmark_group(&group, strategy))
        .collect()
}

fn merge_bookmark_group(group: &[&Bookmark], strategy: MergeStrategy) -> Bookmark {
    if group.len() == 1 {
        return group[0].clone();
    }

    match strategy {
        MergeStrategy::LastWriteWins => {
            let mut best = group[0].clone();
            for bm in &group[1..] {
                if bm.modified_at > best.modified_at {
                    best = (*bm).clone();
                }
            }
            best
        }
        MergeStrategy::UnionMerge => {
            // Start with the most recently modified
            let mut best = group[0].clone();
            for bm in &group[1..] {
                if bm.modified_at > best.modified_at {
                    best = (*bm).clone();
                }
            }

            // Union all tags
            let mut all_tags: Vec<String> = group
                .iter()
                .flat_map(|bm| bm.tags.iter().cloned())
                .collect();
            all_tags.sort();
            all_tags.dedup();
            best.tags = all_tags;

            best
        }
    }
}

/// Deduplicate history entries by URL, summing visit counts
pub fn dedup_history(entries: &[HistoryEntry]) -> Vec<HistoryEntry> {
    let mut by_url: HashMap<String, Vec<&HistoryEntry>> = HashMap::new();

    for entry in entries {
        by_url.entry(entry.url.clone()).or_default().push(entry);
    }

    by_url
        .into_values()
        .map(|group| merge_history_group(&group))
        .collect()
}

fn merge_history_group(group: &[&HistoryEntry]) -> HistoryEntry {
    if group.len() == 1 {
        return group[0].clone();
    }

    let mut best = group[0].clone();
    let mut total_visits: u32 = 0;

    for entry in group {
        total_visits = total_visits.saturating_add(entry.visit_count);
        if entry.last_visited > best.last_visited {
            best = (*entry).clone();
        }
    }

    best.visit_count = total_visits;
    best
}

/// Import with dedup: merge incoming data with existing DB contents
pub fn import_with_dedup(
    db: &Database,
    bookmarks: Vec<Bookmark>,
    history: Vec<HistoryEntry>,
    browser: Browser,
    strategy: MergeStrategy,
) -> Result<MergeStats> {
    let mut stats = MergeStats::default();

    // Dedup incoming bookmarks
    let deduped_bookmarks = dedup_bookmarks(&bookmarks, strategy);
    stats.duplicates_merged += bookmarks.len() - deduped_bookmarks.len();
    stats.new_items += deduped_bookmarks.len();

    db.insert_bookmarks(&deduped_bookmarks)?;
    db.log_sync(browser, "bookmarks", deduped_bookmarks.len())?;

    // Dedup incoming history
    let deduped_history = dedup_history(&history);
    stats.duplicates_merged += history.len() - deduped_history.len();
    stats.new_items += deduped_history.len();

    db.insert_history(&deduped_history)?;
    db.log_sync(browser, "history", deduped_history.len())?;

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_bookmark(url: &str, title: &str, browser: Browser) -> Bookmark {
        Bookmark {
            id: Uuid::new_v4(),
            url: url.to_string(),
            title: title.to_string(),
            folder_path: vec!["Test".to_string()],
            tags: vec![],
            favicon_url: None,
            source_browser: browser,
            source_id: String::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            synced_at: Utc::now(),
        }
    }

    fn make_history(url: &str, visits: u32, browser: Browser) -> HistoryEntry {
        HistoryEntry {
            id: Uuid::new_v4(),
            url: url.to_string(),
            title: "Test".to_string(),
            visit_count: visits,
            last_visited: Utc::now(),
            source_browser: browser,
            duration_secs: None,
        }
    }

    #[test]
    fn test_dedup_bookmarks_same_url() {
        let bookmarks = vec![
            make_bookmark("https://example.com", "Example", Browser::Chrome),
            make_bookmark("https://example.com", "Example Site", Browser::Firefox),
            make_bookmark("https://other.com", "Other", Browser::Chrome),
        ];

        let deduped = dedup_bookmarks(&bookmarks, MergeStrategy::LastWriteWins);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_dedup_bookmarks_union_tags() {
        let mut b1 = make_bookmark("https://example.com", "Example", Browser::Chrome);
        b1.tags = vec!["rust".to_string()];

        let mut b2 = make_bookmark("https://example.com", "Example", Browser::Firefox);
        b2.tags = vec!["web".to_string(), "dev".to_string()];

        let deduped = dedup_bookmarks(&[b1, b2], MergeStrategy::UnionMerge);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].tags.len(), 3);
    }

    #[test]
    fn test_dedup_history_sums_visits() {
        let entries = vec![
            make_history("https://example.com", 10, Browser::Chrome),
            make_history("https://example.com", 5, Browser::Firefox),
            make_history("https://other.com", 3, Browser::Chrome),
        ];

        let deduped = dedup_history(&entries);
        assert_eq!(deduped.len(), 2);

        let example = deduped
            .iter()
            .find(|e| e.url == "https://example.com")
            .unwrap();
        assert_eq!(example.visit_count, 15);
    }

    #[test]
    fn test_import_with_dedup() {
        let db = Database::open_memory().unwrap();
        let bookmarks = vec![
            make_bookmark("https://example.com", "Example", Browser::Chrome),
            make_bookmark("https://example.com", "Example Dup", Browser::Chrome),
            make_bookmark("https://other.com", "Other", Browser::Chrome),
        ];
        let history = vec![
            make_history("https://example.com", 5, Browser::Chrome),
            make_history("https://example.com", 3, Browser::Chrome),
        ];

        let stats =
            import_with_dedup(&db, bookmarks, history, Browser::Chrome, MergeStrategy::LastWriteWins)
                .unwrap();

        assert_eq!(stats.duplicates_merged, 2); // 1 bookmark dup + 1 history dup
        let (bk, hist) = db.counts().unwrap();
        assert_eq!(bk, 2);
        assert_eq!(hist, 1);
    }
}
