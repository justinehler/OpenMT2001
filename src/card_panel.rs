//! Event card button and panel state (original 41b800 presentation). The
//! binary's `card_panel` module draws it and turns clicks into card actions.
use crate::{pe::PeImage, player_manager::Player};

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub enabled: bool,
    pub mode: u32,
    pub target: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryVisual {
    pub textures: [usize; 4],
    pub reset: bool,
    pub blink: u32,
}
impl Default for EntryVisual {
    fn default() -> Self {
        Self {
            textures: [285; 4],
            reset: true,
            blink: 0,
        }
    }
}
impl EntryVisual {
    pub fn tick(&mut self, entry: Entry) {
        if entry.mode == 0 {
            self.reset = true;
            self.textures = [285; 4];
            return;
        }
        let base = if entry.mode == 2 && entry.target {
            281
        } else {
            199
        };
        self.textures = [base, base + 2, base + 3, base + 1];
        if entry.mode == 1 || entry.target {
            if self.reset {
                self.reset = false;
                self.blink = 128;
            }
            self.blink = self.blink.wrapping_sub(1);
            if self.blink & 31 > 16 {
                self.textures = [base + 1, base + 2, base, base + 1];
            }
        }
    }
}

impl Entry {
    pub fn player(player: &Player, auction: bool) -> Result<Self, String> {
        let reference = word(&player.bytes, 0x21c);
        let aux = player
            .auxiliaries
            .iter()
            .find(|a| a.reference == reference)
            .ok_or("Card button auxiliary missing")?;
        Ok(Self {
            enabled: player.bytes[0x10] != 0 && !auction,
            mode: word(&aux.bytes, 0x18),
            target: aux.bytes[8] != 0,
        })
    }
    pub fn can_open(self) -> bool {
        self.enabled && matches!(self.mode, 1 | 2)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    pub card: u32,
    pub body: u32,
    pub amount: Option<u32>,
    pub buttons: [bool; 4],
}
impl Panel {
    pub fn present(card: u32, player: &Player, image: &PeImage<'_>) -> Result<Self, String> {
        let row = crate::event_cards::lifecycle::card_row(image, card)?;
        let targeted = word(row, 0) == 2;
        let entry = Entry::player(player, false)?;
        let amount = if targeted {
            None
        } else {
            let scale = f32::from_bits(word(image.at(0x534200, 4)?, 0));
            let mut value = crate::x87::Sum::from_i32(word(row, 8) as i32);
            value.mul_f32(scale)?;
            Some(value.to_f32().to_bits())
        };
        Ok(Self {
            card,
            body: word(row, 4),
            amount,
            buttons: [
                !targeted,
                targeted && entry.target && entry.mode == 2,
                targeted,
                targeted,
            ],
        })
    }
}
