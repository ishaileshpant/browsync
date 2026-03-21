use anyhow::Result;
use browsync_core::models::Browser;

pub fn run(url: &str, browser: Option<Browser>) -> Result<()> {
    let browser = browser.unwrap_or(Browser::Chrome);

    println!("Opening {} in {}...", url, browser);

    let status = std::process::Command::new("open")
        .args(["-a", browser.open_command(), url])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to open URL in {browser}");
    }

    Ok(())
}
