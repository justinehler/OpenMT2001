//! Original result hubs execute in the retained scenario VM and city.
use super::{CommandAssets, Session};
use openmt2001::{
    business_binding, level_script, level_startup, lua4::Value, outcome_notice::Sequence,
};
#[path = "../result_audio.rs"]
pub mod audio;
#[path = "../result_fireworks.rs"]
pub mod fireworks;
#[path = "../result_lighting.rs"]
pub mod lighting;
#[path = "../result_medallions.rs"]
pub mod medallions;
#[path = "../result_particles_gpu.rs"]
pub mod particles_gpu;
#[path = "../result_text.rs"]
pub mod text;

/// One original4b1b20 record: optional localized heading plus up to five
/// ASCII names from mc.exe, converted to the original font glyph indices.
#[derive(Clone, Debug)]
pub struct CreditRow {
    pub heading_id: Option<u32>,
    pub names: Vec<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct ResultPresentation {
    pub sequence: Sequence,
    pub characters: (u32, u32),
    pub flyby: Option<String>,
    pub fireworks: Option<u32>,
    /// Original 4b16d0 uses its own global RNG node at 68c028.
    pub fireworks_seed: Option<u32>,
    /// Native 4b3230 calls 445ca0 for each category and reads the owning
    /// player's +150 character kind to select its corner texture.
    pub medallion_characters: [Option<u8>; 52],
    pub cut_scene_identifier: Option<u32>,
    pub credits: bool,
    /// Original4b1bc0 constructs 21 fixed rows plus 11 shuffled entries.
    pub credit_rows: usize,
    pub credit_lines: Vec<CreditRow>,
    pub bypass_enabled: bool,
    pub fixed_camera: bool,
    pub end_requested: Option<u32>,
}

impl ResultPresentation {
    fn new(sequence: Sequence, characters: (u32, u32)) -> Self {
        Self {
            sequence,
            characters,
            flyby: None,
            fireworks: None,
            fireworks_seed: None,
            medallion_characters: [None; 52],
            cut_scene_identifier: None,
            credits: false,
            credit_rows: 0,
            credit_lines: Vec::new(),
            bypass_enabled: true,
            fixed_camera: false,
            end_requested: None,
        }
    }
    /// Original4dad30/4b17e0 loads these IDs through the active language
    /// bank into the result text buffer. The localized glyph bytes are owned
    /// by the presentation host, so this remains an ID rather than an English
    /// string embedded in simulation state.
    pub fn text_id(&self) -> Option<u32> {
        match self.cut_scene_identifier {
            Some(1) => Some(0x6e7),
            Some(2) => Some(0x6e8),
            Some(3) => Some(0x6e9),
            _ => None,
        }
    }
    fn apply(&mut self, command: &level_script::Command, args: &[Value]) -> Result<(), String> {
        let number = |index: usize| -> Result<u32, String> {
            match args.get(index) {
                Some(Value::Number(value))
                    if value.is_finite() && *value >= 0. && *value <= u32::MAX as f64 =>
                {
                    Ok(*value as u32)
                }
                _ => Err("result script argument is not an unsigned number".into()),
            }
        };
        match command {
            level_script::Command::ResultFlyby => {
                let [Value::Bytes(path)] = args else {
                    return Err("result flyby path is missing".into());
                };
                let path =
                    std::str::from_utf8(path).map_err(|_| "result flyby path is not UTF-8")?;
                let name = path.replace('\\', "/");
                if !name
                    .to_ascii_lowercase()
                    .starts_with("gamedata/cut_scenes/")
                    || name.split('/').any(|component| component == "..")
                {
                    return Err("result flyby path leaves original cut scenes".into());
                }
                self.flyby = Some(name);
            }
            level_script::Command::ResultCameraMode => self.fixed_camera = number(0)? == 3,
            level_script::Command::ResultFireworks => self.fireworks = Some(number(0)?),
            level_script::Command::ResultCutSceneIdentifier => {
                self.cut_scene_identifier = Some(number(0)?)
            }
            level_script::Command::ResultCredits => {
                self.credits = number(0)? != 0;
                self.credit_rows = if self.credits { 32 } else { 0 };
            }
            level_script::Command::ResultEnableBypass => self.bypass_enabled = true,
            level_script::Command::ResultDisableBypass => self.bypass_enabled = false,
            level_script::Command::ResultEnd => {
                if number(0)? != 1 {
                    return Err("result Game.End did not request level completion".into());
                }
                self.end_requested = Some(number(1)?);
            }
            _ => return Err("non-result command reached result presentation".into()),
        }
        Ok(())
    }
}

impl Session<'_> {
    pub fn result_presentation(&self) -> Option<&ResultPresentation> {
        self.result_presentation.as_ref()
    }

