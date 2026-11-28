//! Moving transport, road, crowd, environment, construction, block and coast
//! sound requests (original 4aefc0 dispatcher). The binary's
//! `transport_audio::Player` serves them.
use crate::traffic::{RouteSound, scene::EnvironmentCall};
use std::sync::mpsc;

#[derive(Clone, Debug)]
pub enum Operation {
    /// Original 4af140 clears the retained city sound handles when leaving
    /// gameplay for a result or an auction transition.
    Reset,
    /// Replace simulation-owned inputs without advancing frame presentation.
    City(Box<CityInputs>),
    Route(RouteSound),
    /// Original 4af860 consumes 650330 and three ranked 63f9c0 rows,
    /// produced together by road::audio::Proximity.
    RoadProximity {
        reciprocal_sum: f32,
        nearest: [[u32; 3]; 3],
    },
    /// Original 4afa40 weights the 52 category resident counts by camera
    /// distance before starting or stopping the busy-people city bed.
    ResidentCrowd(f32),
    /// Original 44ec50 calls 4addd0/4ae2b0/4ae1f0 with the current sound
    /// position. The Host computes distance from its retained camera.
    Environment {
        call: EnvironmentCall,
        distance: f32,
        position: [f32; 3],
    },
    /// Original 4b0750 keeps the two nearest construction positions per
    /// stage; 4adb10/4afcde present their four sound beds.
    Construction([[Option<ConstructionSource>; 2]; 4]),
    /// Original 4afd65 presents block ambience after the construction beds.
    Blocks(Box<BlockSnapshot>),
    /// Original 4afd6c services the retained nonspatial coast loop after blocks.
    Coast {
        camera_z: f32,
    },
}

/// Produced only when simulation advances; camera-dependent presentation is
/// evaluated by the audio owner on every draw, including ordinary pause.
#[derive(Clone, Debug)]
pub struct CityInputs {
    pub reciprocal_sum: f32,
    pub nearest: [[u32; 3]; 3],
    pub categories: [[u8; 0x17c]; 52],
    pub construction: [[Option<ConstructionSource>; 2]; 4],
    pub hour: u32,
    pub tick: u32,
    pub previous_tick: u32,
    pub initial_random_state: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstructionSource {
    pub distance: f32,
    pub position: [f32; 3],
}

/// The native 4b0750 queue inserts after equal distances and discards any
/// candidate beyond the two nearest entries. The camera is 44d060's global.
pub fn insert_construction_source(
    groups: &mut [[Option<ConstructionSource>; 2]; 4],
    group: usize,
    position: [f32; 3],
    camera: [f32; 3],
) -> Result<(), String> {
    let entries = groups
        .get_mut(group)
        .ok_or("construction sound group outside native pool")?;
    // 44d060 keeps the component differences and square root on the x87
    // stack until 4b0750 stores the winning distance as f32.
    let delta: [f64; 3] =
        std::array::from_fn(|axis| f64::from(position[axis]) - f64::from(camera[axis]));
    let distance = ((delta[2] * delta[2] + delta[1] * delta[1]) + delta[0] * delta[0]).sqrt();
    if !distance.is_finite() {
        return Err("construction sound distance is nonfinite".into());
    }
    let source = ConstructionSource {
        distance: distance as f32,
        position,
    };
    if entries[0].is_none_or(|entry| distance < f64::from(entry.distance)) {
        entries[1] = entries[0];
        entries[0] = Some(source);
    } else if entries[1].is_none_or(|entry| distance < f64::from(entry.distance)) {
        entries[1] = Some(source);
    }
    Ok(())
}

pub struct Request {
    pub operation: Operation,
}

pub fn submit(sender: &Option<mpsc::Sender<Request>>, operation: Operation) -> Result<(), String> {
    // Headless Host validation runs the same simulation without a UI owner.
    let Some(sender) = sender else { return Ok(()) };
    // Inspection retirement drops its audio receiver before the worker's
    // 16 ms loop notices its session channel closed. Sound presentation must
    // not turn that ordinary teardown frame into a simulation failure.
    let _ = sender.send(Request { operation });
    Ok(())
}

/// Original 450080 block sound positions with the clock and the 8af3ec
/// startup RNG stream, for the binary's 4aec00 block-sound controller.
#[derive(Clone, Copy, Debug)]
pub struct BlockSnapshot {
    pub centers: [Option<[f32; 2]>; 52],
    pub camera: [f32; 3],
    pub hour: u32,
    pub tick: u32,
    pub previous_tick: u32,
    pub initial_random_state: u32,
}

impl BlockSnapshot {
    pub fn from_categories(
        categories: &[[u8; 0x17c]; 52],
        camera: [f32; 3],
        hour: u32,
        tick: u32,
        previous_tick: u32,
        initial_random_state: u32,
    ) -> Self {
        Self {
            centers: std::array::from_fn(|index| {
                let row = &categories[index];
                (row[4] != 0).then(|| {
                    [
                        f32::from_le_bytes(row[0xe0..0xe4].try_into().unwrap()),
                        f32::from_le_bytes(row[0xe4..0xe8].try_into().unwrap()),
                    ]
                })
            }),
            camera,
            hour,
            tick,
            previous_tick,
            initial_random_state,
        }
    }
}
