//! Original road segments, queue release and movement. Native references are
//! resolved inside caller-owned record slices, never dereferenced as pointers.
use super::{RoadVehicles, put, word};
use crate::x87::Sum;

pub mod audio;
pub mod construction;
pub mod models;
pub mod projection;
pub mod routes;
pub mod runtime;

pub struct Graph<'a> {
    pub segments: &'a mut [[u8; 108]],
    pub segment_base: u32,
    pub nodes: &'a mut [[u8; 44]],
    pub node_base: u32,
}

fn record_index<const N: usize>(
    rows: &[[u8; N]],
    base: u32,
    reference: u32,
) -> Result<usize, String> {
    let offset = reference
        .checked_sub(base)
        .ok_or("road reference precedes pool")?;
    let index = offset as usize / N;
    if reference == 0 || !(offset as usize).is_multiple_of(N) || index >= rows.len() {
        return Err("road reference outside record pool".into());
    }
    Ok(index)
}

impl Graph<'_> {
    /// Original44b6e0, including its4e2900 cubic path. Preserve the native
    /// f32 parameter/weight spills rather than evaluating one host polynomial.
    pub fn position(&self, segment: u32, distance: f32) -> Result<[f32; 3], String> {
        let row = self.segment(segment)?;
        // FLD/FMUL quiet a NaN parameter and all three interpolated stores
        // retain it. Keep its payload explicitly across native/browser hosts.
        if distance.is_nan() {
            return Ok([f32::from_bits(distance.to_bits() | 0x0040_0000); 3]);
        }
        let mut parameter = extended(distance)?;
        parameter.mul_f32(float(row, 0x5c))?;
        let parameter = parameter.to_f32();
        if row[0x22] == 1 {
            let end = self.node(word(row, 0x60))?;
            let start = self.node(word(row, 0x64))?;
            let mut result = [0.; 3];
            for (axis, value) in result.iter_mut().enumerate() {
                let mut position =
                    difference(float(end, 0x20 + axis * 4), float(start, 0x20 + axis * 4))?;
                position.mul_f32(parameter)?;
                position.add_f32(float(start, 0x20 + axis * 4))?;
                *value = position.to_f32();
            }
            return Ok(result);
        }
        cubic(row, parameter)
    }

    pub fn segment(&self, reference: u32) -> Result<&[u8; 108], String> {
        Ok(&self.segments[record_index(self.segments, self.segment_base, reference)?])
    }
    pub fn segment_mut(&mut self, reference: u32) -> Result<&mut [u8; 108], String> {
        let index = record_index(self.segments, self.segment_base, reference)?;
        Ok(&mut self.segments[index])
    }
    pub fn node(&self, reference: u32) -> Result<&[u8; 44], String> {
        Ok(&self.nodes[record_index(self.nodes, self.node_base, reference)?])
    }
    pub fn node_mut(&mut self, reference: u32) -> Result<&mut [u8; 44], String> {
        let index = record_index(self.nodes, self.node_base, reference)?;
        Ok(&mut self.nodes[index])
    }
}

fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}
fn extended(value: f32) -> Result<Sum, String> {
    Sum::from_f32(value).map_err(str::to_owned)
}
fn difference(a: f32, b: f32) -> Result<Sum, String> {
    let mut result = extended(a)?;
    result.add(extended(b)?.negated());
    Ok(result)
}

fn cubic(row: &[u8; 108], parameter: f32) -> Result<[f32; 3], String> {
    let u = difference(1., parameter)?;
    let mut u2 = u;
    u2.mul(u);
    let mut t2 = extended(parameter)?;
    t2.mul_f32(parameter)?;
    let t2 = t2.to_f32();
    let mut u3 = u2;
    u3.mul(u);
    let u3 = u3.to_f32();
    let mut first = u2;
    first.mul_f32(parameter)?;
    first.mul_f32(3.)?;
    let first = first.to_f32();
    let mut second = u;
    second.mul_f32(t2)?;
    second.mul_f32(3.)?;
    let mut third = extended(t2)?;
    third.mul_f32(parameter)?;
    let mut result = [0.; 3];
    for (axis, value) in result.iter_mut().enumerate() {
        let mut total = third;
        total.mul_f32(float(row, 0x4c + axis * 4))?;
        let mut term = second;
        term.mul_f32(float(row, 0x40 + axis * 4))?;
        total.add(term);
        let mut term = extended(first)?;
        term.mul_f32(float(row, 0x34 + axis * 4))?;
        total.add(term);
        let mut term = extended(u3)?;
        term.mul_f32(float(row, 0x28 + axis * 4))?;
        total.add(term);
        *value = total.to_f32();
    }
    Ok(result)
}

