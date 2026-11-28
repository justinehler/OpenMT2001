//! Session save/restore moved into the library (`openmt2001::session_save`, #99).
pub use openmt2001::session_save::*;
#[path = "session_save_auction.rs"]
pub mod auction;

#[cfg(test)]
#[path = "session_save_ai_test.rs"]
mod ai_test;
#[cfg(test)]
#[path = "session_autosave_live_test.rs"]
mod autosave_live_tests;
// The game binary's session-save tests use its load boundary.
#[cfg(any(test, feature = "test-support"))]
#[path = "session_save_continuation_test.rs"]
pub mod continuation_test;
#[cfg(all(test, feature = "capture"))]
#[path = "session_save_mixed_boundary_test.rs"]
mod mixed_boundary_test;
#[cfg(test)]
#[path = "session_save_pause_test.rs"]
mod pause_test;
#[cfg(test)]
#[path = "session_save_resident_test.rs"]
mod resident_test;
// The auction, terms and main save tests need intro, the time panel or the
// rental fixtures, so they build with the game binary (game.rs
// `session_save_host_tests`, #99).

// The moved module's imports, for its tests' `use super::*` (the load
// boundary in continuation_test is also built for the game's tests).
#[cfg(any(test, feature = "test-support"))]
#[allow(unused_imports)]
use openmt2001::session::Session;
#[cfg(any(test, feature = "test-support"))]
#[allow(unused_imports)]
use openmt2001::{
    live_commands::Command,
    save::{SaveFile, extension, writer},
};
