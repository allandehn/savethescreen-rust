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
use crate::savers::{self, Saver};

const PREVIEW_LONG: f32 = 320.0;
const PAD: f32 = 28.0;
const PREVIEW_BUDGET: usize = 2;

/// Shadertoy inputs, in the order Naga lays out this uniform block.
/// Naga packs `iTime` into the tail of `iResolution`, then aligns `iMouse` to 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    resolution: [f32; 3],
    time: f32,
    time_delta: f32,
    frame_rate: f32,
    frame: i32,
    _pad_mouse: f32,
    mouse: [f32; 4],
    date: [f32; 4],
    sample_rate: f32,
    /// Naga rounds the struct up to 16, then gives each `iChannelTime` element a stride of 16.
    _pad_channels: [f32; 3],
    channel_time: [[f32; 4]; 4],
    /// Each `iChannelResolution` vec3 is stored at a stride of 16.
    channel_resolution: [[f32; 4]; 4],
}

const _: () = assert!(std::mem::size_of::<Globals>() == 208);
const _: () = assert!(std::mem::offset_of!(Globals, time) == 12);
const _: () = assert!(std::mem::offset_of!(Globals, time_delta) == 16);
const _: () = assert!(std::mem::offset_of!(Globals, frame_rate) == 20);
const _: () = assert!(std::mem::offset_of!(Globals, frame) == 24);
const _: () = assert!(std::mem::offset_of!(Globals, mouse) == 32);
const _: () = assert!(std::mem::offset_of!(Globals, date) == 48);
const _: () = assert!(std::mem::offset_of!(Globals, sample_rate) == 64);
const _: () = assert!(std::mem::offset_of!(Globals, channel_time) == 80);
const _: () = assert!(std::mem::offset_of!(Globals, channel_resolution) == 144);

pub struct FrameClock {
    pub time: f32,
    pub delta: f32,
    pub frame: i32,
    pub pointer: [f32; 2],
    pub pointer_down: bool,
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
    Gallery(u32, u32, u32, u32, u32, u32, u32),
    Hud(usize, u32, u32, u32, u32),
    Note(usize, u32, u32, u32),
}

pub enum Scene {
    Gallery {
        hover: Option<usize>,
        scroll: f32,
        detail: Option<u8>,
        /// Card under the pointer. That preview plays from its own clock.
        /// The others keep the frame where the pointer left them.
        playing: Option<usize>,
        /// Monitor width divided by height. The card picture uses this.
        aspect: f32,
    },
    Saver {
        index: usize,
        hud_opacity: f32,
        detail: u8,
    },
    /// A shader that did not compile. The note stays still, with no picture.
    Hold { index: usize },
}

struct ShaderGpu {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
}

struct StandIn {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

pub struct Renderer {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    pub adapter_name: String,

    /// `None` keeps the card's place when that shader did not compile.
    shaders: Vec<Option<ShaderGpu>>,
    globals: wgpu::Buffer,
    align: u64,
    /// Kept so the stand-in pictures outlive the views held by the shader bind groups.
    #[allow(dead_code)]
    stand_in: StandIn,
    #[allow(dead_code)]
    stand_in_cube: Option<StandIn>,

    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    quad_buf: wgpu::Buffer,

    rect_pipeline: wgpu::RenderPipeline,
    rect_buf: wgpu::Buffer,
    screen_buf: wgpu::Buffer,
    screen_bind: wgpu::BindGroup,

    previews: Vec<Option<Image>>,
    /// Time and frame each card has reached. Leaving a card keeps these.
    preview_clock: Vec<CardClock>,
    /// True once that preview's texture shows `preview_clock`.
    preview_still: Vec<bool>,
    /// Card played on the previous gallery draw. The same card keeps its clock running.
    preview_playing: Option<usize>,
    saver_image: Option<Image>,
    mouse_was_down: bool,
    mouse_click: [f32; 2],
    mouse_last: [f32; 2],

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
    pub fn new(
        window: Arc<Window>,
        event_loop: &ActiveEventLoop,
        savers: &mut Vec<Saver>,
    ) -> Result<Self, String> {
        pollster::block_on(Self::create(window, event_loop, savers))
    }

