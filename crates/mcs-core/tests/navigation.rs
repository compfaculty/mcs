use mcs_core::{App, Pane};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

// Only uniquely created fixtures are modified. Existing files are never overwritten or deleted.
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "mcs-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create fixture: {error}"),
            }
        }
    }
    fn file(&self, name: &str) {
        fs::File::create_new(self.0.join(name)).unwrap();
    }
    fn dir(&self, name: &str) {
        fs::create_dir(self.0.join(name)).unwrap();
    }
    fn pane(&self) -> Pane {
        let mut pane = Pane::new(self.0.clone());
        settle(&mut pane);
        pane
    }
}
// Leave fixtures in the system temp directory: tests do not delete files.
fn settle(pane: &mut Pane) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while pane.is_loading() {
        pane.poll();
        assert!(Instant::now() < deadline, "directory scan timed out");
        thread::sleep(Duration::from_millis(1));
    }
}
fn names(pane: &Pane) -> Vec<&str> {
    pane.visible_entries()
        .map(|entry| entry.name.as_str())
        .collect()
}
fn select(pane: &mut Pane, name: &str) {
    let index = pane
        .visible_entries()
        .position(|entry| entry.name == name)
        .unwrap();
    pane.selected = index;
}

#[test]
fn sort_filter_hidden_and_marks_preserve_selection() {
    let fixture = Fixture::new();
    fixture.dir("z-dir");
    fixture.file("alpha");
    fixture.file("Beta");
    fixture.file(".secret");
    let mut pane = fixture.pane();
    assert_eq!(names(&pane), ["z-dir", "alpha", "Beta"]);
    select(&mut pane, "alpha");
    pane.toggle_sort();
    assert_eq!(names(&pane), ["z-dir", "Beta", "alpha"]);
    assert_eq!(pane.current().unwrap().name, "alpha");
    pane.toggle_mark();
    assert_eq!(pane.marked_count(), 1);
    pane.set_filter("bE".into());
    assert_eq!(names(&pane), ["Beta"]);
    pane.set_filter("no match".into());
    pane.move_by(isize::MAX);
    assert!(pane.current().is_none());
    assert_eq!(pane.selected, 0);
    pane.set_filter(String::new());
    pane.toggle_hidden();
    assert!(names(&pane).contains(&".secret"));
    assert_eq!(pane.marked_count(), 1);
    select(&mut pane, "alpha");
    pane.toggle_mark();
    assert_eq!(pane.marked_count(), 0);
}

#[test]
fn history_restores_highlight_and_new_visit_discards_forward_history() {
    let fixture = Fixture::new();
    fixture.dir("a");
    fixture.dir("b");
    let mut pane = fixture.pane();
    select(&mut pane, "b");
    pane.enter();
    settle(&mut pane);
    assert_eq!(pane.path, fixture.0.join("b"));
    pane.back();
    settle(&mut pane);
    assert_eq!(pane.current().unwrap().name, "b");
    pane.forward();
    settle(&mut pane);
    assert_eq!(pane.path, fixture.0.join("b"));
    pane.up();
    settle(&mut pane);
    assert_eq!(pane.current().unwrap().name, "b");
    pane.back();
    settle(&mut pane);
    pane.back();
    settle(&mut pane);
    select(&mut pane, "a");
    pane.enter();
    settle(&mut pane);
    assert_eq!(pane.history_counts().1, 0);
    assert_eq!(pane.marked_count(), 0);
}

#[test]
fn failed_visit_preserves_path_entries_selection_and_history() {
    let fixture = Fixture::new();
    fixture.file("file");
    let mut pane = fixture.pane();
    pane.visit(fixture.0.join("missing"));
    settle(&mut pane);
    assert_eq!(pane.path, fixture.0);
    assert_eq!(names(&pane), ["file"]);
    assert_eq!(pane.history_counts(), (0, 0));
    assert!(pane.error.is_some());
    pane.visit(fixture.0.join("file"));
    settle(&mut pane);
    assert!(pane.error.is_some());
    assert_eq!(pane.path, fixture.0);
    pane.refresh();
    settle(&mut pane);
    assert!(pane.error.is_none());
}

