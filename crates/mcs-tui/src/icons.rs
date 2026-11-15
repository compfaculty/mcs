//! Decorative filename categories, independent of viewer support and graphics protocols.
use std::path::Path;

pub fn enabled() -> bool {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty());
    default_enabled(
        std::env::var("MCS_ICONS").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
        locale.as_deref(),
    )
}

fn default_enabled(setting: Option<&str>, term: Option<&str>, locale: Option<&str>) -> bool {
    match setting {
        Some("always") => true,
        Some("never") => false,
        _ => {
            // Font coverage cannot be probed reliably. Be conservative on virtual consoles.
            !matches!(term, None | Some("linux" | "dumb" | ""))
                && locale.is_some_and(|locale| {
                    let locale = locale.to_ascii_uppercase();
                    locale.contains("UTF-8") || locale.contains("UTF8")
                })
        }
    }
}

pub fn icon(path: &Path, directory: bool, symlink: bool, enabled: bool) -> &'static str {
    if !enabled {
        return if symlink {
            "@"
        } else if directory {
            "/"
        } else {
            " "
        };
    }
    if symlink {
        return "🔗";
    }
    if directory {
        return "📁";
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if matches!(
        name,
        "Makefile" | "Dockerfile" | "Containerfile" | "Justfile"
    ) {
        return "💻";
    }
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "svg" | "ico"
        | "heic" | "avif" => "📷",
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v" | "mpeg" | "mpg" => "🎬",
        "mp3" | "flac" | "wav" | "ogg" | "opus" | "m4a" | "aac" | "aiff" => "🎵",
        "pdf" => "📕",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "tgz" => "📦",
        "rs" | "py" | "go" | "c" | "h" | "cpp" | "hpp" | "js" | "ts" | "jsx" | "tsx" | "java"
        | "sh" | "bash" | "zsh" | "html" | "css" | "sql" => "💻",
        "toml" | "yaml" | "yml" | "json" | "ini" | "conf" | "cfg" | "xml" | "env" => "🔧",
        "txt" | "md" | "rst" | "log" | "csv" | "tsv" => "📝",
        _ => "📄",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_are_case_insensitive_and_links_and_directories_take_priority() {
        for (name, expected) in [
            ("PHOTO.JPG", "📷"),
            ("clip.mkv", "🎬"),
            ("song.flac", "🎵"),
            ("book.pdf", "📕"),
            ("source.tar.gz", "📦"),
            ("lib.rs", "💻"),
            ("Dockerfile", "💻"),
            ("config.toml", "🔧"),
            ("README.md", "📝"),
            ("unknown", "📄"),
            ("image.png.exe", "📄"),
        ] {
            assert_eq!(icon(Path::new(name), false, false, true), expected);
        }
        assert_eq!(icon(Path::new("folder.pdf"), true, false, true), "📁");
        assert_eq!(icon(Path::new("link.jpg"), true, true, true), "🔗");
        assert_eq!(icon(Path::new("broken"), false, true, true), "🔗");
        for (directory, symlink, expected) in
            [(true, false, "/"), (false, true, "@"), (false, false, " ")]
        {
            assert_eq!(
                icon(Path::new("file.pdf"), directory, symlink, false),
                expected
            );
        }
    }

    #[test]
    fn defaults_keep_non_utf8_and_virtual_consoles_in_ascii() {
        assert!(default_enabled(
            None,
            Some("xterm-kitty"),
            Some("en_US.UTF-8")
        ));
        assert!(default_enabled(
            None,
            Some("xterm-256color"),
            Some("C.utf8")
        ));
        for term in [None, Some("linux"), Some("dumb"), Some("")] {
            assert!(!default_enabled(None, term, Some("en_US.UTF-8")));
        }
        for locale in [None, Some("C"), Some("en_US.ISO8859-1")] {
            assert!(!default_enabled(None, Some("xterm"), locale));
        }
        assert!(!default_enabled(
            Some("never"),
            Some("xterm-kitty"),
            Some("C.UTF-8")
        ));
        assert!(default_enabled(Some("always"), Some("linux"), None));
    }
}
