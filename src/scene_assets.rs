//! Original startup registration needed to resolve literal fallback handle240.
//! This does not infer a texture from its appearance or substitute a placeholder.
use crate::{
    archive::ArchiveIndex, mesh::CachedMesh, panels::FontPanels, pe::PeImage, texture::TextureSet,
};

/// Index in BuildTextures for the missing-slot handle used by488050.
/// Its five fallback facade entries all use this same index, without offsets.
pub fn initial_facade_fallback(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &[u8],
    display_width: u32,
) -> Result<usize, String> {
    let panels = FontPanels::from_v14(image, index, archive, display_width)?;
    let mut count = panels.textures.len();
    let mut keys = panels.cache_keys;
    let path_at = |address| -> Result<&str, String> {
        let bytes = image.at(address, 260)?;
        let end = bytes
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated startup resource")?;
        std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid startup resource".into())
    };
    let payload = |path: &str| -> Result<&[u8], String> {
        let e = index
            .find(path)
            .ok_or_else(|| format!("missing startup resource {path}"))?;
        archive
            .get(e.offset as usize..(e.offset + e.length) as usize)
            .ok_or_else(|| "startup resource exceeds archive".into())
    };
    // Four auction sources at4bce24..4bce63 followed by thirty player models
    // at4bd333..4bd53b.4bcdef loads FontPanels before these calls.
    let paths = [
        0x546724, 0x5466f0, 0x5466c8, 0x5466ac, 0x546c8c, 0x546c60, 0x546c38, 0x546c10, 0x546be8,
        0x546bc0, 0x546b9c, 0x546b74, 0x546b48, 0x546b18, 0x546aec, 0x546abc, 0x546a90, 0x546a64,
        0x546a38, 0x546a08, 0x5469d8, 0x5469a8, 0x546978, 0x546948, 0x54691c, 0x5468ec, 0x5468c0,
        0x546894, 0x546868, 0x546838, 0x54680c, 0x5467e0, 0x5467b4, 0x546784,
    ];
    for address in paths {
        let mesh = CachedMesh::parse(payload(path_at(address)?)?)?;
        for name in mesh.texture_names {
            let key = name.rsplit('\\').next().unwrap();
            if keys.iter().any(|seen| {
                seen.as_bytes()
                    .get(..key.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key.as_bytes()))
            }) {
                continue;
            }
            // The original dog track contains this non-file material reference.
            //4ffb56..65 returns0 after failed source lookup; no registration.
            if key.eq_ignore_ascii_case("standing_poses_phong3") && index.find(key).is_none() {
                continue;
            }
            let textures = TextureSet::parse(payload(key)?).map_err(|e| e.to_string())?;
            if textures.textures.len() != 1 || key.to_ascii_lowercase().contains(".gtd") {
                return Err("unsupported startup mesh texture reference".into());
            }
            count += 1;
            keys.push(key.to_owned());
        }
    }
    //4c10f0 registers BuildingTextures, then WaterTextures, then BuildTextures.
    for address in [0x54ca14, 0x54c9ec] {
        count += TextureSet::parse(payload(path_at(address)?)?)
            .map_err(|e| e.to_string())?
            .textures
            .len();
    }
    let fallback = u32::from_le_bytes(image.at(0x4880d3, 4)?.try_into().unwrap()) as usize;
    let selected = fallback
        .checked_sub(count)
        .ok_or("fallback predates facade registry")?;
    let facades = TextureSet::parse(payload(path_at(0x54c9c4)?)?).map_err(|e| e.to_string())?;
    if selected >= facades.textures.len() {
        return Err("fallback exceeds facade registry".into());
    }
    Ok(selected)
}

/// Original registered source retained for the four groups containing shared
/// textures referenced by the static city scene. Cache lookup still visits all
/// preceding startup model calls, including models not rendered by this scene.
pub struct SceneryTexture<'a> {
    pub name: Option<String>,
    pub texture: crate::texture::TextureRecord<'a>,
    pub group: i32,
    pub padding: u8,
    pub dedicated: bool,
}

