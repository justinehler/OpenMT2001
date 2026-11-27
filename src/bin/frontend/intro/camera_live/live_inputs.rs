//! Concrete original-record producers for graph installation/refresh.
//! Invoke at the live session boundary while its actual world is available.
//! This does not query/wait for Host and does not infer a mode from a camera.
use super::ui_graph::{Building, Property, ReportInputs};
use openmt2001::{level_startup::World, pe::PeImage, player_manager::Manager};
pub struct Inputs {
    pub reports: ReportInputs,
    pub colors: [u32; 600],
    pub human_enabled: u8,
}
fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or("UI producer record truncated")?
            .try_into()
            .unwrap(),
    ))
}
pub fn capture(
    world: &World,
    manager: &Manager,
    current_reference: u32,
    image: &PeImage<'_>,
) -> Result<Inputs, String> {
    let current = manager
        .players
        .iter()
        .find(|player| player.reference == current_reference)
        .ok_or("UI install requires actual86cfc0 human reference")?;
    let human_enabled = *current.bytes.get(0x10).ok_or("UI human record truncated")?;
    if world.buildings.len() != 600 {
        return Err("D producer requires actual600-slot building arena".into());
    }
    let mut buildings = [Building {
        active: 0,
        owner_color: None,
    }; 600];
    let mut colors = [0u32; 600];
    for (slot, record) in world.buildings.iter().enumerate() {
        colors[slot] = word(record, 0x90)?;
        buildings[slot].active = record[0x3f];
        if record[0x3f] == 0 {
            continue;
        }
        //4454a0 uses building land owner byte2d, never business proprietor.
        let encoded = record[0x2d] as i8;
        if encoded == 0 {
            continue;
        }
        if encoded < 0 {
            return Err("D owner byte is a negative source player index".into());
        }
        let player = manager
            .players
            .get(encoded as usize - 1)
            .ok_or("D owner leaves actual player roster")?;
        let character = word(&player.bytes, 0x150)?;
        if character >= 10 {
            return Err("D owner palette leaves original10 rows".into());
        }
        buildings[slot].owner_color = Some(u32::from_le_bytes(
            image.at(0x541af0 + character * 40, 4)?.try_into().unwrap(),
        ));
    }
    let properties = std::array::from_fn(|i| {
        let record = &world.categories[i];
        //Originalg_human_properties+4: source[0]=category+4,
        //source[88]=category+8c, source[8c]=category+90.
        Property {
            active: record[4],
            excluded: record[0x90],
            title: i32::from_le_bytes(record[0x8c..0x90].try_into().unwrap()),
        }
    });
    let titles = (0..46)
        .map(|i| {
            image
                .at(0x5385a0 + i * 88, 4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Inputs {
        reports: ReportInputs {
            buildings,
            properties,
            titles,
        },
        colors,
        human_enabled,
    })
}
///Use at initial construction only. Ordinary scene refresh updates live input
///suppliers but must preserve already-owned transient colors/legend/mode.
pub fn refresh_suppliers(graph: &mut super::ui_graph::UiGraphState, input: Inputs) {
    graph.human_enabled = input.human_enabled;
    graph.report_inputs = input.reports;
}

///Exact actual load boundary command from timing tag400 (24-byte payload).
///Native restore calls4b5370 only when the current human pointer is nonnull.
///Saved camera sections are independent and may be restored later by root.
pub fn restored_mode_command(
    tag: u32,
    payload: &[u8],
    current_human_present: bool,
) -> Result<Option<i32>, String> {
    if tag != 0x400 || payload.len() != 24 {
        return Err("UI mode producer needs actual timing400/24 section".into());
    }
    if !current_human_present {
        return Ok(None);
    }
    Ok(Some(i32::from_le_bytes(
        payload[16..20].try_into().unwrap(),
    )))
}
