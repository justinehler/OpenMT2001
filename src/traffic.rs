//! Original lane signal phase0044c210 and hourly scene scheduling0044f2a0.
//! Train/aircraft record creation is shared simulation logic. Vehicle motion,
//! model lookup and audio remain explicit boundaries; lane occupancy is separate.

#[cfg(test)]
mod environment_tests;
#[cfg(test)]
mod frame_tests;
#[cfg(test)]
mod locator_tests;
pub mod models;
pub mod proximity;
#[cfg(test)]
mod proximity_tests;
pub mod road;
pub mod scene;
pub mod train_motion;

pub struct Signals {
    pub counter: u32,
    pub phase: u32,
    patterns: [[u8; 32]; 2],
}

/// Original44c0e0 road-vehicle admission state. The entry divisor is6858e0;
/// it is supplied by route construction, not inferred from a road vector length.
pub struct Admission {
    pub random: [u32; 3],
    /// Taxi then car: six script bands plus the adjacent native word read at
    /// hour24 (signal counter for taxis, VanFrequency._0to3 for cars).
    pub demand: [[i32; 7]; 2],
    pub selected: [i32; 2],
    pub counts: [u32; 2],
    pub entries: i32,
}

impl Admission {
    /// Bind live startup_settings traffic (car, van, taxi). Original4daed6..
    /// 4db034 registers the six-word arrays contiguously at684b74/8c/a4.
    /// Preserve hour24 adjacency instead of inventing a seventh script setting.
    /// Refresh before admission when scripts/signals change.
    pub fn refresh_demands(&mut self, frequencies: &[[i32; 6]; 3], signal_counter: u32) {
        self.demand[0][..6].copy_from_slice(&frequencies[2]);
        self.demand[1][..6].copy_from_slice(&frequencies[0]);
        self.demand[0][6] = signal_counter as i32;
        self.demand[1][6] = frequencies[1][0];
    }

    /// Compose admission with the actual road-record producer. Model binding
    /// remains synchronous and can update the same live admission state.
    pub fn advance_with_vehicles(
        &mut self,
        hour: i32,
        era: u32,
        vehicles: &mut RoadVehicles<'_>,
        mut bind: impl FnMut(u32, u32, &mut RoadVehicles<'_>, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        self.advance(hour, |args, admission| {
            vehicles.spawn(args, era, |model, auxiliary, vehicles| {
                bind(model, auxiliary, vehicles, admission)
            })?;
            Ok(())
        })
    }

    /// Preserve the original RNG draws and synchronous44b880 boundaries.
    /// Native counters increment after callbacks even when creation returns null.
    pub fn advance(
        &mut self,
        hour: i32,
        mut create: impl FnMut([i32; 3], &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        let band = usize::try_from(hour / 4).map_err(|_| "negative traffic hour band")?;
        for i in 0..2 {
            self.selected[i] = *self.demand[i]
                .get(band)
                .ok_or("traffic hour band outside tables")?;
        }
        let [seed, multiplier, increment] = self.random;
        let first = seed.wrapping_mul(multiplier).wrapping_add(increment);
        let second = first.wrapping_mul(multiplier).wrapping_add(increment);
        if self.entries == 0 {
            return Err("zero vehicle entry divisor".into());
        }
        let start = ((first >> 16) & 0x7fff) as i32 % self.entries;
        self.random[0] = second;
        let end = ((second >> 16) & 0x7fff) as i32 % self.entries;
        if start == end {
            return Ok(());
        }
        if (self.counts[0] as i32) < self.selected[0] {
            create([start, end, 5], self)?;
            self.counts[0] = self.counts[0].wrapping_add(1);
        }
        if (self.counts[1] as i32) < self.selected[1] {
            // The first callback may reseed the same stream or change its coefficients.
            let [seed, multiplier, increment] = self.random;
            let third = seed.wrapping_mul(multiplier).wrapping_add(increment);
            let fourth = third.wrapping_mul(multiplier).wrapping_add(increment);
            self.random[0] = fourth;
            let mut kind = ((third >> 16) & 0x7fff) as i32 % 18 + 6;
            if ((fourth >> 16) & 0x7fff).is_multiple_of(26) {
                kind = 3;
            }
            create([start, end, kind], self)?;
            self.counts[1] = self.counts[1].wrapping_add(1);
        }
        Ok(())
    }
}

/// Borrowed original road-vehicle and entry records. References are opaque
/// 32-bit IDs in a contiguous namespace, never native host pointers. The
/// original image uses reference_base63f9e8; a restored pool may be rebased.
pub struct RoadVehicles<'a> {
    pub records: &'a mut [[u8; 208]; 200],
    /// Original684bd8: 52-byte rows, count+4, queue head+2c, tail+30.
    pub entries: &'a mut [[u8; 52]],
    pub reference_base: u32,
}

/// The vehicle/auxiliary pool portion of44bac0, after its separate44ba30 route
/// reset. The caller supplies prior template bytes and an opaque auxiliary
/// namespace. Route reset and the later presentation globals are not performed.
pub fn initialize_road_vehicles(
    records: &mut [[u8; 208]; 200],
    template: &mut [u8; 284],
    auxiliary_base: u32,
    mut bind: impl FnMut(u32, &mut [u8; 284], &mut [[u8; 208]; 200]) -> Result<(), String>,
) -> Result<crate::resident::AuxiliaryPool, String> {
    if auxiliary_base == 0 || auxiliary_base.checked_add(200 * 284 - 1).is_none() {
        return Err("invalid road auxiliary namespace".into());
    }
    for (index, row) in records.iter_mut().enumerate() {
        row.fill(0);
        put(row, 0xc0, index as u32);
    }
    let mut auxiliaries = crate::resident::AuxiliaryPool::new(200);
    crate::resident::initialize_auxiliary(template);
    for index in 0..200 {
        template[0x30..0x32].copy_from_slice(&3_u16.to_le_bytes());
        put(template, 0x38, index as u32);
        auxiliaries
            .allocate(template)
            .ok_or("road auxiliary allocation failed")?;
        put(
            &mut records[index],
            0xc4,
            auxiliary_base + index as u32 * 284,
        );
        put(&mut records[index], 0x1c, 0);
        put(&mut records[index], 0x0c, index as u32);
        bind(0x689080, auxiliaries.get_mut(index).unwrap(), records)?;
        // The original binds the base model but stores index1 here.
        put(&mut records[index], 8, 1);
    }
    Ok(auxiliaries)
}

