//! Original40f260 Locator business list and live report navigation.
use crate::ui::{self, Surface};
#[path = "toolbar_collapse.rs"]
pub mod toolbar_collapse;
#[path="locator/strategic.rs"]
pub(crate) mod strategic;
use openmt2001::{
    archive::ArchiveIndex,
    business_pool::BusinessCatalog,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    render_scene::Scene,
    texture::TextureSet,
};
use std::{fs, path::Path};
#[derive(Clone, Copy, Debug, PartialEq)]
struct OwnedRow {
    label: u32,
    building: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    label: u32,
    kind: u32,
}
#[derive(Clone)]
pub struct RuntimeInputs {
    pub catalog: BusinessCatalog,
    pub commodities: [[u8; 0x1c]; 32],
    pub memberships: [u32; 5],
    pub owned_eligible: [bool; 52],
}
impl RuntimeInputs {
    pub fn from_world(world: &openmt2001::level_startup::World) -> Self {
        Self {
            catalog: world.catalog.info.clone(),
            commodities: world.commodities.rows,
            memberships: world.commodities.memberships,
            owned_eligible: std::array::from_fn(|category| {
                let record = &world.categories[category];
                record[4] == 1
                    && record[0x90] & 1 == 0
                    && i32::from_le_bytes(record[0x8c..0x90].try_into().unwrap()) <= 45
            }),
        }
    }
}
pub struct Assets {
    base: Vec<Surface>,
    property_base: Vec<Surface>,
    property_title_bounds: [f32; 4],
    property_title_text_bounds: [f32; 4],
    property_title_icon: Surface,
    property_title_fonts: Vec<Surface>,
    property_title_tab: Vec<u8>,
    icon: Surface,
    player_icon: Surface,
    city_icon: Surface,
    poll_icon: Surface,
    city_tabs: [Surface; 4],
    locator_tabs: [Surface; 3],
    owned_eligible: [bool; 52],
    utility_tabs: [Surface; 2],
    property_tabs: [Surface; 5],
    property_side_icons: [Surface; 3],
    property_side_bounds: [[f32; 4]; 3],
    toolbar_closed_base: Vec<Surface>,
    toolbar_disabled: Surface,
    offer_buttons: [Surface; 3],
    offer_arrows: [[Surface; 3]; 2],
    period_bounds: [[f32; 4]; 2],
    amount_bounds: [f32; 4],
    amount_arrow_bounds: [[f32; 4]; 2],
    offer_button_bounds: [f32; 4],
    rights_button_bounds: [f32; 4],
    rights_fonts: Vec<Surface>,
    rights_tab: Vec<u8>,
    currency_suffix: bool,
    property_lease_available: [bool; 52],
    player_buttons: [[Surface; 4]; 10],
    city_layout: ui::MppModes,
    people_selector: [f32; 4],
    city_tab_backgrounds: [Surface; 3],
    prestige_star: Surface,
    people_dot: Surface,
    rights_icon: Surface,
    people_palette: [u32; 3],
    text_palette: [u32; 18],
    people_arrows: [[Surface; 3]; 2],
    ownership_icons: [Surface; 10],
    ownership_fonts: Vec<Surface>,
    ownership_tab: Vec<u8>,
    ownership_label_widths: [f32; 52],
    short_labels: [Vec<u8>; 52],
    civic_icons: [Option<(Surface, u32)>; 52],
    ownership_styles: [(u32, Option<usize>); 11],
    star_size: [u32; 2],
    star_bias: f64,
    category_centers: [[f32; 2]; 52],
    icon_backgrounds: [Surface; 3],
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    order: [u32; 45],
    owner_palette: [u32; 10],
    commodity_order: [u32; 32],
    memberships: [u32; 5],
    block_labels: [Vec<u8>; 52],
    title_labels: [Vec<u8>; 52],
    board_currency: Vec<u8>,
    bounds: [f32; 4],
    list: [f32; 4],
    scrollbar: [f32; 4],
    scrollbar_textures: [[Surface; 4]; 2],
}
impl Assets {
    #[cfg(all(test, feature = "capture"))]
    pub fn verify_fresh_property(
        &self,
        report: &crate::block_information::Report,
        scene: &Scene,
        category: usize,
        scenario: &str,
    ) {
        let rail_capture = std::env::var("OPENMT2001_FRESH_RAILROAD_CAPTURE").ok();
        let land_capture = std::env::var("OPENMT2001_FRESH_LAND_RENT_CAPTURE").ok();
        let Some(prefix) = land_capture
            .clone()
            .or_else(|| rail_capture.clone())
            .or_else(|| std::env::var("OPENMT2001_FRESH_PROPERTY_CAPTURE").ok())
        else {
            return;
        };
        assert!(report.initialized && report.available(category));
        assert!(scene.saved_players.is_empty());
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let categories: Vec<_> = if land_capture.is_some() {
            vec![category]
        } else if rail_capture.is_some() {
            assert!(report.railroads.is_some());
            report.railroad_categories.to_vec()
        } else {
            std::iter::once(category)
                .chain(report.utility_categories)
                .collect()
        };
        for (i, category) in categories.into_iter().enumerate() {
            let (panel, _) = self.block_panel(&renderer, report, category).unwrap();
            crate::capture_test::capture(
                &renderer,
                Some(&panel),
                scene,
                &format!("{prefix}-{scenario}-{i}.png"),
            );
            if report.categories[category].is_some()
                || report.utility_categories.contains(&category)
                || report.railroad_categories.contains(&category)
            {
                let mut view = crate::utility_report::View::default();
                view.select(Some(category));
                let [l, t, r, b] = self.city_layout.buttons[1];
                self.utility_pointer(&mut view, Some([(l + r) as i32, (t + b) as i32]));
                assert!(view.mouse(true));
                assert!(view.mouse(false));
                assert!(view.details);
                let (panel, _) = self
                    .property_panel(&renderer, report, category, &view)
                    .unwrap();
                crate::capture_test::capture(
                    &renderer,
                    Some(&panel),
                    scene,
                    &format!("{prefix}-{scenario}-{i}-details.png"),
                );
            }
        }
    }
    #[cfg(test)]
    pub fn verify_fresh_players(
        &self,
        scene: &Scene,
        players: crate::player_info::State,
        scenario: &str,
    ) {
        assert!(scene.saved_players.is_empty());
        assert!(players.fresh);
        let mut state = self.people_state().unwrap();
        state.attach_players(players);
        // Player icon is available independently of saved-only Locator controls.
        assert!(self.contains(&state, [32, 180]));
        assert!(!self.contains(&state, [32, 225]));
        state.mouse(self, scene, Some([32, 180]), true);
        state.mouse(self, scene, Some([32, 180]), false);
        assert!(state.body_open() && state.player_info);
        for (position, player) in state.player_display_order().into_iter().enumerate() {
            let rect = self.city_layout.buttons[position];
            let point = [(rect[0] + rect[2]) as i32, (rect[1] + rect[3]) as i32];
            state.mouse(self, scene, Some(point), true);
            state.mouse(self, scene, Some(point), false);
            assert_eq!(state.player.players[state.player.selected], player);
            assert_eq!(
                state.player.rows,
                crate::player_info::report(&state.player.records[player], true).unwrap()
            );
        }
        state.mouse(self, scene, Some([300, 164]), true);
        state.mouse(self, scene, Some([300, 164]), false);
        assert!(!state.player.today);
        let selected = state.player.players[state.player.selected];
        assert_eq!(
            state.player.rows,
            crate::player_info::report(&state.player.records[selected], false).unwrap()
        );
        #[cfg(feature = "capture")]
        if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_PLAYER_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            for today in [false, true] {
                state.player.today = today;
                state.player.selected = 0;
                state.player.refresh().unwrap();
                let panel = self.prepare(&renderer, &state, scene).unwrap();
                crate::capture_test::capture(
                    &renderer,
                    Some(&panel),
                    scene,
                    &format!("{prefix}-{scenario}-{today}.png"),
                );
            }
        }
        let _ = scenario;
    }
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        exe: &[u8],
    ) -> Result<Self, String> {
        let (mut base, regions) = ui::panel_frame_base(
            index,
            archive,
            "gamedata2/brand_new_panels/main_panel_v2.tm",
            &(0..9).collect::<Vec<_>>(),
            9,
        )?;
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index.find(name).ok_or("missing Locator asset")?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("Locator asset range".into())
        };
        let panel_mesh = openmt2001::mesh::CachedMesh::parse(payload(
            "gamedata2/brand_new_panels/main_panel_v2.tm",
        )?)?;
        let (mut toolbar_closed_base, _) = ui::panel_frame_base(index, archive,
            "gamedata2/brand_new_panels/main_panel_v2.tm", &(0..12).collect::<Vec<_>>(), 0)?;
        let closed_regions = openmt2001::ui_mesh::regions(&panel_mesh, 0)?;
        let strip = closed_regions.iter().find(|r|r.name=="P_MAIN_VERTCAL_BAR_BACKING")
            .ok_or("closed toolbar strip missing")?.bounds;
        let mut strip_backing = Surface::solid();
        strip_backing.rect(strip, [0xff777777,0x66777777,0x66777777,0xff777777]);
        toolbar_closed_base.push(strip_backing);
        let property_title_bounds = openmt2001::ui_mesh::regions(&panel_mesh, 9)?
            .into_iter()
            .find(|r| r.name == "P_MAIN_TITLE_AREA_BACKING")
            .ok_or("property title backing missing")?
            .bounds;
        let title_regions = openmt2001::ui_mesh::regions(&panel_mesh, 9)?;
        let title_area = |name| -> Result<[f32; 4], String> {
            title_regions.iter().find(|r| r.name == name)
                .map(|r| r.bounds).ok_or_else(|| format!("property {name} missing"))
        };
        let mut property_title_text_bounds = title_area("P_MAIN_TITLE_AREA1")?;
        property_title_text_bounds[0] += 10.;
        property_title_text_bounds[2] += 10.;
        property_title_text_bounds[3] = title_area("P_MAIN_TITLE_AREA2")?[3];
        let mut property_base = base.clone();
        property_base.push(ui::mpp_background_title(
            &panel_mesh,
            9,
            &PeImage::parse(exe)?,
            Some([0; 4]),
        )?);
        base.push(ui::mpp_background(&panel_mesh, 9, &PeImage::parse(exe)?)?);
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        // Shared MPP mode1 icon, submitted by original 41edb0 at (35,5).
        let mut property_title_icon = Surface::new(&generic.textures[20]);
        let title_icon_bounds = [35., 5., 35. + property_title_icon.width as f32,
            5. + property_title_icon.height as f32];
        property_title_icon.quad(
            [[title_icon_bounds[0],title_icon_bounds[1]], [title_icon_bounds[0],title_icon_bounds[3]],
             [title_icon_bounds[2],title_icon_bounds[3]], [title_icon_bounds[2],title_icon_bounds[1]]],
            [[0.,0.],[0.,1.],[1.,1.],[1.,0.]], [0xffffffff;4]);
        // Original533764: P_MAIN_ICON4, texture198, tooltip1148(Locator), event22.
        let mut icon = Surface::new(&generic.textures[198]);
        let [l, t, r, b] = regions[16];
        icon.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        let mut player_icon = Surface::new(&generic.textures[68]);
        let [l, t, r, b] = regions[15];
        player_icon.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        // Original533738: P_MAIN_ICON2, texture66, tooltip1146, event20.
        let mut city_icon = Surface::new(&generic.textures[66]);
        let [l, t, r, b] = regions[14];
        city_icon.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        // Original533774: P_MAIN_ICON5, texture26, tooltip12, event23.
        let mut poll_icon = Surface::new(&generic.textures[26]);
        let [l, t, r, b] = regions[17];
        poll_icon.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        let button = &generic.textures[13];
        if (button.width, button.height)
            != (generic.textures[195].width, generic.textures[195].height)
        {
            return Err("MPP button spacing texture differs from button size".into());
        }
        let city_layout = ui::mpp_modes(regions[9], [button.width, button.height])?;
        let people_selector = regions[46];
        let city_tabs = std::array::from_fn(|i| {
            let mut icon = Surface::new(&generic.textures[[290, 75, 21, 70][i]]);
            let [l, t, r, b] = city_layout.icons[i];
            icon.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            icon
        });
        let locator_tabs = [70, 24, 73].map(|i| Surface::new(&generic.textures[i]));
        let mut owned_eligible = [false; 52];
        for block in openmt2001::board::Board::from_v14(&PeImage::parse(exe)?)?.blocks {
            owned_eligible[block.id] = block.flags & 1 == 0 && block.id <= 45;
        }
        let property_tabs = [291, 197, 65, 64, 67].map(|i| Surface::new(&generic.textures[i]));
        let mut property_lease_available = [false; 52];
        for block in openmt2001::board::Board::from_v14(&PeImage::parse(exe)?)?.blocks {
            property_lease_available[block.id] = block.kind != 0;
        }
        let [l, t, r, _] = regions[9];
        let arrow = &generic.textures[97];
        let aw = arrow.width as f32;
        let ah = arrow.height as f32;
        let period_bounds = [
            [l + aw, t + 45., l + 2. * aw, t + 45. + ah],
            [r - 3. * aw, t + 45., r - 2. * aw, t + 45. + ah],
        ];
        //40560c reads the source UI object's originY. This host submits the
        //original frame at origin0, as it does for the named mesh rectangles.
        let amount_bounds = [
            l + 20.,
            100.,
            r - 20.,
            100. + generic.textures[118].height as f32,
        ];
        let sw = generic.textures[118].width as f32;
        let amount_arrow_bounds = [
            [
                amount_bounds[0],
                amount_bounds[1],
                amount_bounds[0] + sw,
                amount_bounds[3],
            ],
            [
                amount_bounds[2] - sw,
                amount_bounds[1],
                amount_bounds[2],
                amount_bounds[3],
            ],
        ];
        let bw = generic.textures[76].width as f32;
        let bx = (((r - l) * 0.5) - (bw as i32 / 2) as f32).trunc() + l;
        let offer_button_bounds = [bx, 117., bx + bw, 117. + generic.textures[76].height as f32];
        let utility_tabs = [291, 301].map(|i| Surface::new(&generic.textures[i]));
        let fonts = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[6..8]
            .iter()
            .map(Surface::new)
            .collect();
        let property_title_fonts = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[2..4]
            .iter()
            .map(Surface::new)
            .collect();
        let property_title_tab = fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let tab = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let mut order = [0; 45];
        for (v, b) in order
            .iter_mut()
            .zip(PeImage::parse(exe)?.at(0x53bdc4, 180)?.chunks_exact(4))
        {
            *v = u32::from_le_bytes(b.try_into().unwrap());
        }
        validate_order(&order)?;
        let image = PeImage::parse(exe)?;
        let category_centers = openmt2001::board::Board::from_v14(&image)?
            .category_geometry()
            .centers;
        let mut block_labels: [Vec<u8>; 52] = std::array::from_fn(|_| Vec::new());
        for block in openmt2001::board::Board::from_v14(&image)?.blocks {
            block_labels[block.id] = strings
                .get(block.text_id as usize)
                .filter(|s| s.iter().any(|&b| b != 0))
                .map(|s| s.to_vec())
                .unwrap_or_else(|| {
                    block
                        .name
                        .replace('_', " ")
                        .bytes()
                        .map(|b| b.saturating_sub(32))
                        .collect()
                });
        }
        let mut owner_palette = [0; 10];
        for (i, color) in owner_palette.iter_mut().enumerate() {
            *color = u32::from_le_bytes(image.at(0x541af0 + i as u32 * 40, 4)?.try_into().unwrap());
        }
        let mut commodity_order = [0; 32];
        for (value, bytes) in commodity_order
            .iter_mut()
            .zip(image.at(0x53c218, 128)?.chunks_exact(4))
        {
            *value = u32::from_le_bytes(bytes.try_into().unwrap());
        }
        let mut commodity = openmt2001::commodity_script::State::from_v14(&image, [0; 5])?;
        let mut runtime = openmt2001::lua4::Runtime::new()?;
        let bindings = openmt2001::commodity_script::Bindings::register(&mut runtime, &image, 100)?;
        let source = fs::read(installer.join("scripts/DEFAULT/commoditysettings.lua"))
            .map_err(|e| e.to_string())?;
        runtime.execute(
            "Locator default commodity grouping",
            &source,
            &mut |id, args| bindings.handle(id, args, &mut commodity),
        )?;
        let memberships = commodity.memberships;
        let bounds = regions[9];
        let (list, scrollbar) = type_list_layout(bounds, generic.textures[105].width as f32);
        let ownership_tab =
            fs::read(installer.join("Standard/fonts/cust640p.tab")).map_err(|e| e.to_string())?;
        let ownership_fonts = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[..2]
            .iter()
            .map(Surface::new)
            .collect();
        let names = crate::tooltip::board_strings(installer)?;
        let mut ownership_label_widths = [0.; 52];
        let mut short_labels: [Vec<u8>; 52] = std::array::from_fn(|_| Vec::new());
        let mut title_labels: [Vec<u8>; 52] = std::array::from_fn(|_| Vec::new());
        let mut civic_icons = std::array::from_fn(|_| None);
        for block in openmt2001::board::Board::from_v14(&image)?.blocks {
            let name = names
                .get((block.text_id - 2242 + 2) as usize)
                .ok_or("strategic short name missing")?;
            block_labels[block.id] = names
                .get((block.text_id - 2242 + 1) as usize)
                .ok_or("block full name missing")?
                .clone();
            short_labels[block.id] = name.clone();
            // 40225c reads the category's base text ID; 402263 adds three
            // for the selected pavement title (MARVIN, not MARVIN GARDENS).
            title_labels[block.id] = names
                .get((block.text_id - 2242 + 4) as usize)
                .ok_or("block uppercase short title missing")?
                .clone();
            civic_icons[block.id] = crate::strategic_labels::icon(&image, block.text_id)?
                .map(|v| (Surface::new(&generic.textures[v.texture]), v.color));
            ownership_label_widths[block.id] = ui::text_width(&ownership_tab, name, 1.)? as f32;
        }
        let ownership_styles: Vec<_> = (0..11)
            .map(|i| crate::ownership_badges::style(&image, if i == 10 { None } else { Some(i) }))
            .collect::<Result<_, _>>()?;
        let ownership_icons = std::array::from_fn(|i| {
            Surface::new(&generic.textures[ownership_styles[i].1.unwrap()])
        });
        let mut player_buttons = Vec::new();
        for character in 0..10 {
            let base =
                u32::from_le_bytes(image.at(0x541b0c + character * 40, 4)?.try_into().unwrap())
                    as usize;
            player_buttons.push(std::array::from_fn(|state| {
                Surface::new(&generic.textures[base + state])
            }));
        }
        Ok(Self {
            player_buttons: player_buttons
                .try_into()
                .map_err(|_| "player button count")?,
            ownership_fonts,
            ownership_tab,
            ownership_label_widths,
            short_labels,
            civic_icons,
            ownership_styles: ownership_styles.try_into().unwrap(),
            ownership_icons,
            base,
            property_base,
            property_title_bounds,
            property_title_text_bounds,
            property_title_icon,
            property_title_fonts,
            property_title_tab,
            icon,
            player_icon,
            block_labels,
            title_labels,
            board_currency: names.get(2).ok_or("board currency missing")?.clone(),
            city_icon,
            poll_icon,
            city_tabs,
            locator_tabs,
            owned_eligible,
            utility_tabs,
            property_tabs,
            property_lease_available,
            property_side_icons: [66, 19, 18].map(|i| Surface::new(&generic.textures[i])),
            property_side_bounds: [regions[14], regions[15], regions[16]],
            toolbar_closed_base,
            // Original mode images[2]=68ae6c => generic17 for blocked(+333).
            toolbar_disabled: Surface::new(&generic.textures[17]),
            offer_buttons: [76, 78, 77].map(|i| Surface::new(&generic.textures[i])),
            offer_arrows: [118usize, 122].map(|first| {
                [first, first + 2, first + 1].map(|i| Surface::new(&generic.textures[i]))
            }),
            period_bounds,
            amount_bounds,
            amount_arrow_bounds,
            offer_button_bounds,
            rights_button_bounds: [
                bx,
                t + 20.,
                bx + bw,
                t + 20. + generic.textures[76].height as f32,
            ],
            rights_fonts: TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
                .map_err(|e| e.to_string())?
                .textures[..2]
                .iter()
                .map(Surface::new)
                .collect(),
            rights_tab: fs::read(installer.join("Standard/fonts/cust640p.tab"))
                .map_err(|e| e.to_string())?,
            currency_suffix: names.get(3).and_then(|s| s.first()).copied() != Some(0x4c),
            city_layout,
            people_selector,
            prestige_star: Surface::new(&generic.textures[109]),
            rights_icon: Surface::new(&generic.textures[91]),
            text_palette: std::array::from_fn(|i| {
                u32::from_le_bytes(
                    image
                        .at(0x536288 + i as u32 * 4, 4)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                )
            }),
            people_palette: std::array::from_fn(|i| {
                u32::from_le_bytes(
                    image
                        .at(0x5336b0 + i as u32 * 4, 4)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                )
            }),
            people_dot: Surface::new(
                &TextureSet::parse(payload("gamedata2/GenericTextures/textures.gtd")?)
                    .map_err(|e| e.to_string())?
                    .textures[14],
            ),
            people_arrows: [97usize, 101].map(|first| {
                [first, first + 2, first + 1].map(|i| Surface::new(&generic.textures[i]))
            }),
            star_size: [generic.textures[109].width, generic.textures[109].height],
            star_bias: f64::from_le_bytes(image.at(0x525050, 8)?.try_into().unwrap()),
            category_centers,
            city_tab_backgrounds: [13, 15, 14].map(|i| Surface::new(&generic.textures[i])),
            // Original40d807..40d859: normal191, pressed192, hover193.
            icon_backgrounds: [191, 192, 193].map(|i| Surface::new(&generic.textures[i])),
            fonts,
            tab,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            order,
            owner_palette,
            commodity_order,
            memberships,
            bounds,
            list,
            scrollbar,
            scrollbar_textures: [105, 93].map(|first| {
                std::array::from_fn(|phase| Surface::new(&generic.textures[first + phase]))
            }),
        })
    }
    /// Original4044b0 fields on the retained original MPP frame. Compact host row placement.
    fn people_arrow_bounds(&self, right: bool) -> [f32; 4] {
        let [l, t, r, _] = self.people_selector;
        let texture = &self.people_arrows[usize::from(right)][0];
        let x = if right { r - texture.width as f32 } else { l };
        [
            x,
            t + 21.,
            x + texture.width as f32,
            t + 21. + texture.height as f32,
        ]
    }
    #[cfg(test)]
    pub fn block_panel(
        &self,
        renderer: &Renderer,
        report: &crate::block_information::Report,
        category: usize,
    ) -> Result<(PreparedScene, [f32; 4]), String> {
        self.property_panel(
            renderer,
            report,
            category,
            &crate::utility_report::View::default(),
        )
    }
    pub fn utility_pointer(
        &self,
        state: &mut crate::utility_report::View,
        point: Option<[i32; 2]>,
    ) {
        state.pointer(point, &self.city_layout.buttons);
    }
    /// Native LandRent widget32 owns its linked-business children; successful
    /// linked navigation emits hyperlink6 rather than the parent group.
    pub fn property_tutorial_control_at(
        &self,
        state: &crate::utility_report::View,
        point: Option<[i32; 2]>,
        report: &crate::block_information::Report,
        category: usize,
    ) -> Option<u32> {
        let [x, y] = point?.map(|v| v as f32 * 0.5);
        if let Some(control) = self.property_control_at(state, [x, y], report, category) {
            // Original 4055f0/4060ea constructs the centered texture76,
            // caption53 rights child at [[main+390]+4]. 41ffea routes that
            // child through 4050d0 and then publishes tutorial event91.
            if control == 102 {
                return Some(91);
            }
            return (control < 100).then_some(control);
        }
        if state.property_offer_open() || state.property_rights_open() {
            return None;
        }
        if let Some(tab) = self
            .city_layout
            .buttons
            .iter()
            .take(self.property_tab_count(report, category))
            .position(|&[l, t, r, b]| x >= l && x <= r && y >= t && y <= b)
        {
            return Some(15 + tab as u32);
        }
        let top = self.bounds[1] + 43.;
        if state.details
            && report.categories[category].is_some()
            && x >= self.bounds[0]
            && x <= self.bounds[2]
            && y >= top
            && y < top + 220.
            && matches!(
                report.land_rent[category].get(state.scroll + ((y - top) / 10.) as usize),
                Some(crate::block_information::land_rent::Row::Business(_))
            )
        {
            return Some(32);
        }
        None
    }
    fn property_control_at(
        &self,
        state: &crate::utility_report::View,
        point: [f32; 2],
        report: &crate::block_information::Report,
        category: usize,
    ) -> Option<u32> {
        let hit = |[l, t, r, b]: [f32; 4]| {
            point[0] >= l && point[0] <= r && point[1] >= t && point[1] <= b
        };
        for (index, bounds) in self.property_side_bounds.iter().enumerate() {
            if hit(*bounds)
                && (index == 0 || self.property_lease_available[category])
                && (index != 2 || self.property_tab_count(report, category) == 5)
            {
                return Some(9 + index as u32);
            }
        }
        if state.property_rights_open() {
            return state
                .property_rights_state()
                .filter(|i| i.activate_visible)
                .and_then(|_| hit(self.rights_button_bounds).then_some(102));
        }
        if state.property_offer_open() {
            let panel = &state.property_panel_state()?.panel;
            if panel.visible != 1 || !report.lease_available[category] {
                return None;
            }
            if panel.action_visible != 0 && hit(self.offer_button_bounds) {
                return Some(35);
            }
            if panel.amount_visible != 0 {
                return self
                    .amount_arrow_bounds
                    .iter()
                    .position(|&b| hit(b))
                    .map(|i| 100 + i as u32);
            }
        } else if matches!(state.property_tab, 2 | 3)
            && report.businesses.as_ref().is_some_and(|b| {
                !matches!(
                    b.rows(category, state.property_tab, false),
                    [crate::block_information::business::Row::Empty]
                )
            })
        {
            return self
                .period_bounds
                .iter()
                .position(|&b| hit(b))
                .map(|i| 33 + i as u32);
        }
        None
    }
    /// One property pointer route serves production and original tutorial tests.
    pub fn property_pointer(
        &self,
        state: &mut crate::utility_report::View,
        point: Option<[i32; 2]>,
        report: &crate::block_information::Report,
        category: usize,
    ) {
        state.property_tab_count = self.property_tab_count(report, category);
        state.pointer(
            if state.property_offer_open() || state.property_rights_open() {
                None
            } else {
                point
            },
            &self.city_layout.buttons,
        );
        let control = point
            .map(|p| p.map(|v| v as f32 * 0.5))
            .and_then(|p| self.property_control_at(state, p, report, category));
        let previous = if state.property_tab == 2 {
            state.previous_profit
        } else {
            state.previous_efficiency
        };
        let hover = point.and_then(|p| {
            if state.property_offer_open() || state.property_rights_open() || control.is_some() {
                return None;
            }
            let [x, y] = p.map(|v| v as f32 * 0.5);
            let top = self.bounds[1]
                + if matches!(state.property_tab, 2 | 3) {
                    63.
                } else {
                    43.
                };
            if x < self.bounds[0] || x > self.bounds[2] || y < top || y >= self.bounds[3] - 5. {
                return None;
            }
            let row = state.scroll + ((y - top) / 10.) as usize;
            if state.property_tab == 1 && report.categories[category].is_some() {
                if let Some(crate::block_information::land_rent::Row::Business(b)) =
                    report.land_rent[category].get(row)
                {
                    return Some(b.slot);
                }
            } else if let Some(rows) = report
                .businesses
                .as_ref()
                .map(|b| b.rows(category, state.property_tab, previous))
                && let Some(crate::block_information::business::Row::Business { slot, .. }) =
                    rows.get(row)
            {
                return Some(*slot);
            }
            None
        });
        state.dirty |= state.row_hover != hover;
        state.row_hover = hover;
        state.property_control_pointer(control);
    }
    fn property_tab_count(
        &self,
        report: &crate::block_information::Report,
        category: usize,
    ) -> usize {
        if !self.property_lease_available[category] {
            1
        } else if report.categories[category].is_some() {
            5
        } else {
            2
        }
    }
    pub fn property_panel(&self, renderer: &Renderer, report: &crate::block_information::Report,
        category: usize, state: &crate::utility_report::View,
    ) -> Result<(PreparedScene,[f32;4]),String> {
        self.property_panel_content(renderer,report,category,state,true)
    }
    /// Only ordinary selected-block mode opts into independent side presentation.
    pub fn ordinary_property_body(&self, renderer:&Renderer, report:&crate::block_information::Report,
        category:usize,state:&crate::utility_report::View,
    )->Result<(PreparedScene,[f32;4]),String> {
        if !report.categories.get(category).is_some_and(Option::is_some) {
            return Err("ordinary toolbar body requires ordinary category".into());
        }
        self.property_panel_content(renderer,report,category,state,false)
    }
    fn property_panel_content(
        &self, renderer:&Renderer, report:&crate::block_information::Report,
        category:usize,state:&crate::utility_report::View,include_sides:bool,
    )->Result<(PreparedScene,[f32;4]),String> {
        if let Some(utility) = report
            .utility_categories
            .iter()
            .position(|&id| id == category)
        {
            return self.income_panel(renderer, report, utility, state, false);
        }
        if report.railroads.is_some()
            && let Some(station) = report
                .railroad_categories
                .iter()
                .position(|&id| id == category)
        {
            return self.income_panel(renderer, report, station, state, true);
        }
        let rows = report
            .categories
            .get(category)
            .and_then(Option::as_ref)
            .ok_or("ordinary block report missing")?;
        let mut surfaces = self.property_background(report.title_colors[category]);
        let mut fonts = self.fonts.clone();
        let mut title_font = self.property_title_fonts.clone();
        let mut body = self.ownership_fonts.clone();
        if include_sides { self.property_side_buttons(&mut surfaces, state, report, category); }
        if !state.property_offer_open() && !state.property_rights_open() {
            self.property_buttons(
                &mut surfaces,
                &self.property_tabs[..self.property_tab_count(report, category)],
                state,
            );
        }
        //405744..4057b3 uses font3 for both columns, palette0 for labels
        //and palette3 for right-aligned values. The list starts40 below the
        //main window top, with the text object's original10-pixel line step.
        ui::text_widget_style4(&mut title_font, &self.property_title_tab,
            &self.title_labels[category], self.property_title_text_bounds,
            self.text_palette[0])?;
        if !state.property_offer_open() && !state.property_rights_open() {
            let mut section = Surface::solid();
            section.rect(self.city_layout.header, [self.text_palette[17]; 4]);
            surfaces.push(section);
            ui::text_fit(
                &mut fonts,
                &self.tab,
                &self.strings[match state.property_tab {
                    1 => 1402,
                    2 => 1403,
                    3 => 1404,
                    4 => 1405,
                    _ => 28,
                }],
                [
                    32.,
                    self.city_layout.header[1] + 1.,
                    184.,
                    self.city_layout.header[3],
                ],
                false,
                self.text_palette[16],
            )?;
        }
        if state.property_rights_open() {
            self.rights_rows(&mut surfaces, &mut body, state)?;
        } else if state.property_offer_open() {
            self.offer_rows(
                &mut surfaces,
                &mut fonts,
                &mut body,
                report,
                category,
                state,
            )?;
        } else if state.property_tab == 1 {
            self.land_rent_rows(&mut surfaces, &mut body, report, category, state)?;
        } else if matches!(state.property_tab, 2..=4) {
            self.business_rows(&mut surfaces, &mut body, report, category, state)?;
        } else {
            for (i, row) in rows.iter().enumerate() {
                let y = self.bounds[1] + 40. + i as f32 * 10.;
                ui::text_fit(
                    &mut body,
                    &self.ownership_tab,
                    &self.strings[row.heading as usize],
                    [32., y, 129., y + 10.],
                    false,
                    self.text_palette[0],
                )?;
                if let Some(value) = &row.value {
                    let text = match value {
                        crate::block_information::Value::Label(n) => {
                            self.strings[*n as usize].clone()
                        }
                        crate::block_information::Value::Number(n) => {
                            n.to_string().bytes().map(|b| b - 32).collect()
                        }
                        crate::block_information::Value::Owner(n) => report
                            .names
                            .get(*n as usize - 1)
                            .ok_or("block landlord missing")?
                            .clone(),
                    };
                    ui::text_fit(
                        &mut body,
                        &self.ownership_tab,
                        &text,
                        [130., y, 184., y + 10.],
                        true,
                        self.text_palette[3],
                    )?;
                }
            }
        }
        surfaces.extend(fonts);
        surfaces.extend(title_font);
        surfaces.extend(body);
        let draws = surfaces.iter().map(Surface::draw).collect::<Vec<_>>();
        Ok((
            renderer.prepare(
                &draws,
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
            )?,
            [0., 0., 196., 320.],
        ))
    }
    pub fn land_rent_pointer(
        &self,
        state: &mut crate::utility_report::View,
        point: Option<[i32; 2]>,
        report: &crate::block_information::Report,
        category: usize,
    ) {
        self.property_pointer(state, point, report, category);
    }
    fn land_rent_rows(
        &self,
        surfaces: &mut Vec<Surface>,
        fonts: &mut [Surface],
        report: &crate::block_information::Report,
        category: usize,
        state: &crate::utility_report::View,
    ) -> Result<(), String> {
        use crate::block_information::land_rent::Row;
        let layout = crate::utility_report::Layout {
            bounds: [
                self.bounds[0],
                self.bounds[1] + 40.,
                self.bounds[2] - 2.,
                self.bounds[3],
            ],
        };
        let encoded = |s: &str| s.bytes().map(|b| b - 32).collect::<Vec<_>>();
        let money = |amount: f32| -> Result<Vec<u8>, String> {
            let mut b = self.board_currency.clone();
            b.extend(encoded(&ui::decimal2(amount)?));
            Ok(b)
        };
        for (i, row) in report.land_rent[category]
            .iter()
            .skip(state.scroll)
            .take(22)
            .enumerate()
        {
            let mut heading = false;
            let mut rule = false;
            let (label, value) = match row {
                Row::Blank => continue,
                Row::Heading { owner, landlord } => {
                    heading = true;
                    let name = if *owner == 0 {
                        self.strings[1116].clone()
                    } else {
                        report.names[usize::from(*owner) - 1].clone()
                    };
                    let text = if *landlord {
                        let mut template =
                            self.strings[if *owner == 0 { 1389 } else { 1388 }].clone();
                        let token = encoded(if *owner == 0 { "%i" } else { "%s" });
                        if let Some(at) = template.windows(2).position(|w| w == token) {
                            template.splice(at..at + 2, name);
                        }
                        template
                    } else {
                        name
                    };
                    (text, Vec::new())
                }
                Row::Amount {
                    label,
                    value,
                    rule: r,
                    heading: h,
                } => {
                    rule = *r;
                    heading = *h;
                    (self.strings[*label as usize].clone(), money(*value)?)
                }
                Row::Business(b) => (
                    self.strings[b.label as usize].clone(),
                    ui::integer_money(
                        i32::from(b.rent),
                        &self.board_currency,
                        self.currency_suffix,
                    ),
                ),
            };
            let mut decoration = Surface::solid();
            if heading {
                decoration.rect(layout.heading(i), [0xff01762f; 4]);
            }
            if rule {
                let [l, t, r, _] = layout.line(i, false);
                decoration.rect([l, t - 1., r, t], [0xffe55f49; 4]);
            }
            surfaces.push(decoration);
            let mut left = layout.line(i, false);
            let right = layout.line(i, true);
            if !value.is_empty() {
                let width = ui::text_width(&self.ownership_tab, &value, 1.)? as f32;
                left[2] = (right[2] - width - 3.).max(left[0]);
            }
            ui::text_fit(
                fonts,
                &self.ownership_tab,
                &label,
                left,
                false,
                self.text_palette[if heading {
                    16
                } else if matches!(row, Row::Business(_)) {
                    9
                } else {
                    0
                }],
            )?;
            if !value.is_empty() {
                ui::text_fit(
                    fonts,
                    &self.ownership_tab,
                    &value,
                    right,
                    true,
                    self.text_palette[3],
                )?;
            }
        }
        Ok(())
    }
    fn property_buttons(
        &self,
        surfaces: &mut Vec<Surface>,
        icons: &[Surface],
        state: &crate::utility_report::View,
    ) {
        for (i, icon) in icons.iter().enumerate() {
            let selected = state.property_tab == i;
            let mut background =
                self.city_tab_backgrounds[if selected || state.pressed == Some(i) {
                    1
                } else if state.hover == Some(i) {
                    2
                } else {
                    0
                }]
                .clone();
            let [l, t, r, b] = self.city_layout.buttons[i];
            background.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(background);
            let mut icon = icon.clone();
            let [l, t, r, b] =
                ui::mpp_icon(self.city_layout.buttons[i]).expect("validated property tab bounds");
            icon.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(icon);
        }
    }
    fn control_surface(
        &self,
        surfaces: &mut Vec<Surface>,
        source: &[Surface; 3],
        state: &crate::utility_report::View,
        control: u32,
        bounds: [f32; 4],
        selected: bool,
    ) {
        let index = if selected || state.property_control_selected(control) {
            1
        } else if state.property_control_hover == Some(control) {
            2
        } else {
            0
        };
        let mut surface = source[index].clone();
        let [l, t, r, b] = bounds;
        surface.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        surfaces.push(surface);
    }
    pub fn ordinary_toolbar_bounds(&self)->[[f32;4];3] { self.property_side_bounds }
    pub fn ordinary_toolbar_policy(&self,report:&crate::block_information::Report,
        category:usize,shown:bool,build_allowed:bool,tutorial_allowed:[bool;3],
    )->toolbar_collapse::Policy {
        toolbar_collapse::Policy { category,shown,
            nonzero_kind:self.property_lease_available[category],
            lease_available:report.lease_available[category],build_allowed,tutorial_allowed }
    }
    /// Remove only side-control admission. All existing BODY branches remain.
    pub fn ordinary_body_pointer(&self,state:&mut crate::utility_report::View,
        point:Option<[i32;2]>,report:&crate::block_information::Report,category:usize,
    ) {
        self.property_pointer(state,point,report,category);
        if state.property_control_hover.is_some_and(|id|(9..=11).contains(&id)) {
            state.property_control_pointer(None);
        }
    }
    pub fn ordinary_body_tutorial_at(&self,state:&crate::utility_report::View,
        point:Option<[i32;2]>,report:&crate::block_information::Report,category:usize,
    )->Option<u32> {
        self.property_tutorial_control_at(state,point,report,category)
            .filter(|id|!(9..=11).contains(id))
    }
    fn toolbar_surfaces(&self,policy:toolbar_collapse::Policy,input:&toolbar_collapse::Input,
        state:&crate::utility_report::View,body_open:bool,
    )->Vec<Surface> {
        use toolbar_collapse::Action;
        if !policy.shown { return Vec::new(); }
        let mut surfaces=if body_open {Vec::new()} else {self.toolbar_closed_base.clone()};
        for action in Action::ALL {
            if !policy.visible(action) {continue;}
            let i=action.index();
            let selected=body_open && match action {
                Action::Info=>!state.property_offer_open()&&!state.property_rights_open(),
                Action::Lease=>state.property_offer_open(),Action::Build=>false,
            };
            let enabled=policy.enabled(action);
            let mut backing=if !enabled {self.toolbar_disabled.clone()} else {
                self.icon_backgrounds[if selected||input.pressed(policy.category,action){1}
                    else if input.hover==Some(action){2}else{0}].clone()
            };
            let [l,t,r,b]=self.property_side_bounds[i];
            backing.quad([[l,t],[l,b],[r,b],[r,t]],[[0.,0.],[0.,1.],[1.,1.],[1.,0.]],[0xffffffff;4]);
            surfaces.push(backing);
            let mut icon=self.property_side_icons[i].clone();
            icon.quad([[l,t],[l,b],[r,b],[r,t]],[[0.,0.],[0.,1.],[1.,1.],[1.,0.]],[0xffffffff;4]);
            surfaces.push(icon);
        }
        surfaces
    }
    pub fn ordinary_toolbar_scene(&self,renderer:&Renderer,policy:toolbar_collapse::Policy,
        input:&toolbar_collapse::Input,state:&crate::utility_report::View,body_open:bool,
    )->Result<PreparedScene,String> {
        let surfaces=self.toolbar_surfaces(policy,input,state,body_open);
        renderer.prepare(&surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            [2./640.,0.,0.,0.,0.,-2./480.,0.,0.,0.,0.,1.,0.,-1.,1.,0.,1.],None)
    }
    fn property_side_buttons(
        &self,
        surfaces: &mut Vec<Surface>,
        state: &crate::utility_report::View,
        report: &crate::block_information::Report,
        category: usize,
    ) {
        for index in 0..3 {
            if index != 0 && !self.property_lease_available[category] {
                continue;
            }
            if index == 2 && self.property_tab_count(report, category) != 5 {
                continue;
            }
            let selected = match index {
                0 => !state.property_offer_open() && !state.property_rights_open(),
                1 => state.property_offer_open(),
                _ => state.property_rights_open(),
            };
            self.control_surface(
                surfaces,
                &self.icon_backgrounds,
                state,
                9 + index as u32,
                self.property_side_bounds[index],
                selected,
            );
            let mut icon = self.property_side_icons[index].clone();
            let [l, t, r, b] = self.property_side_bounds[index];
            icon.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(icon);
        }
    }
    fn money(&self, amount: f32) -> Result<Vec<u8>, String> {
        let digits = ui::decimal2(amount)?;
        let mut result = Vec::new();
        if amount < 0. {
            result.push(b'-' - 32);
        }
        if !self.currency_suffix {
            result.extend(&self.board_currency);
        }
        result.extend(digits.trim_start_matches('-').bytes().map(|b| b - 32));
        if self.currency_suffix {
            result.extend(&self.board_currency);
        }
        Ok(result)
    }
    fn offer_rows(
        &self,
        surfaces: &mut Vec<Surface>,
        fonts: &mut [Surface],
        body: &mut [Surface],
        report: &crate::block_information::Report,
        category: usize,
        state: &crate::utility_report::View,
    ) -> Result<(), String> {
        let Some(inspection) = state.property_panel_state() else {
            return Ok(());
        };
        let panel = &inspection.panel;
        if !report.lease_available[category] {
            ui::text_fit(
                body,
                &self.ownership_tab,
                &self.strings[58],
                [self.bounds[0] + 4., 40., self.bounds[2] - 4., 60.],
                false,
                self.text_palette[0],
            )?;
            return Ok(());
        }
        let owner = report.owners[category];
        let owner_name = if owner == 0 {
            self.strings[1116].clone()
        } else {
            report.names[usize::from(owner) - 1].clone()
        };
        for (row, label, value) in [
            (0, 54, Some(owner_name)),
            (1, 55, Some(self.money(panel.minimum)?)),
            (2, if inspection.owned { 57 } else { 56 }, None),
        ] {
            let top = 40. + row as f32 * 12.;
            ui::text_fit(
                body,
                &self.ownership_tab,
                &self.strings[label],
                [self.bounds[0] + 4., top, self.bounds[2] - 4., top + 20.],
                false,
                self.text_palette[0],
            )?;
            if let Some(value) = value {
                ui::text_fit(
                    body,
                    &self.ownership_tab,
                    &value,
                    [self.bounds[0] + 4., top, self.bounds[2] - 4., top + 20.],
                    true,
                    self.text_palette[3],
                )?;
            }
        }
        if panel.visible == 1 && panel.action_visible != 0 {
            let [l, t, r, b] = self.amount_bounds;
            let mut backing = Surface::solid();
            backing.rect([l + 1., t + 3., r - 1., t + 17.], [0xcc183088; 4]);
            surfaces.push(backing);
            let value = self.money(panel.amount)?;
            let width = ui::text_width(&self.ownership_tab, &value, 1.)? as f32;
            ui::text_fit(
                body,
                &self.ownership_tab,
                &value,
                [
                    (l + r - width) * 0.5,
                    t,
                    l + r - (l + r - width) * 0.5,
                    b + 4.,
                ],
                false,
                self.text_palette[0],
            )?;
            if panel.amount_visible != 0 {
                for i in 0..2 {
                    self.control_surface(
                        surfaces,
                        &self.offer_arrows[i],
                        state,
                        100 + i as u32,
                        self.amount_arrow_bounds[i],
                        false,
                    );
                }
            }
            self.control_surface(
                surfaces,
                &self.offer_buttons,
                state,
                35,
                self.offer_button_bounds,
                false,
            );
            let bounds = self.offer_button_bounds;
            let value = &self.strings[if inspection.owned { 48 } else { 47 }];
            let width = ui::text_width(&self.tab, value, 1.)? as f32;
            ui::text_fit(
                fonts,
                &self.tab,
                value,
                [
                    (bounds[0] + bounds[2] - width) * 0.5,
                    bounds[1] + 5.,
                    bounds[2],
                    bounds[3],
                ],
                false,
                self.text_palette[4],
            )?;
        }
        Ok(())
    }
    fn rights_rows(
        &self,
        surfaces: &mut Vec<Surface>,
        fonts: &mut [Surface],
        state: &crate::utility_report::View,
    ) -> Result<(), String> {
        let Some(inspection) = state.property_rights_state() else {
            return Ok(());
        };
        if let Some(message) = inspection.message {
            let l = self.bounds[0] + 5.;
            let r = self.bounds[2] - 5.;
            let top = self.bounds[1] + 80.;
            let (offsets, text) = crate::prompt::wrap_with_gap(
                &self.ownership_tab,
                &self.strings[message as usize],
                (r - l) as i32,
                -1,
            )?;
            for (line, range) in offsets.windows(2).enumerate() {
                let end = range[1].min(text.len());
                let value = &text[range[0]..end];
                let value = value.strip_suffix(&[0]).unwrap_or(value);
                let y = top + line as f32 * 10.;
                if y >= top + 100. {
                    break;
                }
                ui::text_fit(
                    fonts,
                    &self.ownership_tab,
                    value,
                    [l, y, r, y + 10.],
                    false,
                    self.text_palette[0],
                )?;
            }
        }
        if inspection.activate_visible {
            self.control_surface(
                surfaces,
                &self.offer_buttons,
                state,
                102,
                self.rights_button_bounds,
                false,
            );
            let mut caption = self.rights_fonts.clone();
            paint_rights_caption(
                &mut caption,
                &self.rights_tab,
                &self.strings[53],
                self.rights_button_bounds,
                self.text_palette[4],
            )?;
            surfaces.extend(caption);
        }
        Ok(())
    }
    fn business_rows(
        &self,
        surfaces: &mut Vec<Surface>,
        fonts: &mut [Surface],
        report: &crate::block_information::Report,
        category: usize,
        state: &crate::utility_report::View,
    ) -> Result<(), String> {
        use crate::block_information::business::{Row, Value};
        let Some(businesses) = &report.businesses else {
            return Ok(());
        };
        let previous = if state.property_tab == 2 {
            state.previous_profit
        } else {
            state.previous_efficiency
        };
        let rows = businesses.rows(category, state.property_tab, previous);
        let period = matches!(state.property_tab, 2 | 3) && !matches!(rows, [Row::Empty]);
        if period {
            for i in 0..2 {
                self.control_surface(
                    surfaces,
                    &self.people_arrows[i],
                    state,
                    33 + i as u32,
                    self.period_bounds[i],
                    false,
                );
            }
            let top = self.bounds[1] + 45.;
            let value = &self.strings[if previous { 34 } else { 33 }];
            let width = ui::text_width(&self.ownership_tab, value, 1.)? as f32;
            ui::text_fit(
                fonts,
                &self.ownership_tab,
                value,
                [
                    (self.bounds[0] + self.bounds[2] - width) * 0.5,
                    top,
                    self.period_bounds[1][0],
                    top + 16.,
                ],
                false,
                self.text_palette[0],
            )?;
        }
        let layout = crate::utility_report::Layout {
            bounds: [
                self.bounds[0],
                self.bounds[1] + if period { 60. } else { 40. },
                self.bounds[2] - 2.,
                self.bounds[3],
            ],
        };
        let count = ((layout.bounds[3] - layout.bounds[1] - 5.) / 10.).max(0.) as usize;
        for (index, row) in rows.iter().skip(state.scroll).take(count).enumerate() {
            let mut heading = false;
            let mut rule = false;
            let mut left_color = self.text_palette[0];
            let mut right_color = self.text_palette[3];
            let (label, value) = match row {
                Row::Blank => continue,
                Row::Empty => (self.strings[1137].clone(), Vec::new()),
                Row::Heading(owner) => {
                    heading = true;
                    (
                        if *owner == 0 {
                            self.strings[1116].clone()
                        } else {
                            report.names[usize::from(*owner) - 1].clone()
                        },
                        Vec::new(),
                    )
                }
                Row::Total { owner, amount } => {
                    rule = true;
                    heading = *owner == 0;
                    if *amount < 0. {
                        left_color = self.text_palette[10];
                        right_color = self.text_palette[if *owner == 0 { 0 } else { 2 }];
                    }
                    (self.strings[66].clone(), self.money(*amount)?)
                }
                Row::Business {
                    label,
                    value,
                    negative,
                    accent,
                    ..
                } => {
                    left_color = self.text_palette[if *negative { 10 } else { 9 }];
                    right_color = self.text_palette[if *negative {
                        if *accent { 2 } else { 0 }
                    } else if *accent {
                        3
                    } else {
                        0
                    }];
                    let value = match value {
                        Value::Integer(v) => {
                            ui::integer_money(*v, &self.board_currency, self.currency_suffix)
                        }
                        Value::Money(v) => self.money(*v)?,
                        Value::Ratio(a, b) => format!("{a}/{b}").bytes().map(|v| v - 32).collect(),
                        Value::Label(id) => self.strings[*id as usize].clone(),
                    };
                    (self.strings[*label as usize].clone(), value)
                }
            };
            let mut decoration = Surface::solid();
            if heading {
                decoration.rect(layout.heading(index), [self.text_palette[17]; 4]);
                if left_color != self.text_palette[10] {
                    left_color = self.text_palette[16];
                }
            }
            if rule {
                let [l, t, r, _] = layout.line(index, false);
                decoration.rect([l, t - 1., r, t], [0xffe55f49; 4]);
            }
            surfaces.push(decoration);
            let mut left = layout.line(index, false);
            let right = layout.line(index, true);
            if !value.is_empty() {
                left[2] = (right[2] - ui::text_width(&self.ownership_tab, &value, 1.)? as f32 - 3.)
                    .max(left[0]);
            }
            ui::text_fit(fonts, &self.ownership_tab, &label, left, false, left_color)?;
            if !value.is_empty() {
                ui::text_fit(fonts, &self.ownership_tab, &value, right, true, right_color)?;
            }
        }
        Ok(())
    }
    fn property_background(&self, color: u32) -> Vec<Surface> {
        let mut surfaces = self.property_base.clone();
        let faded = (color & 0x00ff_ffff) | (((color >> 24) / 3) << 24);
        let mut title = Surface::solid();
        title.rect(self.property_title_bounds, [color, color, faded, faded]);
        surfaces.push(title);
        surfaces.push(self.property_title_icon.clone());
        surfaces
    }
    fn income_panel(
        &self,
        renderer: &Renderer,
        report: &crate::block_information::Report,
        utility: usize,
        state: &crate::utility_report::View,
        railroad: bool,
    ) -> Result<(PreparedScene, [f32; 4]), String> {
        let categories = if railroad {
            report.railroad_categories
        } else {
            report.utility_categories
        };
        let category = categories[utility];
        let mut surfaces = self.property_background(report.title_colors[category]);
        let mut fonts = self.fonts.clone();
        let mut title_font = self.property_title_fonts.clone();
        let mut body = self.ownership_fonts.clone();
        self.property_side_buttons(&mut surfaces, state, report, category);
        if !state.property_offer_open() && !state.property_rights_open() {
            self.property_buttons(&mut surfaces, &self.utility_tabs, state);
        }
        ui::text_widget_style4(&mut title_font, &self.property_title_tab,
            &self.title_labels[category], self.property_title_text_bounds,
            self.text_palette[0])?;
        if state.property_offer_open() {
            self.offer_rows(
                &mut surfaces,
                &mut fonts,
                &mut body,
                report,
                category,
                state,
            )?;
        } else {
            let layout = crate::utility_report::Layout::new(self.bounds);
            let mut heading_background = Surface::solid();
            heading_background.rect(self.city_layout.header, [self.text_palette[17]; 4]);
            ui::text_fit(
                &mut fonts,
                &self.tab,
                &self.strings[state.title((!railroad).then_some(utility))],
                [
                    32.,
                    self.city_layout.header[1] + 1.,
                    184.,
                    self.city_layout.header[3],
                ],
                false,
                self.text_palette[16],
            )?;
            let owner_name = |owner: u8| -> Result<Vec<u8>, String> {
                if owner == 0 {
                    Ok(self.strings[1116].clone())
                } else {
                    report
                        .names
                        .get(owner as usize - 1)
                        .cloned()
                        .ok_or("utility landlord missing".into())
                }
            };
            let mut rows = vec![
                (
                    self.strings[27].clone(),
                    owner_name(report.owners[category])?,
                    0usize,
                ),
                (
                    self.strings[1393].clone(),
                    report.leases[category]
                        .to_string()
                        .bytes()
                        .map(|b| b - 32)
                        .collect(),
                    1usize,
                ),
            ];
            let replace = |source: &[u8], token: &[u8], value: &[u8]| {
                let mut out = source.to_vec();
                if let Some(i) = out.windows(token.len()).position(|w| w == token) {
                    out.splice(i..i + token.len(), value.iter().copied());
                }
                out
            };
            let encoded = |s: &str| s.bytes().map(|b| b - 32).collect::<Vec<_>>();
            let money = |amount: f32| self.money(amount);
            let mut headers = Vec::new();
            if state.details && railroad {
                rows.clear();
                let amounts = report
                    .railroads
                    .as_ref()
                    .ok_or("railroad income missing")?
                    .amounts(utility, true)?;
                let description = replace(
                    &self.strings[1814],
                    &encoded("%i"),
                    &self.short_labels[category],
                );
                let [l, _, r, _] = layout.line(0, false);
                let (offsets, copy) =
                    crate::prompt::wrap_with_gap(&self.tab, &description, (r - l) as i32, -1)?;
                for (line, pair) in offsets.windows(2).enumerate() {
                    let end = (pair[1] - 1).min(copy.len());
                    ui::text(
                        &mut fonts,
                        &self.tab,
                        &copy[pair[0].min(end)..end],
                        layout.line(line, false),
                        false,
                        self.text_palette[0],
                    )?;
                }
                let start = offsets.len();
                for (i, id) in [1398, 1397, 1476].into_iter().enumerate() {
                    rows.push((self.strings[id].clone(), money(amounts[i])?, start + i));
                }
                let [l, y, r, _] = layout.line(start + 2, false);
                heading_background.line([l, y + 2.], [r, y + 2.], 0xffe55f49);
            } else if state.details {
                rows.clear();
                let owner = usize::from(report.owners[category]);
                let amounts = report.utilities.amounts(utility, true, owner)?;
                let mut row = 0;
                for player in (1..=report.names.len()).filter(|&player| player != owner) {
                    rows.push((owner_name(player as u8)?, money(amounts[row])?, row));
                    row += 1;
                }
                if owner != 0 {
                    rows.push((owner_name(0)?, money(amounts[row])?, row));
                    row += 1;
                }
                //403f91..4040c8 retains source roster order, excludes landlord,
                //then shows the total paid by others and the landlord's own saving.
                let [l, y, r, _] = layout.line(row, false);
                heading_background.line([l, y + 2.], [r, y + 2.], 0xffe55f49);
                rows.push((self.strings[1476].clone(), money(amounts[row])?, row));
                headers.push((row + 2, 1816));
                rows.push((owner_name(owner as u8)?, money(amounts[row + 1])?, row + 3));
            } else {
                //403b76..403ea4: landlord, lease, blank, revenue heading,
                //wrapped explanation, blank, four projections, blank, owners heading.
                headers.push((3usize, if railroad { 1810 } else { 1803 }));
                let projection_row = if railroad {
                    4
                } else {
                    let description = replace(
                        &self.strings[1804],
                        &encoded("%i"),
                        &self.short_labels[category],
                    );
                    let [left, _, right, _] = layout.line(4, false);
                    let (offsets, copy) = crate::prompt::wrap_with_gap(
                        &self.tab,
                        &description,
                        (right - left) as i32,
                        -1,
                    )?;
                    for (line, pair) in offsets.windows(2).enumerate() {
                        let end = (pair[1] - 1).min(copy.len());
                        ui::text(
                            &mut fonts,
                            &self.tab,
                            &copy[pair[0].min(end)..end],
                            layout.line(4 + line, false),
                            false,
                            self.text_palette[0],
                        )?;
                    }
                    4 + offsets.len() - 1 + 1
                };
                let projections = if railroad {
                    report
                        .railroads
                        .as_ref()
                        .ok_or("railroad income missing")?
                        .amounts(utility, false)?
                } else {
                    report.utilities.projections[utility].to_vec()
                };
                for (i, &amount) in projections.iter().enumerate() {
                    let label = if i == 0 {
                        replace(
                            &self.strings[1817],
                            &encoded("%i"),
                            &self.short_labels[category],
                        )
                    } else if i == 1 {
                        self.strings[1818].clone()
                    } else {
                        replace(
                            &self.strings[1819],
                            &encoded("%d"),
                            &encoded(&i.to_string()),
                        )
                    };
                    let value = money(amount)?;
                    rows.push((label, value, projection_row + i));
                }
                let owner_heading = projection_row + 5;
                headers.push((owner_heading, if railroad { 1820 } else { 1805 }));
                for (i, &id) in categories.iter().rev().enumerate() {
                    rows.push((
                        self.short_labels[id].clone(),
                        owner_name(report.owners[id])?,
                        owner_heading + 1 + i,
                    ));
                }
            }
            for (row, id) in headers {
                heading_background.rect(layout.heading(row), [self.text_palette[17]; 4]);
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &self.strings[id],
                    layout.line(row, false),
                    false,
                    self.text_palette[16],
                )?;
            }
            surfaces.push(heading_background);
            for (label, value, row) in rows {
                let value_bounds = layout.line(row, true);
                let mut label_bounds = layout.line(row, false);
                // Preserve readable values when a localized label exceeds the
                // available gap. This fitting is a host accommodation; the full
                // original text-widget clipping path is not reconstructed here.
                let value_width = ui::text_width(&self.tab, &value, 1.)? as f32;
                label_bounds[2] = label_bounds[2].min(value_bounds[2] - value_width - 3.);
                ui::text_fit(
                    &mut fonts,
                    &self.tab,
                    &label,
                    label_bounds,
                    false,
                    self.text_palette[0],
                )?;
                ui::text_fit(
                    &mut fonts,
                    &self.tab,
                    &value,
                    value_bounds,
                    true,
                    self.text_palette[3],
                )?;
            }
        }
        surfaces.extend(fonts);
        surfaces.extend(title_font);
        surfaces.extend(body);
        let draws = surfaces.iter().map(Surface::draw).collect::<Vec<_>>();
        Ok((
            renderer.prepare(
                &draws,
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
            )?,
            [0., 0., 196., 320.],
        ))
    }
    pub fn state(&self, scene: &Scene) -> Result<Option<Locator>, String> {
        scene
            .saved_business_catalog
            .as_ref()
            .map(|catalog| {
                for row in catalog.rows() {
                    let mask = u32::from_le_bytes(row[0x70..0x74].try_into().unwrap());
                    if u32::from(row[0x4c]) != mask.count_ones() {
                        return Err("Locator saved offer count/mask disagree".into());
                    }
                }
                let mut state = self.people_state()?;
                state.saved_panels = true;
                state.player = crate::player_info::State::new(&scene.saved_players)?;
                state.other_rows = commodity_rows(
                    scene
                        .saved_commodity_rows
                        .as_ref()
                        .ok_or("missing Locator saved commodities")?,
                    &self.commodity_order,
                    self.memberships,
                )?;
                state.rows = rows(catalog, &self.order)?;
                state.catalog = Some(catalog.clone());
                Ok(state)
            })
            .transpose()
    }
    /// Fresh scripts mutate these same original BusinessInfo and Commodity
    /// rows. Keep their live values instead of requiring a saved catalog.
    pub fn runtime_state(&self, inputs: &RuntimeInputs) -> Result<Locator, String> {
        let mut state = self.people_state()?;
        state.saved_panels = true;
        state.rows = rows(&inputs.catalog, &self.order)?;
        state.other_rows = commodity_rows(
            &inputs.commodities,
            &self.commodity_order,
            inputs.memberships,
        )?;
        state.catalog = Some(inputs.catalog.clone());
        state.owned_eligible = inputs.owned_eligible;
        Ok(state)
    }
    pub fn people_state(&self) -> Result<Locator, String> {
        Ok(Locator {
            strategic_state:None,
            city_info: false,
            city_people: None,
            people_mode: 0,
            polls: None,
            poll_category: None,
            poll_info: false,
            ownership: None,
            rights: None,
            rights_mode: false,
            ownership_mode: false,
            prestige: None,
            prestige_mode: false,
            ownership_scroll: 0,
            pending_category: None,
            player_info: false,
            player: crate::player_info::State::new(&[])?,
            saved_panels: false,
            catalog: None,
            owned_rows: Vec::new(),
            my_business: true,
            owned_scroll: 0,
            owned_eligible: self.owned_eligible,
            other_rows: Vec::new(),
            commodity: false,
            other_selected: None,
            other_scroll: 0,
            rows: Vec::new(),
            open: false,
            selected: None,
            scroll: 0,
            pressed: None,
            hover: None,
            tutorial_activated: None,
            dirty: true,
            prepared: None,
        })
    }
    pub fn prepare(&self,renderer:&Renderer,state:&Locator,scene:&Scene)->Result<PreparedScene,String>{
        {let mut counter=0;self.prepare_graph(renderer,state,scene,true,true,None,state.navigation_report_mode(),&mut counter)}
    }
    ///C body and D world/report producers have separate actual owners.
    ///Tint keys are original600-building slot identities, not render indices.
    pub(crate) fn prepare_graph(
        &self,renderer:&Renderer,state:&Locator,scene:&Scene,c_body:bool,d_body:bool,
        colors:Option<&[u32;600]>,report_mode:u32,message_counter:&mut u32,
    )->Result<PreparedScene,String>{
        let mut surfaces=Vec::new();
        // Original42a710 submits residents after every city-info producer.
        // Non-People tabs reset global5336bc to zero (white marker mode).
        if d_body && state.body_open() && state.city_info {
            let white_people = matches!(report_mode,2|3|4);
            let mut dots = self.people_dot.clone();
            if let Some(report) = &state.city_people {
                let matrix = glam::Mat4::from_cols_array(&scene.camera);
                for marker in report.markers.iter().filter(|m| m.visible) {
                    let [x, y, z] = marker.position;
                    let clip = matrix * glam::Vec4::new(x, y, z, 1.);
                    if clip.w <= 0. || clip.z < 0. || clip.z > clip.w {
                        continue;
                    }
                    let point = [(clip.x / clip.w + 1.) * 320., (1. - clip.y / clip.w) * 240.];
                    dots.quad(
                        crate::people_markers::quad(point),
                        [[0., 1.], [0., 0.], [1., 0.], [1., 1.]],
                        [if white_people {
                            0xffff_ffff
                        } else {
                            marker.colors[state.people_mode]
                        }; 4],
                    );
                }
            }
            surfaces.insert(0, dots);
        }
        if d_body && state.body_open() && state.city_info && report_mode==2 {
            let mut stars = self.prestige_star.clone();
            if let Some(report) = &state.prestige {
                for &id in &report.overlay_categories {
                    let [x, z] = self.category_centers[id];
                    let matrix = glam::Mat4::from_cols_array(&scene.camera);
                    let clip = matrix * glam::Vec4::new(x, 0., z, 1.);
                    if clip.w <= 0. || clip.z < 0. || clip.z > clip.w {
                        continue;
                    }
                    let point = [(clip.x / clip.w + 1.) * 320., (1. - clip.y / clip.w) * 240.];
                    for [l, t] in crate::prestige_stars::positions(
                        report.values[id],
                        point,
                        self.star_size,
                        self.star_bias,
                    )? {
                        let r = l + self.star_size[0] as f32;
                        let b = t + self.star_size[1] as f32;
                        stars.quad(
                            [[l, t], [l, b], [r, b], [r, t]],
                            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                            [0xffffffff; 4],
                        );
                    }
                }
            }
            surfaces.insert(0, stars);
        }
        if d_body && state.body_open() && state.city_info && report_mode==3 {
            let mut badges = Vec::new();
            if let (Some(report), Some(prestige)) = (&state.ownership, &state.prestige) {
                for &id in &prestige.overlay_categories {
                    let [x, z] = self.category_centers[id];
                    let clip =
                        glam::Mat4::from_cols_array(&scene.camera) * glam::Vec4::new(x, 0., z, 1.);
                    if clip.w <= 0. || clip.z < 0. || clip.z > clip.w {
                        continue;
                    }
                    let point = [(clip.x / clip.w + 1.) * 320., (1. - clip.y / clip.w) * 240.];
                    let badge = report.badges[id];
                    let mut background = Surface::solid();
                    for (radius, color) in [
                        (19., self.ownership_styles[badge.character.unwrap_or(10)].0),
                        (15., 0x55000000),
                    ] {
                        let [l, t, r, b] = crate::ownership_badges::rect(point, radius);
                        background.quad(
                            [[l, t], [l, b], [r, b], [r, t]],
                            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                            [color; 4],
                        );
                    }
                    badges.push(background);
                    if let Some(c) = badge.character {
                        let mut icon = self.ownership_icons[c].clone();
                        let [l, t, r, b] =
                            crate::ownership_badges::icon_rect(point, [icon.width, icon.height]);
                        icon.quad(
                            [[l, t], [l, b], [r, b], [r, t]],
                            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                            [0xffffffff; 4],
                        );
                        badges.push(icon);
                        let point = crate::ownership_badges::count_point(
                            point,
                            self.ownership_label_widths[id],
                            badge.remaining,
                        );
                        let encoded: Vec<_> = badge
                            .remaining
                            .to_string()
                            .bytes()
                            .map(|c| c - 32)
                            .collect();
                        let mut fonts = self.ownership_fonts.clone();
                        for q in openmt2001::font::Font::parse(&self.ownership_tab)?.layout(
                            &encoded,
                            openmt2001::font::Layout {
                                origin: point,
                                depth: 0.5,
                                scale: 1.,
                                gap: 0,
                                baseline: -1,
                            },
                        )? {
                            fonts[q.page].quad(
                                q.positions.map(|p| [p[0], p[1]]),
                                q.uv,
                                [0xffffffff; 4],
                            );
                        }
                        badges.extend(fonts);
                    }
                }
            }
            badges.append(&mut surfaces);
            surfaces = badges;
        }
        if d_body && state.body_open() && state.city_info && report_mode==4 {
            let mut badges = Vec::new();
            if let (Some(rights), Some(prestige)) = (&state.rights, &state.prestige) {
                for &id in &prestige.overlay_categories {
                    let [x, z] = self.category_centers[id];
                    let clip =
                        glam::Mat4::from_cols_array(&scene.camera) * glam::Vec4::new(x, 0., z, 1.);
                    if clip.w <= 0. || clip.z < 0. || clip.z > clip.w {
                        continue;
                    }
                    let point = [(clip.x / clip.w + 1.) * 320., (1. - clip.y / clip.w) * 240.];
                    let c = rights.categories[id];
                    if let Some(badge) = crate::city_rights::badge(
                        c.mode,
                        c.owner == rights.owner_number,
                        point,
                        [self.rights_icon.width, self.rights_icon.height],
                        0,
                    ) {
                        self.draw_rights_badge(&mut badges, badge);
                        if c.owner != 0 {
                            let mut fonts = self.ownership_fonts.clone();
                            let encoded = c
                                .remaining
                                .to_string()
                                .bytes()
                                .map(|b| b - 32)
                                .collect::<Vec<_>>();
                            for q in openmt2001::font::Font::parse(&self.ownership_tab)?.layout(
                                &encoded,
                                openmt2001::font::Layout {
                                    origin: crate::city_rights::text_point(point, c.remaining),
                                    depth: 0.5,
                                    scale: 1.,
                                    gap: 0,
                                    baseline: -1,
                                },
                            )? {
                                fonts[q.page].quad(
                                    q.positions.map(|p| [p[0], p[1]]),
                                    q.uv,
                                    [0xffffffff; 4],
                                );
                            }
                            badges.extend(fonts);
                        }
                    }
                }
            }
            badges.append(&mut surfaces);
            surfaces = badges;
        }
        if d_body && state.body_open() && state.city_info {
            let mut labels = Vec::new();
            if let Some(report) = &state.prestige {
                for &id in &report.overlay_categories {
                    let [x, z] = self.category_centers[id];
                    let clip =
                        glam::Mat4::from_cols_array(&scene.camera) * glam::Vec4::new(x, 0., z, 1.);
                    if clip.w <= 0. || clip.z < 0. || clip.z > clip.w {
                        continue;
                    }
                    let point = [(clip.x / clip.w + 1.) * 320., (1. - clip.y / clip.w) * 240.];
                    let label = crate::strategic_labels::label_point(
                        point,
                        self.ownership_label_widths[id],
                    );
                    let mut fonts = self.ownership_fonts.clone();
                    for q in openmt2001::font::Font::parse(&self.ownership_tab)?.layout(
                        &self.short_labels[id],
                        openmt2001::font::Layout {
                            origin: [label[0].trunc() + 2., label[1].trunc()],
                            depth: 0.5,
                            scale: 1.,
                            gap: 0,
                            baseline: -1,
                        },
                    )? {
                        fonts[q.page].quad(
                            q.positions.map(|p| [p[0], p[1]]),
                            q.uv,
                            [0xffffffff; 4],
                        );
                    }
                    labels.extend(fonts);
                    if let Some((source, color)) = &self.civic_icons[id] {
                        let mut bg = Surface::solid();
                        let [l, t, r, b] = crate::strategic_labels::background(point);
                        bg.quad(
                            [[l, t], [l, b], [r, b], [r, t]],
                            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                            [*color; 4],
                        );
                        labels.push(bg);
                        let mut icon = source.clone();
                        let [l, t] =
                            crate::strategic_labels::icon_point(point, [icon.width, icon.height]);
                        let r = l + icon.width as f32;
                        let b = t + icon.height as f32;
                        icon.quad(
                            [[l, t], [l, b], [r, b], [r, t]],
                            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                            [0xffffffff; 4],
                        );
                        labels.push(icon);
                    }
                }
            }
            labels.append(&mut surfaces);
            surfaces = labels;
        }
        if let Some(id) = state.poll_category.filter(|_|d_body)
            && state
                .prestige
                .as_ref()
                .is_some_and(|p| p.overlay_categories.contains(&id))
        {
            let [x, z] = self.category_centers[id];
            let clip = glam::Mat4::from_cols_array(&scene.camera) * glam::Vec4::new(x, 0., z, 1.);
            if clip.w > 0. && clip.z >= 0. && clip.z <= clip.w {
                let point = [(clip.x / clip.w + 1.) * 320., (1. - clip.y / clip.w) * 240.];
                //Actual42bff0 counter is owned by D, advanced at UI dispatch.
                let bracket = crate::strategic_selection::brackets(point, *message_counter as i32);
                *message_counter=bracket.counter as u32;
                let mut draw = Surface::solid();
                for rect in bracket.rectangles {
                    draw.rect(rect, [bracket.color; 4]);
                }
                // Draw above map badges while retaining panel occlusion.
                let before_panel =
                    surfaces
                        .len()
                        .saturating_sub(if d_body && state.body_open() { self.base.len() } else { 0 });
                surfaces.insert(before_panel, draw);
            }
        }
        let mut world_surfaces=std::mem::take(&mut surfaces);
        if c_body {
        surfaces=if state.body_open(){self.base.clone()}else{Vec::new()};
        if state.body_open()
            && state.city_info
            && !state.prestige_mode
            && !state.ownership_mode
            && !state.rights_mode
        {
            for right in [false, true] {
                let action = Action::PeopleStep(right);
                let mut arrow =
                    self.people_arrows[usize::from(right)][if state.hover == Some(action) {
                        if state.pressed == Some(action) { 1 } else { 2 }
                    } else {
                        0
                    }]
                    .clone();
                let [l, t, r, b] = self.people_arrow_bounds(right);
                arrow.quad(
                    [[l, t], [l, b], [r, b], [r, t]],
                    [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                    [0xffffffff; 4],
                );
                surfaces.push(arrow);
            }
        }
        for (action, bounds) in [
            (Action::City, [5.6, 53.6, 28.8, 76.8]),
            (Action::Player, [5.6, 77.6, 28.8, 100.8]),
            (Action::Toggle, [5.6, 101.6, 28.8, 124.8]),
            (Action::Poll, [5.6, 125.6, 28.8, 148.8]),
        ] {
            if (action == Action::Player && state.player.players.is_empty())
                || (action == Action::Toggle && !state.saved_panels)
            {
                continue;
            }
            if action == Action::Poll && state.polls.is_none() {
                continue;
            }
            if action == Action::City && state.city_people.is_none() {
                continue;
            }
            let mut background = self.icon_backgrounds[if state.hover == Some(action) {
                if state.pressed == Some(action) { 1 } else { 2 }
            } else {
                0
            }]
            .clone();
            let [l, t, r, b] = bounds;
            background.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(background);
        }
        if state.polls.is_some() {
            surfaces.push(self.poll_icon.clone());
        }
        if state.saved_panels {
            surfaces.push(self.icon.clone());
        }
        if !state.player.players.is_empty() {
            surfaces.push(self.player_icon.clone());
        }
        if state.city_people.is_some() {
            surfaces.push(self.city_icon.clone());
        }
        let mut fonts = self.fonts.clone();
        let mut solid = Surface::solid();
        if state.body_open() && state.poll_info {
            self.poll_report(state, scene, &mut fonts)?;
        } else if state.body_open() && state.city_info {
            solid.rect(self.city_layout.header, [self.text_palette[17]; 4]);
            if state.rights_mode {
                surfaces.extend(self.rights_report(state, &mut fonts)?);
            } else if state.prestige_mode {
                self.prestige_report(state, &mut fonts)?;
            } else if state.ownership_mode {
                self.ownership_report(state, &mut fonts)?;
            } else {
                self.people_report(state, &mut fonts, &mut solid)?;
            }
            if state.ownership.is_some() {
                for i in 0..4 {
                    if i == 2 && state.rights.is_none() {
                        continue;
                    }
                    let action = Action::CityMode(i as u8);
                    let selected = if state.rights_mode {
                        i == 2
                    } else if state.ownership_mode {
                        i == 3
                    } else if state.prestige_mode {
                        i == 1
                    } else {
                        i == 0
                    };
                    let mut background =
                        self.city_tab_backgrounds[if selected || state.pressed == Some(action) {
                            1
                        } else if state.hover == Some(action) {
                            2
                        } else {
                            0
                        }]
                        .clone();
                    let [l, t, r, b] = self.city_layout.buttons[i];
                    background.quad(
                        [[l, t], [l, b], [r, b], [r, t]],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [0xffffffff; 4],
                    );
                    surfaces.push(background);
                    surfaces.push(self.city_tabs[i].clone());
                }
            }
        } else if state.body_open() && state.player_info {
            surfaces.extend(self.player_report(state, &mut fonts)?);
        } else if state.body_open() {
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[1451],
                [32., 7., 187., 21.],
                false,
                self.text_palette[0],
            )?;
            let mode = if state.my_business {
                0
            } else if state.commodity {
                2
            } else {
                1
            };
            for index in 0..3 {
                let action = Action::Mode(index);
                let mut background =
                    self.city_tab_backgrounds[if index == mode || state.pressed == Some(action) {
                        1
                    } else if state.hover == Some(action) {
                        2
                    } else {
                        0
                    }]
                    .clone();
                let [l, t, r, b] = self.city_layout.buttons[index];
                background.quad(
                    [[l, t], [l, b], [r, b], [r, t]],
                    [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                    [0xffffffff; 4],
                );
                surfaces.push(background);
                let mut icon = self.locator_tabs[index].clone();
                let [l, t, r, b] = self.city_layout.icons[index];
                icon.quad(
                    [[l, t], [l, b], [r, b], [r, t]],
                    [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                    [0xffffffff; 4],
                );
                surfaces.push(icon);
            }
            solid.rect(self.city_layout.header, [self.text_palette[17]; 4]);
            ui::text_fit(
                &mut fonts,
                &self.tab,
                &self.strings[1447 + mode],
                [
                    32.,
                    self.city_layout.header[1] + 1.,
                    184.,
                    self.city_layout.header[3],
                ],
                false,
                self.text_palette[16],
            )?;
            if state.my_business {
                let list = self.owned_list();
                let mut owned_fonts = self.ownership_fonts.clone();
                for (index, row) in state
                    .owned_rows
                    .iter()
                    .skip(state.owned_scroll)
                    .take(self.owned_visible())
                    .enumerate()
                {
                    if row.label == u32::MAX {
                        continue;
                    }
                    let label = if row.label & 0x8000_0000 != 0 {
                        &self.block_labels[(row.label & 0x7fff_ffff) as usize]
                    } else {
                        self.strings
                            .get(row.label as usize)
                            .ok_or("owned-business Locator label outside language")?
                    };
                    let y = list[1] + 3. + index as f32 * 10.;
                    let bounds = [
                        list[0] + if row.building.is_some() { 2. } else { 0. },
                        y,
                        list[2],
                        y + 10.,
                    ];
                    ui::text_fit(
                        &mut owned_fonts,
                        &self.ownership_tab,
                        label,
                        bounds,
                        false,
                        self.text_palette[if row.building.is_some() { 3 } else { 0 }],
                    )?;
                }
                surfaces.extend(owned_fonts);
            } else {
                let mut type_fonts = self.ownership_fonts.clone();
                let mut type_selection = Surface::solid();
                for (n, row) in state
                    .rows
                    .iter()
                    .skip(state.scroll)
                    .take(self.visible())
                    .enumerate()
                {
                    let y = self.list[1] + n as f32 * 10.;
                    if state.selected == Some(row.kind) && row.kind != u32::MAX {
                        type_selection.rect(type_selection_bounds(self.list, n), [0xff288be1; 4]);
                    }
                    if row.label != u32::MAX {
                        let text = self
                            .strings
                            .get(row.label as usize)
                            .ok_or("Locator label leaves language")?;
                        ui::text(
                            &mut type_fonts,
                            &self.ownership_tab,
                            text,
                            [self.list[0] + 2., y + 3., self.list[2], self.list[3]],
                            false,
                            if row.kind == u32::MAX {
                                self.text_palette[16]
                            } else {
                                self.text_palette[0]
                            },
                        )?;
                    }
                }
                surfaces.push(type_selection);
                surfaces.extend(type_fonts);
                let maximum = state.rows.len().saturating_sub(self.visible()) as i32;
                let enabled = maximum > 0;
                ui::Slider {
                    bounds: self.scrollbar,
                    vertical: true,
                    value: state.scroll as f32,
                    range: [0., maximum as f32],
                    pointer: None,
                    pressed: None,
                }
                .paint_handle(&mut solid);
                for (side, bounds) in ui::slider_arrows(self.scrollbar, true)
                    .into_iter()
                    .enumerate()
                {
                    let action = Action::Scroll(if side == 0 { -1 } else { 1 });
                    let texture = ui::slider_arrow_texture(
                        side,
                        state.pressed == Some(action),
                        state.hover == Some(action),
                        enabled,
                        true,
                    );
                    let mut icon = self.scrollbar_textures[side][texture - [105, 93][side]].clone();
                    let [l, t, r, b] = bounds;
                    icon.quad(
                        [[l, t], [l, b], [r, b], [r, t]],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [0xffffffff; 4],
                    );
                    surfaces.push(icon);
                }
            }
        }
        surfaces.push(solid);
        surfaces.extend(fonts);
        } //C body and child owners
        world_surfaces.extend(surfaces);
        surfaces=world_surfaces;
        let footprint = if d_body {self.footprints_graph(state,scene,colors)?}else{Surface::solid()};
        let mut ground = footprint.draw();
        ground.view_projection = Some(scene.camera);
        let mut draws = vec![ground];
        draws.extend(surfaces.iter().map(Surface::draw));
        renderer.prepare(
            &draws,
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
    fn poll_report(
        &self,
        state: &Locator,
        scene: &Scene,
        fonts: &mut [Surface],
    ) -> Result<(), String> {
        ui::text_fit(
            fonts,
            &self.tab,
            &self.strings[11],
            [32., 7., 187., 21.],
            false,
            0xffffffff,
        )?;
        let Some(poll) = state
            .poll_category
            .and_then(|i| state.polls.as_ref().and_then(|p| p.get(i)))
        else {
            return ui::text_fit(
                fonts,
                &self.tab,
                &self.strings[1465],
                [32., 32., 187., 48.],
                false,
                0xffffffff,
            );
        };
        if let Some(label) = state.poll_category.and_then(|i| self.block_labels.get(i)) {
            ui::text_fit(
                fonts,
                &self.tab,
                label,
                [32., 23., 187., 35.],
                false,
                0xffffd080,
            )?;
        }
        ui::text_fit(
            fonts,
            &self.tab,
            &self.strings[25],
            [32., 40., 147., 54.],
            false,
            0xffffffff,
        )?;
        let count: Vec<_> = poll.residents.to_string().bytes().map(|b| b - 32).collect();
        ui::text_fit(
            fonts,
            &self.tab,
            &count,
            [148., 40., 187., 54.],
            true,
            0xffffffff,
        )?;
        if poll.residents == 0 {
            let (offsets, copy) =
                crate::prompt::wrap_with_gap(&self.tab, &self.strings[1138], 155, -1)?;
            for (i, pair) in offsets.windows(2).enumerate() {
                let end = (pair[1] - 1).min(copy.len());
                let start = pair[0].min(end);
                ui::text(
                    fonts,
                    &self.tab,
                    &copy[start..end],
                    [32., 58. + i as f32 * 13., 187., 100.],
                    false,
                    0xffffffff,
                )?;
            }
        } else if let Some(commodities) = &scene.saved_commodity_rows {
            ui::text_fit(
                fonts,
                &self.tab,
                &self.strings[26],
                [32., 58., 187., 72.],
                false,
                0xffffd080,
            )?;
            for (i, (id, count)) in poll.ranked().iter().enumerate() {
                let label =
                    u32::from_le_bytes(commodities[*id][8..12].try_into().unwrap()) as usize;
                let label = self
                    .strings
                    .get(label)
                    .ok_or("poll commodity label outside language")?;
                let y = 76. + i as f32 * 12.;
                ui::text_fit(
                    fonts,
                    &self.tab,
                    label,
                    [32., y, 157., y + 12.],
                    false,
                    0xffffffff,
                )?;
                let count: Vec<_> = count.to_string().bytes().map(|b| b - 32).collect();
                ui::text_fit(
                    fonts,
                    &self.tab,
                    &count,
                    [158., y, 187., y + 12.],
                    true,
                    0xffffffff,
                )?;
            }
        }
        Ok(())
    }
    fn prestige_report(&self, state: &Locator, fonts: &mut [Surface]) -> Result<(), String> {
        use crate::city_prestige::Row;
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1450],
            [32., 7., 187., 21.],
            false,
            0xffffffff,
        )?;
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1443],
            self.city_layout.header,
            true,
            self.text_palette[16],
        )?;
        if let Some(report) = &state.prestige {
            for (i, row) in report
                .rows
                .iter()
                .skip(state.ownership_scroll)
                .take(17)
                .enumerate()
            {
                let (text, color) = match row {
                    Row::Space => continue,
                    Row::Block(id) => (self.block_labels[*id].clone(), 0xffffffff),
                    Row::Grade(grade) => {
                        let template = &self.strings[43];
                        let marker = [b'%' - 32, b'd' - 32];
                        let at = template
                            .windows(2)
                            .position(|w| w == marker)
                            .ok_or("prestige grade template lacks original integer field")?;
                        let mut text = template[..at].to_vec();
                        text.extend(grade.to_string().bytes().map(|b| b - 32));
                        text.extend_from_slice(&template[at + 2..]);
                        (text, 0xffffd080)
                    }
                };
                let y = 74. + i as f32 * 10.;
                ui::text_fit(
                    fonts,
                    &self.tab,
                    &text,
                    [32., y, 187., y + 10.],
                    false,
                    color,
                )?;
            }
        }
        Ok(())
    }
    fn ownership_report(&self, state: &Locator, fonts: &mut [Surface]) -> Result<(), String> {
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1450],
            [32., 7., 187., 21.],
            false,
            0xffffffff,
        )?;
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1445],
            self.city_layout.header,
            true,
            self.text_palette[16],
        )?;
        let mut rows: Vec<(Vec<u8>, u32)> = Vec::new();
        if let Some(report) = &state.ownership {
            for (i, group) in report.groups.iter().enumerate() {
                // Both saved initialization and retained live snapshots feed
                // this copied native player+4 report representation.
                let player = state
                    .player
                    .records
                    .get(i)
                    .ok_or("ownership player unavailable")?;
                let name = &player[0x154..];
                let end = name
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or("unterminated ownership player name")?;
                let name: Result<Vec<_>, String> = name[..end]
                    .iter()
                    .map(|b| {
                        if (32..=126).contains(b) {
                            Ok(b - 32)
                        } else {
                            Err("unsupported ownership name glyph".into())
                        }
                    })
                    .collect();
                rows.push((name?, 0xffffd080));
                if group.is_empty() {
                    rows.push((self.strings[19].clone(), 0xffffffff));
                }
                for &category in group {
                    rows.push((self.block_labels[category].clone(), 0xffffffff));
                }
            }
        }
        for (i, (text, color)) in rows
            .iter()
            .skip(state.ownership_scroll)
            .take(17)
            .enumerate()
        {
            let y = 74. + i as f32 * 10.;
            ui::text_fit(
                fonts,
                &self.tab,
                text,
                [32., y, 187., y + 10.],
                false,
                *color,
            )?;
        }
        Ok(())
    }
    fn draw_rights_badge(&self, out: &mut Vec<Surface>, badge: crate::city_rights::Badge) {
        let mut background = Surface::solid();
        background.rect(badge.background, [badge.color; 4]);
        out.push(background);
        let mut icon = self.rights_icon.clone();
        let [l, t, r, b] = badge.icon_rect;
        icon.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [badge.icon_color; 4],
        );
        out.push(icon);
    }
    fn rights_report(
        &self,
        state: &Locator,
        fonts: &mut [Surface],
    ) -> Result<Vec<Surface>, String> {
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1450],
            [32., 7., 187., 21.],
            false,
            0xffffffff,
        )?;
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1444],
            self.city_layout.header,
            true,
            self.text_palette[16],
        )?;
        let mut out = Vec::new();
        if let Some(rights) = &state.rights {
            ui::text_fit(
                fonts,
                &self.tab,
                &rights.player_name,
                [32., 72., 187., 84.],
                false,
                0xffffd080,
            )?;
        }
        let mut body = self.ownership_fonts.clone();
        let mut y = 84.;
        for (title, description, mode, owned) in [
            (13, 14, 0, false),
            (15, 16, 1, false),
            (17, 18, 1, true),
            (1801, 1441, 0, true),
        ] {
            self.draw_rights_badge(
                &mut out,
                crate::city_rights::badge(
                    mode,
                    owned,
                    [47., y + 16.],
                    [self.rights_icon.width, self.rights_icon.height],
                    0,
                )
                .unwrap(),
            );
            ui::text_fit(
                fonts,
                &self.tab,
                &self.strings[title],
                [67., y, 187., y + 12.],
                false,
                0xffffd080,
            )?;
            let (offsets, text) = crate::prompt::wrap_with_gap(
                &self.ownership_tab,
                &self.strings[description],
                120,
                -1,
            )?;
            for (line, pair) in offsets.windows(2).enumerate() {
                let encoded =
                    &text[pair[0].min(text.len())..pair[1].saturating_sub(1).min(text.len())];
                for q in openmt2001::font::Font::parse(&self.ownership_tab)?.layout(
                    encoded,
                    openmt2001::font::Layout {
                        origin: [67., y + 14. + line as f32 * 10.],
                        depth: 0.5,
                        scale: 1.,
                        gap: -1,
                        baseline: -1,
                    },
                )? {
                    body[q.page].quad(q.positions.map(|p| [p[0], p[1]]), q.uv, [0xffffffff; 4]);
                }
            }
            y += 16. + offsets.windows(2).count() as f32 * 10.;
        }
        out.extend(body);
        Ok(out)
    }
    fn people_report(
        &self,
        state: &Locator,
        fonts: &mut [Surface],
        solid: &mut Surface,
    ) -> Result<(), String> {
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1450],
            [32., 7., 187., 21.],
            false,
            0xffffffff,
        )?;
        // Original 410f89 names the page; 4105ef sets its People caption.
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1442],
            self.city_layout.header,
            true,
            self.text_palette[16],
        )?;
        let [l, top, r, bottom] = self.people_selector;
        ui::text_fit(
            fonts,
            &self.tab,
            &self.strings[1802],
            [l, top + 4., r, bottom + 4.],
            false,
            self.text_palette[0],
        )?;
        ui::text(
            fonts,
            &self.tab,
            &self.strings[crate::people_markers::LABELS[state.people_mode] as usize],
            [l + 16., top + 21., r - 16., bottom + 21.],
            true,
            0xffffffff,
        )?;
        if let Some(report) = &state.city_people {
            for (i, row) in report.rows.iter().enumerate() {
                let y = self.bounds[1] + 70. + i as f32 * 10.;
                if row.style == 2 {
                    continue;
                }
                if row.style == 1 {
                    //42fa34..42fae6 fills the preceding heading's10-pixel row
                    //when the next ordinary row changes its type.
                    solid.rect([32., y, 187., y + 10.], [self.text_palette[17]; 4]);
                }
                if state.people_mode != 0 {
                    let start = 3 + (state.people_mode - 1) * 5;
                    if (start..start + 3).contains(&i) {
                        let color = self.people_palette[i - start];
                        solid.rect(
                            [32., y, 187., y + 11.],
                            [color, color, color & 0xffffff, color & 0xffffff],
                        );
                    }
                }
                ui::text_fit(
                    fonts,
                    &self.tab,
                    &self.strings[row.label as usize],
                    [32., y, if row.style == 0 { 135. } else { 187. }, y + 13.],
                    false,
                    if row.style == 1 {
                        self.text_palette[16]
                    } else {
                        0xffffffff
                    },
                )?;
                if row.style == 0 {
                    let text = format!("{}{}", row.value, if row.label == 1453 { "" } else { "%" });
                    ui::text_fit(
                        fonts,
                        &self.tab,
                        &text.bytes().map(|b| b - 32).collect::<Vec<_>>(),
                        [136., y, 187., y + 13.],
                        true,
                        self.text_palette[3],
                    )?;
                }
            }
        }
        Ok(())
    }
    fn player_report(
        &self,
        state: &Locator,
        fonts: &mut [Surface],
    ) -> Result<Vec<Surface>, String> {
        ui::text(
            fonts,
            &self.tab,
            &self.strings[1452],
            [32., 7., 187., 21.],
            false,
            self.text_palette[0],
        )?;
        let mut surfaces = Vec::new();
        let ordered = state.player_display_order();
        for (position, player) in ordered.iter().enumerate() {
            let character = u32::from_le_bytes(
                state.player.records[*player][0x14c..0x150]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let selected = state.player.players.get(state.player.selected) == Some(player);
            let action = Action::PlayerSelect {
                player: *player,
                position,
            };
            //443139..443185: selected state overrides press/hover textures.
            let variant = if selected {
                3
            } else if state.pressed == Some(action) {
                2
            } else if state.hover == Some(action) {
                1
            } else {
                0
            };
            let mut button = self
                .player_buttons
                .get(character)
                .ok_or("player character button")?[variant]
                .clone();
            let rect = *self
                .city_layout
                .buttons
                .get(position)
                .ok_or("more than six active player buttons")?;
            let [l, t, r, b] = rect;
            button.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(button);
            let mut icon = self.ownership_icons[character].clone();
            let [l, t, r, b] = crate::player_info::portrait_inset(rect)?;
            icon.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(icon);
        }
        let name = state.player.name()?;
        let token = [b'%' - 32, b's' - 32];
        let format = &self.strings[1446];
        let at = format
            .windows(2)
            .position(|v| v == token)
            .ok_or("finance title substitution missing")?;
        let mut title = format[..at].to_vec();
        title.extend(name);
        title.extend_from_slice(&format[at + 2..]);
        ui::text(
            fonts,
            &self.tab,
            &title,
            self.city_layout.header,
            true,
            self.text_palette[16],
        )?;
        for (i, rect) in self.player_period_rects().into_iter().enumerate() {
            let action = Action::PeriodStep(i != 0);
            let variant = if state.pressed == Some(action) {
                2
            } else if state.hover == Some(action) {
                1
            } else {
                0
            };
            let mut arrow = self.people_arrows[i][variant].clone();
            let [l, t, r, b] = rect;
            arrow.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(arrow);
        }
        let rects = self.player_period_rects();
        ui::text(
            fonts,
            &self.tab,
            &self.strings[if state.player.today { 33 } else { 34 }],
            [rects[0][2], rects[0][1], rects[1][0], rects[0][3]],
            true,
            self.text_palette[0],
        )?;
        let mut body = self.fonts.clone();
        let mut sections = Surface::solid();
        sections.rect(self.city_layout.header, [self.text_palette[17]; 4]);
        for (i, row) in state
            .player
            .rows
            .iter()
            .skip(state.player.scroll)
            .take(19)
            .enumerate()
        {
            let y = self.bounds[1] + 60. + i as f32 * 10.;
            if row.style >= 2 {
                continue;
            }
            if row.style == 1 {
                sections.rect(
                    [self.bounds[0], y, self.bounds[2] - 4., y + 10.],
                    [self.text_palette[17]; 4],
                );
            }
            let label = self
                .strings
                .get(row.label as usize)
                .ok_or("report label leaves language")?;
            ui::text_fit(
                &mut body,
                &self.tab,
                label,
                [32., y, if row.style == 0 { 116. } else { 186. }, y + 10.],
                false,
                self.text_palette[if row.style == 1 { 16 } else { 0 }],
            )?;
            if row.style == 0 {
                let value = f32::from_bits(row.amount);
                let mut text = vec![];
                if value < 0. {
                    text.push(b'-' - 32);
                }
                text.extend_from_slice(self.strings.get(2243).ok_or("currency symbol missing")?);
                text.extend(ui::decimal2(value.abs())?.bytes().map(|b| b - 32));
                ui::text_fit(
                    &mut body,
                    &self.tab,
                    &text,
                    [117., y, 186., y + 10.],
                    true,
                    self.text_palette[3],
                )?;
            }
        }
        surfaces.push(sections);
        surfaces.extend(body);
        Ok(surfaces)
    }
    // Original40dd0a..40df24: texture97 dimensions and main-window bounds.
    fn player_period_rects(&self) -> [[f32; 4]; 2] {
        let texture = &self.people_arrows[0][0];
        let [left, right, _] =
            crate::player_info::period_layout(self.bounds, [texture.width, texture.height]);
        [left, right]
    }
    fn footprints(&self,state:&Locator,scene:&Scene)->Result<Surface,String>{self.footprints_graph(state,scene,None)}
    fn footprints_graph(&self,state:&Locator,scene:&Scene,colors:Option<&[u32;600]>)->Result<Surface,String>{
        let mut surface = Surface::solid();
        if !state.body_open() || state.city_info || state.poll_info {
            return Ok(surface);
        }
        let selected=state.world_selected()?;
        for building in &scene.inspection_buildings {
            let base=if let Some(colors)=colors {
                let slot=building.runtime_slot.or(building.saved_slot).ok_or("D footprint has no actual600-slot identity")?;
                *colors.get(slot).ok_or("D footprint slot leaves600-record arena")?
            }else{owner_color(&self.owner_palette,building.saved_owner_character)?};
            let color = if let Some(business) = building.business() {
                if state.commodity {
                    let mask = if let Some(native)=&state.strategic_state{
                        let row=native.input.catalog.rows().get(business.business_type as usize)
                            .ok_or("C current footprint business type outside actual catalog")?;
                        u32::from_le_bytes(row[0x70..0x74].try_into().unwrap())
                    }else{scene
                        .saved_business_catalog
                        .as_ref()
                        .and_then(|c| c.rows().get(business.business_type as usize))
                        .map(|r| u32::from_le_bytes(r[0x70..0x74].try_into().unwrap()))
                        .unwrap_or(0)};
                    commodity_color(base, mask, selected, 0)
                } else {
                    type_color(base, business.business_type, selected, 0)
                }
            } else {
                0xffffffff
            };
            let bounds = building
                .bounds
                .map(|p| std::array::from_fn(|i| p[i] + building.translation[i]));
            let points = footprint(bounds);
            for i in [0, 1, 2, 0, 2, 3] {
                let p = points[i];
                surface.vertices.push([
                    p[0],
                    p[1],
                    p[2],
                    ((color >> 16) & 255) as f32 / 255.,
                    ((color >> 8) & 255) as f32 / 255.,
                    (color & 255) as f32 / 255.,
                    0.,
                    0.,
                ]);
                surface.alpha.push((color >> 24) as u8);
            }
        }
        Ok(surface)
    }
    pub(crate) fn refresh_extents(&self)->[usize;2]{[self.visible(),self.owned_visible()]}
    fn visible(&self) -> usize {
        ((self.list[3] - self.list[1]) / 10.) as usize
    }
    // Original40e830: body top+40, body bottom-5, width reduced by
    // generic texture105; both linked text columns use native font3.
    fn owned_list(&self) -> [f32; 4] {
        [
            self.list[0],
            self.bounds[1] + 40.,
            self.list[2] + 4.,
            self.bounds[3] - 5.,
        ]
    }
    fn owned_visible(&self) -> usize {
        let bounds = self.owned_list();
        ((bounds[3] - bounds[1] - 3.) / 10.) as usize
    }
    pub fn contains(&self, state: &Locator, p: [i32; 2]) -> bool {
        let p = p.map(|x| x as f32 * 0.5);
        state.city_people.is_some() && inside(p, [5.6, 53.6, 28.8, 76.8])
            || state.polls.is_some() && inside(p, [5.6, 125.6, 28.8, 148.8])
            || !state.player.players.is_empty() && inside(p, [5.6, 77.6, 28.8, 100.8])
            || state.saved_panels && inside(p, [5.6, 101.6, 28.8, 124.8])
            || state.body_open() && inside(p, [4.8, 4.8, self.bounds[2], self.bounds[3]])
    }
    fn hit(&self, state: &Locator, p: [i32; 2]) -> Action {
        let p = p.map(|x| x as f32 * 0.5);
        if state.polls.is_some() && inside(p, [5.6, 125.6, 28.8, 148.8]) {
            return Action::Poll;
        }
        if state.city_people.is_some() && inside(p, [5.6, 53.6, 28.8, 76.8]) {
            return Action::City;
        }
        if !state.player.players.is_empty() && inside(p, [5.6, 77.6, 28.8, 100.8]) {
            return Action::Player;
        }
        if state.saved_panels && inside(p, [5.6, 101.6, 28.8, 124.8]) {
            return Action::Toggle;
        }
        if state.body_open() && state.city_info {
            if state.prestige_mode && inside(p, [32., 74., 187., 244.]) {
                let row = state.ownership_scroll + ((p[1] - 74.) / 10.) as usize;
                if let Some(crate::city_prestige::Row::Block(category)) =
                    state.prestige.as_ref().and_then(|p| p.rows.get(row))
                {
                    return Action::OwnershipBlock(*category);
                }
            }
            if state.ownership_mode && inside(p, [32., 74., 187., 244.]) {
                let row = state.ownership_scroll + ((p[1] - 74.) / 10.) as usize;
                if let Some(report) = &state.ownership {
                    let mut rows = Vec::new();
                    for group in &report.groups {
                        rows.push(None);
                        if group.is_empty() {
                            rows.push(None);
                        } else {
                            rows.extend(group.iter().copied().map(Some));
                        }
                    }
                    if let Some(Some(category)) = rows.get(row) {
                        return Action::OwnershipBlock(*category);
                    }
                }
            }
            if !state.prestige_mode && !state.ownership_mode && !state.rights_mode {
                if inside(p, self.people_arrow_bounds(false)) {
                    return Action::PeopleStep(false);
                }
                if inside(p, self.people_arrow_bounds(true)) {
                    return Action::PeopleStep(true);
                }
            }
            if inside(p, self.city_layout.buttons[0]) {
                return Action::CityMode(0);
            }
            if state.prestige.is_some() && inside(p, self.city_layout.buttons[1]) {
                return Action::CityMode(1);
            }
            if state.rights.is_some() && inside(p, self.city_layout.buttons[2]) {
                return Action::CityMode(2);
            }
            if state.ownership.is_some() && inside(p, self.city_layout.buttons[3]) {
                return Action::CityMode(3);
            }
        }
        if !state.body_open() || state.city_info || state.poll_info {
            return Action::None;
        }
        if state.player_info {
            for (position, player) in state.player_display_order().into_iter().enumerate() {
                if self
                    .city_layout
                    .buttons
                    .get(position)
                    .is_some_and(|b| inside(p, *b))
                {
                    return Action::PlayerSelect { player, position };
                }
            }
            if let Some(i) = self
                .player_period_rects()
                .iter()
                .position(|&b| inside(p, b))
            {
                return Action::PeriodStep(i != 0);
            }
            return Action::None;
        }
        for index in 0..3 {
            if inside(p, self.city_layout.buttons[index]) {
                return Action::Mode(index);
            }
        }
        if state.my_business {
            let list = self.owned_list();
            if inside(p, list) && p[1] >= list[1] + 3. {
                let row = state.owned_scroll + ((p[1] - list[1] - 3.) / 10.) as usize;
                if let Some(building) = state.owned_rows.get(row).and_then(|row| row.building) {
                    return Action::OwnedBusiness(building);
                }
            }
            return Action::None;
        }
        if state.rows.len() > self.visible() {
            for (side, arrow) in ui::slider_arrows(self.scrollbar, true)
                .into_iter()
                .enumerate()
            {
                if inside(p, arrow) {
                    return Action::Scroll(if side == 0 { -1 } else { 1 });
                }
            }
            if inside(p, ui::slider_track(self.scrollbar, true)) {
                return Action::ScrollTrack;
            }
        }
        if inside(p, self.list) {
            let n = state.scroll + ((p[1] - self.list[1] - 3.) / 10.) as usize;
            if let Some(row) = state.rows.get(n)
                && row.kind != u32::MAX
            {
                return Action::Select(row.kind);
            }
        }
        Action::None
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Action {
    None,
    Toggle,
    City,
    Poll,
    CityMode(u8),
    PeopleStep(bool),
    OwnershipBlock(usize),
    Player,
    PlayerSelect { player: usize, position: usize },
    PeriodStep(bool),
    Mode(usize),
    OwnedBusiness(usize),
    Select(u32),
    Scroll(i32),
    ScrollTrack,
}
fn tutorial_control(action: Action) -> Option<u32> {
    match action {
        Action::City => Some(20),
        Action::Player => Some(21),
        Action::Toggle => Some(22),
        Action::Poll => Some(23),
        Action::CityMode(index) => Some(24 + u32::from(index)),
        Action::Mode(index) if index < 3 => Some(24 + index as u32),
        Action::OwnedBusiness(_) => Some(30),
        Action::Select(_) => Some(31),
        Action::PlayerSelect { position, .. } if position < 6 => Some(24 + position as u32),
        _ => None,
    }
}
pub struct Locator {
    strategic_state:Option<strategic::State>,
    saved_panels: bool,
    catalog: Option<BusinessCatalog>,
    city_info: bool,
    city_people: Option<crate::city_people::Report>,
    people_mode: usize,
    polls: Option<[crate::consumer_polls::Poll; 52]>,
    poll_category: Option<usize>,
    poll_info: bool,
    ownership: Option<crate::city_ownership::Ownership>,
    ownership_mode: bool,
    rights: Option<crate::city_rights::Rights>,
    rights_mode: bool,
    prestige: Option<crate::city_prestige::Prestige>,
    prestige_mode: bool,
    ownership_scroll: usize,
    pending_category: Option<usize>,
    player_info: bool,
    player: crate::player_info::State,
    commodity: bool,
    my_business: bool,
    owned_rows: Vec<OwnedRow>,
    owned_scroll: usize,
    owned_eligible: [bool; 52],
    other_rows: Vec<Row>,
    other_selected: Option<u32>,
    other_scroll: usize,
    rows: Vec<Row>,
    pub open: bool,
    selected: Option<u32>,
    scroll: usize,
    pressed: Option<Action>,
    hover: Option<Action>,
    tutorial_activated: Option<u32>,
    pub dirty: bool,
    pub prepared: Option<PreparedScene>,
}
// An offset is meaningful only while its anchor identity still exists.
fn remap_scroll<T, K: PartialEq>(old: &[T], offset: usize, new: &[T], visible: usize, key: impl Fn(&T) -> K) -> usize {
    old.get(offset).and_then(|anchor| {
        let mut matches = new.iter().enumerate().filter(|(_, r)| key(r) == key(anchor));
        let first = matches.next()?.0;
        matches.next().is_none().then_some(first)
    })
        .unwrap_or(0).min(new.len().saturating_sub(visible))
}

impl Locator {
    pub(crate) fn navigation_report_mode(&self)->u32 {
        if let Some(state)=&self.strategic_state{return state.report_mode;}
        if self.poll_info{1}else if self.city_info {
            if self.prestige_mode{2}else if self.rights_mode{4}else if self.ownership_mode{3}else{0}
        }else{0}
    }
    /// Apply only presentation choices to a fully attached authoritative snapshot.
    pub fn retain_ui_from(&mut self, old: &Self, assets: &Assets, old_scene: &Scene, scene: &Scene) -> Result<(), String> {
        self.retain_refresh_ui_from(old, assets.refresh_extents(),
            &old_scene.inspection_buildings, &scene.inspection_buildings)
    }
    pub(crate) fn retain_refresh_ui_from(&mut self, old:&Self, extents:[usize;2],
        old_scene:&[openmt2001::render_scene::InspectionBuilding],
        scene:&[openmt2001::render_scene::InspectionBuilding])->Result<(),String>{
        // A building row is identified by its runtime slot, not its scene index.
        let owned_key = |row: &OwnedRow, scene: &[openmt2001::render_scene::InspectionBuilding]| -> Option<(bool, usize)> {
            match row.building {
                Some(index) => scene.get(index)?.runtime_slot
                    .map(|slot| (true, slot)),
                None if row.label != u32::MAX => Some((false, row.label as usize)),
                None => None,
            }
        };
        let anchor = old.owned_rows.get(old.owned_scroll).and_then(|r| owned_key(r, old_scene));
        let scroll = anchor.and_then(|key| self.owned_rows.iter().position(|r| owned_key(r, scene) == Some(key)))
            .unwrap_or(0).min(self.owned_rows.len().saturating_sub(extents[1]));
        self.retain_ui_choices(old, extents[0], extents[1])?;
        self.owned_scroll = scroll;
        Ok(())
    }

    fn retain_ui_choices(&mut self, old: &Self, visible: usize, owned_visible: usize) -> Result<(), String> {
        // Resolve the financial roster slot, never the old roster position.
        let selected = old.player.players.get(old.player.selected)
            .and_then(|slot| self.player.players.iter().position(|p| p == slot))
            .or_else(|| self.player.players.iter().position(|p| Some(*p) == self.player.current))
            .unwrap_or(0);
        let rows = self.player.players.get(selected)
            .map(|&slot| crate::player_info::report(&self.player.records[slot], old.player.today))
            .transpose()?.unwrap_or_default();
        self.player.selected = selected;
        self.player.today = old.player.today;
        self.player.scroll = old.player.scroll.min(rows.len().saturating_sub(19));
        self.player.rows = rows;

        self.open = old.open;
        self.city_info = old.city_info && self.city_people.is_some();
        self.poll_info = old.poll_info && self.polls.is_some();
        self.player_info = old.player_info && !self.player.players.is_empty();
        self.people_mode = old.people_mode.min(3);
        self.ownership_mode = old.ownership_mode && self.ownership.is_some();
        self.rights_mode = old.rights_mode && self.rights.is_some();
        self.prestige_mode = old.prestige_mode && self.prestige.is_some();
        self.my_business = old.my_business;
        if self.commodity != old.commodity {
            std::mem::swap(&mut self.rows, &mut self.other_rows);
        }
        self.commodity = old.commodity;
        self.selected = old.selected.filter(|id| self.rows.iter().any(|r| r.kind == *id));
        self.other_selected = old.other_selected.filter(|id| self.other_rows.iter().any(|r| r.kind == *id));
        self.scroll = remap_scroll(&old.rows, old.scroll, &self.rows, visible, |r| r.kind);
        self.other_scroll = remap_scroll(&old.other_rows, old.other_scroll, &self.other_rows, visible, |r| r.kind);
        // Owned building indices belong to different scenes. Anchor only to the
        // preceding category header; do not copy a building or its numeric index.
        self.owned_scroll = old.owned_rows.iter().take(old.owned_scroll.saturating_add(1))
            .rposition(|r| r.label & 0x8000_0000 != 0 && r.label != u32::MAX)
            .and_then(|i| self.owned_rows.iter().position(|r| r.label == old.owned_rows[i].label && r.building.is_none()))
            .unwrap_or(0).min(self.owned_rows.len().saturating_sub(owned_visible));
        self.ownership_scroll = if self.prestige_mode {
            match (&old.prestige, &self.prestige) {
                (Some(a), Some(b)) => remap_scroll(&a.rows, old.ownership_scroll, &b.rows, 17, Clone::clone),
                _ => 0,
            }
        } else if self.ownership_mode {
            // Group header identity is roster slot; category identity is block ID.
            let keys = |report: &crate::city_ownership::Ownership| {
                report.groups.iter().enumerate().flat_map(|(slot, group)| {
                    std::iter::once((0, slot)).chain(if group.is_empty() {
                        vec![(1, slot)]
                    } else { group.iter().map(|&id| (2, id)).collect() }.into_iter())
                }).collect::<Vec<_>>()
            };
            match (&old.ownership, &self.ownership) {
                (Some(a), Some(b)) => remap_scroll(&keys(a), old.ownership_scroll, &keys(b), 17, |r| *r),
                _ => 0,
            }
        } else { 0 };
        self.pending_category = None;
        self.poll_category = None; // Recomputed from the remapped Inspection hit.
        self.pressed = None;
        self.hover = None;
        self.tutorial_activated = None;
        self.prepared = None;
        self.dirty = true;
        Ok(())
    }

    ///40d8f2 stores the descriptor tooltip ID at each of the four icon
    /// children +0x34. Use the same hit test as the visible icon controls.
    pub fn icon_tooltip_at(&self, assets: &Assets, point: [i32; 2]) -> Option<u32> {
        match assets.hit(self, point) {
            Action::City => Some(1146),
            Action::Player => Some(1147),
            Action::Toggle => Some(1148),
            Action::Poll => Some(12),
            _ => None,
        }
    }
    pub fn tutorial_control_at(&self, assets: &Assets, p: Option<[i32; 2]>) -> Option<u32> {
        p.and_then(|point| tutorial_control(assets.hit(self, point)))
    }
    pub fn tutorial_control_pressed(&self) -> bool {
        self.pressed.and_then(tutorial_control).is_some()
    }
    /// Original4138b0 walks eligible blocks then their linked buildings, keeping
    /// only businesses owned by the current manager (462ae0).
    pub fn refresh_scene(&mut self, scene: &Scene) {
        if self.strategic_state.is_some(){return;} //owned rows update in the synchronous native callback
        let mut rows = Vec::new();
        if let Some(current) = self.player.current {
            for category in 0..52 {
                if !self.owned_eligible[category] {
                    continue;
                }
                let mut group = Vec::new();
                for (index, building) in scene.inspection_buildings.iter().enumerate() {
                    if building.category != category {
                        continue;
                    }
                    let record = building
                        .business()
                        .map(|business| &business.record)
                        .or_else(|| {
                            building
                                .initialized_apartment
                                .as_ref()
                                .map(|business| &business.record)
                        });
                    if let Some(record) = record
                        && usize::from(record[0xa]) == current + 1
                    {
                        group.push(OwnedRow {
                            label: self
                                .catalog
                                .as_ref()
                                .and_then(|catalog| {
                                    catalog
                                        .rows()
                                        .get(u32::from_le_bytes(record[4..8].try_into().unwrap())
                                            as usize)
                                })
                                .map(|row| u32::from_le_bytes(row[8..12].try_into().unwrap()))
                                .unwrap_or(u32::MAX),
                            building: Some(index),
                        });
                    }
                }
                if !group.is_empty() {
                    rows.push(OwnedRow {
                        label: 0x8000_0000 | category as u32,
                        building: None,
                    });
                    rows.extend(group);
                    rows.push(OwnedRow {
                        label: u32::MAX,
                        building: None,
                    });
                }
            }
        }
        if rows.is_empty() {
            rows.push(OwnedRow {
                label: 1136,
                building: None,
            });
        }
        if self.owned_rows != rows {
            self.owned_rows = rows;
            self.owned_scroll = self
                .owned_scroll
                .min(self.owned_rows.len().saturating_sub(1));
            self.pressed = None;
            self.dirty = true;
        }
    }
    fn player_display_order(&self) -> Vec<usize> {
        let current = self.player.current.or_else(|| {
            self.rights
                .as_ref()
                .map(|r| usize::from(r.owner_number) - 1)
        });
        let mut active = vec![false; self.player.players.iter().max().map_or(0, |p| p + 1)];
        for &p in &self.player.players {
            active[p] = true;
        }
        crate::player_info::player_order(&active, current)
    }
    pub fn attach_rights(&mut self, rights: Option<crate::city_rights::Rights>) {
        self.rights = rights;
        self.dirty = true;
    }
    pub fn attach_players(&mut self, players: crate::player_info::State) {
        self.player = players;
        self.dirty = true;
    }
    pub fn apply_financial_snapshot(
        &mut self,
        snapshot: crate::player_info::Snapshot,
    ) -> Result<(), String> {
        if self.player.apply_snapshot(snapshot)? {
            self.dirty = true;
        }
        Ok(())
    }
    pub fn financial_records(&self) -> &[[u8; 440]] {
        &self.player.records
    }
    pub fn attach_prestige(&mut self, report: Option<crate::city_prestige::Prestige>) {
        self.prestige = report;
        self.dirty = true;
    }
    pub fn take_category_selection(&mut self) -> Option<usize> {
        self.pending_category.take()
    }
    pub fn attach_ownership(&mut self, report: Option<crate::city_ownership::Ownership>) {
        self.ownership = report;
        self.dirty = true;
    }
    pub fn attach_polls(&mut self, polls: Option<[crate::consumer_polls::Poll; 52]>) {
        self.polls = polls;
        self.dirty = true;
    }
    pub fn select_poll_category(&mut self, category: Option<usize>) {
        if self.poll_category != category {
            self.poll_category = category;
            self.dirty |= self.poll_info;
        }
    }
    pub fn pointer(&mut self, assets: &Assets, point: Option<[i32; 2]>) {
        let hover = point.map(|p| assets.hit(self, p)).filter(|a| {
            matches!(
                a,
                Action::City
                    | Action::Player
                    | Action::Toggle
                    | Action::Poll
                    | Action::CityMode(_)
                    | Action::PeopleStep(_)
                    | Action::Mode(_)
                    | Action::OwnedBusiness(_)
                    | Action::Scroll(_)
                    | Action::ScrollTrack
            )
        });
        if hover != self.hover {
            self.hover = hover;
            self.dirty = true;
        }
    }
    pub fn attach_people(&mut self, report: crate::city_people::Report) {
        self.city_people = Some(report);
        self.city_info = true;
        self.player_info = false;
        self.open = true;
        self.dirty = true;
    }
    pub fn apply_people_rows(&mut self, rows: Vec<crate::city_people::Row>) {
        if let Some(report) = &mut self.city_people
            && report.rows != rows
        {
            report.rows = rows;
            self.dirty = true;
        }
    }

    #[cfg(test)]
    fn matches(&self, scene: &Scene) -> Vec<usize> {
        scene
            .inspection_buildings
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                (b.business().is_some_and(|b| {
                    if self.commodity {
                        self.selected.is_some_and(|id| {
                            id < 32
                                && self
                                    .catalog
                                    .as_ref()
                                    .or(scene.saved_business_catalog.as_ref())
                                    .and_then(|c| c.rows().get(b.business_type as usize))
                                    .is_some_and(|r| {
                                        u32::from_le_bytes(r[0x70..0x74].try_into().unwrap())
                                            & (1 << id)
                                            != 0
                                    })
                        })
                    } else {
                        Some(b.business_type) == self.selected
                    }
                }))
                .then_some(i)
            })
            .collect()
    }
    pub fn scroll(&mut self, assets: &Assets, delta: i32) {
        if self.city_info && self.prestige_mode {
            let count = self.prestige.as_ref().map_or(0, |p| p.rows.len());
            self.ownership_scroll = self
                .ownership_scroll
                .saturating_add_signed(delta as isize)
                .min(count.saturating_sub(17));
            self.dirty = true;
            return;
        }
        if self.city_info && self.ownership_mode {
            let count = self.ownership.as_ref().map_or(0, |o| {
                o.groups.iter().map(|g| 1 + g.len().max(1)).sum::<usize>()
            });
            self.ownership_scroll = self
                .ownership_scroll
                .saturating_add_signed(delta as isize)
                .min(count.saturating_sub(17));
            self.dirty = true;
            return;
        }
        if self.city_info || self.poll_info {
            return;
        }
        if self.player_info {
            self.player.scroll = self
                .player
                .scroll
                .saturating_add_signed(delta as isize)
                .min(self.player.rows.len().saturating_sub(19));
            self.dirty = true;
            return;
        }
        if self.my_business {
            self.owned_scroll = self
                .owned_scroll
                .saturating_add_signed(delta as isize)
                .min(self.owned_rows.len().saturating_sub(assets.owned_visible()));
            self.dirty = true;
            return;
        }
        self.scroll = self
            .scroll
            .saturating_add_signed(delta as isize)
            .min(self.rows.len().saturating_sub(assets.visible()));
        self.dirty = true;
    }
    /// Match press/release target; disabled headers never activate or select the city underneath.
    pub fn mouse(
        &mut self,
        assets: &Assets,
        scene: &Scene,
        p: Option<[i32; 2]>,
        pressed: bool,
    ) -> Option<usize> {
        self.tutorial_activated = None;
        self.refresh_scene(scene);
        self.pointer(assets, p);
        let action = p.map(|p| assets.hit(self, p)).unwrap_or(Action::None);
        if pressed {
            self.dirty = true;
            self.pressed = Some(action);
            return None;
        }
        if self.pressed.take() != Some(action) {
            return None;
        }
        self.tutorial_activated = tutorial_control(action);
        match action {
            Action::OwnershipBlock(category) => {
                self.pending_category = Some(category);
            }
            Action::PeopleStep(right) => {
                self.people_mode = crate::people_markers::step(self.people_mode, right);
            }
            Action::CityMode(ownership) => {
                self.rights_mode = ownership == 2;
                self.ownership_mode = ownership == 3;
                self.prestige_mode = ownership == 1;
                self.ownership_scroll = 0;
            }
            Action::Poll => {
                self.open = !self.open || !self.poll_info;
                self.poll_info = true;
                self.city_info = false;
                self.player_info = false;
            }
            Action::Toggle => {
                self.open = !self.open || self.player_info || self.city_info || self.poll_info;
                self.poll_info = false;
                self.player_info = false;
                self.city_info = false;
            }
            Action::City => {
                self.poll_info = false;
                self.open = !self.open || !self.city_info;
                self.city_info = true;
                self.player_info = false;
            }
            Action::Player => {
                self.poll_info = false;
                self.open = !self.open || !self.player_info;
                self.player_info = true;
                self.city_info = false;
            }
            Action::PlayerSelect { player, .. } => {
                if let Some(position) = self.player.players.iter().position(|&p| p == player) {
                    self.player.selected = position;
                    if let Err(e) = self.player.refresh() {
                        eprintln!("Player report: {e}");
                        self.player.rows.clear();
                    }
                }
            }
            Action::PeriodStep(_) => {
                self.player.today = !self.player.today;
                if let Err(e) = self.player.refresh() {
                    eprintln!("Player report: {e}");
                    self.player.rows.clear();
                }
            }
            Action::OwnedBusiness(building) => {
                if self
                    .owned_rows
                    .iter()
                    .any(|row| row.building == Some(building))
                {
                    self.tutorial_activated = Some(6);
                    return Some(building);
                }
            }
            Action::Mode(index) => {
                self.my_business = index == 0;
                let commodity = index == 2;
                if index != 0 && self.commodity != commodity {
                    self.commodity = commodity;
                    std::mem::swap(&mut self.rows, &mut self.other_rows);
                    std::mem::swap(&mut self.selected, &mut self.other_selected);
                    std::mem::swap(&mut self.scroll, &mut self.other_scroll);
                }
            }
            Action::Select(kind) => {
                self.selected = Some(kind);
            }
            Action::Scroll(delta) => self.scroll(assets, delta),
            Action::ScrollTrack => {
                if let Some(point) = p {
                    self.scroll = ui::slider_selection(
                        ui::slider_track(assets.scrollbar, true),
                        point[1] as f32 * 0.5,
                        self.rows.len().saturating_sub(assets.visible()) as i32,
                        true,
                    ) as usize;
                }
            }
            Action::None => (),
        }
        self.dirty = true;
        None
    }
    pub fn cancel_press(&mut self) {
        self.dirty |= self.pressed.is_some() || self.hover.is_some();
        self.pressed = None;
        self.hover = None;
    }
    pub fn pressed(&self) -> bool {
        self.pressed.is_some()
    }
    pub fn take_tutorial_control(&mut self) -> Option<u32> {
        self.tutorial_activated.take()
    }
}
fn commodity_rows(
    rows: &[[u8; 28]; 32],
    order: &[u32; 32],
    memberships: [u32; 5],
) -> Result<Vec<Row>, String> {
    let mut seen = [false; 32];
    for &i in order {
        let s = seen
            .get_mut(i as usize)
            .ok_or("commodity order leaves catalog")?;
        if *s {
            return Err("duplicate commodity order".into());
        }
        *s = true;
    }
    let mut out = vec![];
    for (group, title) in [(1, 1526), (2, 1527)] {
        if !out.is_empty() {
            out.push(Row {
                label: u32::MAX,
                kind: u32::MAX,
            });
        }
        out.push(Row {
            label: title,
            kind: u32::MAX,
        });
        for &i in order {
            let row = &rows[i as usize];
            if row[0x10] == 1 && memberships[group] & (1 << i) != 0 {
                out.push(Row {
                    label: u32::from_le_bytes(row[4..8].try_into().unwrap()),
                    kind: i,
                });
            }
        }
    }
    Ok(out)
}
fn commodity_color(base: u32, mask: u32, selected: Option<u32>, tick: u32) -> u32 {
    if selected.is_some_and(|id| id < 32 && mask & (1 << id) != 0) && tick & 31 > 16 {
        0xffffffff
    } else {
        base
    }
}
fn owner_color(palette: &[u32; 10], character: Option<u32>) -> Result<u32, String> {
    character.map_or(Ok(0xff808080), |c| {
        palette
            .get(c as usize)
            .copied()
            .ok_or("owner character leaves palette".into())
    })
}
fn type_color(base: u32, kind: u32, selected: Option<u32>, tick: u32) -> u32 {
    if Some(kind) == selected && tick & 31 > 16 {
        0xffffffff
    } else {
        base
    }
}
fn footprint(bounds: [[f32; 3]; 8]) -> [[f32; 3]; 4] {
    [(0, 2., 2.), (1, -2., 2.), (7, -2., -2.), (6, 2., -2.)].map(|(i, x, z)| {
        [
            (f64::from(bounds[i][0]) + x) as f32,
            0.,
            (f64::from(bounds[i][2]) + z) as f32,
        ]
    })
}
fn inside(p: [f32; 2], b: [f32; 4]) -> bool {
    p[0] >= b[0] && p[0] < b[2] && p[1] >= b[1] && p[1] < b[3]
}
fn validate_order(order: &[u32; 45]) -> Result<(), String> {
    let mut seen = [false; 45];
    for &i in order {
        let v = seen
            .get_mut(i as usize)
            .ok_or("Locator order leaves catalog")?;
        if *v {
            return Err("duplicate Locator order".into());
        }
        *v = true;
    }
    Ok(())
}
fn rows(catalog: &BusinessCatalog, order: &[u32; 45]) -> Result<Vec<Row>, String> {
    validate_order(order)?;
    let entry = |i: usize| {
        let r = &catalog.rows()[i];
        Row {
            label: u32::from_le_bytes(r[4..8].try_into().unwrap()),
            kind: u32::from_le_bytes(r[..4].try_into().unwrap()),
        }
    };
    let mut out = vec![
        Row {
            label: 1140,
            kind: u32::MAX,
        },
        entry(42),
        entry(43),
    ];
    for (hour, title) in [(9, 1526), (18, 1527)] {
        out.push(Row {
            label: u32::MAX,
            kind: u32::MAX,
        });
        out.push(Row {
            label: title,
            kind: u32::MAX,
        });
        for &i in order {
            let r = &catalog.rows()[i as usize];
            if r[0x30] == 1 && r[0x5c] == hour {
                out.push(entry(i as usize));
            }
        }
    }
    Ok(out)
}
fn paint_rights_caption(
    pages: &mut [Surface],
    tab: &[u8],
    value: &[u8],
    bounds: [f32; 4],
    color: u32,
) -> Result<(), String> {
    let font = openmt2001::font::Font::parse(tab)?;
    let value = value.strip_suffix(&[0]).unwrap_or(value);
    let (offsets, text) =
        crate::prompt::wrap_with_gap(tab, value, (bounds[2] - bounds[0]) as i32, -1)?;
    // Native441220/4419f0: font0, scale1, gap-1, centered two rows;
    // 88x25 yields x offsets7/31, y4 then12 and line height8.
    for (line, range) in offsets.windows(2).enumerate() {
        let value = &text[range[0]..range[1].min(text.len())];
        let value = value.strip_suffix(&[0]).unwrap_or(value);
        let width = ui::text_width(tab, value, 1.)? as f32;
        for glyph in font.layout(
            value,
            openmt2001::font::Layout {
                //440736 adds0.5 beforeFTOL; the parent left is truncated
                //separately at44077b. A half-pixel center must round up here.
                origin: [
                    bounds[0].trunc() + ((bounds[2] - bounds[0] - width) * 0.5 + 0.5).trunc(),
                    bounds[1] + 4. + line as f32 * 8.,
                ],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            // Native4e7730 submits the internal space's degenerate quad too;
            // this caption has no per-glyph host clipping/scaling phase.
            pages[glyph.page].quad(glyph.positions.map(|p| [p[0], p[1]]), glyph.uv, [color; 4]);
        }
    }
    Ok(())
}

//430d8f..430e53 submits selected rows before4419f0, using the full list widget.
fn type_selection_bounds([l, t, r, _]: [f32; 4], row: usize) -> [f32; 4] {
    [
        l + 1.,
        t + row as f32 * 10. + 4.,
        r - 1.,
        t + row as f32 * 10. + 13.,
    ]
}
/// Original40e328/43ac10 and40e448; scroll width comes from texture105.
fn type_list_layout([l, t, r, b]: [f32; 4], scroll_width: f32) -> ([f32; 4], [f32; 4]) {
    use openmt2001::x87::Sum;
    let mut width = Sum::from_f32(r).unwrap();
    width.add_f32(-l).unwrap();
    width.add_f32(-scroll_width).unwrap();
    width.add_f32(-4.).unwrap();
    let mut top = Sum::from_f32(t).unwrap();
    top.add_f32(40.).unwrap();
    let top = top.to_f32();
    let mut height = Sum::from_f32(b).unwrap();
    height.add_f32(-70.).unwrap();
    let height = height.to_f32();
    let mut right = Sum::from_f32(l).unwrap();
    right.add_f32(width.to_f32()).unwrap();
    let mut bottom = Sum::from_f32(top).unwrap();
    bottom.add_f32(height).unwrap();
    let mut y = Sum::from_f32(t).unwrap();
    y.add_f32(42.).unwrap();
    let mut length = Sum::from_f32(b).unwrap();
    length.add(y.negated());
    let length = length.ftol_low_word();
    let y = y.ftol_low_word();
    let mut x = Sum::from_f32(r).unwrap();
    x.add_f32(-scroll_width).unwrap();
    let x = x.ftol_low_word();
    (
        [l, top, right.to_f32(), bottom.to_f32()],
        [x as f32, y as f32, (x + 16) as f32, (y + length) as f32],
    )
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original Locator list and vertical scrollbar fixture"]
    fn matches_native_locator_list_and_scrollbar() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_LOCATOR_LIST").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTLOCL01");
        let mut at = 8;
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        assert_eq!(word(&mut at), 0x037f);
        let count = word(&mut at);
        let mut native_track = [0.; 4];
        for _ in 0..count {
            let data: [f32; 21] = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let (list, scroll) = super::type_list_layout(data[..4].try_into().unwrap(), data[4]);
            let track = crate::ui::slider_track(scroll, true);
            native_track = track;
            let arrows = crate::ui::slider_arrows(scroll, true);
            let actual = list
                .into_iter()
                .chain(track)
                .chain(arrows.into_iter().flatten())
                .map(f32::to_bits)
                .collect::<Vec<_>>();
            assert_eq!(
                actual,
                data[5..].iter().map(|v| v.to_bits()).collect::<Vec<_>>()
            );
        }
        for _ in 0..word(&mut at) {
            let maximum = word(&mut at) as i32;
            let value = word(&mut at) as i32;
            let disabled = word(&mut at) != 0;
            let handle = crate::ui::slider_handle(native_track, value, maximum, true);
            for value in handle {
                assert_eq!(value.to_bits(), word(&mut at));
            }
            let enabled = maximum > 0 && !disabled;
            for _ in 0..4 {
                assert_eq!(word(&mut at), if enabled { 0xff1067cf } else { 0xcc676767 });
            }
            let colors = if enabled {
                [0xff90c0e0, 0xff104790]
            } else {
                [0xb4505050, 0xb4909090]
            };
            for (edge, color) in crate::ui::slider_edges(handle).into_iter().zip(colors) {
                for [x, y] in edge {
                    assert_eq!(x.to_bits(), word(&mut at));
                    assert_eq!(y.to_bits(), word(&mut at));
                    assert_eq!(0.5f32.to_bits(), word(&mut at));
                    assert_eq!(color, word(&mut at));
                }
            }
        }
        for _ in 0..word(&mut at) {
            let maximum = word(&mut at) as i32;
            let point = word(&mut at) as f32;
            let value = word(&mut at) as i32;
            assert_eq!(
                crate::ui::slider_selection(native_track, point, maximum, true),
                value
            );
        }
        for _ in 0..word(&mut at) {
            let side = word(&mut at) as usize;
            for (pressed, hovered, enabled) in [
                (false, false, true),
                (false, true, true),
                (true, true, true),
                (false, false, false),
                (true, false, true),
            ] {
                assert_eq!(
                    crate::ui::slider_arrow_texture(side, pressed, hovered, enabled, true) as u32,
                    word(&mut at)
                );
            }
        }
        assert_eq!(at, bytes.len());
        let native = std::fs::read(
            std::path::Path::new(&std::env::var("OPENMT2001_NATIVE_LOCATOR_LIST").unwrap())
                .with_file_name("native-locator-selection.bin"),
        )
        .unwrap();
        assert_eq!(&native[..8], b"MTLOCSEL");
        let mut at = 8;
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(native[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        assert_eq!(word(&mut at), 0x037f);
        let list: [f32; 4] = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
        for _ in 0..word(&mut at) {
            let selection = word(&mut at) as i32;
            let count = word(&mut at);
            if selection < 0 {
                assert_eq!(count, 0);
                continue;
            }
            assert_eq!(count, 1);
            for value in super::type_selection_bounds(list, selection as usize) {
                assert_eq!(value.to_bits(), word(&mut at));
            }
            assert_eq!(word(&mut at), 0.5f32.to_bits());
            assert_eq!(word(&mut at), 0xff288be1);
        }
        assert_eq!(at, native.len());
    }

    use super::*;
    #[test]
    #[ignore = "requires original commodity Locator oracle"]
    fn matches_native_locator_commodities() {
        let bytes =
            fs::read(std::env::var("OPENMT2001_NATIVE_LOCATOR_COMMODITIES").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTLCM001");
        let mut at = 8;
        let rows = std::array::from_fn(|_| {
            let row = bytes[at..at + 28].try_into().unwrap();
            at += 28;
            row
        });
        let mut word = || {
            let v = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            at += 4;
            v
        };
        let order = std::array::from_fn(|_| word());
        let memberships = std::array::from_fn(|_| word());
        let count = word();
        let expected = (0..count)
            .map(|_| Row {
                label: word(),
                kind: word(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            commodity_rows(&rows, &order, memberships).unwrap(),
            expected
        );
        let count = word();
        for _ in 0..count {
            let mask = word();
            let offers = word();
            let id = word();
            let blink = word();
            let color = word();
            assert_eq!(mask.count_ones(), offers);
            assert_eq!(
                commodity_color(0xff123456, mask, Some(id), if blink != 0 { 17 } else { 0 }),
                color
            );
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original Locator owner and ground quad oracle"]
    fn matches_native_locator_overlay() {
        let bytes = fs::read(std::env::var("OPENMT2001_NATIVE_LOCATOR_OVERLAY").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTLOV001");
        let mut at = 8;
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let palette = std::array::from_fn(|_| {
            let v = word(at);
            at += 4;
            v
        });
        let count = word(at);
        at += 4;
        for _ in 0..count {
            let character = word(at);
            let expected = word(at + 4);
            at += 8;
            assert_eq!(
                owner_color(&palette, (character != u32::MAX).then_some(character)).unwrap(),
                expected
            );
        }
        let count = word(at);
        at += 4;
        for _ in 0..count {
            let bounds = std::array::from_fn(|_| {
                std::array::from_fn(|_| {
                    let f = f32::from_bits(word(at));
                    at += 4;
                    f
                })
            });
            let expected = &bytes[at..at + 48];
            at += 48;
            assert_eq!(
                footprint(bounds)
                    .into_iter()
                    .flatten()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert_eq!(at, bytes.len());
        assert!(owner_color(&palette, Some(10)).is_err());
    }
    #[test]
    #[ignore = "requires original save, scene, font and language assets"]
    fn locator_saved_inspection_flow() {
        use openmt2001::render_scene::{OriginalAssets, Request, Source, assemble};
        let read = |name| fs::read(std::env::var(name).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let off = fs::read(language.join("tycoon.off")).unwrap();
        let bin = fs::read(language.join("tycoon.bin")).unwrap();
        let mut offset = 0;
        let strings = off
            .chunks_exact(2)
            .map(|n| {
                let len = u16::from_le_bytes(n.try_into().unwrap()) as usize;
                let text = &bin[offset..offset + len];
                offset += len;
                text
            })
            .collect::<Vec<_>>();
        let sun = std::path::PathBuf::from(std::env::var("OPENMT2001_SUNCOLS").unwrap());
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(
            &index,
            &archive,
            sun.parent().unwrap().parent().unwrap(),
            &strings,
            &exe,
        )
        .unwrap();
        let mouse_center = |b: [f32; 4]| [(b[0] + b[2]) as i32, (b[1] + b[3]) as i32];
        let city_points = assets.city_layout.buttons.map(mouse_center);
        let people_points =
            [false, true].map(|right| mouse_center(assets.people_arrow_bounds(right)));
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(target_os = "windows") {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let adjustment = crate::business_adjustment::Assets::load(
            &index,
            &archive,
            sun.parent().unwrap().parent().unwrap(),
            &strings,
            &exe,
        )
        .unwrap();
        let prompts = crate::prompt::Assets::load(
            &index,
            &archive,
            sun.parent().unwrap().parent().unwrap(),
            &strings,
            &openmt2001::pe::PeImage::parse(&exe).unwrap(),
        )
        .unwrap();
        for name in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let save = read(name);
            let mut scene = assemble(Request {
                build_file: &build,
                source: Source::Save(&save),
                original: Some(OriginalAssets {
                    executable: &exe,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: None,
                phase: None,
                camera: Some(
                    crate::navigation::Navigation::load(&index, &archive, &exe)
                        .unwrap()
                        .camera,
                ),
                width: 1280,
                height: 960,
                animated: false,
            })
            .unwrap();
            let original = openmt2001::save::SaveFile::parse(&save).unwrap();
            let report = crate::block_information::Report::from_save(
                &original,
                &PeImage::parse(&exe).unwrap(),
            )
            .unwrap()
            .unwrap();
            let board = openmt2001::board::Board::from_v14(&PeImage::parse(&exe).unwrap()).unwrap();
            for block in &board.blocks {
                if !report.available(block.id) {
                    continue;
                }
                let (_panel, bounds) = assets.block_panel(&renderer, &report, block.id).unwrap();
                assert!(bounds[2] < 200.);
                if report.categories[block.id].is_some() {
                    let mut view = crate::utility_report::View::default();
                    view.select(Some(block.id));
                    let [l, t, r, b] = assets.city_layout.buttons[1];
                    assets.utility_pointer(&mut view, Some([(l + r) as i32, (t + b) as i32]));
                    assert!(view.mouse(true));
                    assert!(view.mouse(false));
                    assert!(view.details);
                    let (_land, _) = assets
                        .property_panel(&renderer, &report, block.id, &view)
                        .unwrap();
                    for (row_index, row) in report.land_rent[block.id].iter().enumerate() {
                        if let crate::block_information::land_rent::Row::Business(b) = row {
                            view.scroll = row_index.saturating_sub(21);
                            let y = (assets.bounds[1]
                                + 43.
                                + ((row_index - view.scroll) as f32 + 0.5) * 10.)
                                * 2.;
                            assets.utility_pointer(&mut view, None);
                            assets.land_rent_pointer(
                                &mut view,
                                Some([100, y as i32]),
                                &report,
                                block.id,
                            );
                            assert_eq!(view.row_hover, Some(b.slot));
                            assert!(view.mouse(true));
                            assert!(view.mouse(false));
                            assert_eq!(view.requested_slot.take(), Some(b.slot));
                            assert!(
                                scene
                                    .inspection_buildings
                                    .iter()
                                    .any(|building| building.category == block.id
                                        && building.saved_slot == Some(b.slot))
                            );
                        }
                    }
                    #[cfg(feature = "capture")]
                    if block.name.to_lowercase().contains("james")
                        && let Ok(prefix) = std::env::var("OPENMT2001_LAND_RENT_CAPTURE")
                    {
                        capture(
                            &renderer,
                            &_land,
                            &scene,
                            &format!(
                                "{prefix}-{}.png",
                                if name == "OPENMT2001_SAVE_ONE" {
                                    "one"
                                } else {
                                    "two"
                                }
                            ),
                        );
                    }
                }

                if let Some(utility) = report
                    .utility_categories
                    .iter()
                    .position(|&id| id == block.id)
                {
                    let mut view = crate::utility_report::View::default();
                    view.select(Some(block.id));
                    let [l, t, r, b] = assets.city_layout.buttons[1];
                    assets.utility_pointer(&mut view, Some([(l + r) as i32, (t + b) as i32]));
                    assert!(view.mouse(true));
                    assert!(view.mouse(false));
                    assert!(view.details);
                    assert_eq!(view.title(Some(utility)), 1806 + utility);
                    let (bills, _) = assets
                        .property_panel(&renderer, &report, block.id, &view)
                        .unwrap();
                    #[cfg(feature = "capture")]
                    if let Ok(prefix) = std::env::var("OPENMT2001_UTILITY_REPORT_CAPTURE") {
                        capture(
                            &renderer,
                            &bills,
                            &scene,
                            &format!(
                                "{prefix}-{}-{}-bills.png",
                                if name == "OPENMT2001_SAVE_ONE" {
                                    "one"
                                } else {
                                    "two"
                                },
                                block.id
                            ),
                        );
                    }
                    let [l, t, r, b] = assets.city_layout.buttons[0];
                    assets.utility_pointer(&mut view, Some([(l + r) as i32, (t + b) as i32]));
                    assert!(view.mouse(true));
                    assert!(view.mouse(false));
                    assert!(!view.details);
                    drop(bills);
                }
                #[cfg(feature = "capture")]
                if report.utility_categories.contains(&block.id)
                    && let Ok(prefix) = std::env::var("OPENMT2001_UTILITY_REPORT_CAPTURE")
                {
                    capture(
                        &renderer,
                        &_panel,
                        &scene,
                        &format!(
                            "{prefix}-{}-{}.png",
                            if name == "OPENMT2001_SAVE_ONE" {
                                "one"
                            } else {
                                "two"
                            },
                            block.id
                        ),
                    );
                }

                #[cfg(feature = "capture")]
                if block.name.to_lowercase().contains("pacific")
                    && let Ok(prefix) = std::env::var("OPENMT2001_BLOCK_INFORMATION_CAPTURE")
                {
                    // This is the settled original mode12 property pose, distinct
                    // from the strategic planning camera used by the report suite.
                    let property_id = board
                        .blocks
                        .iter()
                        .find(|b| b.name.to_lowercase().contains("james"))
                        .unwrap()
                        .id;
                    let (property_panel, _) =
                        assets.block_panel(&renderer, &report, property_id).unwrap();
                    let center = assets.category_centers[property_id];
                    let mut controls = openmt2001::camera::BlockCamera {
                        center: [center[0], 0., center[1]],
                        angle: -std::f32::consts::FRAC_PI_2,
                        height: 120.,
                        radius: 80.,
                    };
                    let (eye, target) = controls.tick([false; 4]);
                    let property = assemble(Request {
                        build_file: &build,
                        source: Source::Save(&save),
                        original: Some(OriginalAssets {
                            executable: &exe,
                            directory: &directory,
                            archive: &archive,
                        }),
                        suncols: None,
                        phase: None,
                        camera: Some(openmt2001::flyby::CameraSample {
                            eye,
                            target,
                            up: [0., 1., 0.],
                            vertical_fov: 45.,
                        }),
                        width: 1280,
                        height: 960,
                        animated: false,
                    })
                    .unwrap();
                    capture(
                        &renderer,
                        &property_panel,
                        &property,
                        &format!(
                            "{prefix}-property-{}.png",
                            if name == "OPENMT2001_SAVE_ONE" {
                                "one"
                            } else {
                                "two"
                            }
                        ),
                    );
                    capture(
                        &renderer,
                        &_panel,
                        &scene,
                        &format!(
                            "{prefix}-{}.png",
                            if name == "OPENMT2001_SAVE_ONE" {
                                "one"
                            } else {
                                "two"
                            }
                        ),
                    );
                    if name == "OPENMT2001_SAVE_ONE" {
                        let mut diagnostic = report.clone();
                        diagnostic.categories[block.id] = Some(
                            crate::block_information::rows(2.995, 0.005, 1, 1, 1, 120).unwrap(),
                        );
                        let (panel, _) = assets
                            .block_panel(&renderer, &diagnostic, block.id)
                            .unwrap();
                        capture(
                            &renderer,
                            &panel,
                            &scene,
                            &format!("{prefix}-diagnostic.png"),
                        );
                    }
                }
            }
            let economy = original.economy().unwrap();
            let businesses = economy.businesses().collect::<Vec<_>>();
            let saved_buildings = original.buildings().unwrap();
            let players = original.players().unwrap();
            for building in &scene.inspection_buildings {
                let owner = saved_buildings
                    .iter()
                    .find(|b| Some(b.slot) == building.saved_slot)
                    .unwrap()
                    .bytes[0x2d];
                let expected = if owner == 0 {
                    None
                } else {
                    Some(u32::from_le_bytes(
                        players[usize::from(owner) - 1].bytes[0x14c..0x150]
                            .try_into()
                            .unwrap(),
                    ))
                };
                assert_eq!(building.saved_owner_character, expected);
            }
            for building in &scene.inspection_buildings {
                if let Some(b) = building.business() {
                    assert_eq!(b.business_type, businesses[b.slot].business_type());
                }
            }
            let mut state = assets.state(&scene).unwrap().unwrap();
            // The production host attaches the retained manager snapshot;
            // this saved-scene test supplies the original load selection.
            state.player.current = crate::saved_current_player::normal_load(&original)
                .unwrap()
                .map(|current| current.slot);
            state.attach_people(
                crate::city_people::Report::from_save(&original, &PeImage::parse(&exe).unwrap())
                    .unwrap(),
            );
            assert!(state.city_info && state.open);
            state.attach_prestige(Some(
                crate::city_prestige::Prestige::from_save(
                    &original,
                    &openmt2001::board::Board::from_v14(&PeImage::parse(&exe).unwrap()).unwrap(),
                )
                .unwrap(),
            ));
            state.attach_ownership(Some(
                crate::city_ownership::Ownership::from_save(&original).unwrap(),
            ));
            state.attach_rights(crate::city_rights::Rights::from_save(&original).unwrap());
            state.mouse(&assets, &scene, Some(city_points[3]), true);
            state.mouse(&assets, &scene, Some(city_points[3]), false);
            assert!(state.ownership_mode);
            let ownership = assets.prepare(&renderer, &state, &scene).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_CITY_OWNERSHIP_CAPTURE") {
                capture(
                    &renderer,
                    &ownership,
                    &scene,
                    &format!(
                        "{prefix}-{}.png",
                        if name == "OPENMT2001_SAVE_ONE" {
                            "one"
                        } else {
                            "two"
                        }
                    ),
                );
            }
            let _ = ownership;
            state.mouse(&assets, &scene, Some(city_points[0]), true);
            state.mouse(&assets, &scene, Some(city_points[0]), false);
            assert!(!state.ownership_mode);
            let actual = state.ownership.take();
            state.ownership = Some(crate::city_ownership::Ownership {
                badges: actual.as_ref().unwrap().badges,
                groups: vec![(0..30).collect(), vec![]],
            });
            state.ownership_mode = true;
            state.mouse(&assets, &scene, Some([150, 170]), true);
            state.mouse(&assets, &scene, Some([150, 170]), false);
            assert_eq!(state.take_category_selection(), Some(0));
            state.scroll(&assets, 20);
            assert_eq!(state.ownership_scroll, 16);
            if let Ok(path) = std::env::var("OPENMT2001_OWNERSHIP_SYNTHETIC") {
                let bytes = std::fs::read(path).unwrap();
                let diagnostic = openmt2001::save::SaveFile::parse(&bytes).unwrap();
                let old = std::mem::replace(
                    &mut scene.saved_players,
                    diagnostic
                        .players()
                        .unwrap()
                        .iter()
                        .map(|p| *p.bytes)
                        .collect(),
                );
                let old_player = std::mem::replace(
                    &mut state.player,
                    crate::player_info::State::new(&scene.saved_players).unwrap(),
                );
                state.ownership =
                    Some(crate::city_ownership::Ownership::from_save(&diagnostic).unwrap());
                state.ownership_scroll = 0;
                // Diagnostic palette exposure: actual saved badges remain unchanged.
                for (i, id) in state
                    .prestige
                    .as_ref()
                    .unwrap()
                    .overlay_categories
                    .iter()
                    .take(10)
                    .enumerate()
                {
                    state.ownership.as_mut().unwrap().badges[*id].character = Some(i);
                    state.ownership.as_mut().unwrap().badges[*id].remaining =
                        [1, 9, 10, 99, 100, 1000, 4, 6, 25, 50][i];
                }
                state.select_poll_category(Some(8));
                let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_CITY_OWNERSHIP_CAPTURE") {
                    capture(
                        &renderer,
                        &prepared,
                        &scene,
                        &format!("{prefix}-diagnostic.png"),
                    );
                }
                let _ = prepared;
                state.player = old_player;
                scene.saved_players = old;
            }
            state.ownership = actual;
            state.ownership_mode = false;
            state.ownership_scroll = 0;
            state.attach_prestige(Some(
                crate::city_prestige::Prestige::from_save(
                    &original,
                    &openmt2001::board::Board::from_v14(&PeImage::parse(&exe).unwrap()).unwrap(),
                )
                .unwrap(),
            ));
            state.mouse(&assets, &scene, Some(city_points[2]), true);
            assert!(!state.rights_mode);
            state.mouse(&assets, &scene, Some(city_points[2]), false);
            assert!(state.rights_mode && !state.ownership_mode && !state.prestige_mode);
            let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_CITY_RIGHTS_CAPTURE") {
                capture(
                    &renderer,
                    &prepared,
                    &scene,
                    &format!(
                        "{prefix}-{}.png",
                        if name == "OPENMT2001_SAVE_ONE" {
                            "one"
                        } else {
                            "two"
                        }
                    ),
                );
            }
            // Exercise owned/other-owner locked and timed rights in an explicitly diagnostic view.
            if name == "OPENMT2001_SAVE_ONE" {
                let saved = state.rights.as_ref().unwrap().categories;
                for (i, c) in state
                    .rights
                    .as_mut()
                    .unwrap()
                    .categories
                    .iter_mut()
                    .enumerate()
                {
                    c.mode = if i % 3 == 0 {
                        2
                    } else if i % 3 == 1 {
                        0
                    } else {
                        1
                    };
                    c.owner = if i % 2 == 0 { 1 } else { 2 };
                    c.remaining = [9, 10, 100][i % 3];
                }
                let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_CITY_RIGHTS_CAPTURE") {
                    capture(
                        &renderer,
                        &prepared,
                        &scene,
                        &format!("{prefix}-diagnostic.png"),
                    );
                }
                let _ = prepared;
                state.rights.as_mut().unwrap().categories = saved;
            }
            let _ = prepared;
            state.mouse(&assets, &scene, Some(city_points[1]), true);
            state.mouse(&assets, &scene, Some(city_points[1]), false);
            assert!(state.prestige_mode);
            state.mouse(&assets, &scene, Some([150, 170]), true);
            state.mouse(&assets, &scene, Some([150, 170]), false);
            assert_eq!(state.take_category_selection(), Some(29));
            for bottom in [false, true] {
                if bottom {
                    state.scroll(&assets, 100);
                }
                let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_CITY_PRESTIGE_CAPTURE") {
                    capture(
                        &renderer,
                        &prepared,
                        &scene,
                        &format!(
                            "{prefix}-{}-{}.png",
                            if name == "OPENMT2001_SAVE_ONE" {
                                "one"
                            } else {
                                "two"
                            },
                            if bottom { "bottom" } else { "top" }
                        ),
                    );
                }
                let _ = prepared;
            }
            state.mouse(&assets, &scene, Some(city_points[0]), true);
            state.mouse(&assets, &scene, Some(city_points[0]), false);
            assert!(!state.prestige_mode && !state.ownership_mode);

            assert!(
                assets
                    .footprints(&state, &scene)
                    .unwrap()
                    .vertices
                    .is_empty()
            );
            for mode in 0..4 {
                assert_eq!(state.people_mode, mode);
                let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_PEOPLE_MARKER_CAPTURE") {
                    capture(
                        &renderer,
                        &prepared,
                        &scene,
                        &format!(
                            "{prefix}-{}-{mode}.png",
                            if name == "OPENMT2001_SAVE_ONE" {
                                "one"
                            } else {
                                "two"
                            }
                        ),
                    );
                }
                let _ = prepared;
                state.mouse(&assets, &scene, Some(people_points[1]), true);
                assert_eq!(state.people_mode, mode);
                assert!(matches!(state.hover, Some(Action::PeopleStep(true))));
                state.mouse(&assets, &scene, Some(people_points[1]), false);
            }
            assert_eq!(state.people_mode, 0);
            state.mouse(&assets, &scene, Some(people_points[0]), true);
            state.mouse(&assets, &scene, Some(people_points[0]), false);
            assert_eq!(state.people_mode, 3);
            state.mouse(&assets, &scene, Some(people_points[1]), true);
            state.mouse(&assets, &scene, Some(people_points[1]), false);
            assert_eq!(state.people_mode, 0);
            let people = assets.prepare(&renderer, &state, &scene).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_CITY_PEOPLE_CAPTURE") {
                capture(
                    &renderer,
                    &people,
                    &scene,
                    &format!(
                        "{prefix}-{}.png",
                        if name == "OPENMT2001_SAVE_ONE" {
                            "one"
                        } else {
                            "two"
                        }
                    ),
                );
            }
            let _ = people;
            // The City Information icon is the original P_MAIN_ICON2 region.
            for expected in [false, true] {
                state.mouse(&assets, &scene, Some([32, 130]), true);
                state.mouse(&assets, &scene, Some([32, 130]), false);
                assert_eq!(state.open, expected);
                assert!(state.city_info);
            }
            state.mouse(&assets, &scene, Some([32, 178]), true);
            state.mouse(&assets, &scene, Some([32, 178]), false);
            assert!(state.player_info && !state.city_info && state.open);
            state.mouse(&assets, &scene, Some([32, 130]), true);
            state.mouse(&assets, &scene, Some([32, 130]), false);
            assert!(state.city_info && !state.player_info && state.open);
            state.attach_polls(Some(
                crate::consumer_polls::Poll::from_save(&original).unwrap(),
            ));
            state.mouse(&assets, &scene, Some([32, 274]), true);
            state.mouse(&assets, &scene, Some([32, 274]), false);
            assert!(state.poll_info && state.open && !state.city_info);
            for category in [None, Some(8), Some(0)] {
                state.select_poll_category(category);
                let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
                assert!(
                    assets
                        .footprints(&state, &scene)
                        .unwrap()
                        .vertices
                        .is_empty()
                );
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_CONSUMER_POLLS_CAPTURE") {
                    capture(
                        &renderer,
                        &prepared,
                        &scene,
                        &format!(
                            "{prefix}-{}-{}.png",
                            if name == "OPENMT2001_SAVE_ONE" {
                                "one"
                            } else {
                                "two"
                            },
                            category.map_or("none".into(), |i| i.to_string())
                        ),
                    );
                }
                let _ = prepared;
            }
            // Host coordinates are twice original UI coordinates; press/release must agree.
            state.mouse(&assets, &scene, Some([32, 224]), true);
            state.mouse(&assets, &scene, Some([32, 224]), false);
            assert!(state.open);
            state.refresh_scene(&scene);
            assert!(state.my_business);
            let oracle =
                fs::read(std::env::var("OPENMT2001_NATIVE_OWNED_LOCATOR").unwrap()).unwrap();
            assert_eq!(&oracle[..8], b"MTOWNLOC");
            let mut at = 8;
            let word = |at: &mut usize| {
                let value = u32::from_le_bytes(oracle[*at..*at + 4].try_into().unwrap());
                *at += 4;
                value
            };
            let mut compared = false;
            for _ in 0..word(&mut at) {
                let length = word(&mut at) as usize;
                let filename = &oracle[at..at + length];
                at += length;
                let current = word(&mut at) as usize;
                let expected = (0..word(&mut at))
                    .map(|_| (word(&mut at), word(&mut at)))
                    .collect::<Vec<_>>();
                if filename
                    != std::path::Path::new(&std::env::var(name).unwrap())
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .as_bytes()
                    || Some(current) != state.player.current
                {
                    continue;
                }
                let actual = state
                    .owned_rows
                    .iter()
                    .map(|row| {
                        let label = if row.label != u32::MAX && row.label & 0x8000_0000 != 0 {
                            board
                                .blocks
                                .iter()
                                .find(|b| b.id == (row.label & 0x7fff_ffff) as usize)
                                .unwrap()
                                .text_id
                        } else {
                            row.label
                        };
                        (
                            label,
                            row.building.map_or(u32::MAX, |building| {
                                scene.inspection_buildings[building].saved_slot.unwrap() as u32
                            }),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "native4138b0 {name}");
                compared = true;
            }
            assert!(
                compared,
                "supplied save/current manager is present in original oracle"
            );

            for (row, entry) in state.owned_rows.clone().iter().enumerate() {
                let Some(building) = entry.building else {
                    continue;
                };
                state.owned_scroll = row;
                let point = [
                    (assets.list[0] * 2. + 6.) as i32,
                    (assets.owned_list()[1] * 2. + 8.) as i32,
                ];
                assert_eq!(state.tutorial_control_at(&assets, Some(point)), Some(30));
                state.mouse(&assets, &scene, Some(point), true);
                assert_eq!(state.mouse(&assets, &scene, None, false), None);
                assert_eq!(state.take_tutorial_control(), None);
                state.mouse(&assets, &scene, Some(point), true);
                assert_eq!(
                    state.mouse(&assets, &scene, Some(point), false),
                    Some(building)
                );
                assert_eq!(state.take_tutorial_control(), Some(6));
                let record = &scene.inspection_buildings[building]
                    .business()
                    .unwrap()
                    .record;
                assert_eq!(usize::from(record[0xa]), state.player.current.unwrap() + 1);
            }
            state.owned_scroll = 0;
            let owned_panel = assets.prepare(&renderer, &state, &scene).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE") {
                capture(
                    &renderer,
                    &owned_panel,
                    &scene,
                    &format!(
                        "{prefix}-owned-{}.png",
                        if name == "OPENMT2001_SAVE_ONE" {
                            "one"
                        } else {
                            "two"
                        }
                    ),
                );
            }
            let _ = owned_panel;
            state.mouse(&assets, &scene, Some(city_points[1]), true);
            state.mouse(&assets, &scene, Some(city_points[1]), false);
            assert!(!state.my_business && !state.commodity);
            let header = [
                (assets.list[0] * 2. + 6.) as i32,
                (assets.list[1] * 2. + 4.) as i32,
            ];
            state.mouse(&assets, &scene, Some(header), true);
            state.mouse(&assets, &scene, Some(header), false);
            assert_eq!(state.selected, None);
            let apartment = [header[0], header[1] + 24];
            state.mouse(&assets, &scene, Some(apartment), true);
            state.mouse(&assets, &scene, Some(apartment), false);
            assert_eq!(state.selected, Some(42));
            let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE") {
                capture(
                    &renderer,
                    &prepared,
                    &scene,
                    &format!(
                        "{prefix}-{}.png",
                        if name == "OPENMT2001_SAVE_ONE" {
                            "one"
                        } else {
                            "two"
                        }
                    ),
                );
            }
            #[cfg(not(feature = "capture"))]
            let _ = prepared;
            let entries = state
                .rows
                .iter()
                .filter(|r| r.kind != u32::MAX)
                .map(|r| r.kind)
                .collect::<Vec<_>>();
            let mut nonempty = 0;
            for kind in entries {
                state.selected = Some(kind);
                let expected = scene
                    .inspection_buildings
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| {
                        b.business().filter(|b| b.business_type == kind).map(|_| i)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(state.matches(&scene), expected);
                if !expected.is_empty() {
                    nonempty += 1;
                }
            }
            if name == "OPENMT2001_SAVE_ONE" {
                assert!(nonempty >= 2);
            }
            state.scroll(&assets, 1000);
            assert_eq!(state.scroll, state.rows.len() - assets.visible());
            assets.prepare(&renderer, &state, &scene).unwrap();
            state.mouse(&assets, &scene, Some([32, 224]), true);
            state.cancel_press();
            state.mouse(&assets, &scene, Some([32, 224]), false);
            assert!(state.open);
            state.mouse(&assets, &scene, Some(city_points[2]), true);
            state.mouse(&assets, &scene, Some(city_points[2]), false);
            assert!(state.commodity);
            assert_eq!(state.selected, None);
            let mut commodity_matches = 0;
            let entries = state
                .rows
                .iter()
                .filter(|r| r.kind != u32::MAX)
                .map(|r| r.kind)
                .collect::<Vec<_>>();
            for id in entries {
                state.selected = Some(id);
                let expected = scene
                    .inspection_buildings
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| {
                        b.business()
                            .filter(|b| {
                                economy
                                    .commodity_mask(b.business_type)
                                    .is_some_and(|m| m & (1 << id) != 0)
                            })
                            .map(|_| i)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(state.matches(&scene), expected);
                if !expected.is_empty() {
                    commodity_matches += 1;
                }
            }
            // Actual row hit selects a saved offered commodity (or the first available on an empty city).
            let id = scene
                .inspection_buildings
                .iter()
                .filter_map(|b| b.business())
                .filter_map(|b| economy.commodity_mask(b.business_type))
                .find(|m| *m != 0)
                .map(|m| m.trailing_zeros())
                .unwrap_or(state.rows[1].kind);
            let row = state.rows.iter().position(|r| r.kind == id).unwrap();
            state.scroll = row.saturating_sub(3);
            let point = [
                (assets.list[0] * 2. + 6.) as i32,
                (assets.list[1] * 2. + (row - state.scroll) as f32 * 20. + 8.) as i32,
            ];
            state.mouse(&assets, &scene, Some(point), true);
            state.mouse(&assets, &scene, Some(point), false);
            assert_eq!(state.selected, Some(id));
            let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE") {
                capture(
                    &renderer,
                    &prepared,
                    &scene,
                    &format!(
                        "{prefix}-commodity-{}.png",
                        if name == "OPENMT2001_SAVE_ONE" {
                            "one"
                        } else {
                            "two"
                        }
                    ),
                );
            }
            #[cfg(not(feature = "capture"))]
            let _ = prepared;
            let selected = state.selected;
            state.mouse(&assets, &scene, Some(city_points[1]), true);
            state.mouse(&assets, &scene, Some(city_points[1]), false);
            assert!(!state.commodity);
            state.mouse(&assets, &scene, Some(city_points[2]), true);
            state.mouse(&assets, &scene, Some(city_points[2]), false);
            assert_eq!(state.selected, selected);
            state.mouse(&assets, &scene, Some([32, 176]), true);
            state.mouse(&assets, &scene, Some([32, 176]), false);
            assert!(state.player_info && state.open);
            assert!(state.player.today);
            assert_eq!(
                state.player.players.len(),
                scene.saved_players.iter().filter(|p| p[0xc] == 1).count()
            );
            let prepared = assets.prepare(&renderer, &state, &scene).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_PLAYER_REPORT_CAPTURE")
                .or_else(|_| std::env::var("OPENMT2001_LOCATOR_CAPTURE"))
            {
                capture(
                    &renderer,
                    &prepared,
                    &scene,
                    &format!(
                        "{prefix}-player-{}.png",
                        if name == "OPENMT2001_SAVE_ONE" {
                            "one"
                        } else {
                            "two"
                        }
                    ),
                );
            }
            #[cfg(not(feature = "capture"))]
            let _ = prepared;
            // Explicit synthetic roster exercises all new character hit regions;
            // actual-save captures above retain the original active flags/identity.
            if name == "OPENMT2001_SAVE_ONE" {
                let original_players = scene.saved_players.clone();
                let mut synthetic = vec![original_players[0]; 3];
                for (i, player) in synthetic.iter_mut().enumerate() {
                    player[0xc] = 1;
                    player[0x14c..0x150].copy_from_slice(&(i as u32).to_le_bytes());
                }
                let old_player = std::mem::replace(
                    &mut state.player,
                    crate::player_info::State::new(&synthetic).unwrap(),
                );
                scene.saved_players = synthetic;
                let old_owner = state.rights.as_ref().unwrap().owner_number;
                state.rights.as_mut().unwrap().owner_number = 3;
                assert_eq!(state.player_display_order(), [2, 0, 1]);
                for (position, slot) in [2, 0, 1].into_iter().enumerate() {
                    let b = assets.city_layout.buttons[position];
                    let point = [(b[0] + b[2]) as i32, (b[1] + b[3]) as i32];
                    state.mouse(&assets, &scene, Some(point), true);
                    state.mouse(&assets, &scene, Some(point), false);
                    assert_eq!(state.player.players[state.player.selected], slot);
                    assets.prepare(&renderer, &state, &scene).unwrap();
                }
                let selected = state.player.selected;
                state.mouse(&assets, &scene, Some([86, 86]), true);
                state.mouse(&assets, &scene, None, false);
                assert_eq!(state.player.selected, selected);
                state.rights.as_mut().unwrap().owner_number = old_owner;
                state.player = old_player;
                scene.saved_players = original_players;
            }
            state.mouse(&assets, &scene, Some([300, 164]), true);
            state.mouse(&assets, &scene, Some([300, 164]), false);
            assert!(!state.player.today);
            if let Some(&player) = state.player.players.get(state.player.selected) {
                assert_eq!(
                    state.player.rows,
                    crate::player_info::report(&scene.saved_players[player], false).unwrap()
                );
            }
            state.scroll(&assets, 1000);
            assert_eq!(
                state.player.scroll,
                state.player.rows.len().saturating_sub(19)
            );
            assets.prepare(&renderer, &state, &scene).unwrap();
            state.mouse(&assets, &scene, Some([32, 224]), true);
            state.mouse(&assets, &scene, Some([32, 224]), false);
            assert!(!state.player_info && state.open);
            assert_eq!(state.selected, selected);
            println!(
                "{name}: {commodity_matches} nonempty commodity types, selected{id}; {} active saved report players",
                state.player.players.len()
            );
            if name == "OPENMT2001_SAVE_ONE" {
                let mut retail = 0;
                let mut apartments = 0;
                for building in 0..scene.inspection_buildings.len() {
                    if let Some(mut panel) = adjustment.open(&scene, building).unwrap() {
                        panel.prepare(&adjustment, &renderer).unwrap();
                        let apartment = panel.apartment.is_some();
                        assert!(apartment || !panel.rows.is_empty());
                        #[cfg(feature = "capture")]
                        if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE")
                            && if apartment {
                                apartments < 2
                            } else {
                                retail < 2
                            }
                        {
                            capture(
                                &renderer,
                                panel.prepared.as_ref().unwrap(),
                                &scene,
                                &format!("{prefix}-adjustment-{building}.png"),
                            );
                        }
                        if apartment {
                            for point in [[619, 475], [1042, 475]] {
                                panel.pointer(&adjustment, Some(point));
                                assert!(!panel.mouse(true));
                                assert!(!panel.mouse(false));
                            }
                            panel.prepare(&adjustment, &renderer).unwrap();
                            assert_eq!(panel.apartment.as_ref().unwrap().tenants, 0);
                            #[cfg(feature = "capture")]
                            if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE")
                                && apartments == 0
                            {
                                capture(
                                    &renderer,
                                    panel.prepared.as_ref().unwrap(),
                                    &scene,
                                    &format!("{prefix}-apartment-yesterday.png"),
                                );
                            }
                            // Release outside must leave the selected history untouched.
                            panel.pointer(&adjustment, Some([619, 475]));
                            assert!(!panel.mouse(true));
                            panel.pointer(&adjustment, None);
                            assert!(!panel.mouse(false));
                            assert_eq!(panel.apartment.as_ref().unwrap().tenants, 0);
                        }
                        if apartment && apartments == 0 {
                            panel.pointer(&adjustment, Some([86, 418]));
                            assert!(!panel.mouse(true));
                            assert!(!panel.mouse(false));
                            let target = panel.requested_building.take().unwrap();
                            assert_ne!(target, building);
                            panel = adjustment.navigate(&scene, &panel, target).unwrap();
                            assert_eq!(panel.building, target);
                            assert_eq!(panel.apartment.as_ref().unwrap().tenants, 0);
                            panel.prepare(&adjustment, &renderer).unwrap();
                            #[cfg(feature = "capture")]
                            if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE") {
                                capture(
                                    &renderer,
                                    panel.prepared.as_ref().unwrap(),
                                    &scene,
                                    &format!("{prefix}-apartment-previous.png"),
                                );
                            }
                        }
                        if !apartment && retail == 0 {
                            let previous_name = panel.name;
                            panel.pointer(&adjustment, Some([86, 216]));
                            assert!(!panel.mouse(true));
                            assert!(!panel.mouse(false));
                            let target = panel.requested_building.take().unwrap();
                            panel = adjustment.navigate(&scene, &panel, target).unwrap();
                            assert_ne!(panel.name, previous_name);
                            assert!(!panel.rows.is_empty());
                            panel.prepare(&adjustment, &renderer).unwrap();
                            #[cfg(feature = "capture")]
                            if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE") {
                                capture(
                                    &renderer,
                                    panel.prepared.as_ref().unwrap(),
                                    &scene,
                                    &format!("{prefix}-retail-previous.png"),
                                );
                            }
                        }
                        panel.pointer(&adjustment, Some([1200, 565]));
                        assert!(!panel.mouse(true));
                        assert!(panel.mouse(false));
                        if apartment {
                            apartments += 1;
                        } else {
                            retail += 1;
                        }
                    }
                }
                assert_eq!(retail, 6);
                assert_eq!(apartments, 9);
                let mut prompt = crate::prompt::State::default();
                for title in (1898..1916).step_by(2) {
                    let request = crate::prompt::Request { script: None,
                        dialog: None,
                        card: None,
                        title,
                        body: title + 1,
                        dismissal_id: title + 100,
                    };
                    assert_eq!(prompt.submit(request.clone()), None);
                    let prepared = prompts.prepare(&renderer, &prompt).unwrap();
                    #[cfg(feature = "capture")]
                    if let Ok(prefix) = std::env::var("OPENMT2001_LOCATOR_CAPTURE")
                        && (title == 1898 || title == 1908)
                    {
                        capture(
                            &renderer,
                            &prepared,
                            &scene,
                            &format!("{prefix}-prompt-{title}.png"),
                        );
                    }
                    let _ = prepared;
                    prompt.cursor([0., 0.]);
                    assert_eq!(prompt.left_button(true, prompts.button), None);
                    assert_eq!(prompt.left_button(false, prompts.button), None);
                    assert_eq!(prompt.request(), Some(request.clone()));
                    prompt.cursor([
                        (prompts.button[0] + prompts.button[2]) * 0.5,
                        (prompts.button[1] + prompts.button[3]) * 0.5,
                    ]);
                    assert_eq!(prompt.left_button(true, prompts.button), None);
                    assert_eq!(
                        prompt.left_button(false, prompts.button),
                        Some(request.dismissal_id)
                    );
                    assert_eq!(prompt.request(), None);
                }
            }
            println!(
                "{name}: {} retained buildings, {nonempty} nonempty Locator types",
                scene.inspection_buildings.len()
            );
        }
    }
    #[cfg(feature = "capture")]
    fn capture(renderer: &Renderer, panel: &PreparedScene, scene: &Scene, path: &str) {
        crate::capture_test::capture(renderer, Some(panel), scene, path);
    }
    #[test]
    #[ignore = "requires original saved catalog and native list oracle"]
    fn matches_native_locator() {
        for name in [
            "OPENMT2001_NATIVE_LOCATOR",
            "OPENMT2001_NATIVE_LOCATOR_SECOND",
        ] {
            let bytes = fs::read(std::env::var(name).expect(name)).unwrap();
            assert_eq!(&bytes[..8], b"MTLOC001");
            let mut at = 8;
            let catalog = BusinessCatalog::from_rows(std::array::from_fn(|_| {
                let r = bytes[at..at + 116].try_into().unwrap();
                at += 116;
                r
            }));
            let mut word = || {
                let v = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                at += 4;
                v
            };
            let order = std::array::from_fn(|_| word());
            let count = word();
            let expected = (0..count)
                .map(|_| Row {
                    label: word(),
                    kind: word(),
                })
                .collect::<Vec<_>>();
            assert_eq!(rows(&catalog, &order).unwrap(), expected);
            // Native branch captures independently establish exact type equality, including vacant type MAX.
            let count = word();
            for _ in 0..count {
                let kind = word();
                let selected = word();
                let blink = word();
                let color = word();
                assert_eq!(
                    color,
                    type_color(
                        0xff123456,
                        kind,
                        Some(selected),
                        if blink != 0 { 17 } else { 0 }
                    )
                );
            }
            assert_eq!(at, bytes.len());
        }
    }
}

#[cfg(test)]
#[path = "property_panel_test.rs"]
mod property_panel_tests;

#[cfg(test)]
pub(crate) mod refresh_ui_tests {
    use super::*;
    pub(crate) fn fresh() -> Locator {
        Locator {
            strategic_state:None,
            city_info: false,
            city_people: None,
            people_mode: 0,
            polls: None,
            poll_category: None,
            poll_info: false,
            ownership: None,
            rights: None,
            rights_mode: false,
            ownership_mode: false,
            prestige: None,
            prestige_mode: false,
            ownership_scroll: 0,
            pending_category: None,
            player_info: false,
            player: crate::player_info::State::new(&[]).unwrap(),
            saved_panels: false,
            catalog: None,
            owned_rows: Vec::new(),
            my_business: true,
            owned_scroll: 0,
            owned_eligible: [false; 52],
            other_rows: Vec::new(),
            commodity: false,
            other_selected: None,
            other_scroll: 0,
            rows: Vec::new(),
            open: false,
            selected: None,
            scroll: 0,
            pressed: None,
            hover: None,
            tutorial_activated: None,
            dirty: true,
            prepared: None,
        }
    }
    fn report(value: i32) -> crate::city_people::Report {
        crate::city_people::Report { initialized: true, markers: vec![], rows: vec![crate::city_people::Row { style: 0, label: 1, value }] }
    }
    fn rows(ids: &[u32]) -> Vec<Row> { ids.iter().map(|&kind| Row { label: kind + 100, kind }).collect() }
    #[test]
    fn two_refreshes_keep_poll_choice_and_new_reports_cancel_input() {
        let mut old = fresh();
        old.attach_people(report(1));
        old.city_info = false;
        old.poll_info = true;
        old.people_mode = 3;
        old.pressed = Some(Action::Toggle);
        old.hover = Some(Action::Poll);
        old.pending_category = Some(8);
        old.tutorial_activated = Some(12);
        for value in [2, 3] {
            let mut new = fresh();
            new.attach_people(report(value));
            new.attach_polls(Some(std::array::from_fn(|_| crate::consumer_polls::Poll { residents: value as u16, ..Default::default() })));
            new.retain_ui_choices(&old, 2, 2).unwrap();
            assert_eq!(new.open, old.open);
            assert!(new.poll_info && !new.city_info && !new.player_info);
            assert_eq!(new.people_mode, 3);
            assert_eq!(new.city_people.as_ref().unwrap().rows[0].value, value);
            assert_eq!(new.polls.as_ref().unwrap()[8].residents, value as u16);
            assert!(new.pressed.is_none() && new.hover.is_none() && new.pending_category.is_none());
            assert!(new.tutorial_activated.is_none() && new.prepared.is_none() && new.dirty);
            new.open = false;
            old = new;
        }
    }
    #[test]
    fn commodity_orientation_and_stable_scroll_remap_then_removal() {
        let mut old = fresh();
        old.commodity = true;
        old.rows = rows(&[5, 6, 7, 8]); old.other_rows = rows(&[10, 11, 12]);
        old.selected = Some(6); old.other_selected = Some(11);
        old.scroll = 1; old.other_scroll = 1;
        let mut new = fresh();
        new.rows = rows(&[11, 12, 13]); new.other_rows = rows(&[6, 7, 8]);
        new.retain_ui_choices(&old, 1, 1).unwrap();
        assert!(new.commodity);
        assert_eq!(new.rows, rows(&[6, 7, 8]));
        assert_eq!((new.selected, new.other_selected), (Some(6), Some(11)));
        assert_eq!((new.scroll, new.other_scroll), (0, 0));
        let mut shorter = fresh();
        shorter.rows = rows(&[99]); shorter.other_rows = rows(&[1]);
        shorter.retain_ui_choices(&new, 19, 19).unwrap();
        assert_eq!((shorter.selected, shorter.other_selected), (None, None));
        assert_eq!((shorter.scroll, shorter.other_scroll, shorter.owned_scroll), (0, 0, 0));
    }
    #[test]
    fn financial_slot_and_period_use_new_records_with_fallback() {
        let mut record = [0u8; 440]; record[0xc] = 1;
        let mut old = fresh();
        old.player = crate::player_info::State::new(&[record, record, record]).unwrap();
        old.player.players = vec![2, 0, 1]; old.player.selected = 0;
        old.player.today = false; old.player.scroll = 999;
        old.player_info = true;
        let mut changed = record;
        changed[0xa0 - 4 + 152..0xa0 + 152].copy_from_slice(&42f32.to_le_bytes());
        let mut new = fresh();
        new.player = crate::player_info::State::new(&[record, record, changed]).unwrap();
        new.retain_ui_choices(&old, 19, 19).unwrap();
        assert_eq!(new.player.players[new.player.selected], 2);
        assert!(!new.player.today);
        assert_eq!(new.player.records[2], changed);
        assert_eq!(new.player.rows, crate::player_info::report(&changed, false).unwrap());
        assert!(new.player.rows.iter().any(|r| r.amount == 42f32.to_bits()));
        assert_eq!(new.player.scroll, new.player.rows.len().saturating_sub(19));
        let mut missing = fresh();
        missing.player = crate::player_info::State::new(&[record, record]).unwrap();
        missing.player.current = Some(1);
        missing.retain_ui_choices(&new, 19, 19).unwrap();
        assert_eq!(missing.player.players[missing.player.selected], 1);
        let mut empty = fresh(); empty.retain_ui_choices(&missing, 19, 19).unwrap();
        assert!(!empty.player_info && empty.player.rows.is_empty());
        assert_eq!((empty.player.selected, empty.player.scroll), (0, 0));
    }
    #[test]
    fn missing_categories_reset_scroll_and_page_capabilities() {
        use crate::city_prestige::{Prestige, Row as P};
        let prestige = |rows| Prestige { rows, values: [0.; 52], overlay_categories: vec![] };
        let mut old = fresh(); old.city_info = true; old.prestige_mode = true;
        old.prestige = Some(prestige((0..25).map(P::Block).collect())); old.ownership_scroll = 5;
        let mut new = fresh(); new.city_people = Some(report(2));
        new.prestige = Some(prestige((5..25).map(P::Block).collect()));
        new.retain_ui_choices(&old, 19, 19).unwrap();
        assert_eq!(new.ownership_scroll, 0);
        let mut gone = fresh(); gone.retain_ui_choices(&new, 19, 19).unwrap();
        assert!(!gone.city_info && !gone.prestige_mode);
        assert_eq!(gone.ownership_scroll, 0);
    }
}

#[cfg(test)]
mod ordinary_toolbar_assets_tests {
    use super::*;
    #[test]
    #[ignore="requires supplied archive/installer/v1.4 language; optional capture feature and OPENMT2001_TOOLBAR_CAPTURE directory"]
    fn closed_strip_keeps_disabled_lease_without_body_title_surfaces() {
        let read=|key|fs::read(std::env::var(key).unwrap()).unwrap();
        let directory=read("OPENMT2001_ARCHIVE_DIR");let archive=read("OPENMT2001_ARCHIVE_BIN");
        let exe=read("OPENMT2001_EXE");
        let index=ArchiveIndex::parse(&directory,archive.len()as u64).unwrap();
        let param=std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer=param.parent().unwrap().parent().unwrap();
        let path=std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language=path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let bytes=fs::read(language.join("tycoon.bin")).unwrap();
        let offsets=fs::read(language.join("tycoon.off")).unwrap();let mut at=0;
        let strings=offsets.chunks_exact(2).map(|n|{
            let end=at+u16::from_le_bytes(n.try_into().unwrap())as usize;
            let row=&bytes[at..end];at=end;row
        }).collect::<Vec<_>>();
        let assets=Assets::load(&index,&archive,installer,&strings,&exe).unwrap();
        let mut q=toolbar_collapse::Policy{category:33,shown:true,nonzero_kind:true,
            lease_available:false,build_allowed:true,tutorial_allowed:[true;3]};
        let input=toolbar_collapse::Input::default();
        let state=crate::utility_report::View::default();
        let base=assets.toolbar_closed_base.len();
        let closed=assets.toolbar_surfaces(q,&input,&state,false);
        assert_eq!(closed.len(),base+6,"all three visible buttons retain backing+icon");
        assert_eq!(closed[base+2].pixels,assets.toolbar_disabled.pixels,
            "unavailable lease uses native blocked-image fallback, not omission");
        assert_eq!(closed[base+3].pixels,assets.property_side_icons[1].pixels);
        let opened=assets.toolbar_surfaces(q,&input,&state,true);
        assert_eq!(opened.len(),6,"open side overlay contains no second body/title/base");
        assert_eq!(opened[0].vertices[0][0],5.599999904632568_f32);
        assert_eq!(assets.property_side_bounds[1][1],77.5999984741211_f32);
        q.nonzero_kind=false;
        assert_eq!(assets.toolbar_surfaces(q,&input,&state,false).len(),base+2);
        q.shown=false;
        assert!(assets.toolbar_surfaces(q,&input,&state,false).is_empty());
        #[cfg(feature="capture")]
        if let Ok(out)=std::env::var("OPENMT2001_TOOLBAR_CAPTURE") {
            let out=std::path::PathBuf::from(out);assert!(out.is_dir());
            let mut descriptor=wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows){descriptor.backends=wgpu::Backends::DX12;}
            let renderer=pollster::block_on(Renderer::new(descriptor)).unwrap();
            q.shown=true;q.nonzero_kind=true;
            let panel=assets.ordinary_toolbar_scene(&renderer,q,&input,&state,false).unwrap();
            crate::capture_test::capture_standalone(&renderer,&panel,
                out.join("toolbar-closed-disabled-lease.png").to_str().unwrap());
        }
    }
}


#[cfg(test)]
mod open_facing_fixture {
    use super::*;
    use crate::business_view::facing_fixture::{Inputs,same};
    #[test]
    #[ignore="SHA-pinned supplied mesh/PE/font/language and default commodity script; actual Assets::load, no city/GPU"]
    fn supplied_locator_open_bases_and_unchanged_closed_toolbar() {
        let f=Inputs::read();let index=f.index();let strings=f.strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        // Normal Locator constructor evaluates its existing commodity grouping
        // script. This fixture opens no Session, world, scene or gameplay loop.
        let assets=Assets::load(&index,&f.archive,&f.installer,&strings,&f.executable).unwrap();
        assert_eq!(assets.base.len(),10);assert_eq!(assets.property_base.len(),10);
        let regions=f.open_borders(&assets.base[..9]);f.open_borders(&assets.property_base[..9]);
        assert_eq!(assets.bounds,regions[9].bounds);
        let mesh=openmt2001::mesh::CachedMesh::parse(f.payload("gamedata2/brand_new_panels/main_panel_v2.tm")).unwrap();
        let image=PeImage::parse(&f.executable).unwrap();
        same(&assets.base[9],&ui::mpp_background(&mesh,9,&image).unwrap());
        same(&assets.property_base[9],&ui::mpp_background_title(&mesh,9,&image,Some([0;4])).unwrap());
        assert_eq!(assets.property_title_bounds,regions[11].bounds);
        for color in [0xff808080,0xff10b030] {
            let actual=assets.property_background(color);assert_eq!(actual.len(),12);
            for (a,b) in actual[..10].iter().zip(&assets.property_base){same(a,b);}
            let faded=(color&0x00ff_ffff)|(((color>>24)/3)<<24);let mut title=Surface::solid();
            title.rect(regions[11].bounds,[color,color,faded,faded]);same(&actual[10],&title);
            same(&actual[11],&assets.property_title_icon);
        }
        // Frame0 remains the old complete explicit part list, not frame9's
        // new list. Compare every original vertex/UV/material and strip fill.
        let (mut closed,closed_bounds)=ui::panel_frame_base(&index,&f.archive,
            "gamedata2/brand_new_panels/main_panel_v2.tm",&(0..12).collect::<Vec<_>>(),0).unwrap();
        assert_eq!(closed_bounds.len(),49);assert_eq!(closed.len(),12);
        let closed_regions=openmt2001::ui_mesh::regions(&mesh,0).unwrap();
        let strip=closed_regions.iter().find(|r|r.name=="P_MAIN_VERTCAL_BAR_BACKING").unwrap().bounds;
        let mut fill=Surface::solid();fill.rect(strip,[0xff777777,0x66777777,0x66777777,0xff777777]);closed.push(fill);
        assert_eq!(assets.toolbar_closed_base.len(),13);
        for (a,b) in assets.toolbar_closed_base.iter().zip(&closed){same(a,b);}
        let mut state=crate::utility_report::View::default();state.select(Some(33));
        state.open_property_offer(33);state.details=true;state.scroll=3;
        let before=(state.property_offer_open(),state.property_rights_open(),state.details,state.scroll);
        let q=toolbar_collapse::Policy{category:33,shown:true,nonzero_kind:true,
            lease_available:false,build_allowed:true,tutorial_allowed:[true;3]};
        let input=toolbar_collapse::Input::default();
        let collapsed=assets.toolbar_surfaces(q,&input,&state,false);
        assert_eq!(collapsed.len(),19);
        for (a,b) in collapsed[..13].iter().zip(&closed){same(a,b);}
        let opened=assets.toolbar_surfaces(q,&input,&state,true);assert_eq!(opened.len(),6,
            "open side overlay adds controls only, no second body/title/base");
        for side in 0..3 {same(&opened[side*2+1],&collapsed[13+side*2+1]);}
        assert_eq!(before,(state.property_offer_open(),state.property_rights_open(),state.details,state.scroll));
        assert!(assets.toolbar_surfaces(toolbar_collapse::Policy{shown:false,..q},&input,&state,false).is_empty());
        #[cfg(feature="capture")]
        if let Ok(prefix)=std::env::var("OPENMT2001_FACING_CAPTURE_PREFIX") {
            // Actual loaded base/palette component diagnostics, not full city,
            // report/control/retained Locator choices or native acceptance.
            crate::business_view::facing_fixture::capture(&assets.base,&format!("{prefix}-locator-open-base.png"));
            crate::business_view::facing_fixture::capture(&assets.property_background(0xff10b030),&format!("{prefix}-property-open-base.png"));
            crate::business_view::facing_fixture::capture(&collapsed,&format!("{prefix}-toolbar-closed.png"));
        }
    }
}