    async fn create(
        window: Arc<Window>,
        event_loop: &ActiveEventLoop,
        savers: &mut Vec<Saver>,
    ) -> Result<Self, String> {
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

        let globals_size =
            std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap();
        let errors = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let fullscreen = shader(
            &device,
            "fullscreen",
            wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/fullscreen.wgsl"))),
        );
        let pending = std::mem::take(savers);
        let mut compiled: Vec<Option<Compiled>> = Vec::with_capacity(pending.len());
        for mut saver in pending {
            match compile_fragment(&device, &fullscreen, &saver.id, &saver.source).await {
                Ok(shader) => {
                    compiled.push(Some(shader));
                    savers.push(saver);
                }
                Err(err) => {
                    let hold = explain_compile(&err);
                    eprintln!(
                        "savethescreen: {} ({})\n{}\n\n{err}",
                        saver.name, saver.origin, hold.page
                    );
                    saver.hold = Some(hold);
                    compiled.push(None);
                    savers.push(saver);
                }
            }
        }
        let stand_in = make_stand_in(&device, &queue, false);
        let stand_in_cube = compiled
            .iter()
            .flatten()
            .any(|shader| {
                shader
                    .kinds
                    .iter()
                    .any(|kind| matches!(kind, Some(savers::ChannelKind::Cube)))
            })
            .then(|| make_stand_in(&device, &queue, true));
        let min_align = device.limits().min_uniform_buffer_offset_alignment.max(16) as u64;
        let align = (globals_size.get()).max(min_align).div_ceil(min_align) * min_align;
        // One slot per card, including a shader this app could not compile, plus the running saver.
        let slots = (compiled.len() + 1) as u64;
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: align * slots,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // Repeat, so a lookup can walk off the edge and come back in.
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let shaders: Vec<Option<ShaderGpu>> = compiled
            .into_iter()
            .map(|shader| {
                let shader = shader?;
                let bind = shader_bind(
                    &device,
                    &shader.layout,
                    &globals,
                    globals_size,
                    &sampler,
                    &stand_in.view,
                    stand_in_cube.as_ref().map(|image| &image.view),
                    shader.kinds,
                );
                Some(ShaderGpu {
                    pipeline: shader.pipeline,
                    bind,
                })
            })
            .collect();
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

        let preview_count = shaders.len();
        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            window,
            adapter_name: format!("{} ({:?})", info.name, info.backend),
            shaders,
            globals,
            align,
            stand_in,
            stand_in_cube,
            blit_pipeline,
            blit_layout,
            sampler,
            quad_buf,
            rect_pipeline,
            rect_buf,
            screen_buf,
            screen_bind,
            previews: (0..preview_count).map(|_| None).collect(),
            preview_clock: vec![
                CardClock {
                    time: 0.0,
                    frame: 0
                };
                preview_count
            ],
            preview_still: vec![false; preview_count],
            preview_playing: None,
            saver_image: None,
            mouse_was_down: false,
            mouse_click: [0.0, 0.0],
            mouse_last: [0.0, 0.0],
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

    pub fn draw(
        &mut self,
        logical: [f32; 2],
        scale: f32,
        clock: &FrameClock,
        scene: Scene,
        savers: &[Saver],
        shader_dir: &str,
    ) {
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
            Scene::Gallery {
                hover,
                scroll,
                detail,
                playing,
                aspect,
            } => {
                let layout = layout::gallery(
                    savers, shader_dir, logical[0], logical[1], scroll, hover, detail, aspect,
                );
                self.update_previews(&mut encoder, &layout.cards, scale, playing, clock, savers);
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
                push_rects(
                    &mut rects,
                    &layout::note_marks(&layout.cards, savers),
                    scale,
                );
                runs = layout.texts;
                text_key = Some(TextKey::Gallery(
                    quant(logical[0]),
                    quant(logical[1]),
                    quant(scale * 1000.0),
                    quant(layout.scroll),
                    hover.map(|index| index as u32 + 1).unwrap_or(0),
                    u32::from(detail.unwrap_or(0)),
                    quant(aspect * 1000.0),
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
            Scene::Saver {
                index,
                hud_opacity,
                detail,
            } => {
                // Coming back to the shelf is a new hover, so the card steps on from its still.
                self.preview_playing = None;
                if index >= savers.len() {
                    return;
                }
                if self.runnable(index) {
                    let quality = f32::from(detail) / 100.0;
                    let (width, height) = saver_target(logical[0], logical[1], scale, quality);
                    let stale = self
                        .saver_image
                        .as_ref()
                        .map(|image| image.width != width || image.height != height)
                        .unwrap_or(true);
                    if stale {
                        self.saver_image = Some(self.make_image(width, height));
                    }
                    let view = self.saver_image.as_ref().unwrap().view.clone();
                    let mouse = self.mouse_uniform(
                        shader_pointer(logical, scale, quality, clock.time, clock.pointer),
                        clock.pointer_down,
                    );
                    self.render_shader(
                        &mut encoder,
                        savers[index].id.as_str(),
                        index,
                        savers.len() as u32,
                        &view,
                        width,
                        height,
                        clock,
                        mouse,
                    );
                    quads.push((Blit::Saver, saver_quad(logical, scale, clock.time)));
                }
                if hud_opacity > 0.01 {
                    runs = layout::hud(savers, index, logical[0], logical[1], detail);
                    text_key = Some(TextKey::Hud(
                        index,
                        quant(logical[0]),
                        quant(logical[1]),
                        quant(scale * 1000.0),
                        u32::from(detail),
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
            Scene::Hold { index } => {
                self.preview_playing = None;
                if let Some(saver) = savers.get(index) {
                    runs = layout::hold_page(saver, logical[0], logical[1]);
                    text_key = Some(TextKey::Note(
                        index,
                        quant(logical[0]),
                        quant(logical[1]),
                        quant(scale * 1000.0),
                    ));
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
            let bytes = std::mem::size_of_val(rects.as_slice()) as u64;
            if self.rect_buf.size() < bytes {
                self.rect_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rects"),
                    size: bytes.next_power_of_two().max(256),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
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
            let bytes = std::mem::size_of_val(packed.as_slice()) as u64;
            if self.quad_buf.size() < bytes {
                self.quad_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("quads"),
                    size: bytes.next_power_of_two().max(256),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            self.queue
                .write_buffer(&self.quad_buf, 0, bytemuck::cast_slice(&packed));
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
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

    /// A visible card still needs its picture painted. The shelf can rest once these are done.
    pub fn thumbnails_pending(&self) -> bool {
        self.previews
            .iter()
            .zip(self.preview_still.iter())
            .any(|(image, still)| image.is_some() && !still)
    }

    fn update_previews(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        cards: &[CardGeom],
        scale: f32,
        playing: Option<usize>,
        clock: &FrameClock,
        savers: &[Saver],
    ) {
        if savers.is_empty() {
            return;
        }
        let mut resized = vec![false; savers.len()];
        for card in cards {
            if card.index >= self.previews.len() || !self.runnable(card.index) {
                continue;
            }
            let (width, height) = preview_target(card.w, card.h, scale);
            let stale = self.previews[card.index]
                .as_ref()
                .map(|image| image.width != width || image.height != height)
                .unwrap_or(true);
            if stale {
                self.previews[card.index] = Some(self.make_image(width, height));
                self.preview_still[card.index] = false;
                resized[card.index] = true;
            }
        }
        if let Some(index) = playing {
            if self.runnable(index) && index < savers.len() && self.previews[index].is_some() {
                let continued = self.preview_playing == Some(index);
                if let Some(sample) = preview_sample(
                    &mut self.preview_clock[index],
                    &mut self.preview_still[index],
                    true,
                    continued,
                    clock.delta,
                ) {
                    self.render_preview(
                        encoder,
                        index,
                        savers[index].id.as_str(),
                        &sample_clock(&sample),
                    );
                    resized[index] = false;
                }
                self.preview_playing = Some(index);
            } else {
                self.preview_playing = None;
            }
        } else {
            self.preview_playing = None;
        }
        // Cards with no picture yet, including one that was resized, get the frame
        // their clock is already on. A card that already has its frame is left alone.
        let mut budget = PREVIEW_BUDGET;
        for index in 0..savers.len() {
            if budget == 0 {
                break;
            }
            if playing == Some(index) || !self.runnable(index) || self.previews[index].is_none() {
                continue;
            }
            if self.preview_still[index] && !resized[index] {
                continue;
            }
            if let Some(sample) = preview_sample(
                &mut self.preview_clock[index],
                &mut self.preview_still[index],
                false,
                false,
                0.0,
            ) {
                self.render_preview(
                    encoder,
                    index,
                    savers[index].id.as_str(),
                    &sample_clock(&sample),
                );
                budget -= 1;
            }
        }
    }

    fn render_preview(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        index: usize,
        label: &str,
        clock: &FrameClock,
    ) {
        let Some(image) = self.previews[index].as_ref() else {
            return;
        };
        let view = image.view.clone();
        let width = image.width;
        let height = image.height;
        self.render_shader(
            encoder,
            label,
            index,
            index as u32,
            &view,
            width,
            height,
            clock,
            [0.0; 4],
        );
    }

    fn render_shader(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        label: &str,
        index: usize,
        slot: u32,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        clock: &FrameClock,
        mouse: [f32; 4],
    ) {
        let delta = if clock.delta > 0.0 {
            clock.delta
        } else {
            1.0 / 30.0
        };
        let globals = Globals {
            resolution: [width as f32, height as f32, 1.0],
            time: clock.time,
            time_delta: delta,
            frame_rate: 1.0 / delta,
            frame: clock.frame,
            mouse,
            date: local_date(),
            sample_rate: 44100.0,
            _pad_mouse: 0.0,
            _pad_channels: [0.0; 3],
            channel_time: [[0.0; 4]; 4],
            channel_resolution: stand_in_resolution(),
        };
        if !self.runnable(index) {
            return;
        }
        let offset = slot as u64 * self.align;
        self.queue
            .write_buffer(&self.globals, offset, bytemuck::bytes_of(&globals));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
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
        let gpu = self.shaders[index].as_ref().expect("runnable shader");
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &gpu.bind, &[offset as u32]);
        pass.draw(0..3, 0..1);
    }

    fn runnable(&self, index: usize) -> bool {
        self.shaders.get(index).is_some_and(|slot| slot.is_some())
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
                clip: run
                    .clip
                    .map(|[x, y, w, h]| [x * scale, y * scale, w * scale, h * scale]),
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

fn mouse_uniform_from(
    was_down: &mut bool,
    click: &mut [f32; 2],
    last: &mut [f32; 2],
    frag: [f32; 2],
    down: bool,
) -> [f32; 4] {
    if down {
        let just = !*was_down;
        *was_down = true;
        if just {
            *click = frag;
        }
        *last = frag;
        let w = if just { click[1] } else { -click[1].abs() };
        [frag[0], frag[1], click[0], w]
    } else {
        *was_down = false;
        [last[0], last[1], -click[0].abs(), -click[1].abs()]
    }
}

impl Renderer {
    fn mouse_uniform(&mut self, frag: [f32; 2], down: bool) -> [f32; 4] {
        mouse_uniform_from(
            &mut self.mouse_was_down,
            &mut self.mouse_click,
            &mut self.mouse_last,
            frag,
            down,
        )
    }
}

struct Compiled {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    kinds: [Option<savers::ChannelKind>; 4],
}

async fn compile_fragment(
    device: &wgpu::Device,
    fullscreen: &wgpu::ShaderModule,
    label: &str,
    source: &str,
) -> Result<Compiled, String> {
    let kinds = savers::channel_kinds(source);
    let globals_size = std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap();
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &channel_entries(globals_size, kinds),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let wrapped = savers::fragment_source(source);
    let module = shader(
        device,
        label,
        wgpu::ShaderSource::Glsl {
            shader: Cow::Owned(wrapped),
            stage: wgpu::naga::ShaderStage::Fragment,
            defines: &[],
        },
    );
    let pipeline = render_pipeline(
        device,
        &pipeline_layout,
        fullscreen,
        "vs_main",
        &module,
        "main",
        wgpu::TextureFormat::Rgba8Unorm,
        &[],
        None,
    );
    let _ = device.poll(wgpu::PollType::Poll);
    match scope.pop().await {
        Some(err) => Err(err.to_string()),
        None => Ok(Compiled {
            pipeline,
            layout,
            kinds,
        }),
    }
}

fn channel_entries(
    globals_size: std::num::NonZeroU64,
    kinds: [Option<savers::ChannelKind>; 4],
) -> Vec<wgpu::BindGroupLayoutEntry> {
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: Some(globals_size),
        },
        count: None,
    }];
    if kinds.iter().any(|kind| kind.is_some()) {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    for (index, kind) in kinds.iter().enumerate() {
        let Some(kind) = kind else {
            continue;
        };
        let view_dimension = match kind {
            savers::ChannelKind::Image => wgpu::TextureViewDimension::D2,
            savers::ChannelKind::Cube => wgpu::TextureViewDimension::Cube,
        };
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 2 + index as u32,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension,
                multisampled: false,
            },
            count: None,
        });
    }
    entries
}

fn shader_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    globals_size: std::num::NonZeroU64,
    sampler: &wgpu::Sampler,
    stand_in: &wgpu::TextureView,
    stand_in_cube: Option<&wgpu::TextureView>,
    kinds: [Option<savers::ChannelKind>; 4],
) -> wgpu::BindGroup {
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: globals,
            offset: 0,
            size: Some(globals_size),
        }),
    }];
    if kinds.iter().any(|kind| kind.is_some()) {
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(sampler),
        });
    }
    for (index, kind) in kinds.iter().enumerate() {
        let Some(kind) = kind else {
            continue;
        };
        let view = match kind {
            savers::ChannelKind::Image => stand_in,
            savers::ChannelKind::Cube => stand_in_cube.expect("cube stand-in"),
        };
        entries.push(wgpu::BindGroupEntry {
            binding: 2 + index as u32,
            resource: wgpu::BindingResource::TextureView(view),
        });
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shader"),
        layout,
        entries: &entries,
    })
}

fn stand_in_resolution() -> [[f32; 4]; 4] {
    let size = savers::STAND_IN as f32;
    [[size, size, 1.0, 0.0]; 4]
}

fn make_stand_in(device: &wgpu::Device, queue: &wgpu::Queue, cube: bool) -> StandIn {
    let size = savers::STAND_IN;
    let pixels = stand_in_pixels(size);
    let layers = if cube { 6 } else { 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(if cube { "stand-in-cube" } else { "stand-in" }),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for layer in 0..layers {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(if cube {
            wgpu::TextureViewDimension::Cube
        } else {
            wgpu::TextureViewDimension::D2
        }),
        ..Default::default()
    });
    StandIn {
        _texture: texture,
        view,
    }
}

fn noise_byte(x: u32, y: u32, salt: u32) -> u8 {
    let mut n = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(salt.wrapping_mul(2_246_822_519));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n ^ (n >> 16)) as u8
}

/// RGBA noise in the layout Shadertoy's noise picture uses.
/// Red and blue are independent. Green is red shifted by (37, 17),
/// and alpha is blue shifted the same way, so one sample holds a value
/// and the next slice.
fn stand_in_pixels(size: u32) -> Vec<u8> {
    let mut red = vec![0u8; (size * size) as usize];
    let mut blue = vec![0u8; (size * size) as usize];
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) as usize;
            red[i] = noise_byte(x, y, 0);
            blue[i] = noise_byte(x, y, 1);
        }
    }
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let sx = (x + 37) % size;
            let sy = (y + 17) % size;
            let here = (y * size + x) as usize;
            let shifted = (sy * size + sx) as usize;
            pixels.extend_from_slice(&[red[here], red[shifted], blue[here], blue[shifted]]);
        }
    }
    pixels
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
    quad(
        x,
        y,
        w,
        h,
        radius,
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    )
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

