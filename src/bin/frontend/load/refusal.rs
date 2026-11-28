//! Typed save refusals checked before the loader accepts a world.
use openmt2001::{level_catalog::Catalog, pe::PeImage, save::SaveFile};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    UnavailableLevel(u32),
    Incompatible,
    Storage(String),
}

impl From<String> for Failure {
    fn from(error: String) -> Self {
        Self::Storage(error)
    }
}
impl From<&str> for Failure {
    fn from(error: &str) -> Self {
        Self::Storage(error.into())
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnavailableLevel(id) => write!(f, "Saved level {id} is unavailable"),
            Self::Incompatible => f.write_str("Incompatible saved game"),
            Self::Storage(error) => f.write_str(error),
        }
    }
}

/// Checks the exact bytes the world loader will consume, before adoption.
pub(crate) fn validate<'a>(
    bytes: &'a [u8],
    compatibility: &[u8],
    catalog: &Catalog,
) -> Result<(SaveFile<'a>, usize, [u32; 4]), Failure> {
    let save = SaveFile::parse(bytes)?;
    let header = save
        .sections
        .first()
        .ok_or("save has no compatibility record")?;
    if header.tag != 1 || header.payload != compatibility {
        return Err(Failure::Incompatible);
    }
    let metadata = save.level_metadata()?;
    let index = catalog
        .find(metadata[0])
        .ok_or(Failure::UnavailableLevel(metadata[0]))?;
    Ok((save, index, metadata))
}

#[derive(Default)]
pub(super) struct Messages {
    missing_title: Vec<u8>,
    missing_body: Vec<u8>,
    incompatible_title: Vec<u8>,
    incompatible_body: Vec<u8>,
}
impl Messages {
    pub(super) fn from_original(image: &PeImage<'_>) -> Result<Self, String> {
        // These are executable literals, not language resource IDs. 0x59b
        // is the incompatible notice's OK button (4bc290), not its body.
        Ok(Self {
            missing_title: original_text(image, 0x55a0c0)?,
            missing_body: original_text(image, 0x541e54)?,
            incompatible_title: original_text(image, 0x545ed8)?,
            incompatible_body: original_text(image, 0x545e7c)?,
        })
    }
    pub(super) fn prompt(&self, failure: &Failure) -> Option<crate::prompt::Request> {
        let (title, body) = match failure {
            Failure::UnavailableLevel(_) => (
                self.missing_title.clone(),
                // The validator at 48d990 formats one signed level identity.
                self.missing_body.clone(),
            ),
            Failure::Incompatible => (
                self.incompatible_title.clone(),
                self.incompatible_body.clone(),
            ),
            Failure::Storage(_) => return None,
        };
        let body = if let Failure::UnavailableLevel(id) = failure {
            let mut body = body;
            if let Some(at) = body.windows(2).position(|bytes| bytes == b"%d") {
                body.splice(at..at + 2, (*id as i32).to_string().bytes());
            }
            body
        } else {
            body
        };
        // ScriptText consumes literal percent escapes after mapping the supplied
        // executable text to the active language's glyph row.
        let literal = |text: Vec<u8>| -> std::sync::Arc<[u8]> {
            let mut escaped = Vec::with_capacity(text.len());
            for byte in text {
                escaped.push(byte);
                if byte == b'%' {
                    escaped.push(byte);
                }
            }
            escaped.into()
        };
        let dialog = crate::human_dialog::Dialog {
            buttons: [0x59b, 0],
            button_count: 1,
            timeout: false,
            ..Default::default()
        };
        Some(crate::prompt::Request {
            script: Some(crate::prompt::ScriptText {
                title: Some(literal(title)),
                body: Some(literal(body)),
                format_value: 0,
                timeout: false,
            }),
            dialog: Some(dialog),
            title: 0,
            body: 0,
            dismissal_id: 0,
            card: None,
        })
    }
}

fn original_text(image: &PeImage<'_>, address: u32) -> Result<Vec<u8>, String> {
    let mut text = Vec::new();
    for offset in 0..1024 {
        let byte = image.at(address + offset, 1)?[0];
        if byte == 0 {
            return Ok(text);
        }
        text.push(byte);
    }
    Err("load refusal text is not terminated".into())
}

#[cfg(test)]
mod tests {
    use super::super::{Entry, LevelTextCatalog, Load};
    use super::*;
    use std::{fs, path::PathBuf, time::SystemTime};

