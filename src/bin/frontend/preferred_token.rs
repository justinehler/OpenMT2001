//! Typed V2-owned preference; never import or modify original settings.lua.
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Token(u8);
impl Token {
    pub fn new(value: usize) -> Result<Self, String> {
        if value < 10 {
            Ok(Self(value as u8))
        } else {
            Err("Invalid preferred token".into())
        }
    }
    pub fn index(self) -> usize {
        self.0 as usize
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
            Ok(b) if b.len() == 9 && &b[..8] == b"MTTOK001" => Self::new(b[8] as usize),
            Ok(_) => Err("Invalid V2 preferred token file".into()),
        }
    }
    pub fn save(self, path: &Path) -> Result<(), String> {
        fs::create_dir_all(path.parent().ok_or("Token directory missing")?)
            .map_err(|e| e.to_string())?;
        let temporary = path.with_extension("tmp");
        let mut bytes = b"MTTOK001".to_vec();
        bytes.push(self.0);
        fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        fs::rename(temporary, path).map_err(|e| e.to_string())
    }
}
pub fn path(root: &Path, name: &[u8]) -> PathBuf {
    let key = crate::campaign_store::profile_path(root, name);
    root.join("tokens")
        .join(key.file_stem().unwrap())
        .with_extension("bin")
}
/// Original profile defaults (4c1c50) restore PreferredCharacter to zero.
pub fn reset(root: &Path, name: &[u8]) -> Result<(), String> {
    Token::default().save(&path(root, name))
}
pub fn clear(root: &Path, name: &[u8]) -> Result<(), String> {
    match fs::remove_file(path(root, name)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistence_isolation_cleanup_and_default() {
        let root = std::env::temp_dir().join(format!(
            "token-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let a = path(&root, b"A");
        let b = path(&root, b"B");
        assert_eq!(Token::load(&a).unwrap().index(), 0);
        Token::new(3).unwrap().save(&a).unwrap();
        assert_eq!(Token::load(&a).unwrap().index(), 3);
        assert_eq!(Token::load(&b).unwrap().index(), 0);
        Token::new(7).unwrap().save(&b).unwrap();
        assert_eq!(Token::load(&a).unwrap().index(), 3);
        reset(&root, b"A").unwrap();
        assert_eq!(Token::load(&a).unwrap().index(), 0);
        assert_eq!(Token::load(&b).unwrap().index(), 7);
        clear(&root, b"A").unwrap();
        assert_eq!(Token::load(&a).unwrap().index(), 0);
        assert_eq!(Token::load(&b).unwrap().index(), 7);
        assert!(Token::new(10).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
