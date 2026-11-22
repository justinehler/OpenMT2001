//! Private original report fixture, excluding font/draw host seams.
use super::*;
#[test]
#[ignore = "private original property report fixture"]
fn matches_native_property_reports() {
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_PROPERTY_REPORTS").unwrap()).unwrap();
    let mut at = 8;
    assert_eq!(&bytes[..8], b"MTPRPT01");
    fn word(bytes: &[u8], at: &mut usize) -> u32 {
        let v = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
        *at += 4;
        v
    }
    fn records<const N: usize>(bytes: &[u8], at: &mut usize, count: usize) -> Vec<[u8; N]> {
        (0..count)
            .map(|_| {
                let v = bytes[*at..*at + N].try_into().unwrap();
                *at += N;
                v
            })
            .collect()
    }
    assert_eq!(word(&bytes, &mut at), 0x37f);
    let players = word(&bytes, &mut at) as usize;
    let hour = word(&bytes, &mut at);
    let tick = word(&bytes, &mut at);
    let categories: [[u8; 380]; 52] = records(&bytes, &mut at, 52).try_into().unwrap();
    let buildings = records(&bytes, &mut at, 600);
    let businesses = records(&bytes, &mut at, 600);
    let info = records(&bytes, &mut at, 45);
    let products = std::array::from_fn(|_| std::array::from_fn(|i| [i as u32, 0]));
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let report = Report::from_saved_indices(
        &SavedInputs {
            categories: &categories,
            buildings: &buildings,
            businesses: &businesses,
            info: &info,
            products: &products,
            hour,
            tick,
        },
        players,
        &image,
    )
    .unwrap();
    // Native fixture supplies independent original live links as well as
    // original saved indices. Poison the latter: runtime admission must resolve
    // the heads, not accidentally reuse stale save membership bytes.
    let mut live_headers = categories;
    for row in &mut live_headers {
        row[12..16].fill(0);
    }
    for (live, original) in live_headers.iter().zip(&categories) {
        assert_eq!(&live[0x84..0x88], &original[0x84..0x88]);
    }
    let resolved =
        crate::block_information::land_rent::runtime_memberships(&live_headers, &buildings)
            .unwrap();
    let runtime_report = Report::from_memberships(
        &SavedInputs {
            categories: &live_headers,
            buildings: &buildings,
            businesses: &businesses,
            info: &info,
            products: &products,
            hour,
            tick,
        },
        players,
        &image,
        &resolved,
    )
    .unwrap();
    assert_eq!(runtime_report.profit, report.profit);
    assert_eq!(runtime_report.efficiency, report.efficiency);
    assert_eq!(runtime_report.value, report.value);
    let build_refs: Vec<_> = buildings.iter().map(|r| r.as_slice()).collect();
    let business_refs: Vec<_> = businesses.iter().map(|r| r.as_slice()).collect();
    let saved_refs: Vec<_> = categories.iter().map(|r| r.as_slice()).collect();
    let runtime_refs: Vec<_> = live_headers.iter().map(|r| r.as_slice()).collect();
    let saved_rent = crate::block_information::land_rent::from_records(
        &saved_refs,
        &build_refs,
        &business_refs,
        players,
        &image,
    )
    .unwrap();
    let runtime_rent = crate::block_information::land_rent::from_memberships(
        &runtime_refs,
        &build_refs,
        &business_refs,
        players,
        &image,
        &resolved,
    )
    .unwrap();
    for category in 0..52 {
        let slots = |rows: &[crate::block_information::land_rent::Row]| {
            rows.iter()
                .filter_map(|r| match r {
                    crate::block_information::land_rent::Row::Business(b) => {
                        Some((b.slot, b.owner, b.label, b.rent))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            slots(&saved_rent[category]),
            slots(&runtime_rent[category]),
            "original live LandRent links{category}"
        );
    }
    fn events(report: &Report, category: usize, mode: u32, period: u32) -> Vec<(u32, Vec<u32>)> {
        let mut actual = Vec::new();
        for row in report.rows(category, mode as usize + 2, period == 0) {
            match row {
                Row::Empty => actual.push((0, vec![1137])),
                Row::Blank => actual.push((9, vec![])),
                Row::Heading(owner) => actual.push(if *owner == 0 {
                    (2, vec![1116])
                } else {
                    (1, vec![u32::from(*owner)])
                }),
                Row::Total { owner, amount } => {
                    actual.push((8, vec![]));
                    actual.push((if *owner == 0 { 2 } else { 0 }, vec![66]));
                    actual.push((3, vec![amount.to_bits()]));
                    if *amount < 0. {
                        actual.push((7, vec![10, if *owner == 0 { 0 } else { 2 }, 0]));
                    }
                }
                Row::Business {
                    slot,
                    label,
                    value,
                    accent,
                    negative,
                } => {
                    actual.push((0, vec![*label]));
                    actual.push(match value {
                        Value::Integer(v) => (4, vec![*v as u32]),
                        Value::Money(v) => (3, vec![v.to_bits()]),
                        Value::Ratio(a, b) => (5, vec![*a, *b]),
                        Value::Label(v) => (0, vec![*v]),
                    });
                    if mode != 2 {
                        actual.push((
                            7,
                            if *negative {
                                vec![10, if *accent { 2 } else { 0 }, u32::from(*accent)]
                            } else {
                                vec![9, if *accent { 3 } else { 0 }, u32::from(*accent)]
                            },
                        ));
                    }
                    actual.push((6, vec![*slot as u32]));
                }
            }
        }
        actual
    }
    let count = word(&bytes, &mut at);
    for case in 0..count {
        let category = word(&bytes, &mut at) as usize;
        let mode = word(&bytes, &mut at);
        let period = word(&bytes, &mut at);
        let n = word(&bytes, &mut at);
        let expected = (0..n)
            .map(|_| {
                let tag = word(&bytes, &mut at);
                let n = word(&bytes, &mut at);
                (
                    tag,
                    (0..n).map(|_| word(&bytes, &mut at)).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let actual = events(&report, category, mode, period);
        assert_eq!(
            actual, expected,
            "case{case} category{category} mode{mode} period{period}"
        );
    }
    let extra = word(&bytes, &mut at);
    for case in 0..extra {
        let category = word(&bytes, &mut at) as usize;
        let mode = word(&bytes, &mut at);
        let period = word(&bytes, &mut at);
        let business = word(&bytes, &mut at) as usize;
        let n = word(&bytes, &mut at);
        let mut controlled = businesses.clone();
        controlled[business] = bytes[at..at + 544].try_into().unwrap();
        at += 544;
        let expected = (0..n)
            .map(|_| {
                let tag = word(&bytes, &mut at);
                let n = word(&bytes, &mut at);
                (
                    tag,
                    (0..n).map(|_| word(&bytes, &mut at)).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        let report = Report::from_saved_indices(
            &SavedInputs {
                categories: &categories,
                buildings: &buildings,
                businesses: &controlled,
                info: &info,
                products: &products,
                hour,
                tick,
            },
            players,
            &image,
        )
        .unwrap();
        assert_eq!(
            events(&report, category, mode, period),
            expected,
            "controlled negative case{case}"
        );
    }
    assert_eq!(at, bytes.len());
}
