use browsync_core::db::Database;
use browsync_core::detect;
use browsync_core::models::{Bookmark, Browser, HistoryEntry};
use browsync_core::parsers;
use serde::Serialize;

#[tauri::command]
pub fn ping() -> String {
    eprintln!("[browsync-app] ping received from frontend");
    "pong".to_string()
}

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

// ── Password viewing via Chrome Safe Storage + Keychain ──

#[tauri::command]
pub fn view_password(domain: String) -> Result<serde_json::Value, String> {
    // Step 1: Get Chrome Safe Storage encryption key from Keychain
    let key_output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "Chrome Safe Storage", "-w"])
        .output()
        .map_err(|e| format!("Keychain access failed: {e}"))?;

    if !key_output.status.success() {
        // Fallback: try macOS Keychain internet passwords (Safari, system)
        let output = std::process::Command::new("security")
            .args(["find-internet-password", "-s", &domain, "-w"])
            .output()
            .map_err(|e| format!("Keychain error: {e}"))?;

        if output.status.success() {
            let pw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(
                serde_json::json!({"domain": domain, "password": pw, "source": "macOS Keychain"}),
            );
        }
        return Err(format!(
            "Password for {domain} is encrypted in Chrome's vault. \
            Chrome uses its own encryption — export via chrome://settings/passwords or use a password manager."
        ));
    }

    // Step 2: Query Chrome's Login Data for the domain
    let chrome_login = dirs::home_dir()
        .ok_or("No home dir")?
        .join("Library/Application Support/Google/Chrome/Default/Login Data");

    if !chrome_login.exists() {
        return Err("Chrome Login Data not found.".to_string());
    }

    let temp = std::env::temp_dir().join("browsync_login_pw");
    std::fs::copy(&chrome_login, &temp).map_err(|e| e.to_string())?;

    let conn =
        rusqlite::Connection::open_with_flags(&temp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT origin_url, username_value, LENGTH(password_value) as pw_len
         FROM logins WHERE origin_url LIKE ?1 LIMIT 1",
        )
        .map_err(|e| e.to_string())?;

    let result = stmt.query_row(rusqlite::params![format!("%{domain}%")], |row| {
        let origin: String = row.get(0)?;
        let username: String = row.get(1)?;
        let pw_len: i32 = row.get(2)?;
        Ok((origin, username, pw_len))
    });

    let _ = std::fs::remove_file(&temp);

    match result {
        Ok((origin, username, pw_len)) => {
            if pw_len > 0 {
                Ok(serde_json::json!({
                    "domain": domain,
                    "password": format!("[encrypted - {} bytes]", pw_len),
                    "username": username,
                    "origin": origin,
                    "source": "Chrome (AES-128-CBC encrypted)",
                    "note": "Chrome passwords are encrypted with a key in macOS Keychain. Use chrome://settings/passwords to view, or export to a password manager."
                }))
            } else {
                Err(format!("No password stored for {domain}"))
            }
        }
        Err(_) => Err(format!("No saved login found for {domain} in Chrome.")),
    }
}

// ── Graph data for visualization ──

#[derive(Serialize)]
pub struct TimelinePoint {
    date: String,
    bookmarks: usize,
    history: usize,
}

#[derive(Serialize)]
pub struct DomainCount {
    domain: String,
    count: u32,
}

