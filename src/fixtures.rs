//! Locates the original game files and oracle captures that original-backed
//! tests read.
//!
//! The repository never ships original data. Tests that need it ask for it by
//! name, for example `fixtures::path("EXE")`, and are marked `#[ignore]` so the
//! default suite stays self-contained. A name resolves, in order, from:
//!
//! 1. the environment variable `OPENMT2001_<NAME>`, as a one-off override;
//! 2. the manifest `manifest.toml` inside the directory named by
//!    `OPENMT2001_FIXTURES`. Relative manifest paths are relative to that
//!    directory.
//!
//! Names may be written with or without the `OPENMT2001_` prefix.
//!
//! The manifest is a flat TOML table. Each entry is either a path string or an
//! inline table whose `path` key holds the path. Other keys, such as `sha256`,
//! are for the tool that assembles the fixture directory and are ignored here:
//!
//! ```toml
//! EXE = { path = "originals/mc.exe", sha256 = "..." }
//! LANGUAGE = "originals/US/language/usa"
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const PREFIX: &str = "OPENMT2001_";
const ROOT_VARIABLE: &str = "OPENMT2001_FIXTURES";
const MANIFEST: &str = "manifest.toml";

/// The ffmpeg the audio tests decode the original's music with: the `FFMPEG`
/// fixture (`OPENMT2001_FFMPEG` or the manifest) when it is set, else
/// `ffmpeg` on the `PATH`. Never a path relative to the working directory,
/// which differs per package under cargo.
pub fn ffmpeg() -> PathBuf {
    optional("FFMPEG").unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

/// Returns the location of a required fixture.
///
/// # Panics
///
/// Panics with a message that starts with `missing fixture` when the name is
/// neither overridden in the environment nor listed in the manifest.
pub fn path(name: &str) -> PathBuf {
    optional(name).unwrap_or_else(|| {
        let name = bare(name);
        panic!(
            "missing fixture {name}: set {PREFIX}{name}, or list it in the manifest under \
             {ROOT_VARIABLE} ({})",
            describe_root()
        )
    })
}

/// Returns the location of a fixture when one is configured.
pub fn optional(name: &str) -> Option<PathBuf> {
    let name = bare(name);
    if let Some(value) = std::env::var_os(format!("{PREFIX}{name}")).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(value));
    }
    let manifest = manifest().as_ref()?;
    manifest
        .entries
        .get(name)
        .map(|relative| manifest.root.join(relative))
}

/// Returns an existing, absolute directory that a test may create uniquely
/// named children in.
///
/// `OPENMT2001_<NAME>` overrides the location. Otherwise the directory is
/// `<NAME>` (lowercased) under `OPENMT2001_SCRATCH`, or under the system
/// temporary directory when that is unset.
pub fn scratch(name: &str) -> PathBuf {
    let name = bare(name);
    let directory = std::env::var_os(format!("{PREFIX}{name}"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("OPENMT2001_SCRATCH")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("openmt2001-scratch"))
                .join(name.to_ascii_lowercase())
        });
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("cannot create scratch {}: {error}", directory.display()));
    std::path::absolute(&directory).unwrap_or(directory)
}

fn bare(name: &str) -> &str {
    name.strip_prefix(PREFIX).unwrap_or(name)
}

fn describe_root() -> String {
    match std::env::var_os(ROOT_VARIABLE) {
        Some(root) => format!("currently {}", Path::new(&root).display()),
        None => "currently unset".into(),
    }
}

struct Manifest {
    root: PathBuf,
    entries: HashMap<String, String>,
}

fn manifest() -> &'static Option<Manifest> {
    static MANIFEST_CACHE: OnceLock<Option<Manifest>> = OnceLock::new();
    MANIFEST_CACHE.get_or_init(|| {
        let root = PathBuf::from(std::env::var_os(ROOT_VARIABLE).filter(|v| !v.is_empty())?);
        let file = root.join(MANIFEST);
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
        let entries =
            parse(&text).unwrap_or_else(|error| panic!("invalid {}: {error}", file.display()));
        Some(Manifest { root, entries })
    })
}

