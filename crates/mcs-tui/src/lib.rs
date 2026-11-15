mod capabilities;
mod graphics;
mod icons;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod operations;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod shell;
mod shortcuts;
mod viewer;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use mcs_core::{App, Pane};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::{io, time::Duration};
use viewer::{draw, handle_key, Ui};

#[derive(Default)]
struct BrowserUi {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    operations: operations::Operations,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    shells: shell::Shells,
    filter_before: Option<String>,
    bookmarks: bool,
    bookmark_index: usize,
    icons: bool,
    popup: Option<BrowserPopup>,
    notice: Option<&'static str>,
}

enum BrowserPopup {
    Help(usize),
    Menu(usize),
}

const BROWSER_ACTIONS: [(char, &str); 7] = [
    ('s', "Sort: reverse name order"),
    ('.', "Hidden files: show/hide"),
    ('/', "Filter filenames"),
    ('b', "Browse session bookmarks"),
    ('B', "Bookmark current directory"),
    ('r', "Refresh directory"),
    ('i', "Toggle file and menu icons"),
];

const BROWSER_HELP: &[&str] = &[
    "F1 Help   F2 Actions (session menu)   F3 View",
    "F9 Main menu   F10 Quit   Esc/q closes this help",
    "",
    "F4 Edit (UTF-8, 1 MiB); Ctrl-S reviews save; Esc discards.",
    "F5 Copy, F6 Move, F7 Mkdir: Enter executes, Esc cancels.",
    "F8 Delete and editor Save: confirm with y.",
    "Marks take priority; destinations never overwrite.",
    "Ctrl-T: shell/files toggle. Ctrl-]: pane from shell. F11/F12 reserved.",
    "",
    "Tab switches panes. Arrows/j/k move; PgUp/PgDn page.",
    "Home/End first/last; Enter open; Backspace/Left parent.",
    "[ and ] navigate history; r refreshes the active pane.",
    "Space/Insert marks; s reverses name sort; . toggles hidden.",
    "/ filters; Enter applies; Esc cancels/clears the filter.",
    "B bookmarks the directory; b opens session bookmarks.",
    "i toggles icons; v is an alias for F3; q/Ctrl-C exit.",
    "Menus: Up/Down selects; Enter or the shown key applies.",
    "macOS may require Fn with function keys.",
];

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
    ui.browser.icons = icons::enabled();
    if let Some(capabilities) = capabilities::probe() {
        ui.viewer.font = Some(capabilities.font);
        ui.viewer.compression = capabilities.compression;
    }
    let size = terminal.size()?;
    ui.resize(size.width, size.height);
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    ui.browser.shells.resize(browser_panels(
        Rect::new(0, 0, size.width, size.height),
        false,
    ));
    let mut dirty = true;
    let result = loop {
        dirty |= app.poll();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            dirty |= ui.browser.operations.poll(&mut app);
            dirty |= ui.browser.shells.poll();
        }
        dirty |= ui.poll();
        if matches!(ui.viewer.screen, viewer::Screen::Playback(_)) {
            let viewer::Screen::Playback(playback) = std::mem::take(&mut ui.viewer.screen) else {
                unreachable!()
            };
            if let Err(error) = ui
                .viewer
                .flush_graphics()
                .and_then(|()| ratatui::try_restore())
            {
                break Err(error);
            }
            let outcome = playback.run();
            match ratatui::try_init() {
                Ok(resumed) => terminal = resumed,
                Err(error) => break Err(error),
            }
            if let Err(error) = terminal.clear() {
                break Err(error);
            }
            match outcome {
                Ok(mcs_media::video::Outcome::ExitApplication) => break Ok(()),
                Ok(mcs_media::video::Outcome::Browser) => ui.viewer.close(),
                Err(error) => ui.viewer.screen = viewer::Screen::Error(error.to_string()),
            }
            dirty = true;
        }
        if let Err(error) = ui.viewer.flush_graphics() {
            break Err(error);
        }
        if dirty {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                let size = terminal.size()?;
                ui.browser.shells.resize(browser_panels(
                    Rect::new(0, 0, size.width, size.height),
                    ui.browser.bookmarks,
                ));
            }
            if let Err(error) = terminal.draw(|frame| draw(frame, &app, &ui)) {
                break Err(error);
            }
            dirty = false;
        }
        let loading = app.panes.iter().any(Pane::is_loading) || ui.busy();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let loading = loading || ui.browser.shells.visible.iter().any(|visible| *visible);
        match event::poll(Duration::from_millis(if loading { 30 } else { 250 })) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    if handle_key(&mut app, &mut ui, key) {
                        break Ok(());
                    }
                    dirty = true;
                }
                Ok(Event::Resize(width, height)) => {
                    ui.resize(width, height);
                    #[cfg(any(target_os = "macos", target_os = "linux"))]
                    ui.browser.shells.resize(browser_panels(
                        Rect::new(0, 0, width, height),
                        ui.browser.bookmarks,
                    ));
                    dirty = true;
                }
                Ok(_) => {}
                Err(error) => break Err(error),
            },
            Ok(false) => {}
            Err(error) => break Err(error),
        }
    };
    ui.viewer.close();
    let graphics_cleanup = ui.viewer.flush_graphics();
    let cleanup = ratatui::try_restore().and(graphics_cleanup);
    restore.0 = cleanup.is_err();
    result.and(cleanup)
}

