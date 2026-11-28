pub use openmt2001::session_save::auction::*;

#[cfg(test)]
#[path = "session_save_auction_tests.rs"]
mod tests;

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use super::{Input, Output, Session, put, word};
#[cfg(test)]
#[allow(unused_imports)]
use crate::auction_controls::{State, TimerText, TurnTimer};
