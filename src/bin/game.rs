//! Native boot host. Shared simulation and asset modules do not depend on it.
// Session dependencies that moved into the library (#99); kept nameable as
// crate::<module> in the binary.
use openmt2001::{
    build_panel, campaign_store, compatibility, human_dialog, result_controller, save_menu,
    session::business_operations, skirmish_maps, tutorial_two_interaction,
};
// Panel and report state that moved into the library (#99).
#[cfg(test)]
use openmt2001::{auction_controls, auction_labels};
use openmt2001::{city_people, player_info};
// The acyclic interface layer lives in crates/frontend (#99); kept nameable
// as crate::<module> in the binary.
use openmt_frontend::{
    auction_audio, auction_bridge, auction_notice, auction_panel, auction_scene, audio_bus,
    block_information, block_sidewalk_labels, board_selection, business_adjustment, business_edit,
    business_view, cash_arrow, category_distance, chat_panel, chimney_smoke_scene, city_ownership,
    city_prestige, city_pulse_scene, city_rights, comparison, cursor, desktop_quit, dust_scene,
    era_audio, event_list, game_options, gameplay_sound, locator, market_share_panel, modal_frame,
    navigation, notice_panel, object_scene, ownership_badges, pause_banner, people_scene,
    preferred_token, present, profile, profile_settings, prompt, result_presentation,
    save_appearance, save_dialog, scenarios, selection, settings_menu, static_detail, stats_panel,
    strategic_scene, tooltip, transport_audio, transport_scene, tree_scene, tutorial_visual, ui,
    ui_text, utility_report, vehicle_scene,
};
#[cfg(test)]
use openmt_frontend::{music_gain, saved_current_player};
#[path = "frontend/app.rs"]
mod app;
#[cfg(test)]
#[path = "frontend/building_models_tests.rs"]
mod building_models_tests;
#[path = "frontend/card_panel.rs"]
pub mod card_panel;
#[path = "frontend/headless.rs"]
mod headless;
#[path = "frontend/intro.rs"]
mod intro;
#[path = "frontend/live_host.rs"]
mod live_host;
#[path = "frontend/load.rs"]
mod load;
#[path = "frontend/menu.rs"]
mod menu;
#[path = "frontend/navigator.rs"]
mod navigator;
#[path = "frontend/process_dispatch.rs"]
mod process_dispatch;
#[path = "frontend/session_save.rs"]
pub mod session_save;
#[path = "frontend/singleplayer.rs"]
mod singleplayer;
#[path = "frontend/skirmish_roster.rs"]
mod skirmish_roster;
#[path = "frontend/skirmish_setup.rs"]
mod skirmish_setup;
#[path = "frontend/time_panel.rs"]
pub mod time_panel;
fn main() -> std::process::ExitCode {
    if std::env::args().nth(1).as_deref() == Some("headless") {
        std::process::exit(headless::main(std::env::args().skip(2)));
    }
    match app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(all(test, feature = "capture"))]
use openmt_frontend::capture_test;
#[path = "frontend/fresh_city.rs"]
pub mod fresh_city;

#[cfg(test)]
#[path = "frontend/session_validation.rs"]
mod session_validation;

#[path = "frontend/consumer_polls.rs"]
pub mod consumer_polls;

#[path = "frontend/build_view.rs"]
mod build_view;

#[cfg(test)]
#[path = "frontend/rental_turnover.rs"]
mod rental_turnover;

#[cfg(test)]
#[path = "frontend/scenario_three_play.rs"]
mod scenario_three_play;

// Tests of interface-layer modules (crates/frontend) that need the Host, the
// Session harness or intro, so they build with the game binary (#99).
#[cfg(test)]
#[path = "frontend/era_audio_host_tests.rs"]
mod era_audio_host_tests;
#[cfg(test)]
#[path = "frontend/locator_host_tests.rs"]
mod locator_host_tests;
#[cfg(test)]
#[path = "frontend/property_panel_test.rs"]
mod locator_property_panel_tests;
#[cfg(test)]
#[path = "frontend/transport_audio_host_tests.rs"]
mod transport_audio_host_tests;
