pub use openmt2001::session::custom::*;

#[cfg(test)]
#[path = "custom_session_tests.rs"]
mod tests;

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::{level_catalog::Level, lua4::Runtime};
#[cfg(test)]
#[allow(unused_imports)]
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
