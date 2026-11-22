//! Static map setup and the recovered building-selection stage.
//! This accepts the literal AddBuilding/AddBusiness subset used by apartmentrim;
//! unsupported Lua is rejected. It is not a general Lua VM or business simulation.
use crate::{
    board::Board,
    build_file::{BuildFile, BuildingLayer, ModelMatch, ModelQuery},
};

#[derive(Debug, Clone)]
pub struct BuildingSpec {
    pub block: String,
    pub lot: [u8; 4], // x, y, width, depth
    pub height: u8,
    pub kind: u8, // native RETAIL=0, RESIDENTIAL=1, LANDSCAPE=2
    pub variation: u32,
    pub quality: u8, // native low=0, medium=1, high=2
    pub businesses: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SceneBuilding {
    pub spec: BuildingSpec,
    pub block_id: usize,
    pub center: [f32; 2],
    pub selected: ModelMatch,
    /// Native868190: actual original template matches (at most20).
    pub model_variants: usize,
    pub angle: f32,
    pub mirror: bool,
    pub canonical_size: [u8; 2],
    pub layers: Vec<BuildingLayer>,
}

struct ModelSelection {
    size: [u8; 2],
    angle: f32,
    corner_flip: bool,
    query: ModelQuery,
}

/// The placement-specific search inputs are shared by the visible style count
/// and the actual building constructor. Original00488230 calls004d4e50 with
/// the same corner transpose and at-most-20 match limit for both.
fn model_selection(spec: &BuildingSpec) -> Result<ModelSelection, String> {
    let [x, y, w, d] = spec.lot;
    if w == 0
        || d == 0
        || u16::from(x) + u16::from(w) > 7
        || u16::from(y) + u16::from(d) > 7
        || spec.height == 0
    {
        return Err("invalid building lot".into());
    }
    let east = x + w == 7;
    let south = y + d == 7;
    let corners = (x == 0 && y == 0) as u8
        | ((x == 0 && south) as u8) << 1
        | ((east && y == 0) as u8) << 2
        | ((east && south) as u8) << 3;
    let mut size = [w, d];
    let (angle, corner_flip) = if corners == 0 {
        if y == 0 {
            (0., false)
        } else if x == 0 {
            size = [d, w];
            (90., false)
        } else if south {
            (180., false)
        } else {
            size = [d, w];
            (270., false)
        }
    } else if corners & 1 != 0 {
        (0., false)
    } else if corners & 4 != 0 {
        (270., true)
    } else if corners & 2 != 0 {
        (90., true)
    } else {
        (180., false)
    };
    Ok(ModelSelection {
        size,
        angle,
        corner_flip,
        query: ModelQuery {
            width: size[0],
            depth: size[1],
            height: spec.height,
            quality: if spec.kind == 2 { 0 } else { spec.quality + 1 },
            placement: if corners == 0 { 2 } else { 1 },
            category: match spec.kind {
                0 => 2,
                1 => 1,
                2 => 5,
                _ => return Err("unsupported building type".into()),
            },
            limit: 20,
        },
    })
}

/// Original mirror followed by Y rotation, used by both position and normal buffers.
pub fn baked_normal([x, y, mut z]: [f32; 3], mirror: bool, angle: f32) -> [f32; 3] {
    if mirror {
        z = -z;
    }
    let radians = f64::from(angle) * f64::from(f32::from_bits(0x3c8efa35));
    let sine = f64::from(radians.sin() as f32);
    let cosine = f64::from(radians.cos() as f32);
    [
        (f64::from(x) * cosine - f64::from(z) * sine) as f32,
        y,
        (f64::from(z) * cosine + f64::from(x) * sine) as f32,
    ]
}

impl SceneBuilding {
    /// Count the original template choices as soon as a footprint is selected,
    /// before the asynchronous quote or preview is returned to the frontend.
    pub fn model_variant_count(
        spec: &BuildingSpec,
        file: &BuildFile<'_>,
    ) -> Result<usize, String> {
        Ok(file.search(model_selection(spec)?.query)?.len())
    }

