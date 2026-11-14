use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use mcs_core::{App, Pane};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::{io, time::Duration};

#[derive(Default)]
struct Ui {
    filter_before: Option<String>,
    bookmarks: bool,
    bookmark_index: usize,
}

struct TerminalRestore(bool);
impl Drop for TerminalRestore {
    fn drop(&mut self) {
        if self.0 {
            ratatui::restore();
        }
    }
}

pub fn run() -> io::Result<()> {
    let cwd = std::env::current_dir()?;
    let mut app = App::new(cwd);
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = ratatui::try_restore();
            return Err(error);
        }
    };
    let mut restore = TerminalRestore(true);
    let mut ui = Ui::default();
    let mut dirty = true;
    let result = loop {
        dirty |= app.poll();
        if dirty {
            if let Err(error) = terminal.draw(|frame| draw(frame, &app, &ui)) {
                break Err(error);
            }
            dirty = false;
        }
        let loading = app.panes.iter().any(Pane::is_loading);
        match event::poll(Duration::from_millis(if loading { 30 } else { 250 })) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    if handle_key(&mut app, &mut ui, key) {
                        break Ok(());
                    }
                    dirty = true;
                }
                Ok(Event::Resize(_, _)) => dirty = true,
                Ok(_) => {}
                Err(error) => break Err(error),
            },
            Ok(false) => {}
            Err(error) => break Err(error),
        }
    };
    let cleanup = ratatui::try_restore();
    restore.0 = cleanup.is_err();
    result.and(cleanup)
}

// Returns true only for an explicit exit action. Editing never triggers browser commands.
fn handle_key(app: &mut App, ui: &mut Ui, key: KeyEvent) -> bool {
    if key.code == KeyCode::F(10)
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
    {
        return true;
    }
    if ui.filter_before.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.active_mut()
                    .set_filter(ui.filter_before.take().expect("editing filter"));
            }
            KeyCode::Enter => {
                ui.filter_before = None;
            }
            KeyCode::Backspace => {
                let mut filter = app.panes[app.active].filter.clone();
                filter.pop();
                app.active_mut().set_filter(filter);
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && !ch.is_control() =>
            {
                let mut filter = app.panes[app.active].filter.clone();
                if filter.chars().count() < 256 {
                    filter.push(ch);
                    app.active_mut().set_filter(filter);
                }
            }
            _ => {}
        }
        return false;
    }
    if key.code == KeyCode::Char('q') {
        return true;
    }
    if ui.bookmarks {
        match key.code {
            KeyCode::Esc | KeyCode::Char('b') | KeyCode::Tab => ui.bookmarks = false,
            KeyCode::Up => ui.bookmark_index = ui.bookmark_index.saturating_sub(1),
            KeyCode::Down => {
                ui.bookmark_index =
                    (ui.bookmark_index + 1).min(app.bookmarks.len().saturating_sub(1))
            }
            KeyCode::Enter => {
                if let Some(path) = app.bookmarks.get(ui.bookmark_index).cloned() {
                    app.active_mut().visit(path);
                }
                ui.bookmarks = false;
            }
            _ => {}
        }
        return false;
    }
    match key.code {
        KeyCode::Tab => app.switch(),
        KeyCode::Up | KeyCode::Char('k') => app.active_mut().move_by(-1),
        KeyCode::Down | KeyCode::Char('j') => app.active_mut().move_by(1),
        KeyCode::PageUp => app.active_mut().move_by(-10),
        KeyCode::PageDown => app.active_mut().move_by(10),
        KeyCode::Home => app.active_mut().selected = 0,
        KeyCode::End => app.active_mut().selected = app.panes[app.active].len().saturating_sub(1),
        KeyCode::Enter => app.active_mut().enter(),
        KeyCode::Backspace | KeyCode::Left => app.active_mut().up(),
        KeyCode::Char('[') => app.active_mut().back(),
        KeyCode::Char(']') => app.active_mut().forward(),
        KeyCode::Char('r') => app.active_mut().refresh(),
        KeyCode::Char(' ') | KeyCode::Insert => app.active_mut().toggle_mark(),
        KeyCode::Char('.') => app.active_mut().toggle_hidden(),
        KeyCode::Char('s') => app.active_mut().toggle_sort(),
        KeyCode::Char('/') => ui.filter_before = Some(app.panes[app.active].filter.clone()),
        KeyCode::Esc => app.active_mut().set_filter(String::new()),
        KeyCode::Char('B') => app.toggle_bookmark(),
        KeyCode::Char('b') => {
            ui.bookmarks = true;
            ui.bookmark_index = 0;
        }
        _ => {}
    }
    false
}