fn stored_difference(a: f32, b: f32) -> Result<f32, String> {
    if a.is_nan() {
        return Ok(f32::from_bits(a.to_bits() | 0x0040_0000));
    }
    if b.is_nan() {
        return Ok(f32::from_bits(b.to_bits() | 0x0040_0000));
    }
    if a.is_infinite() || b.is_infinite() {
        return Ok(if a == b {
            f32::from_bits(0xffc0_0000)
        } else {
            a - b
        });
    }
    Ok(difference(a, b)?.to_f32())
}

/// Original model fields+64/+b8 read by44b760. Model registration and bounds
/// lookup stay outside shared simulation; these are the supplied model values.
#[derive(Clone, Copy, Debug)]
pub struct ModelExtent {
    pub low: f32,
    pub high: f32,
}

pub struct Crossings<'a> {
    pub lanes: &'a mut [[u8; 32]],
    pub lane_base: u32,
    pub roads: &'a [[u8; 44]],
    pub road_base: u32,
}

pub struct ControllerSettings {
    masks: [[u32; 4]; 12],
    reservations: [[u32; 4]; 12],
    unbounded: f32,
    end_offset: f32,
    lookahead: f32,
    entry_space: f32,
    spacing: f32,
    node_fraction: f32,
    distance_scale: f32,
    maximum_speed: f32,
    response: f32,
    acceleration: f32,
    minimum_speed: f32,
}

impl ControllerSettings {
    pub fn from_v14(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let table = |address| -> Result<[[u32; 4]; 12], String> {
            let data = image.at(address, 12 * 16)?;
            Ok(std::array::from_fn(|r| {
                std::array::from_fn(|i| word(data, r * 16 + i * 4))
            }))
        };
        Ok(Self {
            masks: table(0x539bc8)?,
            reservations: table(0x539c88)?,
            unbounded: image.f32_at(0x525154)?,
            end_offset: image.f32_at(0x525b3c)?,
            lookahead: image.f32_at(0x525064)?,
            entry_space: image.f32_at(0x525b6c)?,
            spacing: image.f32_at(0x52502c)?,
            node_fraction: image.f32_at(0x525044)?,
            distance_scale: image.f32_at(0x525b68)?,
            maximum_speed: image.f32_at(0x525b64)?,
            response: image.f32_at(0x525150)?,
            acceleration: image.f32_at(0x525b60)?,
            minimum_speed: image.f32_at(0x525b5c)?,
        })
    }
}