fn is_shell_toggle(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('t') && key.modifiers == KeyModifiers::CONTROL
}

// Returns true only for an explicit exit action. Editing never triggers browser commands.
fn handle_browser_key(app: &mut App, ui: &mut BrowserUi, key: KeyEvent) -> bool {
    if key.code == KeyCode::F(10)
        || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
    {
        return true;
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if operations::handle(app, ui, key) {
        return false;
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
        if ui.popup.take().is_some() {
            return false;
        }
        return true;
    }
    if let Some(popup) = &mut ui.popup {
        match key.code {
            KeyCode::Esc => ui.popup = None,
            KeyCode::F(1) => ui.popup = Some(BrowserPopup::Help(0)),
            KeyCode::F(2) | KeyCode::F(9) => ui.popup = Some(BrowserPopup::Menu(0)),
            KeyCode::Up => match popup {
                BrowserPopup::Help(index) | BrowserPopup::Menu(index) => {
                    *index = index.saturating_sub(1)
                }
            },
            KeyCode::Down => match popup {
                BrowserPopup::Help(index) => *index = (*index + 1).min(BROWSER_HELP.len() - 1),
                BrowserPopup::Menu(index) => *index = (*index + 1).min(BROWSER_ACTIONS.len() - 1),
            },
            KeyCode::Enter if matches!(popup, BrowserPopup::Menu(_)) => {
                let BrowserPopup::Menu(index) = popup else {
                    unreachable!()
                };
                let action = BROWSER_ACTIONS[*index].0;
                ui.popup = None;
                return handle_browser_key(
                    app,
                    ui,
                    KeyEvent::new(KeyCode::Char(action), KeyModifiers::NONE),
                );
            }
            KeyCode::Char(action)
                if matches!(popup, BrowserPopup::Menu(_))
                    && BROWSER_ACTIONS.iter().any(|(key, _)| *key == action) =>
            {
                ui.popup = None;
                return handle_browser_key(app, ui, key);
            }
            _ => {}
        }
        return false;
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
    ui.notice = None;
    match key.code {
        KeyCode::F(1) => ui.popup = Some(BrowserPopup::Help(0)),
        KeyCode::F(2) | KeyCode::F(9) => ui.popup = Some(BrowserPopup::Menu(0)),
        KeyCode::F(4..=8) => ui.notice = Some(
            "Unavailable: Edit/Copy/Move/Mkdir/Delete are not implemented in this read-only build.",
        ),
        _ if is_shell_toggle(key) => {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            ui.shells.toggle(app.active, &app.panes[app.active].path);
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            {
                ui.notice = Some("Embedded shell unavailable on this platform.");
            }
        }
        KeyCode::F(11..=12) => ui.notice = Some("F11/F12 are reserved; no operation is assigned."),
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
        KeyCode::Char('i') => ui.icons = !ui.icons,
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn browser_panels(area: Rect, bookmarks: bool) -> [Rect; 2] {
    let sections = Layout::vertical([Constraint::Min(0), Constraint::Length(5)]).split(area);
    let layout = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(4),
    ])
    .split(sections[0]);
    let body = if bookmarks {
        Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(layout[1])[1]
    } else {
        layout[1]
    };
    let panels =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(body);
    [panels[0], panels[1]]
}

fn draw_browser(frame: &mut ratatui::Frame, app: &App, ui: &BrowserUi) {
    let sections =
        Layout::vertical([Constraint::Min(0), Constraint::Length(5)]).split(frame.area());
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(4),
        ])
        .split(sections[0]);
    frame.render_widget(
        Paragraph::new("MCS | F3/v view | F4-F8 files | B bookmark; b sidebar | i icons"),
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
    for index in 0..2 {
        let active = app.active == index && !ui.bookmarks;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if ui.shells.visible[index] {
            ui.shells.draw(frame, index, panels[index], active);
            continue;
        }
        render_pane(frame, &app.panes[index], panels[index], active, ui.icons);
    }
    let pane = &app.panes[app.active];
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let shell_notice = ui.shells.notice(app.active);
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let shell_notice: Option<&str> = None;
    let desc = if let Some(notice) = shell_notice {
        display_text(notice)
    } else if let Some(notice) = ui.notice {
        notice.into()
    } else if let Some(error) = &pane.error {
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
    render_browser_footer(frame, pane, ui, sections[1], app.active);
    if let Some(popup) = &ui.popup {
        draw_browser_popup(frame, popup);
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    ui.operations.draw(frame);
}

fn draw_browser_popup(frame: &mut ratatui::Frame, popup: &BrowserPopup) {
    let screen = frame.area();
    let width = screen.width.min(72);
    let (title, items): (&str, Vec<ListItem>) = match popup {
        BrowserPopup::Help(start) => (
            "Help | Up/Down scroll | Esc close",
            BROWSER_HELP
                .iter()
                .skip(*start)
                .map(|line| ListItem::new(*line))
                .collect(),
        ),
        BrowserPopup::Menu(selected) => (
            "Actions / Main menu | Enter choose | Esc close",
            BROWSER_ACTIONS
                .iter()
                .enumerate()
                .skip(selected.saturating_sub(usize::from(screen.height.saturating_sub(4))))
                .map(|(index, (key, label))| {
                    ListItem::new(format!(" {key}  {label}"))
                        .style(selection_style(index == *selected, true))
                })
                .collect(),
        ),
    };
    let height = screen
        .height
        .min(if matches!(popup, BrowserPopup::Help(_)) {
            19
        } else {
            11
        });
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, area);
    frame.render_widget(
        List::new(items)
            .block(
                Block::bordered()
                    .title(title)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White).bg(Color::Black)),
        area,
    );
}

fn render_browser_footer(
    frame: &mut ratatui::Frame,
    pane: &Pane,
    ui: &BrowserUi,
    area: Rect,
    _active: usize,
) {
    // Anchor the shortcut bar to the last row, including in very short terminals.
    let rows = Layout::vertical([
        Constraint::Length(area.height.saturating_sub(3)),
        Constraint::Length(3),
    ])
    .split(area);
    let status = if ui.filter_before.is_some() {
        format!("Filter: {}_", display_text(&pane.filter))
    } else if ui.bookmarks {
        "Bookmarks | Session only".into()
    } else {
        format!(
            " {} marked | {} | Hidden {} | History {}/{}{}",
            pane.marked_count(),
            match pane.sort {
                mcs_core::SortOrder::Name => "Name A-Z",
                mcs_core::SortOrder::NameDescending => "Name Z-A",
            },
            if pane.show_hidden { "on" } else { "off" },
            pane.history_counts().0,
            pane.history_counts().1,
            if pane.filter.is_empty() {
                String::new()
            } else {
                format!(" | Filter: {}", display_text(&pane.filter))
            }
        )
    };
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let shell_active = ui.shells.visible[_active];
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let shell_active = false;
    let (hint, shortcuts): (&str, &[(&str, &str)]) =
        if shell_active && !ui.bookmarks && ui.filter_before.is_none() {
            (
                " Shell: Tab completes | Ctrl-C interrupts | Ctrl-] pane | Ctrl-T files",
                &[("Ctrl-T", "Files"), ("Ctrl-]", "Pane"), ("F10", "Quit")],
            )
        } else if ui.filter_before.is_some() {
            (
                " Type to filter | Enter apply | Esc cancel",
                &[("Enter", "Apply"), ("Esc", "Cancel"), ("F10", "Quit")],
            )
        } else if ui.bookmarks {
            (
                " Up/Down select | Enter visit | Esc close",
                &[("Enter", "Visit"), ("Esc", "Close"), ("F10", "Quit")],
            )
        } else {
            (
                " Tab pane | Enter open | Space mark | / filter | Ctrl-T shell | F9 menu",
                &shortcuts::FUNCTION_KEYS,
            )
        };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(status, Style::default().fg(Color::Cyan)),
            Line::styled(hint, Style::default().fg(Color::Gray)),
        ])
        .style(Style::default().bg(Color::Black)),
        rows[0],
    );
    shortcuts::draw(frame, rows[1], shortcuts, ui.icons);
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

