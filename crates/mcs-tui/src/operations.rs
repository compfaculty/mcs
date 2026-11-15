//! Compact operation dialogs: Enter confirms a displayed destination; delete/save require y.
use crate::{display_text, BrowserUi};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mcs_core::{
    operations::{Action, Edit, Plan, EDIT_LIMIT},
    App,
};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Clear, Paragraph, Wrap},
    Frame,
};
use std::{
    io,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

#[derive(Default)]
pub struct Operations {
    dialog: Option<Dialog>,
    pending: Option<Receiver<io::Result<Outcome>>>,
    error: Option<String>,
}
enum Outcome {
    Confirm(Plan),
    SaveFailed(Edit, String),
    Done(String),
    Editor(Edit),
}
enum Dialog {
    Target {
        action: Action,
        paths: Vec<PathBuf>,
        base: PathBuf,
        input: String,
    },
    Confirm(Plan),
    Editor {
        edit: Edit,
        cursor: usize,
        confirm: bool,
    },
    Message(String),
}
impl Operations {
    pub fn active(&self) -> bool {
        self.dialog.is_some() || self.pending.is_some()
    }
    fn launch(&mut self, job: impl FnOnce() -> io::Result<Outcome> + Send + 'static) {
        let (tx, rx) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("mcs-file-operation".into())
            .spawn(move || {
                let _ = tx.send(job());
            }) {
            Ok(_) => {
                self.pending = Some(rx);
                self.dialog = None;
            }
            Err(error) => self.dialog = Some(Dialog::Message(error.to_string())),
        }
    }
    pub fn poll(&mut self, app: &mut App) -> bool {
        let Some(rx) = &self.pending else {
            return false;
        };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(io::Error::other("Operation worker stopped"))
            }
        };
        self.pending = None;
        self.dialog = Some(match result {
            Ok(Outcome::Confirm(plan)) => Dialog::Confirm(plan),
            Ok(Outcome::SaveFailed(edit, error)) => {
                self.error = Some(error);
                Dialog::Editor {
                    edit,
                    cursor: 0,
                    confirm: false,
                }
            }
            Ok(Outcome::Editor(edit)) => Dialog::Editor {
                edit,
                cursor: 0,
                confirm: false,
            },
            Ok(Outcome::Done(_message)) => {
                for pane in &mut app.panes {
                    pane.refresh();
                }
                self.dialog = None;
                return true;
            }
            Err(error) => {
                for pane in &mut app.panes {
                    pane.refresh();
                }
                Dialog::Message(format!("Error: {error}"))
            }
        });
        true
    }
    pub fn start(&mut self, app: &App, function: u8) {
        self.error = None;
        if app.panes.iter().any(|p| p.is_loading()) {
            self.dialog = Some(Dialog::Message(
                "Wait for directory loading to finish".into(),
            ));
            return;
        }
        let pane = &app.panes[app.active];
        let paths = pane.operation_paths();
        if function == 4 {
            if let Some(entry) = pane.current() {
                let path = entry.path.clone();
                self.launch(move || Edit::open(path).map(Outcome::Editor));
            } else {
                self.dialog = Some(Dialog::Message("No selected file".into()));
            }
            return;
        }
        let action = match function {
            5 => Action::Copy,
            6 => Action::Move,
            7 => Action::Mkdir,
            _ => Action::Delete,
        };
        if action == Action::Delete {
            self.launch(move || Plan::prepare(action, paths, None).map(Outcome::Confirm));
        } else {
            let input = if action == Action::Mkdir {
                String::new()
            } else {
                app.panes[1 - app.active]
                    .path
                    .to_str()
                    .unwrap_or("")
                    .to_owned()
            };
            self.dialog = Some(Dialog::Target {
                action,
                paths: if action == Action::Mkdir {
                    vec![]
                } else {
                    paths
                },
                base: pane.path.clone(),
                input,
            });
        }
    }
    pub fn key(&mut self, key: KeyEvent) {
        if self.pending.is_some() {
            return;
        }
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        self.dialog = match dialog {
            Dialog::Message(message) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                    None
                } else {
                    Some(Dialog::Message(message))
                }
            }
            Dialog::Confirm(plan) => match key.code {
                KeyCode::Char('y') => {
                    self.launch(move || plan.execute().map(Outcome::Done));
                    return;
                }
                KeyCode::Esc | KeyCode::Char('n') => None,
                _ => Some(Dialog::Confirm(plan)),
            },
            Dialog::Target {
                action,
                paths,
                base,
                mut input,
            } => {
                match key.code {
                    KeyCode::Esc => return,
                    KeyCode::Enter => {
                        let target = PathBuf::from(&input);
                        let target = if target.is_absolute() {
                            target
                        } else {
                            base.join(target)
                        };
                        if input.is_empty() {
                            self.dialog =
                                Some(Dialog::Message("Destination cannot be empty".into()));
                        } else {
                            self.launch(move || {
                                Plan::prepare(action, paths, Some(target))?
                                    .execute()
                                    .map(Outcome::Done)
                            });
                        }
                        return;
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.clear()
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Char(ch) if plain(key) && !ch.is_control() && input.len() < 4096 => {
                        input.push(ch)
                    }
                    _ => {}
                }
                Some(Dialog::Target {
                    action,
                    paths,
                    base,
                    input,
                })
            }
            Dialog::Editor {
                mut edit,
                mut cursor,
                mut confirm,
            } => {
                if confirm {
                    match key.code {
                        KeyCode::Char('y') => {
                            self.launch(move || {
                                Ok(match edit.save() {
                                    Ok(message) => Outcome::Done(message),
                                    Err(error) => Outcome::SaveFailed(edit, error.to_string()),
                                })
                            });
                            return;
                        }
                        KeyCode::Esc | KeyCode::Char('n') => confirm = false,
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => return, // explicit discard, never writes
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            confirm = true
                        }
                        KeyCode::Left => cursor = previous(&edit.text, cursor),
                        KeyCode::Right => cursor = next(&edit.text, cursor),
                        KeyCode::Home => {
                            cursor = edit.text[..cursor].rfind('\n').map_or(0, |i| i + 1)
                        }
                        KeyCode::End => {
                            cursor = edit.text[cursor..]
                                .find('\n')
                                .map_or(edit.text.len(), |i| cursor + i)
                        }
                        KeyCode::Up | KeyCode::Down => {
                            let start = edit.text[..cursor].rfind('\n').map_or(0, |i| i + 1);
                            let column = edit.text[start..cursor].chars().count();
                            let row = if key.code == KeyCode::Up {
                                start
                                    .checked_sub(1)
                                    .map(|end| edit.text[..end].rfind('\n').map_or(0, |i| i + 1))
                            } else {
                                edit.text[cursor..].find('\n').map(|i| cursor + i + 1)
                            };
                            if let Some(row) = row {
                                cursor = row;
                                for _ in 0..column {
                                    if cursor == edit.text.len()
                                        || edit.text[cursor..].starts_with('\n')
                                    {
                                        break;
                                    }
                                    cursor = next(&edit.text, cursor);
                                }
                            }
                        }
                        KeyCode::Backspace if cursor > 0 => {
                            let prev = previous(&edit.text, cursor);
                            edit.text.replace_range(prev..cursor, "");
                            cursor = prev;
                        }
                        KeyCode::Delete if cursor < edit.text.len() => {
                            let end = next(&edit.text, cursor);
                            edit.text.replace_range(cursor..end, "");
                        }
                        KeyCode::Enter if edit.text.len() < EDIT_LIMIT as usize => {
                            edit.text.insert(cursor, '\n');
                            cursor += 1;
                        }
                        KeyCode::Tab if edit.text.len() < EDIT_LIMIT as usize => {
                            edit.text.insert(cursor, '\t');
                            cursor += 1;
                        }
                        KeyCode::Char(ch)
                            if plain(key)
                                && !ch.is_control()
                                && edit.text.len() + ch.len_utf8() <= EDIT_LIMIT as usize =>
                        {
                            edit.text.insert(cursor, ch);
                            cursor += ch.len_utf8();
                        }
                        _ => {}
                    }
                }
                Some(Dialog::Editor {
                    edit,
                    cursor,
                    confirm,
                })
            }
        };
    }
    pub fn draw(&self, frame: &mut Frame) {
        if !self.active() {
            return;
        }
        let screen = frame.area();
        let (title, text) = match &self.dialog {
            None => (
                "Working",
                "Filesystem operation running. Exit may leave partial output.".into(),
            ),
            Some(Dialog::Message(text)) => ("Result | Enter/Esc close", display_text(text)),
            Some(Dialog::Target {
                action,
                paths,
                base,
                input,
            }) => {
                let source = if paths.len() == 1 {
                    format!(
                        "{}: {}",
                        action.label(),
                        display_text(&paths[0].file_name().unwrap_or_default().to_string_lossy())
                    )
                } else if paths.is_empty() {
                    "Create directory".into()
                } else {
                    format!(
                        "{} {} marked entries (including hidden marks)",
                        action.label(),
                        paths.len()
                    )
                };
                ("Destination", format!("{source}\nIn: {}\n\nTo: {}▏\n\nExisting folder or unused name. No overwrite.\nCtrl-U clears the path. Relative paths use this pane.\n\n[ Enter: {} ]    [ Esc: Cancel ]", display_text(&base.to_string_lossy()), display_text(input), action.label()))
            }
            Some(Dialog::Confirm(plan)) => (
                "Confirm | y execute | n/Esc cancel (default)",
                plan.description()
                    .lines()
                    .map(display_text)
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            Some(Dialog::Editor {
                edit,
                cursor,
                confirm,
            }) => {
                let line = edit.text[..*cursor].bytes().filter(|c| *c == b'\n').count();
                let skip = line.saturating_sub(usize::from(screen.height.saturating_sub(8)));
                let mut content = edit.text.clone();
                content.insert(*cursor, '▏');
                let content = content
                    .lines()
                    .skip(skip)
                    .take(usize::from(screen.height.saturating_sub(6)))
                    .map(display_text)
                    .collect::<Vec<_>>()
                    .join("\n");
                (
                    if *confirm {
                        "Save changes? y save | n/Esc return to editor"
                    } else {
                        "Edit | Ctrl-S review save | Esc discard | arrows navigate"
                    },
                    format!(
                        "{} | line {} | {} bytes\n{}\n{}",
                        display_text(&edit.path().to_string_lossy()),
                        line + 1,
                        edit.text.len(),
                        self.error.as_deref().map(display_text).unwrap_or_default(),
                        content
                    ),
                )
            }
        };
        let area = if matches!(self.dialog, Some(Dialog::Editor { .. })) {
            Rect::new(
                screen.x + 1.min(screen.width),
                screen.y + 1.min(screen.height),
                screen.width.saturating_sub(2),
                screen.height.saturating_sub(2),
            )
        } else {
            // Size to wrapped content while keeping the browser visible behind the modal.
            let width = screen
                .width
                .saturating_sub(4)
                .clamp(1, 80)
                .min(screen.width);
            let inner_width = usize::from(width.saturating_sub(2).max(1));
            let lines: usize = text
                .lines()
                .map(|line| {
                    ratatui::text::Line::raw(line)
                        .width()
                        .max(1)
                        .div_ceil(inner_width)
                })
                .sum();
            let height = u16::try_from(lines.saturating_add(2))
                .unwrap_or(u16::MAX)
                .min(screen.height.saturating_sub(2).max(1))
                .min(screen.height);
            Rect::new(
                screen.x + (screen.width - width) / 2,
                screen.y + (screen.height - height) / 2,
                width,
                height,
            )
        };
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .block(Block::bordered().title(title))
                .style(Style::default().fg(Color::White).bg(Color::Black)),
            area,
        );
    }
}
fn plain(key: KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}
fn previous(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .char_indices()
        .next_back()
        .map_or(0, |(i, _)| i)
}
fn next(text: &str, cursor: usize) -> usize {
    cursor + text[cursor..].chars().next().map_or(0, char::len_utf8)
}

