use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use winit::window::Window;

#[derive(Default)]
struct CompletionProfile {
    submitted: AtomicU64,
    completed: AtomicU64,
    elapsed_us: AtomicU64,
    slowest_us: AtomicU64,
}
#[derive(Default)]
struct PresentProfile {
    presented: u32,
    skipped: u32,
    acquire: Duration,
    slowest_acquire: Duration,
    last_present: Option<Instant>,
    longest_gap: Duration,
    completion: Arc<CompletionProfile>,
    menu_updates: u32,
    menu_prepare: Duration,
    menu_submit: Duration,
}

pub struct Presenter {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    blit: Blit,
    cached: Option<(wgpu::Texture, wgpu::BindGroup, usize)>,
    profile: Option<RefCell<PresentProfile>>,
}
impl Presenter {
    pub async fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("OpenMT2001 native"),
                ..Default::default()
            })
            .await?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("No surface configuration")?;
        // Original image values are already display encoded; avoid a second sRGB transfer.
        config.format = surface
            .get_capabilities(&adapter)
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(config.format);
        // Use display-paced presentation explicitly. The adapter's default
        // Mailbox mode can discard completed frames and lets redraws outrun
        // the desktop. Allow CPU/GPU overlap; a requested latency of one can
        // serialize them and miss display intervals on this DX12 adapter.
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        let blit = Blit::new(&device, config.format);
        eprintln!(
            "Graphics: {} ({:?}); presentation {:?}, requested frame latency {}",
            adapter.get_info().name,
            adapter.get_info().backend,
            config.present_mode,
            config.desired_maximum_frame_latency,
        );
        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            blit,
            cached: None,
            profile: std::env::var_os("OPENMT2001_PROFILE_PACE")
                .map(|_| RefCell::new(PresentProfile::default())),
        })
    }

    /// Queue completion is distinct from physical display scanout. Keep this
    /// diagnostic beside CPU timing so skipped presents and backlog are visible.
    pub fn report_presentation(&self) {
        let Some(profile) = &self.profile else {
            return;
        };
        let mut p = profile.borrow_mut();
        let completed = p.completion.completed.swap(0, Ordering::Relaxed);
        let submitted = p.completion.submitted.load(Ordering::Relaxed);
        let elapsed = p.completion.elapsed_us.swap(0, Ordering::Relaxed);
        let slowest = p.completion.slowest_us.swap(0, Ordering::Relaxed);
        eprintln!(
            "PRESENT queued={} skipped={} acquire_mean={:.2}ms acquire_max={:.2}ms gap_max={:.2}ms GPU_completed={} GPU_callback_mean={:.2}ms GPU_callback_max={:.2}ms pending={}",
            p.presented,
            p.skipped,
            p.acquire.as_secs_f64() * 1000. / f64::from(p.presented + p.skipped).max(1.),
            p.slowest_acquire.as_secs_f64() * 1000.,
            p.longest_gap.as_secs_f64() * 1000.,
            completed,
            elapsed as f64 / completed.max(1) as f64 / 1000.,
            slowest as f64 / 1000.,
            submitted,
        );
        if p.menu_updates != 0 {
            eprintln!(
                "MENU CPU prepare_mean={:.2}ms submit_mean={:.2}ms",
                p.menu_prepare.as_secs_f64() * 1000. / f64::from(p.menu_updates),
                p.menu_submit.as_secs_f64() * 1000. / f64::from(p.menu_updates)
            );
        }
        p.presented = 0;
        p.skipped = 0;
        p.acquire = Duration::ZERO;
        p.slowest_acquire = Duration::ZERO;
        p.longest_gap = Duration::ZERO;
        p.menu_updates = 0;
        p.menu_prepare = Duration::ZERO;
        p.menu_submit = Duration::ZERO;
    }
    pub fn profile_started(&self) -> Option<Instant> {
        self.profile.as_ref().map(|_| Instant::now())
    }
    pub fn profile_menu_prepare(&self, started: Option<Instant>) -> Option<Instant> {
        if let (Some(profile), Some(started)) = (&self.profile, started) {
            let mut p = profile.borrow_mut();
            p.menu_prepare += started.elapsed();
            p.menu_updates += 1;
        }
        self.profile_started()
    }
    pub fn profile_menu_submit(&self, started: Option<Instant>) {
        if let (Some(profile), Some(started)) = (&self.profile, started) {
            profile.borrow_mut().menu_submit += started.elapsed();
        }
    }
    pub fn reset_presentation_profile(&self) {
        if let Some(profile) = &self.profile {
            *profile.borrow_mut() = PresentProfile::default();
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
    pub fn draw(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self
            .cached
            .as_ref()
            .is_none_or(|(texture, _, _)| texture.width() != width || texture.height() != height)
        {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Original splash pixels"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: if self.config.format.is_srgb() {
                    wgpu::TextureFormat::Rgba8UnormSrgb
                } else {
                    wgpu::TextureFormat::Rgba8Unorm
                },
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let binding = self.blit.binding(&self.device, &view, &sampler);
            self.cached = Some((texture, binding, 0));
        }
        let (texture, binding, last_pixels) = self.cached.as_mut().unwrap();
        if *last_pixels != rgba.as_ptr() as usize {
            self.queue.write_texture(
                texture.as_image_copy(),
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                texture.size(),
            );
            *last_pixels = rgba.as_ptr() as usize;
        }
        let binding = binding.clone();
        self.present_binding(&binding)
    }

    pub fn scene_view_format(&self) -> wgpu::TextureFormat {
        if self.config.format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        }
    }

    pub fn image_binding(&self, view: &wgpu::TextureView) -> wgpu::BindGroup {
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        self.blit.binding(&self.device, view, &sampler)
    }

    /// Repaint `binding` exactly as the window shows it, at the surface's
    /// pixel size with the same letterboxing, and read it back as top-down
    /// RGBA rows.
    pub fn capture(&self, binding: &wgpu::BindGroup) -> Result<Capture, String> {
        self.blit.capture(
            &self.device,
            &self.queue,
            binding,
            self.config.width,
            self.config.height,
        )
    }

    pub fn present_binding(
        &self,
        binding: &wgpu::BindGroup,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.window.inner_size().width == 0 || self.window.inner_size().height == 0 {
            return Ok(());
        }
        let acquire_started = self.profile.as_ref().map(|_| Instant::now());
        let acquired = self.surface.get_current_texture();
        if let (Some(profile), Some(started)) = (&self.profile, acquire_started) {
            let mut p = profile.borrow_mut();
            let elapsed = started.elapsed();
            p.acquire += elapsed;
            p.slowest_acquire = p.slowest_acquire.max(elapsed);
            if !matches!(
                &acquired,
                wgpu::CurrentSurfaceTexture::Success(_)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(_)
            ) {
                p.skipped += 1;
            }
        }
        let frame = match acquired {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            other => return Err(format!("Surface acquisition failed: {other:?}").into()),
        };
        let target = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.blit.encode(
            &mut encoder,
            &self.blit.surface,
            &target,
            binding,
            self.config.width,
            self.config.height,
        );
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
        if let Some(profile) = &self.profile {
            let mut p = profile.borrow_mut();
            let now = Instant::now();
            if let Some(last) = p.last_present {
                p.longest_gap = p.longest_gap.max(now - last);
            }
            p.last_present = Some(now);
            p.presented += 1;
            let completion = p.completion.clone();
            completion.submitted.fetch_add(1, Ordering::Relaxed);
            self.queue.on_submitted_work_done(move || {
                let elapsed = now.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
                completion.elapsed_us.fetch_add(elapsed, Ordering::Relaxed);
                completion.slowest_us.fetch_max(elapsed, Ordering::Relaxed);
                completion.completed.fetch_add(1, Ordering::Relaxed);
                completion.submitted.fetch_sub(1, Ordering::Relaxed);
            });
        }
        Ok(())
    }
}

