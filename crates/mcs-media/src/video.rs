//! Optional fullscreen mpv handoff. The caller owns terminal suspend/resume.
use crate::{open_regular, process};
use std::{
    ffi::{OsStr, OsString},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::atomic::AtomicU64,
    time::Duration,
};

pub struct Playback {
    program: PathBuf,
    pub args: Vec<OsString>,
    _temporary: tempfile::TempDir,
}
pub enum Outcome {
    Browser,
    ExitApplication,
}

const BINDINGS: &str = "SPACE cycle pause\nLEFT seek -5\nRIGHT seek 5\nUP add volume 5\nDOWN add volume -5\nm cycle mute\nESC quit\nq quit\nF10 quit 42\nCtrl+c quit 42\n";

impl Playback {
    pub fn prepare(path: &Path, token: &AtomicU64, generation: u64) -> io::Result<Self> {
        Self::prepare_with(OsStr::new("mpv"), path, token, generation)
    }
    pub fn prepare_with(
        program: &OsStr,
        path: &Path,
        token: &AtomicU64,
        generation: u64,
    ) -> io::Result<Self> {
        open_regular(path)?;
        let path = std::fs::canonicalize(path)?;
        let output = process::capture(
            program,
            &["--no-config".into(), "--vo=help".into()],
            256 * 1024,
            Duration::from_secs(3),
            token,
            generation,
        )?;
        if !String::from_utf8_lossy(&output)
            .lines()
            .any(|line| line.split_whitespace().next() == Some("kitty"))
        {
            return Err(io::Error::other("Installed mpv has no Kitty output driver"));
        }
        let temporary = tempfile::Builder::new().prefix("mcs-playback-").tempdir()?;
        let input = temporary.path().join("input.conf");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&input)?;
        file.write_all(BINDINGS.as_bytes())?;
        let mut args: Vec<OsString> = [
            "--no-config",
            "--really-quiet",
            "--load-scripts=no",
            "--load-auto-profiles=no",
            "--autoload-files=no",
            "--access-references=no",
            "--input-default-bindings=no",
            "--input-builtin-bindings=no",
            "--osc=no",
            "--terminal=yes",
            "--input-terminal=yes",
            "--vo=kitty",
            "--vo-kitty-alt-screen=yes",
            "--vo-kitty-use-shm=yes",
            "--profile=sw-fast",
            "--save-position-on-quit=no",
            "--resume-playback=no",
            "--demuxer=lavf",
            "--demuxer-lavf-o=protocol_whitelist=file",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let mut input_arg = OsString::from("--input-conf=");
        input_arg.push(input);
        let mut state_arg = OsString::from("--watch-later-directory=");
        state_arg.push(temporary.path());
        args.extend([
            input_arg,
            state_arg,
            OsString::from("--"),
            path.into_os_string(),
        ]);
        Ok(Self {
            program: program.into(),
            args,
            _temporary: temporary,
        })
    }
    pub fn run(&self) -> io::Result<Outcome> {
        let status = Command::new(&self.program).args(&self.args).status()?;
        Self::outcome(status)
    }
    fn outcome(status: ExitStatus) -> io::Result<Outcome> {
        match status.code() {
            Some(0) => Ok(Outcome::Browser),
            Some(42) => Ok(Outcome::ExitApplication),
            _ => Err(io::Error::other(format!(
                "mpv playback failed ({status}); browser state is preserved"
            ))),
        }
    }
}
