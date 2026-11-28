//! Nightly voting widget (57e770), distinct from the statistics history graph.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    pe::PeImage,
    player_share::Call,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

const PANEL: &str = "gamedata2/brand_new_panels/voting_panel.tm";
const VIEW: [f32; 16] = [
    2. / 640.,
    0.,
    0.,
    0.,
    0.,
    -2. / 480.,
    0.,
    0.,
    0.,
    0.,
    1.,
    0.,
    -1.,
    1.,
    0.,
    1.,
];

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: Vec<u8>,
    pub character: usize,
    pub percentage: f32,
    pub progress: u32,
}
impl Row {
    pub fn percentage_text(&self) -> String {
        // 42c4b0 uses the CRT's default %f precision, followed by a literal %.
        format!("{:.6}%", self.percentage)
    }
    pub fn progress_text(&self) -> String {
        format!("{}", self.progress as i32)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub objective: i32,
    pub rows: [Option<Row>; 6],
    pub candidate: Option<usize>,
    pub notice: u32,
}
#[derive(Default)]
pub struct State {
    pub snapshot: Option<Snapshot>,
    remaining: u32,
}
impl State {
    /// Production callback consumer. Share/count values are the callback's
    /// arguments, including the mayor increment made immediately before it.
    pub fn apply(
        &mut self,
        call: Call,
        objective: i32,
        players: &[impl AsRef<[u8]>],
    ) -> Result<(), String> {
        match call {
            Call::Refresh => {
                self.snapshot = Some(Snapshot {
                    objective,
                    rows: std::array::from_fn(|_| None),
                    candidate: None,
                    notice: if objective == 2 { 0x607 } else { 0x606 },
                });
                // 42c850 starts a 750-message lifetime; 42c810 decrements it.
                self.remaining = 750;
            }
            Call::ProfileSelect { index } | Call::Kind1ProfileSelect { index } => {
                let slot = index as usize;
                if slot >= 6 || players.get(slot).is_none() {
                    return Err("market-share profile player leaves roster".into());
                }
                let snapshot = self
                    .snapshot
                    .as_mut()
                    .ok_or("market-share selection before refresh")?;
                snapshot.candidate = Some(slot);
                snapshot.notice = if matches!(call, Call::ProfileSelect { .. }) {
                    0x605
                } else {
                    0x604
                };
            }
            Call::PlayerShare {
                slot,
                normalized,
                auxiliary_reference,
            } => {
                if slot >= 6 || !normalized.is_finite() {
                    return Err("invalid market-share row".into());
                }
                let player = players
                    .get(slot)
                    .ok_or("market-share row leaves roster")?
                    .as_ref();
                if player.len() < 0x1b8 {
                    return Err("market-share player record is truncated".into());
                }
                let name = &player[0x158..0x178];
                let end = name
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or("market-share name is unterminated")?;
                let row = Row {
                    name: name[..end].to_vec(),
                    character: u32::from_le_bytes(player[0x150..0x154].try_into().unwrap())
                        as usize,
                    percentage: normalized,
                    progress: auxiliary_reference,
                };
                self.snapshot
                    .as_mut()
                    .ok_or("market-share row before refresh")?
                    .rows[slot] = Some(row);
            }
            _ => return Err("nonpresentation market-share callback".into()),
        }
        Ok(())
    }
    pub fn dismiss(&mut self) {
        self.snapshot = None;
        self.remaining = 0;
    }
    pub fn interface_tick(&mut self) {
        if self.snapshot.is_some() {
            self.remaining = self.remaining.saturating_sub(1);
            if self.remaining == 0 {
                self.dismiss();
            }
        }
    }
}

pub struct Assets {
    base: Vec<Surface>,
    regions: Vec<(String, [f32; 4])>,
    generic: Vec<Surface>,
    fonts: Vec<Vec<Surface>>,
    tabs: Vec<Vec<u8>>,
    strings: Vec<Vec<u8>>,
    colors: Vec<u32>,
    tokens: Vec<usize>,
    body: u32,
}
impl Assets {
    pub(crate) fn apply_board(
        &mut self,
        board: &crate::intro::camera_live::ui_text_resources::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        executable: &[u8],
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let entry = index
                .find(name)
                .ok_or_else(|| format!("Missing voting asset {name}"))?;
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Voting asset range".into())
        };
        let mesh = CachedMesh::parse(payload(PANEL)?)?;
        let frame = mesh.frames.checked_sub(1).ok_or("Empty voting model")?;
        let regions = openmt2001::ui_mesh::regions(&mesh, frame)?;
        // The border is visible artwork. Other named parts are reverse-facing
        // placement geometry, replaced by the runtime fills and controls.
        let border = regions
            .iter()
            .find(|r| r.name == "VOTING_BORDER01")
            .ok_or("Voting border absent")?
            .part;
        let base = ui::panel_frame_base(index, archive, PANEL, &[border], frame)?.0;
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect();
        let set = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let fonts = (0..4)
            .map(|i| {
                set.textures[i * 2..i * 2 + 2]
                    .iter()
                    .map(Surface::new)
                    .collect()
            })
            .collect();
        let tabs = [
            "cust640p.tab",
            "tahoma12bldnoantip.tab",
            "tahoma11noantip.tab",
            "tahoma10bldnoantip.tab",
        ]
        .iter()
        .map(|name| {
            fs::read(installer.join("Standard/fonts").join(name)).map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
        let image = PeImage::parse(executable)?;
        let word = |at| -> Result<u32, String> {
            Ok(u32::from_le_bytes(image.at(at, 4)?.try_into().unwrap()))
        };
        Ok(Self {
            base,
            regions: regions.into_iter().map(|r| (r.name, r.bounds)).collect(),
            generic,
            fonts,
            tabs,
            strings: crate::tooltip::localized_strings(installer, strings)?,
            colors: (0..10)
                .map(|i| word(0x541af0 + i * 40))
                .collect::<Result<_, _>>()?,
            tokens: (0..10)
                .map(|i| word(0x541af8 + i * 40).map(|v| v as usize))
                .collect::<Result<_, _>>()?,
            body: word(0x5362c0)?,
        })
    }
    fn region(&self, name: &str) -> Result<[f32; 4], String> {
        self.regions
            .iter()
            .find(|r| r.0 == name)
            .map(|r| r.1)
            .ok_or_else(|| format!("Missing voting region {name}"))
    }
    pub fn button_at(&self, point: [f32; 2]) -> bool {
        self.region("VOTING_OK_BUTTON").is_ok_and(|b| {
            point[0] >= b[0] && point[0] <= b[2] && point[1] >= b[1] && point[1] <= b[3]
        })
    }
    fn encode(&self, raw: &[u8]) -> Result<Vec<u8>, String> {
        let alphabet = self.strings.first().ok_or("Voting alphabet absent")?;
        encode_raw(raw, alphabet)
    }
    fn text(
        &self,
        surfaces: &mut Vec<Surface>,
        font: usize,
        text: &[u8],
        bounds: [f32; 4],
    ) -> Result<(), String> {
        if text.is_empty() {
            return Ok(());
        }
        let mut pages = self.fonts[font].clone();
        ui::text_widget_style4(&mut pages, &self.tabs[font], text, bounds, u32::MAX)?;
        surfaces.extend(pages);
        Ok(())
    }
    fn icon(
        &self,
        surfaces: &mut Vec<Surface>,
        index: usize,
        bounds: [f32; 4],
        color: u32,
    ) -> Result<(), String> {
        let mut icon = self
            .generic
            .get(index)
            .ok_or("Voting image absent")?
            .clone();
        let [l, t, r, b] = bounds;
        icon.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [color; 4],
        );
        surfaces.push(icon);
        Ok(())
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        snapshot: &Snapshot,
        pressed: bool,
    ) -> Result<PreparedScene, String> {
        let mut backing = Surface::solid();
        backing.rect(
            self.region("VOTING_TITLE_AREA")?,
            [0xff909090, 0xff909090, 0x80909090, 0x80909090],
        );
        backing.rect(self.region("VOTING_MAIN_BACKING")?, [self.body; 4]);
        backing.rect(self.region("VOTING_TITLE_LABEL")?, [0xff1f2f70; 4]);
        let mut surfaces = vec![backing];
        surfaces.extend(self.base.clone());
        let graph = self.region("VOTING_GRAPH_AREA")?;
        let mut bars = Surface::solid();
        bars.line(
            [graph[0] + 25., graph[1]],
            [graph[0] + 25., graph[3]],
            0xff00ff00,
        );
        bars.line(
            [graph[0] + 25., graph[3]],
            [graph[2] - 25., graph[3]],
            0xff00ff00,
        );
        for (slot, row) in snapshot.rows.iter().enumerate() {
            let Some(row) = row else { continue };
            let color = *self
                .colors
                .get(row.character)
                .ok_or("Voting character outside palette")?;
            let token = *self
                .tokens
                .get(row.character)
                .ok_or("Voting token outside palette")?;
            self.icon(
                &mut surfaces,
                token,
                self.region(&format!("VOTING_TOKEN{}", slot + 1))?,
                u32::MAX,
            )?;
            let rosette = self.region(&format!("VOTING_ROSETTE{}", slot + 1))?;
            let top = bar_top(graph, row.percentage);
            bars.rect([rosette[0], top, rosette[2], graph[3]], [color; 4]);
            self.text(
                &mut surfaces,
                0,
                &self.encode(row.percentage_text().as_bytes())?,
                [rosette[0], top - 20., rosette[2], top],
            )?;
            if snapshot.objective != 2 {
                // Generic image table cell68b2a8 is ordinal288.
                self.icon(&mut surfaces, 288, rosette, color)?;
                self.text(
                    &mut surfaces,
                    1,
                    &self.encode(row.progress_text().as_bytes())?,
                    [rosette[0], rosette[1] - 4., rosette[2], rosette[3] - 4.],
                )?;
            }
        }
        surfaces.insert(1, bars);
        let mut notice = self
            .strings
            .get(snapshot.notice as usize)
            .ok_or("Voting notice language absent")?
            .clone();
        if let Some(slot) = snapshot.candidate {
            let row = snapshot
                .rows
                .get(slot)
                .and_then(Option::as_ref)
                .ok_or("Voting candidate row absent")?;
            let placeholder = self.encode(b"%s")?;
            let at = notice
                .windows(2)
                .position(|s| s == placeholder)
                .ok_or("Voting name placeholder absent")?;
            notice.splice(at..at + 2, self.encode(&row.name)?);
        }
        self.text(
            &mut surfaces,
            3,
            &notice,
            self.region("VOTING_TITLE_LABEL")?,
        )?;
        self.text(
            &mut surfaces,
            1,
            self.strings
                .get(0x603)
                .ok_or("Voting title language absent")?,
            self.region("VOTING_TITLE_AREA")?,
        )?;
        let button = self.region("VOTING_OK_BUTTON")?;
        self.icon(
            &mut surfaces,
            if pressed { 81 } else { 79 },
            button,
            u32::MAX,
        )?;
        self.text(
            &mut surfaces,
            2,
            self.strings
                .get(0x59b)
                .ok_or("Voting button language absent")?,
            button,
        )?;
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            VIEW,
            None,
        )
    }
}
fn encode_raw(raw: &[u8], alphabet: &[u8]) -> Result<Vec<u8>, String> {
    raw.iter()
        .map(|b| {
            b.checked_sub(32)
                .and_then(|i| alphabet.get(usize::from(i)))
                .copied()
                .ok_or("Voting glyph absent".into())
        })
        .collect()
}
fn bar_top(graph: [f32; 4], percentage: f32) -> f32 {
    // 42c4b0 keeps the percentage/scale/span operations in x87 until FST.
    (f64::from(graph[3])
        - f64::from(percentage) * f64::from(0.01_f32) * (f64::from(graph[3]) - f64::from(graph[1])))
        as f32
}