#[tauri::command]
pub fn get_graph_data() -> Result<serde_json::Value, String> {
    let db = db()?;

    // Top domains by visit count
    let bookmarks = db.get_bookmarks(None).map_err(|e| e.to_string())?;
    let history = db.get_history(None, 5000).map_err(|e| e.to_string())?;

    // Domain distribution from bookmarks
    let mut domain_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for b in &bookmarks {
        let domain = b
            .url
            .replace("https://", "")
            .replace("http://", "")
            .split('/')
            .next()
            .unwrap_or("")
            .to_string();
        if !domain.is_empty() {
            *domain_counts.entry(domain).or_insert(0) += 1;
        }
    }
    let mut top_domains: Vec<DomainCount> = domain_counts
        .into_iter()
        .map(|(domain, count)| DomainCount { domain, count })
        .collect();
    top_domains.sort_by(|a, b| b.count.cmp(&a.count));
    top_domains.truncate(15);

    // Top visited from history
    let mut visit_domains: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for h in &history {
        let domain = h
            .url
            .replace("https://", "")
            .replace("http://", "")
            .split('/')
            .next()
            .unwrap_or("")
            .to_string();
        if !domain.is_empty() {
            *visit_domains.entry(domain).or_insert(0) += h.visit_count;
        }
    }
    let mut top_visited: Vec<DomainCount> = visit_domains
        .into_iter()
        .map(|(domain, count)| DomainCount { domain, count })
        .collect();
    top_visited.sort_by(|a, b| b.count.cmp(&a.count));
    top_visited.truncate(15);

    // Bookmark creation timeline (by month)
    let mut timeline: std::collections::BTreeMap<String, (usize, usize)> =
        std::collections::BTreeMap::new();
    for b in &bookmarks {
        let month = b.created_at.format("%Y-%m").to_string();
        timeline.entry(month).or_insert((0, 0)).0 += 1;
    }
    for h in &history {
        let month = h.last_visited.format("%Y-%m").to_string();
        timeline.entry(month).or_insert((0, 0)).1 += 1;
    }
    let timeline_points: Vec<TimelinePoint> = timeline
        .into_iter()
        .map(|(date, (bk, hist))| TimelinePoint {
            date,
            bookmarks: bk,
            history: hist,
        })
        .collect();

    // Browser distribution
    let mut browser_dist: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for b in &bookmarks {
        let name = serde_json::to_string(&b.source_browser)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        *browser_dist.entry(name).or_insert(0) += 1;
    }

    Ok(serde_json::json!({
        "topDomains": top_domains,
        "topVisited": top_visited,
        "timeline": timeline_points,
        "browserDist": browser_dist,
        "totalBookmarks": bookmarks.len(),
        "totalHistory": history.len(),
    }))
}

/// 3D graph data: nodes (domains, folders, browsers) + links (bookmark relationships)
#[tauri::command]
pub fn get_3d_graph_data() -> Result<serde_json::Value, String> {
    let db = db()?;
    let bookmarks = db.get_bookmarks(None).map_err(|e| e.to_string())?;
    let history = db.get_history(None, 2000).map_err(|e| e.to_string())?;

    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut links: Vec<serde_json::Value> = Vec::new();
    let mut node_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Add browser nodes (center hubs)
    let mut browser_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for b in &bookmarks {
        let name = serde_json::to_string(&b.source_browser)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        browser_set.insert(name);
    }
    for name in &browser_set {
        let id = format!("browser:{}", name);
        nodes.push(serde_json::json!({"id": id, "label": name, "group": "browser", "size": 12}));
        node_ids.insert(id);
    }

    // Add folder nodes and domain nodes from bookmarks
    let mut domain_visits: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for h in &history {
        let domain = extract_domain_from_url(&h.url);
        *domain_visits.entry(domain).or_insert(0) += h.visit_count;
    }

    let mut domain_bookmark_count: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut folder_set: std::collections::HashSet<String> = std::collections::HashSet::new();

    for b in &bookmarks {
        let domain = extract_domain_from_url(&b.url);
        *domain_bookmark_count.entry(domain.clone()).or_insert(0) += 1;

        // Top-level folder
        if let Some(folder) = b.folder_path.get(0) {
            if !folder.is_empty() {
                folder_set.insert(folder.clone());
            }
        }
    }

    // Add folder nodes
    for folder in &folder_set {
        let id = format!("folder:{}", folder);
        if node_ids.insert(id.clone()) {
            nodes
                .push(serde_json::json!({"id": id, "label": folder, "group": "folder", "size": 6}));
        }
    }

    // Add top domain nodes (limit to top 80 by bookmark count)
    let mut sorted_domains: Vec<(String, u32)> = domain_bookmark_count.into_iter().collect();
    sorted_domains.sort_by(|a, b| b.1.cmp(&a.1));
    sorted_domains.truncate(80);

    for (domain, bk_count) in &sorted_domains {
        if domain.is_empty() {
            continue;
        }
        let visits = domain_visits.get(domain).copied().unwrap_or(0);
        let size = 2.0 + (*bk_count as f64).sqrt() * 1.5 + (visits as f64).sqrt() * 0.3;
        let id = format!("domain:{}", domain);
        if node_ids.insert(id.clone()) {
            nodes.push(serde_json::json!({
                "id": id, "label": domain, "group": "domain",
                "size": size, "bookmarks": bk_count, "visits": visits
            }));
        }
    }

    // Build links: browser -> folder, folder -> domain, browser -> domain
    for b in &bookmarks {
        let domain = extract_domain_from_url(&b.url);
        let domain_id = format!("domain:{}", domain);
        if !node_ids.contains(&domain_id) {
            continue;
        }

        let browser_name = serde_json::to_string(&b.source_browser)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        let browser_id = format!("browser:{}", browser_name);

        let folder = b.folder_path.get(0).cloned().unwrap_or_default();
        let folder_id = format!("folder:{}", folder);

        // Browser -> folder
        if !folder.is_empty() && node_ids.contains(&folder_id) {
            links.push(serde_json::json!({"source": browser_id, "target": folder_id}));
        }

        // Folder -> domain (or browser -> domain if no folder)
        if !folder.is_empty() && node_ids.contains(&folder_id) {
            links.push(serde_json::json!({"source": folder_id, "target": domain_id}));
        } else {
            links.push(serde_json::json!({"source": browser_id, "target": domain_id}));
        }
    }

    // Deduplicate links
    let mut seen_links: std::collections::HashSet<String> = std::collections::HashSet::new();
    let unique_links: Vec<serde_json::Value> = links
        .into_iter()
        .filter(|l| {
            let key = format!(
                "{}->{}",
                l["source"].as_str().unwrap_or(""),
                l["target"].as_str().unwrap_or("")
            );
            seen_links.insert(key)
        })
        .collect();

    Ok(serde_json::json!({
        "nodes": nodes,
        "links": unique_links,
    }))
}

