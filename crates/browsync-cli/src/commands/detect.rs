use anyhow::Result;

pub fn run() -> Result<()> {
    let browsers = browsync_core::detect::detect_all();

    println!("Detected browsers:\n");
    for detected in &browsers {
        let icon = if detected.has_data {
            "+"
        } else if detected.is_installed {
            "-"
        } else {
            " "
        };
        println!("  [{icon}] {detected}");
    }

    let with_data: Vec<_> = browsers.iter().filter(|b| b.has_data).collect();
    println!(
        "\n{} browser(s) with importable data",
        with_data.len()
    );

    if with_data.is_empty() {
        println!("\nNo browser data found. Make sure browsers have been used at least once.");
    } else {
        println!("\nRun `browsync import` to import data from detected browsers.");
    }

    Ok(())
}
