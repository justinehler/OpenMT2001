//! Original420500 rival notices and4207d0 expiry. Their text panel is separate
//! from the modal prompt queue and never pauses simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub packet: [u32; 10],
    pub remaining: i32,
}
impl Default for Entry {
    fn default() -> Self {
        Self {
            packet: [u32::MAX; 10],
            remaining: -1,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Queue {
    entries: [Entry; 20],
    count: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub category: u32,
    pub category_kind: u32,
    pub character: u32,
    pub actor: Vec<u8>,
    pub business_kind: Option<u32>,
}
impl Queue {
    pub fn entries(&self) -> &[Entry] {
        &self.entries[..self.count]
    }
    pub fn view(&self, session: &crate::fresh_city::Session<'_>) -> Result<Vec<Notice>, String> {
        self.entries()
            .iter()
            .map(|entry| {
                let owner = entry.packet[1]
                    .checked_sub(1)
                    .ok_or("Notification city player palette is unresolved")?;
                let player = session
                    .roster
                    .manager
                    .players
                    .get(owner as usize)
                    .ok_or("Notification owner outside roster")?;
                let category = session
                    .world
                    .city
                    .categories
                    .get(entry.packet[7] as usize)
                    .ok_or("Notification category outside pool")?;
                Ok(Notice {
                    category: entry.packet[7],
                    category_kind: u32::from_le_bytes(category[0x8c..0x90].try_into().unwrap()),
                    character: u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()),
                    actor: player.bytes[0x158..0x1b8].to_vec(),
                    business_kind: (entry.packet[2] == 1).then_some(entry.packet[6]),
                })
            })
            .collect()
    }
    pub fn push(&mut self, packet: [u32; 10], local: u8) {
        if packet[1].wrapping_sub(1) == u32::from(local) || self.count >= 19 || packet[2] > 1 {
            return;
        }
        self.entries[self.count] = Entry {
            packet,
            remaining: 700,
        };
        self.count += 1;
    }
    /// Called by4ba580 only with its suppression/reason3 gates open and
    ///8b22dc nonzero, once per original interface speed iteration.
    pub fn tick(&mut self) {
        if self.count == 0 {
            return;
        }
        for index in 0..20 {
            if self.entries[index].remaining > 0 {
                self.entries[index].remaining -= 1;
            }
            if self.entries[index].remaining == 0 {
                // The original loop advances after removing a row. A row shifted
                // into this position therefore retains its timer for this tick.
                for next in index..19 {
                    self.entries[next] = self.entries[next + 1];
                }
                self.count = self.count.saturating_sub(1);
            }
        }
    }
}

#[test]
#[ignore = "requires original420500/4207d0 observations"]
fn original_notification_queue() {
    let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_NOTIFICATIONS").unwrap()).unwrap();
    assert_eq!(&bytes[..8], b"MTNOTI01");
    let mut words = bytes[8..]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let count = words.next().unwrap();
    for case in 0..count {
        let size = words.next().unwrap() as usize;
        let local = words.next().unwrap() as u8;
        let ticks = words.next().unwrap();
        let packet = std::array::from_fn(|_| words.next().unwrap());
        let mut queue = Queue {
            count: size,
            entries: std::array::from_fn(|_| Entry {
                packet: std::array::from_fn(|_| words.next().unwrap()),
                remaining: words.next().unwrap() as i32,
            }),
        };
        queue.push(packet, local);
        for _ in 0..ticks {
            queue.tick();
        }
        assert_eq!(
            queue.count,
            words.next().unwrap() as usize,
            "count case{case}"
        );
        for (slot, entry) in queue.entries.iter().enumerate() {
            let expected = Entry {
                packet: std::array::from_fn(|_| words.next().unwrap()),
                remaining: words.next().unwrap() as i32,
            };
            assert_eq!(*entry, expected, "case{case} slot{slot}");
        }
    }
    assert!(words.next().is_none());
    eprintln!("Matched {count} native notification queue cases");
}
