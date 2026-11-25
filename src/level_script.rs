//! Recovered level-script bindings over one live city and resident pool.
//! The host owns script buffers, geometry registration, notifications and game
//! objectives. This is the map/population stage, not the complete boot sequence.
pub mod objective_selection;

use crate::{
    business_binding,
    lua4::{Runtime, Value},
};

#[derive(Debug)]
pub enum Command {
    Objective,
    Prompt,
    TutorialDisableAllControls,
    TutorialDisableAllPicking,
    TutorialEnableAllPicking,
    TutorialDisableCameraControl,
    TutorialArrow,
    TutorialMessage,
    TutorialDismissPrompt,
    TutorialEnableControl,
    TutorialEnableControlAndChildren,
    TutorialDisableControl,
    TutorialPositionPrompt,
    TutorialEnableBlockPicking,
    TutorialEnableBuildingPicking,
    TutorialCameraMode,
    TutorialCityView,
    TutorialPlanView,
    ///4db810: numeric-only native FTOL store to builder controller+788.
    TutorialBuildType,
    ///4db850: numeric-only native FTOL store to builder controller+78c.
    TutorialBuildUse,
    ///4db690: normal appmode3 shows original day_tutor_panel instance57e76c.
    CreateEventList,
    ///4dba70/4dbab0/4dbae0; bid amount is an increment over live+6c.
    ForcedAuction { producer: u32, participant: u32, amount: u32 },
    ///4daa60:4ad760(name,1,0,0,1,100), after Lua's last-argument string conversion.
    PlaySound,
    ResultFlyby,
    ResultCameraMode,
    ResultFireworks,
    ResultCutSceneIdentifier,
    ResultEnableBypass,
    ResultDisableBypass,
    ResultCredits,
    ResultEnd,
}

pub trait Hooks: crate::map_script::Hooks {
    /// Original pause manager suspends/resumes timer group2 synchronously.
    fn pause_timers(&mut self, event: crate::pause::Timers) -> Result<(), String>;
    fn calendar_timers(
        &mut self,
    ) -> (
        &mut crate::timers::CalendarFrequency,
        &mut [crate::timers::Task],
    );
    fn command(&mut self, command: Command, arguments: &[Value]) -> Result<(), String>;
}

fn tutorial_build_integer(arguments: &[Value]) -> Option<i32> {
    match arguments.first() {
        Some(Value::Number(value)) => Some(crate::economy::ftol_low_word(*value)),
        _ => None,
    }
}

fn forced_auction(id:u32,arguments:&[Value])->Option<(u32,u32,u32)>{
    let Value::Number(participant)=arguments.first()? else{return None};
    let participant=crate::economy::ftol_low_word(*participant) as u32;
    let amount=match arguments.get(1){Some(Value::Number(value))=>crate::economy::ftol_low_word(*value) as u32,_=>0};
    Some((match id{738=>0x464530,739=>0x4644f0,740=>0x4644a0,_=>return None},participant,amount))
}

/// Borrow authoritative records; no copied city topology or resident counters.
pub struct State<'a> {
    pub clock: &'a mut crate::clock::Clock,
    pub ai_demand: &'a mut crate::ai_demand::State,
    pub objective: &'a mut crate::scenario_objective::State,
    pub settings: &'a mut crate::startup_settings::State,
    pub configuration: &'a mut crate::resident::Configuration,
    pub welfare: &'a mut crate::resident::Welfare,
    pub pause: &'a mut crate::pause::State,
    pub buildings: &'a mut [[u8; 0x394]],
    pub businesses: &'a mut [[u8; 0x220]],
    pub categories: &'a mut [[u8; 0x17c]; 52],
    pub definitions: &'a mut crate::initial_conditions::Definitions,
    pub commodities: &'a mut crate::commodity_script::State,
    pub catalog: &'a mut crate::business_script::State,
    pub residents: &'a mut crate::economy::ResidentAccounts,
    pub creation: &'a mut crate::economy::ResidentCreationState,
    pub available: &'a mut Vec<usize>,
    pub availability_cursor: &'a mut Option<usize>,
    pub players: &'a [Option<business_binding::Player>],
    pub player_accounts: &'a mut [crate::economy::PlayerAccount],
    pub player_money: &'a mut f32,
    pub construction_components: &'a mut [f32; 4],
}

/// Live globals/configuration at the script call boundary. Explicit RNG and
/// allocator state is in State; a fresh seed is never invented by this runner.
#[derive(Clone, Copy)]
pub struct Context<'a, 'image> {
    pub total_clock: u32,
    pub image: &'a crate::pe::PeImage<'image>,
    pub models: &'a [crate::economy::ResidentModelBinding; 32],
    pub difficulty: i32,
    pub hour: i32,
    pub discount: f32,
    pub date: [u8; 2],
    pub timestamp: u32,
    pub ownership_timestamp: u32,
    pub notifications: bool,
    pub accounting_enabled: bool,
    pub auxiliary_template: [u8; 0x11c],
    pub category_base: u32,
    pub building_base: u32,
}

impl Context<'_, '_> {
    /// Refresh all script-visible clock fields together before synchronous calls.
    pub fn update_clock(&mut self, clock: &crate::clock::Clock) {
        self.total_clock = clock.timing[0];
        self.hour = clock.hour() as i32;
        self.date = [clock.calendar[2] as u8, clock.calendar[1] as u8];
        self.timestamp = clock.timing[0];
        self.ownership_timestamp = clock.timing[1];
    }
}

// 741 belongs to TutorialSetBuildUse. Calendar owns 744/745 (get/set).
fn register_calendar(vm: &mut Runtime) -> Result<(), String> {
    vm.register_number_properties("Calendar", 744)?;
    for (name, value) in [("GAME_MINUTE", 10), ("GAME_HOUR", 625),
        ("GAME_YEAR", 15000), ("GAME_DECADE", 150000)] {
        vm.set_number(name, value as f64)?;
    }
    for hour in 0..24 { vm.set_number(&format!("HOUR_{hour}"), hour as f64)?; }
    for year in 0..10 { vm.set_number(&format!("YEAR_{year:02}"), year as f64)?; }
    for era in 0..8 { vm.set_number(&format!("ERA_{}", 1930 + era * 10), era as f64)?; }
    Ok(())
}

