//! Pane-local PTYs. Child output is parsed into cells, never forwarded to the host terminal.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};
use std::{
    io::{self, Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread,
};

#[derive(Default)]
pub struct Shells {
    sessions: [Option<Session>; 2],
    pub visible: [bool; 2],
    areas: [Rect; 2],
    errors: [Option<String>; 2],
}

impl Shells {
    pub fn toggle(&mut self, pane: usize, cwd: &Path) {
        if self.visible[pane] {
            self.visible[pane] = false;
            return;
        }
        if self.sessions[pane].is_none() {
            match Session::spawn(cwd, self.areas[pane]) {
                Ok(session) => {
                    self.sessions[pane] = Some(session);
                    self.errors[pane] = None;
                }
                Err(error) => self.errors[pane] = Some(error.to_string()),
            }
        }
        self.visible[pane] = true;
    }

    pub fn resize(&mut self, areas: [Rect; 2]) {
        self.areas = areas;
        for (index, session) in self.sessions.iter_mut().enumerate() {
            if let Some(session) = session {
                if let Err(error) = session.resize(areas[index]) {
                    self.errors[index] = Some(error.to_string());
                }
            }
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut dirty = false;
        for (index, slot) in self.sessions.iter_mut().enumerate() {
            if let Some(session) = slot {
                // A bounded amount of output per tick keeps navigation responsive under floods.
                for _ in 0..8 {
                    match session.output.try_recv() {
                        Ok(Ok(bytes)) => {
                            session.parser.process(&bytes);
                            dirty = true;
                        }
                        Ok(Err(error)) => {
                            self.errors[index] = Some(error);
                            dirty = true;
                        }
                        Err(_) => break,
                    }
                }
                match session.child.as_mut().expect("owned child").try_wait() {
                    Ok(Some(status)) => {
                        *slot = None;
                        self.visible[index] = false;
                        self.errors[index] = Some(format!(
                            "Shell exited ({status}); Ctrl-T starts a new session."
                        ));
                        dirty = true;
                    }
                    Err(error) => {
                        self.errors[index] = Some(error.to_string());
                        *slot = None;
                        self.visible[index] = false;
                        dirty = true;
                    }
                    Ok(None) => {}
                }
            }
        }
        dirty
    }

    pub fn notice(&self, pane: usize) -> Option<&str> {
        if self.sessions[pane].is_none() {
            self.errors[pane].as_deref()
        } else {
            None
        }
    }

    pub fn key(&mut self, pane: usize, key: KeyEvent) {
        if let Some(session) = &self.sessions[pane] {
            let bytes = encode_key(key, session.parser.screen().application_cursor());
            if !bytes.is_empty() {
                match session.input.try_send(bytes) {
                    Ok(()) => self.errors[pane] = None,
                    Err(error) => {
                        self.errors[pane] = Some(format!("Shell input not sent: {error}"))
                    }
                }
            }
        }
    }

    pub fn draw(&self, frame: &mut Frame, pane: usize, area: Rect, active: bool) {
        let title = self.errors[pane]
            .as_ref()
            .map(|error| super::display_text(error))
            .unwrap_or_else(|| "Shell | Ctrl-T files | Ctrl-] pane".into());
        let block = Block::bordered()
            .title(title)
            .border_style(Style::default().fg(if active { Color::Cyan } else { Color::Gray }));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if let Some(session) = &self.sessions[pane] {
            render_screen(frame, inner, session.parser.screen(), active);
        } else if let Some(error) = &self.errors[pane] {
            frame.render_widget(
                Paragraph::new(format!(
                    "{}\n\nCtrl-T: return to files; toggle again to retry.",
                    super::display_text(error)
                ))
                .wrap(Wrap { trim: false }),
                inner,
            );
        }
    }
}

// vt100/vte's std-enabled OSC buffer is unbounded. Discard unsupported OSC
// strings before parsing (including split writes), without buffering their payload.
// All cursor/SGR/alternate-screen parsing remains delegated to vt100.
struct ScreenParser {
    inner: vt100::Parser,
    osc: bool,
    escape: bool,
}
impl ScreenParser {
    fn new(rows: u16, cols: u16) -> Self {
        Self {
            inner: vt100::Parser::new(rows, cols, 0),
            osc: false,
            escape: false,
        }
    }
    fn screen(&self) -> &vt100::Screen {
        self.inner.screen()
    }
    fn screen_mut(&mut self) -> &mut vt100::Screen {
        self.inner.screen_mut()
    }
    fn process(&mut self, bytes: &[u8]) {
        let mut filtered = Vec::with_capacity(bytes.len() + 1);
        for &byte in bytes {
            if self.osc {
                if matches!(byte, 7 | 24 | 26) || (self.escape && byte == b'\\') {
                    self.osc = false;
                    self.escape = false;
                } else {
                    self.escape = byte == 27;
                }
                continue;
            }
            if self.escape {
                self.escape = false;
                if byte == b']' {
                    // Cancel any preceding partial sequence so stripping cannot splice
                    // an earlier ESC onto a later bare ']' and re-enter OSC in vte.
                    filtered.push(24);
                    self.osc = true;
                    continue;
                }
                filtered.push(27);
            }
            if byte == 27 {
                self.escape = true;
            } else {
                filtered.push(byte);
            }
        }
        self.inner.process(&filtered);
    }
}

struct Session {
    child: Option<Box<dyn Child + Send + Sync>>,
    master: Box<dyn MasterPty + Send>,
    input: SyncSender<Vec<u8>>,
    output: Receiver<Result<Vec<u8>, String>>,
    parser: ScreenParser,
    size: (u16, u16),
    stopped: Arc<AtomicBool>,
}

fn size(area: Rect) -> (u16, u16) {
    // Bound parser allocation even if a hostile outer terminal reports extreme dimensions.
    (
        area.height.saturating_sub(2).clamp(1, 200),
        area.width.saturating_sub(2).clamp(1, 500),
    )
}
fn pty_size((rows, cols): (u16, u16)) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}
fn pty_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

impl Session {
    fn spawn(cwd: &Path, area: Rect) -> io::Result<Self> {
        let size = size(area);
        let pair = native_pty_system()
            .openpty(pty_size(size))
            .map_err(pty_error)?;
        let mut reader = pair.master.try_clone_reader().map_err(pty_error)?;
        let mut writer = pair.master.take_writer().map_err(pty_error)?;
        // Duplicated PTY handles share status flags. Nonblocking I/O plus cancellation
        // lets workers stop even if a detached process retains the slave descriptor.
        use nix::fcntl::{fcntl, FcntlArg, OFlag};
        let fd = pair
            .master
            .as_raw_fd()
            .ok_or_else(|| io::Error::other("PTY descriptor unavailable"))?;
        let flags = fcntl(fd, FcntlArg::F_GETFL).map_err(pty_error)?;
        fcntl(
            fd,
            FcntlArg::F_SETFL(OFlag::from_bits_truncate(flags) | OFlag::O_NONBLOCK),
        )
        .map_err(pty_error)?;
        let shell = std::env::var_os("SHELL")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        // No sh -c, path interpolation, privilege change, or automatic command execution.
        let mut command = CommandBuilder::new(shell);
        command.arg("-i");
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        command.env("MCS_EMBEDDED_SHELL", "1");
        command.env_remove("TERM_PROGRAM");
        command.env_remove("KITTY_WINDOW_ID");
        let child = pair.slave.spawn_command(command).map_err(pty_error)?;
        drop(pair.slave);
        let (output_tx, output) = mpsc::sync_channel(8);
        let (input, input_rx) = mpsc::sync_channel::<Vec<u8>>(32);
        let stopped = Arc::new(AtomicBool::new(false));
        let read_stopped = Arc::clone(&stopped);
        let write_stopped = Arc::clone(&stopped);
        let write_output = output_tx.clone();
        let session = Self {
            child: Some(child),
            master: pair.master,
            input,
            output,
            parser: ScreenParser::new(size.0, size.1),
            size,
            stopped,
        };
        thread::Builder::new()
            .name("mcs-shell-read".into())
            .spawn(move || {
                let mut bytes = [0; 8192];
                while !read_stopped.load(Ordering::Relaxed) {
                    match reader.read(&mut bytes) {
                        Ok(0) => break,
                        Ok(len) => {
                            if output_tx.send(Ok(bytes[..len].to_vec())).is_err() {
                                break;
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(std::time::Duration::from_millis(20));
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => {
                            let _ = output_tx.send(Err(error.to_string()));
                            break;
                        }
                    }
                }
            })?;
        thread::Builder::new()
            .name("mcs-shell-write".into())
            .spawn(move || {
                while !write_stopped.load(Ordering::Relaxed) {
                    let bytes = match input_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                        Ok(bytes) => bytes,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    let mut remaining = bytes.as_slice();
                    while !remaining.is_empty() && !write_stopped.load(Ordering::Relaxed) {
                        match writer.write(remaining) {
                            Ok(0) => return,
                            Ok(len) => remaining = &remaining[len..],
                            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(std::time::Duration::from_millis(20))
                            }
                            Err(error) => {
                                let _ = write_output.send(Err(error.to_string()));
                                return;
                            }
                        }
                    }
                }
            })?;
        Ok(session)
    }
    fn resize(&mut self, area: Rect) -> io::Result<()> {
        let size = size(area);
        if self.size != size {
            self.master.resize(pty_size(size)).map_err(pty_error)?;
            self.parser.screen_mut().set_size(size.0, size.1);
            self.size = size;
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(mut child) = self.child.take() {
            if !matches!(child.try_wait(), Ok(Some(_))) {
                // Only signal the foreground group of our owned PTY and the unreaped shell.
                // Do not wait on the UI thread or allow an HUP-ignoring shell to survive.
                use nix::{
                    sys::signal::{kill, killpg, Signal},
                    unistd::Pid,
                };
                if let Some(group) = self.master.process_group_leader().filter(|pid| *pid > 0) {
                    let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
                }
                if let Some(pid) = child.process_id() {
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
                }
                let _ = thread::Builder::new()
                    .name("mcs-shell-reap".into())
                    .spawn(move || {
                        let _ = child.wait();
                    });
            }
        }
    }
}

fn color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}
fn render_screen(frame: &mut Frame, area: Rect, screen: &vt100::Screen, active: bool) {
    let buffer = frame.buffer_mut();
    for y in 0..area.height {
        for x in 0..area.width {
            if let Some(cell) = screen.cell(y, x) {
                if cell.is_wide_continuation() {
                    continue;
                }
                let mut style = Style::default()
                    .fg(color(cell.fgcolor()))
                    .bg(color(cell.bgcolor()));
                for (enabled, modifier) in [
                    (cell.bold(), Modifier::BOLD),
                    (cell.dim(), Modifier::DIM),
                    (cell.italic(), Modifier::ITALIC),
                    (cell.underline(), Modifier::UNDERLINED),
                    (cell.inverse(), Modifier::REVERSED),
                ] {
                    if enabled {
                        style = style.add_modifier(modifier);
                    }
                }
                // Filter any nonprinting/bidi characters even after VT parsing.
                let text: String = cell.contents().chars().filter(|c| !c.is_control() && !matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')).collect();
                buffer.set_stringn(
                    area.x + x,
                    area.y + y,
                    if text.is_empty() { " " } else { &text },
                    usize::from(area.width - x),
                    style,
                );
            }
        }
    }
    let (y, x) = screen.cursor_position();
    if active && !screen.hide_cursor() && x < area.width && y < area.height {
        frame.set_cursor_position((area.x + x, area.y + y));
    }
}

fn encode_key(key: KeyEvent, application_cursor: bool) -> Vec<u8> {
    let mut bytes = match key.code {
        KeyCode::Char(ch) if key.modifiers.contains(KeyModifiers::CONTROL) => match ch {
            'a'..='z' => vec![ch as u8 - b'a' + 1],
            'A'..='Z' => vec![ch as u8 - b'A' + 1],
            ' ' | '@' => vec![0],
            '[' => vec![27],
            '\\' => vec![28],
            ']' => vec![29],
            '^' => vec![30],
            '_' => vec![31],
            _ => vec![],
        },
        KeyCode::Char(ch) => ch.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![127],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![27],
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Right
        | KeyCode::Left
        | KeyCode::Home
        | KeyCode::End => {
            let suffix = match key.code {
                KeyCode::Up => 'A',
                KeyCode::Down => 'B',
                KeyCode::Right => 'C',
                KeyCode::Left => 'D',
                KeyCode::Home => 'H',
                _ => 'F',
            };
            if key
                .modifiers
                .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT | KeyModifiers::CONTROL)
            {
                let modifier = 1
                    + u8::from(key.modifiers.contains(KeyModifiers::SHIFT))
                    + 2 * u8::from(key.modifiers.contains(KeyModifiers::ALT))
                    + 4 * u8::from(key.modifiers.contains(KeyModifiers::CONTROL));
                format!("\x1b[1;{modifier}{suffix}").into_bytes()
            } else {
                format!("\x1b{}{suffix}", if application_cursor { 'O' } else { '[' }).into_bytes()
            }
        }
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::F(n @ 1..=4) => vec![27, b'O', b'P' + n - 1],
        KeyCode::F(n @ 5..=12) => format!(
            "\x1b[{}~",
            [15, 17, 18, 19, 20, 21, 23, 24][usize::from(n - 5)]
        )
        .into_bytes(),
        _ => vec![],
    };
    if key.modifiers.contains(KeyModifiers::ALT) && matches!(key.code, KeyCode::Char(_)) {
        bytes.insert(0, 27);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn keyboard_preserves_shell_controls_unicode_and_application_cursor() {
        let key = |code, modifiers| KeyEvent::new(code, modifiers);
        assert_eq!(
            encode_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL), false),
            [3]
        );
        assert_eq!(
            encode_key(key(KeyCode::Char('d'), KeyModifiers::CONTROL), false),
            [4]
        );
        assert_eq!(
            encode_key(key(KeyCode::Tab, KeyModifiers::NONE), false),
            [9]
        );
        assert_eq!(
            encode_key(key(KeyCode::Char('雪'), KeyModifiers::NONE), false),
            "雪".as_bytes()
        );
        assert_eq!(
            encode_key(key(KeyCode::Char('b'), KeyModifiers::ALT), false),
            b"\x1bb"
        );
        assert_eq!(
            encode_key(key(KeyCode::Up, KeyModifiers::NONE), true),
            b"\x1bOA"
        );
        assert_eq!(
            encode_key(key(KeyCode::Left, KeyModifiers::CONTROL), false),
            b"\x1b[1;5D"
        );
    }

    #[test]
    fn unterminated_osc_is_discarded_across_chunks_without_accumulation() {
        let mut parser = ScreenParser::new(2, 20);
        parser.process(b"before\x1b");
        parser.process(b"]52;c;");
        for _ in 0..1024 {
            parser.process(&[b'x'; 8192]);
        }
        assert!(parser.osc);
        assert_eq!(parser.screen().contents(), "before");
        parser.process(b"\x1b");
        parser.process(b"\\after");
        assert!(!parser.osc);
        assert_eq!(parser.screen().contents(), "beforeafter");
        let mut parser = ScreenParser::new(2, 20);
        parser.process(b"\x1b\x1b]discard\x07]visible");
        assert_eq!(parser.screen().contents(), "]visible");
    }

    #[test]
    fn parsed_output_is_clipped_and_cannot_control_the_host_terminal() {
        let mut parser = ScreenParser::new(3, 8);
        parser.process(b"\x1b]52;c;dG9rZW4=\x07\x1b[31mred\x1b[0m\r\n\xe9\x9b\xaa\r\nlast");
        let mut terminal = Terminal::new(TestBackend::new(20, 8)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(
                    ratatui::widgets::Paragraph::new("neighbor"),
                    Rect::new(12, 0, 8, 1),
                );
                render_screen(frame, Rect::new(1, 1, 8, 3), parser.screen(), true);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(1, 1)].symbol(), "r");
        assert_eq!(buffer[(1, 1)].fg, Color::Indexed(1));
        assert_eq!(buffer[(1, 2)].symbol(), "雪");
        assert_eq!(buffer[(12, 0)].symbol(), "n");
        for cell in &buffer.content {
            assert!(!cell.symbol().chars().any(char::is_control));
        }
        assert_eq!(size(Rect::new(0, 0, u16::MAX, u16::MAX)), (200, 500));
        assert_eq!(size(Rect::default()), (1, 1));
        parser.process(b"\x1b[?1049hfull\x1b[?1049l");
        assert!(parser.screen().contents().contains("red"));
    }
}
