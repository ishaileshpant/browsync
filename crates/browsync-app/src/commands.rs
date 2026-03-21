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

// ── Password viewing with biometric auth ──

#[tauri::command]
pub fn view_password(domain: String) -> Result<serde_json::Value, String> {
    // Use macOS security CLI with user prompt (triggers Touch ID / password dialog)
    let output = std::process::Command::new("security")
        .args([
            "find-internet-password",
            "-s",
            &domain,
            "-w", // output password only
        ])
        .output()
        .map_err(|e| format!("Failed to access Keychain: {e}"))?;

    if output.status.success() {
        let password = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(serde_json::json!({
            "domain": domain,
            "password": password,
            "source": "keychain"
        }))
    } else {
        // Try generic password
        let output2 = std::process::Command::new("security")
            .args(["find-generic-password", "-s", &domain, "-w"])
            .output()
            .map_err(|e| format!("Keychain error: {e}"))?;

        if output2.status.success() {
            let password = String::from_utf8_lossy(&output2.stdout).trim().to_string();
            Ok(serde_json::json!({
                "domain": domain,
                "password": password,
                "source": "keychain"
            }))
        } else {
            Err(format!(
                "No Keychain entry found for {domain}. Password may be stored in browser's encrypted vault."
            ))
        }
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

// ── Bookmark summarization via Ollama ──

#[tauri::command]
pub async fn summarize_url(url: String) -> Result<serde_json::Value, String> {
    // Step 1: Fetch page content
    let html = fetch_page(&url).map_err(|e| format!("Fetch failed: {e}"))?;

    // Step 2: Extract text (simple HTML strip)
    let text = extract_text(&html);
    if text.len() < 50 {
        return Err("Page has too little content to summarize.".to_string());
    }

    // Step 3: Try Ollama (localhost:11434)
    let truncated = if text.len() > 4000 {
        &text[..4000]
    } else {
        &text
    };

    match call_ollama(truncated).await {
        Ok(summary) => Ok(serde_json::json!({
            "url": url,
            "summary": summary,
            "engine": "ollama",
            "textLength": text.len(),
        })),
        Err(ollama_err) => {
            // Fallback: extractive summary (first 3 sentences)
            let sentences: Vec<&str> = text.split(". ").filter(|s| s.len() > 20).take(3).collect();
            let summary = if sentences.is_empty() {
                text.chars().take(200).collect::<String>() + "..."
            } else {
                sentences.join(". ") + "."
            };
            Ok(serde_json::json!({
                "url": url,
                "summary": summary,
                "engine": "extractive",
                "note": format!("Ollama unavailable ({ollama_err}). Using extractive summary."),
                "textLength": text.len(),
            }))
        }
    }
}

fn fetch_page(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-A",
            "Mozilla/5.0 (compatible; browsync/0.1)",
            url,
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!("HTTP error for {url}").into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn extract_text(html: &str) -> String {
    // Simple HTML tag stripper
    let mut text = String::new();
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    for c in html.chars() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            // Check for script/style end
            let lower = text.to_lowercase();
            if lower.ends_with("script") || lower.ends_with("style") {
                // crude but works for stripping
            }
            continue;
        }
        if in_tag {
            // Check tag names
            let tag_check = html.to_lowercase();
            if tag_check.contains("<script") {
                in_script = true;
            }
            if tag_check.contains("</script") {
                in_script = false;
            }
            if tag_check.contains("<style") {
                in_style = true;
            }
            if tag_check.contains("</style") {
                in_style = false;
            }
            continue;
        }
        if !in_script && !in_style {
            text.push(c);
        }
    }

    // Clean up whitespace
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

async fn call_ollama(text: &str) -> Result<String, String> {
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
