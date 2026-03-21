mod ipc;
mod scheduler;
mod watcher;

use std::time::Duration;

use anyhow::Result;
use browsync_core::db::Database;
use browsync_core::detect;
use browsync_core::parsers;

use ipc::{IpcMessage, IpcServer};
use scheduler::SyncScheduler;
use watcher::{BrowserWatcher, ChangeType};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("start") | None => start_daemon(),
        Some("stop") => stop_daemon(),
        Some("status") => daemon_status(),
        Some(cmd) => {
            eprintln!("Unknown command: {cmd}");
            eprintln!("Usage: browsyncd [start|stop|status]");
            std::process::exit(1);
        }
    }
}

fn start_daemon() -> Result<()> {
    eprintln!("browsync daemon starting...");

    let browsers = detect::detect_with_data();
    if browsers.is_empty() {
        eprintln!("No browsers with data found. Nothing to watch.");
        return Ok(());
    }

    let browser_names: Vec<String> = browsers.iter().map(|b| b.browser.to_string()).collect();
    eprintln!("Watching: {}", browser_names.join(", "));

    // Initialize components
    let file_watcher = BrowserWatcher::new(&browsers)?;
    let ipc_server = IpcServer::bind()?;
    let mut scheduler = SyncScheduler::new(5, 30); // 5s debounce, 30min periodic

    // Write PID file
    let pid_path = browsync_core::db::Database::data_dir()?.join("daemon.pid");
    std::fs::write(&pid_path, std::process::id().to_string())?;

    eprintln!("Daemon ready (PID {})", std::process::id());

    loop {
        // Check for IPC messages
        if let Some((msg, stream)) = ipc_server.try_recv() {
            match msg {
                IpcMessage::Status => {
                    let response = IpcMessage::StatusResponse {
                        running: true,
                        watching: browser_names.clone(),
                        last_sync: Some(chrono::Utc::now().to_rfc3339()),
                        syncs_total: scheduler.total_syncs,
                    };
                    IpcServer::respond(stream, &response)?;
                }
                IpcMessage::Sync { browser } => {
                    eprintln!("Manual sync requested");
                    if let Err(e) = do_sync(&browsers, browser.as_deref()) {
                        eprintln!("Sync error: {e}");
                    }
                    let response = IpcMessage::Ack {
                        message: "Sync completed".to_string(),
                    };
                    IpcServer::respond(stream, &response)?;
                }
                IpcMessage::Stop => {
                    eprintln!("Stop requested, shutting down...");
                    let response = IpcMessage::Ack {
                        message: "Stopping".to_string(),
                    };
                    IpcServer::respond(stream, &response)?;
                    break;
                }
                _ => {}
            }
        }

        // Check for file system events
        let events = file_watcher.drain();
        for event in events {
            eprintln!("Change detected: {event}");
            if scheduler.should_sync(event.browser) {
                match event.change_type {
                    ChangeType::BookmarksModified | ChangeType::HistoryModified => {
                        eprintln!("Syncing {}...", event.browser);
                        if let Err(e) = do_sync(&browsers, Some(event.browser.display_name())) {
                            eprintln!("Sync error: {e}");
                        }
                        scheduler.record_sync(event.browser);
                    }
                    _ => {}
                }
            }
        }

        // Periodic full sync
        if scheduler.periodic_due() {
            eprintln!("Periodic sync...");
            if let Err(e) = do_sync(&browsers, None) {
                eprintln!("Periodic sync error: {e}");
            }
            scheduler.record_periodic();
        }

        // Sleep briefly to avoid busy-waiting
        std::thread::sleep(Duration::from_millis(500));
    }

    // Cleanup
    let _ = std::fs::remove_file(&pid_path);
    eprintln!("Daemon stopped.");
    Ok(())
}

fn do_sync(
    browsers: &[detect::DetectedBrowser],
    filter: Option<&str>,
) -> Result<()> {
    let db = Database::open_default()?;

    for detected in browsers {
        if let Some(f) = filter {
            if !detected.browser.display_name().eq_ignore_ascii_case(f) {
                continue;
            }
        }

        let parser = parsers::parser_for(detected)?;

        match parser.parse_bookmarks() {
            Ok(bookmarks) => {
                let count = db.insert_bookmarks(&bookmarks)?;
                db.log_sync(detected.browser, "bookmarks", count)?;
                eprintln!("  {} bookmarks: {count}", detected.browser);
            }
            Err(e) => eprintln!("  {} bookmarks: {e}", detected.browser),
        }

        match parser.parse_history() {
            Ok(history) => {
                let count = db.insert_history(&history)?;
                db.log_sync(detected.browser, "history", count)?;
                eprintln!("  {} history: {count}", detected.browser);
            }
            Err(e) => eprintln!("  {} history: {e}", detected.browser),
        }
    }

    Ok(())
}

fn stop_daemon() -> Result<()> {
    match ipc::IpcClient::send(&IpcMessage::Stop) {
        Ok(IpcMessage::Ack { message }) => {
            println!("Daemon: {message}");
        }
        Ok(_) => println!("Unexpected response"),
        Err(e) => {
            println!("Could not reach daemon: {e}");
            // Try to kill by PID
            let pid_path = Database::data_dir()?.join("daemon.pid");
            if let Ok(pid) = std::fs::read_to_string(&pid_path) {
                let _ = std::process::Command::new("kill")
                    .arg(pid.trim())
                    .status();
                let _ = std::fs::remove_file(&pid_path);
                println!("Killed daemon process");
            }
        }
    }
    Ok(())
}

fn daemon_status() -> Result<()> {
    match ipc::IpcClient::send(&IpcMessage::Status) {
        Ok(IpcMessage::StatusResponse {
            running,
            watching,
            last_sync,
            syncs_total,
        }) => {
            println!("Daemon: {}", if running { "running" } else { "stopped" });
            println!("Watching: {}", watching.join(", "));
            if let Some(ls) = last_sync {
                println!("Last sync: {ls}");
            }
            println!("Total syncs: {syncs_total}");
        }
        Ok(_) => println!("Unexpected response"),
        Err(_) => {
            println!("Daemon: not running");
        }
    }
    Ok(())
}