    fn catalog() -> Catalog {
        let mut catalog = Catalog::default();
        catalog
            .register(
                openmt2001::level_catalog::Registration {
                    id: 7,
                    directory: b"SYNTHETIC".to_vec(),
                    labels: openmt2001::level_catalog::Labels::Literal {
                        title: vec![],
                        description: vec![],
                    },
                    checksum: 0,
                },
                false,
                true,
            )
            .unwrap();
        catalog
    }
    fn save(id: u32, marker: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&[marker; 40]);
        for word in [0x2000u32, 16, id, 1950, 0, 1, u32::MAX, 0] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }
    fn load(path: PathBuf) -> Load {
        Load {
            base: vec![],
            bounds: vec![[0.; 4]; 25],
            fonts: vec![],
            tab: vec![],
            list_fonts: vec![],
            list_tab: vec![],
            list_layout: openmt2001::frontend_list::Layout::new([20., 40., 284., 302.], 11., 3.)
                .unwrap(),
            info_layout: openmt2001::frontend_list::Layout::new([338., 287., 588., 379.], 12., 3.)
                .unwrap(),
            description_fonts: vec![],
            description_tab: vec![],
            slider_textures: vec![],
            title_fonts: vec![],
            title_tab: vec![],
            strings: vec![],
            level_text: LevelTextCatalog::default(),
            directories: vec![],
            app_root: PathBuf::new(),
            import_directory: PathBuf::new(),
            campaign_filename: None,
            profile_root: PathBuf::new(),
            entries: vec![Entry::read(path, vec![], None, None)],
            chosen: Some(0),
            first: 0,
            description_first: 0,
            over_description: false,
            pointer_position: None,
            visible: true,
            busy: false,
            request: None,
            message: None,
            delete_confirmation: None,
            refusal_messages: Messages {
                missing_title: b"Synthetic warning".to_vec(),
                missing_body: b"Synthetic missing level %d\nNo world accepted".to_vec(),
                incompatible_title: b"Synthetic incompatible title".to_vec(),
                incompatible_body: b"Synthetic incompatible body".to_vec(),
            },
            refusal_prompt: None,
        }
    }

    #[test]
    fn load_refusal_kinds_and_dismissal_leave_the_panel_usable() {
        let root = std::env::temp_dir().join(format!(
            "openmt-load-refusal-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("synthetic.sav");
        let catalog = catalog();
        for (bytes, expected) in [
            (save(987654, 1), Failure::UnavailableLevel(987654)),
            (save(7, 2), Failure::Incompatible),
        ] {
            fs::write(&path, bytes).unwrap();
            let mut load = load(path.clone());
            assert!(load.entries[0].metadata.is_ok());
            load.activate(0);
            assert_eq!(load.request.take(), Some(path.clone()));
            assert!(load.busy);
            let bytes = fs::read(&path).unwrap();
            let failure = validate(&bytes, &[1; 40], &catalog).unwrap_err();
            assert_eq!(failure, expected);
            load.refuse(failure);
            assert!(load.visible && !load.busy && load.request.is_none());
            assert!(load.message.is_none());
            assert_eq!(load.count(), 1);
            let request = load.prompt_request().unwrap();
            let dialog = request.dialog.unwrap();
            assert_eq!(
                (dialog.buttons, dialog.button_count, dialog.timeout),
                ([0x59b, 0], 1, false)
            );
            let script = request.script.unwrap();
            let (title, body) = match expected {
                Failure::UnavailableLevel(_) => (
                    b"Synthetic warning".as_slice(),
                    b"Synthetic missing level 987654\nNo world accepted".as_slice(),
                ),
                Failure::Incompatible => (
                    b"Synthetic incompatible title".as_slice(),
                    b"Synthetic incompatible body".as_slice(),
                ),
                _ => unreachable!(),
            };
            assert_eq!(script.title.as_deref(), Some(title));
            assert_eq!(script.body.as_deref(), Some(body));
            let row: Vec<_> = (0..=223).collect();
            let glyphs =
                crate::prompt::literal_glyphs(script.body.as_deref().unwrap(), &row).unwrap();
            let expected_glyphs = body
                .iter()
                .flat_map(|&byte| {
                    if byte == b'\n' {
                        vec![0x3c, 0x4e]
                    } else {
                        vec![byte - 32]
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(glyphs, expected_glyphs);
            load.activate(3); // The modal cannot select an underlying save.
            assert!(load.prompt_pending() && load.request.is_none());
            load.scroll(1);
            assert_eq!(load.first, 0);
            assert_eq!(load.hit([0.; 2]), None);
            load.activate(0); // OK
            assert!(load.visible && !load.prompt_pending());
            assert_eq!(load.chosen, Some(0));
            load.activate(0); // The same selected row can be loaded again.
            assert_eq!(load.request.take(), Some(path.clone()));
            load.refuse(expected);
            load.activate(0); // OK dismisses only the warning.
            assert!(load.visible && !load.prompt_pending() && !load.busy);
        }
        let mut truncated = save(7, 1);
        truncated.pop();
        // List information being readable must not relax full-load parsing.
        fs::write(&path, truncated).unwrap();
        let mut load = load(path.clone());
        assert!(
            load.entries[0].metadata.is_ok(),
            "metadata-only lists admit this row"
        );
        load.activate(0);
        load.request.take();
        let bytes = fs::read(&path).unwrap();
        let failure = validate(&bytes, &[1; 40], &catalog).unwrap_err();
        assert!(matches!(failure, Failure::Storage(_)));
        load.refuse(failure);
        assert!(load.visible && !load.busy && !load.prompt_pending());
        assert!(
            load.message
                .as_ref()
                .is_some_and(|message| message.contains("truncated"))
        );
        assert!(load.request.is_none());
        assert!(validate(&save(7, 1), &[1; 40], &catalog).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires supplied original v1.4 executable"]
    fn original_refusal_fields_use_executable_literals_and_localized_ok() {
        let bytes = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&bytes).unwrap();
        let messages = Messages::from_original(&image).unwrap();
        for (failure, title_address, body_address) in [
            (Failure::UnavailableLevel(987654), 0x55a0c0, 0x541e54),
            (Failure::Incompatible, 0x545ed8, 0x545e7c),
        ] {
            let request = messages.prompt(&failure).unwrap();
            let script = request.script.unwrap();
            let title = original_text(&image, title_address).unwrap();
            let body = original_text(&image, body_address).unwrap();
            let body = if matches!(failure, Failure::UnavailableLevel(_)) {
                String::from_utf8(body)
                    .unwrap()
                    .replacen("%d", "987654", 1)
                    .into_bytes()
            } else {
                body
            };
            assert_eq!(script.title.as_deref(), Some(title.as_slice()));
            assert_eq!(script.body.as_deref(), Some(body.as_slice()));
            assert_eq!(request.dialog.unwrap().buttons[0], 0x59b);
        }
    }
}