fn shader_pointer(
    logical: [f32; 2],
    pixel_scale: f32,
    quality: f32,
    time: f32,
    pointer: [f32; 2],
) -> [f32; 2] {
    let (tx, ty) = layout::wander(time);
    let css_w = logical[0] + PAD * 2.0;
    let css_h = logical[1] + PAD * 2.0;
    let (buf_w, buf_h) = saver_target(logical[0], logical[1], pixel_scale, quality);
    let px = (pointer[0] + PAD - tx) / css_w * buf_w as f32;
    let py = (pointer[1] + PAD - ty) / css_h * buf_h as f32;
    [px, buf_h as f32 - py]
}

/// Shadertoy `iDate`: year, month, day, and seconds since local midnight.
fn local_date() -> [f32; 4] {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as libc::time_t)
        .unwrap_or(0);
    unsafe {
        let mut tm = std::mem::zeroed::<libc::tm>();
        if libc::localtime_r(&secs, &mut tm).is_null() {
            return [1970.0, 1.0, 1.0, 0.0];
        }
        let seconds = (tm.tm_hour * 3600 + tm.tm_min * 60 + tm.tm_sec) as f32;
        [
            (tm.tm_year + 1900) as f32,
            (tm.tm_mon + 1) as f32,
            tm.tm_mday as f32,
            seconds,
        ]
    }
}