pub struct SceneryPage {
    pub side: u32,
    pub flags: u32,
    pub group: i32,
    pub rgba: Vec<u8>,
}

pub struct SceneryRegion {
    pub page: usize,
    pub uv: [f32; 4],
}

/// Original004ea510 rewrites indexed UVs from an unchanged source copy.
/// A later part may overwrite a vertex shared with an earlier material.
pub fn scenery_mesh_uvs(
    mesh: &CachedMesh<'_>,
    rectangles: &[[f32; 4]],
) -> Result<Vec<[f32; 2]>, String> {
    if rectangles.len() != mesh.texture_names.len() {
        return Err("scenery texture rectangle count differs from mesh".into());
    }
    let source = mesh.uvs();
    let mut result = source.clone();
    for part in &mesh.parts {
        let rectangle = rectangles
            .get(usize::from(part.texture))
            .ok_or("scenery material outside rectangle table")?;
        for i in part.vertex_indices() {
            result[i] = std::array::from_fn(|axis| {
                (f64::from(source[i][axis]) * f64::from(rectangle[axis])
                    + f64::from(rectangle[axis + 2])) as f32
            });
        }
    }
    Ok(result)
}

/// Mixed original004df1f0 page order. Dedicated pages are closed slots;
/// shared placement uses the existing256-page algorithm without treating
/// dedicated image dimensions as free space for later images.
pub fn scenery_pages(
    records: &[SceneryTexture<'_>],
) -> Result<(Vec<SceneryPage>, Vec<SceneryRegion>), String> {
    scenery_pages_with_loader(records, |image| crate::texture::loader_pixels(image).into())
}

pub(crate) fn scenery_pages_with_loader(
    records: &[SceneryTexture<'_>],
    mut pixels: impl FnMut(&crate::texture::TextureRecord<'_>) -> std::sync::Arc<[u8]>,
) -> Result<(Vec<SceneryPage>, Vec<SceneryRegion>), String> {
    let inputs: Vec<_> = records
        .iter()
        .map(|record| crate::atlas::Input {
            width: record.texture.width,
            height: record.texture.height,
            group: record.group,
            padding: record.padding,
        })
        .collect();
    let mut pages: Vec<SceneryPage> = Vec::new();
    let mut shared_pages = Vec::new();
    let mut layout = crate::atlas::Layout::default();
    let mut regions: Vec<_> = records
        .iter()
        .map(|_| SceneryRegion {
            page: 0,
            uv: [0.; 4],
        })
        .collect();
    for i in crate::atlas::Layout::packing_order(&inputs) {
        let record = &records[i];
        let (page, origin) = if record.dedicated {
            let side = record.texture.width.max(record.texture.height);
            if !side.is_power_of_two() || side > 256 {
                return Err("unsupported dedicated scenery page size".into());
            }
            let index = pages.len();
            pages.push(SceneryPage {
                side,
                flags: 0,
                //004de8b0 closes this page without assigning shared-group+d440.
                group: 0,
                rgba: vec![0; side as usize * side as usize * 4],
            });
            (index, [0, 0])
        } else {
            let region = layout.place(inputs[i])?;
            if region.page == shared_pages.len() {
                shared_pages.push(pages.len());
                pages.push(SceneryPage {
                    side: 256,
                    flags: 0,
                    group: record.group,
                    rgba: vec![0; 256 * 256 * 4],
                });
            }
            (shared_pages[region.page], region.origin)
        };
        let pixels = pixels(&record.texture);
        let mut loaded = record.texture.clone();
        loaded.header[0] = crate::texture::loader_flags(&loaded);
        loaded.texels = &pixels;
        let target = &mut pages[page];
        target.flags |= loaded.header[0] & 0xd000;
        let uv = crate::texture::write_atlas_region(
            &loaded,
            &mut target.rgba,
            target.side,
            origin,
            record.padding,
        )?;
        regions[i] = SceneryRegion { page, uv };
    }
    Ok((pages, regions))
}

pub fn scenery_textures<'a>(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &'a [u8],
    display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[1, 6, 12])
}

/// Original static building details also use the group-11 industrial TM
/// registrations. Retain those materials without changing the main scenery
/// collection used by other scene layers.
pub fn static_detail_textures<'a>(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &'a [u8],
    display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[1, 6, 11, 12])
}

