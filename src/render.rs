//! Shared GPU drawing. Host window creation, capture and file I/O live outside.
//! Prepared lighting and material state are supplied by the scene caller.
use wgpu::util::DeviceExt;
use std::{cell::RefCell, collections::VecDeque, hash::{Hash, Hasher}, sync::Arc};

// A bounded content cache also serves transient GUI scenes: their geometry can
// change each frame while original texture pages remain identical. Keep owned
// bytes and compare them after hashing, so recycled pointers/hash collisions
// cannot accidentally select another image.
const TEXTURE_CACHE_BYTES: usize = 64 * 1024 * 1024;
const TEXTURE_CACHE_ENTRIES: usize = 1024;
struct CachedTexture {
    key: (u32, u32, bool, u64),
    source_hint: usize,
    rgba: Arc<[u8]>,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    resident_bytes: usize,
}
#[derive(Default)]
struct TextureCache {
    entries: VecDeque<CachedTexture>,
    resident_bytes: usize,
}

// Original D3D8 sampling/blending uses normalized channel values without
// automatic sRGB transfer. Display gamma is a separate presentation concern.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Original linear table fog on a device advertising FOGTABLE and WFOG.
/// Distances use eye-space depth, not radial distance or depth-buffer values.
#[derive(Clone, Copy, Debug)]
pub struct EyeFog {
    pub color: u32,
    pub start: f32,
    pub end: f32,
}

/// Material state for one textured surface draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceMode {
    Diagnostic,
    /// Screen-space textured UI, diffuse alpha modulation and no depth test.
    Overlay,
    /// Native state1 screen UI: alpha modulation, depth writes and LESSEQUAL.
    UiDepth,
    /// Original untextured sky: state1, Z test enabled, Z writes disabled.
    Sky,
    /// Original star point list, alpha blended with depth disabled.
    Stars,
    /// Original 4ec120/4ec270 result particle points, expanded to camera
    /// billboards with ARGB modulation and no depth writes.
    ResultParticles,
    /// Original sun/moon textured quads, state1 with depth disabled.
    Celestial,
    /// Original state18, two cloud layers with additive blending and no culling.
    Clouds,
    /// Original state 1 with prepared diffuse and specular vertex colors.
    StaticLit,
    /// Opaque state-1 page pass, also the original state-2 facade fallback.
    StaticOpaque,
    /// Original state 6/7: stage 1 blends texture with current RGB using
    /// stage-0 alpha (texture alpha multiplied by prepared diffuse alpha).
    FacadeBlend,
    /// Original state 9 and constant diffuse 0xfffffffe used by 004c5510.
    WaterBase,
    /// Original fallback state20: multiply destination by textured vertex color.
    WaterFallback,
}

/// Vertices contain position xyz, diffuse RGB, then UV. Optional specular RGB
/// is added after texturing. Facade blend supplies its original diffuse alpha
/// and secondary coordinates explicitly; ordinary static draws are opaque in alpha.
pub struct TexturedDraw<'a> {
    /// Optional separate camera, used by the origin-centered sky draw.
    pub view_projection: Option<[f32; 16]>,
    pub vertices: &'a [[f32; 8]],
    pub specular: Option<&'a [[f32; 3]]>,
    pub diffuse_alpha: Option<&'a [u8]>,
    pub mode: SurfaceMode,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    /// Input is a resolved original power-of-two page; generate native mip levels.
    pub mipmaps: bool,
    pub secondary: Option<SecondaryTexture<'a>>,
}

pub struct SecondaryTexture<'a> {
    pub uv: &'a [[f32; 2]],
    pub diffuse_alpha: u8,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub mipmaps: bool,
}

/// GPU surfaces whose textures and bindings persist between frames.
#[derive(Default)]
pub struct PreparedScene {
    draws: Vec<GpuDraw>,
}

struct GpuDraw {
    camera: wgpu::Buffer,
    camera_offset: u64,
    mode: SurfaceMode,
    vertices: wgpu::Buffer,
    count: u32,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    secondary: Option<(wgpu::Texture, wgpu::TextureView)>,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    ui_depth_pipeline: wgpu::RenderPipeline,
    static_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    star_pipeline: wgpu::RenderPipeline,
    result_particle_pipeline: wgpu::RenderPipeline,
    celestial_pipeline: wgpu::RenderPipeline,
    cloud_pipeline: wgpu::RenderPipeline,
    static_opaque_pipeline: wgpu::RenderPipeline,
    facade_pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    water_fallback_pipeline: wgpu::RenderPipeline,
    texture_cache: RefCell<TextureCache>,
    surface_sampler: wgpu::Sampler,
    #[cfg(any(test, feature = "capture"))]
    bypass_texture_cache: std::cell::Cell<bool>,
}

impl Renderer {
    /// Compare original upload work with resident textures in capture benchmarks.
    #[cfg(feature = "capture")]
    #[doc(hidden)]
    pub fn bypass_texture_cache_for_capture(&self, bypass: bool) {
        self.bypass_texture_cache.set(bypass);
    }

