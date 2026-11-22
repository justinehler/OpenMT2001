//! V2-owned, typed per-profile Options storage. Original settings.lua is
//! private evidence and is never parsed as executable code or written here.
use std::{fs, path::{Path, PathBuf}};

use crate::settings_menu::{GameplayLevels, GraphicsLevels, Levels};

const MAGIC: &[u8; 8] = b"MTOPT001";
const LENGTH: usize = MAGIC.len() + 9;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub audio: Levels,
    pub graphics: GraphicsLevels,
    pub gameplay: GameplayLevels,
}

/// Match the reversible profile key used by V2 campaign progress, in a
/// separate V2-owned directory. A profile name cannot become a path segment.
pub fn profile_path(root: &Path, name: &[u8]) -> PathBuf {
    let progress = crate::campaign_store::profile_path(root, name);
    let key = progress.file_stem().expect("encoded campaign profile key");
    root.join("options").join(key).with_extension("bin")
}

impl Snapshot {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        if bytes.len() != LENGTH || &bytes[..MAGIC.len()] != MAGIC {
            return Err(format!("{}: unsupported V2 Options file", path.display()));
        }
        let fields = &bytes[MAGIC.len()..];
        let [music, effects, detail, filtering, distance, auto, invert, cursor, sensitivity]: [u8; 9] =
            fields.try_into().unwrap();
        if music > 100 || effects > 100 || detail > 2 || filtering > 1
            || distance > 100 || auto > 1 || invert > 1 || cursor > 1 || sensitivity > 100 {
            return Err(format!("{}: invalid V2 Options value", path.display()));
        }
        Ok(Self {
            audio: Levels { music, effects },
            graphics: GraphicsLevels { detail, filtering, distance },
            gameplay: GameplayLevels {
                auto_rotate: auto != 0,
                invert_vertical: invert != 0,
                hardware_cursor: cursor != 0,
                mouse_sensitivity: sensitivity,
            },
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let fields = [self.audio.music, self.audio.effects,
            self.graphics.detail, self.graphics.filtering, self.graphics.distance,
            u8::from(self.gameplay.auto_rotate), u8::from(self.gameplay.invert_vertical),
            u8::from(self.gameplay.hardware_cursor), self.gameplay.mouse_sensitivity];
        if fields[0] > 100 || fields[1] > 100 || fields[2] > 2 || fields[3] > 1
            || fields[4] > 100 || fields[8] > 100 {
            return Err("invalid V2 Options snapshot".into());
        }
        let parent = path.parent().ok_or("Options file has no parent directory")?;
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
        let temporary = path.with_extension("tmp");
        let mut bytes = Vec::with_capacity(LENGTH);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&fields);
        fs::write(&temporary, bytes)
            .map_err(|error| format!("{}: {error}", temporary.display()))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("{}: {error}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!("openmt-options-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }

    #[test]
    fn options_round_trip_and_profile_isolation() {
        let root = test_root();
        let alice = profile_path(&root, b"Alice / Test");
        let bob = profile_path(&root, b"Bob");
        assert_ne!(alice, bob);
        assert!(alice.starts_with(root.join("options")));
        assert_eq!(Snapshot::load(&alice).unwrap(), Snapshot::default());
        let saved = Snapshot {
            audio: Levels { music: 17, effects: 91 },
            graphics: GraphicsLevels { detail: 0, filtering: 0, distance: 42 },
            gameplay: GameplayLevels { auto_rotate: false, invert_vertical: true,
                hardware_cursor: true, mouse_sensitivity: 73 },
        };
        saved.save(&alice).unwrap();
        assert_eq!(Snapshot::load(&alice).unwrap(), saved);
        let mut revised = saved;
        revised.audio.music = 64;
        revised.save(&alice).unwrap();
        assert_eq!(Snapshot::load(&alice).unwrap(), revised);
        assert_eq!(Snapshot::load(&bob).unwrap(), Snapshot::default());
        Snapshot::default().save(&bob).unwrap();
        assert_eq!(Snapshot::load(&alice).unwrap(), revised);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_truncated_or_invalid_option_bytes() {
        let root = test_root();
        let path = profile_path(&root, b"Malformed");
        Snapshot::default().save(&path).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        bytes[MAGIC.len() + 2] = 3;
        fs::write(&path, &bytes).unwrap();
        assert!(Snapshot::load(&path).unwrap_err().contains("invalid V2 Options value"));
        fs::write(&path, &bytes[..8]).unwrap();
        assert!(Snapshot::load(&path).unwrap_err().contains("unsupported V2 Options file"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_temporary_write_preserves_published_snapshot() {
        let root = test_root();
        let path = profile_path(&root, b"Keep");
        let published = Snapshot::default();
        published.save(&path).unwrap();
        let temporary = path.with_extension("tmp");
        fs::create_dir(&temporary).unwrap();
        let mut changed = published;
        changed.audio.effects = 12;
        assert!(changed.save(&path).is_err());
        assert_eq!(Snapshot::load(&path).unwrap(), published);
        fs::remove_dir_all(root).unwrap();
    }
}