/// Escape terminal controls and bidi overrides without changing the underlying filesystem path.
fn display_text(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        if ch.is_control()
            || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            result.extend(ch.escape_default());
        } else {
            result.push(ch);
        }
    }
    result
}

fn draw(frame: &mut ratatui::Frame, app: &App, ui: &Ui) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(4),
            Constraint::Length(4),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new("MCS M1 | Read-only | B add/remove bookmark; b sidebar"),
        layout[0],
    );
    let body = if ui.bookmarks {
        let sections = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(layout[1]);
        let height = usize::from(sections[0].height.saturating_sub(2));
        let start = ui.bookmark_index.saturating_sub(height.saturating_sub(1));
        let items: Vec<_> = app
            .bookmarks
            .iter()
            .enumerate()
            .skip(start)
            .take(height)
            .map(|(index, path)| {
                ListItem::new(display_text(&path.to_string_lossy()))
                    .style(selection_style(index == ui.bookmark_index, true))
            })
            .collect();
        frame.render_widget(
            List::new(items).block(Block::bordered().title("Bookmarks (session)")),
            sections[0],
        );
        sections[1]
    } else {
        layout[1]
    };
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(body);
    render_pane(
        frame,
        &app.panes[0],
        panels[0],
        app.active == 0 && !ui.bookmarks,
    );
    render_pane(
        frame,
        &app.panes[1],
        panels[1],
        app.active == 1 && !ui.bookmarks,
    );
    let pane = &app.panes[app.active];
    let desc = if let Some(error) = &pane.error {
        format!("Error: {}", display_text(error))
    } else if pane.is_loading() {
        "Loading directory; navigation waits for scan. Tab/q remain available.".into()
    } else {
        match pane.current() {
            Some(entry) => format!(
                "{}\n{}{}{}",
                display_text(&entry.path.to_string_lossy()),
                if entry.is_dir { "directory" } else { "file" },
                if entry.is_symlink { " (symlink)" } else { "" },
                entry
                    .issue
                    .as_ref()
                    .map(|issue| format!(" | {}", display_text(issue)))
                    .unwrap_or_default()
            ),
            None => "No matching entries (or directory is empty).".into(),
        }
    };
    frame.render_widget(
        Paragraph::new(desc).block(Block::bordered().title("Info")),
        layout[2],
    );
    let mode = if ui.filter_before.is_some() {
        format!(
            "Filter: {} | Enter apply; Esc cancel",
            display_text(&pane.filter)
        )
    } else if ui.bookmarks {
        "Sidebar: Up/Down select; Enter visit; Esc close".into()
    } else {
        format!(
            "Filter: {} | . hidden:{} | s sort:{:?} | marks:{} | history:{}/{}",
            display_text(&pane.filter),
            pane.show_hidden,
            pane.sort,
            pane.marked_count(),
            pane.history_counts().0,
            pane.history_counts().1
        )
    };
    frame.render_widget(Paragraph::new(format!("{mode}\nTab pane  Up/Down move  PgUp/PgDn  Home/End  Enter open  Backspace parent\n[ back  ] forward  / filter  Esc clear  Space mark  r refresh  q/F10/Ctrl-C exit")), layout[3]);
}

fn selection_style(selected: bool, active: bool) -> Style {
    if selected && active {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    }
}

