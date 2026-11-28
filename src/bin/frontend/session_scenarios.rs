pub use openmt2001::session::scenarios::*;

#[cfg(test)]
#[path = "session_scenarios_tests.rs"]
mod tests;

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::lua4::Runtime;
#[cfg(test)]
#[allow(unused_imports)]
use std::path::Path;
