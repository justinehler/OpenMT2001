//! Original404eb0 building-rights gates and4050d0 activation transaction.
use crate::player_manager::word;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub message: Option<u32>,
    pub activate_visible: bool,
    ///404eb0 proceeds to the ordinary building controls at404f68.
    pub open: bool,
}
/// The original owner helper compares resolved player references, rather than
/// treating the current player reference as a roster index. Panel+509 suppresses
/// the activation button when nonzero, but does not change the explanatory text.
pub fn inspect(
    category: Option<&[u8; 380]>,
    owner_reference: u32,
    current_reference: u32,
    panel_mode: u8,
) -> Inspection {
    let Some(row) = category else {
        return Inspection {
            message: None,
            activate_visible: false,
            open: true,
        };
    };
    inspect_values(
        row[0x6d],
        owner_reference == current_reference,
        word(row, 0x78),
        panel_mode,
    )
}
/// Shared404eb0 predicate used by both construction entry and lease controls.
pub fn inspect_values(pending: u8, is_owner: bool, rights: u32, panel_mode: u8) -> Inspection {
    let message = if pending == 1 {
        Some(1406)
    } else if is_owner {
        (rights == 0).then_some(1202)
    } else {
        match rights {
            0 => Some(1201),
            1 => Some(1203),
            _ => None,
        }
    };
    Inspection {
        activate_visible: message == Some(1202) && panel_mode == 0,
        open: message.is_none(),
        message,
    }
}
///4050d0 queues445cc0 before setting the pending byte. The selected category is
/// deliberately re-read after synchronous submission, as in the native body.
pub fn activate(
    categories: &mut [[u8; 380]; 52],
    selected: &mut Option<u32>,
    prior: [u8; 24],
    mut queue: impl FnMut([u8; 24], &mut [[u8; 380]; 52], &mut Option<u32>) -> Result<(), String>,
) -> Result<bool, String> {
    let Some(index) = *selected else {
        return Ok(false);
    };
    let row = categories
        .get(index as usize)
        .ok_or("rights category outside pool")?;
    queue(
        crate::ai_messages::offers::auction::inbox::wait_descriptor(row, prior),
        categories,
        selected,
    )?;
    let index = selected.ok_or("original rights activation dereferences cleared selection")?;
    categories
        .get_mut(index as usize)
        .ok_or("rights category outside pool")?[0x6d] = 1;
    Ok(true)
}

#[test]
#[ignore = "requires original property-rights gate/action observations"]
fn matches_native_property_rights() {
    struct R<'a>(&'a [u8]);
    impl<'a> R<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            word(self.take(4), 0)
        }
    }
    let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_PROPERTY_RIGHTS").unwrap()).unwrap();
    let mut r = R(&bytes);
    assert_eq!(r.take(8), b"MTRGHT01");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 9] = std::array::from_fn(|_| r.word());
        let mut categories = [[0; 380]; 52];
        categories[h[1] as usize] = r.take(380).try_into().unwrap();
        let after = r.take(380);
        let n = r.word();
        let expected: Vec<([u8; 24], u8)> = (0..n)
            .map(|_| (r.take(24).try_into().unwrap(), r.word() as u8))
            .collect();
        let mut selected = (h[5] == 0).then_some(h[1]);
        let mut trace = vec![];
        if h[0] == 0x4050d0 {
            activate(&mut categories, &mut selected, [0; 24], |d, c, _| {
                trace.push((d, c[h[1] as usize][0x6d]));
                Ok(())
            })
            .unwrap();
        }
        let observed = inspect(
            selected.map(|i| &categories[i as usize]),
            if h[2] == 0 {
                0
            } else {
                0x10020000 + (h[2] - 1) * 0x1000
            },
            0x10020000 + h[3] * 0x1000,
            h[4] as u8,
        );
        assert_eq!(
            (
                observed.message.unwrap_or(0),
                observed.open,
                observed.activate_visible
            ),
            (h[6], h[7] != 0, h[8] != 0),
            "gate{case}"
        );
        assert_eq!(
            categories[h[1] as usize].as_slice(),
            after,
            "category{case}"
        );
        assert_eq!(trace, expected, "queue{case}");
    }
    assert!(r.0.is_empty());
}
