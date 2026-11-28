//! A single-player launch chosen in the frontend: level, difficulty, character
//! and the selected profile's progression. The Session reads it at startup.
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub enum Preview {
    Scenario(u32),
    Sandbox,
}
#[derive(Clone, Copy, Debug)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}
#[derive(Clone, Debug)]
pub struct Setup {
    pub preview: Preview,
    pub difficulty: Difficulty,
    pub character_index: usize,
    pub profile_name: Vec<u8>,
    /// The selected profile's authoritative progression at launch time.
    pub catalog: crate::level_catalog::Catalog,
    pub profile_path: PathBuf,
}
