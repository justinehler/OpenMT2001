//! Offline and connected Skirmish launch data handed to the Session (#99).
use crate::network::Content;

/// Confirmed setup and full compact participants, separate from ordinary Inputs.
/// Offline local peer0 is an adapter identity, not a recovered native peer ID.
#[derive(Clone)]
pub struct SkirmishLaunch {
    pub level_id: u32,
    pub directory: String,
    pub map: String,
    pub objective_kind: i32,
    pub target: f32,
    pub difficulty: i32,
    pub participants: Vec<super::roster::Participant>,
}
impl SkirmishLaunch {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_roster(false)
    }
    pub fn validate_roster(&self, connected: bool) -> Result<(), String> {
        use super::roster::Kind;
        // Bounded original supplied map support; never join a caller path.
        if !(0..=2).contains(&self.difficulty)
            || !self.target.is_finite()
            || !(2..=6).contains(&self.participants.len())
            || self.participants[0].kind != Kind::Human
        {
            return Err("invalid offline Skirmish launch".into());
        }
        if self.directory.is_empty() || !self.directory.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("invalid selected level directory".into());
        }
        let mut used = [false; 10];
        for (slot, p) in self.participants.iter().enumerate() {
            let token = p
                .token
                .filter(|&t| t < 10)
                .ok_or("invalid participant token")?;
            if used[token]
                || !p.ready
                || p.name.is_empty()
                || p.name.len() >= 32
                || p.name.contains(&0)
                || (slot > 0 && p.kind != Kind::Ai && !(connected && p.kind == Kind::Human))
                || !(0..=2).contains(&p.difficulty)
            {
                return Err("invalid compact offline participant".into());
            }
            used[token] = true;
        }
        Ok(())
    }
}

/// Connected startup content staged from the admitted roster and its hashed scripts.
pub struct Staged {
    pub catalog: crate::level_catalog::Catalog,
    pub launch: SkirmishLaunch,
    pub scripts: std::collections::BTreeMap<String, Vec<u8>>,
    pub map: crate::skirmish_maps::ResolvedMap,
    pub peers: Vec<Option<u8>>,
    pub local_slot: usize,
    pub parameters: String,
    pub timetable: String,
    pub era_styles: [crate::building_styles::Styles; 8],
    pub road_routes: Vec<u8>,
    pub content: Content,
}
impl Staged {
    pub fn read(&self, relative: &str) -> Result<Vec<u8>, String> {
        self.scripts
            .get(&relative.to_ascii_lowercase())
            .cloned()
            .ok_or_else(|| format!("Missing staged connected content: {relative}"))
    }
}
