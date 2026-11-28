//! Native 00484030 polygon triangulation.
//!
//! The input follows the original profile ABI: the outline is closed by a
//! duplicate final point.  The duplicate is counted in the input but is not
//! an eligible vertex, and emitted indices refer to the original point slice.

/// Triangulate one native closed outline.
///
/// This is the recovered 00484030 ear-selection order, including its strict
/// first-win chord comparison and removal of the selected middle vertex,
/// including index zero. An empty result is the native return value 0
/// (fewer than three effective vertices or no admissible ear).
pub fn triangulate(points: &[[f32; 2]]) -> Vec<[u16; 3]> {
    // 00484030 receives a count that includes the duplicate closing point and
    // immediately decrements it.  Keep that ABI behavior instead of silently
    // repairing open or differently closed outlines.
    let mut remaining = points.len().saturating_sub(1);
    if remaining < 3 {
        return Vec::new();
    }

    let mut order: Vec<usize> = (0..remaining).collect();
    let mut triangles = Vec::with_capacity(remaining - 2);
    // The native FLD loads this f32 constant into x87 extended precision.
    // Each selected chord is then FST to f32 before it becomes the next best
    // distance, so retain the explicit store boundary below.
    while remaining >= 3 {
        let mut best_distance = f64::from(1_000_000_000.0f32);
        let mut best = None;
        for i in 0..remaining {
            let a = order[i % remaining];
            let b = order[(i + 1) % remaining];
            let c = order[(i + 2) % remaining];

            // Native FLD/FSUB/FMUL/FSUB keeps these values in x87 extended
            // precision.  f64 is the closest stable Rust representation; the
            // prepared game outlines are f32 inputs and the observed store
            // boundary is preserved for the distance accumulator.
            let ac_x = f64::from(points[a][0]) - f64::from(points[c][0]);
            let ac_y = f64::from(points[a][1]) - f64::from(points[c][1]);
            let ab_x = f64::from(points[a][0]) - f64::from(points[b][0]);
            let ab_y = f64::from(points[a][1]) - f64::from(points[b][1]);
            let cross = ab_x * ac_y - ab_y * ac_x;
            if cross < 0.0 || cross.is_nan() {
                continue;
            }

            // 004840f5..00484103 computes AC² in x87 and FSTs it to a
            // temporary f32.  The following comparison is against the
            // current extended value, which is either 1e9f or the prior
            // selected value reloaded from that f32 temporary.
            let distance_ext = ac_x * ac_x + ac_y * ac_y;
            if distance_ext < best_distance {
                best = Some(i);
                best_distance = f64::from(distance_ext as f32);
            }
        }

        let Some(best) = best else {
            // The native function may have emitted earlier triangles, but its
            // return value is zero on this path; callers use the return count
            // and therefore observe no usable triangulation.
            return Vec::new();
        };

        let middle = (best + 1) % remaining;
        triangles.push([
            order[best] as u16,
            order[middle] as u16,
            order[(best + 2) % remaining] as u16,
        ]);

        // The native loop shifts entries after middle down one, including
        // when the selected ear wraps around and middle is index zero.
        order.copy_within((middle + 1)..remaining, middle);
        remaining -= 1;
    }

    triangles
}

#[cfg(test)]
mod tests {
    use super::triangulate;

    #[test]
    fn duplicate_closing_point_is_excluded() {
        let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]];
        assert_eq!(triangulate(&square).len(), 2);
        assert!(triangulate(&square).iter().flatten().all(|&i| i < 4));
    }

    #[test]
    fn native_no_ear_is_empty() {
        let opposite_winding = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        assert!(triangulate(&opposite_winding).is_empty());
    }

    #[test]
    #[ignore = "requires work/scene/native-polygon.bin from native_polygon.py"]
    fn matches_native_prepared_polygon_fixture() {
        let path = crate::fixtures::path("NATIVE_POLYGON");
        let data = std::fs::read(path).unwrap();
        assert_eq!(&data[..8], b"MTPOLY01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let cases = word(8) as usize;
        let mut at = 12;
        for case in 0..cases {
            let index = word(at);
            let height = word(at + 4);
            let layer = word(at + 8);
            let point_count = word(at + 12) as usize;
            let triangle_count = word(at + 16) as usize;
            at += 20;
            let mut points = Vec::with_capacity(point_count);
            for _ in 0..point_count {
                points.push([f32::from_bits(word(at)), f32::from_bits(word(at + 4))]);
                at += 8;
            }
            let expected = (0..triangle_count)
                .map(|_| {
                    let half =
                        |offset| u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
                    let triangle = [half(at), half(at + 2), half(at + 4)];
                    at += 6;
                    triangle
                })
                .collect::<Vec<_>>();
            assert_eq!(
                triangulate(&points),
                expected,
                "case {case} profile={index} height={height} layer={layer}"
            );
        }
        assert_eq!(at, data.len());
    }
}
