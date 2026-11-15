//! Confirmed operations on pinned parent directories. Never traverse child symlinks.
//! These APIs must only be executed after the UI presents the captured plan.
use rustix::fs::{self as at, AtFlags, FileType, Mode, OFlags, RenameFlags, Stat};
use std::{
    ffi::OsString,
    fs::File,
    io::{self, Read, Write},
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
    path::{Component, Path, PathBuf},
};

pub const EDIT_LIMIT: u64 = 1024 * 1024;
const MAX_ITEMS: usize = 100_000;
const MAX_DEPTH: usize = 64;
const COPY_LIMIT: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Copy,
    Move,
    Mkdir,
    Delete,
}
impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Move => "Move",
            Self::Mkdir => "Mkdir",
            Self::Delete => "Delete permanently",
        }
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}
fn directory(path: &Path) -> io::Result<OwnedFd> {
    Ok(at::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
}
fn child_dir(parent: &OwnedFd, name: &std::ffi::OsStr) -> io::Result<OwnedFd> {
    Ok(at::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
}
fn snapshot(parent: &OwnedFd, name: &std::ffi::OsStr) -> io::Result<Stat> {
    Ok(at::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?)
}
fn same(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}
struct Source {
    parent: OwnedFd,
    name: OsString,
    state: Stat,
    path: PathBuf,
}
impl Source {
    fn new(path: PathBuf) -> io::Result<Self> {
        let name = path
            .file_name()
            .ok_or_else(|| invalid("Root/parent entries are forbidden"))?
            .to_owned();
        let parent = directory(path.parent().ok_or_else(|| invalid("Missing parent"))?)?;
        let state = snapshot(&parent, &name)?;
        Ok(Self {
            parent,
            name,
            state,
            path,
        })
    }
    fn check(&self) -> io::Result<()> {
        if !same(&self.state, &snapshot(&self.parent, &self.name)?) {
            return Err(invalid(
                "Source changed since confirmation; reopen the dialog",
            ));
        }
        Ok(())
    }
}

pub struct Plan {
    pub action: Action,
    sources: Vec<Source>,
    destination: Option<(OwnedFd, PathBuf, OsString)>,
}
impl Plan {
    /// Copy/move to an exact unused path for one entry, or an existing directory for a batch.
    pub fn prepare(
        action: Action,
        paths: Vec<PathBuf>,
        target: Option<PathBuf>,
    ) -> io::Result<Self> {
        if paths.len() > MAX_ITEMS {
            return Err(invalid("Too many selected entries"));
        }
        let sources: Vec<_> = paths
            .into_iter()
            .map(Source::new)
            .collect::<io::Result<_>>()?;
        if action != Action::Mkdir && sources.is_empty() {
            return Err(invalid("No selected entry"));
        }
        let destination = if action == Action::Delete {
            None
        } else {
            let path = target.ok_or_else(|| invalid("Destination required"))?;
            if path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            {
                return Err(invalid("Use a path without . or .. components"));
            }
            if action != Action::Mkdir && path.is_dir() {
                Some((directory(&path)?, path, OsString::new()))
            } else {
                if sources.len() > 1 {
                    return Err(invalid(
                        "Multiple entries require an existing destination directory",
                    ));
                }
                let name = path
                    .file_name()
                    .ok_or_else(|| invalid("Invalid destination name"))?
                    .to_owned();
                let parent = path
                    .parent()
                    .ok_or_else(|| invalid("Missing destination parent"))?;
                Some((directory(parent)?, parent.to_owned(), name))
            }
        };
        if let Some((_, parent, name)) = &destination {
            let parent = parent.canonicalize()?;
            for source in &sources {
                if FileType::from_raw_mode(source.state.st_mode) == FileType::Directory
                    && parent.starts_with(source.path.canonicalize()?)
                {
                    return Err(invalid("Cannot copy/move a directory into itself"));
                }
                let dest = parent.join(if name.is_empty() { &source.name } else { name });
                if dest.symlink_metadata().is_ok() {
                    return Err(invalid(
                        "Destination already exists; overwriting is disabled",
                    ));
                }
            }
        }
        Ok(Self {
            action,
            sources,
            destination,
        })
    }
    pub fn description(&self) -> String {
        let mut text = format!(
            "{} {} entry/entries\n",
            self.action.label(),
            if self.action == Action::Mkdir {
                1
            } else {
                self.sources.len()
            }
        );
        for source in self.sources.iter().take(12) {
            text.push_str(&format!("{:?}\n", source.path));
        }
        if self.sources.len() > 12 {
            text.push_str("... additional marked entries\n");
        }
        if let Some((_, path, name)) = &self.destination {
            text.push_str(&format!("To: {:?}\n", path.join(name)));
        }
        text.push_str("No overwrite. Batches may partially complete on failure.");
        if self.action == Action::Delete {
            text.push_str("\nIncludes directories recursively; no undo.");
        }
        text
    }
    pub fn execute(self) -> io::Result<String> {
        for source in &self.sources {
            source.check()?;
        }
        let mut budget = Budget { items: 0, bytes: 0 };
        if self.action == Action::Mkdir {
            let (parent, _, name) = self
                .destination
                .as_ref()
                .ok_or_else(|| invalid("Missing target"))?;
            at::mkdirat(parent, name, Mode::from_raw_mode(0o755))?;
        }
        for (index, source) in self.sources.iter().enumerate() {
            let result = (|| -> io::Result<()> {
                source.check()?;
                match self.action {
                    Action::Delete => delete(&source.parent, &source.name, 0, &mut budget),
                    Action::Copy | Action::Move => {
                        let (parent, _, name) = self
                            .destination
                            .as_ref()
                            .ok_or_else(|| invalid("Missing target"))?;
                        let name = if name.is_empty() { &source.name } else { name };
                        if self.action == Action::Move {
                            // Atomic no-clobber, including directories. EXDEV is deliberately an error.
                            at::renameat_with(
                                &source.parent,
                                &source.name,
                                parent,
                                name,
                                RenameFlags::NOREPLACE,
                            )?;
                            Ok(())
                        } else {
                            copy(&source.parent, &source.name, parent, name, 0, &mut budget)
                        }
                    }
                    Action::Mkdir => Ok(()),
                }
            })();
            if let Err(error) = result {
                return Err(invalid(&format!("{} completed before failure at {}: {error}. Partial output may remain; refresh before retrying.", index, source.path.display())));
            }
        }
        Ok(format!("{} completed", self.action.label()))
    }
}
struct Budget {
    items: usize,
    bytes: u64,
}
impl Budget {
    fn visit(&mut self, depth: usize) -> io::Result<()> {
        self.items += 1;
        if self.items > MAX_ITEMS || depth > MAX_DEPTH {
            return Err(invalid(
                "Operation exceeds 100,000 entries or 64 directory levels",
            ));
        }
        Ok(())
    }
}
fn names(fd: &OwnedFd) -> io::Result<impl Iterator<Item = io::Result<OsString>>> {
    Ok(at::Dir::read_from(fd)?.filter_map(|entry| match entry {
        Err(error) => Some(Err(error.into())),
        Ok(entry) => {
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                None
            } else {
                Some(Ok(std::ffi::OsStr::from_bytes(name).to_owned()))
            }
        }
    }))
}

fn copy(
    src: &OwnedFd,
    name: &std::ffi::OsStr,
    dst: &OwnedFd,
    out: &std::ffi::OsStr,
    depth: usize,
    budget: &mut Budget,
) -> io::Result<()> {
    budget.visit(depth)?;
    let state = snapshot(src, name)?;
    match FileType::from_raw_mode(state.st_mode) {
        FileType::Directory => {
            let input = child_dir(src, name)?;
            if !same(&state, &at::fstat(&input)?) {
                return Err(invalid("Directory changed while opening"));
            }
            at::mkdirat(dst, out, Mode::from_raw_mode(0o700))?;
            let output = child_dir(dst, out)?;
            for name in names(&input)? {
                let name = name?;
                copy(&input, &name, &output, &name, depth + 1, budget)?;
            }
            at::fchmod(&output, Mode::from_raw_mode(state.st_mode & 0o777))?;
        }
        FileType::RegularFile => {
            let fd = at::openat(
                src,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )?;
            if !same(&state, &at::fstat(&fd)?) {
                return Err(invalid("Source changed while opening"));
            }
            let mut input = File::from(fd);
            let fd = at::openat(
                dst,
                out,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )?;
            let mut output = File::from(fd);
            let remaining = COPY_LIMIT.saturating_sub(budget.bytes);
            let count = io::copy(&mut (&mut input).take(remaining + 1), &mut output)?;
            budget.bytes += count;
            if budget.bytes > COPY_LIMIT {
                return Err(invalid("Copy exceeds 16 GiB per operation"));
            }
            output.sync_all()?;
            at::fchmod(&output, Mode::from_raw_mode(state.st_mode & 0o777))?;
            if !same(&state, &at::fstat(&input)?) {
                return Err(invalid("Source changed during copy"));
            }
        }
        FileType::Symlink => {
            let target = at::readlinkat(src, name, Vec::new())?;
            at::symlinkat(&target, dst, out)?;
        }
        _ => {
            return Err(invalid(
                "Special files (devices, sockets, FIFOs) cannot be copied",
            ))
        }
    }
    Ok(())
}
fn delete(
    parent: &OwnedFd,
    name: &std::ffi::OsStr,
    depth: usize,
    budget: &mut Budget,
) -> io::Result<()> {
    budget.visit(depth)?;
    if FileType::from_raw_mode(snapshot(parent, name)?.st_mode) == FileType::Directory {
        let state = snapshot(parent, name)?;
        let fd = child_dir(parent, name)?;
        if !same(&state, &at::fstat(&fd)?) {
            return Err(invalid("Directory changed while opening"));
        }
        if at::fstat(parent)?.st_dev != state.st_dev {
            return Err(invalid(
                "Recursive deletion across mount points is disabled",
            ));
        }
        for name in names(&fd)? {
            let name = name?;
            delete(&fd, &name, depth + 1, budget)?;
        }
        at::unlinkat(parent, name, AtFlags::REMOVEDIR)?;
    } else {
        at::unlinkat(parent, name, AtFlags::empty())?;
    }
    Ok(())
}

pub struct Edit {
    source: Source,
    pub text: String,
}
impl Edit {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let source = Source::new(path)?;
        if FileType::from_raw_mode(source.state.st_mode) != FileType::RegularFile
            || source.state.st_size < 0
            || source.state.st_size as u64 > EDIT_LIMIT
        {
            return Err(invalid(
                "Editor accepts regular UTF-8 files up to 1 MiB; symlinks are rejected",
            ));
        }
        if source.state.st_nlink != 1 {
            return Err(invalid("Editing multiply hard-linked files is disabled"));
        }
        let fd = at::openat(
            &source.parent,
            &source.name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        if !same(&source.state, &at::fstat(&fd)?) {
            return Err(invalid("File changed while opening"));
        }
        let mut text = String::new();
        File::from(fd)
            .take(EDIT_LIMIT + 1)
            .read_to_string(&mut text)?;
        if text.len() as u64 > EDIT_LIMIT
            || text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(invalid("Binary or oversized file cannot be edited"));
        }
        source.check()?;
        Ok(Self { source, text })
    }
    pub fn path(&self) -> &Path {
        &self.source.path
    }
    pub fn save(&self) -> io::Result<String> {
        self.source.check()?;
        if self.text.len() as u64 > EDIT_LIMIT {
            return Err(invalid("Editor limit exceeded"));
        }
        // Private sibling staging file; retain original until the complete replacement is synced.
        let mut staging = None;
        for n in 0..1000 {
            let name = format!(".mcs-edit-{}-{n}", std::process::id());
            match at::openat(
                &self.source.parent,
                &name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            ) {
                Ok(fd) => {
                    staging = Some((name, File::from(fd)));
                    break;
                }
                Err(error) if error == rustix::io::Errno::EXIST => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let (name, mut file) =
            staging.ok_or_else(|| invalid("Cannot allocate editor staging file"))?;
        let result = (|| -> io::Result<()> {
            file.write_all(self.text.as_bytes())?;
            at::fchmod(
                &file,
                Mode::from_raw_mode(self.source.state.st_mode & 0o777),
            )?;
            file.sync_all()?;
            self.source.check()?;
            at::renameat(
                &self.source.parent,
                &name,
                &self.source.parent,
                &self.source.name,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            return match at::unlinkat(&self.source.parent, &name, AtFlags::empty()) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(invalid(&format!(
                    "{error}; staging cleanup failed for {name}: {cleanup}"
                ))),
            };
        }
        Ok("File saved".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
        sync::atomic::{AtomicUsize, Ordering},
    };
    fn fixture() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "mcs-ops-{}-{}",
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
    fn run(action: Action, source: &Path, target: Option<PathBuf>) -> io::Result<String> {
        Plan::prepare(action, vec![source.to_owned()], target)?.execute()
    }
    #[test]
    fn copy_move_mkdir_and_recursive_delete_never_follow_symlinks() {
        let root = fixture();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("file"), "hello").unwrap();
        fs::create_dir(source.join("sub")).unwrap();
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep"), "safe").unwrap();
        symlink(&outside, source.join("link")).unwrap();
        symlink("missing", source.join("broken")).unwrap();
        let copy = root.join("copy");
        run(Action::Copy, &source, Some(copy.clone())).unwrap();
        assert_eq!(fs::read_to_string(copy.join("file")).unwrap(), "hello");
        assert_eq!(fs::read_link(copy.join("link")).unwrap(), outside);
        assert_eq!(
            fs::read_link(copy.join("broken")).unwrap(),
            PathBuf::from("missing")
        );
        let moved = root.join("moved");
        run(Action::Move, &copy, Some(moved.clone())).unwrap();
        assert!(!copy.exists());
        run(Action::Delete, &moved, None).unwrap();
        assert!(!moved.exists());
        assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "safe");
        Plan::prepare(Action::Mkdir, vec![], Some(root.join("new")))
            .unwrap()
            .execute()
            .unwrap();
        assert!(root.join("new").is_dir());
    }
    #[test]
    fn collisions_and_changed_sources_and_unsafe_targets_are_rejected() {
        let root = fixture();
        let source = root.join("file");
        fs::write(&source, "source").unwrap();
        let target = root.join("target");
        fs::write(&target, "keep").unwrap();
        for action in [Action::Copy, Action::Move] {
            assert!(run(action, &source, Some(target.clone())).is_err());
        }
        let plan =
            Plan::prepare(Action::Move, vec![source.clone()], Some(root.join("later"))).unwrap();
        fs::write(root.join("later"), "racing destination").unwrap();
        assert!(plan.execute().is_err());
        assert!(source.exists());
        let plan = Plan::prepare(Action::Delete, vec![source.clone()], None).unwrap();
        fs::write(&source, "different length").unwrap();
        assert!(plan.execute().is_err());
        let dir = root.join("dir");
        fs::create_dir(&dir).unwrap();
        assert!(run(Action::Copy, &dir, Some(dir.join("nested"))).is_err());
        assert!(Plan::prepare(Action::Mkdir, vec![], Some(root.join("../bad"))).is_err());
        assert!(Plan::prepare(Action::Delete, vec![PathBuf::from("/")], None).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "keep");
    }
    #[test]
    fn editor_validates_input_conflicts_and_preserves_mode() {
        let root = fixture();
        let file = root.join("edit");
        fs::write(&file, "hello\n雪").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        let mut edit = Edit::open(file.clone()).unwrap();
        edit.text = "new text\n".into();
        edit.save().unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "new text\n");
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let mut edit = Edit::open(file.clone()).unwrap();
        edit.text = "discard me".into();
        fs::write(&file, "external modification").unwrap();
        assert!(edit.save().is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "external modification");
        symlink(&file, root.join("symlink")).unwrap();
        assert!(Edit::open(root.join("symlink")).is_err());
        fs::write(root.join("binary"), b"\0\xff").unwrap();
        assert!(Edit::open(root.join("binary")).is_err());
        let large = File::create_new(root.join("large")).unwrap();
        large.set_len(EDIT_LIMIT + 1).unwrap();
        assert!(Edit::open(root.join("large")).is_err());
        fs::hard_link(&file, root.join("hard")).unwrap();
        assert!(Edit::open(file).is_err());
    }
    #[test]
    fn special_files_are_rejected_without_blocking() {
        let root = fixture();
        let socket = root.join("socket");
        assert!(std::process::Command::new("mkfifo")
            .arg(&socket)
            .status()
            .unwrap()
            .success());
        assert!(run(Action::Copy, &socket, Some(root.join("copy"))).is_err());
        assert!(Edit::open(socket).is_err());
    }
    #[test]
    fn batch_collision_is_preflighted_before_any_mutation() {
        let root = fixture();
        fs::write(root.join("a"), "a").unwrap();
        fs::write(root.join("b"), "b").unwrap();
        let dst = root.join("dst");
        fs::create_dir(&dst).unwrap();
        fs::write(dst.join("b"), "keep").unwrap();
        assert!(Plan::prepare(
            Action::Move,
            vec![root.join("a"), root.join("b")],
            Some(dst.clone())
        )
        .is_err());
        assert!(!dst.join("a").exists());
        assert!(root.join("a").exists());
    }
}
