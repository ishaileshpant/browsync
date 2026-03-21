use browsync_core::db::Database;
use browsync_core::models::{Bookmark, Browser, HistoryEntry};
use browsync_core::sync::{dedup_bookmarks, dedup_history, MergeStrategy};
use chrono::Utc;
use uuid::Uuid;

// ── Helpers ────────────────────────────────────────────

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

fn make_history(url: &str, title: &str, visits: u32, browser: Browser) -> HistoryEntry {
    HistoryEntry {
        id: Uuid::new_v4(),
        url: url.to_string(),
        title: title.to_string(),
        visit_count: visits,
        last_visited: Utc::now(),
        source_browser: browser,
        duration_secs: None,
    }
}

// ── Integration Tests ──────────────────────────────────

#[test]
fn test_full_import_search_export_roundtrip() {
    let db = Database::open_memory().unwrap();

    // Import bookmarks from multiple "browsers"
    let bookmarks = vec![
        make_bookmark("https://github.com", "GitHub", Browser::Chrome),
        make_bookmark("https://docs.rs", "Docs.rs", Browser::Chrome),
        make_bookmark("https://crates.io", "Crates.io", Browser::Firefox),
        make_bookmark("https://github.com", "GitHub Mirror", Browser::Firefox), // dup URL
    ];

    let count = db.insert_bookmarks(&bookmarks).unwrap();
    assert_eq!(count, 4);

    // Import history
    let history = vec![
        make_history("https://github.com", "GitHub", 100, Browser::Chrome),
        make_history("https://stackoverflow.com", "SO", 50, Browser::Chrome),
        make_history("https://docs.rs", "Docs", 25, Browser::Firefox),
    ];

    let count = db.insert_history(&history).unwrap();
    assert_eq!(count, 3);

    // Search bookmarks
    let results = db.search_bookmarks("github").unwrap();
    assert!(results.len() >= 1);

    // Search history
    let results = db.search_history("stack").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://stackoverflow.com");

    // Filter by browser
    let chrome_bm = db.get_bookmarks(Some(Browser::Chrome)).unwrap();
    assert_eq!(chrome_bm.len(), 2);

    let firefox_bm = db.get_bookmarks(Some(Browser::Firefox)).unwrap();
    assert_eq!(firefox_bm.len(), 2);

    // Export
    let all = db.get_bookmarks(None).unwrap();
    let html = browsync_core::exporters::html::export(&all);
    assert!(html.contains("NETSCAPE-Bookmark-file-1"));
    assert!(html.contains("github.com"));

    let json = browsync_core::exporters::json::export_browsync(&all).unwrap();
    let parsed: Vec<Bookmark> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), all.len());

    let csv = browsync_core::exporters::csv::export_bookmarks(&all);
    assert!(csv.starts_with("url,title,"));
    // CSV should have header + n data lines
    assert_eq!(csv.lines().count(), all.len() + 1);
}

#[test]
fn test_dedup_across_browsers() {
    let bookmarks = vec![
        make_bookmark("https://github.com", "GitHub", Browser::Chrome),
        make_bookmark("https://github.com", "GitHub - Code", Browser::Firefox),
        make_bookmark("https://github.com", "GitHub Website", Browser::Safari),
        make_bookmark("https://docs.rs", "Docs", Browser::Chrome),
    ];

    let deduped = dedup_bookmarks(&bookmarks, MergeStrategy::LastWriteWins);
    assert_eq!(deduped.len(), 2); // github.com deduped to 1

    let history = vec![
        make_history("https://github.com", "GH", 50, Browser::Chrome),
        make_history("https://github.com", "GH", 30, Browser::Firefox),
        make_history("https://docs.rs", "Docs", 10, Browser::Chrome),
    ];

    let deduped_h = dedup_history(&history);
    assert_eq!(deduped_h.len(), 2);
    let gh = deduped_h.iter().find(|h| h.url == "https://github.com").unwrap();
    assert_eq!(gh.visit_count, 80); // Summed
}

