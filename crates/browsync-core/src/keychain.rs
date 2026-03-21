use anyhow::{Context, Result};
use std::path::Path;

use crate::models::{AuthEntry, Browser};

/// Extract auth metadata (domain + username only, never passwords) from Chrome's Login Data
pub fn extract_chrome_auth(login_data_path: &Path, browser: Browser) -> Result<Vec<AuthEntry>> {
    // Copy since Chrome locks the file
    let temp_path = std::env::temp_dir().join(format!("browsync_login_{browser}"));
    std::fs::copy(login_data_path, &temp_path)
        .with_context(|| format!("Copying {browser} Login Data"))?;

    let conn = rusqlite::Connection::open_with_flags(
        &temp_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .context("Opening Login Data database")?;

    let mut stmt = conn.prepare(
        "SELECT origin_url, username_value, date_last_used
         FROM logins
         WHERE blacklisted_by_user = 0",
    )?;

    let entries = stmt
        .query_map([], |row| {
            let origin: String = row.get(0)?;
            let username: String = row.get(1)?;
            let date_last_used: i64 = row.get(2)?;
            Ok((origin, username, date_last_used))
        })?
        .filter_map(|r| r.ok())
        .map(|(origin, username, date_last_used)| {
            let domain = extract_domain(&origin);
            let last_used = if date_last_used > 0 {
                // Chrome WebKit timestamp
                let unix_micros = date_last_used - 11_644_473_600_000_000;
                chrono::DateTime::from_timestamp_micros(unix_micros)
            } else {
                None
            };

            AuthEntry {
                id: uuid::Uuid::new_v4(),
                domain,
                username,
                source_browser: browser,
                last_used,
                password_manager: Some("browser".to_string()),
            }
        })
        .collect();

    let _ = std::fs::remove_file(&temp_path);
    Ok(entries)
}

/// Extract domain from a URL
fn extract_domain(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

/// Check if 1Password CLI is available
pub fn has_onepassword_cli() -> bool {
    std::process::Command::new("op")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Check if Bitwarden CLI is available
pub fn has_bitwarden_cli() -> bool {
    std::process::Command::new("bw")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Get auth domains from 1Password CLI (requires sign-in)
pub fn onepassword_domains() -> Result<Vec<String>> {
    let output = std::process::Command::new("op")
        .args(["item", "list", "--format", "json"])
        .output()
        .context("Running 1Password CLI")?;

    if !output.status.success() {
        anyhow::bail!(
            "1Password CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let items: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let domains: Vec<String> = items
        .iter()
        .filter_map(|item| {
            item.get("urls")
                .and_then(|urls| urls.as_array())
                .and_then(|urls| urls.first())
                .and_then(|url| url.get("href"))
                .and_then(|href| href.as_str())
                .map(|href| extract_domain(href))
        })
        .collect();

    Ok(domains)
}

/// Generate auth migration report: sites that need re-login when switching browsers
pub fn migration_report(
    entries: &[AuthEntry],
    from: Browser,
    to: Browser,
) -> Vec<AuthMigrationItem> {
    let from_domains: std::collections::HashSet<String> = entries
        .iter()
        .filter(|e| e.source_browser == from)
        .map(|e| e.domain.clone())
        .collect();

    let to_domains: std::collections::HashSet<String> = entries
        .iter()
        .filter(|e| e.source_browser == to)
        .map(|e| e.domain.clone())
        .collect();

    let mut report = Vec::new();

    for domain in &from_domains {
        let status = if to_domains.contains(domain) {
            MigrationStatus::AlreadySaved
        } else {
            MigrationStatus::NeedsLogin
        };

        let username = entries
            .iter()
            .find(|e| e.domain == *domain && e.source_browser == from)
            .map(|e| e.username.clone())
            .unwrap_or_default();

        report.push(AuthMigrationItem {
            domain: domain.clone(),
            username,
            status,
        });
    }

    report.sort_by(|a, b| a.domain.cmp(&b.domain));
    report
}

#[derive(Debug)]
pub struct AuthMigrationItem {
    pub domain: String,
    pub username: String,
    pub status: MigrationStatus,
}

#[derive(Debug, PartialEq)]
pub enum MigrationStatus {
    AlreadySaved,
    NeedsLogin,
}

impl std::fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationStatus::AlreadySaved => write!(f, "OK"),
            MigrationStatus::NeedsLogin => write!(f, "NEEDS LOGIN"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("https://github.com/login"), "github.com");
        assert_eq!(extract_domain("http://example.com"), "example.com");
        assert_eq!(
            extract_domain("https://accounts.google.com/signin"),
            "accounts.google.com"
        );
    }

    #[test]
    fn test_migration_report() {
        let entries = vec![
            AuthEntry {
                id: uuid::Uuid::new_v4(),
                domain: "github.com".to_string(),
                username: "user".to_string(),
                source_browser: Browser::Chrome,
                last_used: None,
                password_manager: None,
            },
            AuthEntry {
                id: uuid::Uuid::new_v4(),
                domain: "gitlab.com".to_string(),
                username: "user".to_string(),
                source_browser: Browser::Chrome,
                last_used: None,
                password_manager: None,
            },
            AuthEntry {
                id: uuid::Uuid::new_v4(),
                domain: "github.com".to_string(),
                username: "user".to_string(),
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
}
