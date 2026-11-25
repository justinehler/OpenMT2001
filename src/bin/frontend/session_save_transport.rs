//! V2-only continuation of road occupancy and retained non-road transport.
use super::{Input, Output, Session, flat, words};
fn rows<const N: usize>(r: &mut Input<'_>, dst: &mut [[u8; N]]) -> Result<(), String> {
    let b = r.blob()?;
    if b.len() != dst.len() * N {
        return Err("V2 transport pool size differs from reconstructed level".into());
    }
    for (row, source) in dst.iter_mut().zip(b.chunks_exact(N)) {
        row.copy_from_slice(source);
    }
    Ok(())
}
pub(super) fn encode(s: &Session<'_>) -> Result<Vec<u8>, String> {
    encode_parts(&s.world.road_network,&s.road_traffic,&s.transport)
}
pub(super) fn encode_parts(
    network:&openmt2001::traffic::road::construction::RoadNetwork,
    roads:&openmt2001::traffic::road::runtime::RoadTraffic,
    t:&openmt2001::traffic::scene::Scene,
)->Result<Vec<u8>,String> {
    let mut out = Output(vec![]);
    for b in [
        flat(&network.roads),
        flat(&network.segments),
        flat(&network.nodes),
        flat(&network.entries),
        flat(&network.lanes),
    ] {
        out.blob(&b)?;
    }
    out.blob(roads.records.as_flattened())?;
    out.word(roads.auxiliaries.used() as u32);
    out.blob(&flat(roads.auxiliaries.slots()))?;
    out.0
        .extend_from_slice(&words(roads.admission.random));
    out.0.extend_from_slice(&words(
        roads
            .admission
            .demand
            .as_flattened()
            .iter()
            .map(|v| *v as u32),
    ));
    out.0
        .extend_from_slice(&words(roads.admission.selected.map(|v| v as u32)));
    out.0
        .extend_from_slice(&words(roads.admission.counts));
    out.word(roads.admission.entries as u32);
    out.word(roads.signals.counter);
    out.word(roads.signals.phase);
    out.0.extend_from_slice(&words(
        roads
            .proximity
            .nearest
            .as_flattened()
            .iter()
            .copied(),
    ));
    out.word(roads.proximity.reciprocal_sum.to_bits());
    out.word(roads.proximity.count as u32);
    out.0.extend_from_slice(&words([
        t.hourly.tier,
        t.hourly.random[0],
        t.hourly.random[1],
        t.hourly.random[2],
        t.hourly.model_choice,
        t.hourly.variant,
    ]));
    out.blob(&flat(&t.hourly.auxiliaries))?;
    out.blob(&flat(&t.hourly.effects))?;
    out.0.extend_from_slice(&words(t.hourly.models));
    out.blob(&flat(&t.aircraft))?;
    out.blob(&flat(&t.trains))?;
    out.0
        .extend_from_slice(&words(t.environment.crane_position.map(f32::to_bits)));
    out.0
        .extend_from_slice(&words(t.environment.ship_position.map(f32::to_bits)));
    out.0.extend_from_slice(&words([
        t.environment.frame,
        t.environment.sequence_tick,
        u32::from(t.environment.sequence_active),
        u32::from(t.environment.frozen),
        u32::from(t.first_frame),
    ]));
    Ok(out.0)
}
pub(super) fn restore(s: &mut Session<'_>, bytes: &[u8]) -> Result<(), String> {
    let mut r = Input::new(bytes);
    let network = &mut s.world.road_network;
    rows(&mut r, &mut network.roads)?;
    rows(&mut r, &mut network.segments)?;
    rows(&mut r, &mut network.nodes)?;
    rows(&mut r, &mut network.entries)?;
    rows(&mut r, &mut network.lanes)?;
    rows(&mut r, s.road_traffic.records.as_mut())?;
    let used = r.word()? as usize;
    let bytes = r.blob()?;
    if bytes.len() != 200 * 284 {
        return Err("V2 vehicle auxiliary pool size invalid".into());
    }
    s.road_traffic.auxiliaries = openmt2001::resident::AuxiliaryPool::from_slots(
        bytes
            .chunks_exact(284)
            .map(|b| b.try_into().unwrap())
            .collect(),
        used,
    )?;
    for value in &mut s.road_traffic.admission.random {
        *value = r.word()?;
    }
    for value in s.road_traffic.admission.demand.as_flattened_mut() {
        *value = r.word()? as i32;
    }
    for value in &mut s.road_traffic.admission.selected {
        *value = r.word()? as i32;
    }
    for value in &mut s.road_traffic.admission.counts {
        *value = r.word()?;
    }
    s.road_traffic.admission.entries = r.word()? as i32;
    s.road_traffic.signals.counter = r.word()?;
    s.road_traffic.signals.phase = r.word()?;
    for value in s.road_traffic.proximity.nearest.as_flattened_mut() {
        *value = r.word()?;
    }
    s.road_traffic.proximity.reciprocal_sum = f32::from_bits(r.word()?);
    s.road_traffic.proximity.count = r.word()? as usize;
    if s.road_traffic.proximity.count > 200 {
        return Err("V2 traffic proximity count invalid".into());
    }
    let t = &mut s.transport;
    t.hourly.tier = r.word()?;
    for value in &mut t.hourly.random {
        *value = r.word()?;
    }
    t.hourly.model_choice = r.word()?;
    t.hourly.variant = r.word()?;
    rows(&mut r, &mut t.hourly.auxiliaries)?;
    rows(&mut r, &mut t.hourly.effects)?;
    for value in &mut t.hourly.models {
        *value = r.word()?;
    }
    rows(&mut r, &mut t.aircraft)?;
    rows(&mut r, &mut t.trains)?;
    for value in &mut t.environment.crane_position {
        *value = f32::from_bits(r.word()?);
    }
    for value in &mut t.environment.ship_position {
        *value = f32::from_bits(r.word()?);
    }
    t.environment.frame = r.word()?;
    t.environment.sequence_tick = r.word()?;
    t.environment.sequence_active =
        u8::try_from(r.word()?).map_err(|_| "V2 transport active byte invalid")?;
    t.environment.frozen =
        u8::try_from(r.word()?).map_err(|_| "V2 transport frozen byte invalid")?;
    t.first_frame = match r.word()? {
        0 => false,
        1 => true,
        _ => return Err("V2 transport first-frame flag invalid".into()),
    };
    r.end()
}
