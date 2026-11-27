//! Static preview catalog: GAMESTART registration order and direct hub map calls.
//! This is not the campaign's profile-dependent availability/progression list.
#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    pub id: u32,
    pub token: &'static str,
    pub map: Option<&'static str>,
}
pub const TUTORIALS: [Scenario; 13] = [
    tutorial(0, "TUTORIAL1"),
    tutorial(1, "TUTORIAL2"),
    tutorial(2, "TUTORIAL3"),
    tutorial(3, "TUTORIAL4"),
    tutorial(4, "TUTORIAL5"),
    tutorial(5, "TUTORIAL6"),
    tutorial(6, "TUTORIAL6A"),
    tutorial(7, "TUTORIAL7"),
    tutorial(8, "TUTORIAL8"),
    tutorial(9, "TUTORIAL9"),
    tutorial(10, "TUTORIAL10"),
    tutorial(11, "TUTORIAL11"),
    tutorial(12, "TUTORIAL12"),
];
/// Tutorials supported by the retained session host and its compatibility policy.
/// Tutorials5/8 use the documented modern startup repair by default.
pub const fn live_tutorial(id: u32) -> bool {
    matches!(id, 100000..=100012)
}
const fn tutorial(n: u32, token: &'static str) -> Scenario {
    Scenario {
        id: 100000 + n,
        token,
        map: None,
    }
}
pub const CAMPAIGN: [Scenario; 17] = [
    scenario(3, Some("apartmentrim.lua")),
    scenario(4, None),
    scenario(5, Some("apartmentrim.lua")),
    scenario(2, Some("parkfiller.lua")),
    scenario(12, Some("developedmap.lua")),
    scenario(1, Some("apartments2.lua")),
    scenario(6, Some("goodsparsemap.lua")),
    scenario(11, Some("mapbuiltup.lua")),
    scenario(7, Some("goodspread.lua")),
    scenario(8, Some("sparsemap.lua")),
    scenario(9, Some("cheapbuiltmap.lua")),
    scenario(10, Some("mapbuiltupmore.lua")),
    scenario(13, None),
    scenario(14, Some("cheapbuiltbigmap.lua")),
    scenario(15, None),
    scenario(16, None),
    Scenario {
        id: 400001,
        token: "SCENARIOBONUS",
        map: None,
    },
];
const fn scenario(n: u32, map: Option<&'static str>) -> Scenario {
    Scenario {
        id: 200000 + n,
        token: [
            "SCENARIO1",
            "SCENARIO2",
            "SCENARIO3",
            "SCENARIO4",
            "SCENARIO5",
            "SCENARIO6",
            "SCENARIO7",
            "SCENARIO8",
            "SCENARIO9",
            "SCENARIO10",
            "SCENARIO11",
            "SCENARIO12",
            "SCENARIO13",
            "SCENARIO14",
            "SCENARIO15",
            "SCENARIO16",
        ][n as usize - 1],
        map,
    }
}
pub fn find(id: u32) -> Result<Scenario, String> {
    const NET: [Scenario; 7] = [
        Scenario {
            id: 300000,
            token: "NET0",
            map: Some("empty.lua"),
        },
        Scenario {
            id: 300001,
            token: "NET1",
            map: Some("empty.lua"),
        },
        Scenario {
            id: 300002,
            token: "NET2",
            map: Some("empty.lua"),
        },
        Scenario {
            id: 300003,
            token: "NET3",
            map: Some("empty.lua"),
        },
        Scenario {
            id: 300004,
            token: "NET4",
            map: Some("empty.lua"),
        },
        Scenario {
            id: 300005,
            token: "NET5",
            map: Some("empty.lua"),
        },
        Scenario {
            id: 300006,
            token: "NET6",
            map: Some("empty.lua"),
        },
    ];
    CAMPAIGN
        .iter()
        .chain(TUTORIALS.iter())
        .chain(NET.iter())
        .copied()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("unknown preview scenario {id}"))
}