/// Picture size for a running saver. `quality` is 0.1 at 10% and 1.0 at the full window,
/// including the device scale, so 100% is one shader pixel per panel pixel.
fn saver_target(logical_w: f32, logical_h: f32, pixel_scale: f32, quality: f32) -> (u32, u32) {
    let pixel_scale = if pixel_scale.is_finite() && pixel_scale > 0.0 {
        pixel_scale
    } else {
        1.0
    };
    let quality = if quality.is_finite() {
        quality.clamp(0.1, 1.0)
    } else {
        0.5
    };
    let width = ((logical_w.max(1.0) + PAD * 2.0) * pixel_scale * quality)
        .round()
        .clamp(1.0, 8192.0);
    let height = ((logical_h.max(1.0) + PAD * 2.0) * pixel_scale * quality)
        .round()
        .clamp(1.0, 8192.0);
    (width as u32, height as u32)
}

const PREVIEW_FRAME: f32 = 1.0 / 30.0;

/// Where one shelf card has got to. Leaving the card keeps this.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CardClock {
    time: f32,
    frame: i32,
}

struct PreviewSample {
    time: f32,
    frame: i32,
    delta: f32,
}

fn sample_clock(sample: &PreviewSample) -> FrameClock {
    FrameClock {
        time: sample.time,
        delta: sample.delta,
        frame: sample.frame,
        pointer: [0.0, 0.0],
        pointer_down: false,
    }
}

