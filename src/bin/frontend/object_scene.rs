//! Shared original cached-mesh preparation for moving scene objects.
use openmt2001::{
    mesh::CachedMesh,
    scene_assets::{self, SceneryRegion, SceneryTexture},
};

pub struct Part {
    pub page: usize,
    pub vertices: Vec<[f32; 8]>,
    pub normals: Vec<[f32; 3]>,
}

pub struct Model {
    pub parts: Vec<Part>,
}

impl Model {
    /// Register one static frame using the same compacted part and atlas UV
    /// path as retained residents. Locator meshes with no parts stay empty.
    pub fn from_mesh(
        source: &CachedMesh<'_>,
        textures: &[SceneryTexture<'_>],
        regions: &[SceneryRegion],
        untextured_page: Option<usize>,
    ) -> Result<Self, String> {
        Self::from_mesh_frame(source, textures, regions, untextured_page, 0)
    }

    /// The original construction scaffold meshes have 25 geometry frames.
    /// Other moving-object callers keep the original frame-zero behavior.
    pub fn from_mesh_frame(
        source: &CachedMesh<'_>,
        textures: &[SceneryTexture<'_>],
        regions: &[SceneryRegion],
        untextured_page: Option<usize>,
        frame: usize,
    ) -> Result<Self, String> {
        let untextured = source.texture_names.is_empty()
            && source.parts.iter().all(|part| part.texture == 0);
        if untextured && untextured_page.is_none() {
            return Err("moving-object mesh needs an untextured material page".into());
        }
        let material_regions = source
            .texture_names
            .iter()
            .map(|name| {
                let key = name.rsplit('\\').next().unwrap();
                textures
                    .iter()
                    .position(|texture| {
                        texture.name.as_ref().is_some_and(|name| {
                            name.as_bytes()
                                .get(..key.len())
                                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key.as_bytes()))
                        })
                    })
                    .map(|index| &regions[index])
                    .ok_or_else(|| format!("unresolved moving-object texture {key}"))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let uvs = if untextured {
            source.uvs()
        } else {
            scene_assets::scenery_mesh_uvs(
            source,
            &material_regions.iter().map(|r| r.uv).collect::<Vec<_>>(),
        )
        .map_err(|error| {
            format!(
                "{error}; {} names, part materials {:?}",
                source.texture_names.len(),
                source.parts.iter().map(|part| part.texture).collect::<Vec<_>>()
            )
        })?
        };
        let prepared = openmt2001::people::prepare_mesh(source, source)?;
        let positions = source.positions(frame)?;
        let mut parts = Vec::new();
        for part in prepared.parts {
            let material = usize::from(u16::from_le_bytes(
                part.material_header[4..6].try_into().unwrap(),
            ));
            let page = material_regions
                .get(material)
                .map(|region| region.page)
                .or_else(|| (untextured && material == 0).then_some(untextured_page).flatten())
                .ok_or("moving-object part material leaves registry")?;
            let mut vertices = Vec::with_capacity(part.triangles.len() * 3);
            let mut normals = Vec::with_capacity(part.triangles.len() * 3);
            for triangle in &part.triangles {
                for &vertex in triangle {
                    let vertex = vertex as usize;
                    let source_vertex = part.source_vertices[vertex];
                    let position = positions[source_vertex];
                    let uv = uvs[source_vertex];
                    vertices.push([
                        position[0],
                        position[1],
                        position[2],
                        1.,
                        1.,
                        1.,
                        uv[0],
                        uv[1],
                    ]);
                    normals.push(part.normals[vertex]);
                }
            }
            parts.push(Part {
                page,
                vertices,
                normals,
            });
        }
        Ok(Self { parts })
    }
}
