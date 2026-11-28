//! Request protocols between the live host's simulation tick and the binary's
//! renderer. The host sends what to present; the binary owns the GPU state and
//! answers. Keeping the protocol here lets the host build without the render
//! stack (#99 step 3).
pub mod auction;