/// Where a 4:3 frame sits on a `width` x `height` target: the largest
/// centred 4:3 rectangle, as `[x, y, width, height]`. The rest stays black.
pub fn letterbox(width: u32, height: u32) -> [f32; 4] {
    let (width, height) = (width as f32, height as f32);
    let scale = (width / 4.).min(height / 3.);
    [
        (width - scale * 4.) / 2.,
        (height - scale * 3.) / 2.,
        scale * 4.,
        scale * 3.,
    ]
}

/// A presented frame read back from the GPU.
pub struct Capture {
    pub width: u32,
    pub height: u32,
    /// Tightly packed top-down RGBA rows.
    pub rgba: Vec<u8>,
}

/// The full-screen pass that scales a frame onto the window, plus an
/// identical pass into an offscreen RGBA target for screenshots. Both
/// pipelines share one bind group layout, so a frame binding works with
/// either.
pub struct Blit {
    layout: wgpu::BindGroupLayout,
    surface: wgpu::RenderPipeline,
    capture: wgpu::RenderPipeline,
    capture_format: wgpu::TextureFormat,
}
impl Blit {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Boot presentation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Boot presentation"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Boot presentation"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |format| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Boot presentation"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fragment"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // Match the surface's transfer: an sRGB surface encodes on write, so
        // the screenshot target must too for its bytes to match the screen.
        let capture_format = if surface_format.is_srgb() {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        Self {
            surface: pipeline(surface_format),
            capture: pipeline(capture_format),
            capture_format,
            layout,
        }
    }

    pub fn binding(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scene presentation"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Clear `target` to black and draw the frame letterboxed into it.
    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        target: &wgpu::TextureView,
        binding: &wgpu::BindGroup,
        width: u32,
        height: u32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let [x, y, w, h] = letterbox(width, height);
        pass.set_viewport(x, y, w, h, 0., 1.);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, binding, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Draw `binding` as the window would at `width` x `height` into an
    /// offscreen target and read it back.
    pub fn capture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        binding: &wgpu::BindGroup,
        width: u32,
        height: u32,
    ) -> Result<Capture, String> {
        if width == 0 || height == 0 {
            return Err(format!("display is {width}x{height}"));
        }
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let limit = device.limits().max_texture_dimension_2d;
        if width > limit || height > limit {
            return Err(format!(
                "display {width}x{height} exceeds the {limit} texture limit"
            ));
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Screenshot"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.capture_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row = width * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screenshot readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        let view = texture.create_view(&Default::default());
        self.encode(&mut encoder, &self.capture, &view, binding, width, height);
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        receiver
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let mut rgba = Vec::with_capacity(row as usize * height as usize);
        for line in mapped.chunks(padded as usize) {
            rgba.extend_from_slice(&line[..row as usize]);
        }
        Ok(Capture {
            width,
            height,
            rgba,
        })
    }
}