fn render_pane(frame: &mut ratatui::Frame, pane: &Pane, area: Rect, active: bool, icons: bool) {
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
            let icon = icons::icon(&entry.path, entry.is_dir, entry.is_symlink, icons);
            // Reserve two cells in either mode; Ratatui clips using display width.
            let padding = " ".repeat(2usize.saturating_sub(Span::raw(icon).width()));
            let name = format!(
                "{}{icon}{padding} {}",
                if pane.is_marked(entry) { '*' } else { ' ' },
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
    fn render(app: &App, ui: &BrowserUi, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw_browser(frame, app, ui)).unwrap();
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
        let mut ui = BrowserUi::default();
        app.active_mut().set_filter("before".into());
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        assert!(!handle_browser_key(
            &mut app,
            &mut ui,
            key(KeyCode::Char('q'))
        ));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Tab));
        assert_eq!(app.active, 0);
        assert_eq!(app.panes[0].filter, "beforeq");
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Esc));
        assert_eq!(app.panes[0].filter, "before");
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('é')));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Backspace));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Enter));
        assert!(ui.filter_before.is_none());
        assert_eq!(app.panes[0].filter, "before");
    }
    #[test]
    fn filter_size_is_bounded_and_ctrl_c_exits_from_any_mode() {
        let mut app = app();
        let mut ui = BrowserUi::default();
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        for _ in 0..300 {
            handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('x')));
        }
        assert_eq!(app.panes[0].filter.len(), 256);
        assert!(handle_browser_key(
            &mut app,
            &mut ui,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        assert!(handle_browser_key(&mut app, &mut ui, key(KeyCode::F(10))));
    }
    #[test]
    fn bookmark_sidebar_empty_navigation_and_normal_exit() {
        let mut app = app();
        let mut ui = BrowserUi::default();
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('b')));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Down));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Enter));
        assert!(!ui.bookmarks);
        assert_eq!(app.panes[0].path, PathBuf::from("/mcs-test"));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('B')));
        assert_eq!(app.bookmarks, [PathBuf::from("/mcs-test")]);
        assert!(handle_browser_key(
            &mut app,
            &mut ui,
            key(KeyCode::Char('q'))
        ));
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
        let ui = BrowserUi::default();
        for (width, height) in [(80, 24), (100, 30)] {
            let rows = render(&app, &ui, width, height);
            assert_eq!(
                rows[0],
                "MCS | F3/v view | F4-F8 files | B bookmark; b sidebar | i icons"
            );
            assert!(rows[1].starts_with("┌> /mcs-test | 0 loading"));
            assert!(rows[usize::from(height) - 8].starts_with("│Loading directory;"));
            assert!(rows[usize::from(height) - 5]
                .starts_with(" 0 marked | Name A-Z | Hidden off | History 0/0"));
            let bar = rows[usize::from(height) - 3..].join("\n");
            for shortcut in [
                "F1", "Help", "F2", "User", "F3", "View", "F9", "Menu", "F10", "Quit",
            ] {
                assert!(bar.contains(shortcut), "Missing {shortcut}: {bar}");
            }
        }
        let mut ui = BrowserUi {
            bookmarks: true,
            ..BrowserUi::default()
        };
        for (width, height) in [(1, 1), (20, 5), (80, 24), (100, 30)] {
            render(&app, &ui, width, height);
        }
        ui.bookmarks = false;
        ui.filter_before = Some(String::new());
        let rows = render(&app, &ui, 80, 24);
        assert_eq!(rows[19], "Filter: _");
        assert!(rows[21].contains("Enter"));
        assert!(rows[22].contains("Apply"));
        assert!(rows[21].contains("Esc"));
        assert!(rows[22].contains("Cancel"));
        assert!(!rows[22].contains("View"));
    }

    #[test]
    fn footer_adapts_without_losing_quit_or_leaking_mode_shortcuts() {
        let mut app = app();
        let mut ui = BrowserUi::default();
        for width in [8, 20, 24, 32, 40, 63, 64, 80, 100, 160] {
            let rows = render(&app, &ui, width, 24);
            let bar = rows[21..].join("\n");
            assert!(
                bar.contains("F10") && bar.contains("Quit"),
                "width {width}: {bar}"
            );
        }
        for height in [1, 2, 3, 5] {
            let rows = render(&app, &ui, 80, height);
            let bar = rows.join("\n");
            assert!(bar.contains("F10") && bar.contains("Quit"));
        }
        ui.bookmarks = true;
        let rows = render(&app, &ui, 80, 24);
        assert!(rows[21].contains("Enter"));
        assert!(rows[22].contains("Visit"));
        assert!(rows[21].contains("Esc"));
        assert!(rows[22].contains("Close"));
        assert!(!rows[22].contains("View"));
        ui.bookmarks = false;
        app.active_mut().set_filter("\x1b\n\u{202e}雪".into());
        let rows = render(&app, &ui, 160, 24);
        assert!(rows[19].contains("Filter: \\u{1b}\\n\\u{202e}雪"));

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| draw_browser(frame, &app, &ui))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 21)].symbol(), "╭");
        assert_eq!(buffer[(0, 21)].fg, Color::DarkGray);
        assert_eq!(buffer[(0, 21)].bg, Color::Black);
        let menu = &buffer.content[80 * 21..];
        assert_eq!(
            menu.iter().find(|cell| cell.symbol() == "V").unwrap().fg,
            Color::Cyan
        );
        assert_eq!(
            menu.iter().find(|cell| cell.symbol() == "Q").unwrap().fg,
            Color::Red
        );
    }

    #[test]
    fn category_icons_keep_filename_columns_and_marks_and_respect_input_modes() {
        let mut app = App::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while app.panes.iter().any(Pane::is_loading) {
            assert!(
                std::time::Instant::now() < deadline,
                "repository scan timed out"
            );
            app.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.panes[0].is_empty());
        let mut ui = BrowserUi::default();
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('i')));
        assert!(ui.icons);
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char(' ')));
        for enabled in [true, false] {
            let mut terminal = Terminal::new(TestBackend::new(40, 24)).unwrap();
            terminal
                .draw(|frame| render_pane(frame, &app.panes[0], frame.area(), true, enabled))
                .unwrap();
            let buffer = terminal.backend().buffer();
            for (index, entry) in app.panes[0].visible_entries().take(22).enumerate() {
                let y = index as u16 + 1;
                let first = display_text(&entry.name)
                    .chars()
                    .next()
                    .unwrap()
                    .to_string();
                assert_eq!(buffer[(5, y)].symbol(), first);
                assert_eq!(
                    buffer[(1, y)].symbol(),
                    if app.panes[0].is_marked(entry) {
                        "*"
                    } else {
                        " "
                    }
                );
                assert_eq!(
                    buffer[(2, y)].symbol(),
                    icons::icon(&entry.path, entry.is_dir, entry.is_symlink, enabled)
                );
            }
        }
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('i')));
        assert!(ui.icons);
        assert_eq!(app.panes[0].filter, "i");
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Esc));
        ui.bookmarks = true;
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('i')));
        assert!(ui.icons);
        ui.bookmarks = false;
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('i')));
        assert!(!ui.icons);
        for width in [1, 2, 3, 8, 20, 80] {
            ui.icons = true;
            render(&app, &ui, width, 5);
        }
    }

    #[test]
    fn function_key_menu_icons_and_reserved_operation_styles() {
        let app = app();
        let mut ui = BrowserUi {
            icons: true,
            ..BrowserUi::default()
        };
        for width in [160] {
            let rows = render(&app, &ui, width, 24);
            for icon in ["?", "⚡", "📖", "☰", "⏻"] {
                assert!(
                    rows[22].contains(icon),
                    "width {width}: {icon} missing: {}",
                    rows[22]
                );
            }
            assert!(rows[21].contains("F3"));
            assert!(rows[21].contains("F10"));
        }
        ui.icons = false;
        let rows = render(&app, &ui, 80, 24);
        assert!(rows[22].contains("View"));
        assert!(!rows[22].contains("📖"));
        let mut terminal = Terminal::new(TestBackend::new(160, 24)).unwrap();
        terminal
            .draw(|frame| draw_browser(frame, &app, &ui))
            .unwrap();
        let row = &terminal.backend().buffer().content[160 * 22..160 * 23];
        assert_eq!(
            row.iter().find(|cell| cell.symbol() == "E").unwrap().fg,
            Color::Gray
        );
        let rows = render(&app, &ui, 160, 24);
        for number in 1..=12 {
            assert!(rows[21].contains(&format!(" F{number} ")));
        }
        assert!(rows[22].contains("Copy"));
        assert!(rows[22].contains("Unused"));
    }

    #[test]
    fn function_key_menus_apply_actions_and_help_remains_modal() {
        let mut app = app();
        let mut ui = Ui::default();
        handle_key(&mut app, &mut ui, key(KeyCode::F(1)));
        assert!(matches!(ui.browser.popup, Some(BrowserPopup::Help(0))));
        for code in [KeyCode::F(3), KeyCode::Char('v'), KeyCode::Tab] {
            assert!(!handle_key(&mut app, &mut ui, key(code)));
            assert!(matches!(ui.viewer.screen, viewer::Screen::Browser));
            assert!(ui.browser.popup.is_some());
            assert_eq!(app.active, 0);
        }
        for _ in 0..50 {
            handle_key(&mut app, &mut ui, key(KeyCode::Down));
        }
        assert!(
            matches!(ui.browser.popup, Some(BrowserPopup::Help(index)) if index == BROWSER_HELP.len() - 1)
        );
        for (width, height) in [(1, 1), (20, 5), (80, 24)] {
            render(&app, &ui.browser, width, height);
        }
        assert!(!handle_key(&mut app, &mut ui, key(KeyCode::Char('q'))));
        assert!(ui.browser.popup.is_none());
        for code in [KeyCode::F(2), KeyCode::F(9)] {
            handle_key(&mut app, &mut ui, key(code));
            assert!(matches!(ui.browser.popup, Some(BrowserPopup::Menu(0))));
            handle_key(&mut app, &mut ui, key(KeyCode::Down));
            let hidden = app.panes[0].show_hidden;
            handle_key(&mut app, &mut ui, key(KeyCode::Enter));
            assert_eq!(app.panes[0].show_hidden, !hidden);
            assert!(ui.browser.popup.is_none());
        }
        handle_key(&mut app, &mut ui, key(KeyCode::F(9)));
        handle_key(&mut app, &mut ui, key(KeyCode::Char('/')));
        assert!(ui.browser.filter_before.is_some());
        for number in [1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12] {
            handle_key(&mut app, &mut ui, key(KeyCode::F(number)));
            assert!(ui.browser.popup.is_none());
            assert!(ui.browser.notice.is_none());
            assert!(matches!(ui.viewer.screen, viewer::Screen::Browser));
        }
        handle_key(&mut app, &mut ui, key(KeyCode::Esc));
        for number in [1, 9] {
            handle_key(&mut app, &mut ui, key(KeyCode::F(number)));
            assert!(handle_key(&mut app, &mut ui, key(KeyCode::F(10))));
            handle_key(&mut app, &mut ui, key(KeyCode::Esc));
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn shell_input_precedes_browser_shortcuts_and_preserves_browser_location() {
        let mut app = app();
        let mut ui = Ui::default();
        ui.browser.shells.visible = [true, true];
        let path = app.panes[0].path.clone();
        for code in [
            KeyCode::Char('q'),
            KeyCode::Char('v'),
            KeyCode::Tab,
            KeyCode::F(4),
        ] {
            assert!(!handle_key(&mut app, &mut ui, key(code)));
            assert_eq!(app.active, 0);
            assert!(matches!(ui.viewer.screen, viewer::Screen::Browser));
            assert!(!ui.browser.operations.active());
        }
        assert!(!handle_key(
            &mut app,
            &mut ui,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        handle_key(&mut app, &mut ui, key(KeyCode::F(11)));
        assert_eq!(ui.browser.shells.visible, [true, true]);
        // Crossterm decodes legacy Ctrl-] (0x1d) as Ctrl-5.
        handle_key(
            &mut app,
            &mut ui,
            KeyEvent::new(KeyCode::Char('5'), KeyModifiers::CONTROL),
        );
        assert_eq!(app.active, 1);
        assert!(handle_key(&mut app, &mut ui, key(KeyCode::F(10))));
        handle_key(
            &mut app,
            &mut ui,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
        );
        assert_eq!(ui.browser.shells.visible, [true, false]);
        assert_eq!(app.panes[0].path, path);
        assert_eq!(app.panes[0].selected, 0);
    }

    #[test]
    fn reserved_function_keys_leave_browser_state_untouched() {
        let mut app = app();
        let mut ui = BrowserUi::default();
        let path = app.panes[0].path.clone();
        for number in [11, 12] {
            assert!(!handle_browser_key(
                &mut app,
                &mut ui,
                key(KeyCode::F(number))
            ));
            assert!(ui.notice.is_some());
            assert_eq!(app.panes[0].path, path);
            assert_eq!(app.panes[0].selected, 0);
            assert_eq!(app.panes[0].marked_count(), 0);
            assert_eq!(app.active, 0);
            assert!(app.panes[0].filter.is_empty());
            assert!(ui.popup.is_none());
            assert!(!ui.bookmarks);
            render(&app, &ui, 100, 30);
        }
        handle_browser_key(&mut app, &mut ui, key(KeyCode::Char('i')));
        assert!(ui.notice.is_none());
    }
}
