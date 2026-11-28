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
    /// 42c4b0 passes "%f%%" (0x535868) to the game's own text formatter
    /// (0x441770, tokens 0x441480), not to the CRT. Its %f case prints with
    /// its own "%.2f" literal (0x535894), so the label has two decimals.
    pub fn percentage_text(&self) -> Result<String, String> {
        Ok(format!(
            "{}%",
            crate::decimal_text::decimal2(self.percentage)?
        ))
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

#[cfg(test)]
mod tests {
    use super::Row;

    fn label(percentage: f32) -> String {
        Row {
            name: Vec::new(),
            character: 0,
            percentage,
            progress: 0,
        }
        .percentage_text()
        .unwrap()
    }

    #[test]
    fn voting_percentages_have_the_formatters_two_decimals() {
        // #591: the original reads 48.27%, 39.65%, 12.08%, 0.00%.
        assert_eq!(label(48.27), "48.27%");
        assert_eq!(label(26.779999), "26.78%");
        assert_eq!(label(0.), "0.00%");
        assert_eq!(label(100.), "100.00%");
        // An exact halfway value rounds away from zero, as the CRT's %.2f does.
        assert_eq!(label(0.125), "0.13%");
    }
}
