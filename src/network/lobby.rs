//! Six distinct rows and authority checks for the original lobby (0x4a10b0,
//! 0x4a1b40, 0x4a215a). Peer slots are transport identities, not row indices.
use super::{Error, Result, Row, RowKind, SLOTS};
pub mod worker;

pub const START_BOUNDARY: &str = "Connected game startup is not available yet (issue #34).";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub name: Vec<u8>,
    pub level: u32,
    pub parameter_bits: u32,
    pub difficulty: u8,
    pub map: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub player: Row,
    pub peer: Option<u8>,
    pub difficulty: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub settings: Settings,
    pub revision: u64,
    pub rows: [Slot; SLOTS],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Join {
        name: Vec<u8>,
        token: i8,
    },
    /// No target row or peer can be supplied by a client.
    Edit {
        token: i8,
        ready: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Character { row: usize, direction: i8 },
    Ready,
    Kind(usize),
    Difficulty(usize),
    Start,
}
fn text(bytes: &[u8], max: usize) -> bool {
    !bytes.is_empty() && bytes.len() <= max && bytes.iter().all(|&b| b >= 32 && b != 127)
}
impl Settings {
    pub(super) fn validate(&self) -> Result<()> {
        if !text(&self.name, 16)
            || !text(&self.map, 31)
            || self.level == 0
            || self.difficulty > 2
            || !f32::from_bits(self.parameter_bits).is_finite()
        {
            return Err(Error::Content);
        }
        Ok(())
    }
}
impl Snapshot {
    pub fn host(settings: Settings, name: Vec<u8>, token: i8) -> Result<Self> {
        // Per-row difficulty is independent of the scenario difficulty (+44).
        let difficulty = 0;
        let mut snapshot = Self {
            settings,
            revision: 1,
            rows: std::array::from_fn(|_| Slot {
                player: Row {
                    kind: RowKind::Open,
                    token: -1,
                    ready: false,
                    name: vec![],
                },
                peer: None,
                difficulty,
            }),
        };
        snapshot.rows[0].player = Row {
            kind: RowKind::Human,
            token,
            ready: true,
            name,
        };
        snapshot.rows[0].peer = Some(0);
        snapshot.validate()?;
        Ok(snapshot)
    }
    pub fn validate(&self) -> Result<()> {
        self.settings.validate()?;
        if self.revision == 0 || self.rows[0].peer != Some(0) {
            return Err(Error::Row);
        }
        let mut tokens = [false; 10];
        let mut peers = [false; SLOTS];
        for slot in &self.rows {
            slot.player.validate()?;
            if slot.difficulty > 2 {
                return Err(Error::Row);
            }
            if matches!(slot.player.kind, RowKind::Human | RowKind::Ai) {
                if !text(&slot.player.name, 31) {
                    return Err(Error::Row);
                }
                let token = slot.player.token as usize;
                if tokens[token] {
                    return Err(Error::Row);
                }
                tokens[token] = true;
            }
            if (slot.player.kind == RowKind::Human) != slot.peer.is_some() {
                return Err(Error::Peer);
            }
            if let Some(peer) = slot.peer {
                let used = peers.get_mut(peer as usize).ok_or(Error::Peer)?;
                if *used {
                    return Err(Error::Peer);
                }
                *used = true;
            }
        }
        Ok(())
    }
    pub fn local_row(&self, peer: u8) -> Option<usize> {
        self.rows.iter().position(|r| r.peer == Some(peer))
    }
    pub fn capacity(&self) -> (usize, usize) {
        (
            self.rows
                .iter()
                .filter(|r| r.player.kind == RowKind::Human)
                .count(),
            self.rows
                .iter()
                .filter(|r| matches!(r.player.kind, RowKind::Human | RowKind::Open))
                .count(),
        )
    }
    pub fn start_enabled(&self) -> bool {
        self.validate().is_ok()
            && self
                .rows
                .iter()
                .filter(|r| matches!(r.player.kind, RowKind::Human | RowKind::Ai))
                .count()
                > 1
            && self.rows.iter().all(|r| {
                !matches!(r.player.kind, RowKind::Human | RowKind::Ai)
                    || (r.player.ready && r.player.token >= 0 && !r.player.name.is_empty())
            })
    }
    fn change(&mut self, edit: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        let mut next = self.clone();
        edit(&mut next)?;
        next.revision = self.revision.checked_add(1).ok_or(Error::Exhausted)?;
        next.validate()?;
        *self = next;
        Ok(())
    }
    fn unused(&self, current: i8, direction: i8) -> Result<i8> {
        if !matches!(direction, -1 | 1) {
            return Err(Error::Row);
        }
        (1..=10)
            .map(|n| (current + n * direction).rem_euclid(10))
            .find(|t| self.rows.iter().all(|r| r.player.token != *t))
            .ok_or(Error::Row)
    }
    pub fn request(&mut self, peer: u8, request: Request) -> Result<()> {
        self.change(|next| match request {
            Request::Join { name, token } => {
                if peer == 0 || peer as usize >= SLOTS || next.local_row(peer).is_some() {
                    return Err(Error::Peer);
                }
                let token = if (0..10).contains(&token)
                    && next.rows.iter().all(|r| r.player.token != token)
                {
                    token
                } else {
                    next.unused(-1, 1)?
                };
                let row = next
                    .rows
                    .iter_mut()
                    .find(|r| r.player.kind == RowKind::Open)
                    .ok_or(Error::Row)?;
                row.player = Row {
                    kind: RowKind::Human,
                    token,
                    ready: false,
                    name,
                };
                row.peer = Some(peer);
                Ok(())
            }
            Request::Edit { token, ready } => {
                let row = next.local_row(peer).ok_or(Error::Peer)?;
                next.rows[row].player.token = token;
                next.rows[row].player.ready = ready;
                Ok(())
            }
        })
    }
    pub fn remove(&mut self, peer: u8) -> Result<()> {
        if peer == 0 {
            return Err(Error::Peer);
        }
        let Some(row) = self.local_row(peer) else {
            return Ok(());
        };
        self.change(|next| {
            next.rows[row].peer = None;
            next.rows[row].player = Row {
                kind: RowKind::Open,
                token: -1,
                ready: false,
                name: vec![],
            };
            Ok(())
        })
    }
    /// The host can change its own human and AI/open/closed rows. Remote human
    /// controls never become host controls, even if transport and row differ.
    pub fn action(&mut self, action: Action, token_names: &[Vec<u8>; 10]) -> Result<()> {
        if action == Action::Start {
            return Err(Error::Phase);
        }
        self.change(|next| {
            match action {
                Action::Character { row, direction } => {
                    let slot = next.rows.get(row).ok_or(Error::Row)?;
                    if slot.peer != Some(0) && slot.player.kind != RowKind::Ai {
                        return Err(Error::Peer);
                    }
                    let token = next.unused(slot.player.token, direction)?;
                    next.rows[row].player.token = token;
                    if next.rows[row].player.kind == RowKind::Ai {
                        next.rows[row].player.name = token_names[token as usize].clone();
                    }
                }
                Action::Ready => next.rows[0].player.ready = !next.rows[0].player.ready,
                Action::Kind(row) => {
                    let slot = next.rows.get(row).ok_or(Error::Row)?;
                    if slot.peer.is_some() {
                        return Err(Error::Peer);
                    }
                    let kind = match slot.player.kind {
                        RowKind::Open => RowKind::Ai,
                        RowKind::Ai => RowKind::Closed,
                        RowKind::Closed => RowKind::Open,
                        RowKind::Human => return Err(Error::Peer),
                    };
                    let token = if kind == RowKind::Ai {
                        next.unused(-1, 1)?
                    } else {
                        -1
                    };
                    next.rows[row].player = Row {
                        kind,
                        token,
                        ready: kind == RowKind::Ai,
                        name: if token >= 0 {
                            token_names[token as usize].clone()
                        } else {
                            vec![]
                        },
                    };
                }
                Action::Difficulty(row) => {
                    let slot = next.rows.get_mut(row).ok_or(Error::Row)?;
                    if slot.player.kind != RowKind::Ai {
                        return Err(Error::Peer);
                    }
                    slot.difficulty = (slot.difficulty + 1) % 3;
                }
                Action::Start => unreachable!(),
            }
            Ok(())
        })
    }
    pub fn client_edit(&self, peer: u8, action: Action) -> Result<Request> {
        let row = self.local_row(peer).ok_or(Error::Peer)?;
        let player = &self.rows[row].player;
        Ok(match action {
            Action::Character {
                row: target,
                direction,
            } if row == target => Request::Edit {
                token: self.unused(player.token, direction)?,
                ready: player.ready,
            },
            Action::Ready => Request::Edit {
                token: player.token,
                ready: !player.ready,
            },
            _ => return Err(Error::Peer),
        })
    }
}
impl Request {
    pub(super) fn validate(&self) -> Result<()> {
        match self {
            Self::Join { name, token } if text(name, 31) && (0..10).contains(token) => Ok(()),
            Self::Edit { token, .. } if (0..10).contains(token) => Ok(()),
            _ => Err(Error::Row),
        }
    }
}
#[cfg(test)]
mod tests;
