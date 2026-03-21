use std::path::PathBuf;

use anyhow::Result;

use crate::models::Browser;

/// Detected browser with its profile path
#[derive(Debug, Clone)]
pub struct DetectedBrowser {
    pub browser: Browser,
    pub profile_path: PathBuf,
    pub is_installed: bool,
    pub has_data: bool,
    pub bookmarks_path: Option<PathBuf>,
    pub history_path: Option<PathBuf>,
    pub login_data_path: Option<PathBuf>,
}

impl DetectedBrowser {
    pub fn status_label(&self) -> &'static str {
        if self.has_data {
            "active"
        } else if self.is_installed {
            "installed (no data)"
        } else {
            "not found"
        }
    }
}

impl std::fmt::Display for DetectedBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<8} {}",
            self.browser.display_name(),
            self.status_label()
        )?;
        if self.has_data {
            let mut parts = Vec::new();
            if self.bookmarks_path.is_some() {
                parts.push("bookmarks");
            }
            if self.history_path.is_some() {
                parts.push("history");
            }
            if self.login_data_path.is_some() {
                parts.push("logins");
            }
            if !parts.is_empty() {
                write!(f, " ({})", parts.join(", "))?;
            }
        }
        Ok(())
    }
}

/// Profile path for Chromium-based browsers
fn chromium_profile_path(subdir: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir()?;
        Some(
            home.join("Library")
                .join("Application Support")
                .join(subdir)
                .join("Default"),
        )
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = dirs::data_local_dir()?;
        Some(appdata.join(subdir).join("User Data").join("Default"))
    }
    #[cfg(target_os = "linux")]
    {
        let config = dirs::config_dir()?;
        Some(config.join(subdir).join("Default"))
    }
}

fn profile_path_for(browser: Browser) -> Option<PathBuf> {
    match browser {
        Browser::Chrome => {
            #[cfg(target_os = "windows")]
            {
                chromium_profile_path("Google\\Chrome")
            }
            #[cfg(not(target_os = "windows"))]
            {
                chromium_profile_path("Google/Chrome")
            }
        }
        Browser::Edge => {
            #[cfg(target_os = "windows")]
            {
                chromium_profile_path("Microsoft\\Edge")
            }
            #[cfg(not(target_os = "windows"))]
            {
                chromium_profile_path("Microsoft Edge")
            }
        }
        Browser::Brave => {
            #[cfg(target_os = "windows")]
            {
                chromium_profile_path("BraveSoftware\\Brave-Browser")
            }
            #[cfg(not(target_os = "windows"))]
            {
                chromium_profile_path("BraveSoftware/Brave-Browser")
            }
        }
        Browser::Arc => chromium_profile_path("Arc/User Data"),
        Browser::Firefox => {
            #[cfg(target_os = "macos")]
            let profiles_dir = {
                let home = dirs::home_dir()?;
                home.join("Library")
                    .join("Application Support")
                    .join("Firefox")
                    .join("Profiles")
            };
            #[cfg(target_os = "windows")]
            let profiles_dir = {
                let appdata = dirs::data_dir()?;
                appdata.join("Mozilla").join("Firefox").join("Profiles")
            };
            #[cfg(target_os = "linux")]
            let profiles_dir = {
                let home = dirs::home_dir()?;
                home.join(".mozilla").join("firefox")
            };
            // Find the default profile (ends with .default-release or .default)
            if profiles_dir.exists()
                && let Ok(entries) = std::fs::read_dir(&profiles_dir)
            {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".default-release") || name.ends_with(".default") {
                        return Some(entry.path());
                    }
                }
            }
            None
        }
        Browser::Safari => {
            // Safari is macOS only
            #[cfg(target_os = "macos")]
            {
                let home = dirs::home_dir()?;
                Some(home.join("Library").join("Safari"))
            }
            #[cfg(not(target_os = "macos"))]
            {
                None
            }
        }
    }
}

fn is_app_installed(browser: Browser) -> bool {
    #[cfg(target_os = "macos")]
    {
        let app_path = PathBuf::from("/Applications").join(format!("{}.app", browser.app_name()));
        if app_path.exists() {
            return true;
        }
        // Safari is a system app
        if browser == Browser::Safari {
            return PathBuf::from("/System/Cryptexes/App/System/Applications/Safari.app").exists()
                || PathBuf::from("/Applications/Safari.app").exists();
        }
        false
    }
    #[cfg(target_os = "windows")]
    {
        // On Windows, check if the profile directory exists (browser was used)
        profile_path_for(browser)
            .map(|p| p.exists())
            .unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        // Check if the browser binary is in PATH
        let bin_name = match browser {
            Browser::Chrome => "google-chrome",
            Browser::Firefox => "firefox",
            Browser::Edge => "microsoft-edge",
            Browser::Brave => "brave-browser",
            Browser::Arc => "arc",
            Browser::Safari => return false,
        };
        std::process::Command::new("which")
            .arg(bin_name)
            .output()
            .is_ok_and(|o| o.status.success())
    }
}

/// Detect a single browser
fn detect_browser(browser: Browser) -> DetectedBrowser {
    let is_installed = is_app_installed(browser);
    let profile_path = profile_path_for(browser).unwrap_or_default();

    let (has_data, bookmarks_path, history_path, login_data_path) = if profile_path.exists() {
        match browser {
            Browser::Chrome | Browser::Edge | Browser::Brave | Browser::Arc => {
                let bookmarks = profile_path.join("Bookmarks");
                let history = profile_path.join("History");
                let logins = profile_path.join("Login Data");
                let has = bookmarks.exists() || history.exists();
                (
                    has,
                    bookmarks.exists().then_some(bookmarks),
                    history.exists().then_some(history),
                    logins.exists().then_some(logins),
                )
            }
            Browser::Firefox => {
                let places = profile_path.join("places.sqlite");
                let logins = profile_path.join("logins.json");
                let has = places.exists();
                (
                    has,
                    places.exists().then_some(places.clone()),
                    places.exists().then_some(places),
                    logins.exists().then_some(logins),
                )
            }
            Browser::Safari => {
                let bookmarks = profile_path.join("Bookmarks.plist");
                let history = profile_path.join("History.db");
                let has = bookmarks.exists() || history.exists();
                (
                    has,
                    bookmarks.exists().then_some(bookmarks),
                    history.exists().then_some(history),
                    None,
                )
            }
        }
    } else {
        (false, None, None, None)
    };

    DetectedBrowser {
        browser,
        profile_path,
        is_installed,
        has_data,
        bookmarks_path,
        history_path,
        login_data_path,
    }
}

/// Detect all supported browsers
pub fn detect_all() -> Vec<DetectedBrowser> {
    Browser::all().iter().map(|&b| detect_browser(b)).collect()
}

/// Detect only browsers that have importable data
pub fn detect_with_data() -> Vec<DetectedBrowser> {
    detect_all().into_iter().filter(|d| d.has_data).collect()
}

/// Detect a specific browser
pub fn detect_one(browser: Browser) -> Result<DetectedBrowser> {
    let detected = detect_browser(browser);
    if !detected.is_installed {
        anyhow::bail!("{} is not installed", browser);
    }
    Ok(detected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_all_returns_all_browsers() {
        let results = detect_all();
        assert_eq!(results.len(), 6);
    }

    #[test]
    fn test_browser_display() {
        let detected = DetectedBrowser {
            browser: Browser::Chrome,
            profile_path: PathBuf::new(),
            is_installed: true,
            has_data: false,
            bookmarks_path: None,
            history_path: None,
            login_data_path: None,
        };
        assert!(detected.to_string().contains("Chrome"));
    }
}
