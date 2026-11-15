use memchr::{memchr, memchr_iter, memrchr};
use std::{
    fs::{File, Metadata, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};
use unicode_width::UnicodeWidthChar;

const CHUNK: usize = 128 * 1024;
pub const SEGMENT: usize = 64 * 1024;
const MAX_INDEX: usize = 16 * 1024 * 1024 / std::mem::size_of::<(u64, u64)>();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    pub width: usize,
    pub height: usize,
    pub wrap: bool,
    pub horizontal: usize,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 80,
            height: 20,
            wrap: false,
            horizontal: 0,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub offset: u64,
    pub line: Option<u64>,
    pub text: String,
    pub continued: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub viewport: Viewport,
    pub rows: Vec<Row>,
    pub top: u64,
    pub len: u64,
    pub indexed: u64,
    pub message: String,
    pub query: String,
}
#[derive(Clone, Debug)]
pub enum Movement {
    Stay,
    Home,
    End,
    Rows(isize),
    Search { query: String, reverse: bool },
}

pub struct Document {
    file: File,
    len: u64,
    modified: Option<SystemTime>,
    start: u64,
    top: u64,
    index: Vec<(u64, u64)>,
    indexed: u64,
    lines: u64,
    stride: u64,
    last_match: Option<u64>,
    query: String,
}
fn changed(metadata: &Metadata, len: u64, modified: Option<SystemTime>) -> bool {
    metadata.len() != len || metadata.modified().ok() != modified
}
fn cancelled(cancel: &AtomicU64, generation: u64) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) != generation {
        Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled"))
    } else {
        Ok(())
    }
}