fn render_pane(frame: &mut ratatui::Frame, pane: &Pane, area: Rect, active: bool) {
    let title = format!(
        "{} {} | {}{}",
        if active { ">" } else { " " },
        display_text(&pane.path.to_string_lossy()),
        pane.len(),
        if pane.is_loading() { " loading" } else { "" }
    );
    let height = usize::from(area.height.saturating_sub(2));
    let start = pane.selected.saturating_sub(height.saturating_sub(1));
    let items: Vec<_> = pane
        .visible_entries()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, entry)| {
            let name = format!(
                "{}{} {}",
                if pane.is_marked(entry) { '*' } else { ' ' },
                if entry.is_symlink {
                    '@'
                } else if entry.is_dir {
                    '/'
                } else {
                    ' '
                },
                display_text(&entry.name)
            );
            ListItem::new(name).style(selection_style(index == pane.selected, active))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if active { Color::Cyan } else { Color::Gray })),
        ),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};
    use std::path::PathBuf;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn app() -> App {
        App::new(PathBuf::from("/mcs-test"))
    }
    fn render(app: &App, ui: &Ui, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app, ui)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .chunks(usize::from(width))
            .map(|row| {
                row.iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }
    #[test]
    fn filter_mode_isolates_commands_and_esc_restores_filter() {
        let mut app = app();
        let mut ui = Ui::default();
        app.active_mut().set_filter("before".into());
        handle_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        assert!(!handle_key(&mut app, &mut ui, key(KeyCode::Char('q'))));
        handle_key(&mut app, &mut ui, key(KeyCode::Tab));
        assert_eq!(app.active, 0);
        assert_eq!(app.panes[0].filter, "beforeq");
        handle_key(&mut app, &mut ui, key(KeyCode::Esc));
        assert_eq!(app.panes[0].filter, "before");
        handle_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        handle_key(&mut app, &mut ui, key(KeyCode::Char('é')));
        handle_key(&mut app, &mut ui, key(KeyCode::Backspace));
        handle_key(&mut app, &mut ui, key(KeyCode::Enter));
        assert!(ui.filter_before.is_none());
        assert_eq!(app.panes[0].filter, "before");
    }
    #[test]
    fn filter_size_is_bounded_and_ctrl_c_exits_from_any_mode() {
        let mut app = app();
        let mut ui = Ui::default();
        handle_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        for _ in 0..300 {
            handle_key(&mut app, &mut ui, key(KeyCode::Char('x')));
        }
        assert_eq!(app.panes[0].filter.len(), 256);
        assert!(handle_key(
            &mut app,
            &mut ui,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        assert!(handle_key(&mut app, &mut ui, key(KeyCode::F(10))));
    }
    #[test]
    fn bookmark_sidebar_empty_navigation_and_normal_exit() {
        let mut app = app();
        let mut ui = Ui::default();
        handle_key(&mut app, &mut ui, key(KeyCode::Char('b')));
        handle_key(&mut app, &mut ui, key(KeyCode::Down));
        handle_key(&mut app, &mut ui, key(KeyCode::Enter));
        assert!(!ui.bookmarks);
        assert_eq!(app.panes[0].path, PathBuf::from("/mcs-test"));
        handle_key(&mut app, &mut ui, key(KeyCode::Char('B')));
        assert_eq!(app.bookmarks, [PathBuf::from("/mcs-test")]);
        assert!(handle_key(&mut app, &mut ui, key(KeyCode::Char('q'))));
    }
    #[test]
    fn untrusted_display_text_cannot_inject_controls_or_bidi() {
        assert_eq!(
            display_text("file\n\r\t\x1b[31m\u{202e}雪"),
            "file\\n\\r\\t\\u{1b}[31m\\u{202e}雪"
        );
        for ch in (0..=0x9f)
            .filter_map(char::from_u32)
            .filter(|ch| ch.is_control())
        {
            assert!(!display_text(&ch.to_string()).chars().any(char::is_control));
        }
    }
    #[test]
    fn rendering_snapshots_at_standard_sizes_and_tiny_resize() {
        let app = app();
        let ui = Ui::default();
        for (width, height) in [(80, 24), (100, 30)] {
            let rows = render(&app, &ui, width, height);
            assert_eq!(
                rows[0],
                "MCS M1 | Read-only | B add/remove bookmark; b sidebar"
            );
            assert!(rows[1].starts_with("┌> /mcs-test | 0 loading"));
            assert!(rows[usize::from(height) - 7].starts_with("│Loading directory;"));
            assert!(rows[usize::from(height) - 4]
                .starts_with("Filter:  | . hidden:false | s sort:Name | marks:0 | history:0/0"));
        }
        let mut ui = Ui {
            bookmarks: true,
            ..Ui::default()
        };
        for (width, height) in [(1, 1), (20, 5), (80, 24), (100, 30)] {
            render(&app, &ui, width, height);
        }
        ui.bookmarks = false;
        ui.filter_before = Some(String::new());
        assert!(render(&app, &ui, 80, 24)[20].starts_with("Filter:  | Enter apply; Esc cancel"));
    }
}
