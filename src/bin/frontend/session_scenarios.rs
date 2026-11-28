pub use openmt2001::session::scenarios::*;

// session_scenarios_tests.rs needs the single-player menu, the build view and
// the consumer polls, so it builds with the game binary (game.rs
// `fresh_city_scenarios_host_tests`, #99).

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::lua4::Runtime;
#[cfg(test)]
#[allow(unused_imports)]
use std::path::Path;
