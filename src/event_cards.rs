//! Original event-card deck (41b910/41ba60/41b980/41ba00).
//! RNG state is transferred from the startup registry, never reseeded here.
use crate::{pe::PeImage, player_manager::word};

pub mod effects;
pub mod ai;
pub mod lifecycle;
pub mod human;
pub mod targeted;
pub mod selection;

pub const COUNT: usize = 83;
pub struct Deck {
    pub rng: [u32; 3],
    pub cards: [u32; COUNT],
    pub used: [u8; COUNT],
    pub remaining: u32,
}
impl Deck {
    /// The card branches of48c1a0/48c1c0. Other broadcasts belong to their
    /// own consumers. Replies stay synchronous with the original deck update.
    pub fn broadcast(
        &mut self,
        packet: &crate::player_messages::Packet,
        day: u32,
        image: &PeImage<'_>,
        reply: impl FnMut(u32, &mut Self) -> Result<(), String>,
    ) -> Result<bool, String> {
        match word(packet, 4) {
            0x31 => self.draw(day, image, reply)?,
            0x32 => self.draw_specific(word(packet, 0x18), reply)?,
            0x34 => self.release(word(packet, 0x18))?,
            _ => return Ok(false),
        }
        Ok(true)
    }
    /// Fresh/reset41b910. Existing allocation identities are host concerns.
    pub fn new(rng: [u32; 3]) -> Self {
        let mut result = Self {
            rng,
            cards: std::array::from_fn(|i| i as u32),
            used: [0; COUNT],
            remaining: 0,
        };
        result.rebuild();
        result
    }
    ///41ba60 retains the unused tail of the old allocation. Every swap chooses
    /// from the whole active prefix; this is not a shrinking-range shuffle.
    pub fn rebuild(&mut self) {
        self.remaining = 0;
        self.cards[0] = u32::MAX;
        for (card, &used) in self.used.iter().enumerate() {
            if used == 0 {
                self.cards[self.remaining as usize] = card as u32;
                self.remaining += 1;
            }
        }
        for index in 0..self.remaining as usize {
            self.rng[0] = self.rng[0]
                .wrapping_mul(self.rng[1])
                .wrapping_add(self.rng[2]);
            let other = ((self.rng[0] >> 16) & 0x7fff) % self.remaining;
            self.cards.swap(index, other as usize);
        }
    }
    ///41b980. A rejected top card rebuilds the entire available deck.
    pub fn draw(
        &mut self,
        day: u32,
        image: &PeImage<'_>,
        mut reply: impl FnMut(u32, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        // The native loop has no termination when no eligible card remains.
        // Bound malformed/exhausted state instead of hanging the host forever.
        for _ in 0..10000 {
            let at = self
                .remaining
                .checked_sub(1)
                .ok_or("event deck has no top card")? as usize;
            let card = *self
                .cards
                .get(at)
                .ok_or("event deck remaining exceeds capacity")?;
            let entry = card
                .checked_mul(20)
                .and_then(|n| n.checked_add(0x534208))
                .ok_or("event card address overflow")?;
            let row = image.at(entry, 20)?;
            if word(row, 0) == 3 || (day == 0 && row[0x11] == 0) {
                self.rebuild();
                continue;
            }
            self.remaining -= 1;
            *self
                .used
                .get_mut(card as usize)
                .ok_or("event card outside flag table")? = 1;
            return reply(card, self);
        }
        Err("event deck did not find an eligible card after repeated native reshuffles".into())
    }
    ///41ba00 rereads remaining after each synchronous reply/rebuild.
    pub fn draw_specific(
        &mut self,
        card: u32,
        mut reply: impl FnMut(u32, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut index = 0;
        while (index as i32) < self.remaining as i32 {
            if *self
                .cards
                .get(index)
                .ok_or("event deck remaining exceeds capacity")?
                == card
            {
                *self
                    .used
                    .get_mut(card as usize)
                    .ok_or("requested event card outside flag table")? = 1;
                self.rebuild();
                reply(card, self)?;
            }
            index += 1;
        }
        Ok(())
    }
    ///48c21c, broadcast34: makes a card eligible for the next rebuild.
    pub fn release(&mut self, card: u32) -> Result<(), String> {
        *self
            .used
            .get_mut(card as usize)
            .ok_or("released event card outside flag table")? = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    fn check(deck: &Deck, r: &mut Reader<'_>, name: &str) {
        let object = r.blob();
        let cards = r.blob();
        let flags = r.blob();
        assert_eq!(
            deck.rng,
            std::array::from_fn(|i| word(object, i * 4)),
            "rng {name}"
        );
        assert_eq!(word(object, 0x14), 0x10011000);
        assert_eq!(word(object, 0x18), 0x10012000);
        assert_eq!(word(object, 0x1c), 83);
        assert_eq!(deck.remaining, word(object, 0x20), "remaining {name}");
        assert_eq!(
            deck.cards,
            std::array::from_fn(|i| word(cards, i * 4)),
            "deck {name}"
        );
        assert_eq!(deck.used, flags, "flags {name}");
    }
    #[test]
    #[ignore = "requires original card broadcast observations"]
    fn matches_native_broadcast() {
        let bytes=std::fs::read(std::env::var("OPENMT2001_NATIVE_EVENT_BROADCAST").unwrap()).unwrap();
        let exe=std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image=PeImage::parse(&exe).unwrap();let mut r=Reader(&bytes);
        assert_eq!(r.take(8),b"MTECBR01");assert_eq!(r.word(),1);assert_eq!(r.word(),0x037f);
        let read=|r:&mut Reader<'_>| {
            let object=r.blob();let cards=r.blob();let flags=r.blob();
            Deck {rng:std::array::from_fn(|i|word(object,i*4)),cards:std::array::from_fn(|i|word(cards,i*4)),
                used:flags.try_into().unwrap(),remaining:word(object,0x20)}
        };
        for case in 0..r.word() {
            let [_seed,kind,card,day,mutation]=std::array::from_fn::<_,5,_>(|_|r.word());
            let mut deck=read(&mut r);let expected=read(&mut r);
            let trace_expected:Vec<[u32;6]>=(0..r.word()).map(|_|std::array::from_fn(|_|r.word())).collect();
            let mut packet=[0;48];crate::player_manager::put(&mut packet,4,kind);crate::player_manager::put(&mut packet,0x18,card);
            let mut trace=vec![];
            assert!(deck.broadcast(&packet,day,&image,|drawn,deck|{
                trace.push([0x10012000,0x33,drawn,0x5f5e0ff,0x5f5e0ff,0x5f5e0ff]);
                if mutation!=0 {deck.release(card)?;}Ok(())
            }).unwrap());
            assert_eq!(deck.rng,expected.rng,"rng {case}");assert_eq!(deck.cards,expected.cards,"cards {case}");
            assert_eq!(deck.used,expected.used,"used {case}");assert_eq!(deck.remaining,expected.remaining,"remaining {case}");
            assert_eq!(trace,trace_expected,"replies {case}");
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires original event-card observations"]
    fn matches_native_deck() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_EVENT_CARDS").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTECARD1");
        assert_eq!(r.word(), 2);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let [op, seed, day, top, specific, exclude, release] =
                std::array::from_fn::<_, 7, _>(|_| r.word());
            let mut deck = Deck::new([seed, 0x343fd, 0x269ec3]);
            check(&deck, &mut r, name);
            if top != u32::MAX {
                deck.remaining = 1;
                deck.cards[0] = top;
            }
            if exclude != u32::MAX {
                deck.used[exclude as usize] = 1;
            }
            let mut trace = vec![];
            let mut reply = |card, deck: &mut Deck| {
                trace.push([
                    0x48cb70, 0x86d1f0, 0x10013000, 0x33, card, 0x5f5e0ff, 0x5f5e0ff, 0x5f5e0ff,
                ]);
                if release != u32::MAX {
                    deck.release(release)?;
                }
                Ok(())
            };
            match op {
                0 => (),
                1 => deck.draw(day, &image, &mut reply).unwrap(),
                2 => deck.draw_specific(specific, &mut reply).unwrap(),
                3 => deck.rebuild(),
                _ => panic!("bad op"),
            }
            check(&deck, &mut r, name);
            assert_eq!(r.word(), 1);
            assert_eq!(r.word(), 0);
            let expected: Vec<[u32; 8]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            assert_eq!(trace, expected, "reply {name}");
        }
        assert!(r.0.is_empty());
    }
}