fn calendar_property(id: u32, arguments: &[Value], clock: &mut crate::clock::Clock)
    -> Result<Option<f64>, String> {
    let slot = match arguments.first() {
        Some(Value::Bytes(key)) => match key.as_slice() {
            b"hour" => 0, b"year" => 1, b"era" => 2,
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    if id == 744 { return Ok(Some(clock.calendar[slot] as i32 as f64)); }
    // host_property performs lua_tonumber in C, including numeric strings and nil.
    let Some(Value::Number(value)) = arguments.get(1) else {
        return Err("Calendar setter missing C-coerced number".into());
    };
    clock.calendar[slot] = crate::economy::ftol_low_word(*value) as u32;
    Ok(None)
}

pub struct Bindings {
    selected_objective: objective_selection::Selection,
    startup_settings: crate::startup_settings::Bindings,
    resident_settings: crate::resident_settings::Bindings,
    map: crate::map_script::Bindings,
    properties: crate::initial_conditions::Bindings,
    commodities: crate::commodity_script::Bindings,
    businesses: crate::business_script::Bindings,
}

impl Bindings {
    /// Bind the selected record before any script dispatch that may call the getter.
    /// Refresh after every selection/reselection, target edit or reconstruction;
    /// pass None on deselection/teardown. No world-objective or save fallback.
    pub fn bind_selected_level(&mut self, level: Option<&crate::level_catalog::Level>) {
        self.selected_objective.bind(level);
    }

    /// Reserves callback IDs1..14,100..164,200..290,300..406,500..502,600..609,700..746. Host buffers
    /// registered with Runtime::script_buffer satisfy nested C_RunScript calls.
    pub fn register(vm: &mut Runtime, image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        register_calendar(vm)?;
        let map = crate::map_script::Bindings::register(vm, 1, 2)?;
        vm.execute("Business table", b"Business={}", &mut |_, _| Err("unexpected initialization callback".into()))?;
        for index in 0..600 {
            vm.execute("Business entry", b"openmt_business={}", &mut |_, _| Err("unexpected initialization callback".into()))?;
            vm.register_indexed_method("openmt_business", "SetOwner", 734, index)?;
            vm.execute("Business index", format!("Business[{index}]=openmt_business; openmt_business=nil").as_bytes(), &mut |_, _| Err("unexpected initialization callback".into()))?;
        }
        let commodities = crate::commodity_script::Bindings::register(vm, image, 100)?;
        let properties = crate::initial_conditions::Bindings::register(vm, 300)?;
        crate::initial_conditions::Bindings::register_block_constants(vm, image)?;
        // Original4d58fc registers Block before BusinessInfo at4d5906.
        // Both register LANDSCAPE: the final business enum must be44, not2.
        let businesses = crate::business_script::Bindings::register(vm, image, 200)?;
        let startup_settings = crate::startup_settings::Bindings::register(vm, 600)?;
        crate::population_script::register(vm, 6)?;
        vm.register_number_method("People","NumberInCity",14,0)?;
        let resident_settings = crate::resident_settings::Bindings::register(vm, 500)?;
        vm.register_event_triggers(7)?;
        vm.register_coerced("C_PlaySound", 13, "S")?;
        vm.register_calendar_frequency("C_SetCalendarFrequency", 11)?;
        vm.register_calendar_frequency_method("Calendar", "SetFrequency", 11)?;
        // Native 4dbe8d..4dbed8 registers these Tutorial 1 control constants.
        for (name, value) in [
            ("DIFFICULTY_EASY", 0.),
            ("DIFFICULTY_MEDIUM", 1.),
            ("DIFFICULTY_HARD", 2.),
            ("GAME_MINUTE", 10.),
            ("GAME_HOUR", 625.),
            ("GAME_YEAR", 15000.),
            ("GAME_DECADE", 150000.),
            ("HOUR_9", 9.),
            ("HOUR_21", 21.),
            ("FIXEDCAM", 3.),
            ("BLOCKCAM", f64::from(image.f32_at(0x4da076)?)),
            ("BUILDINGCAM", f64::from(image.f32_at(0x4da0c6)?)),
            // Original default registrations4d9810..4d985b and string IDs.
            ("ID_PROMPT_1_OFFSET", f64::from(image.f32_at(0x4dc7ff)?)),
            (
                "ID_TIP_PROMPT_5_TITLE_1",
                f64::from(image.f32_at(0x4dce67)?),
            ),
            (
                "ID_TIP_PROMPT_3_TITLE_1",
                f64::from(image.f32_at(0x4dce3f)?),
            ),
        ] {
            vm.set_number(name, value)?;
        }
        crate::level_catalog::script::register_language(vm,image)?;
        objective_selection::register_constants(vm, image)?;
        for (name, id) in [
            ("GetDifficulty", 3),
            ("GetObjectiveTarget", objective_selection::GET_TARGET_CALLBACK),
            
            ("Pause", 9),
            ("Unpause", 10),
            ("TutorialDisableAllControls", 720),
            ("TutorialDisableAllPicking", 721),
            ("TutorialDisableCameraControl", 722),
            ("TutorialDismissPrompt", 725),
        ] {
            vm.register(name, id)?;
        }
        for (name, value) in [
            ("TUT_NAVIGATOR_PANEL", 103.),
            ("TUT_PCP_OPEN_CLOSE", 76.),
            ("TUT_PCP_TOGGLE_OPTIONS_LEFT", 77.),
            ("TUT_PCP_TOGGLE_OPTIONS_RIGHT", 78.),
            ("TUT_TOGGLE_VIEW", f64::from(image.f32_at(0x4dbe57)?)),
            ("TUT_MPP_GREENBUTTON_6", f64::from(image.f32_at(0x4dbcef)?)),
            ("TUT_MPP_GREENBUTTON_1", f64::from(image.f32_at(0x4dbc8b)?)),
            ("TUT_MPP_GREENBUTTON_3", f64::from(image.f32_at(0x4dbcb3)?)),
            ("TUT_MPP_GREENBUTTON_2", f64::from(image.f32_at(0x4dbc9f)?)),
            ("TUT_MPP_GREENBUTTON_4", f64::from(image.f32_at(0x4dbcc7)?)),
            ("TUT_MPP_GREENBUTTON_5", f64::from(image.f32_at(0x4dbcdb)?)),
            ("TUT_MPP_INFOBUTTON_1", f64::from(image.f32_at(0x4dbd03)?)),
            ("TUT_MPP_INFOBUTTON_2", f64::from(image.f32_at(0x4dbd17)?)),
            ("TUT_MPP_INFOBUTTON_3", f64::from(image.f32_at(0x4dbd2b)?)),
            ("TUT_MPP_INFOBUTTON_4", f64::from(image.f32_at(0x4dbd3f)?)),
            ("TUT_MPP_INFOBUTTON_5", f64::from(image.f32_at(0x4dbd53)?)),
            ("TUT_MPP_BLOCK_LANDRENT", f64::from(image.f32_at(0x4dc14c)?)),
            ("TUT_MPP_BLOCK_LEFT_TOGGLE", f64::from(image.f32_at(0x4dc160)?)),
            ("TUT_MPP_BLOCK_RIGHT_TOGGLE", f64::from(image.f32_at(0x4dc174)?)),
            ("TUT_MPP_BLOCK_BUYBLOCK", f64::from(image.f32_at(0x4dc188)?)),
            ("TUT_BUILD_RIGHTS_BUTTON", f64::from(image.f32_at(0x4dc19c)?)),
            ("TUT_GAME_AUCTION_STARTTIMER", f64::from(image.f32_at(0x4dc1b0)?)),
            ("TUT_AUCTION_TIMER_PANEL",f64::from(image.f32_at(0x4dc110)?)),
            ("TUT_BEGINAUCTION",f64::from(image.f32_at(0x4dc23c)?)),
            ("TUT_ENDAUCTION",f64::from(image.f32_at(0x4dc250)?)),
            ("TUT_AUCTION_50",f64::from(image.f32_at(0x4dc264)?)),
            ("TUT_AUCTION_100",f64::from(image.f32_at(0x4dc278)?)),
            ("TUT_AUCTION_500",f64::from(image.f32_at(0x4dc28c)?)),
            ("TUT_MYGO",f64::from(image.f32_at(0x4dc2c8)?)),
            ("TUT_AUCTION_PASS",f64::from(image.f32_at(0x4dc2b4)?)),
            ("TUT_AUCTION_PULLOUT",f64::from(image.f32_at(0x4dc2a0)?)),
            ("TUT_MPP_BUILDING_LEFT_TOGGLE", f64::from(image.f32_at(0x4dc1d8)?)),
            ("TUT_MPP_BUILDING_RIGHT_TOGGLE", f64::from(image.f32_at(0x4dc1ec)?)),
            ("TUT_HYPERLINK", f64::from(image.f32_at(0x4dc0fc)?)),
            ("TUT_AERIAL_BUSINESS_LOCATOR", f64::from(image.f32_at(0x4dc124)?)),
            ("TUT_AERIAL_LOCATOR_LISTMYBUSINESS", f64::from(image.f32_at(0x4dc138)?)),
            // Native 4dbd75..4dbe20 exposes Strategic View tutorial controls.
            ("TUT_AERIAL_BUTTON_2", f64::from(image.f32_at(0x4dbd7b)?)),
            ("TUT_AERIAL_BUTTON_3", f64::from(image.f32_at(0x4dbd8f)?)),
            ("TUT_AERIAL_BUTTON_4", f64::from(image.f32_at(0x4dbda3)?)),
            ("TUT_AERIAL_INFOBUTTON_1", f64::from(image.f32_at(0x4dbdb7)?)),
            ("TUT_AERIAL_INFOBUTTON_2", f64::from(image.f32_at(0x4dbdcb)?)),
            ("TUT_AERIAL_INFOBUTTON_3", f64::from(image.f32_at(0x4dbddf)?)),
            ("TUT_AERIAL_INFOBUTTON_4", f64::from(image.f32_at(0x4dbdf3)?)),
            ("TUT_AERIAL_INFOBUTTON_5", f64::from(image.f32_at(0x4dbe07)?)),
            ("TUT_AERIAL_INFOBUTTON_6", f64::from(image.f32_at(0x4dbe1b)?)),
            ("TUT_PROMPT_1", 0.), // 4dbf6e uses PUSH imm8 zero.
            // Original4dbfa2..4dc0d4: actual builder widget registrations.
            ("TUT_BUILD_SELECT_TYPE",f64::from(image.f32_at(0x4dbfa8)?)),
            ("TUT_BUILD_SELECT_USE",f64::from(image.f32_at(0x4dbfbc)?)),
            ("TUT_BUILD_SELECT_BACK_DRAG_BUTTON",f64::from(image.f32_at(0x4dbfd0)?)),
            ("TUT_BUILD_SELECT_BACK_DRAG_TEXT",f64::from(image.f32_at(0x4dbfe4)?)),
            ("TUT_BUILD_CHANGE_DESIGN",f64::from(image.f32_at(0x4dbff8)?)),
            ("TUT_BUILD_CHANGE_FLOOR",f64::from(image.f32_at(0x4dc00c)?)),
            ("TUT_BUILD_QUALITY_LOW",f64::from(image.f32_at(0x4dc020)?)),
            ("TUT_BUILD_QUALITY_MED",f64::from(image.f32_at(0x4dc034)?)),
            ("TUT_BUILD_QUALITY_HIGH",f64::from(image.f32_at(0x4dc048)?)),
            ("TUT_BUILD_ACCEPT",f64::from(image.f32_at(0x4dc05c)?)),
            ("TUT_BUILD_CANCEL",f64::from(image.f32_at(0x4dc070)?)),
            ("TUT_BUILD_PANEL",f64::from(image.f32_at(0x4dc084)?)),
            ("TUT_END_BUILD",f64::from(image.f32_at(0x4dc098)?)),
            ("TUT_BUILD_BACKREGION0",f64::from(image.f32_at(0x4dc0ac)?)),
            ("TUT_BUILD_BACKREGION1",f64::from(image.f32_at(0x4dc0c0)?)),
            ("TUT_BUILD_BACKREGION2",f64::from(image.f32_at(0x4dc0d4)?)),
            ("TUT_MPP_BUILDING_LEFT_TOGGLE",f64::from(image.f32_at(0x4dc1d8)?)),
            ("TUT_MPP_BUILDING_RIGHT_TOGGLE",f64::from(image.f32_at(0x4dc1ec)?)),
            ("TUT_MPP_BUILDING_REFURBISH",f64::from(image.f32_at(0x4dc200)?)),
            ("TUT_MPP_BUILDING_DEMOLISH",f64::from(image.f32_at(0x4dc214)?)),
            ("TUT_MPP_BUILDING_REFURBISH_LIST",f64::from(image.f32_at(0x4dc228)?)),
            ("TUT_BAP_PLUS_BUTTON_1",f64::from(image.f32_at(0x4dc2dc)?)),
            ("TUT_BAP_MINUS_BUTTON_1",f64::from(image.f32_at(0x4dc2f0)?)),
            ("TUT_BAP_PLUS_BUTTON_2",f64::from(image.f32_at(0x4dc304)?)),
            ("TUT_BAP_MINUS_BUTTON_2",f64::from(image.f32_at(0x4dc318)?)),
            ("TUT_BAP_OK_BUTTON",f64::from(image.f32_at(0x4dc3cc)?)),
        ] {
            vm.set_number(name, value)?;
        }
        // The active-level result hubs are loaded into this same VM by4b1800.
        // Their host effects remain ordered commands for the presentation owner.
        for (name, id, shape) in [
            ("C_ResultFlyby", 700, "S"),
            ("C_ResultCameraMode", 701, "n"),
            ("C_ResultFireworks", 702, "n"),
            ("C_ResultCutSceneIdentifier", 703, "n"),
            ("C_ResultCredits", 706, "n"),
            ("C_ResultEnd", 707, "nn"),
        ] {
            vm.register_coerced(name, id, shape)?;
        }
        vm.register("C_ResultEnableBypass", 704)?;
        vm.register("C_ResultDisableBypass", 705)?;
        // Original4db346 registers Random.ZeroTo at4db450. The result host
        // consumes its separate static RNG node (93aee8) in the same VM call.
        vm.register_coerced("C_RandomZeroTo", 708, "n")?;
        vm.register_coerced("TutorialAddArrow", 723, "nn")?;
        vm.register_coerced("CreatePrompt", 5, "ppnn")?;
        vm.register_coerced("TutorialCreateMessage", 724, "ppnn")?;
        vm.register_coerced("TutorialEnableControl", 726, "n")?;
        vm.register_coerced("TutorialDisableControl", 727, "n")?;
        vm.register_coerced("TutorialEnableControlAndChildren", 735, "n")?;
        vm.register_coerced("TutorialSetBuildType", 736, "N")?;
        vm.register_coerced("TutorialSetBuildUse", 741, "N")?;
        vm.register("TutorialCreateEventList", 737)?;
        vm.register_coerced("TutorialForceBid", 738, "nn")?;
        vm.register_coerced("TutorialForcePullOut", 739, "N")?;
        vm.register_coerced("TutorialForcePass", 740, "N")?;
        vm.register_coerced("TutorialSetCameraMode", 728, "nn")?;
        vm.register("TutorialSetCityView", 729)?;
        vm.register("TutorialSetPlanView", 730)?;
        // Native 4dc4a7, 4dc5ec and 4dc605 bind the three Tutorial 2
        // position/picking methods to 4db740, 4db970 and 4db9e0.
        vm.register_coerced("TutorialPositionPrompt", 731, "NN")?;
        vm.register("TutorialEnableAllPicking",746)?;
        vm.register_coerced("TutorialEnableBlockPicking", 732, "N")?;
        vm.register_coerced("TutorialEnableBuildingPicking", 733, "N")?;
        //4dac40: two tonumber arguments and a lua_isnumber-gated third.
        // Missing/invalid third arguments preserve the original flag.
        vm.register_coerced("SetObjective", 4, "nnN")?;
        vm.execute(
            "Game bindings",
            b"Game={GetObjectiveTarget=GetObjectiveTarget,GetDifficulty=GetDifficulty,SetObjective=SetObjective,Pause=Pause,Unpause=Unpause,StartFireworks=C_ResultFireworks,SetCutSceneIdentifier=C_ResultCutSceneIdentifier,EnableCutSceneBypass=C_ResultEnableBypass,DisableCutSceneBypass=C_ResultDisableBypass,SetCredits=C_ResultCredits,End=C_ResultEnd,performance=2}; Camera={SetFlybyMode=C_ResultFlyby,SetMode=C_ResultCameraMode}; Random={ZeroTo=C_RandomZeroTo}; Interface={ForcePlayerToBid=TutorialForceBid,ForcePlayerToPass=TutorialForcePass,ForcePlayerToPullOut=TutorialForcePullOut,CreatePrompt=CreatePrompt,CreateEventList=TutorialCreateEventList,CreateMessage=TutorialCreateMessage,DismissPrompt=TutorialDismissPrompt,AddArrow=TutorialAddArrow,DisableAllControls=TutorialDisableAllControls,DisableAll3DPicking=TutorialDisableAllPicking,EnableAll3DPicking=TutorialEnableAllPicking,DisableCameraControl=TutorialDisableCameraControl,EnableControl=TutorialEnableControl,EnableControlAndChildren=TutorialEnableControlAndChildren,DisableControl=TutorialDisableControl,PositionPrompt=TutorialPositionPrompt,EnableBlockPicking=TutorialEnableBlockPicking,EnableBuildingPicking=TutorialEnableBuildingPicking,SetTutorialType=TutorialSetBuildType,SetTutorialUse=TutorialSetBuildUse}",
            &mut |_, _| Err("unexpected binding callback".into()),
        )?;
        vm.register_script_loader("C_RunScript")?;
        Ok(Self {
            selected_objective: objective_selection::Selection::default(),
            startup_settings,
            resident_settings,
            map,
            properties,
            commodities,
            businesses,
        })
    }

    // Keep the live event list separate so a ticking callback can reborrow it.
    #[allow(clippy::too_many_arguments)]
    pub fn handle(
        &self,
        id: u32,
        arguments: &[Value],
        state: &mut State<'_>,
        events: &mut crate::script_events::Events,
        context: &Context<'_, '_>,
        geometry: &mut impl crate::building_pool::GeometryBinding,
        hooks: &mut impl Hooks,
    ) -> Result<Option<f64>, String> {
        // A nested Lua script can change the clock between any two host calls.
        let mut live_context = *context;
        live_context.update_clock(state.clock);
        let context = &live_context;
        if matches!(id, 744 | 745) {
            return calendar_property(id, arguments, state.clock);
        }
        if self.startup_settings.contains(id) {
            return self.startup_settings.handle(id, arguments, state.settings);
        }
        if self.resident_settings.contains(id) {
            return self.resident_settings.handle(
                id,
                arguments,
                &mut crate::resident_settings::State {
                    preferences: &mut state.configuration.preferences,
                    groups: &mut state.configuration.preference_groups,
                    welfare: state.welfare,
                },
            );
        }
        if self.commodities.contains(id) {
            return self.commodities.handle(id, arguments, state.commodities);
        }
        if self.businesses.contains(id) {
            return self
                .businesses
                .handle(id, arguments, state.catalog, state.commodities);
        }
        if self.properties.contains(id) {
            return self.properties.handle(
                id,
                arguments,
                &mut crate::initial_conditions::State {
                    num_cars: &mut state.settings.num_cars,
                    categories: state.categories,
                    definitions: state.definitions,
                    player_money: state.player_money,
                },
            );
        }
        if self.map.contains(id) {
            // Reborrow current resident storage for every map command. A map
            // mutation after People.Create therefore invalidates real residents.
            let mut utility_owners = [0; 4];
            for (result, category) in utility_owners.iter_mut().zip([2, 35, 39, 42]) {
                let owner = state.categories[category][5] as i8;
                if owner != 0 {
                    let index = usize::try_from(i32::from(owner) - 1)
                        .map_err(|_| "negative utility owner")?;
                    *result = state
                        .players
                        .get(index)
                        .ok_or("utility owner leaves player table")?
                        .map_or(0, |player| player.identity);
                }
            }
            let mut map = crate::map_script::State {
                world: business_binding::World {
                    buildings: state.buildings,
                    businesses: state.businesses,
                    categories: state.categories,
                    category_base: context.category_base,
                    catalog: &state.catalog.info,
                    available: state.available,
                    availability_cursor: state.availability_cursor,
                    players: state.players,
                    resident_count: state.residents.active_count as usize,
                },
                runtime: business_binding::Runtime {
                    building_base: context.building_base,
                    products: &state.catalog.products,
                    commodities: &state.commodities.rows,
                    residents: &mut state.residents.nodes,
                    timestamp: context.timestamp,
                },
                image: context.image,
                hour: context.hour,
                discount: context.discount,
                date: context.date,
                ownership_timestamp: context.ownership_timestamp,
                utility_owners,
                notifications: context.notifications,
                construction_components: *state.construction_components,
            };
            let result = self.map.handle(id, arguments, &mut map, geometry, hooks);
            // Preserve partial side effects even if a later required host call fails.
            *state.construction_components = map.construction_components;
            return result;
        }
        match id {
            738..=740 => {
                if let Some((producer,participant,amount))=forced_auction(id,arguments){
                    hooks.command(Command::ForcedAuction{producer,participant,amount},&[])?;
                }
                Ok(None)
            }
            737 => {
                hooks.command(Command::CreateEventList, &[])?;
                Ok(None)
            }
            734 => {
                let [Value::Number(index), Value::Number(owner)] = arguments else {
                    return Err("Business.SetOwner expects index and owner".into());
                };
                let index = usize::try_from(crate::economy::ftol_low_word(*index)).map_err(|_| "negative business index")?;
                let owner = crate::economy::ftol_low_word(*owner) as u8;
                let business = state.businesses.get_mut(index).ok_or("business index leaves pool")?;
                let building = usize::try_from(i16::from_le_bytes(business[8..10].try_into().unwrap())).map_err(|_| "negative business building")?;
                let record = state.buildings.get_mut(building).ok_or("business building leaves pool")?;
                set_business_owner(business, record, owner, context.ownership_timestamp,
                    context.accounting_enabled, state.player_accounts,
                    |record| hooks.building_appearance(building, record))?;
                Ok(None)
            }
            735 => {
                hooks.command(Command::TutorialEnableControlAndChildren, arguments)?;
                Ok(None)
            }
            736 => {
                if let Some(value) = tutorial_build_integer(arguments) {
                    hooks.command(Command::TutorialBuildType,
                        &[Value::Number(f64::from(value))])?;
                }
                Ok(None)
            }
            741 => {
                if let Some(value) = tutorial_build_integer(arguments) {
                    hooks.command(Command::TutorialBuildUse,
                        &[Value::Number(f64::from(value))])?;
                }
                Ok(None)
            }
            objective_selection::GET_TARGET_CALLBACK => self.selected_objective.target(),
            3 => Ok(Some(f64::from(context.difficulty))),
            4 | 5 => {
                if id == 4 {
                    objective_selection::set_objective(state.objective, arguments)?;
                }
                hooks.command(
                    match id {
                        4 => Command::Objective,
                        _ => Command::Prompt,
                    },
                    arguments,
                )?;
                Ok(None)
            }
            9 | 10 => {
                if id == 9 {
                    state.pause.pause(0, |event, _| hooks.pause_timers(event))?;
                } else {
                    state
                        .pause
                        .unpause(0, |event, _| hooks.pause_timers(event))?;
                }
                Ok(None)
            }
            11 => {
                let (frequency, _) = hooks.calendar_timers();
                Ok(Some(f64::from(frequency.calendar_timer.is_some())))
            }
            12 => {
                let [Value::Number(value)] = arguments else {
                    return Err("invalid calendar frequency argument".into());
                };
                let (frequency, tasks) = hooks.calendar_timers();
                frequency.set(tasks, *value)?;
                Ok(None)
            }
            13 => {
                hooks.command(Command::PlaySound, arguments)?;
                Ok(None)
            }
            700..=707 => {
                let command = match id {
                    700 => Command::ResultFlyby,
                    701 => Command::ResultCameraMode,
                    702 => Command::ResultFireworks,
                    703 => Command::ResultCutSceneIdentifier,
                    704 => Command::ResultEnableBypass,
                    705 => Command::ResultDisableBypass,
                    706 => Command::ResultCredits,
                    707 => Command::ResultEnd,
                    _ => unreachable!(),
                };
                hooks.command(command, arguments)?;
                Ok(None)
            }
            708 => Err("Random.ZeroTo requires its live result RNG owner".into()),
            746=>{hooks.command(Command::TutorialEnableAllPicking,arguments)?;Ok(None)}
            720..=733 => {
                let command = match id {
                    720 => Command::TutorialDisableAllControls,
                    721 => Command::TutorialDisableAllPicking,
                    722 => Command::TutorialDisableCameraControl,
                    723 => Command::TutorialArrow,
                    724 => Command::TutorialMessage,
                    725 => Command::TutorialDismissPrompt,
                    726 => Command::TutorialEnableControl,
                    727 => Command::TutorialDisableControl,
                    728 => Command::TutorialCameraMode,
                    729 => Command::TutorialCityView,
                    730 => Command::TutorialPlanView,
                    731 => Command::TutorialPositionPrompt,
                    732 => Command::TutorialEnableBlockPicking,
                    733 => Command::TutorialEnableBuildingPicking,
                    _ => unreachable!(),
                };
                hooks.command(command, arguments)?;
                Ok(None)
            }
            14=>Ok(Some(crate::population_script::number_in_city(state.residents.active_count))),
            7 => Ok(Some(f64::from(context.total_clock))),
            8 => {
                let [
                    Value::Number(reference),
                    Value::Number(delay),
                    Value::Number(argument),
                ] = arguments
                else {
                    return Err("invalid event admission arguments".into());
                };
                events.append(crate::script_events::Event {
                    reference: *reference as u32,
                    remaining: *delay as u32,
                    argument: *argument as i32,
                });
                Ok(None)
            }
            6 => {
                let hour = usize::try_from(context.hour).map_err(|_| "negative population hour")?;
                let commodities = state.commodities.resident_inputs();
                let personal =
                    state
                        .configuration
                        .inputs(&commodities, state.commodities.memberships, hour);
                let inputs = crate::economy::ResidentCreationInputs {
                    personal: &personal,
                    preference_groups: &state.configuration.preference_groups,
                    models: context.models,
                    countdown_period: state.settings.itchy_counter,
                };
                crate::population_script::Population {
                    residents: state.residents,
                    inputs: &inputs,
                    state: state.creation,
                    categories: state.categories,
                    auxiliary_template: context.auxiliary_template,
                    creation_events_enabled: context.notifications,
                }
                .create(
                    crate::population_script::Request::from_arguments(arguments)?,
                    &mut |index, population| {
                        let resident = &population.residents.nodes[index];
                        let target = population
                            .residents
                            .nodes
                            .get(usize::from(resident.pool_index()))
                            .ok_or_else(|| {
                                crate::economy::ResidentCreationError::InvalidInputs(
                                    "created resident identity outside pool".into(),
                                )
                            })?;
                        state
                            .ai_demand
                            .resident_changed(target, 1, 0)
                            .map_err(Into::into)
                    },
                )
                .map_err(|e| format!("population creation {e:?}"))?;
                Ok(None)
            }
            _ => Err("unrecovered level callback".into()),
        }
    }
}

#[cfg(test)]
mod registration_tests {
    #[test]
    #[ignore="requires pinned original forced-auction Lua observations"]
    fn matches_native_forced_auction_bindings(){
        use crate::{lua4::Value,player_manager::{put,word},ai_messages::offers::auction};
        let bytes=std::fs::read(std::env::var("OPENMT2001_NATIVE_FORCED_AUCTION_BINDINGS").unwrap()).unwrap();
        assert_eq!(&bytes[..8],b"MTFORA01");assert_eq!(word(&bytes,8),0x037f);
        assert_eq!(bytes.len(),16+word(&bytes,12) as usize*40);
        for (case,row) in bytes[16..].chunks_exact(40).enumerate(){
            let id=word(row,0);
            let player=f64::from_le_bytes(row[4..12].try_into().unwrap());
            let delta=f64::from_le_bytes(row[12..20].try_into().unwrap());
            let args=if row[20]!=0{vec![Value::Number(player),Value::Number(delta)]}else{vec![Value::Nil]};
            let actual=super::forced_auction(id,&args);
            assert_eq!(actual.is_some(),word(row,28)!=0,"case{case} numeric gate");
            if let Some((producer,participant,amount))=actual{
                let mut state=[0;128];put(&mut state,0x6c,word(row,24));
                let descriptor=auction::forced_action_descriptor(producer,participant,amount,&state,[0;32]).unwrap();
                assert_eq!(word(&descriptor,24),word(row,32),"case{case} participant");
                if id==738{assert_eq!(word(&descriptor,28),word(row,36),"case{case} current bid addition");}
                assert_eq!(word(&descriptor,20),match id{738=>0,739=>2,_=>1});
            }
        }
    }
    #[test]
    #[ignore = "requires original business and commodity settings"]
    fn complete_registration_preserves_business_settings() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&executable).unwrap();
        let mut vm = crate::lua4::Runtime::new().unwrap();
        let bindings = super::Bindings::register(&mut vm, &image).unwrap();
        assert_eq!(vm.number("DELI").unwrap(), Some(2.));
        assert_eq!(vm.number("LANDSCAPE").unwrap(), Some(44.));
        let mut businesses = crate::business_script::State::from_v14(&image).unwrap();
        let mut commodities = crate::commodity_script::State::from_v14(&image, [0; 5]).unwrap();
        vm.script_buffer("commoditysettings", &read("OPENMT2001_COMMODITY_SETTINGS"))
            .unwrap();
        vm.script_buffer("businesssettings", &read("OPENMT2001_BUSINESS_SETTINGS"))
            .unwrap();
        vm.execute(
            "nested original settings",
            b"C_RunScript('commoditysettings'); C_RunScript('businesssettings')",
            &mut |id, args| {
                if bindings.commodities.contains(id) {
                    bindings.commodities.handle(id, args, &mut commodities)
                } else {
                    bindings
                        .businesses
                        .handle(id, args, &mut businesses, &commodities)
                }
            },
        )
        .unwrap();
        assert_eq!(
            f32::from_le_bytes(
                businesses.info.row(2).unwrap()[0x58..0x5c]
                    .try_into()
                    .unwrap()
            ),
            3.
        );
        let properties = read("OPENMT2001_NATIVE_BUSINESS_PROPERTIES");
        assert_eq!(&properties[..8], b"MTBIP001");
        for (index, row) in businesses.info.rows().iter().enumerate() {
            let mut actual = *row;
            // This independent native property fixture omits AddCommodity.
            actual[0x4c] = 0;
            actual[0x70..0x74].fill(0);
            let at = 16 + index * 120;
            assert_eq!(
                u32::from_le_bytes(properties[at..at + 4].try_into().unwrap()),
                index as u32
            );
            assert_eq!(
                &actual,
                &properties[at + 4..at + 120],
                "complete registration native business row{index}"
            );
        }
    }
}

