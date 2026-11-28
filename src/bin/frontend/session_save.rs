//! Session save/restore moved into the library (`openmt2001::session_save`, #99).
pub use openmt2001::session_save::*;
#[path = "session_save_auction.rs"]
pub mod auction;

#[cfg(test)]
#[path = "session_save_ai_test.rs"]
mod ai_test;
#[cfg(all(test, feature = "capture"))]
#[path = "session_save_auction_test.rs"]
pub(crate) mod auction_test;
#[cfg(test)]
#[path = "session_autosave_live_test.rs"]
mod autosave_live_tests;
#[cfg(test)]
#[path = "session_save_continuation_test.rs"]
pub(crate) mod continuation_test;
#[cfg(all(test, feature = "capture"))]
#[path = "session_save_mixed_boundary_test.rs"]
mod mixed_boundary_test;
#[cfg(test)]
#[path = "session_save_pause_test.rs"]
mod pause_test;
#[cfg(test)]
#[path = "session_save_resident_test.rs"]
mod resident_test;
#[cfg(test)]
#[path = "session_save_terms_test.rs"]
mod terms_test;
#[cfg(test)]
#[path = "session_save_tests.rs"]
mod tests;

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::session::Session;
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::{
    live_commands::Command,
    save::{SaveFile, extension, writer},
};