/// Rich timeline data for 3D clustered graph
#[tauri::command]
pub fn get_3d_timeline_data() -> Result<serde_json::Value, String> {
    let db = db()?;
    let bookmarks = db.get_bookmarks(None).map_err(|e| e.to_string())?;
    let history = db.get_history(None, 5000).map_err(|e| e.to_string())?;
    let summaries = db.get_all_summaries().unwrap_or_default();

    // Group bookmarks by domain
    let mut domains: std::collections::HashMap<String, Vec<serde_json::Value>> =
        std::collections::HashMap::new();

    for b in &bookmarks {
        let domain = extract_domain_from_url(&b.url);
        if domain.is_empty() {
            continue;
        }
        let summary = summaries.get(&b.url).cloned();
        domains.entry(domain).or_default().push(serde_json::json!({
            "type": "bookmark",
            "url": b.url,
            "title": b.title,
            "date": b.created_at.to_rfc3339(),
            "timestamp": b.created_at.timestamp(),
            "browser": serde_json::to_string(&b.source_browser).unwrap_or_default().trim_matches('"'),
            "folder": b.folder_path.join("/"),
            "summary": summary,
        }));
    }

    // Add history visits to domains
    for h in &history {
        let domain = extract_domain_from_url(&h.url);
        if domain.is_empty() {
            continue;
        }
        let entry = domains.entry(domain).or_default();
        // Only add if not already a bookmark for same URL
        if !entry.iter().any(|e| e["url"].as_str() == Some(&h.url)) {
            entry.push(serde_json::json!({
                "type": "history",
                "url": h.url,
                "title": h.title,
                "date": h.last_visited.to_rfc3339(),
                "timestamp": h.last_visited.timestamp(),
                "visits": h.visit_count,
                "browser": serde_json::to_string(&h.source_browser).unwrap_or_default().trim_matches('"'),
                "summary": summaries.get(&h.url),
            }));
        }
    }

    // Build domain clusters sorted by total items
    let mut clusters: Vec<serde_json::Value> = domains
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(domain, mut items)| {
            items.sort_by(|a, b| {
                a["timestamp"]
                    .as_i64()
                    .unwrap_or(0)
                    .cmp(&b["timestamp"].as_i64().unwrap_or(0))
            });
            let first_ts = items
                .first()
                .and_then(|i| i["timestamp"].as_i64())
                .unwrap_or(0);
            let last_ts = items
                .last()
                .and_then(|i| i["timestamp"].as_i64())
                .unwrap_or(0);
            let bookmark_count = items.iter().filter(|i| i["type"] == "bookmark").count();
            let history_count = items.iter().filter(|i| i["type"] == "history").count();
            let total_visits: u64 = items.iter().filter_map(|i| i["visits"].as_u64()).sum();

            serde_json::json!({
                "domain": domain,
                "items": items,
                "count": items.len(),
                "bookmarks": bookmark_count,
                "history": history_count,
                "totalVisits": total_visits,
                "firstTimestamp": first_ts,
                "lastTimestamp": last_ts,
            })
        })
        .collect();

    clusters.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });

    // Keep top 100 domains
    clusters.truncate(100);

    // Global timeline bounds
    let min_ts = clusters
        .iter()
        .filter_map(|c| c["firstTimestamp"].as_i64())
        .min()
        .unwrap_or(0);
    let max_ts = clusters
        .iter()
        .filter_map(|c| c["lastTimestamp"].as_i64())
        .max()
        .unwrap_or(0);

    Ok(serde_json::json!({
        "clusters": clusters,
        "minTimestamp": min_ts,
        "maxTimestamp": max_ts,
    }))
}

