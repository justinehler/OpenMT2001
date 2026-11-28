pub use openmt2001::session::restore::*;

#[cfg(test)]
#[path = "session_restore_tests.rs"]
mod tests;
#[cfg(test)]
mod transfer_stream_test {
    include!("session_restore_transfer_test.rs");
}
#[cfg(test)]
mod resident_reset_test {
    include!("session_restore_resident_reset_test.rs");
}
#[cfg(test)]
mod sale_stream_test {
    include!("session_restore_sale_test.rs");
}

// The moved module's imports, for its tests' `use super::*`.
#[cfg(test)]
#[allow(unused_imports)]
use super::{CommandAssets, Inputs, Policy, Session};
#[cfg(test)]
#[allow(unused_imports)]
use openmt2001::{
    building_geometry::BuildingGeometry,
    building_pool, category_reset, economy, housing, level_startup, player_constructor,
    save::{SaveFile, restore},
    startup_rng,
};
