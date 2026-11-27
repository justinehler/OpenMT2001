//! Original 485200 model-detail identities used by saved appearance refresh.
//! These are renderer registry candidates, not a replacement for 4846f0.

use crate::build_file::{BuildFile, ModelMatch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIdentity {
    /// Original building+0x50 low model index and high transpose bit.
    pub descriptor: u32,
    /// Original 485200 renderer entries in profile detail order.
    pub renderer_candidates: Vec<RendererCandidate>,
    /// Native 485200 registry rows before renderer handle construction.
    pub renderer_templates: Vec<RendererTemplate>,
}

/// Convert the retained 488230 selection into the native saved model identity.
/// The current frontend loads a single BuildFile bank, so no bank bits are set.
pub fn model_identity(
    file: &BuildFile<'_>,
    selected: ModelMatch,
    height: u8,
) -> Result<ModelIdentity, String> {
    let index =
        u32::try_from(selected.index).map_err(|_| "saved appearance model index exceeds DWORD")?;
    if index > 0x00ff_ffff {
        return Err("saved appearance model index exceeds native descriptor".into());
    }
    Ok(ModelIdentity {
        descriptor: index | if selected.transposed { 0x8000_0000 } else { 0 },
        renderer_candidates: renderer_candidates(file, selected.index)?,
        renderer_templates: renderer_templates(file, selected, height)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCandidate {
    /// Zero-based detail slot in 4d4a00's 32-row expanded profile.
    pub detail_slot: u8,
    /// Original 485200 model descriptor (profile row +0x18).
    pub descriptor: u32,
}

/// Native 485200's 52-byte per-detail registry prefix after 4846f0 dispatch.
/// Word +0x28 is a renderer-owned model handle; zero means no handle has
/// been constructed by the retained scene. The other words are reproducible
/// from the immutable BuildFile/profile under x87 FPCW 0x037f.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererTemplate {
    pub detail_slot: u8,
    pub descriptor: u32,
    pub row: [u8; 52],
}

/// Owned 485200 rows for one building. Native +2e8 holds these identities in
/// detail order; +0x28 in each row is an opaque renderer handle. The caller
/// owns construction and release of those handles, so this type preserves
/// synchronous 485520 ordering without fabricating renderer objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererRegistry {
    pub rows: Vec<RendererTemplate>,
}

impl RendererRegistry {
    pub fn prepare(
        identity: &ModelIdentity,
        mut construct: impl FnMut(&RendererTemplate) -> Result<u32, String>,
    ) -> Result<Self, String> {
        let mut rows = identity.renderer_templates.clone();
        for row in &mut rows {
            if word(&row.row, 0) == 1 {
                let handle = construct(row)?;
                row.row[0x28..0x2c].copy_from_slice(&handle.to_le_bytes());
            }
        }
        Ok(Self { rows })
    }

    /// Original 485520 releases and reconstructs each active detail before
    /// visiting the next. Inactive entries retain their registry handle.
    pub fn refresh(
        &mut self,
        mut release: impl FnMut(u32) -> Result<(), String>,
        mut construct: impl FnMut(&RendererTemplate) -> Result<u32, String>,
    ) -> Result<(), String> {
        for row in &mut self.rows {
            if word(&row.row, 0) != 1 {
                continue;
            }
            let old = word(&row.row, 0x28);
            release(old)?;
            let new = construct(row)?;
            row.row[0x28..0x2c].copy_from_slice(&new.to_le_bytes());
        }
        Ok(())
    }
}

/// Prepare the ordered native registry rows without claiming renderer handles.
/// Dynamic constructor classes use owned rows. The 4846f0 static-pointer
/// classes are retained as inactive identities; their original static mesh
/// pointers and draw submission remain a separate reconstruction edge.
pub fn renderer_templates(
    file: &BuildFile<'_>,
    selected: ModelMatch,
    height: u8,
) -> Result<Vec<RendererTemplate>, String> {
    let layers = file.profile(selected.index, height)?;
    let model = file
        .models
        .chunks_exact(28)
        .nth(selected.index)
        .ok_or("saved appearance model leaves BuildFile")?;
    let first = word(model, 8) as usize;
    let mut result = Vec::new();
    for candidate in renderer_candidates(file, selected.index)? {
        let detail = file
            .details
            .chunks_exact(32)
            .nth(first + candidate.detail_slot as usize)
            .ok_or("saved appearance detail leaves BuildFile")?;
        let layer = layers
            .get(word(detail, 28) as usize)
            .ok_or("saved appearance detail layer leaves expanded profile")?;
        let (active, subtype) = match candidate.descriptor {
            0x0104_0201 | 0x0200_0001 => (true, false),
            0x0101_0101 | 0x0102_0101 | 0x0101_0102 | 0x0102_0102 | 0x0101_0104 | 0x0101_0201
            | 0x0102_0201 | 0x0101_0204 | 0x0102_0204 | 0x0104_0202 | 0x0104_0204 | 0x0108_0202 => {
                (true, true)
            }
            // Native 4846f0 copies one of its static 16-byte model pointers
            // and clears the active flag for these values. V2 currently keeps
            // their identity but does not materialize those separate meshes.
            0x0200_0002
            | 0x0200_0003
            | 0x0200_0005..=0x0200_0018
            | 0x0200_0026
            | 0x0200_0037
            | 0x0200_003b
            | 0x0200_003e
            | 0x0200_003f
            | 0x0200_0041
            | 0x0200_0042
            | 0x0200_0044
            | 0x0200_0049..=0x0200_004f
            | 0x44
            | 0x49..=0x4f => (false, false),
            _ => {
                return Err(format!(
                    "saved appearance descriptor {:08x} has no native 4846f0 class",
                    candidate.descriptor
                ));
            }
        };
        let f = |at| f32::from_bits(word(detail, at));
        let scaled = |at| (f64::from(f(at)) * 10.0) as f32;
        let mut y = (f64::from(layer.base) + f64::from(f(4))) as f32;
        if candidate.descriptor == 0x0108_0202 {
            // Native stores scaled detail+0x14 as f32 before multiplying
            // by its 0.7f constant and subtracting from the stored Y.
            y = (f64::from(y) - f64::from(scaled(20)) * f64::from(f32::from_bits(0x3f33_3333)))
                as f32;
        }
        let mut row = [0u8; 52];
        let mut put = |at: usize, value: u32| row[at..at + 4].copy_from_slice(&value.to_le_bytes());
        put(0, u32::from(active));
        put(4, candidate.descriptor);
        put(8, 1 | if subtype { 0x100 } else { 0 });
        put(12, scaled(0).to_bits());
        put(16, y.to_bits());
        put(20, ((-f64::from(f(8)) * 10.0) as f32).to_bits());
        put(24, 1.0f32.to_bits());
        put(28, 1.0f32.to_bits());
        put(32, 1.0f32.to_bits());
        put(36, word(detail, 12));
        put(40, 0); // renderer-owned model handle
        put(44, scaled(16).to_bits());
        put(48, scaled(20).to_bits());
        result.push(RendererTemplate {
            detail_slot: candidate.detail_slot,
            descriptor: candidate.descriptor,
            row,
        });
    }
    Ok(result)
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// The 485200 category filter before registry allocation or 4846f0.
/// Original 4d4a00 copies the model's detail rows in order to +1c698;
/// rows beyond the model's count are zero and are skipped by this filter.
pub fn renderer_candidates(
    file: &BuildFile<'_>,
    model_index: usize,
) -> Result<Vec<RendererCandidate>, String> {
    let model = file
        .models
        .chunks_exact(28)
        .nth(model_index)
        .ok_or("saved appearance model leaves BuildFile")?;
    let word = |row: &[u8], at: usize| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
    let first = word(model, 8) as usize;
    let count = word(model, 12) as usize;
    let mut candidates = Vec::new();
    for slot in 0..count.min(32) {
        let row = file
            .details
            .chunks_exact(32)
            .nth(first + slot)
            .ok_or("saved appearance detail leaves BuildFile")?;
        let descriptor = word(row, 24);
        if matches!(
            descriptor,
            u32::MAX
                | 0
                | 0x0200_0045
                | 0x45
                | 0x0200_0004
                | 0x0200_0046
                | 0x46
                | 0x0200_0047
                | 0x47
                | 0x0200_0048
                | 0x48
        ) {
            continue;
        }
        candidates.push(RendererCandidate {
            detail_slot: slot as u8,
            descriptor,
        });
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original v1.4 BuildFile"]
    fn native_static_pointer_and_double_sign_dispatch_classes() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_BUILD_FILE").unwrap()).unwrap();
        let file = BuildFile::parse(&bytes).unwrap();
        let static_model = model_identity(
            &file,
            ModelMatch {
                index: 1059,
                transposed: false,
            },
            1,
        )
        .unwrap();
        assert_eq!(
            static_model
                .renderer_templates
                .iter()
                .map(|row| (row.descriptor, word(&row.row, 0)))
                .collect::<Vec<_>>(),
            [(0x0108_0202, 1), (0x0200_000f, 0)]
        );
        let double_sign = model_identity(
            &file,
            ModelMatch {
                index: 335,
                transposed: false,
            },
            1,
        )
        .unwrap();
        assert!(
            double_sign
                .renderer_templates
                .iter()
                .any(|row| row.descriptor == 0x0104_0204 && word(&row.row, 0) == 1)
        );
    }

    #[test]
    #[ignore = "requires original v1.4 BuildFile"]
    fn native_double_backed_text_registry_class() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_BUILD_FILE").unwrap()).unwrap();
        let file = BuildFile::parse(&bytes).unwrap();
        let identity = model_identity(
            &file,
            ModelMatch {
                index: 311,
                transposed: false,
            },
            1,
        )
        .unwrap();
        assert!(
            identity
                .renderer_templates
                .iter()
                .any(|row| row.descriptor == 0x0102_0204 && word(&row.row, 0) == 1)
        );
    }

    #[test]
    #[ignore = "requires original v1.4 supplied save and native replacement fixture"]
    fn supplied_save_positive_refresh_preserves_native_handle_order() {
        compare_positive_refresh(false);
    }

    #[test]
    #[ignore = "requires original completed binding4450a0/485520 registry fixture"]
    fn completed_binding_refresh_preserves_native_cache_and_inputs() {
        compare_positive_refresh(true);
    }

    fn compare_positive_refresh(binding: bool) {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let saved = crate::save::SaveFile::parse(&save_bytes).unwrap();
        let buildings = saved.buildings().unwrap();
        let fixture = read(if binding {
            "OPENMT2001_NATIVE_COMPLETED_BINDING_APPEARANCE"
        } else {
            "OPENMT2001_NATIVE_SAVE_APPEARANCE_REPLACEMENTS"
        });
        assert_eq!(
            &fixture[..8],
            if binding { b"MTCAPHC1" } else { b"MTAPREP1" }
        );
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&fixture[8..40], sha);
        assert_eq!(word(&fixture, 40), 0x037f);
        assert_eq!(word(&fixture, 44), 15);
        let mut at = 48;
        let mut active_count = 0;
        for slot in 0..15 {
            assert_eq!(word(&fixture, at), slot);
            let descriptor = word(&fixture, at + 4);
            let count = word(&fixture, at + 8) as usize;
            at += 12;
            let building = buildings
                .iter()
                .find(|row| row.slot == slot as usize)
                .unwrap();
            let selected =
                crate::scene::SceneBuilding::from_saved(building, &board, &file).unwrap();
            let identity = model_identity(&file, selected.selected, building.height()).unwrap();
            assert_eq!(identity.descriptor, descriptor);
            assert_eq!(identity.renderer_templates.len(), count);
            let mut expected = Vec::new();
            for template in &identity.renderer_templates {
                assert_eq!(word(&fixture, at), u32::from(template.detail_slot));
                assert_eq!(word(&fixture, at + 4), template.descriptor);
                let before: [u8; 52] = fixture[at + 8..at + 60].try_into().unwrap();
                let after: [u8; 52] = fixture[at + 60..at + 112].try_into().unwrap();
                expected.push((before, after));
                at += 112;
            }
            let native_calls = if binding {
                let source: &[u8; 916] = fixture[at..at + 916].try_into().unwrap();
                at += 916;
                assert_eq!(&source[..64], building.bytes);
                assert_eq!(word(source, 0x4c), 1);
                assert_eq!(word(source, 0x50), identity.descriptor);
                assert!(crate::building_geometry::refresh_appearance_on_bind(source));
                let owner = word(&fixture, at);
                let business = word(&fixture, at + 4) as usize;
                let kind = word(&fixture, at + 8);
                let name_id = word(&fixture, at + 12);
                at += 16;
                let metadata = &fixture[at..at + 48];
                at += 48;
                assert_eq!(owner, u32::from(source[0x2d]));
                assert_eq!(business, word(source, 0x1c) as usize);
                assert_eq!(
                    kind,
                    saved
                        .economy()
                        .unwrap()
                        .businesses()
                        .nth(business)
                        .unwrap()
                        .business_type()
                );
                assert_eq!(
                    name_id,
                    word(&saved.business_catalog().unwrap()[kind as usize], 4)
                );
                if owner != 0 {
                    assert_eq!(
                        metadata,
                        saved.players().unwrap()[owner as usize - 1].metadata
                    );
                }
                let class = if owner == 0 { 10 } else { word(metadata, 0) };
                let count = word(&fixture, at) as usize;
                at += 4;
                let mut calls = Vec::new();
                for _ in 0..count {
                    let old = word(&fixture, at);
                    let args: [u32; 10] = std::array::from_fn(|i| word(&fixture, at + 4 + i * 4));
                    at += 44;
                    assert_eq!(args[3], class);
                    assert_eq!(args[7], class);
                    assert_eq!(args[6], kind);
                    if args[2] & 0x20000 != 0 {
                        assert_eq!(args[5], name_id);
                    } else if args[2] & 0x10000 != 0 && owner != 0 {
                        assert_eq!(args[4], 0x86ce78 + owner * 48);
                    }
                    calls.push((old, args[2]));
                }
                Some(calls)
            } else {
                None
            };
            let mut prepared_index = 0;
            let mut created = 0;
            let mut registry = RendererRegistry::prepare(&identity, |row| {
                while word(&expected[prepared_index].0, 0) != 1 {
                    prepared_index += 1;
                }
                let before = &expected[prepared_index].0;
                assert_eq!(word(before, 4), row.descriptor);
                assert_eq!(before[8..40], row.row[8..40]);
                prepared_index += 1;
                created += 1;
                Ok(word(before, 0x28))
            })
            .unwrap();
            assert_eq!(registry.rows.len(), count);
            for (row, (before, _)) in registry.rows.iter().zip(&expected) {
                assert_eq!(&row.row, before);
            }
            let calls = std::cell::RefCell::new(Vec::new());
            let mut next = 0;
            registry
                .refresh(
                    |old| {
                        calls.borrow_mut().push((0u8, old));
                        Ok(())
                    },
                    |row| {
                        calls.borrow_mut().push((1u8, row.descriptor));
                        while word(&expected[next].0, 0) != 1 {
                            next += 1;
                        }
                        let result = word(&expected[next].1, 0x28);
                        next += 1;
                        Ok(result)
                    },
                )
                .unwrap();
            let active = expected
                .iter()
                .filter(|(before, _)| word(before, 0) == 1)
                .count();
            if let Some(native) = native_calls {
                assert_eq!(native.len(), active);
                let actual = calls.borrow();
                for (index, (old, descriptor)) in native.into_iter().enumerate() {
                    assert_eq!(actual[index * 2], (0, old));
                    assert_eq!(actual[index * 2 + 1], (1, descriptor));
                }
            }
            assert_eq!(created, active);
            assert_eq!(calls.borrow().len(), active * 2);
            active_count += active;
            for (row, (_, after)) in registry.rows.iter().zip(&expected) {
                assert_eq!(&row.row, after);
            }
            let mut call_index = 0;
            for (template, (before, _)) in identity.renderer_templates.iter().zip(&expected) {
                if word(before, 0) != 1 {
                    continue;
                }
                assert_eq!(calls.borrow()[call_index], (0, word(before, 0x28)));
                assert_eq!(calls.borrow()[call_index + 1], (1, template.descriptor));
                call_index += 2;
            }
        }
        assert_eq!(active_count, 45);
        assert_eq!(at, fixture.len());
    }

    #[test]
    #[ignore = "requires original v1.4 supplied-save renderer registry fixture"]
    fn supplied_save_renderer_templates_match_native_registry() {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let saved = crate::save::SaveFile::parse(&save_bytes).unwrap();
        let buildings = saved.buildings().unwrap();
        let fixture = read("OPENMT2001_NATIVE_SAVE_APPEARANCE_REGISTRY");
        assert_eq!(&fixture[..8], b"MTAPREG1");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&fixture[8..40], sha);
        let at_word = |at: usize| word(&fixture, at);
        assert_eq!(at_word(40), 0x037f);
        assert_eq!(at_word(44), 15);
        let mut at = 48;
        let mut active = 0;
        for slot in 0..15 {
            assert_eq!(at_word(at), slot);
            let saved = buildings
                .iter()
                .find(|building| building.slot == slot as usize)
                .unwrap();
            let building = crate::scene::SceneBuilding::from_saved(saved, &board, &file).unwrap();
            let descriptor = model_identity(&file, building.selected, saved.height())
                .unwrap()
                .descriptor;
            assert_eq!(at_word(at + 4), descriptor);
            let count = at_word(at + 8) as usize;
            at += 12;
            let templates = renderer_templates(&file, building.selected, saved.height()).unwrap();
            assert_eq!(templates.len(), count);
            for template in templates {
                assert_eq!(at_word(at), template.detail_slot as u32);
                assert_eq!(at_word(at + 4), template.descriptor);
                assert_eq!(
                    &fixture[at + 8..at + 60],
                    template.row.as_slice(),
                    "building {slot} detail {}",
                    template.detail_slot
                );
                active += u32::from(word(&template.row, 0) == 1);
                at += 60;
            }
        }
        assert_eq!(active, 45);
        assert_eq!(at, fixture.len());
    }

    #[test]
    #[ignore = "requires original v1.4 BuildFile and native supplied-save appearance fixture"]
    fn supplied_save_renderer_candidates_match_native() {
        let file_bytes = std::fs::read(std::env::var("OPENMT2001_BUILD_FILE").unwrap()).unwrap();
        let file = BuildFile::parse(&file_bytes).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let save_bytes = std::fs::read(std::env::var("OPENMT2001_SAVE_ONE").unwrap()).unwrap();
        let saved = crate::save::SaveFile::parse(&save_bytes).unwrap();
        let buildings = saved.buildings().unwrap();
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_SAVE_APPEARANCE_COUNTS").unwrap())
                .unwrap();
        assert_eq!(&bytes[..8], b"MTAPCNT1");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let hash: Vec<u8> = pinned
            .as_bytes()
            .chunks_exact(2)
            .map(|part| u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], hash);
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(40), 0x037f);
        assert_eq!(word(44), 15);
        let mut at = 48;
        for slot in 0..15 {
            assert_eq!(word(at), slot);
            let descriptor = word(at + 4);
            assert_eq!(descriptor & 0x7f00_0000, 0);
            let saved = buildings
                .iter()
                .find(|building| building.slot == slot as usize)
                .unwrap();
            let selected = crate::scene::SceneBuilding::from_saved(saved, &board, &file)
                .unwrap()
                .selected;
            let identity = model_identity(&file, selected, saved.height()).unwrap();
            assert_eq!(
                identity.descriptor, descriptor,
                "saved building {slot} selected model and transpose bit"
            );
            let count = word(at + 8) as usize;
            at += 12;
            let actual = identity.renderer_candidates;
            assert_eq!(actual.len(), count, "saved building {slot}");
            for candidate in actual {
                assert_eq!(candidate.detail_slot as u32, word(at));
                assert_eq!(candidate.descriptor, word(at + 4));
                at += 8;
            }
        }
        assert_eq!(at, bytes.len());
    }
}