#[derive(Default)]
pub struct View {
    pub prepared: Option<PreparedScene>,
    snapshot: Option<Snapshot>,
    pressed: bool,
    down: bool,
    dirty: bool,
}
impl View {
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        snapshot: Option<&Snapshot>,
    ) -> Result<(), String> {
        if self.snapshot.as_ref() != snapshot || self.dirty {
            self.prepared = snapshot
                .map(|s| assets.prepare(renderer, s, self.pressed))
                .transpose()?;
            self.snapshot = snapshot.cloned();
            self.dirty = false;
            if snapshot.is_none() {
                self.pressed = false;
                self.down = false;
            }
        }
        Ok(())
    }
    pub fn pointer(&mut self, assets: &Assets, point: Option<[f32; 2]>, down: bool) -> bool {
        let hit = point.is_some_and(|p| assets.button_at(p));
        let was_down = std::mem::replace(&mut self.down, down);
        if down {
            if !was_down {
                self.pressed = hit;
                self.dirty = true;
            }
            false
        } else {
            self.dirty |= self.pressed;
            std::mem::take(&mut self.pressed) && hit
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::{
        player_manager::Player,
        player_share::{self, CallbackEffect, Controls},
    };

    fn players(incomes: &[i32]) -> Vec<Option<Player>> {
        incomes
            .iter()
            .enumerate()
            .map(|(slot, income)| {
                let mut bytes = vec![0; 0x220];
                bytes[0x10] = 1;
                bytes[0x150..0x154].copy_from_slice(&(slot as u32).to_le_bytes());
                bytes[0x158..0x15d].copy_from_slice(b"Alice");
                bytes[0x1ac..0x1b0].copy_from_slice(&income.to_le_bytes());
                bytes[0x14c..0x150].copy_from_slice(&(slot as u32 + 1).to_le_bytes());
                Some(Player {
                    reference: 0x1000 + slot as u32 * 0x1000,
                    bytes,
                    auxiliaries: Vec::new(),
                })
            })
            .collect()
    }
    fn nightly(state: &mut State, objective: i32, players: &mut [Option<Player>]) -> bool {
        let mut result_proceeded = false;
        player_share::apply_with_state_callback(
            Controls {
                manager_kind: objective,
                manager_value: 10.,
                day: 10,
                player_count: players.len(),
                network_enabled: false,
                network_override: false,
                current_slot: 0,
            },
            None,
            [0.; 6],
            players,
            |call, _, _, players| {
                match call {
                    Call::Refresh
                    | Call::ProfileSelect { .. }
                    | Call::Kind1ProfileSelect { .. }
                    | Call::PlayerShare { .. } => {
                        let records: Vec<_> = players
                            .iter()
                            .map(|p| {
                                p.as_ref()
                                    .map_or_else(|| vec![0; 0x220], |p| p.bytes.clone())
                            })
                            .collect();
                        state.apply(call, objective, &records)?;
                    }
                    Call::CommandProducer { slot } => {
                        // The real callback order must populate the candidate and
                        // every share before the elimination outcome is produced.
                        let snapshot = state.snapshot.as_ref().unwrap();
                        assert_eq!(snapshot.candidate, Some(slot as usize));
                        assert!(
                            snapshot
                                .rows
                                .iter()
                                .take(players.len())
                                .all(Option::is_some)
                        );
                        result_proceeded = true;
                    }
                    _ => (),
                }
                Ok(CallbackEffect::default())
            },
        )
        .unwrap();
        result_proceeded
    }
    #[test]
    fn mayor_callbacks_present_names_shares_and_incremented_rosettes() {
        let mut state = State::default();
        let mut players = players(&[75, 25]);
        assert!(!nightly(&mut state, 1, &mut players));
        let snapshot = state.snapshot.as_ref().unwrap();
        assert_eq!(snapshot.candidate, Some(0));
        assert_eq!(snapshot.notice, 0x604);
        let row = snapshot.rows[0].as_ref().unwrap();
        assert_eq!(row.name, b"Alice");
        assert_eq!(row.character, 0);
        assert_eq!(row.percentage, 75.);
        assert_eq!(row.progress, 2);
        assert_eq!(row.percentage_text(), "75.000000%");
        assert_eq!(row.progress_text(), "2");
        assert_eq!(snapshot.rows[1].as_ref().unwrap().percentage, 25.);
        assert_eq!(snapshot.rows[1].as_ref().unwrap().progress, 2);
        assert!(snapshot.rows[2..].iter().all(Option::is_none));
    }
    #[test]
    fn weakest_link_callbacks_present_before_the_elimination_outcome() {
        let mut state = State::default();
        assert!(nightly(&mut state, 2, &mut players(&[25, 75])));
        let snapshot = state.snapshot.as_ref().unwrap();
        assert_eq!(snapshot.candidate, Some(0));
        assert_eq!(snapshot.notice, 0x605);
        assert_eq!(snapshot.rows[0].as_ref().unwrap().percentage, 25.);
    }
    #[test]
    fn tied_night_clears_previous_candidate_and_keeps_native_notice() {
        for (kind, notice) in [(1, 0x606), (2, 0x607)] {
            let mut state = State::default();
            nightly(&mut state, kind, &mut players(&[25, 75]));
            assert!(state.snapshot.as_ref().unwrap().candidate.is_some());
            let mut tied = players(&[50, 50, 0]);
            tied[2].as_mut().unwrap().bytes[0x10] = 0;
            assert!(!nightly(&mut state, kind, &mut tied));
            let snapshot = state.snapshot.as_ref().unwrap();
            assert_eq!(snapshot.candidate, None);
            assert_eq!(snapshot.notice, notice);
            assert_eq!(snapshot.rows[0].as_ref().unwrap().percentage, 50.);
            assert_eq!(snapshot.rows[1].as_ref().unwrap().percentage, 50.);
            assert!(snapshot.rows[2].is_none());
        }
    }
    #[test]
    fn display_lasts_750_interface_ticks_or_until_dismissal() {
        let mut state = State::default();
        nightly(&mut state, 1, &mut players(&[75, 25]));
        for _ in 0..749 {
            state.interface_tick();
        }
        assert!(state.snapshot.is_some());
        state.interface_tick();
        assert!(state.snapshot.is_none());
        nightly(&mut state, 1, &mut players(&[75, 25]));
        state.dismiss();
        assert!(state.snapshot.is_none());
    }
    #[test]
    fn literal_names_and_numbers_use_the_language_character_map() {
        let mut alphabet = (0..=255).collect::<Vec<u8>>();
        alphabet[usize::from(b'A' - 32)] = 7;
        assert_eq!(
            encode_raw(b"Alice 75.000000%", &alphabet).unwrap(),
            [vec![7], b"lice 75.000000%".iter().map(|b| b - 32).collect()].concat()
        );
        assert!(encode_raw(b"\x01", &alphabet).is_err());
    }
    #[test]
    fn bar_endpoints_and_signed_count_follow_original_format() {
        assert_eq!(bar_top([0., 20., 100., 220.], 0.), 220.);
        assert_eq!(bar_top([0., 20., 100., 220.], 50.), 120.);
        assert!((bar_top([0., 20., 100., 220.], 100.) - 20.).abs() < 0.00001);
        let row = Row {
            name: Vec::new(),
            character: 0,
            percentage: 12.345,
            progress: u32::MAX,
        };
        assert_eq!(row.percentage_text(), "12.345000%");
        assert_eq!(row.progress_text(), "-1");
    }
}
