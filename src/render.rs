use std::borrow::Cow;
use std::sync::Arc;

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Wrap,
};
use wgpu::util::DeviceExt;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use crate::layout::{self, CardGeom, PaintRect, TextRun, BG};
use crate::savers::SAVERS;

const MAX_LONG: f32 = 1600.0;
const PREVIEW_LONG: f32 = 320.0;
const PAD: f32 = 28.0;
const PREVIEW_BUDGET: usize = 2;
const SLOTS: u32 = 11;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    resolution: [f32; 2],
    time: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Screen {
    size: [f32; 2],
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    local: [f32; 2],
    size: [f32; 2],
    radius: f32,
    _pad: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex {
    pos: [f32; 2],
    color: [f32; 4],
    rect: [f32; 4],
    params: [f32; 2],
    _pad: [f32; 2],
}

#[derive(Clone, Copy)]
enum Blit {
    Preview(usize),
    Saver,
}

struct Image {
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TextKey {
    Gallery(u32, u32, u32, u32),
    Hud(usize, u32, u32, u32),
}

pub enum Scene {
    Gallery { hover: Option<usize>, scroll: f32 },
    Saver { index: usize, hud_opacity: f32 },
}

pub struct Renderer {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    pub adapter_name: String,

    pipelines: Vec<wgpu::RenderPipeline>,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    align: u64,

    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    quad_buf: wgpu::Buffer,

    rect_pipeline: wgpu::RenderPipeline,
    rect_buf: wgpu::Buffer,
    screen_buf: wgpu::Buffer,
    screen_bind: wgpu::BindGroup,

    previews: Vec<Option<Image>>,
    preview_cursor: usize,
    saver_image: Option<Image>,

    font_system: FontSystem,
    swash_cache: SwashCache,
    // Holds the shared glyphon pipeline cache for the life of the renderer.
    #[allow(dead_code)]
    cache: Cache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    text_buffers: Vec<Buffer>,
    text_runs: Vec<TextRun>,
    text_key: Option<TextKey>,
}

impl Renderer {
    pub fn new(window: Arc<Window>, event_loop: &ActiveEventLoop) -> Result<Self, String> {
        pollster::block_on(Self::create(window, event_loop))
    }

    async fn create(window: Arc<Window>, event_loop: &ActiveEventLoop) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(event_loop.owned_display_handle()),
        ));
        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| err.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                apply_limit_buckets: false,
            })
            .await
            .map_err(|err| err.to_string())?;
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|err| err.to_string())?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = pick_format(&caps);
        let present = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };
        let alpha = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: present,
            alpha_mode: alpha,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);

        let align = device.limits().min_uniform_buffer_offset_alignment.max(16) as u64;
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: align * SLOTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(std::num::NonZeroU64::new(16).unwrap()),
                },
                count: None,
            }],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &globals,
                    offset: 0,
                    size: Some(std::num::NonZeroU64::new(16).unwrap()),
                }),
            }],
        });
        let saver_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("saver"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let errors = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let fullscreen = shader(
            &device,
            "fullscreen",
            wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/fullscreen.wgsl"))),
        );
        let mut pipelines = Vec::with_capacity(SAVERS.len());
        for saver in SAVERS {
            let module = shader(
                &device,
                saver.id,
                wgpu::ShaderSource::Glsl {
                    shader: Cow::Borrowed(saver.source),
                    stage: wgpu::naga::ShaderStage::Fragment,
                    defines: &[],
                },
            );
            pipelines.push(render_pipeline(
                &device,
                &saver_layout,
                &fullscreen,
                "vs_main",
                &module,
                "main",
                wgpu::TextureFormat::Rgba8Unorm,
                &[],
                None,
            ));
        }

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
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
        let screen_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("screen"),
            contents: bytemuck::bytes_of(&Screen {
                size: [size.width as f32, size.height as f32],
                _pad: [0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let screen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("screen"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(std::num::NonZeroU64::new(16).unwrap()),
                },
                count: None,
            }],
        });
        let screen_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("screen"),
            layout: &screen_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buf.as_entire_binding(),
            }],
        });
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&blit_layout), Some(&screen_layout)],
            immediate_size: 0,
        });
        let quad_module = shader(
            &device,
            "quad",
            wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/quad.wgsl"))),
        );
        let blit_pipeline = render_pipeline(
            &device,
            &blit_pipeline_layout,
            &quad_module,
            "vs",
            &quad_module,
            "fs",
            format,
            &quad_layout(),
            None,
        );
        let rect_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect"),
            bind_group_layouts: &[Some(&screen_layout)],
            immediate_size: 0,
        });
        let rect_module = shader(
            &device,
            "rect",
            wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/rect.wgsl"))),
        );
        let rect_pipeline = render_pipeline(
            &device,
            &rect_layout,
            &rect_module,
            "vs",
            &rect_module,
            "fs",
            format,
            &rect_layout_attrs(),
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );

        let quad_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quads"),
            size: (std::mem::size_of::<QuadVertex>() * 6 * 16) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let rect_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rects"),
            size: (std::mem::size_of::<RectVertex>() * 6 * 64) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut font_system = FontSystem::new();
        font_system.db_mut().set_sans_serif_family("Adwaita Sans");
        font_system.db_mut().set_serif_family("C059");
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::new(&device, &queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);

        let _ = device.poll(wgpu::PollType::Poll);
        if let Some(err) = errors.pop().await {
            return Err(format!("gpu: {err}"));
        }

        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            window,
            adapter_name: format!("{} ({:?})", info.name, info.backend),
            pipelines,
            globals,
            globals_bind,
            align,
            blit_pipeline,
            blit_layout,
            sampler,
            quad_buf,
            rect_pipeline,
            rect_buf,
            screen_buf,
            screen_bind,
            previews: (0..SAVERS.len()).map(|_| None).collect(),
            preview_cursor: 0,
            saver_image: None,
            font_system,
            swash_cache,
            cache,
            viewport,
            atlas,
            text_renderer,
            text_buffers: Vec::new(),
            text_runs: Vec::new(),
            text_key: None,
        })
    }

    pub fn draw(&mut self, logical: [f32; 2], scale: f32, time: f32, scene: Scene) {
        let physical = self.window.inner_size();
        if physical.width == 0 || physical.height == 0 || scale <= 0.0 {
            return;
        }
        if physical.width != self.config.width || physical.height != self.config.height {
            self.config.width = physical.width;
            self.config.height = physical.height;
            self.surface.configure(&self.device, &self.config);
            self.text_key = None;
        }

        self.queue.write_buffer(
            &self.screen_buf,
            0,
            bytemuck::bytes_of(&Screen {
                size: [physical.width as f32, physical.height as f32],
                _pad: [0.0, 0.0],
            }),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });

        let mut quads: Vec<(Blit, Vec<QuadVertex>)> = Vec::new();
        let mut rects: Vec<RectVertex> = Vec::new();
        let mut runs: Vec<TextRun> = Vec::new();
        let mut text_key = None;
        let mut text_opacity = 1.0;

        match scene {
            Scene::Gallery { hover, scroll } => {
                let layout = layout::gallery(logical[0], logical[1], scroll);
                self.update_previews(&mut encoder, &layout.cards, scale, time);
                push_rects(&mut rects, &layout.rects, scale);
                let paints = layout::card_paints(&layout.cards, hover);
                push_rects(
                    &mut rects,
                    &paints
                        .iter()
                        .copied()
                        .filter(|rect| rect.mode < 0.5)
                        .collect::<Vec<_>>(),
                    scale,
                );
                for card in &layout.cards {
                    if self.previews[card.index].is_some() {
                        quads.push((Blit::Preview(card.index), card_quad(card, scale)));
                    }
                }
                let solid_count = rects.len();
                push_rects(
                    &mut rects,
                    &paints
                        .iter()
                        .copied()
                        .filter(|rect| rect.mode >= 0.5)
                        .collect::<Vec<_>>(),
                    scale,
                );
                runs = layout.texts;
                text_key = Some(TextKey::Gallery(
                    quant(logical[0]),
                    quant(logical[1]),
                    quant(scale * 1000.0),
                    quant(layout.scroll),
                ));
                self.paint(
                    encoder,
                    rects,
                    solid_count,
                    quads,
                    &runs,
                    text_key,
                    text_opacity,
                    scale,
                );
            }
            Scene::Saver { index, hud_opacity } => {
                let (width, height) = saver_target(logical[0], logical[1]);
                let stale = self
                    .saver_image
                    .as_ref()
                    .map(|image| image.width != width || image.height != height)
                    .unwrap_or(true);
                if stale {
                    self.saver_image = Some(self.make_image(width, height));
                }
                let view = self.saver_image.as_ref().unwrap().view.clone();
                self.render_shader(&mut encoder, index, 10, &view, width, height, time);
                quads.push((Blit::Saver, saver_quad(logical, scale, time)));
                if hud_opacity > 0.01 {
                    runs = layout::hud(index, logical[0], logical[1]);
                    text_key = Some(TextKey::Hud(
                        index,
                        quant(logical[0]),
                        quant(logical[1]),
                        quant(scale * 1000.0),
                    ));
                    text_opacity = hud_opacity;
                }
                self.paint(
                    encoder,
                    rects,
                    0,
                    quads,
                    &runs,
                    text_key,
                    text_opacity,
                    scale,
                );
            }
        }
    }

    fn paint(
        &mut self,
        mut encoder: wgpu::CommandEncoder,
        rects: Vec<RectVertex>,
        gradient_at: usize,
        quads: Vec<(Blit, Vec<QuadVertex>)>,
        runs: &[TextRun],
        text_key: Option<TextKey>,
        text_opacity: f32,
        scale: f32,
    ) {
        if let Some(key) = text_key {
            self.sync_text(key, runs, scale);
        }
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.queue.submit(Some(encoder.finish()));
                let _ = self.device.poll(wgpu::PollType::Poll);
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                self.queue.submit(Some(encoder.finish()));
                let _ = self.device.poll(wgpu::PollType::Poll);
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .expect("recreate surface");
                self.surface.configure(&self.device, &self.config);
                self.queue.submit(Some(encoder.finish()));
                let _ = self.device.poll(wgpu::PollType::Poll);
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                eprintln!("swapchain validation error");
                return;
            }
        };

        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.config.width,
                height: self.config.height,
            },
        );
        if text_key.is_some() && text_opacity > 0.01 {
            self.prepare_text(text_opacity);
        }

        if !rects.is_empty() {
            self.queue
                .write_buffer(&self.rect_buf, 0, bytemuck::cast_slice(&rects));
        }
        let mut packed = Vec::new();
        let mut ranges = Vec::new();
        for (source, verts) in &quads {
            let start = packed.len() as u32;
            packed.extend_from_slice(verts);
            ranges.push((*source, start, packed.len() as u32));
        }
        if !packed.is_empty() {
            self.queue
                .write_buffer(&self.quad_buf, 0, bytemuck::cast_slice(&packed));
        }

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("window"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(gpu_color(BG)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !rects.is_empty() {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_bind_group(0, &self.screen_bind, &[]);
                pass.set_vertex_buffer(0, self.rect_buf.slice(..));
                let solid = gradient_at.min(rects.len()) as u32;
                if solid > 0 {
                    pass.draw(0..solid, 0..1);
                }
            }

            if !ranges.is_empty() {
                pass.set_pipeline(&self.blit_pipeline);
                pass.set_bind_group(1, &self.screen_bind, &[]);
                pass.set_vertex_buffer(0, self.quad_buf.slice(..));
                for (source, start, end) in ranges {
                    let bind = match source {
                        Blit::Preview(index) => &self.previews[index].as_ref().unwrap().bind,
                        Blit::Saver => &self.saver_image.as_ref().unwrap().bind,
                    };
                    pass.set_bind_group(0, bind, &[]);
                    pass.draw(start..end, 0..1);
                }
            }

            if gradient_at < rects.len() {
                pass.set_pipeline(&self.rect_pipeline);
                pass.set_bind_group(0, &self.screen_bind, &[]);
                pass.set_vertex_buffer(0, self.rect_buf.slice(..));
                pass.draw(gradient_at as u32..rects.len() as u32, 0..1);
            }

            if text_key.is_some() && text_opacity > 0.01 {
                let _ = self
                    .text_renderer
                    .render(&self.atlas, &self.viewport, &mut pass);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        if suboptimal {
            self.surface.configure(&self.device, &self.config);
        }
        self.atlas.trim();
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    fn update_previews(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        cards: &[CardGeom],
        scale: f32,
        time: f32,
    ) {
        let mut dirty = [false; 10];
        for card in cards {
            let (width, height) = preview_target(card.w, card.h, scale);
            let stale = self.previews[card.index]
                .as_ref()
                .map(|image| image.width != width || image.height != height)
                .unwrap_or(true);
            if stale {
                self.previews[card.index] = Some(self.make_image(width, height));
                dirty[card.index] = true;
            }
        }
        for index in 0..SAVERS.len() {
            if dirty[index] {
                self.render_preview(encoder, index, time);
            }
        }
        let mut budget = PREVIEW_BUDGET;
        let mut guard = 0;
        while budget > 0 && guard < SAVERS.len() {
            let index = self.preview_cursor;
            self.preview_cursor = (self.preview_cursor + 1) % SAVERS.len();
            guard += 1;
            if dirty[index] || self.previews[index].is_none() {
                continue;
            }
            self.render_preview(encoder, index, time);
            budget -= 1;
        }
    }

    fn render_preview(&mut self, encoder: &mut wgpu::CommandEncoder, index: usize, time: f32) {
        let Some(image) = self.previews[index].as_ref() else {
            return;
        };
        let view = image.view.clone();
        let width = image.width;
        let height = image.height;
        self.render_shader(encoder, index, index as u32, &view, width, height, time);
    }

    fn render_shader(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        index: usize,
        slot: u32,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f32,
    ) {
        let globals = Globals {
            resolution: [width as f32, height as f32],
            time,
            _pad: 0.0,
        };
        let offset = slot as u64 * self.align;
        self.queue
            .write_buffer(&self.globals, offset, bytemuck::bytes_of(&globals));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(SAVERS[index].id),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        pass.set_pipeline(&self.pipelines[index]);
        pass.set_bind_group(0, &self.globals_bind, &[offset as u32]);
        pass.draw(0..3, 0..1);
    }

    fn make_image(&self, width: u32, height: u32) -> Image {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("offscreen"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Image {
            view,
            bind,
            width,
            height,
        }
    }

    fn sync_text(&mut self, key: TextKey, runs: &[TextRun], scale: f32) {
        if self.text_key == Some(key) {
            return;
        }
        self.text_key = Some(key);
        self.text_runs = runs
            .iter()
            .map(|run| TextRun {
                text: run.text.clone(),
                x: run.x * scale,
                y: run.y * scale,
                w: run.w * scale,
                size: run.size * scale,
                line_height: run.line_height * scale,
                color: run.color,
                serif: run.serif,
                italic: run.italic,
                tracking: run.tracking,
                clip: run.clip.map(|[x, y, w, h]| [x * scale, y * scale, w * scale, h * scale]),
            })
            .collect();
        self.text_buffers.clear();
        for run in &self.text_runs {
            let metrics = Metrics::new(run.size, run.line_height.max(run.size));
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            let mut attrs = Attrs::new().family(if run.serif {
                Family::Name("C059")
            } else {
                Family::Name("Adwaita Sans")
            });
            if run.italic {
                attrs = attrs.style(Style::Italic);
            }
            if run.tracking != 0.0 {
                attrs = attrs.letter_spacing(run.tracking);
            }
            {
                let mut borrowed = buffer.borrow_with(&mut self.font_system);
                borrowed.set_wrap(Wrap::Word);
                borrowed.set_size(Some(run.w), Some(run.line_height.max(run.size) * 8.0));
                borrowed.set_text(&run.text, &attrs, Shaping::Advanced, None);
                // glyphon reads layout_runs(), which stays empty until the buffer is shaped.
                borrowed.shape_until_scroll(false);
            }
            self.text_buffers.push(buffer);
        }
    }

    fn prepare_text(&mut self, opacity: f32) {
        let areas: Vec<TextArea> = self
            .text_buffers
            .iter()
            .zip(self.text_runs.iter())
            .map(|(buffer, run)| {
                let alpha = ((run.color[3] as f32) * opacity).round().clamp(0.0, 255.0) as u8;
                let (left, top, right, bottom) = match run.clip {
                    Some([x, y, w, h]) => (x as i32, y as i32, (x + w) as i32, (y + h) as i32),
                    None => (0, 0, self.config.width as i32, self.config.height as i32),
                };
                TextArea {
                    buffer,
                    left: run.x,
                    top: run.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left,
                        top,
                        right,
                        bottom,
                    },
                    default_color: Color::rgba(run.color[0], run.color[1], run.color[2], alpha),
                    custom_glyphs: &[],
                }
            })
            .collect();
        if let Err(err) = self.text_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        ) {
            eprintln!("text: {err}");
        }
    }
}

fn shader(device: &wgpu::Device, label: &str, source: wgpu::ShaderSource) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source,
    })
}