/// Original4d88f1 Business:SetOwner tail. Renderer effects are host callbacks;
/// the accounting flag is the independent native8b22dc lifecycle gate.
pub fn set_business_owner(
    business: &mut [u8; 0x220], building: &mut [u8; 0x394], owner: u8,
    ownership_timestamp: u32, accounting_enabled: bool,
    players: &mut [crate::economy::PlayerAccount],
    mut appearance: impl FnMut(&mut [u8; 0x394]) -> Result<(), String>,
) -> Result<(), String> {
    building[0x2d] = owner;
    building[0x30..0x34].copy_from_slice(&ownership_timestamp.to_le_bytes());
    let refresh = building[0x4c..0x50] == 1u32.to_le_bytes() &&
        (building[0x1c..0x20] == u32::MAX.to_le_bytes() ||
        u32::from_le_bytes(building[0x24..0x28].try_into().unwrap()) <= 1);
    if refresh { appearance(building)?; }
    crate::economy::transfer_business_owner(business, building[..64].try_into().unwrap(), owner, accounting_enabled, players)?;
    if refresh { appearance(building)?; }
    Ok(())
}

#[cfg(test)]
mod tutorial_owner_tests {
    #[test]
    #[ignore = "requires tools/native_tutorial_build_type.py original v1.4 fixture"]
    fn original_tutorial_build_type_numeric_binding() {
        let bytes=std::fs::read("work/economy/native-tutorial-build-type.bin").unwrap();
        assert_eq!(&bytes[..8],b"MTBTYP01");
        assert_eq!(u16::from_le_bytes(bytes[8..10].try_into().unwrap()),0x037f);
        let count=u16::from_le_bytes(bytes[10..12].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(),12+count*13);
        let mut runtime=crate::lua4::Runtime::new().unwrap();
        runtime.register_coerced("ProbeBuildType",736,"N").unwrap();
        for (index,case) in bytes[12..].chunks_exact(13).enumerate() {
            let value=f64::from_le_bytes(case[..8].try_into().unwrap());
            let numeric=case[8]!=0;
            let expected=i32::from_le_bytes(case[9..13].try_into().unwrap());
            let source=if numeric {format!("ProbeBuildType({value:.17})")}else{"ProbeBuildType(nil)".into()};
            let mut observed=None;
            runtime.execute("native build type binding",source.as_bytes(),&mut |id,args|{
                assert_eq!(id,736);
                observed=super::tutorial_build_integer(args);
                Ok(None)
            }).unwrap();
            assert_eq!(observed,numeric.then_some(expected),"native SetTutorialType case{index}");
        }
    }
    #[test]
    #[ignore = "requires tools/native_tutorial_owner.py original v1.4 fixture"]
    fn original_tutorial_owner_records_counters_and_callbacks() {
        let bytes = std::fs::read("work/economy/native-tutorial-owner.bin").unwrap();
        assert_eq!(&bytes[..8], b"MTUTOWN0");
        let mut at = 8;
        let count = u32::from_le_bytes(bytes[at..at+4].try_into().unwrap()); at += 4;
        assert_eq!(count, 40);
        for case in 0..count {
            let flag = bytes[at]; let owner = bytes[at+1]; let calls = bytes[at+2]; at += 3;
            let mut business: [u8; 0x220] = bytes[at..at+0x220].try_into().unwrap(); at += 0x220;
            let mut building: [u8; 0x394] = bytes[at..at+0x394].try_into().unwrap(); at += 0x394;
            let expected_business = &bytes[at..at+0x220]; at += 0x220;
            let expected_building = &bytes[at..at+0x394]; at += 0x394;
            let expected_counters = [i32::from_le_bytes(bytes[at..at+4].try_into().unwrap()), i32::from_le_bytes(bytes[at+4..at+8].try_into().unwrap())]; at += 8;
            let mut players = vec![crate::economy::PlayerAccount::from_runtime(&[0; 0x1b8]).unwrap(); 2];
            for (i,p) in players.iter_mut().enumerate() { p.housing_counter = (i as i32 + 1) * 100; }
            let mut refreshes = 0;
            super::set_business_owner(&mut business, &mut building, owner, 0x12345678, flag != 0,
                &mut players, |_| { refreshes += 1; Ok(()) }).unwrap();
            assert_eq!(business, expected_business, "case {case} business");
            assert_eq!(building, expected_building, "case {case} building");
            assert_eq!(players.iter().map(|p|p.housing_counter).collect::<Vec<_>>(), expected_counters, "case {case} counters");
            assert_eq!(refreshes, calls, "case {case} appearance");
        }
        assert_eq!(at, bytes.len());
    }
}

