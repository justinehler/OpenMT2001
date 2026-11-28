//! Native boot host. Shared simulation and asset modules do not depend on it.
// Session dependencies that moved into the library (#99); kept nameable as
// crate::<module> in the binary.
use openmt2001::{campaign_store, compatibility, result_controller, save_menu, skirmish_maps};
#[path = "frontend/app.rs"]
mod app;
#[path = "frontend/auction_bridge.rs"]
pub mod auction_bridge;
#[path = "frontend/auction_controls.rs"]
pub mod auction_controls;
#[path = "frontend/auction_labels.rs"]
pub mod auction_labels;
#[path = "frontend/auction_notice.rs"]
pub mod auction_notice;
#[path = "frontend/auction_panel.rs"]
pub mod auction_panel;
#[path = "frontend/auction_scene.rs"]
pub mod auction_scene;
#[path = "frontend/block_sidewalk_labels.rs"]
pub mod block_sidewalk_labels;
#[path = "frontend/building_models.rs"]
pub mod building_models;
#[path = "frontend/business_adjustment.rs"]
pub mod business_adjustment;
#[path = "frontend/business_edit.rs"]
mod business_edit;
#[path = "frontend/business_operations.rs"]
mod business_operations;
#[path = "frontend/business_view.rs"]
mod business_view;
#[path = "frontend/card_panel.rs"]
pub mod card_panel;
#[path = "frontend/category_distance.rs"]
pub mod category_distance;
#[path = "frontend/city_pulse_scene.rs"]
pub mod city_pulse_scene;
#[path = "frontend/cursor.rs"]
mod cursor;
#[path = "frontend/dust_scene.rs"]
pub mod dust_scene;
#[path = "frontend/event_list.rs"]
pub mod event_list;
#[path = "frontend/game_options.rs"]
mod game_options;
#[path = "frontend/human_dialog.rs"]
pub mod human_dialog;
#[path = "frontend/intro.rs"]
mod intro;
#[path = "frontend/live_host.rs"]
mod live_host;
#[path = "frontend/load.rs"]
mod load;
#[path = "frontend/locator.rs"]
mod locator;
#[path = "frontend/market_share_panel.rs"]
pub mod market_share_panel;
#[path = "frontend/menu.rs"]
mod menu;
#[path = "frontend/navigation.rs"]
mod navigation;
#[path = "frontend/navigator.rs"]
mod navigator;
#[path = "frontend/notice_panel.rs"]
pub mod notice_panel;
#[path = "frontend/notifications.rs"]
pub mod notifications;
#[path = "frontend/player_info.rs"]
mod player_info;
#[path = "frontend/present.rs"]
mod present;
#[path = "frontend/process_dispatch.rs"]
mod process_dispatch;
#[path = "frontend/profile.rs"]
mod profile;
#[path = "frontend/profile_settings.rs"]
mod profile_settings;
#[path = "frontend/prompt.rs"]
pub mod prompt;
#[path = "frontend/save_appearance.rs"]
pub mod save_appearance;
#[path = "frontend/save_dialog.rs"]
mod save_dialog;
#[path = "frontend/scenarios.rs"]
mod scenarios;
#[path = "frontend/selection.rs"]
mod selection;
#[path = "frontend/session_save.rs"]
pub mod session_save;
#[path = "frontend/settings_menu.rs"]
mod settings_menu;
#[path = "frontend/singleplayer.rs"]
mod singleplayer;
#[path = "frontend/skirmish_roster.rs"]
mod skirmish_roster;
#[path = "frontend/skirmish_setup.rs"]
mod skirmish_setup;
#[path = "frontend/static_detail.rs"]
pub mod static_detail;
#[path = "frontend/stats_panel.rs"]
pub mod stats_panel;
#[path = "frontend/strategic_scene.rs"]
mod strategic_scene;
#[path = "frontend/time_panel.rs"]
pub mod time_panel;
#[path = "frontend/tooltip.rs"]
mod tooltip;
#[path = "frontend/tutorial_two_interaction.rs"]
pub mod tutorial_two_interaction;
#[path = "frontend/ui.rs"]
mod ui;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}

#[cfg(all(test, feature = "capture"))]
#[path = "frontend/capture_test.rs"]
mod capture_test;
#[path = "frontend/fresh_city.rs"]
pub mod fresh_city;

#[cfg(test)]
#[path = "frontend/session_validation.rs"]
mod session_validation;

#[path = "frontend/audio_bus.rs"]
mod audio_bus;
#[path = "frontend/era_audio.rs"]
pub mod era_audio;
#[path = "frontend/gameplay_sound.rs"]
pub mod gameplay_sound;
#[path = "frontend/music_gain.rs"]
mod music_gain;
#[path = "frontend/object_scene.rs"]
pub mod object_scene;
#[path = "frontend/people_scene.rs"]
pub mod people_scene;
#[path = "frontend/transport_audio.rs"]
pub mod transport_audio;
#[path = "frontend/transport_scene.rs"]
pub mod transport_scene;
#[path = "frontend/tree_scene.rs"]
pub mod tree_scene;
#[path = "frontend/tutorial_visual.rs"]
mod tutorial_visual;
#[path = "frontend/vehicle_scene.rs"]
pub mod vehicle_scene;

#[path = "frontend/city_ownership.rs"]
pub mod city_ownership;
#[path = "frontend/city_people.rs"]
pub mod city_people;
#[path = "frontend/city_prestige.rs"]
pub mod city_prestige;
#[path = "frontend/comparison.rs"]
pub mod comparison;
#[path = "frontend/consumer_polls.rs"]
pub mod consumer_polls;
#[path = "frontend/prestige_stars.rs"]
pub mod prestige_stars;

#[path = "frontend/ownership_badges.rs"]
pub mod ownership_badges;

#[path = "frontend/block_information.rs"]
pub mod block_information;
#[path = "frontend/city_rights.rs"]
pub mod city_rights;
#[path = "frontend/saved_current_player.rs"]
pub mod saved_current_player;
#[path = "frontend/strategic_labels.rs"]
pub mod strategic_labels;
#[path = "frontend/strategic_selection.rs"]
pub mod strategic_selection;
#[path = "frontend/utility_report.rs"]
pub mod utility_report;

#[path = "frontend/people_markers.rs"]
pub mod people_markers;

#[path = "frontend/build_panel.rs"]
pub mod build_panel;
#[path = "frontend/build_view.rs"]
mod build_view;
#[path = "frontend/cash_arrow.rs"]
mod cash_arrow;

#[cfg(test)]
#[path = "frontend/rental_turnover.rs"]
mod rental_turnover;

#[cfg(test)]
#[path = "frontend/scenario_three_play.rs"]
mod scenario_three_play;
