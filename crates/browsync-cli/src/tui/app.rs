use anyhow::Result;
use browsync_core::db::Database;
use browsync_core::models::{Bookmark, HistoryEntry};
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Bookmarks = 0,
    History = 1,
    Search = 2,
    Status = 3,
}

pub struct App {
    pub active_tab: Tab,
    pub bookmarks: Vec<Bookmark>,
    pub history: Vec<HistoryEntry>,
    pub selected_index: usize,
    pub search_active: bool,
    pub search_query: String,
    pub search_bookmark_results: Vec<Bookmark>,
    pub search_history_results: Vec<HistoryEntry>,
    pub bookmark_count: usize,
    pub history_count: usize,
    pub sync_log: Vec<(String, String, i64, String)>,
    db: Database,
}

impl App {
    pub fn new() -> Result<Self> {
        let db = Database::open_default()?;
        let bookmarks = db.get_bookmarks(None)?;
        let history = db.get_history(None, 500)?;
        let (bookmark_count, history_count) = db.counts()?;
        let sync_log = db.sync_status()?;

        Ok(Self {
            active_tab: Tab::Bookmarks,
            bookmarks,
            history,
            selected_index: 0,
            search_active: false,
            search_query: String::new(),
            search_bookmark_results: Vec::new(),
            search_history_results: Vec::new(),
            bookmark_count,
            history_count,
            sync_log,
            db,
        })
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Bookmarks => Tab::History,
            Tab::History => Tab::Search,
            Tab::Search => Tab::Status,
            Tab::Status => Tab::Bookmarks,
        };
        self.selected_index = 0;
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            Tab::Bookmarks => Tab::Status,
            Tab::History => Tab::Bookmarks,
            Tab::Search => Tab::History,
            Tab::Status => Tab::Search,
        };
        self.selected_index = 0;
    }

    pub fn toggle_search(&mut self) {
        self.search_active = !self.search_active;
        if self.search_active {
            self.active_tab = Tab::Search;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.search_active || self.active_tab == Tab::Search {
            self.handle_search_key(key);
            return;
        }

        let list_len = match self.active_tab {
            Tab::Bookmarks => self.bookmarks.len(),
            Tab::History => self.history.len(),
            _ => 0,
        };

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index + 1 < list_len {
                    self.selected_index += 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected_index = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if list_len > 0 {
                    self.selected_index = list_len - 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                self.open_selected();
            }
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.run_search();
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.run_search();
            }
            KeyCode::Esc => {
                self.search_active = false;
                self.search_query.clear();
                self.search_bookmark_results.clear();
                self.search_history_results.clear();
            }
            KeyCode::Enter => {
                self.search_active = false;
            }
            _ => {}
        }
    }

    fn run_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_bookmark_results.clear();
            self.search_history_results.clear();
            return;
        }

        if let Ok(bookmarks) = self.db.search_bookmarks(&self.search_query) {
            self.search_bookmark_results = bookmarks;
        }
        if let Ok(history) = self.db.search_history(&self.search_query) {
            self.search_history_results = history;
        }
    }

    fn open_selected(&self) {
        let url = match self.active_tab {
            Tab::Bookmarks => self.bookmarks.get(self.selected_index).map(|b| &b.url),
            Tab::History => self.history.get(self.selected_index).map(|h| &h.url),
            _ => None,
        };

        if let Some(url) = url {
            let _ = std::process::Command::new("open").arg(url).spawn();
        }
    }
}
