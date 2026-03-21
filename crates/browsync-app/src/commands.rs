use browsync_core::db::Database;
use browsync_core::detect;
use browsync_core::models::{Bookmark, Browser, HistoryEntry};
use browsync_core::parsers;
use serde::Serialize;

#[derive(Serialize)]
pub struct BrowserInfo {
    name: String,
    short_code: String,
    is_installed: bool,
    has_data: bool,
    has_bookmarks: bool,
    has_history: bool,
    has_logins: bool,
}

#[derive(Serialize)]
pub struct Stats {
    bookmark_count: usize,
    history_count: usize,
}

#[derive(Serialize)]
pub struct SyncEntry {
    browser: String,
    sync_type: String,
    items: i64,
    time: String,
}

#[derive(Serialize)]
pub struct ImportResult {
    browser: String,
    bookmarks: usize,
    history: usize,
}

#[derive(Serialize)]
pub struct SearchResults {
    bookmarks: Vec<Bookmark>,
    history: Vec<HistoryEntry>,
}

fn db() -> Result<Database, String> {
    Database::open_default().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn detect_browsers() -> Result<Vec<BrowserInfo>, String> {
    let browsers = detect::detect_all();
    Ok(browsers
        .iter()
        .map(|b| BrowserInfo {
            name: b.browser.display_name().to_string(),
            short_code: b.browser.short_code().to_string(),
            is_installed: b.is_installed,
            has_data: b.has_data,
            has_bookmarks: b.bookmarks_path.is_some(),
            has_history: b.history_path.is_some(),
            has_logins: b.login_data_path.is_some(),
        })
        .collect())
}

#[tauri::command]
pub fn import_browser(browser_name: String) -> Result<ImportResult, String> {
    let browser: Browser = browser_name
        .parse()
        .map_err(|e: anyhow::Error| e.to_string())?;
    let detected = detect::detect_one(browser).map_err(|e| e.to_string())?;

    if !detected.has_data {
        return Err(format!("{} has no importable data", browser));
    }

    let db = db()?;
    let parser = parsers::parser_for(&detected).map_err(|e| e.to_string())?;

    let bk_count = match parser.parse_bookmarks() {
        Ok(bms) => {
            let c = db.insert_bookmarks(&bms).map_err(|e| e.to_string())?;
            let _ = db.log_sync(browser, "bookmarks", c);
            c
        }
        Err(_) => 0,
    };

    let h_count = match parser.parse_history() {
        Ok(hist) => {
            let c = db.insert_history(&hist).map_err(|e| e.to_string())?;
            let _ = db.log_sync(browser, "history", c);
            c
        }
        Err(_) => 0,
    };

    Ok(ImportResult {
        browser: browser.display_name().to_string(),
        bookmarks: bk_count,
        history: h_count,
    })
}

#[tauri::command]
pub fn import_all() -> Result<Vec<ImportResult>, String> {
    let browsers = detect::detect_with_data();
    let mut results = Vec::new();
    for detected in &browsers {
        let db = db()?;
        let parser = parsers::parser_for(detected).map_err(|e| e.to_string())?;

        let bk = parser
            .parse_bookmarks()
            .ok()
            .map(|b| {
                let c = db.insert_bookmarks(&b).unwrap_or(0);
                let _ = db.log_sync(detected.browser, "bookmarks", c);
                c
            })
            .unwrap_or(0);

        let h = parser
            .parse_history()
            .ok()
            .map(|h| {
                let c = db.insert_history(&h).unwrap_or(0);
                let _ = db.log_sync(detected.browser, "history", c);
                c
            })
            .unwrap_or(0);

        results.push(ImportResult {
            browser: detected.browser.display_name().to_string(),
            bookmarks: bk,
            history: h,
        });
    }
    Ok(results)
}

#[tauri::command]
pub fn search_all(query: String) -> Result<SearchResults, String> {
    let db = db()?;
    let bookmarks = db.search_bookmarks(&query).map_err(|e| e.to_string())?;
    let history = db.search_history(&query).map_err(|e| e.to_string())?;
    Ok(SearchResults { bookmarks, history })
}

#[tauri::command]
pub fn get_bookmarks(browser_filter: Option<String>) -> Result<Vec<Bookmark>, String> {
    let db = db()?;
    let browser = browser_filter
        .map(|s| s.parse::<Browser>())
        .transpose()
        .map_err(|e: anyhow::Error| e.to_string())?;
    db.get_bookmarks(browser).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_history(
    limit: Option<usize>,
    browser_filter: Option<String>,
) -> Result<Vec<HistoryEntry>, String> {
    let db = db()?;
    let browser = browser_filter
        .map(|s| s.parse::<Browser>())
        .transpose()
        .map_err(|e: anyhow::Error| e.to_string())?;
    db.get_history(browser, limit.unwrap_or(200))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stats() -> Result<Stats, String> {
    let db = db()?;
    let (bookmark_count, history_count) = db.counts().map_err(|e| e.to_string())?;
    Ok(Stats {
        bookmark_count,
        history_count,
    })
}

#[tauri::command]
pub fn get_sync_log() -> Result<Vec<SyncEntry>, String> {
    let db = db()?;
    let log = db.sync_status().map_err(|e| e.to_string())?;
    Ok(log
        .into_iter()
        .map(|(browser, sync_type, items, time)| SyncEntry {
            browser,
            sync_type,
            items,
            time,
        })
        .collect())
}

#[tauri::command]
pub fn export_bookmarks(
    format: String,
    path: String,
    browser_filter: Option<String>,
) -> Result<String, String> {
    let db = db()?;
    let browser = browser_filter
        .map(|s| s.parse::<Browser>())
        .transpose()
        .map_err(|e: anyhow::Error| e.to_string())?;
    let bookmarks = db.get_bookmarks(browser).map_err(|e| e.to_string())?;

    let content = match format.as_str() {
        "html" => browsync_core::exporters::html::export(&bookmarks),
        "json" => browsync_core::exporters::json::export_browsync(&bookmarks)
            .map_err(|e| e.to_string())?,
        "csv" => browsync_core::exporters::csv::export_bookmarks(&bookmarks),
        _ => return Err(format!("Unknown format: {format}")),
    };

    std::fs::write(&path, &content).map_err(|e| e.to_string())?;
    Ok(format!("Exported {} bookmarks to {path}", bookmarks.len()))
}

#[tauri::command]
pub fn open_url(url: String, browser_name: Option<String>) -> Result<(), String> {
    let browser: Browser = browser_name
        .unwrap_or_else(|| "chrome".to_string())
        .parse()
        .map_err(|e: anyhow::Error| e.to_string())?;

    std::process::Command::new("open")
        .args(["-a", browser.open_command(), &url])
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_auth_entries() -> Result<Vec<serde_json::Value>, String> {
    let browsers = detect::detect_with_data();
    let mut entries = Vec::new();

    for detected in &browsers {
        if let Some(login_path) = &detected.login_data_path {
            if let Ok(auths) =
                browsync_core::keychain::extract_chrome_auth(login_path, detected.browser)
            {
                for auth in auths {
                    entries.push(serde_json::json!({
                        "domain": auth.domain,
                        "username": auth.username,
                        "browser": detected.browser.display_name(),
                    }));
                }
            }
        }
    }

    entries.sort_by(|a, b| {
        a["domain"]
            .as_str()
            .unwrap_or("")
            .cmp(b["domain"].as_str().unwrap_or(""))
    });
    entries.dedup_by(|a, b| a["domain"] == b["domain"] && a["browser"] == b["browser"]);
    Ok(entries)
}

#[tauri::command]
pub fn delete_browser_data(browser_name: String) -> Result<String, String> {
    let browser: Browser = browser_name
        .parse()
        .map_err(|e: anyhow::Error| e.to_string())?;
    let db = db()?;
    db.clear_browser(browser).map_err(|e| e.to_string())?;
    Ok(format!("Cleared all {} data", browser))
}