/// The next picture for one shelf card.
///
/// The first paint is whatever time the card is on, which starts at the opening
/// frame. While the pointer stays, time follows the draw. Leaving draws nothing,
/// so the texture stays. The next hover steps one frame on from that still.
fn preview_sample(
    clock: &mut CardClock,
    held: &mut bool,
    playing: bool,
    continued: bool,
    live_delta: f32,
) -> Option<PreviewSample> {
    if !*held {
        *held = true;
        return Some(PreviewSample {
            time: clock.time,
            frame: clock.frame,
            delta: PREVIEW_FRAME,
        });
    }
    if !playing {
        return None;
    }
    let step = if continued {
        if live_delta.is_finite() {
            live_delta.clamp(0.0, 0.1)
        } else {
            PREVIEW_FRAME
        }
    } else {
        PREVIEW_FRAME
    };
    // A second draw in the same instant would repeat the frame. Wait for time to move.
    if step <= 1.0 / 1000.0 {
        return None;
    }
    clock.time += step;
    clock.frame = (clock.time / PREVIEW_FRAME).round() as i32;
    Some(PreviewSample {
        time: clock.time,
        frame: clock.frame,
        delta: step,
    })
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
    for format in [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ] {
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

const TURN_CARD: &str =
    "This shader's turn is written too briefly for this app. Open the card for the line to change.";
const GENERIC_CARD: &str = "This app could not read this shader. Open the card for what to try.";

struct Cited {
    number: u32,
    text: String,
    pointed: bool,
}

/// Plain words for a compile failure. The card is one sentence. The page is the still note.
fn explain_compile(err: &str) -> savers::Hold {
    let cited = citations(err);
    if let Some(line) = turn_line(err, &cited) {
        return savers::Hold {
            card: TURN_CARD.to_string(),
            page: turn_page(line.number, &line.text),
        };
    }
    savers::Hold {
        card: GENERIC_CARD.to_string(),
        page: generic_page(focus_line(&cited)),
    }
}

fn turn_line<'a>(err: &str, cited: &'a [Cited]) -> Option<&'a Cited> {
    if let Some(hit) = cited
        .iter()
        .find(|item| item.text.contains("mat2(") && item.text.contains("vec4"))
    {
        return Some(hit);
    }
    if err.contains("Composing") {
        return cited.iter().rev().find(|item| item.text.contains("mat2("));
    }
    None
}

fn turn_page(line_no: u32, source_line: &str) -> String {
    let quoted = source_line.trim();
    let call = twist_call(quoted);
    let vec4 = call
        .as_ref()
        .map(|(args, _)| args.clone())
        .or_else(|| vec4_on_line(quoted))
        .unwrap_or_else(|| "0, -11, 11, 0".to_string());
    let replacement = if let Some((_, angle)) = &call {
        format!("On line {line_no}, write twist({angle}) in place of mat2(cos(...)).")
    } else {
        format!(
            "On line {line_no}, write twist(...) in place of mat2(cos(...)). Put the four numbers from your line into the vec4 above, and pass the rest of the cos to twist."
        )
    };
    let finished = call
        .as_ref()
        .and_then(|(_, angle)| replace_mat2_cos(quoted, angle));
    let shown = if let Some(line) = &finished {
        format!("The line there now is:\n\n{quoted}\n\nThe line can read:\n\n{line}")
    } else {
        format!("The line there now is:\n\n{quoted}")
    };
    format!(
        "Line {line_no} writes a turn in a short form this app cannot read. Add the longer form below, save the file, and start the app again.\n\n\
Add this above mainImage:\n\n\
mat2 twist(float a)\n\
{{\n\
    vec4 c = cos(a + vec4({vec4}));\n\
    return mat2(c.xy, c.zw);\n\
}}\n\n\
{replacement}\n\n\
{shown}"
    )
}

