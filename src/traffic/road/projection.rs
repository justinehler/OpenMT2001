//! Original44d340 frame projection and visibility on the vehicle auxiliary pool.
use super::{Graph, difference, extended, float, put, record_index, word};

pub struct ProjectionSettings {
    midpoint: f32,
    degrees: f32,
    heading_offset: f32,
    slope_threshold: f32,
}
impl ProjectionSettings {
    pub fn from_v14(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        Ok(Self {
            midpoint: image.f32_at(0x52502c)?,
            degrees: image.f32_at(0x525b70)?,
            heading_offset: image.f32_at(0x5251dc)?,
            slope_threshold: image.f32_at(0x52508c)?,
        })
    }
}
/// Iterate auxiliary records in collection order. Each record's +38 selects a
/// vehicle; its+c4 separately selects the auxiliary receiving the transform.
/// This distinction matters when collection order differs from vehicle order.
pub fn project(
    records: &[[u8; 208]; 200],
    graph: &Graph<'_>,
    auxiliaries: &mut [[u8; 284]],
    auxiliary_base: u32,
    fraction: f32,
    settings: &ProjectionSettings,
) -> Result<(), String> {
    for index in 0..auxiliaries.len() {
        let vehicle = word(&auxiliaries[index], 0x38) as usize;
        let row = records
            .get(vehicle)
            .ok_or("projection vehicle index outside pool")?;
        if row[0x14] == 0 {
            auxiliaries[index][0xb8] &= !2;
            continue;
        }
        let mut advance = extended(fraction)?;
        advance.mul_f32(float(row, 0x28))?;
        let mut front = advance;
        front.add_f32(float(row, 0x2c))?;
        let mut rear = advance;
        rear.add_f32(float(row, 0x30))?;
        let front = graph.position(word(row, 0xb4), front.to_f32())?;
        let rear = graph.position(word(row, 0xb8), rear.to_f32())?;
        let target = record_index(auxiliaries, auxiliary_base, word(row, 0xc4))?;
        for axis in 0..3 {
            let mut value = difference(front[axis], rear[axis])?;
            value.mul_f32(settings.midpoint)?;
            value.add_f32(rear[axis])?;
            put(
                &mut auxiliaries[target],
                4 + axis * 4,
                value.to_f32().to_bits(),
            );
        }
        let dx = difference(front[0], rear[0])?;
        let dz = difference(front[2], rear[2])?;
        let dy = difference(front[1], rear[1])?;
        let mut heading = dz.atan2(dx)?;
        heading.mul_f32(settings.degrees)?;
        heading.add_f32(settings.heading_offset)?;
        put(&mut auxiliaries[target], 0x14, heading.to_f32().to_bits());
        let abs_dy = if dy.compare(extended(0.)?).is_lt() {
            dy.negated()
        } else {
            dy
        };
        let pitch = if abs_dy.compare(extended(settings.slope_threshold)?).is_gt() {
            let mut angle = dy.atan2(dx)?;
            angle.mul_f32(settings.degrees)?;
            angle.negated().to_f32()
        } else {
            0.
        };
        put(&mut auxiliaries[target], 0x10, pitch.to_bits());
        auxiliaries[index][0xb8] |= 2;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timers;

    const SEGMENT_BASE: u32 = 0x1001_0000;
    const NODE_BASE: u32 = 0x1001_1000;
    const AUXILIARY_BASE: u32 = 0x1002_0000;
    const SPEED: f32 = 0.8;

    fn settings() -> ProjectionSettings {
        // The v1.4 values at 52502c, 525b70, 5251dc and 52508c.
        ProjectionSettings {
            midpoint: 0.5,
            degrees: 57.295_78,
            heading_offset: 90.,
            slope_threshold: 0.1,
        }
    }
    type Lane = (
        Box<[[u8; 208]; 200]>,
        [[u8; 108]; 1],
        [[u8; 44]; 2],
        [[u8; 284]; 1],
    );
    /// One straight lane from x=0 to x=100 and one active car on it.
    fn straight_lane() -> Lane {
        let mut segments = [[0; 108]; 1];
        segments[0][0x22] = 1;
        put(&mut segments[0], 0x5c, (1f32 / 100.).to_bits());
        put(&mut segments[0], 0x60, NODE_BASE + 44);
        put(&mut segments[0], 0x64, NODE_BASE);
        let mut nodes = [[0; 44]; 2];
        put(&mut nodes[1], 0x20, 100f32.to_bits());
        let mut records: Box<[[u8; 208]; 200]> =
            vec![[0; 208]; 200].into_boxed_slice().try_into().unwrap();
        let car = &mut records[0];
        car[0x14] = 1;
        put(car, 0x28, SPEED.to_bits());
        put(car, 0x2c, 12f32.to_bits());
        put(car, 0x30, 8f32.to_bits());
        put(car, 0xb4, SEGMENT_BASE);
        put(car, 0xb8, SEGMENT_BASE);
        put(car, 0xc4, AUXILIARY_BASE);
        let auxiliaries = [[0; 284]; 1];
        (records, segments, nodes, auxiliaries)
    }
    fn drawn_x(auxiliary: &[u8; 284]) -> f32 {
        float(auxiliary, 4)
    }
    /// What a logic tick does to a car on a free lane: both ends move by speed.
    fn tick(records: &mut [[u8; 208]; 200]) {
        for at in [0x2c, 0x30] {
            let moved = float(&records[0], at) + SPEED;
            put(&mut records[0], at, moved.to_bits());
        }
    }
    fn logic_task() -> timers::Task {
        let mut task = [0; 20];
        task[4..8].copy_from_slice(&10u32.to_le_bytes());
        timers::configure(&mut task, 25., timers::LOGIC_CALLBACK);
        task
    }

    #[test]
    fn factor_moves_only_the_drawn_pose() {
        let (records, mut segments, mut nodes, mut auxiliaries) = straight_lane();
        let (records_before, segments_before, nodes_before) = (records.clone(), segments, nodes);
        {
            let graph = Graph {
                segments: &mut segments,
                segment_base: SEGMENT_BASE,
                nodes: &mut nodes,
                node_base: NODE_BASE,
            };
            let mut previous = None;
            for factor in [0., 0.25, 0.5, 0.75] {
                let before = auxiliaries[0];
                project(
                    &records,
                    &graph,
                    &mut auxiliaries,
                    AUXILIARY_BASE,
                    factor,
                    &settings(),
                )
                .unwrap();
                // Midpoint of front 12+f*0.8 and rear 8+f*0.8.
                let expected = 10. + factor * SPEED;
                assert!((drawn_x(&auxiliaries[0]) - expected).abs() < 1e-5);
                assert_eq!(float(&auxiliaries[0], 0x14), 90.);
                for at in (0..284).filter(|at| !(4..0x18).contains(at) && *at != 0xb8) {
                    assert_eq!(auxiliaries[0][at], before[at], "auxiliary byte {at:#x}");
                }
                assert_ne!(auxiliaries[0][0xb8] & 2, 0);
                if let Some(x) = previous {
                    assert!(drawn_x(&auxiliaries[0]) > x);
                }
                previous = Some(drawn_x(&auxiliaries[0]));
            }
        }
        assert_eq!(records, records_before);
        assert_eq!(segments, segments_before);
        assert_eq!(nodes, nodes_before);
    }

    #[test]
    fn car_glides_across_frames_between_logic_ticks() {
        let (mut records, mut segments, mut nodes, mut auxiliaries) = straight_lane();
        let mut tasks = vec![logic_task()];
        let mut ticks = 0;
        let mut previous = f32::NEG_INFINITY;
        // 60 fps: 17, 17, 16 ms frames; the 25 Hz logic task fires every 40 ms.
        for frame in 0..30 {
            let elapsed = [17, 17, 16][frame % 3];
            timers::dispatch(&mut tasks, elapsed, |_, address, _| {
                assert_eq!(address, timers::LOGIC_CALLBACK);
                tick(&mut records);
                ticks += 1;
                Ok(())
            })
            .unwrap();
            let factor = timers::logic_progress(&tasks).unwrap();
            assert!((0. ..1.).contains(&factor), "factor {factor}");
            let graph = Graph {
                segments: &mut segments,
                segment_base: SEGMENT_BASE,
                nodes: &mut nodes,
                node_base: NODE_BASE,
            };
            project(
                &records,
                &graph,
                &mut auxiliaries,
                AUXILIARY_BASE,
                factor,
                &settings(),
            )
            .unwrap();
            let x = drawn_x(&auxiliaries[0]);
            assert!(x > previous, "frame {frame}: {x} after {previous}");
            previous = x;
        }
        assert_eq!(ticks, 30 * 50 / 3 / 40);
    }

    #[test]
    fn paused_logic_timer_holds_the_drawn_pose() {
        let (records, mut segments, mut nodes, mut auxiliaries) = straight_lane();
        let mut tasks = vec![logic_task()];
        timers::dispatch(&mut tasks, 25, |_, _, _| Ok(())).unwrap();
        // A pause disables the group-2 logic task (4eb720); its accumulator
        // and so the factor stop, and nothing moves while frames continue.
        tasks[0][0] = 0;
        let graph = Graph {
            segments: &mut segments,
            segment_base: SEGMENT_BASE,
            nodes: &mut nodes,
            node_base: NODE_BASE,
        };
        let mut drawn = None;
        for _ in 0..10 {
            timers::dispatch(&mut tasks, 17, |_, _, _| panic!("paused logic ticked")).unwrap();
            let factor = timers::logic_progress(&tasks).unwrap();
            assert_eq!(factor, 25. / 40.);
            project(
                &records,
                &graph,
                &mut auxiliaries,
                AUXILIARY_BASE,
                factor,
                &settings(),
            )
            .unwrap();
            let x = drawn_x(&auxiliaries[0]);
            assert_eq!(*drawn.get_or_insert(x), x);
        }
    }
    #[test]
    #[ignore = "requires original unseamed vehicle projection observations"]
    fn matches_original_projection() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let settings = ProjectionSettings::from_v14(&image).unwrap();
        let data = std::fs::read(crate::fixtures::path("NATIVE_VEHICLE_PROJECTION")).unwrap();
        assert_eq!(&data[..8], b"MTVPROJ1");
        let mut at = 12;
        fn rows<const N: usize, const M: usize>(data: &[u8], at: &mut usize) -> [[u8; N]; M] {
            let rows =
                std::array::from_fn(|i| data[*at + i * N..*at + (i + 1) * N].try_into().unwrap());
            *at += N * M;
            rows
        }
        for case in 0..word(&data, 8) {
            let fraction = f32::from_bits(word(&data, at));
            at += 4;
            let records = rows::<208, 200>(&data, &mut at);
            let mut segments = rows::<108, 4>(&data, &mut at);
            let mut nodes = rows::<44, 4>(&data, &mut at);
            let mut aux = rows::<284, 3>(&data, &mut at);
            let expected = rows::<284, 3>(&data, &mut at);
            let graph = Graph {
                segments: &mut segments,
                segment_base: 0x10010000,
                nodes: &mut nodes,
                node_base: 0x10011000,
            };
            project(&records, &graph, &mut aux, 0x10020000, fraction, &settings).unwrap();
            for i in 0..3 {
                for offset in 0..284 {
                    assert_eq!(
                        aux[i][offset], expected[i][offset],
                        "case {case} auxiliary{i} byte{offset:#x}"
                    );
                }
            }
        }
        assert_eq!(at, data.len());
    }
}
