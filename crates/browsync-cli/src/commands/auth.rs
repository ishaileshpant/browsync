use anyhow::Result;
use browsync_core::detect;
use browsync_core::keychain;
use browsync_core::models::Browser;

pub fn list() -> Result<()> {
    let browsers = detect::detect_with_data();

    let mut all_entries = Vec::new();

    for detected in &browsers {
        if let Some(login_path) = &detected.login_data_path {
            match keychain::extract_chrome_auth(login_path, detected.browser) {
                Ok(entries) => {
                    println!(
                        "{}: {} saved logins",
                        detected.browser,
                        entries.len()
                    );
                    all_entries.extend(entries);
                }
                Err(e) => {
                    eprintln!("{}: could not read logins ({})", detected.browser, e);
                }
            }
        }
    }

    if all_entries.is_empty() {
        println!("No auth entries found.");
        return Ok(());
    }

    // Deduplicate by domain
    all_entries.sort_by(|a, b| a.domain.cmp(&b.domain));
    all_entries.dedup_by(|a, b| a.domain == b.domain && a.source_browser == b.source_browser);

    println!("\nSaved logins by domain:\n");
    println!("  {:<35} {:<25} BROWSER", "DOMAIN", "USERNAME");
    println!("  {}", "-".repeat(70));

    for entry in &all_entries {
        let username = if entry.username.is_empty() {
            "(no username)"
        } else {
            &entry.username
        };
        println!(
            "  {:<35} {:<25} [{}]",
            entry.domain,
            username,
            entry.source_browser.short_code()
        );
    }

    println!("\n{} total entries across {} domains", all_entries.len(), {
        let mut domains: Vec<_> = all_entries.iter().map(|e| &e.domain).collect();
        domains.sort();
        domains.dedup();
        domains.len()
    });

    // Password manager status
    println!();
    if keychain::has_onepassword_cli() {
        println!("1Password CLI: available");
    }
    if keychain::has_bitwarden_cli() {
        println!("Bitwarden CLI: available");
    }

    Ok(())
}

pub fn migrate(from: Browser, to: Browser) -> Result<()> {
    println!("Auth migration report: {} -> {}\n", from, to);

    let browsers = detect::detect_with_data();
    let mut all_entries = Vec::new();

    for detected in &browsers {
        if let Some(login_path) = &detected.login_data_path
            && let Ok(entries) = keychain::extract_chrome_auth(login_path, detected.browser) {
                all_entries.extend(entries);
            }
    }

    let report = keychain::migration_report(&all_entries, from, to);

    if report.is_empty() {
        println!("No auth entries found for {from}.");
        return Ok(());
    }

    let needs_login: Vec<_> = report
        .iter()
        .filter(|r| r.status == keychain::MigrationStatus::NeedsLogin)
        .collect();
    let already_saved: Vec<_> = report
        .iter()
        .filter(|r| r.status == keychain::MigrationStatus::AlreadySaved)
        .collect();

    if !needs_login.is_empty() {
        println!("Sites that NEED RE-LOGIN in {}:\n", to);
        for item in &needs_login {
            let user = if item.username.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.username)
            };
            println!("  ! {}{}", item.domain, user);
        }
    }

    if !already_saved.is_empty() {
        println!("\nSites already saved in {}:\n", to);
        for item in &already_saved {
            println!("  + {}", item.domain);
        }
    }

    println!(
        "\nSummary: {} need login, {} already saved",
        needs_login.len(),
        already_saved.len()
    );

    Ok(())
}
