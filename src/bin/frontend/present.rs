use std::{
    cell::RefCell,
    sync::{Arc, atomic::{AtomicU64, Ordering}},
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
    pipeline: wgpu::RenderPipeline,
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Boot presentation"),
            source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Boot presentation"),
            layout: None,
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
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
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
            pipeline,
            cached: None,
            profile: std::env::var_os("OPENMT2001_PROFILE_PACE")
                .map(|_| RefCell::new(PresentProfile::default())),
        })
    }

    /// Queue completion is distinct from physical display scanout. Keep this
    /// diagnostic beside CPU timing so skipped presents and backlog are visible.
    pub fn report_presentation(&self) {
        let Some(profile) = &self.profile else { return; };
        let mut p = profile.borrow_mut();
        let completed = p.completion.completed.swap(0, Ordering::Relaxed);
        let submitted = p.completion.submitted.load(Ordering::Relaxed);
        let elapsed = p.completion.elapsed_us.swap(0, Ordering::Relaxed);
        let slowest = p.completion.slowest_us.swap(0, Ordering::Relaxed);
        eprintln!(
            "PRESENT queued={} skipped={} acquire_mean={:.2}ms acquire_max={:.2}ms gap_max={:.2}ms GPU_completed={} GPU_callback_mean={:.2}ms GPU_callback_max={:.2}ms pending={}",
            p.presented, p.skipped,
            p.acquire.as_secs_f64() * 1000. / f64::from(p.presented + p.skipped).max(1.),
            p.slowest_acquire.as_secs_f64() * 1000., p.longest_gap.as_secs_f64() * 1000.,
            completed, elapsed as f64 / completed.max(1) as f64 / 1000.,
            slowest as f64 / 1000., submitted,
        );
        if p.menu_updates != 0 {
            eprintln!("MENU CPU prepare_mean={:.2}ms submit_mean={:.2}ms",
                p.menu_prepare.as_secs_f64() * 1000. / f64::from(p.menu_updates),
                p.menu_submit.as_secs_f64() * 1000. / f64::from(p.menu_updates));
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
            let binding = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
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
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scene presentation"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
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
            if !matches!(&acquired, wgpu::CurrentSurfaceTexture::Success(_)
                | wgpu::CurrentSurfaceTexture::Suboptimal(_)) { p.skipped += 1; }
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
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
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
            let scale = (self.config.width as f32 / 4.).min(self.config.height as f32 / 3.);
            pass.set_viewport(
                (self.config.width as f32 - scale * 4.) / 2.,
                (self.config.height as f32 - scale * 3.) / 2.,
                scale * 4.,
                scale * 3.,
                0.,
                1.,
            );
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, binding, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
        if let Some(profile) = &self.profile {
            let mut p = profile.borrow_mut();
            let now = Instant::now();
            if let Some(last) = p.last_present { p.longest_gap = p.longest_gap.max(now - last); }
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