    pub fn begin_result_sequence(
        &mut self,
        sequence: Sequence,
        characters: (u32, u32),
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        if self.result_presentation.is_some() {
            return Err("result sequence already active".into());
        }
        let folder = match sequence {
            Sequence::Win => "WIN_SEQUENCE",
            Sequence::Lose => "LOSE_SEQUENCE",
            Sequence::Complete => "COMPLETE_SEQUENCE",
        };
        let path = self.script_directory.join(folder).join("hub.lua");
        let source = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut medallions = [None; 52];
        for (index, category) in self.world.categories.iter().enumerate() {
            let owner = usize::from(category[5]);
            if owner == 0 {
                continue;
            }
            let player = self
                .roster
                .manager
                .players
                .get(owner - 1)
                .ok_or("result category owner leaves roster")?;
            let character = u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap());
            if character >= 10 {
                return Err("result medallion character leaves original table".into());
            }
            medallions[index] = Some(character as u8);
        }
        let mut presentation = ResultPresentation::new(sequence, characters);
        presentation.medallion_characters = medallions;
        self.result_presentation = Some(presentation);
        let result = self.run_result(
            &format!("{folder}/hub.lua"),
            Some(&source),
            &["EVENTINITIALISE_Level", "EVENT_Level"],
            assets,
        );
        if result.is_err() {
            self.result_presentation = None;
        }
        result?;
        Ok(())
    }

    /// Native4b1f82 calls4b1bc0 on the 50Hz title-fade tick where phase1
    /// enters phase12 (tick256 in the original 32-row trace). The UI sends
    /// that edge to this owning worker; only here do row RNG and names advance.
    pub fn construct_result_credits(
        &mut self,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<(), String> {
        let presentation = self
            .result_presentation
            .as_ref()
            .ok_or("result credits without sequence")?;
        if presentation.sequence != Sequence::Complete
            || !presentation.credits
            || !presentation.credit_lines.is_empty()
        {
            return Err("result credits constructor outside its single COMPLETE edge".into());
        }
        let lines = credit_lines(image, &mut self.credit_random_state)?;
        if lines.len() != presentation.credit_rows {
            return Err("native result credit count changed".into());
        }
        self.result_presentation
            .as_mut()
            .ok_or("result credits lost")?
            .credit_lines = lines;
        Ok(())
    }

    pub fn dispatch_result_event(
        &mut self,
        name: &str,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        if self.result_presentation.is_none() {
            return Err("result event without active sequence".into());
        }
        if !matches!(name, "FlybyEnded" | "CutSceneEnded") {
            return Err("unsupported result event".into());
        }
        self.run_result(name, None, &[name], assets)
    }

    fn run_result(
        &mut self,
        label: &str,
        source: Option<&[u8]>,
        events: &[&str],
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        let context = self.world.script_context(
            assets.image,
            &self.resident_models,
            level_startup::HostContext {
                accounting_enabled: true,
                difficulty: self.difficulty as i32,
                notifications: self.world.city.ai_demand.ready != 0,
                auxiliary_template: self.policy.auxiliary_prior_bytes,
                category_base: 0x617628,
                building_base: 0x591318,
            },
        )?;
        let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
            assets.board,
            assets.file,
            std::mem::take(&mut self.models),
        );
        let result = (|| {
            for index in 0..geometry.registrar().models.len() {
                geometry.retain_model(
                    openmt2001::building_geometry::RegisteredModel::from_definition(
                        index as u32 + 1,
                        &geometry.registrar().models[index].definition,
                    )?,
                )?;
            }
            let players: Vec<_> = self
                .roster
                .manager
                .players
                .iter()
                .map(|p| {
                    Some(business_binding::Player {
                        identity: p.reference,
                        kind: u32::from_le_bytes(p.bytes[0x154..0x158].try_into().unwrap()),
                    })
                })
                .collect();
            let before = self.hooks.commands.len();
            if let Some(source) = source {
                self.scripts
                    .runtime
                    .execute(label, source, &mut |id, args| {
                        if id == 708 {
                            return random_zero_to(&mut self.script_random_state, args).map(Some);
                        }
                        self.scripts.bindings.handle(
                            id,
                            args,
                            &mut self.world.script_state(&players),
                            &mut self.scripts.events,
                            &context,
                            &mut geometry,
                            &mut self.hooks,
                        )
                    })?;
            }
            for event in events {
                if *event == "EVENTINITIALISE_Level" || *event == "EVENT_Level" {
                    self.scripts.runtime.call(event, &[0.], &mut |id, args| {
                        if id == 708 {
                            return random_zero_to(&mut self.script_random_state, args).map(Some);
                        }
                        self.scripts.bindings.handle(
                            id,
                            args,
                            &mut self.world.script_state(&players),
                            &mut self.scripts.events,
                            &context,
                            &mut geometry,
                            &mut self.hooks,
                        )
                    })?;
                    if *event == "EVENTINITIALISE_Level"
                        && self
                            .result_presentation
                            .as_ref()
                            .is_some_and(|result| result.sequence != Sequence::Lose)
                    {
                        if self.world.clock.hour() != 21 {
                            return Err("result hub Calendar setter did not select hour21".into());
                        }
                    }
                } else {
                    self.scripts.dispatch_catch_event(
                        (event, 0),
                        &mut self.world.script_state(&players),
                        &context,
                        &mut geometry,
                        &mut self.hooks,
                    )?;
                }
            }
            let presentation = self
                .result_presentation
                .as_mut()
                .ok_or("result state lost")?;
            let already_ended = presentation.end_requested.is_some();
            for (command, args) in &self.hooks.commands[before..] {
                presentation.apply(command, args)?;
                if matches!(command, level_script::Command::ResultFireworks) {
                    presentation.fireworks_seed = Some(self.particle_random_state);
                }
            }
            if !already_ended && presentation.end_requested.is_some() {
                //4ba1d0 samples all six history series before showing the
                //stopped graph, including a partial final day.
                let accounts = self.roster.account_prefixes(&self.world.city)?;
                self.population.sample_history(
                    &mut self.history,
                    &self.world.city,
                    &accounts,
                    assets.image,
                )?;
            }
            Ok(())
        })();
        self.models = std::mem::take(geometry.registrar_mut());
        result
    }
}