/// Original complete resident texture group3, plus the panel cache prefix.
pub fn people_textures<'a>(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &'a [u8],
    display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[3])
}

/// Original road-model registration uses startup texture group 4.
pub fn vehicle_textures<'a>(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &'a [u8],
    display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[4])
}

/// Transport meshes register in group 10 but reuse earlier cached texture keys
/// from road, scenery and actor groups 4, 6 and 9.
pub fn transport_textures<'a>(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &'a [u8],
    display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[4, 6, 9, 10])
}

/// Original auction room group1 and player-character group2, in startup order.
pub fn auction_textures<'a>(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &'a [u8],
    display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[1, 2])
}

/// Result actors reuse the player group and Mr Monopoly from original group9.
pub fn result_textures<'a>(
    image: &PeImage<'_>, index: &ArchiveIndex, archive: &'a [u8], display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[1, 2, 9])
}

/// Original tutor meshes are registered in startup group 8 at 4bd04e.
pub fn tutor_textures<'a>(
    image: &PeImage<'_>, index: &ArchiveIndex, archive: &'a [u8], display_width: u32,
) -> Result<Vec<SceneryTexture<'a>>, String> {
    startup_textures(image, index, archive, display_width, &[8])
}

fn startup_textures<'a>(
    image: &PeImage<'_>,
    index: &ArchiveIndex,
    archive: &'a [u8],
    display_width: u32,
    groups: &[i32],
) -> Result<Vec<SceneryTexture<'a>>, String> {
    let panels = FontPanels::from_v14(image, index, archive, display_width)?;
    let mut keys = panels.cache_keys;
    let panel_collection_count = panels.billboards.end;
    let mut result: Vec<_> = panels
        .textures
        .into_iter()
        .enumerate()
        .map(|(i, texture)| SceneryTexture {
            name: i
                .checked_sub(panel_collection_count)
                .map(|i| keys[i].clone()),
            texture,
            group: 0,
            padding: 1,
            dedicated: false,
        })
        .collect();
    for &(group, padding, paths) in SCENERY_STARTUP_MESHES {
        for &address in paths {
            let path = image.at(address, 260)?;
            let end = path
                .iter()
                .position(|&b| b == 0)
                .ok_or("unterminated scenery path")?;
            let path = std::str::from_utf8(&path[..end]).map_err(|_| "invalid scenery path")?;
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing startup mesh {path}"))?;
            let data = archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("startup mesh outside archive")?;
            let mesh = CachedMesh::parse(data)?;
            for name in mesh.texture_names {
                let key = name.rsplit('\\').next().unwrap();
                if keys.iter().any(|seen| {
                    seen.as_bytes()
                        .get(..key.len())
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key.as_bytes()))
                }) {
                    continue;
                }
                if key.to_ascii_lowercase().contains(".gtd") {
                    return Err("unsupported scenery GTD model reference".into());
                }
                let Some(entry) = index.find(key) else {
                    // Original004ffb56..65 returns0 without registration.
                    if key.eq_ignore_ascii_case("standing_poses_phong3")
                        || key.eq_ignore_ascii_case("maya_models_white4by4.tt")
                    {
                        continue;
                    }
                    return Err(format!("missing startup texture {key}"));
                };
                let data = archive
                    .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                    .ok_or("startup texture outside archive")?;
                let mut set = TextureSet::parse(data).map_err(|e| e.to_string())?;
                if set.textures.len() != 1 {
                    return Err("unsupported scenery texture collection".into());
                }
                let texture = set.textures.remove(0);
                keys.push(key.to_owned());
                if groups.contains(&group) {
                    result.push(SceneryTexture {
                        name: Some(key.to_owned()),
                        dedicated: texture.header[0] & 2 != 0,
                        texture,
                        group,
                        padding,
                    });
                }
            }
        }
    }
    // Original004fed60 copies cache user-data at packed directory record+0x112.
    // Original004ffdcb..004ffdda merges only bit2 before004ffde0 selects a page.
    for record in &mut result {
        if let Some(name) = &record.name {
            let entry = index.find(name).ok_or("missing scenery cache metadata")?;
            let cache_flags = (entry.metadata[4] >> 16) | (entry.metadata[5] << 16);
            record.texture.header[0] |= cache_flags & 2;
            record.dedicated = record.texture.header[0] & 2 != 0;
        }
    }
    Ok(result)
}

