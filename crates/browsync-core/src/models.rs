use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Chrome,
    Firefox,
    Safari,
    Edge,
    Brave,
    Arc,
}

impl Browser {
    pub fn display_name(&self) -> &'static str {
        match self {
            Browser::Chrome => "Chrome",
            Browser::Firefox => "Firefox",
            Browser::Safari => "Safari",
            Browser::Edge => "Edge",
            Browser::Brave => "Brave",
            Browser::Arc => "Arc",
        }
    }

    pub fn short_code(&self) -> &'static str {
        match self {
            Browser::Chrome => "C",
            Browser::Firefox => "F",
            Browser::Safari => "S",
            Browser::Edge => "E",
            Browser::Brave => "B",
            Browser::Arc => "A",
        }
    }

    /// macOS app bundle name
    pub fn app_name(&self) -> &'static str {
        match self {
            Browser::Chrome => "Google Chrome",
            Browser::Firefox => "Firefox",
            Browser::Safari => "Safari",
            Browser::Edge => "Microsoft Edge",
            Browser::Brave => "Brave Browser",
            Browser::Arc => "Arc",
        }
    }

    /// `open -a` identifier for launching URLs
    pub fn open_command(&self) -> &'static str {
        match self {
            Browser::Chrome => "Google Chrome",
            Browser::Firefox => "Firefox",
            Browser::Safari => "Safari",
            Browser::Edge => "Microsoft Edge",
            Browser::Brave => "Brave Browser",
            Browser::Arc => "Arc",
        }
    }

    pub fn all() -> &'static [Browser] {
        &[
            Browser::Chrome,
            Browser::Firefox,
            Browser::Safari,
            Browser::Edge,
            Browser::Brave,
            Browser::Arc,
        ]
    }
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for Browser {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "chrome" | "google chrome" => Ok(Browser::Chrome),
            "firefox" => Ok(Browser::Firefox),
            "safari" => Ok(Browser::Safari),
            "edge" | "microsoft edge" => Ok(Browser::Edge),
            "brave" | "brave browser" => Ok(Browser::Brave),
            "arc" => Ok(Browser::Arc),
            _ => anyhow::bail!("Unknown browser: {s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub folder_path: Vec<String>,
    pub tags: Vec<String>,
    pub favicon_url: Option<String>,
    pub source_browser: Browser,
    pub source_id: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub visit_count: u32,
    pub last_visited: DateTime<Utc>,
    pub source_browser: Browser,
    pub duration_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub tab_group: Option<String>,
    pub window_id: u32,
    pub source_browser: Browser,
    pub is_active: bool,
    pub last_accessed: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthEntry {
    pub id: Uuid,
    pub domain: String,
    pub username: String,
    pub source_browser: Browser,
    pub last_used: Option<DateTime<Utc>>,
    pub password_manager: Option<String>,
}

/// Summary of what was imported from a browser
#[derive(Debug, Default)]
pub struct ImportStats {
    pub bookmarks: usize,
    pub history_entries: usize,
    pub auth_entries: usize,
}

impl std::fmt::Display for ImportStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} bookmarks, {} history entries, {} auth entries",
            self.bookmarks, self.history_entries, self.auth_entries
        )
    }
}