#[cfg(test)]
mod calendar_binding_tests {
    use super::*;
    #[test]
    fn tagged_calendar_is_immediate_signed_coerced_nested_and_partial() {
        let mut vm = Runtime::new().unwrap();
        register_calendar(&mut vm).unwrap();
        vm.register_script_loader("C_RunScript").unwrap();
        vm.script_buffer("inner.lua", b"Calendar.era=ERA_1980; inner=Calendar.era").unwrap();
        let mut clock = crate::clock::Clock {calendar:[13,7,3,0xabcdef,77],timing:[11,22,33,44,55,66]};
        vm.execute("get", b"h=Calendar.hour; y=Calendar.year; e=Calendar.era", &mut |id,args| calendar_property(id,args,&mut clock)).unwrap();
        assert_eq!(vm.number("h").unwrap(),Some(13.));
        assert_eq!(vm.number("y").unwrap(),Some(7.));
        assert_eq!(vm.number("e").unwrap(),Some(3.));
        for (input,expected) in [("9.9",9),("-9.9",-9),("'12.7'",12),("'bad'",0),("nil",0),("{}",0),("function() end",0),("4294967301",5)] {
            vm.execute("coerce",format!("Calendar.hour={input}; observed=Calendar.hour").as_bytes(),&mut |id,args|calendar_property(id,args,&mut clock)).unwrap();
            assert_eq!(vm.number("observed").unwrap(),Some(expected as f64));
            assert_eq!(&clock.calendar[1..], &[7,3,0xabcdef,77]);
            assert_eq!(clock.timing,[11,22,33,44,55,66]);
        }
        clock.calendar[1]=u32::MAX;
        vm.execute("host change", b"signed=Calendar.year; Calendar.era=3; first=Calendar.era; C_RunScript('inner.lua'); second=Calendar.era; Calendar.era=7; third=Calendar.era", &mut |id,args|calendar_property(id,args,&mut clock)).unwrap();
        for (key,value) in [("signed",-1.),("first",3.),("inner",5.),("second",5.),("third",7.)] {assert_eq!(vm.number(key).unwrap(),Some(value));}
        assert!(vm.execute("partial error",b"Calendar.hour=21; missing_calendar_test_function()",&mut |id,args|calendar_property(id,args,&mut clock)).is_err());
        assert_eq!(clock.calendar[0],21);
        assert_eq!(&clock.calendar[3..],&[0xabcdef,77]);
    }
    #[test]
    #[ignore = "requires OPENMT2001_EXE original binding constants; registration only"]
    fn full_registration_attaches_calendar_frequency_without_property_dispatch() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&bytes).unwrap();
        let mut vm = Runtime::new().unwrap();
        // register executes all initialization chunks with an error-only host.
        // Ordinary Calendar.SetFrequency assignment would invoke that host.
        let _bindings = Bindings::register(&mut vm, &image).unwrap();
        let mut calls = Vec::new();
        vm.execute("frequency closure", b"Calendar.SetFrequency(99,'0.04')", &mut |id,args| {
            calls.push((id,args.to_vec()));
            match id {11=>Ok(Some(1.)),12=>Ok(None),_=>Err("frequency routed to property setter".into())}
        }).unwrap();
        assert_eq!(calls[0].0,11);
        assert_eq!(calls[1],(12,vec![Value::Number(0.04)]));
        for (name,value) in [("ERA_1960",3.),("ERA_1980",5.),("GAME_HOUR",625.),("YEAR_09",9.),("HOUR_23",23.)] {
            assert_eq!(vm.number(name).unwrap(),Some(value));
        }
    }

    #[test]
    fn era_callback_partial_error_and_clock_export_cold_readback() {
        let mut vm=Runtime::new().unwrap();
        register_calendar(&mut vm).unwrap();
        let mut clock=crate::clock::Clock{calendar:[1,0,3,0x12345678,99],timing:[100,200,300,400,500,600]};
        let mut saw_script=false;
        crate::era::advance(3,&mut clock,|call,clock| {
            if matches!(call,crate::era::Call::Script{..}) {
                saw_script=true;
                vm.execute("era callback",b"Calendar.year=2; y1=Calendar.year; Calendar.year=7; y2=Calendar.year; Calendar.hour=4; h1=Calendar.hour; Calendar.hour=13; h2=Calendar.hour",&mut |id,args|calendar_property(id,args,clock))?;
            }
            Ok(())
        }).unwrap();
        assert!(saw_script);
        for (name,value) in [("y1",2.),("y2",7.),("h1",4.),("h2",13.)] {assert_eq!(vm.number(name).unwrap(),Some(value));}
        assert_eq!(clock.calendar,[13,7,3,0x12345678,99]);
        // This is the real save writer/reader clock path, not a synthetic table.
        let mut categories=[[0;380];52];
        let mut saved=crate::save::writer::State {
            compatibility:&[0;40],level:500002,difficulty:0,current_cash:0.,players:&[],
            businesses:&[],buildings:&[],building_base:0,building_sentinel_next:0,
            categories:&mut categories,residents:&[],cameras:&[[0;196];2],
            calendar:clock.calendar,timing:clock.timing,pending:&[0;3588],resident_accounts:&[],
            scheduled:&[],auction:None,objective:&[0;60],business_info:&[[0;116];45],
            commodities:&[[0;28];32],history:&[0;0x10f44],game_mode:0,
        };
        let bytes=crate::save::writer::encode(&mut saved,0x600,[0;4]).unwrap();
        let mut cold=crate::save::SaveFile::parse(&bytes).unwrap().clock().unwrap();
        assert_eq!(cold,clock);
        vm.execute("cold clock getter",b"cold_hour=Calendar.hour; Calendar.year=-2.9; cold_year=Calendar.year",&mut |id,args|calendar_property(id,args,&mut cold)).unwrap();
        assert_eq!(vm.number("cold_hour").unwrap(),Some(13.));
        assert_eq!(vm.number("cold_year").unwrap(),Some(-2.));
        assert_eq!(cold.calendar[1],(-2i32) as u32);
        assert_eq!(cold.timing,clock.timing);
    }

}
