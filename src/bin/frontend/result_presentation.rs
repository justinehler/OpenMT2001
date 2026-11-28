//! Result-screen presentation (audio, fireworks, lighting, medallions, text).
//! Mounted outside the Session so the Session can move into the library (#99).
#[path = "result_audio.rs"]
pub mod audio;
#[path = "result_fireworks.rs"]
pub mod fireworks;
#[path = "result_lighting.rs"]
pub mod lighting;
#[path = "result_medallions.rs"]
pub mod medallions;
#[path = "result_particles_gpu.rs"]
pub mod particles_gpu;
#[path = "result_text.rs"]
pub mod text;