impl Document {
    pub fn open(path: &Path) -> io::Result<Self> {
        // Reject special files before opening (in particular FIFOs). Recheck the opened handle.
        if !std::fs::metadata(path)?.is_file() {
            return Err(io::Error::other("Viewer accepts regular files only"));
        }
        let mut options = OpenOptions::new();
        options.read(true);
        // A path can become a FIFO between metadata and open. Nonblocking open prevents a hang.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let mut file = options.open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::other("Not a regular file"));
        }
        let mut sample = vec![0; metadata.len().min(8192) as usize];
        file.read_exact(&mut sample)?;
        if sample.contains(&0) {
            return Err(io::Error::other(
                "Binary/UTF-16 content is unsupported in the text viewer",
            ));
        }
        let start = if sample.starts_with(&[0xef, 0xbb, 0xbf]) {
            3
        } else {
            0
        };
        Ok(Self {
            file,
            len: metadata.len(),
            modified: metadata.modified().ok(),
            start,
            top: start,
            index: vec![(start, 1)],
            indexed: start,
            lines: 1,
            stride: 1024 * 1024,
            last_match: None,
            query: String::new(),
        })
    }
    pub fn set_top(&mut self, top: u64) {
        self.top = top.clamp(self.start, self.len.max(self.start));
    }
    pub fn progress(&self) -> u64 {
        self.indexed
            .saturating_mul(100)
            .checked_div(self.len)
            .unwrap_or(100)
    }
    fn validate(&self) -> io::Result<()> {
        if changed(&self.file.metadata()?, self.len, self.modified) {
            Err(io::Error::other("File changed; press r to reload"))
        } else {
            Ok(())
        }
    }
    fn read_at(&mut self, offset: u64, count: usize) -> io::Result<Vec<u8>> {
        self.file.seek(SeekFrom::Start(offset))?;
        let count = (self.len.saturating_sub(offset)).min(count as u64) as usize;
        let mut data = vec![0; count];
        self.file.read_exact(&mut data)?;
        Ok(data)
    }
    /// One bounded indexing step; the worker checks requests between steps.
    pub fn index_step(&mut self) -> io::Result<bool> {
        if self.indexed >= self.len {
            return Ok(false);
        }
        self.validate()?;
        let bytes = self.read_at(self.indexed, CHUNK)?;
        self.lines += memchr_iter(b'\n', &bytes).count() as u64;
        self.indexed += bytes.len() as u64;
        if self
            .indexed
            .saturating_sub(self.index.last().expect("initial checkpoint").0)
            >= self.stride
        {
            if self.index.len() == MAX_INDEX {
                self.index = self.index.iter().step_by(2).copied().collect();
                self.stride *= 2;
            }
            self.index.push((self.indexed, self.lines));
        }
        Ok(true)
    }
    fn line_at(&mut self, offset: u64) -> io::Result<Option<u64>> {
        if offset > self.indexed {
            return Ok(None);
        }
        let index = self
            .index
            .partition_point(|&(byte, _)| byte <= offset)
            .saturating_sub(1);
        let (mut byte, mut line) = self.index[index];
        while byte < offset {
            let bytes = self.read_at(byte, (offset - byte).min(CHUNK as u64) as usize)?;
            line += memchr_iter(b'\n', &bytes).count() as u64;
            byte += bytes.len() as u64;
        }
        Ok(Some(line))
    }
    fn next(&mut self, offset: u64, viewport: Viewport) -> io::Result<(u64, String, bool)> {
        let data = self.read_at(offset, SEGMENT)?;
        if data.is_empty() {
            return Ok((offset, String::new(), false));
        }
        let newline = memchr(b'\n', &data);
        let end = newline.unwrap_or(data.len());
        let mut used = end;
        // Do not split a valid UTF-8 character at the bounded read edge.
        if newline.is_none() && offset + (end as u64) < self.len {
            if let Err(error) = std::str::from_utf8(&data[..end]) {
                if error.error_len().is_none() && error.valid_up_to() > 0 {
                    used = error.valid_up_to();
                }
            }
        }
        let raw = &data[..used];
        let mut text = String::new();
        let mut cells = 0;
        let mut consumed = 0;
        let width = viewport.width.clamp(1, 1024);
        let mut column = 0;
        let mut byte = 0;
        while byte < raw.len() {
            let first = raw[byte];
            let length = match first {
                0..=0x7f => 1,
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 1,
            };
            let decoded = raw
                .get(byte..byte + length)
                .and_then(|slice| std::str::from_utf8(slice).ok())
                .and_then(|value| value.chars().next());
            let (ch, length) = decoded.map(|ch| (ch, length)).unwrap_or(('\u{fffd}', 1));
            let escaped = if ch == '\r' && byte + length == raw.len() && newline.is_some() {
                String::new()
            } else {
                display_text(&ch.to_string())
            };
            let size = escaped
                .chars()
                .map(|ch| ch.width().unwrap_or(0))
                .sum::<usize>();
            if viewport.wrap
                && (cells + size > width || text.len() + escaped.len() > 8192)
                && consumed > 0
            {
                break;
            }
            if (viewport.wrap || column >= viewport.horizontal.min(SEGMENT))
                && cells + size <= width
                && text.len() + escaped.len() <= 8192
            {
                text.push_str(&escaped);
                cells += size;
            }
            column += size;
            byte += length;
            consumed = byte;
        }
        if !viewport.wrap {
            consumed = used;
        }
        if consumed == 0 && used > 0 {
            consumed = used.min(1);
        }
        let complete = consumed == end && newline.is_some();
        let next = offset + consumed as u64 + u64::from(complete);
        Ok((next, text, !complete && next < self.len))
    }
    fn previous(&mut self, offset: u64, viewport: Viewport) -> io::Result<u64> {
        if offset <= self.start {
            return Ok(self.start);
        }
        let begin = offset.saturating_sub(SEGMENT as u64).max(self.start);
        let bytes = self.read_at(begin, (offset - begin) as usize)?;
        let without_final_newline = if bytes.last() == Some(&b'\n') {
            &bytes[..bytes.len() - 1]
        } else {
            &bytes
        };
        let line_start = memrchr(b'\n', without_final_newline)
            .map(|index| begin + index as u64 + 1)
            .unwrap_or(begin);
        if !viewport.wrap {
            return Ok(line_start);
        }
        let mut cursor = line_start;
        let mut previous = line_start;
        while cursor < offset {
            previous = cursor;
            let next = self.next(cursor, viewport)?.0;
            if next <= cursor {
                break;
            }
            cursor = next;
        }
        Ok(previous)
    }
    fn search(
        &mut self,
        query: &str,
        reverse: bool,
        cancel: &AtomicU64,
        generation: u64,
        progress: &mut impl FnMut(u64, u64),
    ) -> io::Result<Option<u64>> {
        let needle = query.as_bytes();
        if needle.is_empty() {
            return Ok(None);
        }
        if query.chars().count() > 256 {
            return Err(io::Error::other("Query exceeds 256 characters"));
        }
        let origin = self
            .last_match
            .filter(|_| self.query == query)
            .unwrap_or(self.top);
        let split = if reverse {
            origin
        } else {
            origin
                .saturating_add(u64::from(self.last_match.is_some() && self.query == query))
                .min(self.len)
        };
        let ranges = if reverse {
            [(self.start, split), (split, self.len)]
        } else {
            [(split, self.len), (self.start, split)]
        };
        let mut scanned = 0u64;
        for (begin, end) in ranges {
            if reverse {
                let mut stop = end;
                while stop > begin {
                    progress(scanned.min(self.len), self.len);
                    scanned = scanned.saturating_add(CHUNK as u64);
                    cancelled(cancel, generation)?;
                    self.validate()?;
                    let start = stop.saturating_sub(CHUNK as u64).max(begin);
                    let data = self.read_at(
                        start,
                        (end - start).min(CHUNK as u64 + needle.len() as u64 - 1) as usize,
                    )?;
                    let mut length = data.len();
                    while let Some(index) = memchr::memmem::rfind(&data[..length], needle) {
                        let found = start + index as u64;
                        if found < stop {
                            return Ok(Some(found));
                        }
                        length = index + needle.len() - 1;
                    }
                    stop = start;
                }
            } else {
                let mut cursor = begin;
                while cursor < end {
                    progress(scanned.min(self.len), self.len);
                    scanned = scanned.saturating_add(CHUNK as u64);
                    cancelled(cancel, generation)?;
                    self.validate()?;
                    let data = self.read_at(
                        cursor,
                        (self.len - cursor).min(CHUNK as u64 + needle.len() as u64 - 1) as usize,
                    )?;
                    if let Some(index) = memchr::memmem::find(&data, needle) {
                        let found = cursor + index as u64;
                        if found < end {
                            return Ok(Some(found));
                        }
                    }
                    cursor = cursor.saturating_add(CHUNK as u64).min(end);
                }
            }
        }
        Ok(None)
    }
    pub fn page(
        &mut self,
        movement: Movement,
        viewport: Viewport,
        cancel: &AtomicU64,
        generation: u64,
    ) -> io::Result<Page> {
        self.page_with_progress(movement, viewport, cancel, generation, |_, _| {})
    }
    pub fn page_with_progress(
        &mut self,
        movement: Movement,
        viewport: Viewport,
        cancel: &AtomicU64,
        generation: u64,
        mut progress: impl FnMut(u64, u64),
    ) -> io::Result<Page> {
        self.validate()?;
        cancelled(cancel, generation)?;
        let mut message = String::new();
        match movement {
            Movement::Stay => {}
            Movement::Home => self.top = self.start,
            Movement::End => {
                self.top = self.len;
                for _ in 0..viewport.height.clamp(1, 256) {
                    self.top = self.previous(self.top, viewport)?;
                }
            }
            Movement::Rows(delta) => {
                for _ in 0..delta.unsigned_abs().min(256) {
                    cancelled(cancel, generation)?;
                    let next = if delta < 0 {
                        self.previous(self.top, viewport)?
                    } else {
                        self.next(self.top, viewport)?.0
                    };
                    if next < self.len {
                        self.top = next;
                    }
                }
            }
            Movement::Search { query, reverse } => {
                let found = self.search(&query, reverse, cancel, generation, &mut progress)?;
                self.query = query;
                self.last_match = found;
                if let Some(found) = found {
                    let begin = found.saturating_sub(SEGMENT as u64).max(self.start);
                    let bytes = self.read_at(begin, (found - begin) as usize)?;
                    self.top = memrchr(b'\n', &bytes)
                        .map(|i| begin + i as u64 + 1)
                        .unwrap_or(found);
                    if found.saturating_sub(self.top) > viewport.width as u64 / 2 {
                        self.top = found;
                    }
                    message = format!("Match at byte {found}");
                } else {
                    message = "No match (searched entire file)".into();
                }
            }
        }
        let mut offset = self.top;
        let mut rows = Vec::new();
        for _ in 0..viewport.height.clamp(1, 256) {
            cancelled(cancel, generation)?;
            if offset >= self.len {
                break;
            }
            let (next, text, continued) = self.next(offset, viewport)?;
            let line = self.line_at(offset)?;
            if text.contains('\u{fffd}') {
                message = "Invalid UTF-8 displayed with replacement characters".into();
            }
            rows.push(Row {
                offset,
                text,
                line,
                continued,
            });
            if next <= offset {
                break;
            }
            offset = next;
        }
        self.validate()?;
        Ok(Page {
            viewport,
            rows,
            top: self.top,
            len: self.len,
            indexed: self.indexed,
            message,
            query: self.query.clone(),
        })
    }
}

