//! Browser state. Filesystem scans never run on the UI thread.
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod operations;
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread,
};

pub const MAX_ENTRIES: usize = 100_000;
const MAX_HISTORY: usize = 128;
const MAX_BOOKMARKS: usize = 32;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub issue: Option<String>,
    sort_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Name,
    NameDescending,
}

#[derive(Clone)]
struct Location {
    path: PathBuf,
    selected: Option<PathBuf>,
}

enum Navigation {
    Refresh,
    Visit,
    Back,
    Forward,
}

struct Pending {
    location: Location,
    navigation: Navigation,
}

#[derive(Debug)]
struct Listing {
    entries: Vec<Entry>,
    warning: Option<String>,
}

// One request and one result at most per pane. No thread is spawned per navigation.
struct Loader {
    requests: SyncSender<PathBuf>,
    results: Receiver<io::Result<Listing>>,
}

impl Loader {
    fn new() -> io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<PathBuf>(1);
        let (sender, results) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("mcs-directory".into())
            .spawn(move || {
                while let Ok(path) = receiver.recv() {
                    if sender.send(read_entries(&path)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self { requests, results })
    }
}

pub struct Pane {
    pub path: PathBuf,
    pub selected: usize,
    pub error: Option<String>,
    pub show_hidden: bool,
    pub filter: String,
    pub sort: SortOrder,
    entries: Vec<Entry>,
    visible: Vec<usize>,
    marked: BTreeSet<PathBuf>,
    back: Vec<Location>,
    forward: Vec<Location>,
    loader: Option<Loader>,
    pending: Option<Pending>,
}

impl Pane {
    pub fn new(path: PathBuf) -> Self {
        let loader = Loader::new();
        let error = loader.as_ref().err().map(ToString::to_string);
        let mut pane = Self {
            path,
            selected: 0,
            error,
            show_hidden: false,
            filter: String::new(),
            sort: SortOrder::Name,
            entries: Vec::new(),
            visible: Vec::new(),
            marked: BTreeSet::new(),
            back: Vec::new(),
            forward: Vec::new(),
            loader: loader.ok(),
            pending: None,
        };
        pane.refresh();
        pane
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }
    pub fn len(&self) -> usize {
        self.visible.len()
    }
    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }
    pub fn marked_count(&self) -> usize {
        self.marked.len()
    }
    /// Marks include entries hidden by the current filter. Otherwise use the highlight.
    pub fn operation_paths(&self) -> Vec<PathBuf> {
        if self.marked.is_empty() {
            self.current()
                .map(|entry| vec![entry.path.clone()])
                .unwrap_or_default()
        } else {
            self.marked.iter().cloned().collect()
        }
    }
    pub fn is_marked(&self, entry: &Entry) -> bool {
        self.marked.contains(&entry.path)
    }
    pub fn history_counts(&self) -> (usize, usize) {
        (self.back.len(), self.forward.len())
    }
    pub fn visible_entries(&self) -> impl Iterator<Item = &Entry> {
        self.visible.iter().map(|&index| &self.entries[index])
    }
    pub fn current(&self) -> Option<&Entry> {
        self.visible
            .get(self.selected)
            .map(|&index| &self.entries[index])
    }
    fn location(&self) -> Location {
        Location {
            path: self.path.clone(),
            selected: self.current().map(|entry| entry.path.clone()),
        }
    }
    fn request(&mut self, location: Location, navigation: Navigation) {
        if self.is_loading() {
            return;
        }
        if let Some(loader) = &self.loader {
            match loader.requests.try_send(location.path.clone()) {
                Ok(()) => {
                    self.pending = Some(Pending {
                        location,
                        navigation,
                    });
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("Directory loader unavailable: {e}")),
            }
        }
    }
    /// Commit a scan atomically; a failed visit preserves the previous path and history.
    pub fn poll(&mut self) -> bool {
        if !self.is_loading() {
            return false;
        }
        let result = match self
            .loader
            .as_ref()
            .expect("pending request has loader")
            .results
            .try_recv()
        {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err(io::Error::other("Directory loader stopped")),
        };
        let pending = self.pending.take().expect("checked above");
        match result {
            Ok(listing) => {
                let previous = self.location();
                match pending.navigation {
                    Navigation::Visit => {
                        push_history(&mut self.back, previous);
                        self.forward.clear();
                    }
                    Navigation::Back => {
                        self.back.pop();
                        push_history(&mut self.forward, previous);
                    }
                    Navigation::Forward => {
                        self.forward.pop();
                        push_history(&mut self.back, previous);
                    }
                    Navigation::Refresh => {}
                }
                if self.path != pending.location.path {
                    self.marked.clear();
                }
                self.path = pending.location.path;
                self.entries = listing.entries;
                if !self.marked.is_empty() {
                    let available: BTreeSet<_> =
                        self.entries.iter().map(|entry| &entry.path).collect();
                    self.marked.retain(|path| available.contains(path));
                }
                self.rebuild(pending.location.selected.as_deref());
                self.error = listing.warning;
            }
            Err(e) => self.error = Some(format!("{}: {e}", pending.location.path.display())),
        }
        true
    }
    pub fn refresh(&mut self) {
        self.request(self.location(), Navigation::Refresh);
    }
    pub fn visit(&mut self, path: PathBuf) {
        if path == self.path {
            self.refresh();
        } else {
            self.request(
                Location {
                    path,
                    selected: None,
                },
                Navigation::Visit,
            );
        }
    }
    pub fn back(&mut self) {
        if let Some(location) = self.back.last().cloned() {
            self.request(location, Navigation::Back);
        }
    }
    pub fn forward(&mut self) {
        if let Some(location) = self.forward.last().cloned() {
            self.request(location, Navigation::Forward);
        }
    }
    pub fn up(&mut self) {
        if let Some(parent) = self.path.parent().map(Path::to_path_buf) {
            if parent != self.path {
                self.request(
                    Location {
                        path: parent,
                        selected: Some(self.path.clone()),
                    },
                    Navigation::Visit,
                );
            }
        }
    }
    pub fn enter(&mut self) {
        if let Some(entry) = self.current() {
            if entry.is_dir {
                self.visit(entry.path.clone());
            } else if let Some(issue) = &entry.issue {
                self.error = Some(issue.clone());
            }
        }
    }
    pub fn move_by(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.len().saturating_sub(1));
    }
    pub fn toggle_mark(&mut self) {
        if let Some(path) = self.current().map(|entry| entry.path.clone()) {
            if !self.marked.remove(&path) {
                self.marked.insert(path);
            }
            self.move_by(1);
        }
    }
    pub fn set_filter(&mut self, filter: String) {
        let selected = self.location().selected;
        self.filter = filter;
        self.rebuild(selected.as_deref());
    }
    pub fn toggle_hidden(&mut self) {
        let selected = self.location().selected;
        self.show_hidden = !self.show_hidden;
        self.rebuild(selected.as_deref());
    }
    pub fn toggle_sort(&mut self) {
        let selected = self.location().selected;
        self.sort = match self.sort {
            SortOrder::Name => SortOrder::NameDescending,
            SortOrder::NameDescending => SortOrder::Name,
        };
        self.rebuild(selected.as_deref());
    }
    fn rebuild(&mut self, selected: Option<&Path>) {
        let filter = self.filter.to_lowercase();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                (self.show_hidden || !entry.name.starts_with('.'))
                    && entry.sort_key.contains(&filter)
            })
            .map(|(index, _)| index)
            .collect();
        if self.sort == SortOrder::NameDescending {
            let directories = self
                .visible
                .partition_point(|&index| self.entries[index].is_dir);
            self.visible[..directories].reverse();
            self.visible[directories..].reverse();
        }
        self.selected = selected
            .and_then(|path| {
                self.visible
                    .iter()
                    .position(|&index| self.entries[index].path == path)
            })
            .unwrap_or(0);
    }
}

