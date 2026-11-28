//! The game's Host and session (#99): `live_host`, `fresh_city` (with
//! `custom_session`, `session_restore` and `session_scenarios`), `session_save`
//! and `process_dispatch`, built against the library alone. The game binary
//! and the `headless` binary both link these modules from here instead of
//! compiling them themselves, so a change to intro or the interface layer
//! doesn't rebuild them, and their tests build without intro.
//!
//! The sources stay in src/bin/frontend. Non-test code here may name only the
//! library and these modules. Tests may also name the interface layer
//! (crates/frontend, a dev-dependency); tests that need intro or the game's
//! menus live in the game crate as `*_host_tests.rs`.
// Library modules the mounted sources name as crate::<module>.
#[cfg(test)]
use openmt2001::{auction_controls, auction_labels};
#[allow(unused_imports)]
use openmt2001::{
    build_panel, campaign_store, city_people, compatibility, human_dialog, player_info,
    result_controller, save_menu, skirmish_maps, tutorial_two_interaction,
};
// Interface-layer modules the mounted tests name as crate::<module>.
#[cfg(all(test, feature = "capture"))]
use openmt_frontend::capture_test;
#[cfg(test)]
#[allow(unused_imports)]
use openmt_frontend::{
    auction_bridge, auction_panel, auction_scene, block_information, block_sidewalk_labels,
    business_adjustment, business_edit, business_view, cash_arrow, city_ownership, city_prestige,
    city_rights, comparison, game_options, gameplay_sound, locator, navigation, people_scene,
    prompt, scenarios, selection, stats_panel, utility_report,
};
#[path = "../../../src/bin/frontend/fresh_city.rs"]
pub mod fresh_city;
#[path = "../../../src/bin/frontend/live_host.rs"]
pub mod live_host;
#[path = "../../../src/bin/frontend/process_dispatch.rs"]
pub mod process_dispatch;
#[path = "../../../src/bin/frontend/session_save.rs"]
pub mod session_save;
// Live-link checks the game binary's tests share.
#[cfg(any(test, feature = "test-support"))]
#[path = "../../../src/bin/frontend/session_validation.rs"]
pub mod session_validation;
