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
    pub fn from_session(session: &crate::fresh_city::Session<'_>) -> Result<Self, String> {
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
    let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
    let exe = read("OPENMT2001_EXE");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
    let installer = parameters.parent().unwrap().parent().unwrap();
    let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
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
    let hidden_fixture = fs::read("work/frontend/native-auction-hidden-button.bin").unwrap();
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
        assert_eq!(value.to_bits(), word(32 + i * 4), "original hidden hitbox {i}");
    }
    for i in 0..4 { assert_eq!(word(48 + i * 4), u32::MAX); }
    for bounds in assets.buttons {
        assert!(bounds[2] > bounds[0] && bounds[3] > bounds[1]);
    }
    let auction = crate::live_host::Auction { visible: true, hidden_bid_toggle_enabled: true, controls: crate::auction_controls::State {buttons_enabled:31,..Default::default()}, ..Default::default() };
    let mut view = View::default();
    for (index,bounds) in assets.buttons.iter().enumerate() {
        let point = [(bounds[0]+bounds[2])*0.5,(bounds[1]+bounds[3])*0.5];
        assert_eq!(view.tutorial_control_at(&assets,Some(point),&auction),Some([65,66,67,70,69][index]));
        assert_eq!(view.mouse(&assets,&auction,Some(point),true),None);
        assert!(view.pointer_click_cue(&assets,&auction,Some(point)));
        assert!(!view.pointer_click_cue(&assets,&auction,Some(point)));
        assert!(!view.pointer_click_cue(&assets,&auction,None));
        assert!(view.pointer_click_cue(&assets,&auction,Some(point)));
        assert!(view.tutorial_control_pressed());
        assert_eq!(view.mouse(&assets,&auction,Some(point),false),Some(index as u32));
        assert!(!view.pointer_click_cue(&assets,&auction,Some(point)));
        assert!(!view.tutorial_control_pressed());
        view.mouse(&assets,&auction,Some(point),true);
        assert_eq!(view.mouse(&assets,&auction,None,false),None);
        view.mouse(&assets,&auction,Some(point),true);
        view.cancel_press();
        assert_eq!(view.mouse(&assets,&auction,Some(point),false),None);
    }
    let hidden = assets.hidden_bid_toggle;
    let title = assets.bid.title;
    assert_eq!(hidden[0].to_bits(), (title[2] - 16.).to_bits());
    assert_eq!(hidden[1..], title[1..]);
    let point = [(hidden[0]+hidden[2])*0.5,(hidden[1]+hidden[3])*0.5];
    assert_eq!(view.tutorial_control_at(&assets,Some(point),&auction),None);
    assert_eq!(view.mouse(&assets,&auction,Some(point),true),None);
    assert!(view.pointer_click_cue(&assets,&auction,Some(point)));
    assert_eq!(view.mouse(&assets,&auction,Some(point),false),Some(5));
    let network = crate::live_host::Auction { hidden_bid_toggle_enabled: false,
        ..auction.clone() };
    assert_eq!(view.mouse(&assets,&network,Some(point),true),None);
    assert!(!view.pointer_click_cue(&assets,&network,Some(point)));
    assert_eq!(view.mouse(&assets,&network,Some(point),false),None);
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
        let panel = assets
            .prepare_with_info(&renderer, &state, Some(1), false, Some(&info))
            .unwrap();
        let scene =
            crate::auction_scene::Assets::load(&PeImage::parse(&exe).unwrap(), &index, &archive)
                .unwrap()
                .prepare(&renderer, &[0, 2, 8, 9])
                .unwrap();
        crate::capture_test::capture_overlay(&renderer, &scene, &panel, &path);
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
    fonts: [Vec<Surface>; 2],
    tabs: [Vec<u8>; 2],
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
        assets.buttons.iter().enumerate().find_map(|(i, b)| {
            (auction.visible
                && auction.controls.buttons_enabled & (1 << i) != 0
                && x >= b[0]
                && x <= b[2]
                && y >= b[1]
                && y <= b[3])
                .then_some(i)
        }).or_else(|| {
            let b=assets.hidden_bid_toggle;
            (auction.visible && auction.hidden_bid_toggle_enabled
                && x>=b[0] && x<=b[2] && y>=b[1] && y<=b[3])
                .then_some(5)
        })
    }
    ///413de0 binds50/100/500/PullOut/Pass to globals65/66/67/70/69.
    pub fn tutorial_control_at(&self, assets: &Assets, pointer: Option<[f32; 2]>, auction: &crate::live_host::Auction) -> Option<u32> {
        Self::hover(assets, pointer, auction)
            .and_then(|index| [65,66,67,70,69].get(index).copied())
    }
    pub fn tutorial_control_pressed(&self) -> bool { self.pressed.is_some() }
    pub fn cancel_press(&mut self) {
        self.pressed = None;
        self.held = false;
        self.cue_latch = None;
    }
    /// All six bidder children use generic 43ed20/443360. Its +0x330 latch
    /// sounds on first held entry, clears on leave, and stays silent on hold.
    pub fn pointer_click_cue(&mut self, assets: &Assets,
        auction: &crate::live_host::Auction, pointer: Option<[f32; 2]>) -> bool {
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
                    assets.prepare_with_info(renderer, &a.controls, *h, *p, a.info.as_ref())
                })
                .transpose()?;
            self.key = key;
        }
        Ok(())
    }
}
impl Assets {
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
        let panel = |name: &str, backing: &str, title: &str| -> Result<Panel, String> {
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
            let base = ui::panel_frame_base(index, archive, name, &[0], 0)?.0;
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
        )?;
        let hidden_bid_toggle = [bid.title[2]-16.,bid.title[1],bid.title[2],bid.title[3]];
        let timer = panel(
            "gamedata2/brand_new_panels/new_auction_timer.tm",
            "AUCTION_TIMER_BACKING",
            "AUCTION_TIMER_TITLE_LABEL",
        )?;
        let top = panel(
            "gamedata2/brand_new_panels/new_auction_top.tm",
            "AUCTION_TOP_BACKING",
            "AUCTION_TOP_SALE_AREA",
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
            portraits,
            fonts: [
                font.textures[2..4].iter().map(Surface::new).collect(),
                font.textures[6..8].iter().map(Surface::new).collect(),
            ],
            tabs: [title?, body?],
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
        self.prepare_with_info(renderer, state, hover, pressed, None)
    }
    fn prepare_with_info(
        &self,
        renderer: &Renderer,
        state: &crate::auction_controls::State,
        hover: Option<usize>,
        pressed: bool,
        info: Option<&Info>,
    ) -> Result<PreparedScene, String> {
        let mut surfaces = self.bid.base.clone();
        surfaces.push(self.bid.fill.clone());
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
        for i in 0..5 {
            let over = hover == Some(i) && state.buttons_enabled & (1 << i) != 0;
            let mut surface = self.textures[i][if over {
                if pressed {
                    2
                } else {
                    1
                }
            } else {
                0
            }]
            .clone();
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
