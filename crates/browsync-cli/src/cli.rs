use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "browsync", version, about = "Cross-browser bookmark, history & session consolidation")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Auto-detect installed browsers and their data
    Detect,

    /// Import bookmarks and history from browsers
    Import {
        /// Browser to import from (imports all if not specified)
        #[arg(short, long)]
        browser: Option<BrowserArg>,

        /// Only import bookmarks
        #[arg(long)]
        bookmarks_only: bool,

        /// Only import history
        #[arg(long)]
        history_only: bool,
    },

    /// Search across all bookmarks and history
    Search {
        /// Search query
        query: String,

        /// Only search bookmarks
        #[arg(long)]
        bookmarks: bool,

        /// Only search history
        #[arg(long)]
        history: bool,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,
    },

    /// Show sync status and database stats
    Status,

    /// Open a URL in a specific browser
    Open {
        /// URL to open
        url: String,

        /// Browser to open in
        #[arg(short, long)]
        browser: Option<BrowserArg>,
    },

    /// Export bookmarks to a file
    Export {
        /// Output format
        #[arg(short, long, default_value = "html")]
        format: ExportFormat,

        /// Output file path
        #[arg(short, long)]
        output: Option<String>,

        /// Only export from specific browser
        #[arg(short, long)]
        browser: Option<BrowserArg>,
    },

    /// Launch the interactive TUI
    Tui,

    /// Manage auth entries and password migration
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
}

#[derive(Subcommand)]
pub enum AuthCommands {
    /// List all saved login domains across browsers
    List,

    /// Generate migration report between browsers
    Migrate {
        /// Source browser
        #[arg(long)]
        from: BrowserArg,

        /// Target browser
        #[arg(long)]
        to: BrowserArg,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum BrowserArg {
    Chrome,
    Firefox,
    Safari,
    Edge,
    Brave,
    Arc,
}

impl From<BrowserArg> for browsync_core::models::Browser {
    fn from(b: BrowserArg) -> Self {
        match b {
            BrowserArg::Chrome => Self::Chrome,
            BrowserArg::Firefox => Self::Firefox,
            BrowserArg::Safari => Self::Safari,
            BrowserArg::Edge => Self::Edge,
            BrowserArg::Brave => Self::Brave,
            BrowserArg::Arc => Self::Arc,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ExportFormat {
    Html,
    Json,
    Csv,
}
