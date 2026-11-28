//! Read-only lookup for optional original-backed test inputs.
use std::path::PathBuf;

/// Resolve a named fixture through an explicit override or the private
/// campaign manifest. No original data or machine paths belong in the repo.
pub fn path(name: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(format!("OPENMT2001_{name}")) {
        return path.into();
    }
    let root = PathBuf::from(
        std::env::var_os("OPENMT2001_FIXTURES")
            .expect("set OPENMT2001_FIXTURES to run original-backed tests"),
    );
    let manifest =
        std::fs::read_to_string(root.join("manifest.toml")).expect("read fixture manifest");
    for line in manifest.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != name {
            continue;
        }
        // build.py emits one inline table per fixture and escapes only quotes
        // and backslashes in its path string.
        let value = value.split_once("path = \"").expect("fixture path entry").1;
        let mut path = String::new();
        let mut chars = value.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '"' => return root.join(path),
                '\\' => path.push(chars.next().expect("fixture path escape")),
                _ => path.push(ch),
            }
        }
        panic!("unterminated fixture path");
    }
    panic!("fixture {name} is not configured");
}