    /// Compact fields are consumed directly by original 00488230. Membership
    /// was rebuilt by SaveFile::buildings from the 00447d70 category ID lists.
    pub fn from_saved(
        saved: &crate::save::SavedBuilding<'_>,
        board: &Board,
        file: &BuildFile<'_>,
    ) -> Result<Self, String> {
        let block = board
            .blocks
            .iter()
            .find(|b| b.id == saved.category)
            .ok_or("saved building belongs to an unknown/inactive block")?;
        let [x0, y0, x1, y1] = saved.lot_endpoints();
        if x1 < x0 || y1 < y0 || x1 >= 7 || y1 >= 7 {
            return Err("invalid saved building endpoints".into());
        }
        let kind = u32::from_le_bytes(saved.bytes[0x24..0x28].try_into().unwrap());
        let quality = saved.bytes[0x2c];
        if kind > 2 || quality > 2 {
            return Err("unsupported saved building kind or quality".into());
        }
        let mut result = Self::resolve(
            BuildingSpec {
                block: block.name.clone(),
                lot: [x0, y0, x1 - x0 + 1, y1 - y0 + 1],
                height: saved.height(),
                kind: kind as u8,
                variation: u32::from_le_bytes(saved.bytes[0x20..0x24].try_into().unwrap()),
                quality,
                businesses: Vec::new(),
            },
            board,
            file,
        )?;
        // The save records authoritative world coordinates. Keep those bytes;
        // the corpus test separately checks the recovered placement formula.
        if !saved.center().into_iter().all(f32::is_finite) {
            return Err("non-finite saved building center".into());
        }
        result.center = saved.center();
        Ok(result)
    }

    /// Original mesh-space placement, before adding the building's world center.
    /// 004889e2 translates, 004ea9f0 mirrors, then 004eab40 rotates.
    pub fn local_position(&self, position: [f32; 3]) -> [f32; 3] {
        let [mut width, mut depth] = self.canonical_size.map(f32::from);
        if self.selected.transposed {
            core::mem::swap(&mut width, &mut depth);
        }
        let x = position[0] - width * 5.;
        let y = position[1] + 0.;
        let z = position[2] - ((7. - depth) * 10. + depth * 5.);
        baked_normal([x, y, z], self.mirror, self.angle)
    }

    /// Auxiliary entrance placement at00488ef7, then category-list adjustment
    /// at004460d6. Unlike the mesh transform, cosine remains a retained double
    /// and the original auxiliary path negates final Z.
    pub fn entrance_positions(&self, category_center: [f32; 2]) -> Result<Vec<[f32; 3]>, String> {
        let delta: [f32; 2] = std::array::from_fn(|i| {
            (f64::from(category_center[i]) - f64::from(self.center[i])) as f32
        });
        Ok(self
            .prepared_entrance_positions()?
            .into_iter()
            .map(|[x, y, z]| {
                [
                    (f64::from(x) - f64::from(delta[0])) as f32,
                    y,
                    (f64::from(z) - f64::from(delta[1])) as f32,
                ]
            })
            .collect())
    }

    /// Original488ef7 output before4460d6 applies the category-center offset.
    /// Fresh building binding consumes these values and performs that step once.
    pub fn prepared_entrance_positions(&self) -> Result<Vec<[f32; 3]>, String> {
        let prepared = crate::geometry::prepare(&self.layers);
        let points = crate::geometry::entrance_positions(&prepared)?;
        let [mut width, mut depth] = self.canonical_size.map(f32::from);
        if self.selected.transposed {
            core::mem::swap(&mut width, &mut depth);
        }
        let offset = [-width * 5., 0., -((7. - depth) * 10. + depth * 5.)];
        let radians = f64::from(self.angle) * f64::from(f32::from_bits(0x3c8efa35));
        let sine = f64::from(radians.sin() as f32);
        let cosine = radians.cos();
        Ok(points
            .into_iter()
            .map(|point| {
                let x = f64::from((f64::from(point[0]) + f64::from(offset[0])) as f32);
                let y = (f64::from(point[1]) + f64::from(offset[1])) as f32;
                let mut z = (f64::from(point[2]) + f64::from(offset[2])) as f32;
                if self.mirror {
                    z = -z;
                }
                let z = f64::from(z);
                let local_x = (x * cosine - z * sine) as f32;
                let local_z = -(x * sine + z * cosine) as f32;
                [local_x, y, local_z]
            })
            .collect())
    }

    /// Existing generated normals are mirrored/rotated by004ea9f0/004eab40.
    /// They are not recomputed from rounded transformed positions.
    pub fn local_normal(&self, normal: [f32; 3]) -> [f32; 3] {
        baked_normal(normal, self.mirror, self.angle)
    }