/// Original4b1bc0 constructs one opening title, eleven shuffled programmer
/// names from the table at545274, then these twenty fixed credit records.
/// The RNG at8b06e8 is separate from Lua Random.ZeroTo's93aee8 node.
fn credit_descriptors(
    image: &openmt2001::pe::PeImage<'_>,
    random_state: &mut u32,
) -> Result<Vec<[u32; 6]>, String> {
    const FIXED: [[u32; 6]; 20] = [
        [0, 0x545544, 0, 0, 0, 0],
        [0, 0x545534, 0, 0, 0, 0],
        [0x6fc, 0x5454f0, 0x5454fc, 0x545514, 0x545528, 0],
        [0x6ec, 0, 0, 0, 0, 0],
        [0x6ee, 0x5454e0, 0, 0, 0, 0],
        [0x6ef, 0x5454d0, 0, 0, 0, 0],
        [0x6f0, 0x5454c0, 0, 0, 0, 0],
        [0x6f1, 0x5454ac, 0, 0, 0, 0],
        [0x6f2, 0x5454a0, 0, 0, 0, 0],
        [0x6f3, 0x545484, 0x545490, 0, 0, 0],
        [0x6f4, 0x545474, 0, 0, 0, 0],
        [0x6f5, 0x545464, 0, 0, 0, 0],
        [0x6f6, 0x545414, 0x545424, 0x545434, 0x545444, 0x545454],
        [0x6f6, 0x5453e8, 0x5453f8, 0x545408, 0, 0],
        [0x6f7, 0x5453d4, 0, 0, 0, 0],
        [0x6f8, 0x5453c4, 0, 0, 0, 0],
        [0x6f9, 0x5453b0, 0, 0, 0, 0],
        [0x6fa, 0x5453a4, 0, 0, 0, 0],
        [0x6fb, 0x54537c, 0x545388, 0x545394, 0, 0],
        [0x704, 0x545344, 0x545364, 0x545370, 0, 0],
    ];
    let mut order = [0u32; 11];
    for (slot, value) in order.iter_mut().enumerate() {
        *value = slot as u32;
    }
    for i in 0..11 {
        *random_state = random_state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        let pick = ((*random_state >> 16) & 0x7fff) as usize % 11;
        order.swap(i, pick);
    }
    let mut rows = Vec::with_capacity(32);
    rows.push([0x6eb, 0, 0, 0, 0, 0]);
    for position in order {
        let address = 0x545274 + position * 4;
        let name = u32::from_le_bytes(image.at(address, 4)?.try_into().unwrap());
        rows.push([0, name, 0, 0, 0, 0]);
    }
    rows.extend(FIXED);
    Ok(rows)
}