fn render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    vertex: &wgpu::ShaderModule,
    vertex_entry: &str,
    fragment: &wgpu::ShaderModule,
    fragment_entry: &str,
    format: wgpu::TextureFormat,
    buffers: &[Option<wgpu::VertexBufferLayout>],
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment_entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: vertex,
            entry_point: Some(vertex_entry),
            buffers,
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: fragment,
            entry_point: Some(fragment_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn quad_layout() -> [Option<wgpu::VertexBufferLayout<'static>>; 1] {
    const ATTRS: [wgpu::VertexAttribute; 5] = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 8,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 16,
            shader_location: 2,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 24,
            shader_location: 3,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32,
            offset: 32,
            shader_location: 4,
        },
    ];
    [Some(wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<QuadVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRS,
    })]
}

fn rect_layout_attrs() -> [Option<wgpu::VertexBufferLayout<'static>>; 1] {
    const ATTRS: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 8,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 24,
            shader_location: 2,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x2,
            offset: 40,
            shader_location: 3,
        },
    ];
    [Some(wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<RectVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRS,
    })]
}

fn push_rects(out: &mut Vec<RectVertex>, rects: &[PaintRect], scale: f32) {
    for rect in rects {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            continue;
        }
        let x = rect.x * scale;
        let y = rect.y * scale;
        let w = (rect.w * scale).max(1.0);
        let h = (rect.h * scale).max(1.0);
        let radius = rect.radius * scale;
        let corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
        for index in [0, 1, 2, 0, 2, 3] {
            out.push(RectVertex {
                pos: corners[index],
                color: rect.color,
                rect: [x, y, w, h],
                params: [radius, rect.mode],
                _pad: [0.0, 0.0],
            });
        }
    }
}

