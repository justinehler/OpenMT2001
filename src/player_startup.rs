//! Checks for player-supplied files before the native frontend opens media.
use std::{
    fmt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, PartialEq, Eq)]
pub enum PlayerFacingError {
    MissingPath {
        path: PathBuf,
        flag: &'static str,
        kind: &'static str,
    },
    FfmpegUnavailable(PathBuf),
}

impl fmt::Display for PlayerFacingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath { path, flag, kind } => write!(
                f,
                "Could not find the {kind} at '{}'; pass {flag} with its location (see 'Nightly alpha builds' in the README).",
                path.display()
            ),
            Self::FfmpegUnavailable(path) => write!(
                f,
                "Could not run FFmpeg ('{}'), which the original movies and compressed audio need; install it on PATH or pass --ffmpeg with its location (see 'Nightly alpha builds' in the README).",
                path.display()
            ),
        }
    }
}

impl std::error::Error for PlayerFacingError {}

/// The caller supplies resolved paths; this never opens media or starts a session.
pub fn validate_paths(
    assets: &Path,
    scene_root: &Path,
    executable: &Path,
    intro_map: &Path,
    language: &Path,
) -> Result<(), PlayerFacingError> {
    for (path, flag, kind, directory) in [
        (assets, "--assets", "asset folder", true),
        (scene_root, "--scene-root", "scene folder", true),
        (executable, "--executable", "game executable", false),
        (intro_map, "--intro-map", "intro map file", false),
        (language, "--language", "language folder", true),
    ] {
        if !(if directory {
            path.is_dir()
        } else {
            path.is_file()
        }) {
            return Err(PlayerFacingError::MissingPath {
                path: path.to_path_buf(),
                flag,
                kind,
            });
        }
    }
    Ok(())
}

/// Probe the same command (including PATH lookup) that media decoding uses.
pub fn check_ffmpeg(ffmpeg: &Path) -> Result<(), PlayerFacingError> {
    let mut command = Command::new(ffmpeg);
    command
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    match command.status() {
        Ok(status) if status.success() => Ok(()),
        _ => Err(PlayerFacingError::FfmpegUnavailable(ffmpeg.to_path_buf())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Install(PathBuf);

    impl Install {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "openmt-startup-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            for name in ["assets", "max", "usa"] {
                std::fs::create_dir(root.join(name)).unwrap();
            }
            for name in ["mc.exe", "intromap.lua"] {
                std::fs::write(root.join(name), []).unwrap();
            }
            Self(root)
        }

        fn validate(&self) -> Result<(), PlayerFacingError> {
            validate_paths(
                &self.0.join("assets"),
                &self.0.join("max"),
                &self.0.join("mc.exe"),
                &self.0.join("intromap.lua"),
                &self.0.join("usa"),
            )
        }
    }

    impl Drop for Install {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn existing_paths_are_accepted_without_reading_original_files() {
        assert_eq!(Install::new().validate(), Ok(()));
    }

    #[test]
    fn each_missing_path_names_its_flag_and_resolved_location() {
        for (name, flag, kind) in [
            ("assets", "--assets", "asset folder"),
            ("max", "--scene-root", "scene folder"),
            ("mc.exe", "--executable", "game executable"),
            ("intromap.lua", "--intro-map", "intro map file"),
            ("usa", "--language", "language folder"),
        ] {
            let install = Install::new();
            let path = install.0.join(name);
            if path.is_dir() {
                std::fs::remove_dir(&path).unwrap();
            } else {
                std::fs::remove_file(&path).unwrap();
            }
            let error = install.validate().unwrap_err();
            assert_eq!(
                error,
                PlayerFacingError::MissingPath {
                    path: path.clone(),
                    flag,
                    kind
                }
            );
            assert_eq!(
                error.to_string(),
                format!(
                    "Could not find the {kind} at '{}'; pass {flag} with its location (see 'Nightly alpha builds' in the README).",
                    path.display()
                )
            );
        }
    }

    #[test]
    fn directories_and_files_are_not_interchangeable() {
        let install = Install::new();
        std::fs::remove_dir(install.0.join("assets")).unwrap();
        std::fs::write(install.0.join("assets"), []).unwrap();
        assert!(install.validate().is_err());
        std::fs::remove_file(install.0.join("assets")).unwrap();
        std::fs::create_dir(install.0.join("assets")).unwrap();
        std::fs::remove_file(install.0.join("mc.exe")).unwrap();
        std::fs::create_dir(install.0.join("mc.exe")).unwrap();
        assert!(install.validate().is_err());
    }

    #[test]
    fn missing_ffmpeg_has_an_actionable_single_line_message() {
        let install = Install::new();
        let path = install.0.join("absent-ffmpeg");
        let error = check_ffmpeg(&path).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "Could not run FFmpeg ('{}'), which the original movies and compressed audio need; install it on PATH or pass --ffmpeg with its location (see 'Nightly alpha builds' in the README).",
                path.display()
            )
        );
        assert_eq!(error.to_string().lines().count(), 1);
    }
}