fn extract_domain_from_url(url: &str) -> String {
    url.replace("https://", "")
        .replace("http://", "")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

// ── Bookmark summarization ──

/// Get cached summaries for display
#[tauri::command]
pub fn get_summaries() -> Result<serde_json::Value, String> {
    let db = db()?;
    let summaries = db.get_all_summaries().map_err(|e| e.to_string())?;
    Ok(serde_json::json!(summaries))
}

/// Summarize a single URL (on-demand)
#[tauri::command]
pub async fn summarize_url(url: String) -> Result<serde_json::Value, String> {
    // Check cache first
    let db = db()?;
    if let Ok(Some((summary, engine))) = db.get_summary(&url) {
        return Ok(
            serde_json::json!({"url": url, "summary": summary, "engine": engine, "cached": true}),
        );
    }

    let (summary, engine) = do_summarize(&url)?;
    db.save_summary(&url, &summary, &engine)
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"url": url, "summary": summary, "engine": engine, "cached": false}))
}

/// Background batch: summarize N unsummarized bookmarks
#[tauri::command]
pub async fn summarize_batch(count: Option<usize>) -> Result<serde_json::Value, String> {
    let db = db()?;
    let limit = count.unwrap_or(10);
    let urls = db.get_unsummarized_urls(limit).map_err(|e| e.to_string())?;

    let mut done = 0;
    let mut failed = 0;
    for (url, _title) in &urls {
        match do_summarize(url) {
            Ok((summary, engine)) => {
                let _ = db.save_summary(url, &summary, &engine);
                done += 1;
            }
            Err(_) => {
                failed += 1;
            }
        }
    }

    let total_summaries: usize = db.get_all_summaries().map(|m| m.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "processed": done,
        "failed": failed,
        "remaining": urls.len().saturating_sub(done),
        "totalSummaries": total_summaries,
    }))
}

fn do_summarize(url: &str) -> Result<(String, String), String> {
    let html = fetch_page(url).map_err(|e| format!("Fetch: {e}"))?;
    let text = extract_text(&html);
    if text.len() < 50 {
        return Err("Too little content".to_string());
    }

    let truncated = if text.len() > 4000 {
        &text[..4000]
    } else {
        &text
    };

    // Try Ollama first
    match call_ollama(truncated) {
        Ok(s) => Ok((s, "ollama".to_string())),
        Err(_) => {
            // Extractive fallback
            let sentences: Vec<&str> = text.split(". ").filter(|s| s.len() > 20).take(3).collect();
            let summary = if sentences.is_empty() {
                text.chars().take(200).collect::<String>() + "..."
            } else {
                sentences.join(". ") + "."
            };
            Ok((summary, "extractive".to_string()))
        }
    }
}