fn card_quad(card: &CardGeom, scale: f32) -> Vec<QuadVertex> {
    let x = (card.x + 1.0) * scale;
    let y = (card.y + 1.0) * scale;
    let w = ((card.w - 2.0) * scale).max(1.0);
    let h = ((card.h - 2.0) * scale).max(1.0);
    let radius = 13.0 * scale;
    quad(x, y, w, h, radius, [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
}

fn saver_quad(logical: [f32; 2], scale: f32, time: f32) -> Vec<QuadVertex> {
    let (tx, ty) = layout::wander(time);
    let ox = -PAD + tx;
    let oy = -PAD + ty;
    let cw = logical[0] + PAD * 2.0;
    let ch = logical[1] + PAD * 2.0;
    let uv = |x: f32, y: f32| [(x - ox) / cw, (y - oy) / ch];
    let w = logical[0] * scale;
    let h = logical[1] * scale;
    quad(
        0.0,
        0.0,
        w,
        h,
        0.0,
        [
            uv(0.0, 0.0),
            uv(logical[0], 0.0),
            uv(logical[0], logical[1]),
            uv(0.0, logical[1]),
        ],
    )
}

fn quad(x: f32, y: f32, w: f32, h: f32, radius: f32, uv: [[f32; 2]; 4]) -> Vec<QuadVertex> {
    let pos = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    let local = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    let mut verts = Vec::with_capacity(6);
    for index in [0, 1, 2, 0, 2, 3] {
        verts.push(QuadVertex {
            pos: pos[index],
            uv: uv[index],
            local: local[index],
            size: [w, h],
            radius,
            _pad: [0.0, 0.0, 0.0],
        });
    }
    verts
}

fn saver_target(logical_w: f32, logical_h: f32) -> (u32, u32) {
    let css_w = logical_w + PAD * 2.0;
    let css_h = logical_h + PAD * 2.0;
    let scale = (MAX_LONG / css_w.max(css_h)).min(1.0);
    (
        (css_w * scale).round().max(1.0) as u32,
        (css_h * scale).round().max(1.0) as u32,
    )
}

fn preview_target(card_w: f32, card_h: f32, scale: f32) -> (u32, u32) {
    let mut width = (card_w * scale).round().max(1.0);
    let mut height = (card_h * scale).round().max(1.0);
    let long = width.max(height);
    if long > PREVIEW_LONG {
        let fit = PREVIEW_LONG / long;
        width = (width * fit).round().max(1.0);
        height = (height * fit).round().max(1.0);
    }
    (width as u32, height as u32)
}

fn pick_format(caps: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
    for format in [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm] {
        if caps.formats.contains(&format) {
            return format;
        }
    }
    caps.formats[0]
}

fn gpu_color(color: [f32; 4]) -> wgpu::Color {
    wgpu::Color {
        r: color[0] as f64,
        g: color[1] as f64,
        b: color[2] as f64,
        a: color[3] as f64,
    }
}

fn quant(value: f32) -> u32 {
    value.round().max(0.0) as u32
}