// Ordered004fff00 path operands after FontPanels in004bcda0's startup caller.
// Entries retain original group and registration byte32 (padding). Repeated
// model calls are intentional; cache hits are resolved from original names.
// GTD-only registrations do not add TT cache keys. All TT registrations in
// groups1/6/12 are retained, including dedicated pages affecting packing order.
const SCENERY_STARTUP_MESHES: &[(i32, u8, &[u32])] = &[
    (1, 2, &[0x546724, 0x5466f0, 0x5466c8, 0x5466ac]),
    (
        2,
        2,
        &[
            0x546c8c, 0x546c60, 0x546c38, 0x546c10, 0x546be8, 0x546bc0, 0x546b9c, 0x546b74,
            0x546b48, 0x546b18, 0x546aec, 0x546abc, 0x546a90, 0x546a64, 0x546a38, 0x546a08,
            0x5469d8, 0x5469a8, 0x546978, 0x546948, 0x54691c, 0x5468ec, 0x5468c0, 0x546894,
            0x546868, 0x546838, 0x54680c, 0x5467e0, 0x5467b4, 0x546784,
        ],
    ),
    (
        3,
        2,
        &[
            0x549040, 0x549008, 0x548fd4, 0x548f9c, 0x548f68, 0x548f30, 0x548efc, 0x548ec4,
            0x548e84, 0x548e3c, 0x548dfc, 0x548db4, 0x548d74, 0x548d2c, 0x548cec, 0x548ca4,
            0x548c68, 0x548c28, 0x548bec, 0x548bac, 0x548b70, 0x548b30, 0x548af4, 0x548ab4,
            0x548a80, 0x548a48, 0x548a14, 0x5489dc, 0x5489a8, 0x548970, 0x54893c, 0x548904,
            0x5488d4, 0x5488a0, 0x548870, 0x54883c, 0x54880c, 0x5487d8, 0x5487a8, 0x548774,
            0x548730, 0x5486e4, 0x5486a0, 0x548654, 0x548610, 0x5485c4, 0x548580, 0x548534,
            0x5484f4, 0x5484b0, 0x548470, 0x54842c, 0x5483ec, 0x5483a8, 0x548368, 0x548324,
            0x5482ec, 0x5482b0, 0x548278, 0x54823c, 0x548204, 0x5481c8, 0x548190, 0x548154,
            0x548120, 0x5480dc, 0x54809c, 0x548064, 0x548120, 0x54801c, 0x547fd8, 0x547f9c,
        ],
    ),
    (
        4,
        2,
        &[
            0x54b470, 0x54b3f4, 0x54b3a4, 0x54b378, 0x54b34c, 0x54b320, 0x54b2f4, 0x54b2c8,
            0x54b29c, 0x54b26c, 0x54b23c, 0x54b20c, 0x54b1e0, 0x54b1b0, 0x54b180, 0x54b154,
            0x54b128, 0x54b0fc, 0x54b0d0, 0x54b0a4, 0x54b078, 0x54af80, 0x54aef8, 0x54ae9c,
            0x54ae6c, 0x54ae3c, 0x54ae0c, 0x54addc, 0x54adac, 0x54ad7c, 0x54ad48, 0x54ad14,
            0x54ace0, 0x54acac, 0x54ac78, 0x54ac44, 0x54ac14, 0x54abe0, 0x54abac, 0x54ab7c,
            0x54ab48, 0x54ab18, 0x54aa10, 0x54a994, 0x54a944, 0x54a918, 0x54a8ec, 0x54a8c0,
            0x54a894, 0x54a868, 0x54a83c, 0x54a80c, 0x54a7dc, 0x54a7ac, 0x54a77c, 0x54a74c,
            0x54a71c, 0x54a6f0, 0x54a6c4, 0x54a698, 0x54a66c, 0x54a640, 0x54a614, 0x54a51c,
            0x54a494, 0x54a438, 0x54a408, 0x54a3d8, 0x54a3a8, 0x54a378, 0x54a348, 0x54a318,
            0x54a2e4, 0x54a2b0, 0x54a27c, 0x54a248, 0x54a214, 0x54a1e0, 0x54a1ac, 0x54a17c,
            0x54a148, 0x54a118, 0x54a0e8, 0x54a0b8, 0x549fb0, 0x549f34, 0x549ee4, 0x549eb8,
            0x549e8c, 0x549e60, 0x549e34, 0x549e08, 0x549ddc, 0x549dac, 0x549d7c, 0x549d4c,
            0x549d20, 0x549cf0, 0x549cc0, 0x549c94, 0x549c68, 0x549c3c, 0x549c10, 0x549be4,
            0x549bb8, 0x549ac0, 0x549a38, 0x5499dc, 0x5499ac, 0x549978, 0x549948, 0x549914,
            0x5498e4, 0x5498b4, 0x549880, 0x54984c, 0x549818, 0x5497e4, 0x5497b0, 0x54977c,
            0x54974c, 0x549718, 0x5496e4, 0x5496b4, 0x549684, 0x549654,
        ],
    ),
    (
        6,
        2,
        &[
            0x547f64, 0x547f38, 0x547f0c, 0x547ee0, 0x547eb4, 0x547e88, 0x547e5c, 0x547e30,
            0x547e04, 0x547dd8, 0x547dac, 0x547d80, 0x547d54, 0x547d28, 0x547cfc, 0x547cd0,
            0x547ca4, 0x547c78, 0x547c4c, 0x547c20,
        ],
    ),
    (
        6,
        1,
        &[
            0x547be8, 0x547ba4, 0x547b60, 0x547b28, 0x547af0, 0x547aac, 0x547be8, 0x547a68,
            0x547a2c, 0x5479f0, 0x547af0, 0x5479b4, 0x54797c, 0x547954, 0x547928, 0x5478f8,
            0x5478c8, 0x547898, 0x5476dc, 0x547870, 0x547844, 0x547814, 0x5477e0, 0x5477c4,
        ],
    ),
    (6, 2, &[0x546678]),
    (
        6,
        1,
        &[
            0x546640, 0x546608, 0x5465d0, 0x546594, 0x546558, 0x54651c, 0x5464e0, 0x5464ac,
        ],
    ),
    (8, 2, &[0x546448, 0x546424, 0x546404]),
    (
        9,
        2,
        &[
            0x547778, 0x547740, 0x54770c, 0x5476dc, 0x5476a8, 0x547688, 0x547668, 0x547648,
            0x547628, 0x547604, 0x5475e4, 0x5475c8, 0x5475a0, 0x547578, 0x547550, 0x547528,
            0x5474fc, 0x5474cc, 0x5474a8,
        ],
    ),
    (
        10,
        2,
        &[
            0x54953c, 0x549520, 0x549504, 0x5494dc, 0x5494b8, 0x54949c, 0x54947c, 0x54945c,
            0x549438, 0x549418, 0x5493f8, 0x5493d8, 0x5493b0, 0x549384, 0x549358, 0x54932c,
            0x549304, 0x5492d8, 0x5492ac, 0x54927c, 0x54925c, 0x54923c, 0x54921c, 0x5491fc,
            0x5491d8, 0x5491b4, 0x549190, 0x54916c, 0x549148, 0x549124, 0x549100, 0x5490e0,
            0x5490b4, 0x549084,
        ],
    ),
    (
        11,
        2,
        &[
            0x546384, 0x54634c, 0x546318, 0x5462e4, 0x5462ac, 0x546274, 0x54623c, 0x546204,
        ],
    ),
    (
        12,
        2,
        &[
            0x54c834, 0x54c7f8, 0x54c7b8, 0x54c77c, 0x54c748, 0x54c704, 0x54c6c0, 0x54c67c,
            0x54c638, 0x54c5f4, 0x54c5b0, 0x54c570, 0x54c52c, 0x54c4e8, 0x54c4a4, 0x54c45c,
            0x54c414, 0x54c3cc, 0x54c388, 0x54c344, 0x54c300, 0x54c2c0, 0x54c27c, 0x54c234,
            0x54c1f8, 0x54c1bc, 0x54c190, 0x54c164, 0x54c138, 0x54c110, 0x54c0e8, 0x54c0c4,
            0x54c098, 0x54c05c, 0x54c020, 0x54bfd8, 0x54bf94, 0x54bf54, 0x54bf10, 0x54becc,
            0x54be84, 0x54be54, 0x54be10, 0x54bdcc, 0x54bd88, 0x54bd48, 0x54bd08, 0x54bcc8,
            0x54bc88, 0x54bc48, 0x54bc0c, 0x54bbcc, 0x54bb90, 0x54bb64, 0x54bb28, 0x54baf8,
            0x54bab8, 0x54ba78, 0x54ba38, 0x54b9f8, 0x54b9b8, 0x54b978, 0x54b93c, 0x54b900,
            0x54b8c4, 0x54b888, 0x54b84c, 0x54b810, 0x54b7e4, 0x54b7a0, 0x54b75c, 0x54b71c,
            0x54b6dc, 0x54b694, 0x54b668, 0x54b640, 0x54b614, 0x54b5e0, 0x54b5b4, 0x54b588,
            0x54b548, 0x54b50c, 0x54b4d4, 0x54b50c, 0x54b4d4,
        ],
    ),
    (12, 1, &[0x5461e4]),
];

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original scenery mesh UV observations and assets"]
    fn matches_native_scenery_mesh_uvs() {
        check_mesh_uvs(false, "OPENMT2001_NATIVE_SCENERY_MESH_UV", 57);
    }

    #[test]
    #[ignore = "requires original people atlas UV observations and assets"]
    fn matches_native_people_mesh_uvs() {
        check_mesh_uvs(true, "OPENMT2001_NATIVE_PEOPLE_UV", 64);
    }

    fn check_mesh_uvs(people: bool, fixture: &str, count: u32) {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = crate::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let loader = if people {
            super::people_textures
        } else {
            super::scenery_textures
        };
        let records = loader(
            &crate::pe::PeImage::parse(&exe).unwrap(),
            &index,
            &archive,
            1280,
        )
        .unwrap();
        let (_, regions) = super::scenery_pages(&records).unwrap();
        let bytes = read(fixture);
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSCUV01");
        assert_eq!(word(8), count);
        let mut at = 12;
        for _ in 0..word(8) {
            let name_len = word(at) as usize;
            let count = word(at + 4) as usize;
            at += 8;
            let name = std::str::from_utf8(&bytes[at..at + name_len]).unwrap();
            at += name_len;
            let entry = index.find(name).unwrap();
            let mesh = crate::mesh::CachedMesh::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap();
            let rectangles: Vec<_> = mesh
                .texture_names
                .iter()
                .map(|name| {
                    let key = name.rsplit('\\').next().unwrap();
                    records
                        .iter()
                        .position(|record| {
                            record.name.as_ref().is_some_and(|seen| {
                                seen.as_bytes().get(..key.len()).is_some_and(|prefix| {
                                    prefix.eq_ignore_ascii_case(key.as_bytes())
                                })
                            })
                        })
                        .map_or([1., 1., 0., 0.], |i| regions[i].uv)
                })
                .collect();
            let uvs = super::scenery_mesh_uvs(&mesh, &rectangles).unwrap();
            assert_eq!(uvs.len(), count);
            for uv in uvs {
                assert_eq!(uv.map(f32::to_bits), [word(at), word(at + 4)], "{name}");
                at += 8;
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original complete scenery group observations and assets"]
    fn matches_native_scenery_registration() {
        check_registration(
            false,
            "OPENMT2001_NATIVE_SCENERY_GROUPS",
            [246, 171, 0x027f],
        );
    }

    #[test]
    #[ignore = "requires original complete people group observations and assets"]
    fn matches_native_people_registration() {
        check_registration(true, "OPENMT2001_NATIVE_PEOPLE_PAGES", [211, 14, 0x037f]);
    }

    fn check_registration(people: bool, fixture: &str, expected: [u32; 3]) {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = crate::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let loader = if people {
            super::people_textures
        } else {
            super::scenery_textures
        };
        let records = loader(
            &crate::pe::PeImage::parse(&exe).unwrap(),
            &index,
            &archive,
            1280,
        )
        .unwrap();
        let bytes = read(fixture);
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSCGP01");
        assert_eq!([word(8), word(12), word(16)], expected);
        assert_eq!(records.len(), word(8) as usize);
        let (pages, regions) = super::scenery_pages(&records).unwrap();
        assert_eq!(pages.len(), word(12) as usize);
        let mut at = 20;
        for (record, region) in records.iter().zip(&regions) {
            let name_len = word(at) as usize;
            assert_eq!(u32::from(record.dedicated), word(at + 8));
            assert_eq!(record.group as u32, word(at + 12));
            assert_eq!(record.padding, bytes[at + 18]);
            at += 22;
            let name = std::str::from_utf8(&bytes[at..at + name_len]).unwrap();
            at += name_len;
            if let Some(actual_name) = &record.name {
                assert!(actual_name.eq_ignore_ascii_case(name));
            }
            assert_eq!(
                crate::texture::loader_flags(&record.texture),
                word(at),
                "{name}"
            );
            assert_eq!(
                (record.texture.width, record.texture.height),
                (word(at + 4), word(at + 8)),
                "{name}"
            );
            assert_eq!(region.page as u32, word(at + 12), "page for {name}");
            assert_eq!(
                region.uv.map(f32::to_bits),
                std::array::from_fn(|i| word(at + 24 + i * 4)),
                "UV for {name}"
            );
            at += 40;
            let pixels = crate::texture::loader_pixels(&record.texture);
            assert_eq!(pixels, bytes[at..at + pixels.len()], "{name}");
            at += pixels.len();
        }
        for (i, page) in pages.iter().enumerate() {
            assert_eq!(
                (page.flags, page.group as u32),
                (word(at), word(at + 4)),
                "page flags/group {i}"
            );
            assert_eq!((page.side, page.side), (word(at + 8), word(at + 12)));
            let mut levels = vec![page.rgba.clone()];
            levels.extend(crate::texture::page_mipmaps(page.side, page.side, &page.rgba).unwrap());
            assert_eq!(levels.len(), word(at + 16) as usize);
            at += 20;
            for level in levels {
                assert_eq!(level, bytes[at..at + level.len()], "page {i}");
                at += level.len();
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original startup registration fixture, image and archive"]
    fn matches_native_fallback_handle() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let dir = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let native = read("OPENMT2001_NATIVE_STARTUP_REGISTRY");
        assert_eq!(&native[..8], b"MTREG240");
        let index = crate::archive::ArchiveIndex::parse(&dir, archive.len() as u64).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let actual = super::initial_facade_fallback(&image, &index, &archive, 1280).unwrap();
        assert_eq!(
            actual as u32,
            u32::from_le_bytes(native[8..12].try_into().unwrap())
        );
    }
}
