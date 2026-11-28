//! Original413de0/4141a0 bid controls and415aa0/415bd0 turn timer geometry.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

struct Panel {
    base: Vec<Surface>,
    fill: Surface,
    title: [f32; 4],
    regions: Vec<openmt2001::ui_mesh::Region>,
    backing: [f32; 4],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub category: u32,
    pub category_color: u32,
    pub character: u32,
    pub actor: Vec<u8>,
    pub price: u32,
}
impl Info {
    pub fn from_session(session: &openmt2001::session::Session<'_>) -> Result<Self, String> {
        let record = &session.world.city.auction.controller.record;
        let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let category = word(4);
        let row = session
            .world
            .categories
            .get(category as usize)
            .ok_or("Auction title category outside pool")?;
        let player = session
            .roster
            .manager
            .players
            .get(word(0x5c) as usize)
            .ok_or("Auction high bidder outside roster")?;
        Ok(Self {
            category,
            category_color: u32::from_le_bytes(row[0x94..0x98].try_into().unwrap()),
            character: u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()),
            actor: player.bytes[0x158..0x1b8].to_vec(),
            price: word(0x6c),
        })
    }
}

#[test]
#[ignore = "requires original auction panel resources"]
fn original_auction_panel_resources() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let exe = read("OPENMT2001_EXE");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameters.parent().unwrap().parent().unwrap();
    let exe_path = openmt2001::fixtures::path("EXE");
    let language = exe_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let offsets = fs::read(language.join("tycoon.off")).unwrap();
    let bytes = fs::read(language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings = offsets
        .chunks_exact(2)
        .map(|n| {
            let end = at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
            let text = &bytes[at..end];
            at = end;
            text
        })
        .collect::<Vec<_>>();
    let assets = Assets::load(&index, &archive, installer, &strings, &exe).unwrap();
    let hidden_fixture =
        fs::read(openmt2001::fixtures::path("NATIVE_AUCTION_HIDDEN_BUTTON")).unwrap();
    assert_eq!(&hidden_fixture[..8], b"MTAHIT01");
    assert_eq!(hidden_fixture.len(), 8 + 8 + 8 * 4 + 4 * 4);
    let word = |at: usize| u32::from_le_bytes(hidden_fixture[at..at + 4].try_into().unwrap());
    assert_eq!(word(8), 5);
    assert_eq!(word(12), 2);
    let title = assets.bid.title;
    for (i, value) in title.into_iter().enumerate() {
        assert_eq!(value.to_bits(), word(16 + i * 4), "original bid title {i}");
    }
    for (i, value) in assets.hidden_bid_toggle.into_iter().enumerate() {
        assert_eq!(
            value.to_bits(),
            word(32 + i * 4),
            "original hidden hitbox {i}"
        );
    }
    for i in 0..4 {
        assert_eq!(word(48 + i * 4), u32::MAX);
    }
    for bounds in assets.buttons {
        assert!(bounds[2] > bounds[0] && bounds[3] > bounds[1]);
    }
    let auction = crate::live_host::Auction {
        visible: true,
        hidden_bid_toggle_enabled: true,
        controls: crate::auction_controls::State {
            buttons_enabled: 31,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut view = View::default();
    for (index, bounds) in assets.buttons.iter().enumerate() {
        let point = [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5];
        assert_eq!(
            view.tutorial_control_at(&assets, Some(point), &auction),
            Some([65, 66, 67, 70, 69][index])
        );
        assert_eq!(view.mouse(&assets, &auction, Some(point), true), None);
        assert!(view.pointer_click_cue(&assets, &auction, Some(point)));
        assert!(!view.pointer_click_cue(&assets, &auction, Some(point)));
        assert!(!view.pointer_click_cue(&assets, &auction, None));
        assert!(view.pointer_click_cue(&assets, &auction, Some(point)));
        assert!(view.tutorial_control_pressed());
        assert_eq!(
            view.mouse(&assets, &auction, Some(point), false),
            Some(index as u32)
        );
        assert!(!view.pointer_click_cue(&assets, &auction, Some(point)));
        assert!(!view.tutorial_control_pressed());
        view.mouse(&assets, &auction, Some(point), true);
        assert_eq!(view.mouse(&assets, &auction, None, false), None);
        view.mouse(&assets, &auction, Some(point), true);
        view.cancel_press();
        assert_eq!(view.mouse(&assets, &auction, Some(point), false), None);
    }
    let hidden = assets.hidden_bid_toggle;
    let title = assets.bid.title;
    assert_eq!(hidden[0].to_bits(), (title[2] - 16.).to_bits());
    assert_eq!(hidden[1..], title[1..]);
    let point = [(hidden[0] + hidden[2]) * 0.5, (hidden[1] + hidden[3]) * 0.5];
    assert_eq!(
        view.tutorial_control_at(&assets, Some(point), &auction),
        None
    );
    assert_eq!(view.mouse(&assets, &auction, Some(point), true), None);
    assert!(view.pointer_click_cue(&assets, &auction, Some(point)));
    assert_eq!(view.mouse(&assets, &auction, Some(point), false), Some(5));
    let network = crate::live_host::Auction {
        hidden_bid_toggle_enabled: false,
        ..auction.clone()
    };
    assert_eq!(view.mouse(&assets, &network, Some(point), true), None);
    assert!(!view.pointer_click_cue(&assets, &network, Some(point)));
    assert_eq!(view.mouse(&assets, &network, Some(point), false), None);
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_AUCTION_PANEL_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let state = crate::auction_controls::State {
            ticks: 1,
            price_label: Some(1250),
            buttons_enabled: 31,
            timer_visible: true,
            timer_text: Some(crate::auction_controls::TimerText::Seconds(15)),
            timer: crate::auction_controls::TurnTimer {
                last: 15,
                ..Default::default()
            },
            ..Default::default()
        };
        let board = openmt2001::board::Board::from_v14(&PeImage::parse(&exe).unwrap()).unwrap();
        let info = Info {
            category: 33,
            category_color: board
                .blocks
                .iter()
                .find(|b| b.id == 33)
                .unwrap()
                .pavement_color,
            character: 2,
            actor: b"Dog\0".to_vec(),
            price: 1250,
        };
        let scene =
            crate::auction_scene::Assets::load(&PeImage::parse(&exe).unwrap(), &index, &archive)
                .unwrap()
                .prepare(&renderer, &[0, 2, 8, 9])
                .unwrap();
        // Exercise the live View cache across state changes as well as the
        // original draw path, including the network-suppressed presentation.
        let mut view = View::default();
        for (suffix, enabled, selected) in [
            ("", true, false),
            ("-selected.png", true, true),
            ("-suppressed.png", false, true),
            ("-reset.png", true, false),
        ] {
            let auction = crate::live_host::Auction {
                visible: true,
                controls: state.clone(),
                hidden_bid_toggle_enabled: enabled,
                bid_voice_gate: selected,
                info: Some(info.clone()),
                ..Default::default()
            };
            view.prepare(&assets, &renderer, Some(&auction), None)
                .unwrap();
            crate::capture_test::capture_overlay(
                &renderer,
                &scene,
                view.prepared.as_ref().unwrap(),
                &format!("{path}{suffix}"),
            );
        }
    }
}
pub struct Assets {
    bid: Panel,
    timer: Panel,
    top: Panel,
    background: u32,
    category_titles: Vec<u32>,
    character_colors: Vec<u32>,
    portraits: Vec<Surface>,
    pub buttons: [[f32; 4]; 5],
    //414040 creates a textureless generic child over the last 16px of the
    // bid-amount title. Its event12 index5 toggles the bidder voice gate.
    hidden_bid_toggle: [f32; 4],
    textures: [[Surface; 3]; 5],
    toggle_indicators: [Surface; 2],
    fonts: [Vec<Surface>; 2],
    tabs: [Vec<u8>; 2],
    label_font: Vec<Surface>,
    label_tab: Vec<u8>,
    text_colors: Vec<u32>,
    strings: Vec<Vec<u8>>,
    timer_labels: [Vec<u8>; 2],
}
#[derive(Default)]
pub struct View {
    pressed: Option<usize>,
    held: bool,
    cue_latch: Option<usize>,
    key: Option<(crate::live_host::Auction, Option<usize>, bool)>,
    pub prepared: Option<PreparedScene>,
}
impl View {
    fn hover(
        assets: &Assets,
        pointer: Option<[f32; 2]>,
        auction: &crate::live_host::Auction,
    ) -> Option<usize> {
        let [x, y] = pointer?;
        assets
            .buttons
            .iter()
            .enumerate()
            .find_map(|(i, b)| {
                (auction.visible
                    && auction.controls.buttons_enabled & (1 << i) != 0
                    && x >= b[0]
                    && x <= b[2]
                    && y >= b[1]
                    && y <= b[3])
                    .then_some(i)
            })
            .or_else(|| {
                let b = assets.hidden_bid_toggle;
                (auction.visible
                    && auction.hidden_bid_toggle_enabled
                    && x >= b[0]
                    && x <= b[2]
                    && y >= b[1]
                    && y <= b[3])
                    .then_some(5)
            })
    }
    ///413de0 binds50/100/500/PullOut/Pass to globals65/66/67/70/69.
    pub fn tutorial_control_at(
        &self,
        assets: &Assets,
        pointer: Option<[f32; 2]>,
        auction: &crate::live_host::Auction,
    ) -> Option<u32> {
        Self::hover(assets, pointer, auction)
            .and_then(|index| [65, 66, 67, 70, 69].get(index).copied())
    }
    pub fn tutorial_control_pressed(&self) -> bool {
        self.pressed.is_some()
    }
    pub fn cancel_press(&mut self) {
        self.pressed = None;
        self.held = false;
        self.cue_latch = None;
    }
    /// All six bidder children use generic 43ed20/443360. Its +0x330 latch
    /// sounds on first held entry, clears on leave, and stays silent on hold.
    pub fn pointer_click_cue(
        &mut self,
        assets: &Assets,
        auction: &crate::live_host::Auction,
        pointer: Option<[f32; 2]>,
    ) -> bool {
        if !self.held {
            self.cue_latch = None;
            return false;
        }
        let hit = Self::hover(assets, pointer, auction);
        let cue = hit.is_some() && hit != self.cue_latch;
        self.cue_latch = hit;
        cue
    }
    pub fn mouse(
        &mut self,
        assets: &Assets,
        auction: &crate::live_host::Auction,
        pointer: Option<[f32; 2]>,
        down: bool,
    ) -> Option<u32> {
        let hover = Self::hover(assets, pointer, auction);
        if down {
            self.held = true;
            self.pressed = hover;
            None
        } else {
            self.held = false;
            self.cue_latch = None;
            let result = self.pressed.filter(|p| Some(*p) == hover).map(|p| p as u32);
            self.pressed = None;
            result
        }
    }
    pub fn prepare(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        auction: Option<&crate::live_host::Auction>,
        pointer: Option<[f32; 2]>,
    ) -> Result<(), String> {
        let key = auction.filter(|a| a.visible).map(|a| {
            let hover = Self::hover(assets, pointer, a);
            (a.clone(), hover, hover.is_some() && self.pressed == hover)
        });
        if self.key != key {
            self.prepared = key
                .as_ref()
                .map(|(a, h, p)| {
                    assets.prepare_with_info(
                        renderer,
                        &a.controls,
                        *h,
                        *p,
                        a.info.as_ref(),
                        &a.participants,
                        toggle_indicator(a.hidden_bid_toggle_enabled, a.bid_voice_gate),
                    )
                })
                .transpose()?;
            self.key = key;
        }
        Ok(())
    }
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
        exe: &[u8],
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("Missing auction panel {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("Auction panel range".into())
        };
        let image = PeImage::parse(exe)?;
        let color = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        let panel =
            |name: &str, backing: &str, title: &str, parts: &[usize]| -> Result<Panel, String> {
                let mesh = CachedMesh::parse(payload(name)?)?;
                let regions = openmt2001::ui_mesh::regions(&mesh, 0)?;
                let bounds = |name: &str| {
                    regions
                        .iter()
                        .find(|r| r.name == name)
                        .map(|r| r.bounds)
                        .ok_or_else(|| format!("Missing auction region {name}"))
                };
                let title = bounds(title)?;
                let backing = bounds(backing)?;
                let mut fill = Surface::solid();
                fill.rect(backing, [color; 4]);
                fill.rect(title, [0xff909090, 0xff909090, 0x7f909090, 0x7f909090]);
                //43c3a0 draws the widget mesh before its custom draw callback.
                let base = ui::panel_frame_base(index, archive, name, parts, 0)?.0;
                Ok(Panel {
                    base,
                    fill,
                    title,
                    regions,
                    backing,
                })
            };
        let bid = panel(
            "gamedata2/brand_new_panels/new_auction_bid_v2.tm",
            "AUCTION_BID_BACKING",
            "AUCTION_BID_BID_AMOUNT",
            &[1],
        )?;
        let hidden_bid_toggle = [bid.title[2] - 16., bid.title[1], bid.title[2], bid.title[3]];
        let timer = panel(
            "gamedata2/brand_new_panels/new_auction_timer.tm",
            "AUCTION_TIMER_BACKING",
            "AUCTION_TIMER_TITLE_LABEL",
            &[0],
        )?;
        let top = panel(
            "gamedata2/brand_new_panels/new_auction_top.tm",
            "AUCTION_TOP_BACKING",
            "AUCTION_TOP_SALE_AREA",
            &[0],
        )?;
        let buttons = [
            "AUCTION_BID50",
            "AUCTION_BID100",
            "AUCTION_BID500",
            "AUCTION_PULLOUT_BUTTON",
            "AUCTION_PASS_BUTTON",
        ]
        .map(|name| {
            bid.regions
                .iter()
                .find(|r| r.name == name)
                .map(|r| r.bounds)
                .ok_or_else(|| format!("Missing auction button {name}"))
        });
        let [a, b, c, d, e] = buttons;
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let textures = [128, 131, 134, 85, 82]
            .map(|first| std::array::from_fn(|i| Surface::new(&generic.textures[first + i])));
        let portraits = (0..10)
            .map(|character| {
                let texture =
                    u32::from_le_bytes(image.at(0x541b00 + character * 40, 4)?.try_into().unwrap());
                generic
                    .textures
                    .get(texture as usize)
                    .map(Surface::new)
                    .ok_or("Auction high bidder portrait index".into())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let font = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let tabs = ["tahoma12bldnoantip.tab", "tahoma10bldnoantip.tab"].map(|name| {
            fs::read(installer.join("Standard/fonts").join(name)).map_err(|e| e.to_string())
        });
        let [title, body] = tabs;
        let timer_labels = [0x533a5c, 0x533a74].map(|address| {
            let bytes = image.at(address, 24)?;
            let end = bytes
                .iter()
                .position(|b| *b == 0)
                .ok_or("Auction timer label terminator")?;
            Ok(bytes[..end].iter().map(|b| b - 32).collect::<Vec<_>>())
        });
        let [waiting, sold]: [Result<Vec<u8>, String>; 2] = timer_labels;
        Ok(Self {
            bid,
            timer,
            top,
            background: color,
            category_titles: (0..52)
                .map(|i| {
                    image
                        .at(0x5385a0 + i * 88, 4)
                        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                })
                .collect::<Result<_, _>>()?,
            character_colors: (0..10)
                .map(|i| {
                    image
                        .at(0x541af0 + i * 40, 4)
                        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                })
                .collect::<Result<_, _>>()?,
            buttons: [a?, b?, c?, d?, e?],
            hidden_bid_toggle,
            textures,
            // 4141a0 selects globals68b274/68b278, entries275/276 in
            // the generic image bank initialized at4c0f9f.
            toggle_indicators: [275, 276].map(|i| Surface::new(&generic.textures[i])),
            portraits,
            fonts: [
                font.textures[2..4].iter().map(Surface::new).collect(),
                font.textures[6..8].iter().map(Surface::new).collect(),
            ],
            tabs: [title?, body?],
            label_font: font.textures[0..2].iter().map(Surface::new).collect(),
            label_tab: fs::read(installer.join("Standard/fonts/cust640p.tab"))
                .map_err(|e| e.to_string())?,
            text_colors: (0..18)
                .map(|i| {
                    image
                        .at(0x536288 + i * 4, 4)
                        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                })
                .collect::<Result<_, _>>()?,
            strings: crate::tooltip::localized_strings(installer, strings)?,
            timer_labels: [waiting?, sold?],
        })
    }
    fn text(
        &self,
        surfaces: &mut Vec<Surface>,
        font: usize,
        id: usize,
        bounds: [f32; 4],
        substitution: Option<&[u8]>,
    ) -> Result<(), String> {
        self.text_values(
            surfaces,
            font,
            id,
            bounds,
            &substitution.into_iter().collect::<Vec<_>>(),
        )
    }
    fn text_values(
        &self,
        surfaces: &mut Vec<Surface>,
        font: usize,
        id: usize,
        bounds: [f32; 4],
        substitutions: &[&[u8]],
    ) -> Result<(), String> {
        let mut text = self
            .strings
            .get(id)
            .ok_or("Auction language index")?
            .clone();
        for value in substitutions {
            let at = text
                .windows(2)
                .position(|s| {
                    s[0] == b'%' - 32 && [b'd' - 32, b's' - 32, b'i' - 32].contains(&s[1])
                })
                .ok_or("Auction language placeholder")?;
            text.splice(at..at + 2, value.iter().copied());
        }
        self.text_content(surfaces, font, bounds, &text)
    }
    fn text_content(
        &self,
        surfaces: &mut Vec<Surface>,
        font: usize,
        bounds: [f32; 4],
        text: &[u8],
    ) -> Result<(), String> {
        let mut pages = self.fonts[font].clone();
        let (offsets, copy) = crate::prompt::wrap(
            &self.tabs[font],
            text,
            (bounds[2] - bounds[0]).trunc() as i32,
        )?;
        let height = [12., 10.][font];
        let top = (bounds[1]
            + ((bounds[3] - bounds[1]) - (offsets.len() - 1) as f32 * height) * 0.5)
            .trunc();
        for (i, pair) in offsets.windows(2).enumerate() {
            let end = (pair[1] - 1).min(copy.len());
            ui::text(
                &mut pages,
                &self.tabs[font],
                &copy[pair[0].min(end)..end],
                [bounds[0], top + i as f32 * height, bounds[2], bounds[3]],
                true,
                u32::MAX,
            )?;
        }
        surfaces.extend(pages);
        Ok(())
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        state: &crate::auction_controls::State,
        hover: Option<usize>,
        pressed: bool,
    ) -> Result<PreparedScene, String> {
        self.prepare_with_info(
            renderer,
            state,
            hover,
            pressed,
            None,
            &[],
            toggle_indicator(true, false),
        )
    }
    fn participant_surfaces(
        &self,
        surfaces: &mut Vec<Surface>,
        participants: &[crate::auction_labels::Participant],
    ) -> Result<(), String> {
        if participants.is_empty() {
            return Ok(());
        }
        let characters = participants.iter().map(|p| p.character).collect::<Vec<_>>();
        let poses = crate::auction_scene::initial_layout(&characters)?;
        let camera = glam::Mat4::from_cols_array(&crate::auction_scene::Assets::camera_matrix()?);
        let rect = |[x, y, w, h]: [f32; 4]| [x, y, x + w, y + h];
        for (participant, pose) in participants.iter().zip(&poses[7..]) {
            // 414c40 projects the participant anchor one world unit below
            // their seat, using the same camera and aspect as the scene.
            let [x, y, z] = pose.position;
            let clip = camera * glam::Vec4::new(x, y - 1., z, 1.);
            let projected = [(clip.x / clip.w + 1.) * 320., (1. - clip.y / clip.w) * 240.];
            let bounds = crate::auction_labels::Bounds::from_projected(
                projected,
                [640, 480],
                participant.high_bidder,
                participant.winner,
            );
            let name_cash = rect(bounds.name_cash);
            let status = rect(bounds.status);
            let color = *self
                .character_colors
                .get(participant.character as usize)
                .ok_or("Auction participant character outside palette")?;
            let gradient = [
                color | 0xff000000,
                color | 0xff000000,
                (color & 0xffffff) | 0x7f000000,
                (color & 0xffffff) | 0x7f000000,
            ];
            let mut fill = Surface::solid();
            fill.rect(rect(bounds.background), [0x7f000000; 4]);
            for [l, t, r, b] in [name_cash, status] {
                fill.rect([l, t, r, b], gradient);
                for (a, z) in [
                    ([l, t], [r, t]),
                    ([r, t], [r, b]),
                    ([r, b], [l, b]),
                    ([l, b], [l, t]),
                ] {
                    fill.line(a, z, 0xff000000);
                }
            }
            surfaces.push(fill);
            let [l, t, r, b] = name_cash;
            let mut portrait = self.portraits[participant.character as usize].clone();
            portrait.quad(
                [[l, t], [l, b], [l + 23., b], [l + 23., t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [u32::MAX; 4],
            );
            surfaces.push(portrait);
            let name = participant
                .name
                .iter()
                .map(|&b| {
                    b.checked_sub(32)
                        .filter(|&b| b <= 94)
                        .ok_or("Auction participant name glyph mapping")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let money = |value| {
                ui::integer_money(
                    value,
                    &self.strings[2243],
                    self.strings[2244].first() != Some(&0x4c),
                )
            };
            let mut pages = self.label_font.clone();
            ui::text_fit(
                &mut pages,
                &self.label_tab,
                &name,
                [l + 24., t + 2., r - 2., t + 12.],
                true,
                self.text_colors[0],
            )?;
            ui::text_fit(
                &mut pages,
                &self.label_tab,
                &money(participant.label.cash),
                [l + 24., t + 12., r - 2., b - 2.],
                true,
                self.text_colors[participant.label.cash_color as usize],
            )?;
            surfaces.extend(pages);
            let (text, tab, mut pages) = match participant.label.status {
                crate::auction_labels::Text::Empty => continue,
                crate::auction_labels::Text::Language(id) => (
                    self.strings
                        .get(id as usize)
                        .ok_or("Auction participant status language index")?
                        .clone(),
                    &self.label_tab,
                    self.label_font.clone(),
                ),
                crate::auction_labels::Text::Money(value) => {
                    (money(value), &self.tabs[0], self.fonts[0].clone())
                }
            };
            ui::text_widget_style4(&mut pages, tab, &text, status, self.text_colors[0])?;
            surfaces.extend(pages);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_with_info(
        &self,
        renderer: &Renderer,
        state: &crate::auction_controls::State,
        hover: Option<usize>,
        pressed: bool,
        info: Option<&Info>,
        participants: &[crate::auction_labels::Participant],
        toggle: Option<usize>,
    ) -> Result<PreparedScene, String> {
        let mut surfaces = self.bid.base.clone();
        surfaces.push(self.bid.fill.clone());
        self.participant_surfaces(&mut surfaces, participants)?;
        if let Some(info) = info {
            let bounds = |name| {
                self.top
                    .regions
                    .iter()
                    .find(|r| r.name == name)
                    .map(|r| r.bounds)
                    .ok_or_else(|| format!("Missing auction top region {name}"))
            };
            let character = *self
                .character_colors
                .get(info.character as usize)
                .ok_or("Auction high bidder character outside palette")?;
            let gradient = |rgb: u32| {
                [
                    rgb | 0xff000000,
                    rgb | 0xff000000,
                    (rgb & 0xffffff) | 0x7f000000,
                    (rgb & 0xffffff) | 0x7f000000,
                ]
            };
            let mut fill = Surface::solid();
            fill.rect(self.top.backing, [self.background; 4]);
            fill.rect(self.top.title, gradient(info.category_color));
            fill.rect(bounds("AUCTION_TOP_BID_SHADING")?, gradient(character));
            surfaces.extend(self.top.base.iter().cloned());
            surfaces.push(fill);
            let title = *self
                .category_titles
                .get(info.category as usize)
                .ok_or("Auction title outside registry")?;
            let category = self
                .strings
                .get(title as usize)
                .ok_or("Auction category language index")?;
            self.text_values(&mut surfaces, 0, 1563, self.top.title, &[category])?;
            let amount = ui::integer_money(
                info.price as i32,
                &self.strings[2243],
                self.strings[2244].first() != Some(&0x4c),
            );
            let end = info
                .actor
                .iter()
                .position(|b| *b == 0)
                .ok_or("Unterminated auction high bidder")?;
            let actor = info.actor[..end]
                .iter()
                .map(|b| {
                    b.checked_sub(32)
                        .filter(|b| *b <= 94)
                        .ok_or("Auction actor glyph mapping")
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.text_values(
                &mut surfaces,
                1,
                1564,
                bounds("AUCTION_TOP_BID_INFO_LABEL")?,
                &[&amount, &actor],
            )?;
            let mut portrait = self
                .portraits
                .get(info.character as usize)
                .ok_or("Auction high bidder portrait outside roster")?
                .clone();
            let [l, t, r, b] = bounds("AUCTION_TOP_BID_ICON")?;
            portrait.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [u32::MAX; 4],
            );
            surfaces.push(portrait);
        }
        if state.price_label.is_some() {
            //414b17 passes only language1549 to441220; the preceding formatted
            //price buffer is unused in retail1.4.
            self.text(&mut surfaces, 0, 1549, self.bid.title, None)?;
        } else if state.ticks == 0 {
            self.text(&mut surfaces, 0, 1550, self.bid.title, None)?;
        }
        if let Some(image) = toggle {
            // 413e55 configures language1551 in font1, style5, inset20,
            // over the bid title. The indicator uses its final16px strip.
            let mut bounds = self.bid.title;
            bounds[2] -= 20.;
            let mut pages = self.fonts[0].clone();
            ui::text_widget_style5(
                &mut pages,
                &self.tabs[0],
                self.strings
                    .get(1551)
                    .ok_or("Auction toggle language index")?,
                bounds,
                u32::MAX,
            )?;
            surfaces.extend(pages);
            let mut indicator = self.toggle_indicators[image - 275].clone();
            let [l, t, r, b] = self.hidden_bid_toggle;
            indicator.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [u32::MAX; 4],
            );
            surfaces.push(indicator);
        }
        for i in 0..5 {
            let over = hover == Some(i) && state.buttons_enabled & (1 << i) != 0;
            let mut surface =
                self.textures[i][if over { if pressed { 2 } else { 1 } } else { 0 }].clone();
            let [l, t, r, b] = self.buttons[i];
            surface.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [u32::MAX; 4],
            );
            surfaces.push(surface);
            if i >= 3 {
                self.text(&mut surfaces, 1, [1553, 1554][i - 3], self.buttons[i], None)?;
            }
        }
        if state.timer_visible {
            surfaces.extend(self.timer.base.clone());
            surfaces.push(self.timer.fill.clone());
            self.text(&mut surfaces, 0, 1561, self.timer.title, None)?;
            let body = self
                .timer
                .regions
                .iter()
                .find(|r| r.name == "AUCTION_TIMER_TIMER_AREA")
                .ok_or("Auction timer text area")?
                .bounds;
            match state.timer_text {
                Some(crate::auction_controls::TimerText::Seconds(seconds)) => {
                    let encoded = seconds
                        .to_string()
                        .bytes()
                        .map(|b| b - 32)
                        .collect::<Vec<_>>();
                    self.text(&mut surfaces, 1, 1562, body, Some(&encoded))?;
                }
                Some(crate::auction_controls::TimerText::Label { no_sale }) => {
                    self.text_content(
                        &mut surfaces,
                        1,
                        body,
                        &self.timer_labels[usize::from(no_sale)],
                    )?;
                }
                None => {}
            }
        }
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            [
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
            ],
            None,
        )
    }
}

#[cfg(test)]
mod bid_facing_tests {
    use super::*;
    fn area(p: &[[f32; 3]], ids: [usize; 3]) -> f64 {
        let [a, b, c] = ids.map(|i| [f64::from(p[i][0]), -f64::from(p[i][1])]);
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    #[test]
    fn own_pose_oracle_rejects_clockwise_and_degenerate_triangles() {
        let p = [[0., 0., 8.], [0., -2., 8.], [3., -2., 8.], [1.5, -1., 8.]];
        assert_eq!(area(&p, [0, 1, 2]), -6.);
        assert_eq!(area(&p, [0, 2, 1]), 6.);
        assert_eq!(area(&p, [0, 3, 2]), 0.);
        assert_eq!(area(&p, [0, 0, 1]), 0.);
        assert_eq!(area(&p.map(|[x, y, z]| [x, -y, z]), [0, 1, 2]), 6.);
        assert_eq!(
            area(&p.map(|[x, y, _]| [x + 12., y - 4., -9.]), [0, 1, 2]),
            -6.
        );
    }
    fn guard(bytes: &[u8], expected: &str) {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        assert_eq!(
            std::str::from_utf8(&result.stdout)
                .unwrap()
                .split_whitespace()
                .next(),
            Some(expected)
        );
    }
    fn same(a: &Surface, b: &Surface) {
        assert_eq!(a.vertices, b.vertices);
        assert_eq!(a.alpha, b.alpha);
        assert_eq!((a.width, a.height), (b.width, b.height));
        assert_eq!(a.pixels, b.pixels);
        assert!(matches!(
            a.draw().mode,
            openmt2001::render::SurfaceMode::Overlay
        ));
    }
    #[test]
    #[ignore = "SHA-pinned original auction meshes/materials/PE/font/language; CPU fixture, optional capture"]
    fn supplied_panels_cull_reversed_faces_and_preserve_fills_and_regions() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let exe = read("OPENMT2001_EXE");
        guard(
            &directory,
            "8ecb0d18a3896eba61db9ff6edc10869a84a64229169b52f0f7d981b81980d52",
        );
        guard(
            &exe,
            "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b",
        );
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let payload = |name: &str| {
            let e = index.find(name).unwrap();
            &archive[e.offset as usize..(e.offset + e.length) as usize]
        };
        for (name, hash) in [
            (
                "gamedata2/brand_new_panels/new_auction_bid_v2.tm",
                "350a047ad94909a687878e5b74fdd36bd143d0225bb06f1885b6243c67eafaf2",
            ),
            (
                "gamedata2/brand_new_panels/new_auction_timer.tm",
                "9a44fe1e200310f2333ad816b1d9cfa0bfa660bf3ae570e78089b99e6c16b2db",
            ),
            (
                "gamedata2/brand_new_panels/new_auction_top.tm",
                "eb9783ff1fa45c81d44088afac3b9f0390b59123772bf6d1dafd8160a774b06a",
            ),
            (
                "1New_Maya_grey_border.tt",
                "31a3aafc028c5b3798eb8fb8eca314c1f2cf4e88b0fd3af3204d9c004396cd9f",
            ),
            (
                "gamedata2/GenericTextures640/textures.gtd",
                "10a278bd4ed76076f5b517069f1c8a96ddc7a8b3bb282ff8998842fa8beb88f4",
            ),
            (
                "gamedata2/font640/textures.gtd",
                "21ed8e328fc95d98eb8cf6f99927d2dc28bd7b3bd309b67bbfbbec52f9be5912",
            ),
        ] {
            guard(payload(name), hash);
        }
        let param = openmt2001::fixtures::path("PARAMETERS");
        let installer = param.parent().unwrap().parent().unwrap();
        guard(
            &fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab")).unwrap(),
            "03b9924e83bb7a8c650bd58c87bcb3d669dcdf0198bb240e5f964d434d8c62be",
        );
        guard(
            &fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab")).unwrap(),
            "7df258462744a70bce18516dd6d79239cd0fd434c7e100ce4ef8ad399d36c644",
        );
        guard(
            &fs::read(installer.join("Standard/fonts/board/usa/tycoon.off")).unwrap(),
            "0cb07d00caba4717e0ae7ee5a757ce9e2a18eb3b3013d5812e9da485a1090f3a",
        );
        guard(
            &fs::read(installer.join("Standard/fonts/board/usa/tycoon.bin")).unwrap(),
            "42696b634f2b7517a645516317341cf40ed2d25efe6931e033f65c527a3d28e1",
        );
        let exe_path = openmt2001::fixtures::path("EXE");
        let language = openmt2001::fixtures::optional("AUCTION_LANGUAGE").unwrap_or_else(|| {
            exe_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa")
        });
        let off = fs::read(language.join("tycoon.off")).unwrap();
        let bytes = fs::read(language.join("tycoon.bin")).unwrap();
        guard(
            &off,
            "54a2c8adf44e37138bcaf36bf9c9218e0543880e308a0b9cc9a883ee98265b4d",
        );
        guard(
            &bytes,
            "4d5e302035f3579d2c0cb8cd35a58abae5ea6d8159e6c3661bef091fedff6c38",
        );
        let mut at = 0;
        let strings = off
            .chunks_exact(2)
            .map(|n| {
                let end = at + u16::from_le_bytes(n.try_into().unwrap()) as usize;
                let row = &bytes[at..end];
                at = end;
                row
            })
            .collect::<Vec<_>>();
        assert_eq!(at, bytes.len());
        let assets = Assets::load(&index, &archive, installer, &strings, &exe).unwrap();
        assert_eq!(
            assets.bid.base.len(),
            1,
            "one authored border surface; no part0 backing"
        );
        let image = PeImage::parse(&exe).unwrap();
        let body = u32::from_le_bytes(image.at(0x5362c0, 4).unwrap().try_into().unwrap());
        assert_eq!(body, 0xbb000a28);
        assert_eq!(assets.background, body);
        for (panel, name, count, backing, title, parts, visible_parts) in [
            (
                &assets.bid,
                "new_auction_bid_v2.tm",
                9,
                "AUCTION_BID_BACKING",
                "AUCTION_BID_BID_AMOUNT",
                &[1][..],
                1,
            ),
            (
                &assets.timer,
                "new_auction_timer.tm",
                4,
                "AUCTION_TIMER_BACKING",
                "AUCTION_TIMER_TITLE_LABEL",
                &[0][..],
                1,
            ),
            (
                &assets.top,
                "new_auction_top.tm",
                7,
                "AUCTION_TOP_BACKING",
                "AUCTION_TOP_SALE_AREA",
                &[0][..],
                0,
            ),
        ] {
            let name = format!("gamedata2/brand_new_panels/{name}");
            let mesh = CachedMesh::parse(payload(&name)).unwrap();
            let pos = mesh.positions(0).unwrap();
            let regions = openmt2001::ui_mesh::regions(&mesh, 0).unwrap();
            assert_eq!(regions.len(), count);
            assert_eq!(panel.regions, regions);
            let (expected_base, bounds) =
                ui::panel_frame_base(&index, &archive, &name, parts, 0).unwrap();
            assert_eq!(panel.base.len(), visible_parts, "{name} visible parts");
            if visible_parts == 0 {
                // The top panel's selected part0 is a reverse-facing layout
                // quad. Its runtime fill and all regions still survive below.
                let ids = mesh.parts[0].vertex_indices().collect::<Vec<_>>();
                assert!(ids.as_chunks::<3>().0.iter().all(|&t| area(&pos, t) > 0.));
            }
            assert_eq!(bounds.len(), count);
            for (a, b) in panel.base.iter().zip(&expected_base) {
                same(a, b);
            }
            for (i, r) in regions.iter().enumerate() {
                let ids = mesh.parts[r.part].vertex_indices().collect::<Vec<_>>();
                let expected = [
                    ids.iter().map(|&j| pos[j][0]).fold(f32::INFINITY, f32::min),
                    ids.iter()
                        .map(|&j| -pos[j][1])
                        .fold(f32::INFINITY, f32::min),
                    ids.iter()
                        .map(|&j| pos[j][0])
                        .fold(f32::NEG_INFINITY, f32::max),
                    ids.iter()
                        .map(|&j| -pos[j][1])
                        .fold(f32::NEG_INFINITY, f32::max),
                ];
                assert_eq!(r.bounds, expected);
                assert_eq!(bounds[i], expected);
            }
            let bounds = |n| regions.iter().find(|r| r.name == n).unwrap().bounds;
            assert_eq!(panel.backing, bounds(backing));
            assert_eq!(panel.title, bounds(title));
            let mut fill = Surface::solid();
            fill.rect(bounds(backing), [body; 4]);
            fill.rect(
                bounds(title),
                [0xff909090, 0xff909090, 0x7f909090, 0x7f909090],
            );
            same(&panel.fill, &fill);
        }
        let mesh =
            CachedMesh::parse(payload("gamedata2/brand_new_panels/new_auction_bid_v2.tm")).unwrap();
        assert_eq!(
            (mesh.vertex_count, mesh.parts.len(), mesh.frames),
            (60, 9, 10)
        );
        let p = mesh.positions(0).unwrap();
        let ids = mesh.parts[0].vertex_indices().collect::<Vec<_>>();
        assert_eq!(ids.len(), 6);
        assert!(
            ids.chunks_exact(3)
                .all(|t| area(&p, t.try_into().unwrap()) > 0.)
        );
        let material =
            TextureSet::parse(payload(mesh.texture_names[mesh.parts[0].texture as usize])).unwrap();
        assert!(
            material.textures[0]
                .texels
                .chunks_exact(4)
                .all(|p| p == [146, 144, 144, 255])
        );
        // Original parent submits the complete mesh; only part1 survives
        // its CW cull. Its seven strips are not a bounding-box outline.
        let border = &mesh.parts[1];
        assert_eq!((border.first_vertex, border.last_vertex), (4, 31));
        guard(
            border.header,
            "9271fc22bd7aad843492f48a663665d393e12869425206d44376d232def161b0",
        );
        guard(
            border.indices,
            "5149c5e580a38aa158b704739020a83fd14bc012f29c42ea4891041520a531ce",
        );
        assert_eq!(mesh.flags & 0x30, 0);
        guard(
            &mesh.vertex_bytes[4 * 12..32 * 12],
            "ab6712831f264b23d8e175a92e925712c7f41e55b3220b7855f4e0e678faf99d",
        );
        let border_ids = border.vertex_indices().collect::<Vec<_>>();
        assert_eq!(border_ids.len(), 42);
        assert_eq!(border_ids.chunks_exact(3).count(), 14);
        assert!(border_ids.chunks_exact(3).all(|t| {
            let a = area(&p, t.try_into().unwrap());
            a.is_finite() && a < 0.
        }));
        let uv = mesh.uvs();
        let texture =
            TextureSet::parse(payload(mesh.texture_names[border.texture as usize])).unwrap();
        let mut expected_border = Surface::new(&texture.textures[0]);
        for &i in &border_ids {
            expected_border
                .vertices
                .push([p[i][0], -p[i][1], 0.5, 1., 1., 1., uv[i][0], uv[i][1]]);
            expected_border.alpha.push(255);
        }
        let actual = &assets.bid.base[0];
        assert_eq!(actual.vertices.len(), 42);
        same(actual, &expected_border); // Exact original index/UV/material order and vertex alpha.
        // All 42 actual vertices must belong to borderpart1. Since base has
        // exactly this one surface, part0's opaque six-vertex quad is absent.
        assert_ne!(actual.vertices.len(), ids.len());
        for frame in 1..mesh.frames {
            let later = mesh.positions(frame).unwrap();
            for &i in &border_ids {
                assert_eq!(later[i][..2], p[i][..2]);
            }
        }
        let r = &assets.bid.regions;
        assert_eq!(r[0].name, "AUCTION_BID_BACKING");
        for (i, name) in [
            "AUCTION_BID50",
            "AUCTION_BID100",
            "AUCTION_BID500",
            "AUCTION_PULLOUT_BUTTON",
            "AUCTION_PASS_BUTTON",
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                assets.buttons[i],
                r.iter().find(|r| r.name == name).unwrap().bounds
            );
        }
        let title = assets.bid.title;
        assert_eq!(
            assets.hidden_bid_toggle,
            [title[2] - 16., title[1], title[2], title[3]]
        );
        // All regions and live control/fill paths survive face rejection;
        // the reverse-facing top layout quad does not become a mesh backing.
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_AUCTION_BID_FACING_CAPTURE") {
            let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                d.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(d)).unwrap();
            let state = crate::auction_controls::State {
                ticks: 1,
                price_label: Some(1250),
                buttons_enabled: 31,
                timer_visible: true,
                timer_text: Some(crate::auction_controls::TimerText::Seconds(15)),
                ..Default::default()
            };
            let info = Info {
                category: 33,
                category_color: 0xff10b030,
                character: 3,
                actor: b"Fixture\0".to_vec(),
                price: 1250,
            };
            let panel = assets
                .prepare_with_info(
                    &renderer,
                    &state,
                    None,
                    false,
                    Some(&info),
                    &[],
                    toggle_indicator(true, false),
                )
                .unwrap();
            let mut checker = Surface::solid();
            for row in 0..15 {
                for col in 0..20 {
                    let c = if (row + col) % 2 == 0 {
                        0xfff04020
                    } else {
                        0xff1040e0
                    };
                    checker.rect(
                        [
                            col as f32 * 32.,
                            row as f32 * 32.,
                            (col + 1) as f32 * 32.,
                            (row + 1) as f32 * 32.,
                        ],
                        [c; 4],
                    );
                }
            }
            let view = [
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
            let underlay = renderer.prepare(&[checker.draw()], view, None).unwrap();
            crate::capture_test::capture_overlay(&renderer, &underlay, &panel, &path);
        }
    }
}

/// 41426c tests the same network flag as414160's input gate. Offline,
/// details858 selects the checked image; reopening starts with the empty box.
fn toggle_indicator(enabled: bool, selected: bool) -> Option<usize> {
    enabled.then_some(if selected { 276 } else { 275 })
}

#[test]
fn toggle_indicator_tracks_live_state_and_network_suppression() {
    assert_eq!(toggle_indicator(true, false), Some(275));
    assert_eq!(toggle_indicator(true, true), Some(276));
    assert_eq!(toggle_indicator(false, false), None);
    assert_eq!(toggle_indicator(false, true), None);
}
