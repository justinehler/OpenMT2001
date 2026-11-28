//! Compatibility entry point for the saved-preview test hook in intro.rs.
//! The natural-host observers were removed with their unavailable-save tests.

// Retain this test-only entry point while intro.rs is being split separately.
pub(crate) fn after_restore(_: &crate::fresh_city::Session<'_>) {}
