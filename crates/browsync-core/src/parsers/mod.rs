pub mod chrome;
pub mod firefox;
pub mod safari;

use anyhow::Result;

use crate::detect::DetectedBrowser;
use crate::models::{Bookmark, Browser, HistoryEntry, ImportStats};

/// Trait for browser-specific data parsers
pub trait BrowserParser {
    fn parse_bookmarks(&self) -> Result<Vec<Bookmark>>;
    fn parse_history(&self) -> Result<Vec<HistoryEntry>>;
}

/// Get the appropriate parser for a detected browser
pub fn parser_for(detected: &DetectedBrowser) -> Result<Box<dyn BrowserParser>> {
    match detected.browser {
        Browser::Chrome | Browser::Edge | Browser::Brave | Browser::Arc => {
            Ok(Box::new(chrome::ChromeParser::new(detected)?))
        }
        Browser::Firefox => Ok(Box::new(firefox::FirefoxParser::new(detected)?)),
        Browser::Safari => Ok(Box::new(safari::SafariParser::new(detected)?)),
    }
}

/// Import all data from a detected browser
pub fn import_browser(
    detected: &DetectedBrowser,
) -> Result<(Vec<Bookmark>, Vec<HistoryEntry>, ImportStats)> {
    let parser = parser_for(detected)?;

    let bookmarks = match parser.parse_bookmarks() {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "Warning: Could not parse {} bookmarks: {e}",
                detected.browser
            );
            Vec::new()
        }
    };

    let history = match parser.parse_history() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Warning: Could not parse {} history: {e}", detected.browser);
            Vec::new()
        }
    };

    let stats = ImportStats {
        bookmarks: bookmarks.len(),
        history_entries: history.len(),
        auth_entries: 0,
    };

    Ok((bookmarks, history, stats))
}
