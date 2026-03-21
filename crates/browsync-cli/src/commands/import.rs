use anyhow::Result;
use browsync_core::db::Database;
use browsync_core::detect;
use browsync_core::models::Browser;
use browsync_core::parsers;

pub fn run(browser: Option<Browser>, bookmarks_only: bool, history_only: bool) -> Result<()> {
    let db = Database::open_default()?;

    let browsers_to_import: Vec<_> = match browser {
        Some(b) => {
            let detected = detect::detect_one(b)?;
            if !detected.has_data {
                anyhow::bail!("{} has no importable data", b);
            }
            vec![detected]
        }
        None => {
            let detected = detect::detect_with_data();
            if detected.is_empty() {
                anyhow::bail!("No browsers with importable data found");
            }
            detected
        }
    };

    for detected in &browsers_to_import {
        println!("Importing from {}...", detected.browser);

        let parser = parsers::parser_for(detected)?;

        if !history_only {
            match parser.parse_bookmarks() {
                Ok(bookmarks) => {
                    let count = db.insert_bookmarks(&bookmarks)?;
                    db.log_sync(detected.browser, "bookmarks", count)?;
                    println!("  Bookmarks: {count} imported");
                }
                Err(e) => {
                    eprintln!("  Bookmarks: skipped ({e})");
                }
            }
        }

        if !bookmarks_only {
            match parser.parse_history() {
                Ok(history) => {
                    let count = db.insert_history(&history)?;
                    db.log_sync(detected.browser, "history", count)?;
                    println!("  History:   {count} imported");
                }
                Err(e) => {
                    eprintln!("  History:   skipped ({e})");
                }
            }
        }
    }

    let (bk, hist) = db.counts()?;
    println!("\nDatabase totals: {bk} bookmarks, {hist} history entries");

    Ok(())
}
