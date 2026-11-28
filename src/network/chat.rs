//! Lobby group chat and in-game selected-recipient chat (0x452160, 0x4523b0, 0x4e2620).
//!
//! Both routes append to one retained display ring per connected session. A line stores
//! only its roster row and text; the sender's name and colour are read from the current
//! admitted roster when the line is drawn, never from the message.
use super::{Error, Result, SLOTS, chat_ring::Ring, lobby::Snapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    /// Originating peer ID + 1. Clients send zero; the host replaces it with the
    /// admitted connection's peer before relaying, so a client cannot choose it.
    pub sender: u32,
    /// Six lobby-row bits. Zero is group delivery.
    pub recipients: u8,
    pub text: Vec<u8>,
}
impl Packet {
    /// Longest text a line can hold: the ring's 256-byte text region minus its NUL.
    pub const MAX_TEXT: usize = 255;
    pub(super) fn validate(&self) -> Result<()> {
        if self.recipients & !63 != 0 || self.text.len() > Self::MAX_TEXT || self.text.contains(&0)
        {
            return Err(Error::Content);
        }
        Ok(())
    }
    /// The originating peer the host stamped into this packet.
    pub fn sender_peer(&self) -> Option<u8> {
        u8::try_from(self.sender.checked_sub(1)?)
            .ok()
            .filter(|&p| usize::from(p) < SLOTS)
    }
    /// Group packets reach every row; selected packets only the masked rows.
    pub fn reaches(&self, row: usize) -> bool {
        self.recipients == 0 || self.recipients & (1 << row) != 0
    }
}
/// The six recipient toggles of the in-game Chat panel (0x41a923/0x41abbe).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recipients(pub [bool; SLOTS]);
impl Recipients {
    pub fn clear(&mut self) {
        self.0.fill(false);
    }
    /// All sets all six slots, including the local one; [`Self::mask`] drops it.
    pub fn all(&mut self) {
        self.0.fill(true);
    }
    /// Delivery mask for a selected send: the chosen rows other than `local`.
    pub fn mask(self, local: usize) -> u8 {
        self.0.iter().enumerate().fold(0, |mask, (i, &selected)| {
            mask | if selected && i != local { 1 << i } else { 0 }
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub row: usize,
    pub token: i8,
    /// `<name>: <text>`, as drawn.
    pub text: Vec<u8>,
}
impl Line {
    /// The sender's player colour, opaque.
    pub fn color(&self, character_colors: &[u32; 10]) -> u32 {
        0xff000000
            | usize::try_from(self.token)
                .ok()
                .and_then(|token| character_colors.get(token))
                .copied()
                .unwrap_or(0xffffff)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub lines: Vec<Line>,
    pub snapshot: Snapshot,
    pub local_peer: u8,
}
pub(super) struct Display {
    pub ring: Ring,
    pub snapshot: Option<Snapshot>,
    pub local_peer: u8,
}
impl Display {
    pub fn new() -> Self {
        Self {
            ring: Ring::construct452040([[0; 264]; 11]),
            snapshot: None,
            local_peer: 0,
        }
    }
    /// Appends `text` from an admitted roster row.
    pub fn append(&mut self, row: usize, text: &[u8]) -> Result<()> {
        let row = u32::try_from(row)
            .ok()
            .filter(|&r| (r as usize) < SLOTS)
            .ok_or(Error::Row)?;
        let mut line = text.to_vec();
        line.push(0);
        self.ring
            .append452070(row, &line)
            .map_err(|_| Error::Content)
    }
    pub fn view(&self) -> Option<View> {
        let snapshot = self.snapshot.clone()?;
        let mut lines = Vec::new();
        let mut index = self.ring.head();
        while index != self.ring.tail() {
            let bytes = &self.ring.slots()[index as usize];
            index = (index + 1) % 11;
            let row = u32::from_le_bytes([bytes[260], bytes[261], bytes[262], bytes[263]]) as usize;
            let (Some(slot), Some(end)) = (
                snapshot.rows.get(row),
                bytes[..256].iter().position(|&b| b == 0),
            ) else {
                continue;
            };
            let mut text = slot.player.name.clone();
            text.extend_from_slice(b": ");
            text.extend_from_slice(&bytes[..end]);
            lines.push(Line {
                row,
                token: slot.player.token,
                text,
            });
        }
        Some(View {
            lines,
            snapshot,
            local_peer: self.local_peer,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::lobby::{Request, Settings};

    #[test]
    fn lines_read_name_and_colour_from_the_current_roster_and_keep_ten() {
        let settings = Settings {
            name: b"Game".to_vec(),
            level: 300001,
            parameter_bits: 10000f32.to_bits(),
            difficulty: 0,
            map: b"Map".to_vec(),
        };
        let mut snapshot = Snapshot::host(settings, b"Host".to_vec(), 0).unwrap();
        snapshot
            .request(
                1,
                Request::Join {
                    name: b"Blue".to_vec(),
                    token: 3,
                },
            )
            .unwrap();
        let mut display = Display::new();
        assert!(display.view().is_none(), "no lines before admission");
        display.snapshot = Some(snapshot.clone());
        display.append(1, b"hi").unwrap();
        assert!(display.append(SLOTS, b"x").is_err());
        let view = display.view().unwrap();
        assert_eq!(view.lines.len(), 1);
        assert_eq!(
            (view.lines[0].text.as_slice(), view.lines[0].token),
            (&b"Blue: hi"[..], 3)
        );
        snapshot.rows[1].player.token = 5;
        display.snapshot = Some(snapshot);
        assert_eq!(display.view().unwrap().lines[0].token, 5);
        for i in 0..12u8 {
            display.append(0, &[b'a' + i]).unwrap();
        }
        let lines = display.view().unwrap().lines;
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0].text, b"Host: c");
        assert_eq!(lines[9].text, b"Host: l");
    }

    #[test]
    fn sender_peer_and_reach() {
        let packet = |sender, recipients| Packet {
            sender,
            recipients,
            text: vec![],
        };
        assert_eq!(packet(0, 0).sender_peer(), None);
        assert_eq!(packet(1, 0).sender_peer(), Some(0));
        assert_eq!(packet(6, 0).sender_peer(), Some(5));
        assert_eq!(packet(7, 0).sender_peer(), None);
        assert_eq!(packet(0xdeadbeef, 0).sender_peer(), None);
        assert!((0..SLOTS).all(|row| packet(1, 0).reaches(row)));
        assert!(packet(1, 0b10).reaches(1) && !packet(1, 0b10).reaches(0));
    }
}
