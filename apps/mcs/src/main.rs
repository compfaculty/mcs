mod instance;

use std::{env, ffi::OsString};

const USAGE: &str = "Usage: mcs [--backend tui|auto|drm|native] [--help]";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Tui,
    Native,
    Help,
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    let first = args.next();
    let command = match first
        .as_ref()
        .map(|arg| arg.to_str().ok_or(USAGE))
        .transpose()?
    {
        None => Command::Tui,
        Some("--help" | "-h") => Command::Help,
        Some("--backend") => match args.next().as_deref().and_then(|arg| arg.to_str()) {
            Some("tui" | "auto") => Command::Tui,
            Some("drm" | "native") => Command::Native,
            _ => return Err(USAGE),
        },
        _ => return Err(USAGE),
    };
    if args.next().is_some() {
        return Err(USAGE);
    }
    Ok(command)
}

fn main() {
    // Nested shells never launch MCS, even after the parent has gone away.
    if env::var_os(instance::EMBEDDED_SHELL).is_some() {
        return;
    }
    let _instance = match instance::acquire() {
        Ok(Some(guard)) => guard,
        Ok(None) => return,
        Err(error) => {
            eprintln!("mcs: single-instance guard: {error}");
            std::process::exit(1);
        }
    };
    let command = match parse_args(env::args_os().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let result = match command {
        Command::Tui => mcs_tui::run(),
        Command::Native => mcs_native::run(),
        Command::Help => {
            println!("{USAGE}");
            return;
        }
    };
    if let Err(error) = result {
        eprintln!("mcs: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Command, &'static str> {
        parse_args(args.iter().map(OsString::from))
    }
    #[test]
    fn supported_commands() {
        assert_eq!(parse(&[]), Ok(Command::Tui));
        for backend in ["tui", "auto"] {
            assert_eq!(parse(&["--backend", backend]), Ok(Command::Tui));
        }
        for backend in ["drm", "native"] {
            assert_eq!(parse(&["--backend", backend]), Ok(Command::Native));
        }
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
    }
    #[test]
    fn rejects_malformed_arguments_before_terminal_initialization() {
        for args in [
            vec!["--backend"],
            vec!["--unknown"],
            vec!["--backend", "bad"],
            vec!["--backend", "tui", "extra"],
            vec!["--help", "extra"],
            vec!["--backend", "tui", "--backend", "drm"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_option() {
        use std::os::unix::ffi::OsStringExt;
        assert!(parse_args([OsString::from_vec(vec![0xff])]).is_err());
    }
}