fn fetch_page(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "--max-time", "10",
            "--compressed",
            "-H", "Accept: text/html,application/xhtml+xml",
            "-H", "Accept-Language: en-US,en;q=0.9",
            "-A", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            url,
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!("HTTP error for {url}").into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn extract_text(html: &str) -> String {
    // Strategy: use meta description + title if available (best quality),
    // then fall back to article/main content, skip nav/footer/header junk.

    let lower = html.to_lowercase();

    // 1. Extract <meta name="description" content="...">
    let meta_desc = extract_meta_content(&lower, html, "description");

    // 2. Extract <meta property="og:description" content="...">
    let og_desc = extract_meta_content(&lower, html, "og:description");

    // 3. Extract <title>
    let title = extract_between(&lower, html, "<title", "</title>");

    // 4. Try to get clean body text from <article> or <main>
    let article = extract_between(&lower, html, "<article", "</article>")
        .or_else(|| extract_between(&lower, html, "<main", "</main>"));

    // Build the best summary source
    let mut parts: Vec<String> = Vec::new();

    if let Some(t) = &title {
        let clean = strip_tags(t);
        if clean.len() > 5 {
            parts.push(clean);
        }
    }

    // Prefer meta description (human-written summary on most sites)
    if let Some(desc) = meta_desc.or(og_desc) {
        if desc.len() > 20 {
            parts.push(desc);
            // Meta description is usually good enough
            return parts.join(". ");
        }
    }

    // Fall back to article/main content, or full body
    let body_html = article.unwrap_or_else(|| html.to_string());

    // Remove junk blocks
    let mut clean = body_html;
    for tag in &[
        "script", "style", "noscript", "svg", "nav", "header", "footer", "aside", "form", "iframe",
    ] {
        loop {
            let open = format!("<{}", tag);
            let close = format!("</{}>", tag);
            let lc = clean.to_lowercase();
            if let Some(start) = lc.find(&open) {
                if let Some(end) = lc[start..].find(&close) {
                    clean = format!(
                        "{} {}",
                        &clean[..start],
                        &clean[start + end + close.len()..]
                    );
                } else {
                    clean = clean[..start].to_string();
                    break;
                }
            } else {
                break;
            }
        }
    }

    let body_text = strip_tags(&clean);

    // Filter out junk sentences
    let sentences: Vec<&str> = body_text
        .split(". ")
        .filter(|s| {
            let sl = s.to_lowercase();
            s.len() > 30
                && !sl.contains("skip to")
                && !sl.contains("accept cookie")
                && !sl.contains("toggle")
                && !sl.contains("sign in")
                && !sl.contains("log in")
                && !sl.contains("menu")
                && !sl.contains("navigation")
                && !sl.starts_with("click")
                && !sl.starts_with("home ")
                && s.chars().filter(|c| c.is_alphabetic()).count() > s.len() / 3
        })
        .take(4)
        .collect();

    if !sentences.is_empty() {
        parts.push(sentences.join(". "));
    }

    let result = parts.join(". ");
    if result.len() > 500 {
        result[..500].to_string() + "..."
    } else {
        result
    }
}

fn extract_meta_content(lower: &str, original: &str, name: &str) -> Option<String> {
    // Match <meta name="description" content="..."> or <meta property="og:..." content="...">
    let patterns = [
        format!("name=\"{}\"", name),
        format!("property=\"{}\"", name),
        format!("name='{}'", name),
        format!("property='{}'", name),
    ];

    for pat in &patterns {
        if let Some(meta_pos) = lower.find(pat.as_str()) {
            // Find the content= attribute near this position
            let search_range =
                &lower[meta_pos.saturating_sub(200)..lower.len().min(meta_pos + 500)];
            if let Some(content_pos) = search_range.find("content=\"") {
                let start = content_pos + 9;
                if let Some(end) = search_range[start..].find('"') {
                    let orig_start = meta_pos.saturating_sub(200) + start;
                    let orig_end = orig_start + end;
                    if orig_end <= original.len() {
                        let content = &original[orig_start..orig_end];
                        let decoded = content
                            .replace("&amp;", "&")
                            .replace("&lt;", "<")
                            .replace("&gt;", ">")
                            .replace("&quot;", "\"")
                            .replace("&#39;", "'");
                        if decoded.len() > 10 {
                            return Some(decoded);
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_between(lower: &str, original: &str, open_tag: &str, close_tag: &str) -> Option<String> {
    if let Some(start) = lower.find(open_tag) {
        // Skip past the opening tag's >
        if let Some(gt) = lower[start..].find('>') {
            let content_start = start + gt + 1;
            if let Some(end) = lower[content_start..].find(close_tag) {
                return Some(original[content_start..content_start + end].to_string());
            }
        }
    }
    None
}

fn strip_tags(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            text.push(' ');
            continue;
        }
        if !in_tag {
            text.push(c);
        }
    }
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn call_ollama(text: &str) -> Result<String, String> {
    let prompt =
        format!("Summarize the following web page content in 2-3 concise sentences:\n\n{text}");

    let body = serde_json::json!({
        "model": "qwen2.5:0.5b",
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.3,
            "num_predict": 150
        }
    });

    let output = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "30",
            "-X",
            "POST",
            "http://localhost:11434/api/generate",
            "-H",
            "Content-Type: application/json",
            "-d",
            &body.to_string(),
        ])
        .output()
        .map_err(|e| format!("curl failed: {e}"))?;

    if !output.status.success() {
        return Err(
            "Ollama not running or model not available. Run: ollama pull qwen2.5:0.5b".to_string(),
        );
    }

    let resp: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Parse error: {e}"))?;

    resp["response"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "No response from Ollama".to_string())
}

// ── Theme management ──

#[tauri::command]
pub fn save_settings(settings_json: String) -> Result<(), String> {
    let path = dirs::home_dir()
        .ok_or("No home dir")?
        .join(".browsync")
        .join("settings.json");
    std::fs::write(&path, &settings_json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_settings() -> Result<String, String> {
    let path = dirs::home_dir()
        .ok_or("No home dir")?
        .join(".browsync")
        .join("settings.json");
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(_) => Ok("{}".to_string()),
    }
}