/// `mat2(cos(...))` on a line, with the rest of that line left as it is.
fn replace_mat2_cos(line: &str, angle: &str) -> Option<String> {
    let start = line.find("mat2(")?;
    let inner = paren_inner(line, start + 4)?;
    if !inner.trim_start().starts_with("cos(") {
        return None;
    }
    let close = start + 4 + 1 + inner.len();
    if line.as_bytes().get(close) != Some(&b')') {
        return None;
    }
    let mut out = String::new();
    out.push_str(&line[..start]);
    out.push_str("twist(");
    out.push_str(angle);
    out.push(')');
    out.push_str(&line[close + 1..]);
    Some(out)
}

fn generic_page(cited: Option<&Cited>) -> String {
    match cited {
        Some(line) => format!(
            "This app could not read this shader, so the card stays on the shelf. It may still run on Shadertoy.\n\n\
It stopped on line {}:\n\n\
{}\n\n\
The full detail is also printed where the app was started.",
            line.number,
            line.text.trim()
        ),
        None => "This app could not read this shader, so the card stays on the shelf. It may still run on Shadertoy. The full detail is also printed where the app was started.".to_string(),
    }
}

fn focus_line(cited: &[Cited]) -> Option<&Cited> {
    cited
        .iter()
        .rev()
        .find(|item| item.pointed)
        .or_else(|| cited.last())
}

fn citations(err: &str) -> Vec<Cited> {
    let raw: Vec<&str> = err.lines().collect();
    let mut out = Vec::new();
    for (index, line) in raw.iter().enumerate() {
        let Some((number, text)) = split_citation(line) else {
            continue;
        };
        let text = text.trim();
        if text.is_empty() || text.chars().all(|c| c == '^') {
            continue;
        }
        let pointed = raw[index + 1..]
            .iter()
            .take_while(|next| split_citation(next).is_none())
            .any(|next| is_caret_line(next));
        out.push(Cited {
            number,
            text: text.to_string(),
            pointed,
        });
    }
    out
}

fn split_citation(line: &str) -> Option<(u32, &str)> {
    let at = citation_bar(line)?;
    let bar_len = if line[at..].starts_with('\u{2502}') {
        '\u{2502}'.len_utf8()
    } else {
        1
    };
    let head = line[..at].trim();
    if head.is_empty() {
        return None;
    }
    let number = head.parse().ok()?;
    Some((number, &line[at + bar_len..]))
}

fn citation_bar(line: &str) -> Option<usize> {
    if let Some(at) = line.find('\u{2502}') {
        return Some(at);
    }
    let at = line.find('|')?;
    let head = line[..at].trim();
    if head.is_empty() || head.bytes().all(|byte| byte.is_ascii_digit()) {
        Some(at)
    } else {
        None
    }
}

fn is_caret_line(line: &str) -> bool {
    let Some(at) = citation_bar(line) else {
        return false;
    };
    split_citation(line).is_none() && line[at..].contains('^')
}

/// `mat2(cos(angle + vec4(...)))` split into the four numbers and the angle.
fn twist_call(line: &str) -> Option<(String, String)> {
    let start = line.find("cos(")?;
    let inner = paren_inner(line, start + 3)?;
    let vec_at = inner.find("vec4(")?;
    let args = paren_inner(inner, vec_at + 4)?;
    let close = vec_at + 5 + args.len();
    if inner.as_bytes().get(close) != Some(&b')') {
        return None;
    }
    let before = inner[..vec_at].trim();
    let after = inner[close + 1..].trim();
    let angle = if let Some(rest) = before.strip_suffix('+') {
        if !after.is_empty() {
            return None;
        }
        rest.trim()
    } else if let Some(rest) = after.strip_prefix('+') {
        if !before.is_empty() {
            return None;
        }
        rest.trim()
    } else {
        return None;
    };
    if angle.is_empty() {
        return None;
    }
    Some((normalize_list(args), angle.to_string()))
}

fn vec4_on_line(line: &str) -> Option<String> {
    let start = line.find("vec4(")?;
    Some(normalize_list(paren_inner(line, start + 4)?))
}

