//! Request protocols between the live host's simulation tick and the binary's
//! audio players. The host sends what to play; the binary owns the rodio
//! sinks and answers. Keeping the protocol here lets the host build without
//! the audio stack (#99 step 3).
pub mod era;
