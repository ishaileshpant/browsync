use std::io::Write;

use anyhow::Result;
use browsync_core::db::Database;
use browsync_core::models::Browser;

use crate::cli::ExportFormat;

pub fn run(format: &ExportFormat, output: Option<&str>, browser: Option<Browser>) -> Result<()> {
    let db = Database::open_default()?;
    let bookmarks = db.get_bookmarks(browser)?;

    if bookmarks.is_empty() {
        println!("No bookmarks to export. Run `browsync import` first.");
        return Ok(());
    }

    let content = match format {
        ExportFormat::Html => browsync_core::exporters::html::export(&bookmarks),
        ExportFormat::Json => browsync_core::exporters::json::export_browsync(&bookmarks)?,
        ExportFormat::Csv => browsync_core::exporters::csv::export_bookmarks(&bookmarks),
    };

    match output {
        Some(path) => {
            let mut file = std::fs::File::create(path)?;
            file.write_all(content.as_bytes())?;
            println!("Exported {} bookmarks to {path}", bookmarks.len());
        }
        None => {
            print!("{content}");
        }
    }

    Ok(())
}
