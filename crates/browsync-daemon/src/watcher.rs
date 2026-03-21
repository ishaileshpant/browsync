use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use browsync_core::detect::DetectedBrowser;
use browsync_core::models::Browser;

/// Event from a browser profile change
#[derive(Debug, Clone)]
pub struct BrowserChangeEvent {
    pub browser: Browser,
    pub change_type: ChangeType,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum ChangeType {
    BookmarksModified,
    HistoryModified,
    LoginDataModified,
    Other,
}

impl std::fmt::Display for BrowserChangeEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {:?} at {}", self.browser, self.change_type, self.path.display())
    }
}

/// Watch browser profile directories for changes
pub struct BrowserWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<BrowserChangeEvent>,
}

impl BrowserWatcher {
    pub fn new(browsers: &[DetectedBrowser]) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        // Map profile paths to browsers for event classification
        let browser_paths: Vec<(Browser, PathBuf)> = browsers
            .iter()
            .filter(|b| b.has_data)
            .map(|b| (b.browser, b.profile_path.clone()))
            .collect();

        let tx_clone = tx.clone();
        let paths_clone = browser_paths.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_)
                ) {
                    for path in &event.paths {
                        if let Some(change_event) = classify_event(path, &paths_clone) {
                            let _ = tx_clone.send(change_event);
                        }
                    }
                }
            }
        })?;

        // Watch each browser's profile directory
        for (browser, profile_path) in &browser_paths {
            if profile_path.exists() {
                match watcher.watch(profile_path, RecursiveMode::NonRecursive) {
                    Ok(()) => {
                        eprintln!("Watching {} at {}", browser, profile_path.display());
                    }
                    Err(e) => {
                        eprintln!("Could not watch {}: {e}", browser);
                    }
                }
            }
        }

        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }

    /// Receive the next change event (blocking with timeout)
    pub fn recv_timeout(&self, timeout: Duration) -> Option<BrowserChangeEvent> {
        self.rx.recv_timeout(timeout).ok()
    }

    /// Receive all pending events (non-blocking drain)
    pub fn drain(&self) -> Vec<BrowserChangeEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }
}

fn classify_event(path: &PathBuf, browser_paths: &[(Browser, PathBuf)]) -> Option<BrowserChangeEvent> {
    let filename = path.file_name()?.to_str()?;

    // Find which browser this path belongs to
    let browser = browser_paths
        .iter()
        .find(|(_, profile_path)| path.starts_with(profile_path))
        .map(|(b, _)| *b)?;

    let change_type = match filename {
        "Bookmarks" | "places.sqlite" | "Bookmarks.plist" => ChangeType::BookmarksModified,
        "History" | "History.db" => ChangeType::HistoryModified,
        "Login Data" | "logins.json" => ChangeType::LoginDataModified,
        _ => return None, // Ignore unrelated files
    };

    Some(BrowserChangeEvent {
        browser,
        change_type,
        path: path.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_chrome_bookmarks() {
        let paths = vec![(
            Browser::Chrome,
            PathBuf::from("/Users/test/Library/Application Support/Google/Chrome/Default"),
        )];

        let event = classify_event(
            &PathBuf::from("/Users/test/Library/Application Support/Google/Chrome/Default/Bookmarks"),
            &paths,
        );

        assert!(event.is_some());
        let e = event.unwrap();
        assert_eq!(e.browser, Browser::Chrome);
        assert!(matches!(e.change_type, ChangeType::BookmarksModified));
    }

    #[test]
    fn test_classify_ignores_unrelated() {
        let paths = vec![(
            Browser::Chrome,
            PathBuf::from("/Users/test/Chrome/Default"),
        )];

        let event = classify_event(
            &PathBuf::from("/Users/test/Chrome/Default/GPUCache"),
            &paths,
        );

        assert!(event.is_none());
    }
}
