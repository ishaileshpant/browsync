pub mod app;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, Tab};

pub fn run() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    loop {
        terminal.draw(|frame| ui(frame, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Global keybindings
            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Char('q')) => {
                    break;
                }
                (_, KeyCode::Tab) => {
                    app.next_tab();
                }
                (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                    app.prev_tab();
                }
                (_, KeyCode::Char('/')) => {
                    app.toggle_search();
                }
                (_, KeyCode::Char('1')) => app.active_tab = Tab::Bookmarks,
                (_, KeyCode::Char('2')) => app.active_tab = Tab::History,
                (_, KeyCode::Char('3')) => app.active_tab = Tab::Search,
                (_, KeyCode::Char('4')) => app.active_tab = Tab::Status,
                _ => {
                    app.handle_key(key);
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tab bar
            Constraint::Min(10),  // Content
            Constraint::Length(3), // Search / status bar
        ])
        .split(frame.area());

    // Tab bar
    let tab_titles: Vec<Line> = [" Bookmarks ", " History ", " Search ", " Status "]
        .iter()
        .map(|t| Line::from(*t))
        .collect();

    let tabs = ratatui::widgets::Tabs::new(tab_titles)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title(" browsync "),
        )
        .select(app.active_tab as usize)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Cyan).bold());

    frame.render_widget(tabs, chunks[0]);

    // Content area
    match app.active_tab {
        Tab::Bookmarks => render_bookmarks(frame, app, chunks[1]),
        Tab::History => render_history(frame, app, chunks[1]),
        Tab::Search => render_search(frame, app, chunks[1]),
        Tab::Status => render_status(frame, app, chunks[1]),
    }

    // Bottom bar
    let help = if app.search_active {
        format!(" Search: {}_ ", app.search_query)
    } else {
        " [/]search [1-4]tabs [o]pen [q]uit ".to_string()
    };

    let bottom = ratatui::widgets::Paragraph::new(help).block(
        ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(bottom, chunks[2]);
}

fn render_bookmarks(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ratatui::widgets::ListItem> = app
        .bookmarks
        .iter()
        .enumerate()
        .map(|(i, bm)| {
            let style = if i == app.selected_index {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default()
            };

            let folder = bm.folder_path.join(" > ");
            let line = Line::from(vec![
                Span::styled(
                    format!("[{}] ", bm.source_browser.short_code()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(&bm.title, style),
                Span::styled(format!("  {folder}"), Style::default().fg(Color::DarkGray)),
            ]);
            ratatui::widgets::ListItem::new(line)
        })
        .collect();

    let list = ratatui::widgets::List::new(items)
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title(format!(" Bookmarks ({}) ", app.bookmarks.len())),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(list, area);
}

fn render_history(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ratatui::widgets::ListItem> = app
        .history
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let style = if i == app.selected_index {
                Style::default().fg(Color::Cyan).bold()
            } else {
                Style::default()
            };

            let visits = format!("{}x", entry.visit_count);
            let line = Line::from(vec![
                Span::styled(
                    format!("[{}] ", entry.source_browser.short_code()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(&entry.title, style),
                Span::styled(format!("  {visits}"), Style::default().fg(Color::Green)),
                Span::styled(
                    format!("  {}", entry.url),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ratatui::widgets::ListItem::new(line)
        })
        .collect();

    let list = ratatui::widgets::List::new(items).block(
        ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .title(format!(" History ({}) ", app.history.len())),
    );

    frame.render_widget(list, area);
}

fn render_search(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(area);

    let input = ratatui::widgets::Paragraph::new(format!(" > {}", app.search_query))
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title(" Search "),
        )
        .style(Style::default().fg(Color::Cyan));

    frame.render_widget(input, chunks[0]);

    let mut items: Vec<ratatui::widgets::ListItem> = Vec::new();

    for bm in &app.search_bookmark_results {
        let line = Line::from(vec![
            Span::styled("[B] ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("[{}] ", bm.source_browser.short_code()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(&bm.title),
            Span::styled(format!("  {}", bm.url), Style::default().fg(Color::DarkGray)),
        ]);
        items.push(ratatui::widgets::ListItem::new(line));
    }

    for entry in &app.search_history_results {
        let line = Line::from(vec![
            Span::styled("[H] ", Style::default().fg(Color::Green)),
            Span::styled(
                format!("[{}] ", entry.source_browser.short_code()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(&entry.title),
            Span::styled(
                format!("  {}", entry.url),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        items.push(ratatui::widgets::ListItem::new(line));
    }

    let total = app.search_bookmark_results.len() + app.search_history_results.len();
    let list = ratatui::widgets::List::new(items).block(
        ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .title(format!(" Results ({total}) ")),
    );

    frame.render_widget(list, chunks[1]);
}

fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Bookmarks: ", Style::default().bold()),
            Span::raw(format!("{}", app.bookmark_count)),
        ]),
        Line::from(vec![
            Span::styled("History:   ", Style::default().bold()),
            Span::raw(format!("{}", app.history_count)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Recent syncs:",
            Style::default().bold().underlined(),
        )),
    ];

    for (browser, sync_type, items, when) in &app.sync_log {
        lines.push(Line::from(format!(
            "  {browser:<8} {sync_type:<12} {items:>5} items  {when}"
        )));
    }

    let paragraph = ratatui::widgets::Paragraph::new(lines).block(
        ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .title(" Status "),
    );

    frame.render_widget(paragraph, area);
}