impl RoadVehicles<'_> {
    fn reference(&self, index: usize) -> Result<u32, String> {
        self.reference_base
            .checked_add(index as u32 * 208)
            .filter(|&reference| reference != 0)
            .ok_or_else(|| "invalid vehicle reference namespace".into())
    }

    fn index(&self, reference: u32) -> Result<usize, String> {
        let offset = reference
            .checked_sub(self.reference_base)
            .ok_or("vehicle reference precedes pool")?;
        if !offset.is_multiple_of(208) || offset / 208 >= 200 {
            return Err("vehicle reference outside pool".into());
        }
        Ok((offset / 208) as usize)
    }

    /// Original44b880: first inactive record, append to entry queue, then bind
    /// its retained auxiliary. Bytes not written by the original stay intact.
    /// Exhaustion is an explicit error: the original instead dereferences null.
    pub fn spawn(
        &mut self,
        [start, destination, kind]: [i32; 3],
        era: u32,
        mut bind: impl FnMut(u32, u32, &mut Self) -> Result<(), String>,
    ) -> Result<u32, String> {
        let index = self
            .records
            .iter()
            .position(|row| word(row, 0x1c) == 0)
            .ok_or("road vehicle pool exhausted (original null dereference)")?;
        let reference = self.reference(index)?;
        let entry = usize::try_from(start).map_err(|_| "negative vehicle entry")?;
        let tail = word(
            self.entries
                .get(entry)
                .ok_or("vehicle entry outside table")?,
            0x30,
        );
        if tail != 0 {
            let previous = self.index(tail)?;
            put(&mut self.records[previous], 0xb0, reference);
        } else {
            put(&mut self.entries[entry], 0x2c, reference);
        }
        put(&mut self.records[index], 0xac, tail);
        put(&mut self.entries[entry], 0x30, reference);
        put(&mut self.records[index], 0xb0, 0);
        let count = word(&self.entries[entry], 4).wrapping_add(1);
        put(&mut self.entries[entry], 4, count);
        for (offset, value) in [
            (0x34, start as u32),
            (0x7c, destination as u32),
            (0x1c, 11),
            (0x38, 0x40),
            (0x3c, 0x800),
            (0x78, 0),
            (0x18, kind as u32),
        ] {
            put(&mut self.records[index], offset, value);
        }
        let offset: u32 = match era {
            0..=2 => 0,
            3..=4 => 29,
            _ => 58,
        };
        let model_index = offset.wrapping_add(kind as u32);
        let model = 0x689080_u32.wrapping_add(model_index.wrapping_mul(16));
        let auxiliary = word(&self.records[index], 0xc4);
        bind(model, auxiliary, self)?;
        put(&mut self.records[index], 8, model_index);
        Ok(reference)
    }
}

/// Inputs read by original44f2a0. Callbacks may change the live inputs before
/// later native-style reads; era-derived tier is captured at entry.
pub struct HourlyInputs {
    pub era: u32,
    pub hour: u32,
    pub population: i32,
    pub tick: u32,
}

/// Original route endpoints are vertex2 in cached mesh frames1 and2.
#[derive(Clone, Copy, Debug)]
pub struct RouteGeometry {
    pub start: [f32; 3],
    pub end: [f32; 3],
}

impl RouteGeometry {
    /// Original4bea68..4beaee registers these eight train locator meshes into
    /// table6860e8. Read their paths from the supplied executable and their
    /// samples from the supplied archive, keeping filesystem I/O outside.
    pub fn load_routes(
        image: &crate::pe::PeImage<'_>,
        index: &crate::archive::ArchiveIndex,
        archive: &[u8],
    ) -> Result<[Self; 8], String> {
        let mut routes = Vec::with_capacity(8);
        for address in [
            0x5493b0, 0x549384, 0x549358, 0x54932c, 0x549304, 0x5492d8, 0x5492ac, 0x54927c,
        ] {
            let path = image.at(address, 256)?;
            let end = path
                .iter()
                .position(|&b| b == 0)
                .ok_or("unterminated train locator path")?;
            let path =
                std::str::from_utf8(&path[..end]).map_err(|_| "invalid train locator path")?;
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing train locator {path}"))?;
            let start =
                usize::try_from(entry.offset).map_err(|_| "train locator offset overflow")?;
            let end = usize::try_from(
                entry
                    .offset
                    .checked_add(entry.length)
                    .ok_or("train locator range overflow")?,
            )
            .map_err(|_| "train locator range overflow")?;
            let bytes = archive
                .get(start..end)
                .ok_or("train locator outside archive")?;
            routes.push(Self::from_mesh(&crate::mesh::CachedMesh::parse(bytes)?)?);
        }
        routes.try_into().map_err(|_| "train locator count".into())
    }