pub fn handle(app: &mut App, ui: &mut BrowserUi, key: KeyEvent) -> bool {
    if ui.operations.active() {
        ui.operations.key(key);
        true
    } else if ui.filter_before.is_none() && !ui.bookmarks && ui.popup.is_none() {
        if let KeyCode::F(function @ 4..=8) = key.code {
            ui.operations.start(app, function);
            return true;
        }
        false
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };
    fn fixture() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "mcs-ops-ui-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return path,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create operation fixture: {error}"),
            }
        }
    }
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn settle(app: &mut App, ops: &mut Operations) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while ops.pending.is_some() || app.panes.iter().any(|p| p.is_loading()) {
            assert!(Instant::now() < deadline);
            app.poll();
            ops.poll(app);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn destination_enter_confirms_and_escape_cancels_without_writes() {
        let root = fixture();
        let mut app = App::new(root.clone());
        let mut ops = Operations::default();
        settle(&mut app, &mut ops);
        ops.start(&app, 7);
        for ch in "new".chars() {
            ops.key(key(KeyCode::Char(ch)));
        }
        assert!(!root.join("new").exists());
        ops.key(key(KeyCode::Esc));
        assert!(!ops.active());
        assert!(!root.join("new").exists());
        ops.start(&app, 7);
        for ch in "new".chars() {
            ops.key(key(KeyCode::Char(ch)));
        }
        ops.key(key(KeyCode::Enter));
        settle(&mut app, &mut ops);
        assert!(root.join("new").is_dir());
        assert!(!ops.active());
        // A completed deletion must also restore browser input without acknowledgement.
        ops.start(&app, 8);
        settle(&mut app, &mut ops);
        assert!(matches!(ops.dialog, Some(Dialog::Confirm(_))));
        ops.key(key(KeyCode::Char('y')));
        settle(&mut app, &mut ops);
        assert!(!root.join("new").exists());
        assert!(!ops.active());
    }
    #[test]
    fn f6_moves_to_other_pane_on_enter_and_keeps_collision_protection() {
        let root = fixture();
        let dst = root.join("destination");
        fs::create_dir(&dst).unwrap();
        let file = root.join("source.txt");
        fs::write(&file, "keep bytes").unwrap();
        let mut app = App::new(root.clone());
        let mut ops = Operations::default();
        settle(&mut app, &mut ops);
        app.panes[1].visit(dst.clone());
        settle(&mut app, &mut ops);
        app.panes[0].set_filter("source.txt".into());
        ops.start(&app, 6);
        assert!(
            matches!(&ops.dialog, Some(Dialog::Target { action: Action::Move, input, .. }) if input == dst.to_str().unwrap())
        );
        ops.key(key(KeyCode::Esc));
        assert!(file.exists());
        ops.start(&app, 6);
        ops.key(key(KeyCode::Enter));
        settle(&mut app, &mut ops);
        assert!(!file.exists());
        assert_eq!(
            fs::read_to_string(dst.join("source.txt")).unwrap(),
            "keep bytes"
        );
        assert!(!ops.active());
        fs::write(&file, "do not overwrite").unwrap();
        app.panes[0].refresh();
        settle(&mut app, &mut ops);
        ops.start(&app, 6);
        ops.key(key(KeyCode::Enter));
        settle(&mut app, &mut ops);
        assert!(
            matches!(&ops.dialog, Some(Dialog::Message(message)) if message.contains("already exists"))
        );
        assert_eq!(fs::read_to_string(file).unwrap(), "do not overwrite");
        assert_eq!(
            fs::read_to_string(dst.join("source.txt")).unwrap(),
            "keep bytes"
        );
    }
    #[test]
    fn operation_dialogs_are_centered_and_preserve_background() {
        let root = fixture();
        let mut app = App::new(root);
        let mut ops = Operations::default();
        settle(&mut app, &mut ops);
        for function in [5, 6, 7] {
            ops.start(&app, function);
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 50)).unwrap();
            terminal
                .draw(|frame| {
                    frame.render_widget(Paragraph::new("browser background"), frame.area());
                    ops.draw(frame);
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            assert_eq!(buffer[(0, 0)].symbol(), "b");
            let border: Vec<_> = (0..50)
                .flat_map(|y| (0..160).map(move |x| (x, y)))
                .filter(|&(x, y)| buffer[(x, y)].symbol() == "┌")
                .collect();
            assert_eq!(border.len(), 1);
            let (x, y) = border[0];
            assert_eq!(x, 40);
            assert!((15..25).contains(&y));
            let text: String = buffer.content.iter().map(|c| c.symbol()).collect();
            assert!(text.contains("Enter:"));
            assert!(text.contains("Esc: Cancel"));
            for (width, height) in [(1, 1), (20, 5), (80, 24)] {
                let mut terminal =
                    ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                        .unwrap();
                terminal.draw(|frame| ops.draw(frame)).unwrap();
            }
        }
        ops.dialog = Some(Dialog::Message("A small result".into()));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(160, 50)).unwrap();
        terminal.draw(|frame| ops.draw(frame)).unwrap();
        assert_eq!(terminal.backend().buffer()[(40, 23)].symbol(), "┌");
    }
    #[test]
    fn editor_isolated_input_save_confirmation_and_conflict_recovery() {
        let root = fixture();
        let file = root.join("file");
        fs::write(&file, "雪").unwrap();
        let mut app = App::new(root);
        let mut ops = Operations::default();
        settle(&mut app, &mut ops);
        ops.start(&app, 4);
        settle(&mut app, &mut ops);
        ops.key(key(KeyCode::End));
        ops.key(key(KeyCode::Char('q')));
        assert_eq!(fs::read_to_string(&file).unwrap(), "雪");
        ops.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        ops.key(key(KeyCode::Enter));
        assert!(ops.pending.is_none());
        ops.key(key(KeyCode::Char('y')));
        settle(&mut app, &mut ops);
        assert_eq!(fs::read_to_string(&file).unwrap(), "雪q");
        assert!(!ops.active());
        ops.start(&app, 4);
        settle(&mut app, &mut ops);
        ops.key(key(KeyCode::Char('x')));
        fs::write(&file, "external").unwrap();
        ops.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        ops.key(key(KeyCode::Char('y')));
        settle(&mut app, &mut ops);
        assert!(matches!(&ops.dialog, Some(Dialog::Editor { edit, .. }) if edit.text == "x雪q"));
        assert!(ops.error.is_some());
        assert_eq!(fs::read_to_string(file).unwrap(), "external");
    }
}