impl Crossings<'_> {
    fn lane(&self, reference: u32) -> Result<&[u8; 32], String> {
        Ok(&self.lanes[record_index(self.lanes, self.lane_base, reference)?])
    }
    fn lane_mut(&mut self, reference: u32) -> Result<&mut [u8; 32], String> {
        let index = record_index(self.lanes, self.lane_base, reference)?;
        Ok(&mut self.lanes[index])
    }
    fn road(&self, reference: u32) -> Result<&[u8; 44], String> {
        Ok(&self.roads[record_index(self.roads, self.road_base, reference)?])
    }
    fn clear(
        &self,
        reference: u32,
        direction: u32,
        turn: u32,
        settings: &ControllerSettings,
    ) -> Result<bool, String> {
        if reference == 0 {
            return Ok(true);
        }
        let row = self.lane(reference)?;
        if word(row, 0x18) != 0 {
            return Ok(false);
        }
        let mask = settings
            .masks
            .get(direction.wrapping_mul(3).wrapping_add(turn) as usize)
            .ok_or("crossing mask index outside table")?;
        Ok((0..4).all(|i| word(row, 8 + i * 4) & mask[i] == 0))
    }
    fn reserve(
        &mut self,
        reference: u32,
        direction: u32,
        turn: u32,
        add: bool,
        settings: &ControllerSettings,
    ) -> Result<(), String> {
        if reference == 0 {
            return Ok(());
        }
        let values = settings
            .reservations
            .get(direction.wrapping_mul(3).wrapping_add(turn) as usize)
            .ok_or("crossing reservation index outside table")?;
        let row = self.lane_mut(reference)?;
        for (i, &value) in values.iter().enumerate() {
            let old = word(row, 8 + i * 4);
            put(
                row,
                8 + i * 4,
                if add {
                    old.wrapping_add(value)
                } else {
                    old.wrapping_sub(value)
                },
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteRequest {
    pub source_road: u32,
    pub direction: u32,
    pub destination_road: u32,
}

pub struct RouteChoice {
    pub command: u32,
    pub found: bool,
}

fn less(a: Sum, b: f32) -> Result<bool, String> {
    Ok(a.compare(extended(b)?).is_lt())
}
fn greater(a: Sum, b: f32) -> Result<bool, String> {
    Ok(a.compare(extended(b)?).is_gt())
}

impl RoadVehicles<'_> {
    /// Original44d200's ordered 200-record state dispatch, also used by44c210
    /// after admission and signal updates. Callbacks execute synchronously, so
    /// changes to later records are visible during this same pass. The final
    /// audio-ranking call44d0a0 is outside this simulation operation.
    pub fn advance_pool(
        &mut self,
        graph: &mut Graph<'_>,
        counts: &mut [u32; 2],
        mut moving: impl FnMut(&mut Self, usize, &mut Graph<'_>, &mut [u32; 2]) -> Result<(), String>,
        mut waiting: impl FnMut(&mut Self, usize, &mut Graph<'_>) -> Result<(), String>,
    ) -> Result<(), String> {
        for index in 0..self.records.len() {
            match word(&self.records[index], 0x1c) {
                9 => moving(self, index, graph, counts)?,
                10 => self.retire_head(index, graph, counts)?,
                11 => waiting(self, index, graph)?,
                13 => put(&mut self.records[index], 0x28, 0),
                _ => {}
            }
        }
        Ok(())
    }

    /// Original44c6b0 and the equivalent inline state10 dispatch. This retires
    /// the segment's head; it is not a general arbitrary-list removal routine.
    pub fn retire_head(
        &mut self,
        index: usize,
        graph: &mut Graph<'_>,
        counts: &mut [u32; 2],
    ) -> Result<(), String> {
        let row = self
            .records
            .get(index)
            .ok_or("vehicle index outside pool")?;
        let kind = word(row, 0x18) as i32;
        let previous = word(row, 0xac);
        let next = word(row, 0xb0);
        let segment = word(row, 0xb4);
        put(&mut self.records[index], 0x1c, 0);
        if kind == 5 {
            counts[0] = counts[0].wrapping_sub(1);
        }
        if (6..=23).contains(&kind) {
            counts[1] = counts[1].wrapping_sub(1);
        }
        if next != 0 {
            let next_index = self.index(next)?;
            put(&mut self.records[next_index], 0xac, previous);
            put(graph.segment_mut(segment)?, 0x18, next);
        } else {
            put(graph.segment_mut(segment)?, 0x1c, 0);
            put(graph.segment_mut(segment)?, 0x18, 0);
        }
        self.records[index][0x14] = 0;
        Ok(())
    }

    /// Original44c7f0 movement/controller. Route choice448590 is synchronous:
    /// its original mode2/search1 arguments are fixed; graph search/cache
    /// ownership remains with the caller. Finite arithmetic uses PC64.
    pub fn advance_vehicle(
        &mut self,
        index: usize,
        graph: &mut Graph<'_>,
        crossings: &mut Crossings<'_>,
        settings: &ControllerSettings,
        counts: &mut [u32; 2],
        mut choose: impl FnMut(RouteRequest) -> Result<RouteChoice, String>,
    ) -> Result<(), String> {
        if index >= self.records.len() {
            return Err("vehicle index outside pool".into());
        }
        let mut stop = extended(settings.unbounded)?;
        let mut segment = word(&self.records[index], 0xb4);
        let mut remaining = difference(
            float(graph.segment(segment)?, 0x58),
            float(&self.records[index], 0x2c),
        )?;
        remaining.add(extended(settings.end_offset)?.negated());
        let gap = remaining.to_f32();
        if less(remaining, settings.lookahead)? {
            let mut command = word(&self.records[index], 0);
            if command == 0 {
                let cursor = word(&self.records[index], 0x78);
                let at = 0x38_usize
                    .checked_add(cursor as usize * 4)
                    .ok_or("vehicle command index overflow")?;
                let bytes = self.records[index]
                    .get(at..at + 4)
                    .ok_or("vehicle command outside record")?;
                command = u32::from_le_bytes(bytes.try_into().unwrap());
                put(&mut self.records[index], 0x78, cursor.wrapping_add(1));
                put(&mut self.records[index], 0, command);
                if command == 0x800 {
                    let row = graph.segment(segment)?;
                    let direction = u32::from(row[0x20]);
                    let source = crossings.road(word(row, 0x24))?[0] as usize;
                    let neighbor = match direction {
                        1 => 0x25,
                        2 => 0x27,
                        4 => 0x24,
                        8 => 0x26,
                        _ => return Err("invalid road direction for route request".into()),
                    };
                    let source_road = u32::from(
                        crossings
                            .roads
                            .get(source)
                            .ok_or("route source outside roads")?[neighbor],
                    );
                    let destination = word(&self.records[index], 0x7c) as usize;
                    let destination = crossings.road(word(
                        self.entries
                            .get(destination)
                            .ok_or("route destination outside entries")?,
                        0x18,
                    ))?[0];
                    let choice = choose(RouteRequest {
                        source_road,
                        direction,
                        destination_road: u32::from(destination),
                    })?;
                    command = choice.command;
                    for (at, value) in [
                        (0x38, command),
                        (0x3c, 0x40),
                        (0x40, 0x800),
                        (0, command),
                        (0x78, 1),
                    ] {
                        put(&mut self.records[index], at, value);
                    }
                    if choice.found {
                        for (at, value) in [(0x3c, 0x20), (0x40, 0x40), (0x44, 0x40), (0x48, 0x40)]
                        {
                            put(&mut self.records[index], at, value);
                        }
                    }
                }
                if let Some(at) = command_link(command) {
                    if command != 0x40 {
                        put(&mut self.records[index], 0xc8, command);
                    }
                    let next = word(graph.node(word(graph.segment(segment)?, 0x60))?, at);
                    let next = word(graph.node(word(graph.segment(next)?, 0x60))?, 0x10);
                    put(&mut self.records[index], 0xbc, next);
                }
                if command != 0 && graph.segment(segment)?[0] & 1 != 0 {
                    if let Some(direction) = match graph.segment(segment)?[0x20] {
                        2 => Some(0),
                        1 => Some(1),
                        8 => Some(2),
                        4 => Some(3),
                        _ => None,
                    } {
                        put(&mut self.records[index], 0x8c, direction);
                    }
                    if let Some(turn) = match command {
                        0x40 => Some(0),
                        0x10 => Some(1),
                        0x20 => Some(2),
                        _ => None,
                    } {
                        put(&mut self.records[index], 0x90, turn);
                    }
                }
            }
            self.records[index][0x20] = 0;
            if command != 0 && graph.segment(segment)?[0] & 4 != 0 {
                let lane = word(graph.segment(segment)?, 0x68);
                let direction = word(&self.records[index], 0x8c);
                let turn = word(&self.records[index], 0x90);
                if crossings.lane(lane)?[0] & graph.segment(segment)?[0x20] != 0 {
                    if word(&self.records[index], 0x94) == 0 {
                        if !crossings.clear(lane, direction, turn, settings)? {
                            stop = extended(gap)?;
                            put(&mut self.records[index], 0x80, 0x539d74);
                        } else {
                            let previous = word(&self.records[index], 0xac);
                            if previous == 0
                                || word(&self.records[self.index(previous)?], 0x94) != 0
                            {
                                let mut need = extended(float(&self.records[index], 0x88))?;
                                need.mul_f32(settings.entry_space)?;
                                let next = word(&self.records[index], 0xbc);
                                if greater(need, float(graph.segment(next)?, 4))? {
                                    stop = extended(gap)?;
                                    put(&mut self.records[index], 0x80, 0x539d88);
                                    self.records[index][0x20] = 1;
                                } else {
                                    crossings.reserve(lane, direction, turn, true, settings)?;
                                    put(&mut self.records[index], 0x94, 1);
                                    put(&mut self.records[index], 0x80, 0x539d9c);
                                }
                            }
                        }
                    }
                    let stalled = float(&self.records[index], 0x28) == 0.
                        && word(&self.records[index], 0xac) == 0
                        && graph.segment(segment)?[0x21] == 1;
                    let count = if stalled {
                        word(&self.records[index], 0x24).wrapping_add(1)
                    } else {
                        0
                    };
                    put(&mut self.records[index], 0x24, count);
                    if count as i32 >= 500 {
                        if word(&self.records[index], 0x94) == 1 {
                            crossings.reserve(lane, direction, turn, false, settings)?;
                            put(&mut self.records[index], 0x94, 0);
                        }
                        adjust_node_occupancy(graph, segment, false)?;
                        let mut available = extended(float(&self.records[index], 0x88))?;
                        available.add_f32(float(graph.segment(segment)?, 4))?;
                        put(graph.segment_mut(segment)?, 4, available.to_f32().to_bits());
                        self.retire_head(index, graph, counts)?;
                    }
                } else {
                    put(&mut self.records[index], 0x80, 0x539d60);
                    stop = extended(gap)?;
                    if word(&self.records[index], 0x94) == 1 {
                        crossings.reserve(lane, direction, turn, false, settings)?;
                        put(&mut self.records[index], 0x94, 0);
                    }
                }
            }
        }
        let previous = word(&self.records[index], 0xac);
        let following = if previous != 0 {
            let mut distance = difference(
                float(&self.records[self.index(previous)?], 0x30),
                float(&self.records[index], 0x2c),
            )?;
            distance.add(extended(settings.spacing)?.negated());
            distance.to_f32()
        } else {
            settings.unbounded
        };
        let row = graph.segment(segment)?;
        if word(graph.node(word(row, 0x60))?, 8) != 0 && row[0] & 8 != 0 {
            let mut distance = extended(float(row, 0x58))?;
            distance.mul_f32(settings.node_fraction)?;
            distance.add(extended(float(&self.records[index], 0x2c))?.negated());
            let distance = distance.to_f32();
            if greater(stop, distance)? {
                stop = extended(distance)?;
            }
        }
        if extended(following)?.compare(stop).is_lt() {
            stop = extended(following)?;
        }
        let mut speed = stop;
        speed.mul_f32(settings.distance_scale)?;
        if greater(speed, settings.maximum_speed)? {
            speed = extended(settings.maximum_speed)?;
        }
        speed.add(extended(float(&self.records[index], 0x28))?.negated());
        speed.mul_f32(settings.response)?;
        if greater(speed, settings.acceleration)? {
            speed = extended(settings.acceleration)?;
        }
        speed.add_f32(float(&self.records[index], 0x28))?;
        let mut speed = speed.to_f32();
        if less(stop, settings.spacing)? {
            speed = 0.;
        }
        if speed > settings.maximum_speed {
            speed = f32::from_bits(0x3f4ccccd);
        }
        if speed < settings.minimum_speed {
            speed = 0.;
        }
        put(&mut self.records[index], 0x28, speed.to_bits());
        let mut position = extended(speed)?;
        position.add_f32(float(&self.records[index], 0x2c))?;
        put(&mut self.records[index], 0x2c, position.to_f32().to_bits());
        let mut rear = extended(float(&self.records[index], 0x30))?;
        rear.add_f32(speed)?;
        put(&mut self.records[index], 0x30, rear.to_f32().to_bits());
        if !less(position, float(graph.segment(segment)?, 0x58))? {
            let mut available = extended(float(graph.segment(segment)?, 4))?;
            available.add_f32(float(&self.records[index], 0x88))?;
            put(graph.segment_mut(segment)?, 4, available.to_f32().to_bits());
            if graph.segment(segment)?[0x21] == 8 {
                put(&mut self.records[index], 0x1c, 10);
                return Ok(());
            }
            let command = word(&self.records[index], 0);
            if let Some(at) = command_link(command) {
                let node = word(graph.segment(segment)?, 0x60);
                put(graph.node_mut(node)?, 4, command);
                let position = difference(
                    float(&self.records[index], 0x2c),
                    float(graph.segment(segment)?, 0x58),
                )?
                .to_f32();
                put(&mut self.records[index], 0x2c, position.to_bits());
                segment = word(graph.node(node)?, at);
                put(&mut self.records[index], 0xb4, segment);
            }
            put(&mut self.records[index], 4, command);
            put(&mut self.records[index], 0, 0);
            let tail = word(graph.segment(segment)?, 0x1c);
            put(&mut self.records[index], 0xac, tail);
            let reference = self.reference(index)?;
            put(graph.segment_mut(segment)?, 0x1c, reference);
            if tail == 0 {
                put(graph.segment_mut(segment)?, 0x18, reference);
            } else {
                let previous = self.index(tail)?;
                put(&mut self.records[previous], 0xb0, reference);
            }
            adjust_node_occupancy(graph, segment, true)?;
            let available = difference(
                float(graph.segment(segment)?, 4),
                float(&self.records[index], 0x88),
            )?
            .to_f32();
            put(graph.segment_mut(segment)?, 4, available.to_bits());
        }
        let rear_segment = word(&self.records[index], 0xb8);
        if float(&self.records[index], 0x30) >= float(graph.segment(rear_segment)?, 0x58) {
            if graph.segment(rear_segment)?[0] & 2 != 0 {
                crossings.reserve(
                    word(graph.segment(rear_segment)?, 0x68),
                    word(&self.records[index], 0x8c),
                    word(&self.records[index], 0x90),
                    false,
                    settings,
                )?;
                put(&mut self.records[index], 0x94, 0);
            }
            adjust_node_occupancy(graph, rear_segment, false)?;
            if graph.segment(rear_segment)?[0x21] == 2 {
                put(&mut self.records[index], 0xc8, 0);
            }
            let next = word(&self.records[index], 0xb0);
            put(graph.segment_mut(rear_segment)?, 0x18, next);
            if next == 0 {
                put(graph.segment_mut(rear_segment)?, 0x1c, 0);
            } else {
                let next = self.index(next)?;
                put(&mut self.records[next], 0xac, 0);
            }
            put(&mut self.records[index], 0xb0, 0);
            let at = command_link(word(&self.records[index], 4)).unwrap_or(0x18);
            let node = word(graph.segment(rear_segment)?, 0x60);
            put(graph.node_mut(node)?, 4, 0);
            let position = difference(
                float(&self.records[index], 0x30),
                float(graph.segment(rear_segment)?, 0x58),
            )?
            .to_f32();
            put(&mut self.records[index], 0x30, position.to_bits());
            put(&mut self.records[index], 4, 0);
            put(&mut self.records[index], 0xb8, word(graph.node(node)?, at));
        }
        Ok(())
    }
}

fn command_link(command: u32) -> Option<usize> {
    match command {
        0x40 => Some(0x10),
        0x10 => Some(0x14),
        0x20 => Some(0x18),
        _ => None,
    }
}
fn adjust_node_occupancy(graph: &mut Graph<'_>, segment: u32, add: bool) -> Result<(), String> {
    if graph.segment(segment)?[0x21] == 0x20 {
        let node = word(graph.segment(segment)?, 0x60);
        let count = word(graph.node(node)?, 8);
        put(
            graph.node_mut(node)?,
            8,
            if add {
                count.wrapping_add(1)
            } else {
                count.wrapping_sub(1)
            },
        );
    }
    Ok(())
}

impl RoadVehicles<'_> {
    /// Original44b760. Initialize both tracked ends on a segment and append
    /// the vehicle to its queue. The original truncates the low eight bits of
    /// the stored length; this is not ordinary rounding to a decimal precision.
    pub fn enter_segment(
        &mut self,
        index: usize,
        segment: u32,
        graph: &mut Graph<'_>,
        auxiliary: &mut [u8; 284],
        extent: ModelExtent,
    ) -> Result<(), String> {
        let reference = self.reference(index)?;
        if index >= self.records.len() {
            return Err("vehicle index outside pool".into());
        }
        let length = difference(extent.high, extent.low)?.to_f32().to_bits() & 0xffff_ff00;
        let tail = word(graph.segment(segment)?, 0x1c);
        put(&mut self.records[index], 0x1c, 9);
        put(&mut self.records[index], 0x84, (-extent.low).to_bits());
        put(&mut self.records[index], 0x88, length);
        put(&mut self.records[index], 0xb4, segment);
        put(&mut self.records[index], 0xb8, segment);
        put(&mut self.records[index], 0x30, 0);
        put(&mut self.records[index], 0x2c, length);
        for (position, coordinate) in [(4, 0x20), (12, 0x28)] {
            let row = graph.segment(segment)?;
            let a = graph.node(word(row, 0x64))?;
            let b = graph.node(word(row, 0x60))?;
            let mut value = extended(float(a, coordinate))?;
            value.add_f32(float(b, coordinate))?;
            value.mul_f32(0.5)?;
            put(auxiliary, position, value.to_f32().to_bits());
        }
        put(auxiliary, 8, 0);
        for (at, value) in [
            (0x28, 0.1_f32.to_bits()),
            (0, 0),
            (4, 0),
            (0xac, tail),
            (0xb0, 0),
        ] {
            put(&mut self.records[index], at, value);
        }
        if tail == 0 {
            put(graph.segment_mut(segment)?, 0x18, reference);
        } else {
            let previous = self.index(tail)?;
            put(&mut self.records[previous], 0xb0, reference);
        }
        put(graph.segment_mut(segment)?, 0x1c, reference);
        let available =
            stored_difference(float(graph.segment(segment)?, 4), f32::from_bits(length))?;
        put(graph.segment_mut(segment)?, 4, available.to_bits());
        Ok(())
    }

    /// Original44c720 including44b760. The equality flag accepts unordered
    /// comparisons too, so NaN availability has the same gate behavior as x87.
    pub fn release_from_entry(
        &mut self,
        index: usize,
        graph: &mut Graph<'_>,
        auxiliary: &mut [u8; 284],
        extent: ModelExtent,
    ) -> Result<bool, String> {
        let row = self
            .records
            .get(index)
            .ok_or("vehicle index outside pool")?;
        let entry = word(row, 0x34) as usize;
        let entry_row = self
            .entries
            .get(entry)
            .ok_or("vehicle entry outside table")?;
        if word(entry_row, 4) == 0 {
            return Ok(false);
        }
        let segment = word(entry_row, 0x24);
        for at in [0x24, 0x28] {
            let route = graph.segment(word(entry_row, at))?;
            let (available, length) = (float(route, 4), float(route, 0x58));
            if available != length && !available.is_nan() && !length.is_nan() {
                return Ok(false);
            }
        }
        let previous = word(row, 0xac);
        let next = word(row, 0xb0);
        if previous != 0 {
            let previous = self.index(previous)?;
            put(&mut self.records[previous], 0xb0, next);
        } else {
            put(&mut self.entries[entry], 0x2c, next);
        }
        if next != 0 {
            let next = self.index(next)?;
            put(&mut self.records[next], 0xac, previous);
        } else {
            put(&mut self.entries[entry], 0x30, previous);
        }
        self.enter_segment(index, segment, graph, auxiliary, extent)?;
        for (at, value) in [(0x78, 0), (0x1c, 9), (0x24, 0)] {
            put(&mut self.records[index], at, value);
        }
        self.records[index][0x14] = 1;
        let count = word(&self.entries[entry], 4).wrapping_sub(1);
        put(&mut self.entries[entry], 4, count);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original queue release and road geometry observations"]
    fn matches_native_release_and_geometry() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_VEHICLE_MOTION").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTVMOTN1");
        assert_eq!(word(&data, 8), 2);
        let mut at = 16;
        const SIZE: usize = 200 * 208 + 64 * 52 + 8 * 108 + 8 * 44 + 200 * 284;
        for case in 0..word(&data, 12) {
            let input: [u32; 18] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 72;
            assert_eq!(input[13], 0, "native fault in case {case}");
            let before = &data[at..at + SIZE];
            at += SIZE;
            let after = &data[at..at + SIZE];
            at += SIZE;
            let output: [u32; 3] = std::array::from_fn(|i| word(&data, at + i * 4));
            at += 12;
            let mut expected_events = vec![];
            for _ in 0..input[10] {
                expected_events.push(word(&data, at));
                at += 20;
            }
            for _ in 0..input[11] {
                assert_eq!(word(&data, at), 0x4e2900);
                at += 20;
            }
            let mut cursor = 0;
            fn rows<const N: usize, const M: usize>(data: &[u8], at: &mut usize) -> [[u8; N]; M] {
                let result = std::array::from_fn(|i| {
                    data[*at + i * N..*at + (i + 1) * N].try_into().unwrap()
                });
                *at += N * M;
                result
            }
            let mut records = rows::<208, 200>(before, &mut cursor);
            let mut entries = rows::<52, 64>(before, &mut cursor);
            let mut segments = rows::<108, 8>(before, &mut cursor);
            let mut nodes = rows::<44, 8>(before, &mut cursor);
            let mut auxiliaries = rows::<284, 200>(before, &mut cursor);
            let mut vehicles = RoadVehicles {
                records: &mut records,
                entries: &mut entries,
                reference_base: 0x63f9e8,
            };
            let mut graph = Graph {
                segments: &mut segments,
                segment_base: 0x10070000,
                nodes: &mut nodes,
                node_base: 0x10090000,
            };
            let index = input[2] as usize;
            let extent = ModelExtent {
                low: f32::from_bits(input[8]),
                high: f32::from_bits(input[9]),
            };
            match input[0] {
                1 => {
                    let released = vehicles
                        .release_from_entry(index, &mut graph, &mut auxiliaries[index], extent)
                        .unwrap_or_else(|error| panic!("case {case}: {error}"));
                    assert_eq!(u32::from(released), input[12] & 255, "case {case} returned");
                    assert_eq!(
                        expected_events,
                        if released {
                            vec![0x44c720, 0x44b760]
                        } else {
                            vec![0x44c720]
                        }
                    );
                }
                2 => {
                    vehicles
                        .enter_segment(
                            index,
                            0x10070000,
                            &mut graph,
                            &mut auxiliaries[index],
                            extent,
                        )
                        .unwrap();
                    assert_eq!(expected_events, [0x44b760]);
                }
                3 => {
                    let point = graph
                        .position(0x10070000 + input[5] * 108, f32::from_bits(input[4]))
                        .unwrap();
                    assert_eq!(point.map(f32::to_bits), output, "case {case} XYZ");
                    assert!(expected_events.is_empty());
                }
                4 => {
                    let mut events = vec![];
                    vehicles
                        .advance_pool(
                            &mut graph,
                            &mut [0; 2],
                            |_, _, _, _| panic!("newly released vehicle must not move twice"),
                            |vehicles, index, graph| {
                                events.push(0x44c720);
                                if vehicles.release_from_entry(
                                    index,
                                    graph,
                                    &mut auxiliaries[index],
                                    extent,
                                )? {
                                    events.push(0x44b760);
                                }
                                Ok(())
                            },
                        )
                        .unwrap();
                    assert_eq!(events, expected_events, "case {case} dispatch order");
                }
                _ => panic!("unknown mode"),
            }
            let mut actual = Vec::with_capacity(SIZE);
            for row in &records {
                actual.extend_from_slice(row);
            }
            for row in &entries {
                actual.extend_from_slice(row);
            }
            for row in &segments {
                actual.extend_from_slice(row);
            }
            for row in &nodes {
                actual.extend_from_slice(row);
            }
            for row in &auxiliaries {
                actual.extend_from_slice(row);
            }
            for (offset, (actual, expected)) in actual.iter().zip(after).enumerate() {
                assert_eq!(actual, expected, "case {case} byte {offset:#x}");
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original vehicle controller observations"]
    fn matches_native_controller() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let settings = ControllerSettings::from_v14(&image).unwrap();
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_VEHICLE_CONTROLLER").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTVCTRL1");
        assert_eq!(word(&data, 8), 3);
        let mut at = 16;
        const SIZE: usize = 46568;
        for case in 0..word(&data, 12) {
            let choice = word(&data, at);
            let found = word(&data, at + 4) != 0;
            let calls = word(&data, at + 8);
            let ticks = word(&data, at + 12);
            let batch = word(&data, at + 16) != 0;
            at += 20;
            let before = &data[at..at + SIZE];
            at += SIZE;
            let after = &data[at..at + SIZE];
            at += SIZE;
            let mut offset = 0;
            fn rows<const N: usize, const M: usize>(data: &[u8], at: &mut usize) -> [[u8; N]; M] {
                let result = std::array::from_fn(|i| {
                    data[*at + i * N..*at + (i + 1) * N].try_into().unwrap()
                });
                *at += N * M;
                result
            }
            let mut records = rows::<208, 200>(before, &mut offset);
            let mut segments = rows::<108, 8>(before, &mut offset);
            let mut nodes = rows::<44, 8>(before, &mut offset);
            let mut lanes = rows::<32, 2>(before, &mut offset);
            let mut entries = rows::<52, 64>(before, &mut offset);
            let roads = rows::<44, 8>(before, &mut offset);
            let mut counts = [word(before, offset), word(before, offset + 4)];
            let mut vehicles = RoadVehicles {
                records: &mut records,
                entries: &mut entries,
                reference_base: 0x63f9e8,
            };
            let mut graph = Graph {
                segments: &mut segments,
                segment_base: 0x661d70,
                nodes: &mut nodes,
                node_base: 0x63a1c0,
            };
            let mut crossings = Crossings {
                lanes: &mut lanes,
                lane_base: 0x661370,
                roads: &roads,
                road_base: 0x6375b8,
            };
            let mut actual_calls = vec![];
            for tick in 0..ticks {
                let mut choose = |request: RouteRequest| {
                    actual_calls.push([
                        tick,
                        request.source_road,
                        request.direction,
                        request.destination_road,
                        2,
                        1,
                    ]);
                    Ok(RouteChoice {
                        command: choice,
                        found,
                    })
                };
                let result = if batch {
                    vehicles.advance_pool(
                        &mut graph,
                        &mut counts,
                        |vehicles, index, graph, counts| {
                            vehicles.advance_vehicle(
                                index,
                                graph,
                                &mut crossings,
                                &settings,
                                counts,
                                &mut choose,
                            )
                        },
                        |vehicles, index, graph| {
                            let mut auxiliary = [0; 284];
                            vehicles
                                .release_from_entry(
                                    index,
                                    graph,
                                    &mut auxiliary,
                                    ModelExtent {
                                        low: 0.0,
                                        high: 4.0,
                                    },
                                )
                                .map(|_| ())
                        },
                    )
                } else {
                    vehicles.advance_vehicle(
                        0,
                        &mut graph,
                        &mut crossings,
                        &settings,
                        &mut counts,
                        &mut choose,
                    )
                };
                result.unwrap_or_else(|error| panic!("case {case}: {error}"));
            }
            let mut expected_calls = vec![];
            for _ in 0..calls {
                expected_calls.push(std::array::from_fn::<_, 6, _>(|i| word(&data, at + i * 4)));
                at += 24;
            }
            assert_eq!(actual_calls, expected_calls, "case {case} route requests");
            let mut actual = Vec::with_capacity(SIZE);
            for row in &records {
                actual.extend_from_slice(row);
            }
            for row in &segments {
                actual.extend_from_slice(row);
            }
            for row in &nodes {
                actual.extend_from_slice(row);
            }
            for row in &lanes {
                actual.extend_from_slice(row);
            }
            for row in &entries {
                actual.extend_from_slice(row);
            }
            for row in &roads {
                actual.extend_from_slice(row);
            }
            for value in counts {
                actual.extend_from_slice(&value.to_le_bytes());
            }
            for (offset, (actual, expected)) in actual.iter().zip(after).enumerate() {
                assert_eq!(actual, expected, "case {case} byte {offset:#x}");
            }
        }
        assert_eq!(at, data.len());
    }
}