    /// 004eaa70 reverses each triangle when the mesh was mirrored.
    pub fn local_triangle(&self, mut triangle: [u32; 3]) -> [u32; 3] {
        if self.mirror {
            triangle.swap(0, 1);
        }
        triangle
    }

    /// Native 00488230 through 004888da, and placement math from 00446130.
    pub fn resolve(
        spec: BuildingSpec,
        board: &Board,
        file: &BuildFile<'_>,
    ) -> Result<Self, String> {
        let block = board
            .blocks
            .iter()
            .rev()
            .find(|b| b.name == spec.block)
            .ok_or("unknown/inactive block")?;
        let selection = model_selection(&spec)?;
        let [x, y, w, d] = spec.lot;
        let mut angle = selection.angle;
        let size = selection.size;
        let matches = file.search(selection.query)?;
        if matches.is_empty() {
            return Err(format!(
                "no original building template for {} {:?}",
                spec.block, spec.lot
            ));
        }
        let selected = matches[((spec.variation >> 16) & 0x7fff) as usize % matches.len()];
        let mirror = selected.transposed != selection.corner_flip;
        if mirror {
            angle -= 90.;
        }
        let layers = file.profile(selected.index, spec.height)?;
        let center = [
            block.center[0] - 35. + f32::from(x) * 10. + f32::from(w) * 5.,
            block.center[1] - 35. + f32::from(y) * 10. + f32::from(d) * 5.,
        ];
        Ok(Self {
            spec,
            block_id: block.id,
            center,
            selected,
            model_variants: matches.len(),
            angle,
            mirror,
            canonical_size: size,
            layers,
        })
    }
}

/// Parse the entire static map or fail, rather than quietly skipping script logic.
pub fn parse_static_map(source: &str) -> Result<Vec<BuildingSpec>, String> {
    let mut buildings: Vec<BuildingSpec> = Vec::new();
    // The supported map export uses one literal statement per line. Comments
    // and blank lines are permitted; expressions/control flow deliberately fail.
    for (line_number, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("--") {
            continue;
        }
        let fail = || {
            format!(
                "unsupported static map statement at line {}",
                line_number + 1
            )
        };
        if let Some(args) = line
            .strip_prefix("index = AddBuilding(")
            .and_then(|s| s.strip_suffix(");"))
        {
            let args: Vec<_> = args.split(',').map(str::trim).collect();
            if args.len() != 9 {
                return Err(fail());
            }
            let string = |s: &str| -> Result<String, String> {
                s.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .filter(|s| !s.contains(['"', '\\']))
                    .map(str::to_owned)
                    .ok_or_else(fail)
            };
            let block = string(args[0])?;
            let mut lot = [0u8; 4];
            for i in 0..4 {
                lot[i] = args[i + 1].parse().map_err(|_| fail())?;
            }
            let height = args[5].parse().map_err(|_| fail())?;
            let kind = match string(args[6])?.to_ascii_uppercase().as_str() {
                "RETAIL" => 0,
                "RESIDENTIAL" => 1,
                "LANDSCAPE" => 2,
                _ => return Err(fail()),
            };
            let variation = args[7].parse().map_err(|_| fail())?;
            let quality = match string(args[8])?.to_ascii_lowercase().as_str() {
                "low" => 0,
                "medium" => 1,
                "high" => 2,
                _ => return Err(fail()),
            };
            buildings.push(BuildingSpec {
                block,
                lot,
                height,
                kind,
                variation,
                quality,
                businesses: Vec::new(),
            });
        } else if let Some(business) = line
            .strip_prefix("AddBusiness(index, \"")
            .and_then(|s| s.strip_suffix("\");"))
        {
            if business.contains(['"', '\\']) {
                return Err(fail());
            }
            buildings
                .last_mut()
                .ok_or_else(fail)?
                .businesses
                .push(business.to_owned());
        } else {
            return Err(fail());
        }
    }
    Ok(buildings)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original BuildFile/PE and native488230 count observations"]
    fn original_model_variant_counts() {
        let executable = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&executable).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let bytes = std::fs::read(std::env::var("OPENMT2001_BUILD_FILE").unwrap()).unwrap();
        let file = BuildFile::parse(&bytes).unwrap();
        let native = std::fs::read("work/economy/native-build-variant-count.bin").unwrap();
        assert_eq!(&native[..8], b"MTBVAR01");
        let mut words = native[8..].chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()));
        assert_eq!(words.next(), Some(0x037f));
        assert_eq!(words.next(), Some(45));
        let mut populated = 0;
        for case in 0..45 {
            let input: [u32; 16] = std::array::from_fn(|_| words.next().unwrap());
            let spec = BuildingSpec {
                block: board.blocks[0].name.clone(),
                lot: std::array::from_fn(|i| input[i] as u8),
                height: input[4] as u8, kind: input[5] as u8,
                quality: input[6] as u8, variation: 0, businesses: Vec::new(),
            };
            assert_eq!((input[14], input[15]), (20, 0), "native cap/random flag case{case}");
            // Exact search operands have their own original-query fixture;
            // this checks the count and oriented dimensions exported to UI.
            assert_eq!(SceneBuilding::model_variant_count(&spec, &file).unwrap(),
                input[7] as usize, "early native UI count case{case}");
            let resolved = SceneBuilding::resolve(spec, &board, &file);
            if input[7] == 0 {
                assert!(resolved.is_err(), "native empty template case{case}");
            } else {
                let resolved = resolved.unwrap();
                assert_eq!(resolved.model_variants, input[7] as usize, "native count case{case}");
                assert_eq!(resolved.canonical_size, [input[8] as u8, input[9] as u8],
                    "native query dimensions case{case}");
                populated += 1;
            }
        }
        assert_eq!(populated, 30);
        assert!(words.next().is_none());
    }

    use super::*;
    #[test]
    #[ignore = "requires original BuildFile and native entrance emission/placement fixture"]
    fn matches_native_housing_positions() {
        let data = std::fs::read(std::env::var("OPENMT2001_BUILD_FILE").unwrap()).unwrap();
        let file = BuildFile::parse(&data).unwrap();
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_HOUSING_POSITIONS").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(&bytes[..8], b"MTHPOS01");
        assert_eq!((word(8), word(12)), (39, 0x027f));
        let mut at = 16;
        for case in 0..word(8) {
            let model = word(at) as usize;
            let height = word(at + 4) as u8;
            let building = SceneBuilding {
                spec: BuildingSpec {
                    block: String::new(),
                    lot: [0, 0, 1, 1],
                    height,
                    kind: 1,
                    variation: 0,
                    quality: 0,
                    businesses: vec![],
                },
                block_id: 0,
                center: [float(at + 28), float(at + 32)],
                selected: ModelMatch {
                    index: model,
                    transposed: word(at + 20) != 0,
                },
                model_variants: 0, // This fixture supplies a profile, not a template search.
                angle: float(at + 24),
                mirror: word(at + 16) != 0,
                canonical_size: [word(at + 8) as u8, word(at + 12) as u8],
                layers: file.profile(model, height).unwrap(),
            };
            let category_center = [float(at + 36), float(at + 40)];
            let count = word(at + 44) as usize;
            at += 48;
            let prepared = crate::geometry::prepare(&building.layers);
            let entrances = crate::geometry::entrance_positions(&prepared).unwrap();
            assert_eq!(entrances.len(), count, "entrance count case {case}");
            for point in entrances {
                assert_eq!(
                    point.map(f32::to_bits),
                    [word(at), word(at + 4), word(at + 8)],
                    "emission case {case}"
                );
                at += 12;
            }
            for point in building.entrance_positions(category_center).unwrap() {
                assert_eq!(
                    point.map(f32::to_bits),
                    [word(at), word(at + 4), word(at + 8)],
                    "placement case {case}"
                );
                at += 12;
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original map, executable, BuildFile and native mesh transforms"]
    fn matches_native_mesh_transforms() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let data = read("OPENMT2001_BUILD_FILE");
        let native = read("OPENMT2001_NATIVE_TRANSFORMS");
        let map = read("OPENMT2001_MAP");
        let specs = parse_static_map(std::str::from_utf8(&map).unwrap()).unwrap();
        let board = Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let file = BuildFile::parse(&data).unwrap();
        assert_eq!(&native[..8], b"MTTRANS1");
        assert_eq!(native.len(), 12 + specs.len() * 60);
        let probes = [
            [0., 0., 0.],
            [10., 3.25, 70.],
            [23.75, 8., 41.5],
            [-2., -1., 5.],
        ];
        for (i, spec) in specs.into_iter().enumerate() {
            let building = SceneBuilding::resolve(spec, &board, &file).unwrap();
            let at = 12 + i * 60;
            for (v, probe) in probes.into_iter().enumerate() {
                let expected: [u32; 3] = core::array::from_fn(|axis| {
                    let p = at + v * 12 + axis * 4;
                    u32::from_le_bytes(native[p..p + 4].try_into().unwrap())
                });
                assert_eq!(
                    building.local_position(probe).map(f32::to_bits),
                    expected,
                    "building {i} probe {v}"
                );
            }
            for (t, triangle) in [[0, 1, 2], [0, 2, 3]].into_iter().enumerate() {
                let expected = core::array::from_fn(|axis| {
                    let p = at + 48 + t * 6 + axis * 2;
                    u32::from(u16::from_le_bytes(native[p..p + 2].try_into().unwrap()))
                });
                assert_eq!(
                    building.local_triangle(triangle),
                    expected,
                    "building {i} triangle {t}"
                );
            }
        }
    }
    #[test]
    #[ignore = "requires original static map, executable, BuildFile and native selection observations"]
    fn matches_native_map_selection() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let data = read("OPENMT2001_BUILD_FILE");
        let native = read("OPENMT2001_NATIVE_PLACEMENTS");
        let map = read("OPENMT2001_MAP");
        let specs = parse_static_map(std::str::from_utf8(&map).unwrap()).unwrap();
        let board = Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let file = BuildFile::parse(&data).unwrap();
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTPLACE1");
        assert_eq!(specs.len(), word(8) as usize);
        for (i, spec) in specs.into_iter().enumerate() {
            let building = SceneBuilding::resolve(spec, &board, &file).unwrap();
            let at = 12 + i * 20;
            assert_eq!(
                building.selected.index as u32
                    | if building.selected.transposed {
                        0x80000000
                    } else {
                        0
                    },
                word(at),
                "building {i} template"
            );
            assert_eq!(building.angle.to_bits(), word(at + 4), "building {i} angle");
            assert_eq!(
                building.canonical_size.map(u32::from),
                [word(at + 8), word(at + 12)],
                "building {i} dimensions"
            );
            assert_eq!(
                u32::from(building.mirror),
                word(at + 16),
                "building {i} mirror"
            );
        }
    }
    #[test]
    #[ignore = "requires supplied save, original assets and native saved-building selection observations"]
    fn saved_buildings_match_native_selection() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let data = read("OPENMT2001_BUILD_FILE");
        let bytes = read("OPENMT2001_SAVE_ONE");
        let native = read("OPENMT2001_NATIVE_SAVED_PLACEMENTS");
        let board = Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let file = BuildFile::parse(&data).unwrap();
        let save = crate::save::SaveFile::parse(&bytes).unwrap();
        let mut saved = save.buildings().unwrap();
        saved.sort_by_key(|b| b.slot);
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTPLACE1");
        assert_eq!(word(8) as usize, saved.len());
        for (i, saved) in saved.iter().enumerate() {
            let b = SceneBuilding::from_saved(saved, &board, &file).unwrap();
            let at = 12 + i * 20;
            assert_eq!(
                b.selected.index as u32 | if b.selected.transposed { 0x80000000 } else { 0 },
                word(at),
                "saved building {i}"
            );
            assert_eq!(b.angle.to_bits(), word(at + 4));
            assert_eq!(
                b.canonical_size.map(u32::from),
                [word(at + 8), word(at + 12)]
            );
            assert_eq!(u32::from(b.mirror), word(at + 16));
            assert_eq!(b.center.map(f32::to_bits), saved.center().map(f32::to_bits));
            let rebuilt = SceneBuilding::resolve(b.spec, &board, &file).unwrap();
            assert_eq!(
                rebuilt.center.map(f32::to_bits),
                saved.center().map(f32::to_bits),
                "saved building {i} reconstructed center"
            );
        }
        assert_eq!(native.len(), 12 + saved.len() * 20);
    }

    #[test]
    fn rejects_unimplemented_script_logic() {
        assert!(parse_static_map("for i=1,10 do").is_err());
        assert!(parse_static_map("AddBusiness(index, \"apartments\");").is_err());
    }
}
