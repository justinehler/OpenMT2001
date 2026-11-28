//! Market share (voting) panel state: the presented rows and the 750-tick
//! display timeout. The binary's `market_share_panel` module draws it.
use crate::player_share::Call;

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: Vec<u8>,
    pub character: usize,
    pub percentage: f32,
    pub progress: u32,
}
impl Row {
    pub fn percentage_text(&self) -> String {
        // 42c4b0 uses the CRT's default %f precision, followed by a literal %.
        format!("{:.6}%", self.percentage)
    }
    pub fn progress_text(&self) -> String {
        format!("{}", self.progress as i32)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub objective: i32,
    pub rows: [Option<Row>; 6],
    pub candidate: Option<usize>,
    pub notice: u32,
}
#[derive(Default)]
pub struct State {
    pub snapshot: Option<Snapshot>,
    remaining: u32,
}
impl State {
    /// Production callback consumer. Share/count values are the callback's
    /// arguments, including the mayor increment made immediately before it.
    pub fn apply(
        &mut self,
        call: Call,
        objective: i32,
        players: &[impl AsRef<[u8]>],
    ) -> Result<(), String> {
        match call {
            Call::Refresh => {
                self.snapshot = Some(Snapshot {
                    objective,
                    rows: std::array::from_fn(|_| None),
                    candidate: None,
                    notice: if objective == 2 { 0x607 } else { 0x606 },
                });
                // 42c850 starts a 750-message lifetime; 42c810 decrements it.
                self.remaining = 750;
            }
            Call::ProfileSelect { index } | Call::Kind1ProfileSelect { index } => {
                let slot = index as usize;
                if slot >= 6 || players.get(slot).is_none() {
                    return Err("market-share profile player leaves roster".into());
                }
                let snapshot = self
                    .snapshot
                    .as_mut()
                    .ok_or("market-share selection before refresh")?;
                snapshot.candidate = Some(slot);
                snapshot.notice = if matches!(call, Call::ProfileSelect { .. }) {
                    0x605
                } else {
                    0x604
                };
            }
            Call::PlayerShare {
                slot,
                normalized,
                auxiliary_reference,
            } => {
                if slot >= 6 || !normalized.is_finite() {
                    return Err("invalid market-share row".into());
                }
                let player = players
                    .get(slot)
                    .ok_or("market-share row leaves roster")?
                    .as_ref();
                if player.len() < 0x1b8 {
                    return Err("market-share player record is truncated".into());
                }
                let name = &player[0x158..0x178];
                let end = name
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or("market-share name is unterminated")?;
                let row = Row {
                    name: name[..end].to_vec(),
                    character: u32::from_le_bytes(player[0x150..0x154].try_into().unwrap())
                        as usize,
                    percentage: normalized,
                    progress: auxiliary_reference,
                };
                self.snapshot
                    .as_mut()
                    .ok_or("market-share row before refresh")?
                    .rows[slot] = Some(row);
            }
            _ => return Err("nonpresentation market-share callback".into()),
        }
        Ok(())
    }
    pub fn dismiss(&mut self) {
        self.snapshot = None;
        self.remaining = 0;
    }
    pub fn interface_tick(&mut self) {
        if self.snapshot.is_some() {
            self.remaining = self.remaining.saturating_sub(1);
            if self.remaining == 0 {
                self.dismiss();
            }
        }
    }
}
