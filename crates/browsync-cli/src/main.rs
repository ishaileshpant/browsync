mod cli;
mod commands;
mod tui;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Detect => commands::detect::run(),
        Commands::Import {
            browser,
            bookmarks_only,
            history_only,
        } => commands::import::run(browser.map(Into::into), bookmarks_only, history_only),
        Commands::Search {
            query,
            bookmarks,
            history,
            limit,
        } => commands::search::run(&query, bookmarks, history, limit),
        Commands::Status => commands::status::run(),
        Commands::Open { url, browser } => commands::open::run(&url, browser.map(Into::into)),
        Commands::Export {
            ref format,
            ref output,
            browser,
        } => commands::export::run(format, output.as_deref(), browser.map(Into::into)),
        Commands::Tui => tui::run(),
        Commands::Auth { command } => match command {
            cli::AuthCommands::List => commands::auth::list(),
            cli::AuthCommands::Migrate { from, to } => {
                commands::auth::migrate(from.into(), to.into())
            }
        },
    }
}