pub fn display_text(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        if ch.is_control()
            || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            output.extend(ch.escape_default());
        } else {
            output.push(ch);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::OpenOptions, io::Write, sync::atomic::AtomicUsize};
    fn fixture(bytes: &[u8]) -> std::path::PathBuf {
        static ID: AtomicUsize = AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "mcs-text-{}-{}",
                std::process::id(),
                ID.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(bytes).unwrap();
                    return path;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
    }
    fn page(doc: &mut Document, movement: Movement, viewport: Viewport) -> Page {
        doc.page(movement, viewport, &AtomicU64::new(1), 1).unwrap()
    }
    #[test]
    fn navigation_wrap_crlf_bom_and_numbers() {
        let mut doc =
            Document::open(&fixture("\u{feff}hello\r\n雪abcdef\nlast".as_bytes())).unwrap();
        while doc.index_step().unwrap() {}
        let view = Viewport {
            width: 4,
            height: 2,
            wrap: true,
            horizontal: 0,
        };
        let first = page(&mut doc, Movement::Home, view);
        assert_eq!(first.rows[0].text, "hell");
        assert_eq!(first.rows[1].text, "o");
        assert_eq!(first.rows[0].line, Some(1));
        let next = page(&mut doc, Movement::Rows(2), view);
        assert_eq!(next.rows[0].text, "雪ab");
        let previous = page(&mut doc, Movement::Rows(-2), view);
        assert_eq!(previous.top, first.top);
        let end = page(&mut doc, Movement::End, view);
        assert!(end.rows.iter().any(|row| row.text == "last"));
    }
    #[test]
    fn literal_search_crosses_chunks_and_wraps_both_ways() {
        let mut bytes = vec![b'a'; CHUNK - 2];
        bytes.extend_from_slice(b"NEEDLE\nsecond NEEDLE\n");
        let mut doc = Document::open(&fixture(&bytes)).unwrap();
        let search = |reverse| Movement::Search {
            query: "NEEDLE".into(),
            reverse,
        };
        let first = page(&mut doc, search(false), Viewport::default());
        assert!(first.message.contains(&(CHUNK - 2).to_string()));
        let second = page(&mut doc, search(false), Viewport::default());
        assert_ne!(first.message, second.message);
        assert_eq!(
            page(&mut doc, search(false), Viewport::default()).message,
            first.message
        );
        assert_eq!(
            page(&mut doc, search(true), Viewport::default()).message,
            second.message
        );
        assert_eq!(
            page(&mut doc, search(true), Viewport::default()).message,
            first.message
        );
    }
    #[test]
    fn huge_single_line_is_bounded_and_utf8_boundary_is_preserved() {
        let mut bytes = vec![b'x'; SEGMENT - 1];
        bytes.extend_from_slice("雪".as_bytes());
        bytes.extend(vec![b'x'; SEGMENT]);
        let mut doc = Document::open(&fixture(&bytes)).unwrap();
        let first = page(&mut doc, Movement::Home, Viewport::default());
        assert!(first.rows.iter().all(|row| row.text.len() <= 1024));
        assert!(first.rows[0].continued);
        assert_eq!(first.rows[1].offset, (SEGMENT - 1) as u64);
        assert!(first.rows[1].text.starts_with('雪'));
    }
    #[test]
    fn cancellation_mutation_binary_and_lossy_text() {
        let path = fixture(b"good\xffbad\n");
        let mut doc = Document::open(&path).unwrap();
        let view = Viewport {
            width: 3,
            wrap: true,
            ..Viewport::default()
        };
        let result = page(&mut doc, Movement::Home, view);
        assert!(result.rows.iter().any(|row| row.text.contains('\u{fffd}')));
        assert!(doc
            .page(Movement::End, view, &AtomicU64::new(2), 1)
            .is_err());
        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"new")
            .unwrap();
        assert!(doc
            .page(Movement::Stay, view, &AtomicU64::new(1), 1)
            .unwrap_err()
            .to_string()
            .contains("changed"));
        assert!(Document::open(&fixture(b"a\0b")).is_err());
        assert!(Document::open(Path::new(env!("CARGO_MANIFEST_DIR"))).is_err());
    }
    #[test]
    fn index_compaction_stays_within_budget() {
        let mut doc = Document::open(&fixture(b"text\n")).unwrap();
        doc.index = vec![(0, 1); MAX_INDEX];
        doc.stride = 1;
        doc.index_step().unwrap();
        assert!(doc.index.len() <= MAX_INDEX);
        assert!(doc.index.capacity() * std::mem::size_of::<(u64, u64)>() <= 16 * 1024 * 1024);
        assert_eq!(doc.stride, 2);
    }
    #[test]
    fn multigigabyte_file_is_seeked_not_loaded() {
        let path = fixture(&vec![b'x'; 8192]);
        let mut file = OpenOptions::new().write(true).open(&path).unwrap();
        let size = 3 * 1024 * 1024 * 1024u64;
        file.set_len(size).unwrap();
        file.seek(SeekFrom::End(-5)).unwrap();
        file.write_all(b"\ntail").unwrap();
        let mut doc = Document::open(&path).unwrap();
        let end = page(
            &mut doc,
            Movement::End,
            Viewport {
                height: 1,
                ..Viewport::default()
            },
        );
        assert_eq!(end.len, size);
        assert_eq!(end.rows[0].text, "tail");
        assert_eq!(doc.index.len(), 1);
    }
}