#[test]
fn test_sync_log_tracking() {
    let db = Database::open_memory().unwrap();

    db.log_sync(Browser::Chrome, "bookmarks", 100).unwrap();
    db.log_sync(Browser::Chrome, "history", 500).unwrap();
    db.log_sync(Browser::Firefox, "bookmarks", 50).unwrap();

    let log = db.sync_status().unwrap();
    assert_eq!(log.len(), 3);
}

#[test]
fn test_clear_browser_data() {
    let db = Database::open_memory().unwrap();

    let bms = vec![
        make_bookmark("https://a.com", "A", Browser::Chrome),
        make_bookmark("https://b.com", "B", Browser::Firefox),
    ];
    db.insert_bookmarks(&bms).unwrap();

    let hist = vec![
        make_history("https://a.com", "A", 1, Browser::Chrome),
        make_history("https://b.com", "B", 1, Browser::Firefox),
    ];
    db.insert_history(&hist).unwrap();

    // Clear Chrome data
    db.clear_browser(Browser::Chrome).unwrap();

    let (bk, h) = db.counts().unwrap();
    assert_eq!(bk, 1); // Only Firefox bookmark remains
    assert_eq!(h, 1);  // Only Firefox history remains
}

#[test]
fn test_browser_detection() {
    let browsers = browsync_core::detect::detect_all();
    assert_eq!(browsers.len(), 6); // Always returns all 6

    // At least one should be installed on this macOS system
    let installed = browsers.iter().filter(|b| b.is_installed).count();
    assert!(installed >= 1, "At least Safari should be installed");
}

