use anyhow::Result;
use browsync_core::db::Database;

pub fn run(query: &str, bookmarks_only: bool, history_only: bool, limit: usize) -> Result<()> {
    let db = Database::open_default()?;

    let mut found = false;

    if !history_only {
        let bookmarks = db.search_bookmarks(query)?;
        if !bookmarks.is_empty() {
            found = true;
            println!("Bookmarks ({} results):\n", bookmarks.len().min(limit));
            for (i, bm) in bookmarks.iter().take(limit).enumerate() {
                let folder = bm.folder_path.join(" > ");
                println!(
                    "  {}. {} [{}]",
                    i + 1,
                    bm.title,
                    bm.source_browser.short_code()
                );
                println!("     {}", bm.url);
                if !folder.is_empty() {
                    println!("     {folder}");
                }
            }
        }
    }

    if !bookmarks_only {
        let history = db.search_history(query)?;
        if !history.is_empty() {
            found = true;
            if !history_only {
                println!();
            }
            println!("History ({} results):\n", history.len().min(limit));
            for (i, entry) in history.iter().take(limit).enumerate() {
                let ago = format_time_ago(entry.last_visited);
                println!(
                    "  {}. {} [{}] ({} visits, {})",
                    i + 1,
                    entry.title,
                    entry.source_browser.short_code(),
                    entry.visit_count,
                    ago
                );
                println!("     {}", entry.url);
            }
        }
    }

    if !found {
        println!("No results for \"{query}\"");
        println!("Try `browsync import` first to populate the database.");
    }

    Ok(())
}

fn format_time_ago(dt: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now - dt;

    if diff.num_days() > 365 {
        format!("{}y ago", diff.num_days() / 365)
    } else if diff.num_days() > 30 {
        format!("{}mo ago", diff.num_days() / 30)
    } else if diff.num_days() > 0 {
        format!("{}d ago", diff.num_days())
    } else if diff.num_hours() > 0 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_minutes() > 0 {
        format!("{}m ago", diff.num_minutes())
    } else {
        "just now".to_string()
    }
}
