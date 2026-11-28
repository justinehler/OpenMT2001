//! Participant status selection414f90 and projected label bounds414c40.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Text {
    Empty,
    Language(u32),
    Money(i32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    pub status: Text,
    pub status_font: u32,
    pub cash: i32,
    pub cash_color: u32,
}

/// Text inputs retained in the live host snapshot, in auction seat order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
    pub name: Vec<u8>,
    pub character: u32,
    pub label: Label,
    pub high_bidder: bool,
    pub winner: bool,
}

pub fn participants(session: &crate::fresh_city::Session<'_>) -> Result<Vec<Participant>, String> {
    let record = &session.world.city.auction.controller.record;
    let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    session
        .roster
        .manager
        .players
        .iter()
        .enumerate()
        .map(|(slot, player)| {
            let name = &player.bytes[0x158..0x1b8];
            let end = name
                .iter()
                .position(|&b| b == 0)
                .ok_or("Unterminated auction participant name")?;
            let cash = session
                .world
                .ledgers
                .players
                .get(slot)
                .ok_or("Auction participant account missing")?
                .cash;
            Ok(Participant {
                name: name[..end].to_vec(),
                character: u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()),
                label: Label::from_record(record, slot, cash)?,
                high_bidder: word(0x5c) == slot as u32,
                winner: word(0x60) == slot as u32,
            })
        })
        .collect()
}
impl Label {
    pub fn from_record(record: &[u8; 344], slot: usize, cash: f32) -> Result<Self, String> {
        if slot >= 6 {
            return Err("Auction participant outside six slots".into());
        }
        let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let cash = openmt2001::x87::Sum::from_f32(cash)?.ftol_low_word();
        let bid = word(0xc + slot * 12) as i32;
        let status = if word(0x60) == slot as u32 {
            Text::Language(1552)
        } else if record[0x12 + slot * 12] != 0 {
            Text::Language(1555)
        } else if record[0x10 + slot * 12] != 0 {
            Text::Language(1556)
        } else if word(0x58) == slot as u32 {
            Text::Language(1558)
        } else if bid > 0 {
            Text::Money(bid)
        } else if word(0x50) == slot as u32 {
            Text::Language(1557)
        } else {
            Text::Empty
        };
        Ok(Self {
            status,
            status_font: u32::from(matches!(status, Text::Money(_))),
            cash,
            cash_color: if cash < 0 { 6 } else { 0 },
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Original rectangles store x/y/width/height, not right/bottom.
    pub background: [f32; 4],
    pub name_cash: [f32; 4],
    pub status: [f32; 4],
}
impl Bounds {
    pub fn from_projected(point: [f32; 2], viewport: [u32; 2], high: bool, winner: bool) -> Self {
        let [x, y] = scaled(point, viewport);
        let left = (f64::from(x) - 40.) as f32;
        let top = (f64::from(y) - 20.) as f32;
        Self {
            background: [left, top, 80., 40.],
            name_cash: [left, top, 80., 23.],
            status: [
                left,
                (f64::from(top) + 25.) as f32,
                80.,
                if high && !winner { 45. } else { 15. },
            ],
        }
    }
}
pub fn high_bidder_marker(point: [f32; 2], viewport: [u32; 2]) -> [f32; 4] {
    let [x, y] = scaled(point, viewport);
    //414c40 stores the marker origin and its32-pixel dimensions.
    [
        (f64::from(x) - 16.) as f32,
        (f64::from(y) - 16.) as f32,
        32.,
        32.,
    ]
}
fn scaled(point: [f32; 2], viewport: [u32; 2]) -> [f32; 2] {
    std::array::from_fn(|i| (f64::from(point[i]) * [640., 480.][i] / f64::from(viewport[i])) as f32)
}

#[test]
fn participant_status_priority_and_whole_cash() {
    let mut record = [0; 344];
    let slot = 2;
    record[0x50..0x54].copy_from_slice(&2u32.to_le_bytes());
    record[0x58..0x5c].copy_from_slice(&u32::MAX.to_le_bytes());
    record[0x60..0x64].copy_from_slice(&u32::MAX.to_le_bytes());
    let status = |record: &[u8; 344]| Label::from_record(record, slot, -1234.75).unwrap().status;
    assert_eq!(status(&record), Text::Language(1557));
    record[0xc + slot * 12..0x10 + slot * 12].copy_from_slice(&250u32.to_le_bytes());
    assert_eq!(status(&record), Text::Money(250));
    record[0x58..0x5c].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(status(&record), Text::Language(1558));
    record[0x10 + slot * 12] = 1;
    assert_eq!(status(&record), Text::Language(1556));
    record[0x12 + slot * 12] = 1;
    assert_eq!(status(&record), Text::Language(1555));
    record[0x60..0x64].copy_from_slice(&2u32.to_le_bytes());
    let label = Label::from_record(&record, slot, -1234.75).unwrap();
    assert_eq!(label.status, Text::Language(1552));
    assert_eq!((label.cash, label.cash_color), (-1234, 6));
    let label = Label::from_record(&record, slot, 1234.75).unwrap();
    assert_eq!((label.cash, label.cash_color), (1234, 0));
}

#[test]
fn projected_bounds_keep_original_coordinate_space() {
    let high = Bounds::from_projected([640., 480.], [1280, 960], true, false);
    assert_eq!(high.background, [280., 220., 80., 40.]);
    assert_eq!(high.name_cash, [280., 220., 80., 23.]);
    assert_eq!(high.status, [280., 245., 80., 45.]);
    let won = Bounds::from_projected([320., 240.], [640, 480], true, true);
    assert_eq!(won.status, [280., 245., 80., 15.]);
}

#[test]
#[ignore = "requires original414f90 label observations"]
fn original_auction_labels() {
    let data = std::fs::read(openmt2001::fixtures::path("NATIVE_AUCTION_LABELS")).unwrap();
    assert_eq!(&data[..8], b"MTAULB01");
    let mut words = data[8..]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    assert_eq!(words.next(), Some(0x37f));
    for case in 0..words.next().unwrap() {
        let count = words.next().unwrap() as usize;
        let mut record = [0; 344];
        for offset in [0x58, 0x60, 0x50] {
            record[offset..offset + 4].copy_from_slice(&words.next().unwrap().to_le_bytes());
        }
        let mut cash = Vec::new();
        for slot in 0..count {
            cash.push(f32::from_bits(words.next().unwrap()));
            record[0x10 + slot * 12] = words.next().unwrap() as u8;
            record[0x12 + slot * 12] = words.next().unwrap() as u8;
            record[0xc + slot * 12..0x10 + slot * 12]
                .copy_from_slice(&words.next().unwrap().to_le_bytes());
        }
        for (slot, cash) in cash.into_iter().enumerate() {
            let expected: [u32; 5] = std::array::from_fn(|_| words.next().unwrap());
            let label = Label::from_record(&record, slot, cash).unwrap();
            let (kind, value) = match label.status {
                Text::Empty => (0, 0),
                Text::Language(v) => (1, v),
                Text::Money(v) => (2, v as u32),
            };
            assert_eq!(
                [
                    label.status_font,
                    kind,
                    value,
                    label.cash as u32,
                    label.cash_color
                ],
                expected,
                "case{case} slot{slot}"
            );
        }
    }
    assert!(words.next().is_none());
}

#[test]
#[ignore = "requires original414c40 layout observations"]
fn original_auction_label_layout() {
    let data = std::fs::read(openmt2001::fixtures::path("NATIVE_AUCTION_LABEL_LAYOUT")).unwrap();
    assert_eq!(&data[..8], b"MTAULY01");
    let mut words = data[8..]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    assert_eq!(words.next(), Some(0x37f));
    for case in 0..words.next().unwrap() {
        let count = words.next().unwrap() as usize;
        let viewport = [words.next().unwrap(), words.next().unwrap()];
        let high = words.next().unwrap();
        let winner = words.next().unwrap();
        let points = (0..=count)
            .map(|_| std::array::from_fn(|_| f32::from_bits(words.next().unwrap())))
            .collect::<Vec<[f32; 2]>>();
        let expected: [u32; 4] = std::array::from_fn(|_| words.next().unwrap());
        let marker = high_bidder_marker(points[0], viewport);
        assert_eq!(marker.map(f32::to_bits), expected, "marker{case}");
        for slot in 0..count {
            let bounds = Bounds::from_projected(
                points[slot + 1],
                viewport,
                high == slot as u32,
                winner == slot as u32,
            );
            for actual in [bounds.background, bounds.name_cash, bounds.status] {
                let expected: [u32; 4] = std::array::from_fn(|_| words.next().unwrap());
                assert_eq!(actual.map(f32::to_bits), expected, "case{case} slot{slot}");
            }
            for [x, y, w, h] in [bounds.status, bounds.name_cash, bounds.name_cash] {
                let actual = [
                    x,
                    y,
                    (f64::from(x) + f64::from(w)) as f32,
                    (f64::from(y) + f64::from(h)) as f32,
                    0.5,
                ];
                let expected: [u32; 5] = std::array::from_fn(|_| words.next().unwrap());
                assert_eq!(
                    actual.map(f32::to_bits),
                    expected,
                    "text case{case} slot{slot}"
                );
            }
        }
    }
    assert!(words.next().is_none());
}
