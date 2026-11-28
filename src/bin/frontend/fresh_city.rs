//! The Session moved into the library (`openmt2001::session`, #99). This shim
//! keeps `crate::fresh_city::…` paths and mounts the Session tests here.
pub use openmt2001::session::*;
#[path = "custom_session.rs"]
pub mod custom;
#[path = "session_restore.rs"]
pub mod restore;
#[path = "fresh_city/result.rs"]
pub mod result;
#[path = "session_scenarios.rs"]
pub mod scenarios;

#[cfg(test)]
#[path = "fresh_city/ai_resume_probe.rs"]
mod ai_resume_probe;
#[cfg(test)]
#[path = "fresh_city/objective_tests.rs"]
mod objective_tests;
#[cfg(all(test, feature = "capture"))]
#[path = "outcome_capture.rs"]
mod outcome_capture;
#[cfg(test)]
#[path = "selected_income_tests.rs"]
mod selected_income_tests;
#[cfg(test)]
#[path = "session_option_word_test.rs"]
mod session_option_word_tests;
#[cfg(test)]
#[path = "fresh_city/skirmish_session_acceptance.rs"]
mod skirmish_session_acceptance;
#[cfg(test)]
#[path = "fresh_city/test_observers.rs"]
mod test_observers;
#[cfg(test)]
#[path = "fresh_city/tests.rs"]
mod tests;

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::{
    business_binding, business_pool, economy, level_script, level_startup, lua4::Value, map_script,
};
#[cfg(test)]
#[allow(unused_imports)]
use std::{fs, path::Path};