#[test]
fn test_large_bookmark_insert() {
    let db = Database::open_memory().unwrap();

    let bookmarks: Vec<Bookmark> = (0..5000)
        .map(|i| make_bookmark(
            &format!("https://example.com/page/{i}"),
            &format!("Page {i}"),
            if i % 2 == 0 { Browser::Chrome } else { Browser::Firefox },
        ))
        .collect();

    let count = db.insert_bookmarks(&bookmarks).unwrap();
    assert_eq!(count, 5000);

    let (bk, _) = db.counts().unwrap();
    assert_eq!(bk, 5000);

    // Search should still work
    let results = db.search_bookmarks("Page 42").unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_fts_prefix_search() {
    let db = Database::open_memory().unwrap();

    let bms = vec![
        make_bookmark("https://kubernetes.io", "Kubernetes Documentation", Browser::Chrome),
        make_bookmark("https://k8s.io", "K8s Quick Start", Browser::Chrome),
        make_bookmark("https://python.org", "Python Language", Browser::Chrome),
    ];
    db.insert_bookmarks(&bms).unwrap();

    // Prefix search
    let results = db.search_bookmarks("kube").unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].title.contains("Kubernetes"));

    // Multi-word search
    let results = db.search_bookmarks("python lang").unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_history_ordering() {
    let db = Database::open_memory().unwrap();

    let h1 = HistoryEntry {
        id: Uuid::new_v4(),
        url: "https://old.com".to_string(),
        title: "Old".to_string(),
        visit_count: 1,
        last_visited: chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        source_browser: Browser::Chrome,
        duration_secs: None,
    };

    let h2 = HistoryEntry {
        id: Uuid::new_v4(),
        url: "https://new.com".to_string(),
        title: "New".to_string(),
        visit_count: 1,
        last_visited: chrono::DateTime::parse_from_rfc3339("2025-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        source_browser: Browser::Chrome,
        duration_secs: None,
    };

    db.insert_history(&[h1, h2]).unwrap();

    let results = db.get_history(None, 10).unwrap();
    assert_eq!(results.len(), 2);
    // Most recent first
    assert_eq!(results[0].url, "https://new.com");
    assert_eq!(results[1].url, "https://old.com");
}

#[test]
fn test_export_html_roundtrip_structure() {
    let bookmarks = vec![
        make_bookmark("https://a.com", "Site A", Browser::Chrome),
        make_bookmark("https://b.com", "Site B", Browser::Firefox),
    ];

    let html = browsync_core::exporters::html::export(&bookmarks);

    // Valid Netscape format
    assert!(html.contains("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
    assert!(html.contains("<DL><p>"));
    assert!(html.contains("</DL><p>"));
    assert!(html.contains("ADD_DATE="));
    assert!(html.contains("https://a.com"));
    assert!(html.contains("https://b.com"));
}

#[test]
fn test_csv_export_with_special_chars() {
    let bm = Bookmark {
        id: Uuid::new_v4(),
        url: "https://example.com?foo=1&bar=2".to_string(),
        title: "Test, \"quoted\" & <special>".to_string(),
        folder_path: vec!["A/B".to_string()],
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        favicon_url: None,
        source_browser: Browser::Chrome,
        source_id: String::new(),
        created_at: Utc::now(),
        modified_at: Utc::now(),
        synced_at: Utc::now(),
    };

    let csv = browsync_core::exporters::csv::export_bookmarks(&[bm]);
    // Title with comma and quotes should be properly escaped
    assert!(csv.contains("\"Test, \"\"quoted\"\" & <special>\""));
}

#[test]
fn test_auth_migration_report() {
    use browsync_core::keychain::{migration_report, MigrationStatus};
    use browsync_core::models::AuthEntry;

    let entries = vec![
        AuthEntry {
            id: Uuid::new_v4(),
            domain: "github.com".to_string(),
            username: "user1".to_string(),
            source_browser: Browser::Chrome,
            last_used: None,
            password_manager: None,
        },
        AuthEntry {
            id: Uuid::new_v4(),
            domain: "gitlab.com".to_string(),
            username: "user1".to_string(),
            source_browser: Browser::Chrome,
            last_used: None,
            password_manager: None,
        },
        AuthEntry {
            id: Uuid::new_v4(),
            domain: "github.com".to_string(),
            username: "user1".to_string(),
            source_browser: Browser::Firefox,
            last_used: None,
            password_manager: None,
        },
    ];

    let report = migration_report(&entries, Browser::Chrome, Browser::Firefox);
    assert_eq!(report.len(), 2);

    let github = report.iter().find(|r| r.domain == "github.com").unwrap();
    assert_eq!(github.status, MigrationStatus::AlreadySaved);

    let gitlab = report.iter().find(|r| r.domain == "gitlab.com").unwrap();
    assert_eq!(gitlab.status, MigrationStatus::NeedsLogin);
}

#[test]
fn test_browser_from_str() {
    let chrome: Browser = "chrome".parse().unwrap();
    assert_eq!(chrome, Browser::Chrome);

    let firefox: Browser = "Firefox".parse().unwrap();
    assert_eq!(firefox, Browser::Firefox);

    let edge: Browser = "Microsoft Edge".parse().unwrap();
    assert_eq!(edge, Browser::Edge);

    assert!("unknown".parse::<Browser>().is_err());
}

#[test]
fn test_union_merge_preserves_all_tags() {
    let mut b1 = make_bookmark("https://example.com", "Ex", Browser::Chrome);
    b1.tags = vec!["rust".to_string(), "web".to_string()];

    let mut b2 = make_bookmark("https://example.com", "Ex", Browser::Firefox);
    b2.tags = vec!["dev".to_string(), "rust".to_string()]; // "rust" is duplicate

    let deduped = dedup_bookmarks(&[b1, b2], MergeStrategy::UnionMerge);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].tags.len(), 3); // rust, web, dev (deduped)
    assert!(deduped[0].tags.contains(&"rust".to_string()));
    assert!(deduped[0].tags.contains(&"web".to_string()));
    assert!(deduped[0].tags.contains(&"dev".to_string()));
}