#[test]
fn failed_back_does_not_consume_history() {
    let fixture = Fixture::new();
    fixture.dir("child");
    let mut pane = fixture.pane();
    pane.enter();
    settle(&mut pane);
    // Rename the owned directory to simulate a stale history path, without deleting anything.
    let renamed = fixture.0.with_extension("renamed");
    fs::create_dir(&renamed).unwrap(); // Reserve an exclusively owned, empty destination.
    fs::rename(&fixture.0, &renamed).unwrap();
    pane.back();
    settle(&mut pane);
    assert!(pane.error.is_some());
    assert_eq!(pane.history_counts(), (1, 0));
    fs::rename(renamed, &fixture.0).unwrap();
    pane.back();
    settle(&mut pane);
    assert_eq!(pane.path, fixture.0);
}

#[test]
fn refresh_preserves_path_based_selection_and_marks() {
    let fixture = Fixture::new();
    fixture.file("b");
    fixture.file("c");
    let mut pane = fixture.pane();
    select(&mut pane, "c");
    pane.toggle_mark();
    fixture.file("a");
    pane.refresh();
    settle(&mut pane);
    assert_eq!(pane.current().unwrap().name, "c");
    assert_eq!(pane.marked_count(), 1);
}

#[test]
fn empty_and_missing_directories_keep_selection_safe() {
    let fixture = Fixture::new();
    let mut pane = fixture.pane();
    pane.move_by(isize::MAX);
    pane.move_by(isize::MIN);
    pane.enter();
    pane.toggle_mark();
    assert!(pane.is_empty());
    assert_eq!(pane.selected, 0);
    let mut missing = Pane::new(fixture.0.join("absent"));
    settle(&mut missing);
    assert!(missing.error.is_some());
    assert!(missing.current().is_none());
}

#[test]
fn bookmarks_are_session_only_unique_and_bounded() {
    let fixture = Fixture::new();
    let mut app = App::new(fixture.0.clone());
    app.toggle_bookmark();
    app.toggle_bookmark();
    assert!(app.bookmarks.is_empty());
    for index in 0..33 {
        app.active_mut().path = fixture.0.join(index.to_string());
        app.toggle_bookmark();
    }
    assert_eq!(app.bookmarks.len(), 32);
    assert!(app.panes[0].error.is_some());
    app.switch();
    assert_eq!(app.active, 1);
}

#[cfg(unix)]
#[test]
fn symlink_directory_and_broken_link() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fixture.dir("target");
    symlink("target", fixture.0.join("link")).unwrap();
    symlink("absent", fixture.0.join("broken")).unwrap();
    let mut pane = fixture.pane();
    select(&mut pane, "broken");
    pane.enter();
    assert!(pane.error.is_some());
    select(&mut pane, "link");
    assert!(pane.current().unwrap().is_symlink);
    pane.enter();
    settle(&mut pane);
    assert_eq!(pane.path, fixture.0.join("link"));
    pane.up();
    settle(&mut pane);
    assert_eq!(pane.current().unwrap().name, "link");
}

#[cfg(unix)]
#[test]
fn permission_failure_preserves_previous_location() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    fixture.dir("private");
    let private = fixture.0.join("private");
    let mut pane = fixture.pane();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o0)).unwrap();
    let denied = fs::read_dir(&private).is_err();
    pane.enter();
    settle(&mut pane);
    fs::set_permissions(private, fs::Permissions::from_mode(0o700)).unwrap();
    if denied {
        assert_eq!(pane.path, fixture.0);
        assert!(pane.error.is_some());
    } else {
        eprintln!("Permission denial not enforceable for this user; denial assertions skipped");
    }
}

#[test]
fn large_directory_loads_and_navigation_remains_bounded() {
    let fixture = Fixture::new();
    for index in 0..20_000 {
        fixture.file(&format!("item-{index:05}"));
    }
    let start = Instant::now();
    let mut pane = fixture.pane();
    eprintln!("20,000-entry scan+commit: {:?}", start.elapsed());
    assert_eq!(pane.len(), 20_000);
    pane.move_by(isize::MAX);
    assert_eq!(pane.current().unwrap().name, "item-19999");
    pane.move_by(isize::MIN);
    assert_eq!(pane.selected, 0);
    pane.set_filter("19999".into());
    assert_eq!(pane.len(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_filename_keeps_original_path_identity() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let fixture = Fixture::new();
    let raw = fixture.0.join(OsString::from_vec(vec![0xff, b'a']));
    fs::File::create_new(&raw).unwrap();
    let pane = fixture.pane();
    assert!(pane.visible_entries().any(|entry| entry.path == raw));
}