/// Parses the flat TOML subset described in the module documentation.
fn parse(text: &str) -> Result<HashMap<String, String>, String> {
    let mut entries = HashMap::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let mut cursor = Cursor(line.trim());
        if cursor.is_finished() {
            continue;
        }
        let key = cursor
            .key()
            .ok_or(format!("line {number}: expected a name"))?;
        cursor.expect('=', number)?;
        let path = if cursor.eat('{') {
            let mut path = None;
            loop {
                let field = cursor
                    .key()
                    .ok_or(format!("line {number}: expected a field name"))?;
                cursor.expect('=', number)?;
                let value = cursor.string(number)?;
                if field == "path" {
                    path = Some(value);
                }
                if cursor.eat('}') {
                    break;
                }
                cursor.expect(',', number)?;
            }
            path.ok_or(format!("line {number}: {key} has no path"))?
        } else {
            cursor.string(number)?
        };
        if !cursor.is_finished() {
            return Err(format!("line {number}: unexpected trailing text"));
        }
        if entries.insert(key.to_owned(), path).is_some() {
            return Err(format!("line {number}: {key} is listed twice"));
        }
    }
    Ok(entries)
}

struct Cursor<'a>(&'a str);

impl<'a> Cursor<'a> {
    fn skip_space(&mut self) {
        self.0 = self.0.trim_start();
    }

    fn is_finished(&mut self) -> bool {
        self.skip_space();
        self.0.is_empty() || self.0.starts_with('#')
    }

    fn eat(&mut self, token: char) -> bool {
        self.skip_space();
        match self.0.strip_prefix(token) {
            Some(rest) => {
                self.0 = rest;
                true
            }
            None => false,
        }
    }

    fn expect(&mut self, token: char, line: usize) -> Result<(), String> {
        if self.eat(token) {
            Ok(())
        } else {
            Err(format!("line {line}: expected `{token}`"))
        }
    }

    fn key(&mut self) -> Option<&'a str> {
        self.skip_space();
        let end = self
            .0
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(self.0.len());
        let (key, rest) = self.0.split_at(end);
        self.0 = rest;
        (!key.is_empty()).then_some(key)
    }

    fn string(&mut self, line: usize) -> Result<String, String> {
        self.skip_space();
        if let Some(rest) = self.0.strip_prefix('\'') {
            let end = rest
                .find('\'')
                .ok_or(format!("line {line}: unterminated string"))?;
            self.0 = &rest[end + 1..];
            return Ok(rest[..end].to_owned());
        }
        let rest = self
            .0
            .strip_prefix('"')
            .ok_or(format!("line {line}: expected a quoted string"))?;
        let mut value = String::new();
        let mut characters = rest.char_indices();
        while let Some((at, character)) = characters.next() {
            match character {
                '"' => {
                    self.0 = &rest[at + 1..];
                    return Ok(value);
                }
                '\\' => match characters.next().map(|(_, escaped)| escaped) {
                    Some('\\') => value.push('\\'),
                    Some('"') => value.push('"'),
                    Some('t') => value.push('\t'),
                    _ => return Err(format!("line {line}: unsupported escape")),
                },
                other => value.push(other),
            }
        }
        Err(format!("line {line}: unterminated string"))
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_plain_and_table_entries() {
        let entries = parse(
            "# private fixtures\n\
             \n\
             EXE = { path = \"originals/Essential/mc.exe\", sha256 = \"00ff\" }\n\
             LANGUAGE = 'originals/US/language/usa' # a directory\n\
             QUOTED = \"with \\\"quotes\\\" and \\\\ slash\"\n",
        )
        .unwrap();
        assert_eq!(entries["EXE"], "originals/Essential/mc.exe");
        assert_eq!(entries["LANGUAGE"], "originals/US/language/usa");
        assert_eq!(entries["QUOTED"], "with \"quotes\" and \\ slash");
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn rejects_malformed_entries() {
        for text in [
            "EXE",
            "EXE = unquoted",
            "EXE = { sha256 = \"00\" }",
            "EXE = \"a\" trailing",
            "EXE = \"a\"\nEXE = \"b\"",
            "EXE = \"unterminated",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn prefixed_and_bare_names_are_the_same_fixture() {
        assert_eq!(super::bare("OPENMT2001_EXE"), "EXE");
        assert_eq!(super::bare("EXE"), "EXE");
    }
}
