//! In-game Chat panel opened from Player Comparison (0x41a920/0x427210): recipient
//! toggles, All, Close and the input line.
use openmt2001::network::chat::{Recipients, View};
#[derive(Default)]
pub struct Panel {
    pub open: bool,
    pub recipients: Recipients,
    pub input: Vec<u8>,
    pressed: Option<usize>,
}
impl Panel {
    /// A token clicked in Player Comparison (compact manager index). The first click
    /// opens the panel with only that player selected; later clicks toggle players, and
    /// the local player's own token selects All.
    pub fn select_player(&mut self, manager: usize, chat: &View) {
        let row = chat
            .snapshot
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                matches!(
                    r.player.kind,
                    openmt2001::network::RowKind::Human | openmt2001::network::RowKind::Ai
                )
            })
            .nth(manager)
            .map(|(i, _)| i);
        let Some(row) = row else {
            return;
        };
        if !self.open {
            self.recipients.clear();
            self.open = true;
        }
        if chat.snapshot.rows[row].peer == Some(chat.local_peer) {
            self.recipients.all();
        } else {
            self.recipients.0[row] = !self.recipients.0[row];
        }
    }
    pub fn text(&mut self, text: &str) {
        self.input.extend(
            text.chars()
                .filter_map(|c| u8::try_from(c as u32).ok())
                .filter(|&b| b >= 32 && b != 127)
                .take(128usize.saturating_sub(self.input.len())),
        );
    }
    /// Enter: the typed text and recipients to submit, clearing the input. Nothing is
    /// taken when the text is empty or no player other than the local one is selected.
    pub fn take_submission(&mut self, chat: &View) -> Option<(Vec<u8>, Recipients)> {
        let local = chat.snapshot.local_row(chat.local_peer)?;
        if self.input.is_empty() || self.recipients.mask(local) == 0 {
            return None;
        }
        Some((std::mem::take(&mut self.input), self.recipients))
    }
    pub fn close(&mut self) {
        self.open = false;
        self.input.clear();
        self.pressed = None;
    }
    pub fn mouse(&mut self, point: Option<[i32; 2]>, down: bool, layout: &Layout, chat: &View) {
        let others = other_rows(chat);
        let target = point.and_then(|p| {
            let p = p.map(|v| v as f32 * 0.5);
            if inside(layout.all, p) {
                Some(6)
            } else if inside(layout.close, p) {
                Some(7)
            } else {
                layout
                    .icons
                    .iter()
                    .zip(&others)
                    .find(|(b, _)| inside(**b, p))
                    .map(|(_, &row)| row)
            }
        });
        if down {
            self.pressed = target;
        } else if self.pressed.take().is_some_and(|p| Some(p) == target) {
            match target {
                Some(6) => self.recipients.all(),
                Some(7) => self.close(),
                Some(i) if i < 6 => self.recipients.0[i] = !self.recipients.0[i],
                _ => (),
            }
        }
    }
}
fn inside([l, t, r, b]: [f32; 4], [x, y]: [f32; 2]) -> bool {
    x >= l && x <= r && y >= t && y <= b
}
pub fn other_rows(chat: &View) -> Vec<usize> {
    chat.snapshot
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            matches!(
                row.player.kind,
                openmt2001::network::RowKind::Human | openmt2001::network::RowKind::Ai
            ) && row.peer != Some(chat.local_peer)
        })
        .map(|(i, _)| i)
        .collect()
}
#[derive(Clone)]
pub struct Layout {
    pub title: [f32; 4],
    pub backing: [f32; 4],
    pub to: [f32; 4],
    pub input: [f32; 4],
    pub all: [f32; 4],
    pub close: [f32; 4],
    pub icons: [[f32; 4]; 5],
}
pub struct Art {
    pub base: Vec<crate::ui::Surface>,
    pub layout: Layout,
    pub body: u32,
}
impl Art {
    pub fn load(
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
        body: u32,
    ) -> Result<Self, String> {
        let name = "gamedata2/brand_new_panels/chat_panel_v2.tm";
        let entry = index.find(name).ok_or("Missing Chat panel")?;
        let bytes = archive
            .get(entry.offset as usize..(entry.offset + entry.length) as usize)
            .ok_or("Chat panel asset range")?;
        let mesh = openmt2001::mesh::CachedMesh::parse(bytes)?;
        let frame = mesh.frames.saturating_sub(1);
        let regions = openmt2001::ui_mesh::regions(&mesh, frame)?;
        let bounds = |name: &str| {
            regions
                .iter()
                .find(|r| r.name == name || r.name.ends_with(&format!("_{name}")))
                .map(|r| r.bounds)
                .ok_or_else(|| format!("Missing Chat region {name}"))
        };
        let layout = Layout {
            title: bounds("CHAT_TITLE_AREA")?,
            backing: bounds("CHAT_BACKING")?,
            to: bounds("CHAT_TO_LABEL")?,
            input: bounds("CHAT_INPUT_AREA")?,
            all: bounds("CHAT_ALL_BUTTON")?,
            close: bounds("CHAT_CANCEL_BUTTON")?,
            icons: [
                bounds("CHAT_ICON_SLOT1")?,
                bounds("CHAT_ICON_SLOT2")?,
                bounds("CHAT_ICON_SLOT3")?,
                bounds("CHAT_ICON_SLOT4")?,
                bounds("CHAT_ICON_SLOT5")?,
            ],
        };
        let (base, _) = crate::ui::panel_frame_base(
            index,
            archive,
            name,
            &(0..mesh.parts.len()).collect::<Vec<_>>(),
            frame,
        )?;
        Ok(Self { base, layout, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_panel_selects_opponents_all_six_and_cancels_unsent_text() {
        use openmt2001::network::lobby::{Request, Settings, Snapshot};
        let mut snapshot = Snapshot::host(
            Settings {
                name: b"Game".to_vec(),
                level: 300001,
                parameter_bits: 10000f32.to_bits(),
                difficulty: 0,
                map: b"Map".to_vec(),
            },
            b"Host".to_vec(),
            0,
        )
        .unwrap();
        snapshot
            .request(
                1,
                Request::Join {
                    name: b"Blue".to_vec(),
                    token: 3,
                },
            )
            .unwrap();
        let chat = View {
            snapshot,
            local_peer: 0,
            lines: vec![],
        };
        let layout = Layout {
            title: [0.; 4],
            backing: [0.; 4],
            to: [0.; 4],
            input: [0.; 4],
            all: [100., 10., 120., 30.],
            close: [130., 10., 160., 30.],
            icons: [[10., 10., 30., 30.]; 5],
        };
        let mut panel = Panel::default();
        panel.recipients.all();
        panel.select_player(1, &chat);
        assert!(panel.open);
        assert_eq!(
            panel.recipients.0,
            [false, true, false, false, false, false]
        );
        panel.mouse(Some([40, 40]), true, &layout, &chat);
        panel.mouse(Some([40, 40]), false, &layout, &chat);
        assert_eq!(panel.recipients.0, [false; 6]);
        panel.mouse(Some([220, 40]), true, &layout, &chat);
        panel.mouse(Some([220, 40]), false, &layout, &chat);
        assert_eq!(panel.recipients.0, [true; 6]);
        panel.text(&"x".repeat(200));
        assert_eq!(panel.input.len(), 128);
        panel.mouse(Some([280, 40]), true, &layout, &chat);
        panel.mouse(Some([280, 40]), false, &layout, &chat);
        assert!(!panel.open);
        assert!(panel.input.is_empty());
        panel.select_player(0, &chat);
        assert_eq!(panel.recipients.0, [true; 6]);
    }
    #[test]
    fn enter_takes_text_only_for_a_recipient_other_than_self() {
        use openmt2001::network::lobby::{Request, Settings, Snapshot};
        let mut snapshot = Snapshot::host(
            Settings {
                name: b"Game".to_vec(),
                level: 300001,
                parameter_bits: 10000f32.to_bits(),
                difficulty: 0,
                map: b"Map".to_vec(),
            },
            b"Host".to_vec(),
            0,
        )
        .unwrap();
        snapshot
            .request(
                1,
                Request::Join {
                    name: b"Blue".to_vec(),
                    token: 3,
                },
            )
            .unwrap();
        let chat = View {
            snapshot,
            local_peer: 0,
            lines: vec![],
        };
        let mut panel = Panel::default();
        panel.select_player(0, &chat);
        panel.recipients.clear();
        panel.recipients.0[0] = true;
        panel.text("hello");
        assert_eq!(panel.take_submission(&chat), None);
        assert_eq!(panel.input, b"hello");
        panel.recipients.0[1] = true;
        let (text, recipients) = panel.take_submission(&chat).unwrap();
        assert_eq!((text.as_slice(), recipients.mask(0)), (&b"hello"[..], 0b10));
        assert!(panel.input.is_empty());
        assert_eq!(panel.take_submission(&chat), None);
    }
    #[test]
    #[ignore = "requires original Chat mesh and UI textures"]
    fn chat_original_authored_regions_load() {
        let dir = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index = openmt2001::archive::ArchiveIndex::parse(&dir, archive.len() as u64).unwrap();
        let art = Art::load(&index, &archive, 0xbb000a28).unwrap();
        assert!(!art.base.is_empty());
        for bounds in [
            art.layout.title,
            art.layout.input,
            art.layout.all,
            art.layout.close,
        ] {
            assert!(bounds[0] < bounds[2] && bounds[1] < bounds[3]);
        }
    }
}