    pub fn from_mesh(mesh: &crate::mesh::CachedMesh<'_>) -> Result<Self, String> {
        if mesh.vertex_count < 3 {
            return Err("transport route requires vertex2".into());
        }
        Ok(Self {
            start: mesh.positions(1)?[2],
            end: mesh.positions(2)?[2],
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RouteSound {
    pub id: u32,
    pub distance: f32,
    pub arguments: [u32; 2],
}

fn length3(vector: [crate::x87::Sum; 3]) -> Result<crate::x87::Sum, String> {
    let squares = vector.map(|mut value| {
        value.mul(value);
        value
    });
    let mut total = squares[0];
    total.add(squares[1]);
    total.add(squares[2]);
    total.sqrt().map_err(str::to_owned)
}

/// Original44e480, including4e2af0/4e2b20 and sound distance arithmetic.
/// Model lookup and sound submission are explicit boundaries; neither requires
/// a native pointer or host audio API in the shared simulation.
pub fn spawn_route(
    records: &mut [[u8; 60]; 10],
    tier: u32,
    route: u32,
    tick: u32,
    geometry: impl FnOnce(u32) -> Result<RouteGeometry, String>,
    camera: [f32; 3],
    mut sound: impl FnMut(RouteSound, &mut [[u8; 60]; 10]) -> Result<(), String>,
) -> Result<bool, String> {
    use crate::x87::Sum;
    let Some(row) = records.iter_mut().find(|row| word(*row, 0) == 0) else {
        return Ok(false);
    };
    put(row, 0, 1);
    put(row, 4, tier);
    put(row, 8, route);
    put(row, 12, 0);
    put(row, 16, tick);
    put(row, 0x30, 2_f32.to_bits());
    let geometry = geometry(route)?;
    let mut direction = [0.; 3];
    let mut camera_vector = [Sum::default(); 3];
    for axis in 0..3 {
        let start = geometry.start[axis];
        put(row, 0x18 + axis * 4, start.to_bits());
        let mut delta = Sum::from_f32(geometry.end[axis])?;
        delta.add(Sum::from_f32(start)?.negated());
        direction[axis] = delta.to_f32();
        let mut distance = Sum::from_f32(start)?;
        if axis == 2 {
            distance = distance.negated();
        }
        distance.add(Sum::from_f32(camera[axis])?.negated());
        camera_vector[axis] = distance;
    }
    let length = length3([
        Sum::from_f32(direction[0])?,
        Sum::from_f32(direction[1])?,
        Sum::from_f32(direction[2])?,
    ])?;
    put(row, 0x34, length.to_f32().to_bits());
    put(row, 0x38, length.to_f32().to_bits());
    let normalized = if direction.iter().all(|&v| v == 0.) {
        // Original zero-length normalization executes1/0 then0*infinity.
        [f32::from_bits(0xffc00000); 3]
    } else {
        let mut inverse = Sum::from_f32(1.)?;
        inverse.div(length).map_err(|e|format!("transport route direction normalization: {e}"))?;
        let mut result = [0.; 3];
        for (output, component) in result.iter_mut().zip(direction) {
            let mut value = inverse;
            value.mul_f32(component)?;
            *output = value.to_f32();
        }
        result
    };
    for (axis, value) in normalized.into_iter().enumerate() {
        put(row, 0x24 + axis * 4, value.to_bits());
    }
    // Inline44e5a7 sums Z²+Y² before X²; the reusable vector helper sums X²+Y²+Z².
    let distance = length3([camera_vector[2], camera_vector[1], camera_vector[0]])?.to_f32();
    sound(
        RouteSound {
            id: 8,
            distance,
            arguments: [0, 0],
        },
        records,
    )?;
    Ok(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HourlyCall {
    ///44e480(tier, route). Its model geometry and sound work remain explicit.
    Route(u32, u32),
    ///44df70(tier, variant): plane1940/737/747 selected by era tier.
    Secondary(u32, u32),
    ///4df9c0 on the selected300-byte record's embedded auxiliary at+10.
    InitializeAuxiliary(usize),
    ///4dfa40 on that auxiliary, with original chopper registration687550.
    BindAuxiliary(usize, u32),
}

/// Original44f2a0 hourly scene scheduling, separate from lane signal/motion.
/// Storage and model handles are supplied by scene startup/load; this type does
/// not claim that zero storage is a complete initialized transport system.
pub struct HourlyEffects {
    pub tier: u32,
    pub random: [u32; 3],
    pub model_choice: u32,
    pub variant: u32,
    pub auxiliaries: [[u8; 300]; 2],
    /// The original scans eight slots, then tests the ninth sentinel record.
    pub effects: [[u8; 20]; 9],
    pub models: [u32; 4],
}

impl HourlyEffects {
    /// Compose44f2a0 with44df70 and the shared4df9c0 initializer. Route geometry
    /// and model lookup remain explicit host-independent asset callbacks.
    pub fn advance_with_secondary(
        &mut self,
        secondary: &mut [[u8; 304]; 8],
        inputs: &mut HourlyInputs,
        mut bind: impl FnMut(u32, &mut [u8; 0x11c], &mut HourlyInputs) -> Result<(), String>,
        mut route: impl FnMut(u32, u32, &mut Self, &mut HourlyInputs) -> Result<(), String>,
    ) -> Result<(), String> {
        self.advance(inputs, |call, state, inputs| {
            match call {
                HourlyCall::Route(tier, index) => route(tier, index, state, inputs)?,
                HourlyCall::Secondary(tier, variant) => {
                    spawn_secondary(secondary, tier, variant, inputs, &mut bind)?;
                }
                HourlyCall::InitializeAuxiliary(index) => {
                    crate::resident::initialize_auxiliary(
                        (&mut state.auxiliaries[index][16..]).try_into().unwrap(),
                    );
                }
                HourlyCall::BindAuxiliary(index, reference) => bind(
                    reference,
                    (&mut state.auxiliaries[index][16..]).try_into().unwrap(),
                    inputs,
                )?,
            }
            Ok(())
        })
    }

    pub fn advance(
        &mut self,
        inputs: &mut HourlyInputs,
        mut callback: impl FnMut(HourlyCall, &mut Self, &mut HourlyInputs) -> Result<(), String>,
    ) -> Result<(), String> {
        let tier = match inputs.era {
            0..=2 => 0,
            3..=4 => 1,
            _ => 2,
        };
        self.tier = tier;
        let secondary = HourlyCall::Secondary(tier, u32::from(tier != 0));
        let hour = inputs.hour;
        let route = match hour {
            5 | 13 => Some(0),
            6 | 14 => Some(1),
            7 | 15 => Some(2),
            8 | 16 => Some(3),
            9 | 17 => Some(4),
            10 | 18 => Some(5),
            11 | 19 => Some(6),
            12 | 20 => Some(7),
            _ => None,
        };
        if let Some(route) = route {
            callback(HourlyCall::Route(tier, route), self, inputs)?;
        }
        if hour == 12 {
            if let Some(index) = self.auxiliaries.iter().position(|row| word(row, 0) == 0) {
                let row = &mut self.auxiliaries[index];
                put(row, 0, 1);
                put(row, 4, 0);
                put(row, 8, 0);
                put(row, 12, inputs.tick.wrapping_sub(1));
                callback(HourlyCall::InitializeAuxiliary(index), self, inputs)?;
                callback(HourlyCall::BindAuxiliary(index, 0x687550), self, inputs)?;
            }
            if inputs.population >= 400 {
                callback(secondary, self, inputs)?;
            }
            let draw = |state: &mut [u32; 3]| {
                state[0] = state[0].wrapping_mul(state[1]).wrapping_add(state[2]);
                (state[0] >> 16) & 0x7fff
            };
            self.model_choice = draw(&mut self.random) % 4;
            self.variant = draw(&mut self.random) % 101 + 1;
            if self.variant > 2 {
                self.variant = 0;
            }
            if inputs.era <= 2 && self.model_choice == 2 {
                self.model_choice = 0;
            } else if (3..=7).contains(&inputs.era) && self.model_choice == 0 {
                self.model_choice = 2;
            }
            if let Some(row) = self.effects.iter_mut().find(|row| word(*row, 0) == 0) {
                put(row, 0, 1);
                put(row, 4, self.models[self.model_choice as usize]);
                put(
                    row,
                    8,
                    if self.variant == 1 {
                        0x6876a0
                    } else {
                        0x687690
                    },
                );
                put(row, 12, 1);
                put(row, 16, inputs.tick);
            }
        } else {
            let spawn = match hour {
                6 | 14 | 22 => inputs.population >= 200,
                8 | 20 => inputs.population >= 800,
                16 => inputs.population > 400,
                10 | 18 => true,
                _ => false,
            };
            if spawn {
                callback(secondary, self, inputs)?;
            }
        }
        Ok(())
    }
}

/// Original44df70 aircraft creation. Select the first inactive304-byte record, initialize its
/// embedded auxiliary, then resolve the tier's model. Full pools return false
/// without changing any record or calling the model binder.
pub fn spawn_secondary(
    records: &mut [[u8; 304]; 8],
    tier: u32,
    variant: u32,
    inputs: &mut HourlyInputs,
    mut bind: impl FnMut(u32, &mut [u8; 0x11c], &mut HourlyInputs) -> Result<(), String>,
) -> Result<bool, String> {
    let Some(row) = records.iter_mut().find(|row| word(*row, 0) == 0) else {
        return Ok(false);
    };
    put(row, 0, 1);
    put(row, 4, tier);
    put(row, 8, variant);
    put(row, 12, 0);
    put(row, 16, inputs.tick.wrapping_sub(1));
    let auxiliary: &mut [u8; 0x11c] = (&mut row[20..]).try_into().unwrap();
    crate::resident::initialize_auxiliary(auxiliary);
    bind(
        0x6887e0_u32.wrapping_add(tier.wrapping_mul(16)),
        auxiliary,
        inputs,
    )?;
    Ok(true)
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

impl Signals {
    /// Read the original two signal strings. A fresh process starts counter0;
    /// reload callers must supply their retained counter instead.
    pub fn from_v14(image: &crate::pe::PeImage<'_>, counter: u32) -> Result<Self, String> {
        Ok(Self {
            counter,
            phase: 0,
            patterns: [
                image.at(0x539b80, 32)?.try_into().unwrap(),
                image.at(0x539ba4, 32)?.try_into().unwrap(),
            ],
        })
    }

    ///0044c217..0044c2a9, after spawn and before vehicle updates. Original signed
    /// division truncates toward zero, even after the32-bit counter wraps.
    pub fn advance(&mut self, lanes: &mut [[u8; 32]]) {
        self.counter = self.counter.wrapping_add(1);
        self.phase = ((self.counter as i32 / 25).wrapping_mul(2) as u32) & 31;
        let a = self.patterns[0][self.phase as usize];
        let b = self.patterns[1][self.phase as usize];
        let green = if a == b'G' {
            5_u32
        } else if b == b'G' {
            10
        } else {
            0
        };
        let amber = if a == b'A' {
            5_u32
        } else if b == b'A' {
            10
        } else {
            0
        };
        for row in lanes {
            row[..4].copy_from_slice(&green.to_le_bytes());
            row[4..8].copy_from_slice(&amber.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original composed vehicle admission/creation observations"]
    fn matches_native_composed_vehicles() {
        use super::*;
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_VEHICLE_COMPOSED").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTVCOMP1");
        assert_eq!(word(&data, 8), 1);
        let mut at = 16;
        const BYTES: usize = 200 * 208 + 64 * 52;
        for case in 0..word(&data, 12) {
            let input: [u32; 14] = std::array::from_fn(|i| word(&data, at + i * 4));
            assert_eq!(input[9], 0, "case {case} native fault");
            at += 56;
            let before_state: [u32; 23] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 92;
            let after_state: [u32; 23] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 92 + 16;
            let before = &data[at..at + BYTES];
            at += BYTES + 16;
            let after = &data[at..at + BYTES];
            at += BYTES;
            let mut expected = vec![];
            for _ in 0..input[7] {
                expected.push((
                    word(&data, at + 28),
                    std::array::from_fn::<_, 4, _>(|i| word(&data, at + i * 4)),
                ));
                at += 32;
            }
            let mut admission = Admission {
                random: before_state[1..4].try_into().unwrap(),
                selected: [before_state[4] as i32, before_state[5] as i32],
                counts: before_state[6..8].try_into().unwrap(),
                entries: before_state[8] as i32,
                demand: std::array::from_fn(|r| {
                    std::array::from_fn(|i| before_state[9 + r * 7 + i] as i32)
                }),
            };
            let mut records =
                std::array::from_fn(|i| before[i * 208..(i + 1) * 208].try_into().unwrap());
            let mut entries: [[u8; 52]; 64] = std::array::from_fn(|i| {
                before[41600 + i * 52..41600 + (i + 1) * 52]
                    .try_into()
                    .unwrap()
            });
            let mut vehicles = RoadVehicles {
                records: &mut records,
                entries: &mut entries,
                reference_base: 0x63f9e8,
            };
            let mut actual = vec![];
            for tick in 0..input[11] {
                admission
                    .advance_with_vehicles(
                        input[0] as i32,
                        input[4],
                        &mut vehicles,
                        |model, auxiliary, vehicles, _| {
                            let row = vehicles
                                .records
                                .iter()
                                .find(|row| word(*row, 0xc4) == auxiliary)
                                .unwrap();
                            actual.push((
                                tick,
                                [0x44b880, word(row, 0x34), word(row, 0x7c), word(row, 0x18)],
                            ));
                            actual.push((tick, [0x4dfa40, auxiliary, model, 0]));
                            Ok(())
                        },
                    )
                    .unwrap();
            }
            assert_eq!(actual, expected, "case {case} ordered producers/bindings");
            for (i, row) in records.iter().enumerate() {
                assert_eq!(
                    row.as_slice(),
                    &after[i * 208..(i + 1) * 208],
                    "case {case} vehicle {i}"
                );
            }
            for (i, row) in entries.iter().enumerate() {
                assert_eq!(
                    row.as_slice(),
                    &after[41600 + i * 52..41600 + (i + 1) * 52],
                    "case {case} entry {i}"
                );
            }
            let mut state = before_state;
            state[1..4].copy_from_slice(&admission.random);
            state[4..6].copy_from_slice(&admission.selected.map(|v| v as u32));
            state[6..8].copy_from_slice(&admission.counts);
            state[8] = admission.entries as u32;
            for r in 0..2 {
                for i in 0..7 {
                    state[9 + r * 7 + i] = admission.demand[r][i] as u32;
                }
            }
            assert_eq!(state, after_state, "case {case} admission");
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original road vehicle creation observations"]
    fn matches_native_vehicle_creation() {
        use super::*;
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_VEHICLE_CREATE").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTVCREA1");
        assert_eq!(word(&data, 8), 1);
        let mut at = 16;
        const BYTES: usize = 200 * 208 + 64 * 52;
        for case in 0..word(&data, 12) {
            let input: [u32; 10] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 40;
            at += 16;
            let before = &data[at..at + BYTES];
            at += BYTES + 16;
            let after = &data[at..at + BYTES];
            at += BYTES;
            let mut expected_calls = vec![];
            let mut mutation = false;
            for _ in 0..input[7] {
                assert_eq!(word(&data, at), 0x4dfa40);
                assert_eq!(word(&data, at + 16), 4);
                mutation |= word(&data, at + 20) != 0;
                expected_calls.push([word(&data, at + 8), word(&data, at + 4)]);
                at += 24;
            }
            // Compare both the original reference namespace and an unrelated
            // host-neutral namespace against the same observed link graph.
            for base in [0x63f9e8, 0x2000_0000] {
                let map_reference = |reference: u32| {
                    if (0x63f9e8..0x63f9e8 + 200 * 208).contains(&reference)
                        && (reference - 0x63f9e8).is_multiple_of(208)
                    {
                        base + reference - 0x63f9e8
                    } else {
                        reference
                    }
                };
                let rebase = |bytes: &[u8]| {
                    let mut records: [[u8; 208]; 200] =
                        std::array::from_fn(|i| bytes[i * 208..(i + 1) * 208].try_into().unwrap());
                    let mut entries: [[u8; 52]; 64] = std::array::from_fn(|i| {
                        bytes[41600 + i * 52..41600 + (i + 1) * 52]
                            .try_into()
                            .unwrap()
                    });
                    for row in &mut records {
                        for at in [0xac, 0xb0] {
                            let reference = map_reference(word(row, at));
                            put(row, at, reference);
                        }
                    }
                    for row in &mut entries {
                        for at in [0x2c, 0x30] {
                            let reference = map_reference(word(row, at));
                            put(row, at, reference);
                        }
                    }
                    (records, entries)
                };
                let (mut records, mut entries) = rebase(before);
                let mut calls = vec![];
                let result = RoadVehicles {
                    records: &mut records,
                    entries: &mut entries,
                    reference_base: base,
                }
                .spawn(
                    [input[3] as i32, input[4] as i32, input[2] as i32],
                    input[1],
                    |model, auxiliary, state| {
                        calls.push([model, auxiliary]);
                        if mutation {
                            put(&mut state.records[input[0] as usize], 8, 0xdeadc0de);
                        }
                        Ok(())
                    },
                );
                if input[9] != 0 {
                    assert!(result.unwrap_err().contains("exhausted"));
                    let (expected_records, expected_entries) = rebase(before);
                    assert_eq!(
                        records, expected_records,
                        "safe exhaustion leaves storage intact"
                    );
                    assert_eq!(entries, expected_entries);
                } else {
                    assert_eq!(
                        result.unwrap(),
                        map_reference(input[8]),
                        "case {case} return"
                    );
                    let (expected_records, expected_entries) = rebase(after);
                    for i in 0..200 {
                        assert_eq!(records[i], expected_records[i], "case {case} row {i}");
                    }
                    assert_eq!(entries, expected_entries, "case {case} entries");
                }
                assert_eq!(calls, expected_calls, "case {case} bindings");
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original road vehicle initialization observations"]
    fn matches_native_vehicle_initialization() {
        use super::*;
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_VEHICLE_INITIALIZE").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTVINIT1");
        assert_eq!(word(&data, 8), 1);
        let mut at = 16;
        for case in 0..word(&data, 12) {
            let seed = word(&data, at) as u8;
            let allocated = word(&data, at + 4);
            let base = word(&data, at + 8);
            let calls = word(&data, at + 12) as usize;
            at += 16;
            let mut template: [u8; 284] = data[at..at + 284].try_into().unwrap();
            at += 284;
            let definition: [u8; 116] = data[at..at + 116].try_into().unwrap();
            at += 116;
            let expected_template = &data[at..at + 284];
            at += 284;
            let expected_records = &data[at..at + 200 * 208];
            at += 200 * 208;
            let expected_auxiliaries = &data[at..at + 200 * 284];
            at += 200 * 284;
            // Captured presentation globals belong to the remainder of44bac0;
            // this API deliberately implements only its storage portion.
            at += 74 * 2;
            let mut records = [[seed; 208]; 200];
            let mut bound = vec![];
            let auxiliaries = initialize_road_vehicles(
                &mut records,
                &mut template,
                base,
                |model, auxiliary, _| {
                    bound.push(model);
                    crate::resident::bind_auxiliary_model(auxiliary, model, &definition);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                template.as_slice(),
                expected_template,
                "case {case} retained template"
            );
            assert_eq!(auxiliaries.used(), 200);
            for i in 0..200 {
                assert_eq!(
                    records[i].as_slice(),
                    &expected_records[i * 208..(i + 1) * 208],
                    "case {case} vehicle {i}"
                );
                assert_eq!(
                    auxiliaries.slots()[i].as_slice(),
                    &expected_auxiliaries[i * 284..(i + 1) * 284],
                    "case {case} auxiliary {i}"
                );
            }
            let mut expected_calls = vec![[0x44ba30, 0, 0]];
            if allocated != 0 {
                expected_calls.push([0x4dfb20, 0, 0]);
            }
            expected_calls.push([0x4dfad0, 200, 0]);
            for (i, model) in bound.into_iter().enumerate() {
                expected_calls.extend([
                    [0x4dfb50, i as u32, 0],
                    [0x4dfba0, i as u32, 0],
                    [0x4dfa40, base + i as u32 * 284, model],
                ]);
            }
            assert_eq!(calls, expected_calls.len());
            for expected in expected_calls {
                assert_eq!(
                    std::array::from_fn::<_, 3, _>(|i| word(&data, at + i * 4)),
                    expected
                );
                at += 12;
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original vehicle admission observations"]
    fn matches_native_vehicle_admission() {
        use super::*;
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_VEHICLE_ADMISSION").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTVADMIT");
        assert_eq!(word(&data, 8), 1);
        let mut at = 16;
        for case in 0..word(&data, 12) {
            let input: [u32; 30] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 120;
            let before: [u32; 23] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 92;
            let after: [u32; 23] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 92;
            let mut admission = Admission {
                random: before[..3].try_into().unwrap(),
                selected: [before[3] as i32, before[4] as i32],
                counts: [before[5], before[6]],
                entries: before[7] as i32,
                demand: [[0; 7]; 2],
            };
            admission.refresh_demands(
                &[
                    std::array::from_fn(|i| before[16 + i] as i32),
                    std::array::from_fn(|i| if i == 0 { before[22] as i32 } else { 0 }),
                    std::array::from_fn(|i| before[9 + i] as i32),
                ],
                before[15],
            );
            let mut expected = vec![];
            for _ in 0..input[21] {
                assert_eq!(word(&data, at), 0x44b880);
                assert_eq!(word(&data, at + 20), 0);
                expected.push(std::array::from_fn::<_, 3, _>(|i| {
                    word(&data, at + 4 + i * 4) as i32
                }));
                at += 28;
            }
            let mut actual = vec![];
            admission
                .advance(input[0] as i32, |args, state| {
                    actual.push(args);
                    if actual.len() == 1 && input[23] != 0 {
                        state.random.copy_from_slice(&input[24..27]);
                        state.counts.copy_from_slice(&input[27..29]);
                        state.selected[1] = input[29] as i32;
                    }
                    Ok(())
                })
                .unwrap();
            let mut result = before;
            result[..3].copy_from_slice(&admission.random);
            result[3..5].copy_from_slice(&admission.selected.map(|v| v as u32));
            result[5..7].copy_from_slice(&admission.counts);
            result[7] = admission.entries as u32;
            for r in 0..2 {
                for i in 0..7 {
                    result[9 + r * 7 + i] = admission.demand[r][i] as u32;
                }
            }
            assert_eq!(actual, expected, "case {case} calls");
            assert_eq!(result, after, "case {case} state");
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original route producer observations and train locator assets"]
    fn matches_native_route_records() {
        use super::*;
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = crate::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let real_routes = RouteGeometry::load_routes(&image, &index, &archive).unwrap();
        let mut observed_real_routes = [false; 8];
        let data = read("OPENMT2001_NATIVE_TRANSPORT_ROUTES");
        assert_eq!(&data[..8], b"MTROUTE4");
        assert_eq!(word(&data, 8), 2);
        let mut at = 16;
        for case in 0..word(&data, 12) {
            let tier = word(&data, at);
            let route = word(&data, at + 4);
            let calls = word(&data, at + 12);
            let returned = word(&data, at + 16);
            let tick = word(&data, at + 20);
            let camera = std::array::from_fn(|i| f32::from_bits(word(&data, at + 24 + i * 4)));
            at += 40;
            let before = &data[at..at + 1816];
            at += 1816;
            let after = &data[at..at + 1816];
            at += 1816;
            let mut records: [[u8; 60]; 10] =
                std::array::from_fn(|i| before[i * 60..(i + 1) * 60].try_into().unwrap());
            let vectors = 600 + 8 * 128 + route as usize * 24;
            let geometry = RouteGeometry {
                start: std::array::from_fn(|i| f32::from_bits(word(before, vectors + i * 4))),
                end: std::array::from_fn(|i| f32::from_bits(word(before, vectors + 12 + i * 4))),
            };
            let real = real_routes[route as usize];
            if geometry.start.map(f32::to_bits) == real.start.map(f32::to_bits)
                && geometry.end.map(f32::to_bits) == real.end.map(f32::to_bits)
            {
                observed_real_routes[route as usize] = true;
            }
            let mut expected_sound = vec![];
            for _ in 0..calls {
                assert_eq!(word(&data, at), 0x4aefc0);
                assert_eq!(word(&data, at + 4), 16);
                assert_eq!(word(&data, at + 8), 0x8af288);
                assert_eq!(word(&data, at + 12), 4);
                expected_sound.push(std::array::from_fn::<_, 4, _>(|i| {
                    word(&data, at + 16 + i * 4)
                }));
                at += 36;
            }
            let mut actual_sound = vec![];
            let result = spawn_route(
                &mut records,
                tier,
                route,
                tick,
                |_| Ok(geometry),
                camera,
                |sound, _| {
                    actual_sound.push([
                        sound.id,
                        sound.distance.to_bits(),
                        sound.arguments[0],
                        sound.arguments[1],
                    ]);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(u32::from(result), returned, "case {case} result");
            for (i, record) in records.iter().enumerate() {
                assert_eq!(
                    record.as_slice(),
                    &after[i * 60..(i + 1) * 60],
                    "case {case} full record {i}"
                );
            }
            assert_eq!(
                &before[600..],
                &after[600..],
                "case {case} source geometry unchanged"
            );
            assert_eq!(actual_sound, expected_sound, "case {case} sound");
        }
        assert_eq!(at, data.len());
        assert_eq!(
            observed_real_routes, [true; 8],
            "fixture must include all original train locators"
        );
    }

    #[test]
    #[ignore = "requires private original hourly scene observations"]
    fn matches_native_hourly_effects() {
        check_hourly_effects("OPENMT2001_NATIVE_HOURLY_EFFECTS", false);
    }

    #[test]
    #[ignore = "requires private original hourly scene consumer observations"]
    fn matches_native_hourly_secondary() {
        check_hourly_effects("OPENMT2001_NATIVE_HOURLY_SECONDARY", true);
    }

    #[test]
    #[ignore = "requires original combined hourly transport observations"]
    fn matches_native_hourly_transport() {
        check_hourly_effects("OPENMT2001_NATIVE_HOURLY_TRANSPORT", true);
    }

    fn check_hourly_effects(environment: &str, secondary_mode: bool) {
        use super::*;
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let data = std::fs::read(std::env::var(environment).unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTH44F2A");
        let version = word(&data, 8);
        assert!((1..=2).contains(&version));
        let global_count = if version == 1 { 10 } else { 13 };
        let real_routes = if environment == "OPENMT2001_NATIVE_HOURLY_TRANSPORT" {
            let archive = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
            let directory =
                std::fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
            let index =
                crate::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            Some(RouteGeometry::load_routes(&image, &index, &archive).unwrap())
        } else {
            None
        };
        let sizes: [usize; 4] = std::array::from_fn(|i| word(&data, 16 + i * 4) as usize);
        assert_eq!(sizes[1], 600);
        assert_eq!(sizes[3], 180);
        let models = std::array::from_fn(|i| {
            u32::from_le_bytes(
                image
                    .at(0x539d50 + i as u32 * 4, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )
        });
        let mut at = 32;
        for case in 0..word(&data, 12) {
            let calls = word(&data, at + 12);
            let writes = word(&data, at + 16);
            assert_eq!(word(&data, at + 20) != 0, secondary_mode);
            assert_eq!(word(&data, at + 20) & 2 != 0, real_routes.is_some());
            at += 24;
            let before: Vec<u32> = (0..global_count).map(|i| word(&data, at + i * 4)).collect();
            at += global_count * 4;
            let mut arrays: [Vec<u8>; 4] = std::array::from_fn(|i| {
                let bytes = data[at..at + sizes[i]].to_vec();
                at += sizes[i];
                bytes
            });
            let after: Vec<u32> = (0..global_count).map(|i| word(&data, at + i * 4)).collect();
            at += global_count * 4;
            let expected_arrays: [Vec<u8>; 4] = std::array::from_fn(|i| {
                let bytes = data[at..at + sizes[i]].to_vec();
                at += sizes[i];
                bytes
            });
            let mut state = HourlyEffects {
                tier: before[7],
                random: before[4..7].try_into().unwrap(),
                model_choice: before[8],
                variant: before[9],
                auxiliaries: std::array::from_fn(|i| {
                    arrays[1][i * 300..(i + 1) * 300].try_into().unwrap()
                }),
                effects: std::array::from_fn(|i| {
                    arrays[3][i * 20..(i + 1) * 20].try_into().unwrap()
                }),
                models,
            };
            let mut inputs = HourlyInputs {
                hour: before[0],
                era: before[1],
                population: before[2] as i32,
                tick: before[3],
            };
            let mut expected_calls = vec![];
            let mut expected_sound = vec![];
            for _ in 0..calls {
                let address = word(&data, at);
                let ecx = word(&data, at + 8);
                let first = word(&data, at + 16);
                let second = word(&data, at + 20);
                if secondary_mode {
                    match address {
                        0x44e480 => expected_calls.push(HourlyCall::Route(first, second)),
                        0x4dfa40 => expected_calls.push(HourlyCall::BindAuxiliary(0, first)),
                        0x44df70 | 0x4df9c0 => {}
                        0x4aefc0 => expected_sound.push(std::array::from_fn::<_, 4, _>(|i| {
                            word(&data, at + 16 + i * 4)
                        })),
                        _ => panic!("unexpected scene consumer call"),
                    }
                } else {
                    expected_calls.push(match address {
                        0x44e480 => HourlyCall::Route(first, second),
                        0x44df70 => HourlyCall::Secondary(first, second),
                        0x4df9c0 => {
                            HourlyCall::InitializeAuxiliary(((ecx - 0x64fd88) / 300) as usize)
                        }
                        0x4dfa40 => {
                            HourlyCall::BindAuxiliary(((ecx - 0x64fd88) / 300) as usize, first)
                        }
                        _ => panic!("unexpected original hourly scene callee"),
                    });
                }
                at += if version == 1 { 24 } else { 32 };
            }
            // The native write trace supplements the full before/after oracle.
            at += writes as usize * 20;
            let actual_calls = std::cell::RefCell::new(vec![]);
            let mut actual_sound = vec![];
            if secondary_mode {
                let mut routes: [[u8; 60]; 10] =
                    std::array::from_fn(|i| arrays[0][i * 60..(i + 1) * 60].try_into().unwrap());
                let mut records: [[u8; 304]; 8] =
                    std::array::from_fn(|i| arrays[2][i * 304..(i + 1) * 304].try_into().unwrap());
                state
                    .advance_with_secondary(
                        &mut records,
                        &mut inputs,
                        |reference, auxiliary, _| {
                            actual_calls
                                .borrow_mut()
                                .push(HourlyCall::BindAuxiliary(0, reference));
                            let definition = std::array::from_fn(|i| {
                                reference.wrapping_add(i as u32 * 13) as u8
                            });
                            crate::resident::bind_auxiliary_model(
                                auxiliary,
                                reference,
                                &definition,
                            );
                            Ok(())
                        },
                        |tier, route, _, inputs| {
                            actual_calls
                                .borrow_mut()
                                .push(HourlyCall::Route(tier, route));
                            if let Some(geometry) = &real_routes {
                                let camera =
                                    std::array::from_fn(|i| f32::from_bits(before[10 + i]));
                                spawn_route(
                                    &mut routes,
                                    tier,
                                    route,
                                    inputs.tick,
                                    |index| Ok(geometry[index as usize]),
                                    camera,
                                    |sound, _| {
                                        actual_sound.push([
                                            sound.id,
                                            sound.distance.to_bits(),
                                            sound.arguments[0],
                                            sound.arguments[1],
                                        ]);
                                        Ok(())
                                    },
                                )?;
                            }
                            Ok(())
                        },
                    )
                    .unwrap();
                arrays[2] = records.into_iter().flatten().collect();
                arrays[0] = routes.into_iter().flatten().collect();
            } else {
                state
                    .advance(&mut inputs, |call, _, _| {
                        actual_calls.borrow_mut().push(call);
                        Ok(())
                    })
                    .unwrap();
            }
            assert_eq!(
                actual_calls.into_inner(),
                expected_calls,
                "case {case} ordered calls"
            );
            assert_eq!(
                actual_sound, expected_sound,
                "case {case} sound submissions"
            );
            let mut actual = vec![
                inputs.hour,
                inputs.era,
                inputs.population as u32,
                inputs.tick,
                state.random[0],
                state.random[1],
                state.random[2],
                state.tier,
                state.model_choice,
                state.variant,
            ];
            if version == 2 {
                actual.extend_from_slice(&before[10..13]);
            }
            assert_eq!(actual, after, "case {case} globals");
            arrays[1] = state.auxiliaries.into_iter().flatten().collect();
            arrays[3] = state.effects.into_iter().flatten().collect();
            assert_eq!(
                arrays, expected_arrays,
                "case {case} complete scene records"
            );
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires private original signal-phase observations"]
    fn matches_native_signal_phase() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_TRAFFIC_SIGNALS").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTSIGNAL");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let mut signals = super::Signals::from_v14(&image, word(at)).unwrap();
            at += 4;
            let mut lanes: Vec<[u8; 32]> = data[at..at + 96]
                .chunks_exact(32)
                .map(|r| r.try_into().unwrap())
                .collect();
            at += 96;
            signals.advance(&mut lanes);
            assert_eq!(signals.counter, word(at), "case {case} counter");
            assert_eq!(signals.phase, word(at + 4), "case {case} phase");
            at += 8;
            for row in lanes {
                assert_eq!(row.as_slice(), &data[at..at + 32], "case {case} full lane");
                at += 32;
            }
        }
        assert_eq!(at, data.len());
    }
}
