//! Startup-only bounded probing. Never leave a detached reader competing with Crossterm.
use ratatui_image::{
    picker::cap_parser::{Parser, Response},
    FontSize,
};

pub struct GraphicsCapabilities {
    pub font: FontSize,
    pub compression: bool,
}

struct Replies {
    parser: Parser,
    kitty: bool,
    compression: bool,
    font: Option<FontSize>,
    status: bool,
}
impl Replies {
    fn new() -> Self {
        Self {
            parser: Parser::new(),
            kitty: false,
            compression: false,
            font: None,
            status: false,
        }
    }
    fn push(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            for response in self.parser.push(char::from(byte)) {
                match response {
                    Response::Kitty => self.kitty = true,
                    Response::KittyCompression => self.compression = true,
                    Response::CellSize(Some((width, height))) if width > 0 && height > 0 => {
                        self.font = Some(FontSize::new(width, height))
                    }
                    Response::Status => self.status = true,
                    _ => {}
                }
            }
        }
    }
    fn font(&self) -> Option<FontSize> {
        self.font.filter(|_| self.kitty)
    }
}

#[cfg(unix)]
pub fn probe() -> Option<GraphicsCapabilities> {
    use mio::{unix::SourceFd, Events, Interest, Poll, Token};
    use std::{
        io::{self, IsTerminal, Read, Write},
        os::fd::AsRawFd,
        time::{Duration, Instant},
    };
    let stdin = io::stdin();
    if !stdin.is_terminal() || !io::stdout().is_terminal() || std::env::var_os("TMUX").is_some() {
        return None;
    }
    let mut poll = Poll::new().ok()?;
    let fd = stdin.as_raw_fd();
    poll.registry()
        .register(&mut SourceFd(&fd), Token(0), Interest::READABLE)
        .ok()?;
    let query = Parser::query(
        false,
        ratatui_image::picker::cap_parser::QueryStdioOptions {
            kitty_compression: true,
            ..Default::default()
        },
    );
    io::stdout().write_all(query.as_bytes()).ok()?;
    io::stdout().flush().ok()?;
    let deadline = Instant::now() + Duration::from_millis(300);
    let mut events = Events::with_capacity(4);
    let mut input = stdin.lock();
    let mut replies = Replies::new();
    let mut received = 0;
    // Called only after raw-mode entry and before any event reader starts.
    // Readiness is consumed by this thread alone; a raw TTY read returns available bytes.
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match poll.poll(&mut events, Some(remaining)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        if events.is_empty() {
            break;
        }
        let mut buffer = [0; 512];
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                received += count;
                replies.push(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        if received >= 4096 || (replies.status && replies.font().is_some()) {
            break;
        }
    }
    replies.font().map(|font| GraphicsCapabilities {
        font,
        compression: replies.compression,
    })
}

#[cfg(not(unix))]
pub fn probe() -> Option<GraphicsCapabilities> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_and_reordered_replies_confirm_actual_support() {
        let mut replies = Replies::new();
        for byte in b"\x1b[0n\x1b[6;40;20t\x1b_Gi=31;OK\x1b\\" {
            replies.push(&[*byte]);
        }
        let font = replies.font().unwrap();
        assert_eq!((font.width, font.height), (20, 40));
    }
    #[test]
    fn compression_requires_its_own_positive_reply() {
        let mut replies = Replies::new();
        replies.push(b"\x1b_Gi=31;OK\x1b\\\x1b[6;40;20t\x1b[0n");
        assert!(replies.font().is_some());
        assert!(!replies.compression);
        replies.push(b"\x1b_Gi=32;ENOENT\x1b\\");
        assert!(!replies.compression);
        replies.push(b"\x1b_Gi=32;OK\x1b\\");
        assert!(replies.compression);
    }
    #[test]
    fn missing_or_rejected_graphics_and_invalid_metrics_stay_text_only() {
        for bytes in [
            b"\x1b[6;40;20t\x1b[0n".as_slice(),
            b"\x1b_Gi=31;ENOENT\x1b\\\x1b[6;40;20t",
            b"\x1b_Gi=31;OK\x1b\\\x1b[6;0;20t",
        ] {
            let mut replies = Replies::new();
            replies.push(bytes);
            assert!(replies.font().is_none());
        }
    }
}