fn normalize_list(args: &str) -> String {
    args.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn paren_inner(text: &str, open_at: usize) -> Option<&str> {
    if text.as_bytes().get(open_at) != Some(&b'(') {
        return None;
    }
    let mut depth = 0;
    for (index, ch) in text[open_at..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open_at + 1..open_at + index]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod gpu_tests {
    use super::*;

    #[test]
    fn builtin_shaders_compile() {
        pollster::block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    force_fallback_adapter: false,
                    compatible_surface: None,
                    apply_limit_buckets: false,
                })
                .await
                .expect("adapter");
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("device");
            let globals_size =
                std::num::NonZeroU64::new(std::mem::size_of::<Globals>() as u64).unwrap();
            let fullscreen = shader(
                &device,
                "fullscreen",
                wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shaders/fullscreen.wgsl"))),
            );
            let dir =
                std::env::temp_dir().join(format!("savethescreen-gpu-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let loaded = savers::load_from(&dir);
            assert_eq!(loaded.len(), 10);
            for saver in &loaded {
                if let Err(err) =
                    compile_fragment(&device, &fullscreen, &saver.id, &saver.source).await
                {
                    panic!("{} failed to compile: {err}", saver.id);
                }
            }
            let _ = std::fs::remove_dir_all(&dir);

            // A shader that reads every Shadertoy input, so a wrong uniform offset fails here.
            let probe = r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    bool ok = iResolution.x == 1.0 && iResolution.y == 1.0 && iResolution.z == 1.0
        && iTime > 0.49 && iTime < 0.51
        && iTimeDelta > 0.09 && iTimeDelta < 0.11
        && iFrameRate > 9.0 && iFrameRate < 11.0
        && iFrame == 7
        && iMouse.x == 10.0 && iMouse.y == 4.0 && iMouse.z == 3.0 && iMouse.w == 2.0
        && iDate.x == 2026.0 && iDate.y == 10.0 && iDate.z == 20.0 && iDate.w == 100.0
        && iSampleRate == 44100.0
        && fragCoord.x > 0.0 && fragCoord.y > 0.0;
    fragColor = ok ? vec4(1.0) : vec4(0.0);
}
"#;
            let compiled = compile_fragment(&device, &fullscreen, "probe", probe)
                .await
                .expect("probe shader");
            let pipeline = compiled.pipeline;
            let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("probe-globals"),
                size: globals_size.get(),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(
                &globals_buf,
                0,
                bytemuck::bytes_of(&Globals {
                    resolution: [1.0, 1.0, 1.0],
                    time: 0.5,
                    time_delta: 0.1,
                    frame_rate: 10.0,
                    frame: 7,
                    _pad_mouse: 0.0,
                    mouse: [10.0, 4.0, 3.0, 2.0],
                    date: [2026.0, 10.0, 20.0, 100.0],
                    sample_rate: 44100.0,
                    _pad_channels: [0.0; 3],
                    channel_time: [[0.0; 4]; 4],
                    channel_resolution: stand_in_resolution(),
                }),
            );
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("probe"),
                layout: &compiled.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &globals_buf,
                        offset: 0,
                        size: Some(globals_size),
                    }),
                }],
            });
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("probe"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let read = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("probe-read"),
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("probe"),
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("probe"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
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
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[0]);
                pass.draw(0..3, 0..1);
            }
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &read,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit(Some(encoder.finish()));
            let (sender, receiver) = std::sync::mpsc::channel();
            read.slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    sender.send(result).unwrap();
                });
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("poll");
            receiver.recv().unwrap().expect("map");
            let pixel = read.slice(..).get_mapped_range().unwrap()[..4].to_vec();
            assert_eq!(
                pixel,
                [255, 255, 255, 255],
                "shadertoy uniforms were misread"
            );
            read.unmap();

            let side = savers::STAND_IN;
            let channel = format!(
                r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {{
    float gray = texture(iChannel0, vec2(0.5)).r;
    bool ok = abs(iChannelResolution[0].x - {side}.0) < 0.5
        && abs(iChannelResolution[0].y - {side}.0) < 0.5
        && abs(iChannelResolution[0].z - 1.0) < 0.5
        && abs(iChannelResolution[3].y - {side}.0) < 0.5
        && iChannelTime[0] == 0.0
        && iChannelTime[3] == 0.0
        && gray > 0.9;
    fragColor = ok ? vec4(1.0) : vec4(gray, 0.0, 0.0, 1.0);
}}
"#
            );
            let channel_shader = compile_fragment(&device, &fullscreen, "channel", &channel)
                .await
                .expect("channel shader");
            let white = vec![255u8; (savers::STAND_IN * savers::STAND_IN * 4) as usize];
            let channel_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("channel-white"),
                size: wgpu::Extent3d {
                    width: savers::STAND_IN,
                    height: savers::STAND_IN,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &channel_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &white,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(savers::STAND_IN * 4),
                    rows_per_image: Some(savers::STAND_IN),
                },
                wgpu::Extent3d {
                    width: savers::STAND_IN,
                    height: savers::STAND_IN,
                    depth_or_array_layers: 1,
                },
            );
            let channel_view = channel_tex.create_view(&Default::default());
            let channel_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("channel"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let channel_bind = shader_bind(
                &device,
                &channel_shader.layout,
                &globals_buf,
                globals_size,
                &channel_sampler,
                &channel_view,
                None,
                channel_shader.kinds,
            );
            let cube = r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = texture(iChannel0, vec3(0.0, 0.0, 1.0));
}
"#;
            compile_fragment(&device, &fullscreen, "cube", cube)
                .await
                .expect("cube channel shader");
            let wormhood = savers::shader_dir().join("wormhood.frag");
            if let Ok(source) = std::fs::read_to_string(&wormhood) {
                if let Err(err) = compile_fragment(&device, &fullscreen, "wormhood", &source).await
                {
                    panic!("wormhood failed to compile: {err}");
                }
            }
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("channel"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
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
                pass.set_pipeline(&channel_shader.pipeline);
                pass.set_bind_group(0, &channel_bind, &[0]);
                pass.draw(0..3, 0..1);
            }
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &read,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            queue.submit(Some(encoder.finish()));
            let (sender, receiver) = std::sync::mpsc::channel();
            read.slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    sender.send(result).unwrap();
                });
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("poll");
            receiver.recv().unwrap().expect("map");
            let pixel = read.slice(..).get_mapped_range().unwrap()[..4].to_vec();
            assert_eq!(pixel, [255, 255, 255, 255], "channel stand-in was misread");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_picture_keeps_the_shifted_green() {
        let size = savers::STAND_IN;
        let pixels = stand_in_pixels(size);
        let at = |x: u32, y: u32, channel: usize| pixels[(((y * size + x) * 4) as usize) + channel];
        for y in [0, 17, 100, 255] {
            for x in [0, 37, 200, 255] {
                let sx = (x + 37) % size;
                let sy = (y + 17) % size;
                assert_eq!(at(x, y, 1), at(sx, sy, 0));
                assert_eq!(at(x, y, 3), at(sx, sy, 2));
            }
        }
        assert_ne!(at(0, 0, 0), at(0, 0, 2));
    }

    #[test]
    fn detail_scales_with_the_window() {
        // Logical 100 by 50, plus the 28px pad on each side, at a device scale of 2.
        assert_eq!(saver_target(100.0, 50.0, 2.0, 1.0), (312, 212));
        assert_eq!(saver_target(100.0, 50.0, 2.0, 0.5), (156, 106));
        assert_eq!(saver_target(100.0, 50.0, 2.0, 0.1), (31, 21));
    }

    #[test]
    fn preview_pauses_on_the_frame_you_leave() {
        let mut clock = CardClock {
            time: 0.0,
            frame: 0,
        };
        let mut held = false;

        let opening = preview_sample(&mut clock, &mut held, false, false, 2.0).unwrap();
        assert_eq!(opening.frame, 0);
        assert!(opening.time.abs() < 1e-6);
        assert!(held);

        // The first hover steps one frame. The long gap since the last draw stays out of it.
        let started = preview_sample(&mut clock, &mut held, true, false, 2.0).unwrap();
        assert_eq!(started.frame, 1);
        assert!((started.time - PREVIEW_FRAME).abs() < 1e-5);

        // The pointer staying uses the draw delta, and a stall moves at most a tenth of a second.
        let before = clock.time;
        let ran = preview_sample(&mut clock, &mut held, true, true, 0.5).unwrap();
        assert!((ran.time - (before + 0.1)).abs() < 1e-5);

        let paused = clock;
        assert!(preview_sample(&mut clock, &mut held, false, false, 2.0).is_none());
        assert_eq!(clock, paused);

        let again = preview_sample(&mut clock, &mut held, true, false, 2.0).unwrap();
        assert!((again.time - (paused.time + PREVIEW_FRAME)).abs() < 1e-4);

        // A resized card is painted again at the paused time, and that time stays put.
        held = false;
        let repaint = preview_sample(&mut clock, &mut held, false, false, 2.0).unwrap();
        assert!((repaint.time - clock.time).abs() < 1e-6);
        assert!(held);
    }

    #[test]
    fn a_short_turn_names_the_line_to_change() {
        let err = r#"Validation Error

Caused by:
  In Device::create_shader_module, label = 'darkness-conduit-golfed'
Shader validation error: Function [0] 'mainImage' is invalid
    ┌─ darkness-conduit-golfed:89:1
 89 │ void mainImage( out vec4 o, in vec2 f )
110 │         p.xy *= mat2(cos(p.z*.1683+vec4(0,-11,11,0))), // 3/4*2PI/28
    = Expression [134] is invalid
    = Composing 0's component type is not expected
"#;
        let hold = explain_compile(err);
        assert!(hold.card.contains("turn"));
        assert!(hold.page.contains("110"));
        assert!(hold.page.contains("mat2(c.xy, c.zw)"));
        assert!(hold.page.contains("twist(p.z*.1683)"));
        assert!(hold.page.contains("p.xy *= twist(p.z*.1683),"));
        assert!(hold.page.contains("vec4(0, -11, 11, 0)"));
        assert!(!hold.page.contains("void mainImage"));
    }

    #[test]
    fn another_compile_error_quotes_the_line_it_stopped_on() {
        let err = r#"
Shader validation error
    ┌─ harbor:3:1
  3 │ void mainImage(out vec4 fragColor, in vec2 fragCoord)
    │ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
 12 │     fragColor = vec4(unknown);
    │                 ^^^^^^^
    = unknown identifier
"#;
        let hold = explain_compile(err);
        assert!(!hold.card.to_lowercase().contains("turn"));
        assert!(hold.page.contains("line 12"));
        assert!(hold.page.contains("fragColor = vec4(unknown);"));
        assert!(!hold.page.contains("void mainImage"));
        assert!(hold.page.to_lowercase().contains("shadertoy"));
    }

    #[test]
    fn a_compile_error_without_a_line_still_explains() {
        let hold = explain_compile("Shader validation error: no span");
        assert!(!hold.card.is_empty());
        assert!(hold.page.to_lowercase().contains("could not read"));
        assert!(hold.page.to_lowercase().contains("shadertoy"));
    }
}
