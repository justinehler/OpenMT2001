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
    #[test]
    #[ignore = "requires original unseamed vehicle projection observations"]
    fn matches_original_projection() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let settings = ProjectionSettings::from_v14(&image).unwrap();
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_VEHICLE_PROJECTION").unwrap()).unwrap();
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
