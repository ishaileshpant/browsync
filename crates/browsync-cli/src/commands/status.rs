use anyhow::Result;
use browsync_core::db::Database;
use browsync_core::detect;

pub fn run() -> Result<()> {
    // Detect browsers
    let browsers = detect::detect_all();
    let active: Vec<_> = browsers.iter().filter(|b| b.has_data).collect();

    println!("Browsers:\n");
    for b in &browsers {
        let icon = if b.has_data {
            "+"
        } else if b.is_installed {
            "-"
        } else {
            " "
        };
        println!("  [{icon}] {b}");
    }

    // Database stats
    println!();
    match Database::open_default() {
        Ok(db) => {
            let (bk, hist) = db.counts()?;
            println!("Database: {bk} bookmarks, {hist} history entries");
            let db_path = Database::data_dir()?.join("browsync.db");
            println!("Location: {}", db_path.display());

            let sync_log = db.sync_status()?;
            if !sync_log.is_empty() {
                println!("\nRecent syncs:");
                for (browser, sync_type, items, when) in sync_log.iter().take(10) {
                    println!("  {browser:<8} {sync_type:<12} {items:>5} items  {when}");
                }
            }
        }
        Err(_) => {
            println!("Database: not initialized yet");
            println!("Run `browsync import` to get started.");
        }
    }

    println!("\n{} browser(s) with importable data", active.len());

    Ok(())
}