    pub async fn new(descriptor: wgpu::InstanceDescriptor) -> Result<Self, String> {
        let instance = wgpu::Instance::new(descriptor);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("OpenMT2001"),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self::from_device(
            device,
            queue,
            format!("{} ({:?})", info.name, info.backend),
        ))
    }

    /// Use the host surface's device rather than creating another graphics context.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue, adapter_name: String) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("diagnostic surfaces"),
            source: wgpu::ShaderSource::Wgsl(include_str!("surface.wgsl").into()),
        });
        let make_pipeline = |mode: SurfaceMode| {
            let water = matches!(mode, SurfaceMode::WaterBase | SurfaceMode::WaterFallback);
            let static_lit = matches!(
                mode,
                SurfaceMode::Sky
                    | SurfaceMode::Celestial
                    | SurfaceMode::StaticLit
                    | SurfaceMode::StaticOpaque
                    | SurfaceMode::FacadeBlend
            );
            let blend = if matches!(
                mode,
                SurfaceMode::Diagnostic | SurfaceMode::StaticOpaque | SurfaceMode::FacadeBlend
            ) {
                None
            } else {
                let component = wgpu::BlendComponent {
                    src_factor: if mode == SurfaceMode::Clouds {
                        wgpu::BlendFactor::One
                    } else if mode != SurfaceMode::WaterFallback {
                        wgpu::BlendFactor::SrcAlpha
                    } else {
                        wgpu::BlendFactor::Zero
                    },
                    dst_factor: if mode == SurfaceMode::Clouds {
                        wgpu::BlendFactor::One
                    } else if mode != SurfaceMode::WaterFallback {
                        wgpu::BlendFactor::OneMinusSrcAlpha
                    } else {
                        wgpu::BlendFactor::Src
                    },
                    operation: wgpu::BlendOperation::Add,
                };
                Some(wgpu::BlendState {
                    color: component,
                    alpha: component,
                })
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene surfaces"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 56,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3,
                            1 => Float32x3,
                            2 => Float32x2,
                            3 => Float32x3,
                            4 => Float32x2,
                            5 => Float32
                        ],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: if mode == SurfaceMode::Stars {
                        wgpu::PrimitiveTopology::PointList
                    } else {
                        wgpu::PrimitiveTopology::TriangleList
                    },
                    // D3D CULL_CW discards clockwise screen triangles.
                    cull_mode: static_lit.then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!matches!(
                        mode,
                        SurfaceMode::WaterBase
                            | SurfaceMode::Sky
                            | SurfaceMode::Clouds
                            | SurfaceMode::Stars
                            | SurfaceMode::ResultParticles
                            | SurfaceMode::Celestial
                            | SurfaceMode::Overlay
                    )),
                    depth_compare: Some(
                        if water
                            || matches!(
                                mode,
                                SurfaceMode::Clouds
                                    | SurfaceMode::Stars
                                    | SurfaceMode::Celestial
                                    | SurfaceMode::Overlay
                            )
                        {
                            wgpu::CompareFunction::Always
                        } else if static_lit || mode == SurfaceMode::UiDepth {
                            wgpu::CompareFunction::LessEqual
                        } else {
                            wgpu::CompareFunction::Less
                        },
                    ),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(match mode {
                        SurfaceMode::WaterBase => "water_fragment",
                        SurfaceMode::WaterFallback => "water_fallback_fragment",
                        SurfaceMode::Diagnostic => "fragment_main",
                        SurfaceMode::Sky
                        | SurfaceMode::Celestial
                        | SurfaceMode::StaticLit
                        | SurfaceMode::StaticOpaque => "static_fragment",
                        SurfaceMode::FacadeBlend => "facade_fragment",
                        SurfaceMode::Clouds => "cloud_fragment",
                        SurfaceMode::Stars
                        | SurfaceMode::Overlay
                        | SurfaceMode::UiDepth
                        | SurfaceMode::ResultParticles => "star_fragment",
                    }),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let overlay_pipeline = make_pipeline(SurfaceMode::Overlay);
        let ui_depth_pipeline = make_pipeline(SurfaceMode::UiDepth);
        let pipeline = make_pipeline(SurfaceMode::Diagnostic);
        let water_pipeline = make_pipeline(SurfaceMode::WaterBase);
        let water_fallback_pipeline = make_pipeline(SurfaceMode::WaterFallback);
        let static_pipeline = make_pipeline(SurfaceMode::StaticLit);
        let sky_pipeline = make_pipeline(SurfaceMode::Sky);
        let star_pipeline = make_pipeline(SurfaceMode::Stars);
        let result_particle_pipeline = make_pipeline(SurfaceMode::ResultParticles);
        let celestial_pipeline = make_pipeline(SurfaceMode::Celestial);
        let cloud_pipeline = make_pipeline(SurfaceMode::Clouds);
        let static_opaque_pipeline = make_pipeline(SurfaceMode::StaticOpaque);
        let facade_pipeline = make_pipeline(SurfaceMode::FacadeBlend);
        let surface_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("repeat surface sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        Self {
            texture_cache: RefCell::default(),
            surface_sampler,
            #[cfg(any(test, feature = "capture"))]
            bypass_texture_cache: std::cell::Cell::new(false),
            overlay_pipeline,
            ui_depth_pipeline,
            sky_pipeline,
            star_pipeline,
            result_particle_pipeline,
            celestial_pipeline,
            cloud_pipeline,
            static_pipeline,
            static_opaque_pipeline,
            facade_pipeline,
            water_fallback_pipeline,
            water_pipeline,
            device,
            queue,
            pipeline,
            adapter_name,
        }
    }

    /// Vertices contain world position followed by diagnostic RGB color.
    /// A white 1x1 texture and zero UV preserve the solid-draw behavior.
    pub fn draw(
        &self,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        vertices: &[[f32; 6]],
        view_projection: [f32; 16],
    ) {
        let expanded: Vec<[f32; 8]> = vertices
            .iter()
            .map(|vertex| {
                [
                    vertex[0], vertex[1], vertex[2], vertex[3], vertex[4], vertex[5], 0., 0.,
                ]
            })
            .collect();
        // These constants are valid by construction, retaining the historical
        // infallible solid API while sharing the textured implementation.
        let white = [255, 255, 255, 255];
        let draw = TexturedDraw {
            view_projection: None,
            mode: SurfaceMode::Diagnostic,
            vertices: &expanded,
            specular: None,
            diffuse_alpha: None,
            width: 1,
            height: 1,
            rgba: &white,
            mipmaps: false,
            secondary: None,
        };
        self.draw_textured(target, depth, &[draw], view_projection, None)
            .expect("solid diagnostic draw is valid");
    }

    /// Draw surfaces in caller order using their selected recovered or diagnostic material.
    pub fn draw_textured(
        &self,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        draws: &[TexturedDraw<'_>],
        view_projection: [f32; 16],
        fog: Option<EyeFog>,
    ) -> Result<(), String> {
        let prepared = self.prepare(draws, view_projection, fog)?;
        self.draw_prepared(target, depth, &prepared);
        Ok(())
    }

    /// Upload textures and geometry once. Subsequent frames can update vertex
    /// values and camera buffers without recreating image resources or bindings.
    pub fn prepare(
        &self,
        draws: &[TexturedDraw<'_>],
        camera: [f32; 16],
        fog: Option<EyeFog>,
    ) -> Result<PreparedScene, String> {
        if draws.is_empty() { return Ok(PreparedScene::default()); }
        // Static offsets preserve independent per-draw camera semantics while
        // placing their uniforms in one upload allocation. No dynamic shader
        // binding or pipeline layout change is needed.
        let alignment = self.device.limits().min_uniform_buffer_offset_alignment.max(1) as usize;
        let stride = 96usize.div_ceil(alignment) * alignment;
        let bytes = draws.len().checked_mul(stride).ok_or("camera uniform allocation overflow")?;
        if bytes as u64 > self.device.limits().max_buffer_size {
            return Err("camera uniform allocation exceeds device limits".into());
        }
        let mut uniforms = vec![0u8; bytes];
        for (index, draw) in draws.iter().enumerate() {
            let values = camera_uniform(draw.view_projection.unwrap_or(camera), fog)?;
            uniforms[index * stride..index * stride + 96].copy_from_slice(bytemuck::cast_slice(&values));
        }
        let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("retained camera and fog slots"), contents: &uniforms,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let mut prepared = Vec::with_capacity(draws.len());
        for (index, draw) in draws.iter().enumerate() {
            prepared.push(self.upload_draw(draw, uniform.clone(), (index * stride) as u64)?);
        }
        Ok(PreparedScene { draws: prepared })
    }

    pub fn draw_prepared(
        &self,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        scene: &PreparedScene,
    ) {
        self.submit(target, depth, &scene.draws, false, false);
    }

    /// Refresh a GUI scene, retaining camera/vertex buffers and bindings for
    /// unchanged materials. Different textures or modes replace only that draw;
    /// a different draw count prepares the complete new layout.
    pub fn refresh_prepared(
        &self, scene: &mut PreparedScene, draws: &[TexturedDraw<'_>],
        camera: [f32; 16], fog: Option<EyeFog>,
    ) -> Result<(), String> {
        if scene.draws.len() != draws.len() {
            *scene = self.prepare(draws, camera, fog)?;
            return Ok(());
        }
        for (index, draw) in draws.iter().enumerate() {
            let primary = self.upload_texture(draw.width, draw.height, draw.rgba, draw.mipmaps)?;
            let secondary = draw.secondary.as_ref().map(|image|
                self.upload_texture(image.width, image.height, image.rgba, image.mipmaps)).transpose()?;
            let old = &scene.draws[index];
            if old.mode == draw.mode && old.texture == primary.0
                && old.secondary.as_ref().map(|image| &image.0)
                    == secondary.as_ref().map(|image| &image.0) {
                self.update_dynamic_surface(scene, index, draw, camera, fog)?;
            } else {
                let values = camera_uniform(draw.view_projection.unwrap_or(camera), fog)?;
                let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("retained camera and fog"), contents: bytemuck::cast_slice(&values),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
                scene.draws[index] = self.upload_draw(draw, uniform, 0)?;
            }
        }
        Ok(())
    }

    /// Draw world geometry after the base scene using its retained depth.
    /// Pavement lettering uses this so buildings and other nearer surfaces
    /// occlude it exactly as they do other ground geometry.
    pub fn draw_world_overlay(
        &self,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        scene: &PreparedScene,
    ) {
        self.submit(target, depth, &scene.draws, true, true);
    }

    /// Composite retained UI over the existing color attachment. This clears
    /// scene depth; draw any depth-tested world overlays first.
    pub fn draw_overlay(
        &self,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        scene: &PreparedScene,
    ) {
        self.submit(target, depth, &scene.draws, true, false);
    }

    /// Update a retained surface. Its image/material and maximum vertex count
    /// stay fixed; changing either requires preparing a new scene.
    pub fn update_surface(
        &self,
        scene: &mut PreparedScene,
        index: usize,
        draw: &TexturedDraw<'_>,
        camera: [f32; 16],
        fog: Option<EyeFog>,
    ) -> Result<(), String> {
        let surface = scene
            .draws
            .get_mut(index)
            .ok_or("missing retained surface")?;
        if surface.mode != draw.mode || draw.vertices.len() as u64 * 56 > surface.vertices.size() {
            return Err("retained surface layout changed".into());
        }
        let expanded = expand_vertices(draw)?;
        let values = camera_uniform(draw.view_projection.unwrap_or(camera), fog)?;
        self.queue
            .write_buffer(&surface.camera, surface.camera_offset, bytemuck::cast_slice(&values));
        if !expanded.is_empty() {
            self.queue
                .write_buffer(&surface.vertices, 0, bytemuck::cast_slice(&expanded));
        }
        surface.count = draw.vertices.len() as u32;
        Ok(())
    }

    /// Update vertices for an unchanged material, growing only its vertex buffer
    /// when camera-dependent sky or live actors need more geometry. Textures stay resident.
    pub fn update_dynamic_surface(
        &self,
        scene: &mut PreparedScene,
        index: usize,
        draw: &TexturedDraw<'_>,
        camera: [f32; 16],
        fog: Option<EyeFog>,
    ) -> Result<(), String> {
        let surface = scene
            .draws
            .get_mut(index)
            .ok_or("missing retained surface")?;
        if surface.mode != draw.mode {
            return Err("dynamic surface material changed".into());
        }
        let bytes = draw.vertices.len() as u64 * 56;
        if bytes > surface.vertices.size() {
            surface.vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("live population vertices"),
                size: bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.update_surface(scene, index, draw, camera, fog)
    }

    /// Update camera-dependent vertices before one batch camera upload. Image,
    /// material, and camera slots remain unchanged; only geometry may grow.
    pub fn update_dynamic_vertices(
        &self, scene: &mut PreparedScene, index: usize, draw: &TexturedDraw<'_>,
    ) -> Result<(), String> {
        let surface = scene.draws.get_mut(index).ok_or("missing retained surface")?;
        if surface.mode != draw.mode { return Err("dynamic surface material changed".into()); }
        let expanded = expand_vertices(draw)?;
        let bytes = draw.vertices.len() as u64 * 56;
        if bytes > surface.vertices.size() {
            surface.vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("live population vertices"), size: bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false,
            });
        }
        if !expanded.is_empty() {
            self.queue.write_buffer(&surface.vertices, 0, bytemuck::cast_slice(&expanded));
        }
        surface.count = draw.vertices.len() as u32;
        Ok(())
    }

    /// Copy camera-dependent geometry with one temporary upload allocation.
    /// Validate all inputs before changing retained buffers; camera/material
    /// resources remain untouched. Queue order keeps these copies before draws.
    pub fn update_dynamic_vertices_batch(
        &self, scene: &mut PreparedScene, updates: &[(usize, TexturedDraw<'_>)],
    ) -> Result<(), String> {
        let mut expanded = Vec::<[f32; 14]>::new();
        let mut copies = Vec::with_capacity(updates.len());
        for (index, draw) in updates {
            let surface = scene.draws.get(*index).ok_or("missing retained surface")?;
            if surface.mode != draw.mode { return Err("dynamic surface material changed".into()); }
            let offset = expanded.len() as u64 * 56;
            let vertices = expand_vertices(draw)?;
            let bytes = vertices.len() as u64 * 56;
            expanded.extend(vertices);
            copies.push((*index, draw.vertices.len() as u32, offset, bytes));
        }
        for &(index, count, _, bytes) in &copies {
            let surface = &mut scene.draws[index];
            if bytes > surface.vertices.size() {
                surface.vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("live population vertices"), size: bytes,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            surface.count = count;
        }
        if expanded.is_empty() { return Ok(()); }
        let upload = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("batched dynamic vertex upload"),
            contents: bytemuck::cast_slice(&expanded), usage: wgpu::BufferUsages::COPY_SRC,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("batched dynamic vertex copies"),
        });
        for &(index, _, offset, bytes) in &copies {
            if bytes != 0 {
                encoder.copy_buffer_to_buffer(&upload, offset, &scene.draws[index].vertices, 0, bytes);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        Ok(())
    }

    /// Upload independent per-draw camera/fog slots in one queue write. A GUI
    /// material refresh can introduce standalone buffers; retain its individual
    /// update behavior rather than assuming those buffers share this allocation.
    pub fn update_cameras(
        &self, scene: &PreparedScene, cameras: &[[f32; 16]], fog: Option<EyeFog>,
    ) -> Result<(), String> {
        if cameras.len() != scene.draws.len() { return Err("camera and surface counts differ".into()); }
        let Some(first) = scene.draws.first() else { return Ok(()); };
        if scene.draws.iter().any(|draw| draw.camera != first.camera) {
            for (index, &camera) in cameras.iter().enumerate() { self.update_camera(scene, index, camera, fog)?; }
            return Ok(());
        }
        let mut bytes = vec![0u8; first.camera.size() as usize];
        for (draw, &camera) in scene.draws.iter().zip(cameras) {
            let values = camera_uniform(camera, fog)?;
            let offset = draw.camera_offset as usize;
            bytes[offset..offset + 96].copy_from_slice(bytemuck::cast_slice(&values));
        }
        self.queue.write_buffer(&first.camera, 0, &bytes);
        Ok(())
    }

    pub fn update_camera(
        &self,
        scene: &PreparedScene,
        index: usize,
        camera: [f32; 16],
        fog: Option<EyeFog>,
    ) -> Result<(), String> {
        let surface = scene.draws.get(index).ok_or("missing retained surface")?;
        let values = camera_uniform(camera, fog)?;
        self.queue
            .write_buffer(&surface.camera, surface.camera_offset, bytemuck::cast_slice(&values));
        Ok(())
    }

    fn upload_draw(
        &self,
        draw: &TexturedDraw<'_>,
        camera: wgpu::Buffer,
        camera_offset: u64,
    ) -> Result<GpuDraw, String> {
        let expanded = expand_vertices(draw)?;
        let vertices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("textured scene vertices"),
                contents: bytemuck::cast_slice(&expanded),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        let (texture, texture_view) =
            self.upload_texture(draw.width, draw.height, draw.rgba, draw.mipmaps)?;
        let secondary = draw
            .secondary
            .as_ref()
            .map(|s| self.upload_texture(s.width, s.height, s.rgba, s.mipmaps))
            .transpose()?;
        let sampler = self.surface_sampler.clone();
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &camera, offset: camera_offset, size: std::num::NonZeroU64::new(96),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ];
        if let Some((_, view)) = &secondary {
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(view),
            });
        }
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("surface texture bindings"),
            layout: &self.pipeline_for(draw.mode).get_bind_group_layout(0),
            entries: &entries,
        });
        Ok(GpuDraw {
            camera,
            camera_offset,
            mode: draw.mode,
            vertices,
            count: draw.vertices.len() as u32,
            texture,
            texture_view,
            sampler,
            bind_group,
            secondary,
        })
    }

    fn upload_texture(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        mipmaps: bool,
    ) -> Result<(wgpu::Texture, wgpu::TextureView), String> {
        #[cfg(any(test, feature = "capture"))]
        if self.bypass_texture_cache.get() {
            return self.upload_texture_uncached(width, height, rgba, mipmaps);
        }
        let source_hint = rgba.as_ptr() as usize;
        let mut cache = self.texture_cache.borrow_mut();
        if let Some(index) = cache.entries.iter().position(|entry|
            entry.source_hint == source_hint
                && (entry.key.0, entry.key.1, entry.key.2) == (width, height, mipmaps)
                && entry.rgba.as_ref() == rgba) {
            let entry = cache.entries.remove(index).unwrap();
            let result = (entry.texture.clone(), entry.view.clone());
            cache.entries.push_back(entry);
            return Ok(result);
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        rgba.hash(&mut hash);
        let key = (width, height, mipmaps, hash.finish());
        if let Some(index) = cache.entries.iter().position(|entry|
            entry.key == key && entry.rgba.as_ref() == rgba) {
            let mut entry = cache.entries.remove(index).unwrap();
            entry.source_hint = source_hint;
            let result = (entry.texture.clone(), entry.view.clone());
            cache.entries.push_back(entry);
            return Ok(result);
        }
        let result = self.upload_texture_uncached(width, height, rgba, mipmaps)?;
        // CPU copy plus an upper bound for GPU image/mips. Very large textures
        // can still render, but do not evict the entire working set to cache one.
        let resident_bytes = rgba.len().saturating_mul(if mipmaps { 3 } else { 2 });
        if resident_bytes <= TEXTURE_CACHE_BYTES {
            while cache.resident_bytes + resident_bytes > TEXTURE_CACHE_BYTES
                || cache.entries.len() >= TEXTURE_CACHE_ENTRIES {
                let oldest = cache.entries.pop_front().unwrap();
                cache.resident_bytes -= oldest.resident_bytes;
            }
            cache.entries.push_back(CachedTexture {
                key, source_hint, rgba: Arc::from(rgba), texture: result.0.clone(),
                view: result.1.clone(), resident_bytes,
            });
            cache.resident_bytes += resident_bytes;
        }
        Ok(result)
    }

    fn upload_texture_uncached(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        mipmaps: bool,
    ) -> Result<(wgpu::Texture, wgpu::TextureView), String> {
        if width == 0 || height == 0 {
            return Err("textured draw dimensions must be nonzero".into());
        }
        if width > self.device.limits().max_texture_dimension_2d
            || height > self.device.limits().max_texture_dimension_2d
        {
            return Err("textured draw exceeds device texture dimensions".into());
        }
        let texel_count = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or("textured draw dimensions overflow")?;
        let expected = texel_count.checked_mul(4).ok_or("RGBA length overflow")?;
        let expected_len =
            usize::try_from(expected).map_err(|_| "RGBA length exceeds host size")?;
        let row_bytes = width.checked_mul(4).ok_or("texture row length overflow")?;
        if rgba.len() != expected_len {
            return Err(format!(
                "RGBA length {} does not match {}x{} texture ({expected})",
                rgba.len(),
                width,
                height
            ));
        }

        let reduced = if mipmaps {
            crate::texture::page_mipmaps(width, height, rgba)?
        } else {
            Vec::new()
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("surface texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1 + reduced.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        for (index, rgba) in reduced.iter().enumerate() {
            let level = index as u32 + 1;
            let (width, height) = (width >> level, height >> level);
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok((texture, texture_view))
    }

    fn pipeline_for(&self, mode: SurfaceMode) -> &wgpu::RenderPipeline {
        match mode {
            SurfaceMode::Diagnostic => &self.pipeline,
            SurfaceMode::Overlay => &self.overlay_pipeline,
            SurfaceMode::UiDepth => &self.ui_depth_pipeline,
            SurfaceMode::Sky => &self.sky_pipeline,
            SurfaceMode::Stars => &self.star_pipeline,
            SurfaceMode::ResultParticles => &self.result_particle_pipeline,
            SurfaceMode::Celestial => &self.celestial_pipeline,
            SurfaceMode::Clouds => &self.cloud_pipeline,
            SurfaceMode::StaticLit => &self.static_pipeline,
            SurfaceMode::StaticOpaque => &self.static_opaque_pipeline,
            SurfaceMode::FacadeBlend => &self.facade_pipeline,
            SurfaceMode::WaterBase => &self.water_pipeline,
            SurfaceMode::WaterFallback => &self.water_fallback_pipeline,
        }
    }

    fn submit(
        &self,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        draws: &[GpuDraw],
        overlay: bool,
        preserve_depth: bool,
    ) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if overlay {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.025,
                                g: 0.035,
                                b: 0.05,
                                a: 1.,
                            })
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if preserve_depth {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(1.)
                        },
                        store: if overlay && !preserve_depth {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            for draw in draws {
                if draw.count == 0 {
                    continue;
                }
                pass.set_pipeline(self.pipeline_for(draw.mode));
                // Keep texture, view, and sampler owned until submission; the
                // bind group references each resource for this pass.
                let _keep_alive = (
                    &draw.texture,
                    &draw.texture_view,
                    &draw.sampler,
                    &draw.secondary,
                );
                pass.set_bind_group(0, &draw.bind_group, &[]);
                pass.set_vertex_buffer(0, draw.vertices.slice(..));
                pass.draw(0..draw.count, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
    }
}

fn camera_uniform(camera: [f32; 16], fog: Option<EyeFog>) -> Result<[f32; 24], String> {
    if !camera.into_iter().all(f32::is_finite) {
        return Err("non-finite draw camera".into());
    }
    let mut values = [0.; 24];
    values[..16].copy_from_slice(&camera);
    if let Some(fog) = fog {
        if !fog.start.is_finite() || !fog.end.is_finite() || fog.end <= fog.start {
            return Err("fog requires finite, increasing distances".into());
        }
        values[16..20].copy_from_slice(&[
            ((fog.color >> 16) & 255) as f32 / 255.,
            ((fog.color >> 8) & 255) as f32 / 255.,
            (fog.color & 255) as f32 / 255.,
            1.,
        ]);
        values[20] = fog.start;
        values[21] = fog.end;
    }
    Ok(values)
}

fn expand_vertices(draw: &TexturedDraw<'_>) -> Result<Vec<[f32; 14]>, String> {
    if draw
        .specular
        .is_some_and(|s| s.len() != draw.vertices.len())
    {
        return Err("specular and vertex counts differ".into());
    }
    if matches!(draw.mode, SurfaceMode::FacadeBlend | SurfaceMode::Clouds)
        != draw.secondary.is_some()
    {
        return Err("two-layer material requires its secondary texture".into());
    }
    if draw
        .secondary
        .as_ref()
        .is_some_and(|s| s.uv.len() != draw.vertices.len())
    {
        return Err("secondary UV and vertex counts differ".into());
    }
    if draw
        .diffuse_alpha
        .is_some_and(|a| a.len() != draw.vertices.len())
    {
        return Err("diffuse alpha and vertex counts differ".into());
    }
    let expanded: Vec<[f32; 14]> = draw
        .vertices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let mut out = [0.; 14];
            out[..8].copy_from_slice(v);
            // Absence of a per-vertex override means opaque diffuse color.
            // Zero silently hides alpha-blended original material pages.
            out[13] = 1.;
            if let Some(specular) = draw.specular {
                out[8..11].copy_from_slice(&specular[i]);
            }
            if let Some(secondary) = &draw.secondary {
                out[11..13].copy_from_slice(&secondary.uv[i]);
                out[13] = f32::from(secondary.diffuse_alpha) / 255.;
            }
            if let Some(alpha) = draw.diffuse_alpha {
                out[13] = f32::from(alpha[i]) / 255.;
            }
            out
        })
        .collect();
    Ok(expanded)
}

#[cfg(all(test, feature = "capture"))]
mod depth_tests {
    use super::*;

    #[test]
    #[ignore = "requires GPU"]
    fn batched_dynamic_vertex_validation_and_growth() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let identity = [1.,0.,0.,0., 0.,1.,0.,0., 0.,0.,1.,0., 0.,0.,0.,1.];
        let triangle = [[-1.,-1.,0.5,1.,0.,0.,0.,0.],
            [1.,-1.,0.5,1.,0.,0.,1.,0.], [0.,1.,0.5,1.,0.,0.,0.5,1.]];
        fn make(vertices: &[[f32; 8]]) -> TexturedDraw<'_> {
            TexturedDraw { view_projection: None, vertices,
                specular: None, diffuse_alpha: None, mode: SurfaceMode::Diagnostic,
                width:1, height:1, rgba:&[255u8; 4], mipmaps:false, secondary:None }
        }
        let draws = [make(triangle.as_slice()), make(triangle.as_slice())];
        let mut scene = renderer.prepare(&draws, identity, None).unwrap();
        let cameras = scene.draws.iter().map(|draw| draw.camera.clone()).collect::<Vec<_>>();
        let larger = triangle.repeat(3);
        renderer.update_dynamic_vertices_batch(&mut scene,
            &[(0, make(&larger)), (1, make(&[]))]).unwrap();
        assert_eq!(scene.draws[0].count, 9);
        assert_eq!(scene.draws[1].count, 0);
        assert!(scene.draws[0].vertices.size() >= 9*56);
        for (draw, camera) in scene.draws.iter().zip(&cameras) {
            assert_eq!(&draw.camera, camera);
            assert_eq!(draw.mode, SurfaceMode::Diagnostic);
        }
        let retained = scene.draws[0].vertices.clone();
        assert!(renderer.update_dynamic_vertices_batch(&mut scene,
            &[(0, make(triangle.as_slice())), (99, make(&[]))]).is_err());
        assert_eq!(scene.draws[0].count, 9);
        assert_eq!(scene.draws[0].vertices, retained);
        let mut invalid = make(triangle.as_slice());
        invalid.specular = Some(&[]);
        assert!(renderer.update_dynamic_vertices_batch(&mut scene,
            &[(0, make(&[])), (1, invalid)]).is_err());
        assert_eq!(scene.draws[0].count, 9);
        assert_eq!(scene.draws[1].count, 0);
        let mut invalid_mode = make(&[]);
        invalid_mode.mode = SurfaceMode::StaticLit;
        assert!(renderer.update_dynamic_vertices_batch(&mut scene,
            &[(0, invalid_mode)]).is_err());
        renderer.update_dynamic_vertices_batch(&mut scene, &[]).unwrap();
        assert_eq!(scene.draws[0].count, 9);
        // Match sequential writes when a caller repeats an index: the final
        // prefix and count win, while the already-grown allocation is retained.
        renderer.update_dynamic_vertices_batch(&mut scene,
            &[(0, make(&larger)), (0, make(triangle.as_slice()))]).unwrap();
        assert_eq!(scene.draws[0].count, 3);
        assert_eq!(scene.draws[0].vertices, retained);
        renderer.queue.submit([]);
        renderer.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }

    #[test]
    fn texture_cache_reuses_exact_images_and_separates_layouts() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut image = vec![255; 4 * 4 * 4];
        renderer.upload_texture(4, 4, &image, false).unwrap();
        renderer.upload_texture(4, 4, &image.clone(), false).unwrap();
        renderer.upload_texture(4, 4, &image, false).unwrap();
        assert_eq!(renderer.texture_cache.borrow().entries.len(), 1);
        image[0] = 0;
        renderer.upload_texture(4, 4, &image, false).unwrap();
        renderer.upload_texture(4, 4, &image, true).unwrap();
        renderer.upload_texture(8, 2, &image, false).unwrap();
        let cache = renderer.texture_cache.borrow();
        assert_eq!(cache.entries.len(), 4);
        assert_eq!(cache.entries[0].rgba[0], 255);
        assert_eq!(cache.entries[2].texture.mip_level_count(), 3);
        assert_eq!(cache.entries[3].texture.width(), 8);
        assert!(cache.resident_bytes <= TEXTURE_CACHE_BYTES);
        drop(cache);
        assert!(renderer.upload_texture(3, 4, &image, false).is_err());
        assert_eq!(renderer.texture_cache.borrow().entries.len(), 4);
    }

    #[test]
    #[ignore = "requires supplied original city assets and offscreen GPU; release performance measurement"]
    fn original_city_texture_upload_performance() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let save = read("OPENMT2001_SAVE_ONE");
        let assemble = |phase, cache: Option<&mut crate::texture::LoaderCache>| {
            let request = crate::render_scene::Request {
            build_file: &build, source: crate::render_scene::Source::Save(&save),
            original: Some(crate::render_scene::OriginalAssets {
                executable: &executable, directory: &directory, archive: &archive,
            }), suncols: Some(&suncols), phase: Some(phase), camera: None,
            width: 1280, height: 960, animated: true,
            };
            if let Some(cache) = cache {
                crate::render_scene::assemble_with_style_cached(request, None, cache).unwrap()
            } else { crate::render_scene::assemble(request).unwrap() }
        };
        let mut converted = crate::texture::LoaderCache::default();
        let scene = assemble(9, Some(&mut converted));
        for phase in [10, 21] {
            let started = std::time::Instant::now();
            let plain = assemble(phase, None);
            let uncached = started.elapsed();
            let started = std::time::Instant::now();
            let cached = assemble(phase, Some(&mut converted));
            let elapsed = started.elapsed();
            assert_eq!(plain.draws.len(), cached.draws.len());
            for (a, b) in plain.draws.iter().zip(&cached.draws) {
                assert_eq!(a.mode, b.mode);
                assert_eq!(a.vertices, b.vertices);
                assert_eq!(a.specular, b.specular);
                assert_eq!(a.diffuse_alpha, b.diffuse_alpha);
                assert_eq!(a.rgba, b.rgba);
                assert_eq!((a.width, a.height, a.mipmaps), (b.width, b.height, b.mipmaps));
                if let (Some(a), Some(b)) = (&a.secondary, &b.secondary) {
                    assert_eq!(a.uv, b.uv);
                    assert_eq!(a.rgba, b.rgba);
                } else { assert_eq!(a.secondary.is_some(), b.secondary.is_some()); }
            }
            eprintln!("Original city hour {phase} assembly: uncached {uncached:?}, cached original conversions {elapsed:?}; all draw vertices/colors/specular/textures equal");
        }
        let started = std::time::Instant::now();
        for _ in 0..10 { std::hint::black_box(scene.clone()); }
        eprintln!("Original city full scene clone: {:?} per clone (10 repetitions)", started.elapsed() / 10);
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let draws = scene.draws.iter().map(crate::render_scene::OwnedDraw::borrowed)
            .collect::<Vec<_>>();
        let started = std::time::Instant::now();
        // Preserve original per-draw uploads for the baseline. Camera/vertex
        // buffers and bind groups take the identical path in both measurements.
        renderer.bypass_texture_cache.set(true);
        for _ in 0..3 { renderer.prepare(&draws, scene.camera, scene.fog).unwrap(); }
        let uncached = started.elapsed();
        renderer.bypass_texture_cache.set(false);
        renderer.prepare(&draws, scene.camera, scene.fog).unwrap();
        let started = std::time::Instant::now();
        for _ in 0..3 { renderer.prepare(&draws, scene.camera, scene.fog).unwrap(); }
        let cached = started.elapsed();
        eprintln!("Original city prepare: {} draws, 3 repetitions: cold per-draw {uncached:?}, resident {cached:?}, {} retained texture pages / {} bytes", draws.len(),
            renderer.texture_cache.borrow().entries.len(), renderer.texture_cache.borrow().resident_bytes);
    }

    #[test]
    fn world_overlay_retains_scene_occlusion() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let identity = [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ];
        let white = [255, 255, 255, 255];
        let triangle = |z: f32, color: [f32; 3]| {
            [
                [-1., -1., z, color[0], color[1], color[2], 0., 0.],
                [3., -1., z, color[0], color[1], color[2], 0., 0.],
                [-1., 3., z, color[0], color[1], color[2], 0., 0.],
            ]
        };
        let red = triangle(0.2, [1., 0., 0.]);
        let blue_far = triangle(0.8, [0., 0., 1.]);
        let blue_near = triangle(0.1, [0., 0., 1.]);
        let prepared = |vertices: &[[f32; 8]]| {
            renderer
                .prepare(
                    &[TexturedDraw {
                        view_projection: None,
                        vertices,
                        specular: None,
                        diffuse_alpha: None,
                        mode: SurfaceMode::Diagnostic,
                        width: 1,
                        height: 1,
                        rgba: &white,
                        mipmaps: false,
                        secondary: None,
                    }],
                    identity,
                    None,
                )
                .unwrap()
        };
        let base = prepared(&red);
        let far = prepared(&blue_far);
        let near = prepared(&blue_near);
        let pixel = |overlay: &PreparedScene, preserve: bool| {
            let size = wgpu::Extent3d {
                width: 8,
                height: 8,
                depth_or_array_layers: 1,
            };
            let texture = |format, usage| {
                renderer.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
            };
            let color = texture(
                COLOR_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            );
            let depth = texture(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
            let color_view = color.create_view(&Default::default());
            let depth_view = depth.create_view(&Default::default());
            renderer.draw_prepared(&color_view, &depth_view, &base);
            if preserve {
                renderer.draw_world_overlay(&color_view, &depth_view, overlay);
            } else {
                renderer.draw_overlay(&color_view, &depth_view, overlay);
            }
            let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 256 * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = renderer.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &color,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(8),
                    },
                },
                size,
            );
            renderer.queue.submit([encoder.finish()]);
            let (sender, receiver) = std::sync::mpsc::channel();
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    sender.send(result).unwrap()
                });
            renderer
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            receiver.recv().unwrap().unwrap();
            let bytes = buffer.slice(..).get_mapped_range().unwrap();
            let at = 4 * 256 + 4 * 4;
            [bytes[at], bytes[at + 1], bytes[at + 2]]
        };
        let hidden = pixel(&far, true);
        assert!(
            hidden[0] > hidden[2],
            "far world label covered red ground: {hidden:?}"
        );
        let visible = pixel(&near, true);
        assert!(
            visible[2] > visible[0],
            "near world label visible: {visible:?}"
        );
        let ui = pixel(&far, false);
        assert!(
            ui[2] > ui[0],
            "ordinary UI overlay still clears depth: {ui:?}"
        );
    }
}