/// Original4e7fe0 %c branch; startup4c1496 selects language2243/2244.
/// Only this no-argument format is needed by the campaign title/description set.
pub fn currency_text(text: &[u8], strings: &[&[u8]]) -> Result<Vec<u8>, String> {
    let symbol = *strings.get(2243).ok_or("currency symbol language entry")?;
    let left = strings.get(2244).and_then(|s| s.first()) == Some(&0x4c);
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with(&[b'%' - 32, b'c' - 32]) {
            i += 2;
            if !left {
                while i < text.len() && (b'0' - 32..=b'9' - 32).contains(&text[i]) {
                    out.push(text[i]);
                    i += 1;
                }
            }
            out.extend_from_slice(symbol);
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original scenario text oracle fixture"]
    fn matches_native_scenario_currency_text() {
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_SCENARIO_TEXT")).unwrap();
        assert_eq!(&b[..8], b"MTSTXT01");
        let mut at = 8;
        fn word(b: &[u8], at: &mut usize) -> usize {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap()) as usize;
            *at += 4;
            v
        }
        let count = word(&b, &mut at);
        let n = word(&b, &mut at);
        let mut strings = vec![&[][..]; 2245];
        strings[2243] = &b[at..at + n];
        at += n;
        strings[2244] = &b[at..at + 1];
        at += 1;
        for _ in 0..count {
            let id = word(&b, &mut at);
            let input = word(&b, &mut at);
            let output = word(&b, &mut at);
            let actual = currency_text(&b[at..at + input], &strings).unwrap();
            at += input;
            assert_eq!(actual, b[at..at + output], "language {id}");
            at += output;
        }
        assert_eq!(at, b.len());
        assert_eq!(count, 34);
    }
    #[test]
    #[ignore = "requires supplied original installer scripts"]
    fn original_campaign_registration_and_maps() {
        let root = std::path::PathBuf::from(openmt2001::fixtures::path("ORIGINAL_SCRIPTS"));
        let registrations = std::fs::read_to_string(root.join("GAMESTART/hub.lua")).unwrap();
        let ids: Vec<u32> = registrations
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("Frontend.RegisterLevel_Id("))
            .filter_map(|l| l.split_once('(')?.1.split(',').next()?.parse().ok())
            .filter(|id| (200001..=200016).contains(id) || *id == 400001)
            .collect();
        assert_eq!(ids, CAMPAIGN.map(|s| s.id));
        for s in CAMPAIGN {
            let registration = format!(
                "Frontend.RegisterLevel_Id({}, \"{}\"",
                s.id,
                s.token.to_ascii_lowercase()
            );
            assert!(registrations.contains(&registration));
            let hub = std::fs::read_to_string(root.join(s.token).join("hub.lua")).unwrap();
            let maps: Vec<&str> = hub
                .lines()
                .map(str::trim)
                .filter(|l| !l.starts_with("--"))
                .filter_map(|l| l.strip_prefix("C_RunScript(\"scripts\\\\maps\\\\"))
                .filter_map(|l| l.split('"').next())
                .collect();
            assert_eq!(maps, s.map.into_iter().collect::<Vec<_>>(), "{}", s.token);
            if let Some(map) = s.map {
                let input = std::fs::read_to_string(root.join("maps").join(map)).unwrap();
                openmt2001::scene::parse_static_map(&input).unwrap();
            }
        }
        let tutorials: Vec<(u32, String)> = registrations
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("Frontend.RegisterLevel_Id(1"))
            .filter_map(|line| {
                let row = line.strip_prefix("Frontend.RegisterLevel_Id(")?;
                let (id, rest) = row.split_once(',')?;
                let token = rest.split('"').nth(1)?;
                Some((id.parse().ok()?, token.to_ascii_uppercase()))
            })
            .collect();
        assert_eq!(
            tutorials,
            TUTORIALS
                .iter()
                .map(|row| (row.id, row.token.to_owned()))
                .collect::<Vec<_>>()
        );
        assert!(root.join("MAPS/tempmap.lua").is_file());
        for tutorial in TUTORIALS {
            assert!(root.join(tutorial.token).join("hub.lua").is_file());
        }
    }
}