fn push_history(history: &mut Vec<Location>, location: Location) {
    if history.len() == MAX_HISTORY {
        history.remove(0);
    }
    history.push(location);
}

fn read_entries(path: &Path) -> io::Result<Listing> {
    read_entries_with_limit(path, MAX_ENTRIES)
}

fn read_entries_with_limit(path: &Path, limit: usize) -> io::Result<Listing> {
    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for (index, item) in fs::read_dir(path)?.enumerate() {
        if index == limit {
            return Err(io::Error::other(format!(
                "Directory exceeds {limit} entries; listing not loaded"
            )));
        }
        let item = match item {
            Ok(item) => item,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let file_type = match item.file_type() {
            Ok(kind) => kind,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let is_symlink = file_type.is_symlink();
        let (is_dir, issue) = if is_symlink {
            match fs::metadata(item.path()) {
                Ok(metadata) => (metadata.is_dir(), None),
                Err(e) => (false, Some(format!("Symlink target unavailable: {e}"))),
            }
        } else {
            (file_type.is_dir(), None)
        };
        let name = item.file_name().to_string_lossy().into_owned();
        entries.push(Entry {
            sort_key: name.to_lowercase(),
            name,
            path: item.path(),
            is_dir,
            is_symlink,
            issue,
        });
    }
    entries.sort_unstable_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.sort_key.cmp(&b.sort_key))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(Listing {
        entries,
        warning: (skipped > 0)
            .then(|| format!("{skipped} entries unavailable during scan; refresh to retry")),
    })
}

pub struct App {
    pub panes: [Pane; 2],
    pub active: usize,
    pub bookmarks: Vec<PathBuf>,
}
impl App {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            panes: [Pane::new(cwd.clone()), Pane::new(cwd)],
            active: 0,
            bookmarks: Vec::new(),
        }
    }
    pub fn active_mut(&mut self) -> &mut Pane {
        &mut self.panes[self.active]
    }
    pub fn switch(&mut self) {
        self.active = 1 - self.active;
    }
    pub fn poll(&mut self) -> bool {
        let left = self.panes[0].poll();
        self.panes[1].poll() || left
    }
    pub fn toggle_bookmark(&mut self) {
        let path = self.panes[self.active].path.clone();
        if let Some(index) = self.bookmarks.iter().position(|bookmark| bookmark == &path) {
            self.bookmarks.remove(index);
        } else if self.bookmarks.len() < MAX_BOOKMARKS {
            self.bookmarks.push(path);
        } else {
            self.active_mut().error = Some(format!(
                "Limit of {MAX_BOOKMARKS} session bookmarks reached"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_limit_rejects_partial_listing() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(read_entries_with_limit(path, 0)
            .unwrap_err()
            .to_string()
            .contains("exceeds 0 entries"));
    }
    #[test]
    fn history_storage_is_bounded() {
        let mut history = Vec::new();
        for index in 0..MAX_HISTORY + 10 {
            push_history(
                &mut history,
                Location {
                    path: PathBuf::from(index.to_string()),
                    selected: None,
                },
            );
        }
        assert_eq!(history.len(), MAX_HISTORY);
        assert_eq!(history[0].path, PathBuf::from("10"));
    }
    #[test]
    fn pending_scan_does_not_queue_unbounded_requests() {
        let (requests, receiver) = mpsc::sync_channel(1);
        let (sender, results) = mpsc::sync_channel(1);
        let mut pane = Pane::new(PathBuf::from("unused"));
        pane.loader = Some(Loader { requests, results });
        pane.pending = None;
        pane.refresh();
        for _ in 0..1000 {
            pane.visit(PathBuf::from("ignored"));
        }
        assert_eq!(receiver.try_recv().unwrap(), PathBuf::from("unused"));
        assert!(receiver.try_recv().is_err());
        assert!(!pane.poll());
        pane.move_by(1); // Input state remains available while the worker has no result.
        sender
            .send(Err(io::Error::other("simulated I/O failure")))
            .unwrap();
        assert!(pane.poll());
        assert!(!pane.is_loading());
        assert!(pane.error.is_some());
    }
}