fn credit_lines(
    image: &openmt2001::pe::PeImage<'_>,
    random_state: &mut u32,
) -> Result<Vec<CreditRow>, String> {
    credit_descriptors(image, random_state)?
        .into_iter()
        .map(|row| {
            let mut names = Vec::new();
            for address in row[1..].iter().copied().filter(|&address| address != 0) {
                let bytes = image.at(address, 96)?;
                let end = bytes
                    .iter()
                    .position(|&value| value == 0)
                    .ok_or("credit name lacks original terminator")?;
                let encoded = bytes[..end]
                    .iter()
                    .map(|&value| {
                        value
                            .checked_sub(32)
                            .filter(|&value| value < 95)
                            .ok_or_else(|| "credit name has unsupported font glyph".to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                names.push(encoded);
            }
            Ok(CreditRow {
                heading_id: (row[0] != 0).then_some(row[0]),
                names,
            })
        })
        .collect()
}

/// Original4db450: numeric upper bound is truncated by50e928, the separate
/// static RNG at93aee8 advances once, then signed division returns 0..upper.
/// The result hub reads the live process Game.performance bound; larger uses are bounded
/// here because native `inc ecx` overflows at i32::MAX.
fn random_zero_to(state: &mut u32, args: &[Value]) -> Result<f64, String> {
    let [Value::Number(upper)] = args else {
        return Err("Random.ZeroTo requires one numeric upper bound".into());
    };
    if !upper.is_finite() || *upper < 0. || *upper >= i32::MAX as f64 {
        return Err("Random.ZeroTo upper bound leaves supported native range".into());
    }
    let divisor = (*upper as i32 + 1) as u32;
    *state = state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
    let value = ((*state >> 16) & 0x7fff) % divisor;
    Ok(f64::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn result_random_uses_the_native_static_stream() {
        // 4db450 uses node93aee8, not the process RNG. Check two consecutive
        // calls and the full saved LCG word so a reseed cannot masquerade as
        // an independent draw.
        let mut state = 0x1234_5678;
        assert_eq!(
            random_zero_to(&mut state, &[Value::Number(2.)]).unwrap(),
            2.
        );
        assert_eq!(state, 0xb3e9_7b5b);
        assert_eq!(
            random_zero_to(&mut state, &[Value::Number(2.)]).unwrap(),
            1.
        );
        assert_eq!(state, 0x5b3f_58b2);
    }

    #[test]
    fn complete_credits_have_native_constructor_count() {
        let mut presentation = ResultPresentation::new(Sequence::Complete, (0, 1));
        presentation
            .apply(&level_script::Command::ResultCredits, &[Value::Number(1.)])
            .unwrap();
        assert!(presentation.credits);
        assert_eq!(presentation.credit_rows, 32);
    }

    #[test]
    fn result_identifiers_select_original_language_rows() {
        let mut presentation = ResultPresentation::new(Sequence::Win, (0, 1));
        assert_eq!(presentation.text_id(), None);
        for (identifier, expected) in [
            (1., Some(1767)),
            (2., Some(1768)),
            (3., Some(1769)),
            (4., None),
        ] {
            presentation
                .apply(
                    &level_script::Command::ResultCutSceneIdentifier,
                    &[Value::Number(identifier)],
                )
                .unwrap();
            assert_eq!(presentation.text_id(), expected);
        }
    }

    #[test]
    #[ignore = "requires original v1.4 US language data"]
    fn result_text_ids_address_supplied_localized_glyphs() {
        let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let lengths = std::fs::read(language.join("tycoon.off")).unwrap();
        let glyphs = std::fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let mut selected = Vec::new();
        for (index, length) in lengths.chunks_exact(2).enumerate() {
            let end = at + usize::from(u16::from_le_bytes(length.try_into().unwrap()));
            if (1767..=1769).contains(&index) {
                selected.push(glyphs[at..end].to_vec());
            }
            at = end;
        }
        assert_eq!(at, glyphs.len());
        assert_eq!(
            selected,
            [
                b"9OU\x07VE\x007ON".to_vec(),
                b"9OU\x07VE\x00,OST".to_vec(),
                b"9OU\x07VE\x00COMPLETED\x00THE\x00GAME\x0e".to_vec(),
            ]
        );
    }

    #[test]
    #[ignore = "requires original v1.4 4b1bc0 row-constructor fixture"]
    fn complete_credit_count_matches_original_calls() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_RESULT_CREDITS").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTRCRED1");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x037f);
        let rows = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 16 + rows * 24);
        let mut presentation = ResultPresentation::new(Sequence::Complete, (0, 1));
        presentation
            .apply(&level_script::Command::ResultCredits, &[Value::Number(1.)])
            .unwrap();
        assert_eq!(presentation.credit_rows, rows);
    }

    #[test]
    #[ignore = "requires original v1.4 credit-constructor fixture and executable"]
    fn complete_credit_rows_match_native_constructor_arguments() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_RESULT_CREDITS").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTRCRED1");
        let image_bytes = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = openmt2001::pe::PeImage::parse(&image_bytes).unwrap();
        let mut state = 0x1234_5678;
        let descriptors = credit_descriptors(&image, &mut state).unwrap();
        assert_eq!(descriptors.len(), 32);
        assert_eq!(bytes.len(), 16 + descriptors.len() * 24);
        for (index, row) in descriptors.iter().enumerate() {
            let expected: [u32; 6] = std::array::from_fn(|column| {
                let offset = 16 + index * 24 + column * 4;
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
            });
            assert_eq!(*row, expected, "credit row {index}");
        }
        assert_eq!(state, 3_701_378_197);
        let mut state = 0x1234_5678;
        let resolved = credit_lines(&image, &mut state).unwrap();
        assert_eq!(resolved.len(), 32);
        assert_eq!(resolved[0].heading_id, Some(1771));
        assert_eq!(
            resolved[1].names[0],
            b"Keith Taylor".iter().map(|&b| b - 32).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "requires original v1.4 scripts, archive, model, road and parameter assets"]
    fn result_hubs_keep_final_history_and_game_end_gate() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
                let executable = read("OPENMT2001_EXE");
                let archive = read("OPENMT2001_ARCHIVE_BIN");
                let directory = read("OPENMT2001_ARCHIVE_DIR");
                let build = read("OPENMT2001_BUILD_FILE");
                let parameters = read("OPENMT2001_PARAMETERS");
                let timetable = read("OPENMT2001_TIMETABLE");
                let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
                let board = openmt2001::board::Board::from_v14(&image).unwrap();
                let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
                let index =
                    openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
                        .unwrap();
                let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
                let scripts = exe_path.parent().unwrap().join("scripts");
                let language = exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa");
                let names = super::super::character_names(&image, &language).unwrap();
                let era_styles = super::super::load_era_styles(
                    Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
                        .parent()
                        .unwrap(),
                )
                .unwrap();
                let assets = CommandAssets {
                    image: &image,
                    board: &board,
                    file: &file,
                };
                let process = crate::live_host::process_runtime::startup(&executable, "")
                    .unwrap()
                    .0;
                let mut session = Session::initialize_in_process(
                    super::super::Inputs {
                        tree_locators: None,
                        image: &image,
                        board: &board,
                        file: &file,
                        people: openmt2001::people::PeopleModels::load(&image, &index, &archive)
                            .unwrap(),
                        vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                            &image, &index, &archive,
                        )
                        .unwrap(),
                        transport_models: openmt2001::traffic::models::Models::from_original(
                            &image, &index, &archive,
                        )
                        .unwrap(),
                        road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
                        scripts: &scripts,
                        parameters: std::str::from_utf8(&parameters).unwrap(),
                        era_styles,
                        timetable: std::str::from_utf8(&timetable).unwrap(),
                        scenario: "SCENARIO1",
                        difficulty: 0,
                        selected_character: 4,
                        profile_name: b"Complete validation",
                        character_names: &names,
                    },
                    super::super::Policy {
                        seed: 0x12345678,
                        auxiliary_prior_bytes: [0; 0x11c],
                    },
                    process,
                )
                .unwrap();
                let mut expected_state = session.script_random_state;
                let selected =
                    random_zero_to(&mut expected_state, &[Value::Number(2.)]).unwrap() as u32;
                session
                    .begin_result_sequence(Sequence::Complete, (0, 1), &assets)
                    .unwrap();
                let result = session.result_presentation().unwrap().clone();
                assert_eq!(session.script_random_state, expected_state);
                assert_eq!(
                    result.flyby.as_deref(),
                    Some(match selected {
                        0 => "gamedata/cut_scenes/win_low.tm",
                        1 => "gamedata/cut_scenes/win_medium.tm",
                        2 => "gamedata/cut_scenes/win_high.tm",
                        _ => unreachable!(),
                    })
                );
                assert_eq!(session.world.clock.hour(), 21);
                assert_eq!(result.fireworks, Some(1));
                assert!(result.credits);
                assert_eq!(result.credit_rows, 32);
                assert!(
                    result.credit_lines.is_empty(),
                    "credits constructed before native title fade"
                );
                assert!(!result.bypass_enabled);
                assert_eq!(result.cut_scene_identifier, Some(3));
                assert_eq!(result.text_id(), Some(1769));
                assert_eq!(result.end_requested, None);
                let credit_rng_before = session.credit_random_state;
                let mut controller = crate::result_controller::Controller::new(false, true, 32);
                controller.set_identifier(Some(3));
                for tick in 1..=256 {
                    controller.tick(false);
                    assert_eq!(controller.take_credit_construction_edge(), tick == 256);
                }
                assert_eq!(session.credit_random_state, credit_rng_before);
                session.construct_result_credits(&image).unwrap();
                assert_eq!(
                    session.result_presentation().unwrap().credit_lines.len(),
                    32
                );
                assert_ne!(session.credit_random_state, credit_rng_before);
                assert!(session.construct_result_credits(&image).is_err());
                session
                    .dispatch_result_event("FlybyEnded", &assets)
                    .unwrap();
                let result = session.result_presentation().unwrap();
                assert!(result.fixed_camera);
                assert!(result.bypass_enabled);
                assert_eq!(result.end_requested, None);
                let before = session.history.count(0, 2).unwrap();
                session
                    .dispatch_result_event("CutSceneEnded", &assets)
                    .unwrap();
                assert_eq!(
                    session.result_presentation().unwrap().end_requested,
                    Some(1)
                );
                assert_eq!(session.history.count(0, 2), Some((before + 1).min(480)));
                let state = crate::stats_panel::State::show(
                    session.world.objective.kind,
                    false,
                    session.roster.manager.players.len(),
                )
                .unwrap();
                let snapshot =
                    crate::stats_panel::Snapshot::from_session(&session, &state).unwrap();
                assert_eq!(snapshot.history, session.history.as_bytes());
                let history = snapshot.history;
                session
                    .dispatch_result_event("CutSceneEnded", &assets)
                    .unwrap();
                assert_eq!(
                    session.history.as_bytes(),
                    history,
                    "duplicate end must not sample twice"
                );
                for (sequence, expected_end) in [(Sequence::Win, 1), (Sequence::Lose, 2)] {
                    session.result_presentation = None;
                    session
                        .begin_result_sequence(sequence, (0, 1), &assets)
                        .unwrap();
                    let count = session.history.count(0, 2).unwrap();
                    session
                        .dispatch_result_event("FlybyEnded", &assets)
                        .unwrap();
                    session
                        .dispatch_result_event("CutSceneEnded", &assets)
                        .unwrap();
                    assert_eq!(
                        session.result_presentation().unwrap().end_requested,
                        Some(expected_end)
                    );
                    assert_eq!(session.history.count(0, 2), Some((count + 1).min(480)));
                    let history = session.history.as_bytes().to_vec();
                    session
                        .dispatch_result_event("CutSceneEnded", &assets)
                        .unwrap();
                    assert_eq!(session.history.as_bytes(), history);
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
#[path = "performance_tests.rs"]
mod performance_tests;
