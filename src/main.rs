mod layout;
mod render;
mod savers;

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use render::{Renderer, Scene};
use savers::SAVERS;

const SAVER_FRAME: Duration = Duration::from_nanos(1_000_000_000 / 30);
const GALLERY_FRAME: Duration = Duration::from_nanos(1_000_000_000 / 12);

enum Mode {
    Gallery,
    Saver(usize),
}

struct Clock {
    epoch: Instant,
    paused: Duration,
    hidden_at: Option<Instant>,
}

impl Clock {
    fn secs(&self) -> f32 {
        let mut elapsed = self.epoch.elapsed().saturating_sub(self.paused);
        if let Some(started) = self.hidden_at {
            elapsed = elapsed.saturating_sub(started.elapsed());
        }
        elapsed.as_secs_f32()
    }

    fn hide(&mut self) {
        if self.hidden_at.is_none() {
            self.hidden_at = Some(Instant::now());
        }
    }

    fn show(&mut self) {
        if let Some(started) = self.hidden_at.take() {
            self.paused += started.elapsed();
        }
    }
}

struct Hud {
    hold_until: Instant,
}

impl Hud {
    fn poke(&mut self, visible_for: Duration) {
        self.hold_until = Instant::now() + visible_for;
    }

    fn opacity(&self) -> f32 {
        let now = Instant::now();
        if now <= self.hold_until {
            1.0
        } else {
            (1.0 - (now - self.hold_until).as_secs_f32() / 0.9).max(0.0)
        }
    }
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    mode: Mode,
    clock: Clock,
    hud: Hud,
    next_frame: Instant,
    cursor: (f32, f32),
    cursor_shown: bool,
    occluded: bool,
    scroll: f32,
}

impl App {
    fn frame_interval(&self) -> Duration {
        match self.mode {
            Mode::Gallery => GALLERY_FRAME,
            Mode::Saver(_) => SAVER_FRAME,
        }
    }

    fn show_gallery(&mut self) {
        let Some(window) = &self.window else { return };
        window.set_fullscreen(None);
        self.mode = Mode::Gallery;
        window.set_title("Save the Screen");
        window.request_redraw();
    }

    fn open_saver(&mut self, index: usize) {
        let Some(window) = &self.window else { return };
        self.mode = Mode::Saver(index);
        self.hud.poke(Duration::from_millis(4200));
        window.set_title(&format!("{} · Save the Screen", SAVERS[index].name));
        window.request_redraw();
    }

    fn toggle_fullscreen(&mut self) {
        let Some(window) = &self.window else { return };
        if window.fullscreen().is_some() {
            window.set_fullscreen(None);
        } else {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
    }

    fn sync_cursor(&mut self, show: bool) {
        if show == self.cursor_shown {
            return;
        }
        if let Some(window) = &self.window {
            window.set_cursor_visible(show);
            self.cursor_shown = show;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attributes = Window::default_attributes()
            .with_title("Save the Screen")
            .with_inner_size(LogicalSize::new(1440.0, 920.0));
        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;
            attributes = attributes.with_name("savethescreen", "Save the Screen");
        }
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        match Renderer::new(window.clone(), event_loop) {
            Ok(renderer) => {
                eprintln!(
                    "savethescreen on {}. Savers draw at 30 fps with a 1600px long edge.",
                    renderer.adapter_name
                );
                self.renderer = Some(renderer);
                self.window = Some(window);
                self.next_frame = Instant::now();
            }
            Err(err) => {
                eprintln!("savethescreen: {err}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = &self.window else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Occluded(hidden) => {
                self.occluded = hidden;
                if hidden {
                    self.clock.hide();
                } else {
                    self.clock.show();
                    self.next_frame = Instant::now();
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    self.clock.hide();
                } else if self.occluded {
                    // A real size means the window is back, even if occlusion was missed.
                    self.occluded = false;
                    self.clock.show();
                }
                window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = window.scale_factor() as f32;
                if scale > 0.0 {
                    self.cursor = (position.x as f32 / scale, position.y as f32 / scale);
                }
                if matches!(self.mode, Mode::Saver(_)) {
                    self.hud.poke(Duration::from_millis(2800));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !matches!(self.mode, Mode::Gallery) {
                    return;
                }
                let scale = window.scale_factor() as f32;
                let dy = match delta {
                    // Positive moves the content down, which reveals what is above.
                    MouseScrollDelta::LineDelta(_, y) => y * 64.0,
                    MouseScrollDelta::PixelDelta(pos) => {
                        if scale > 0.0 {
                            pos.y as f32 / scale
                        } else {
                            pos.y as f32
                        }
                    }
                };
                if dy != 0.0 {
                    self.scroll = (self.scroll - dy).max(0.0);
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state != ElementState::Pressed || button != winit::event::MouseButton::Left {
                    return;
                }
                match self.mode {
                    Mode::Gallery => {
                        let (width, height) = logical_size(window);
                        let layout = layout::gallery(width, height, self.scroll);
                        self.scroll = layout.scroll;
                        if let Some(index) = layout.hit(self.cursor.0, self.cursor.1) {
                            self.open_saver(index);
                        }
                    }
                    Mode::Saver(_) => {
                        self.hud.poke(Duration::from_millis(2800));
                        if window.fullscreen().is_none() {
                            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                if is_gallery_key(&event) {
                    self.show_gallery();
                    return;
                }
                if let Some(index) = digit_index(event.physical_key) {
                    self.open_saver(index);
                    return;
                }
                if !matches!(self.mode, Mode::Saver(_)) {
                    return;
                }
                match event.logical_key {
                    Key::Character(ref text) if text.eq_ignore_ascii_case("f") => {
                        self.toggle_fullscreen();
                    }
                    Key::Named(NamedKey::Escape) => {
                        window.set_fullscreen(None);
                        self.hud.poke(Duration::from_millis(2800));
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = &self.window else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        if self.occluded || window.inner_size().width == 0 {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        if self.next_frame <= now {
            window.request_redraw();
            self.next_frame = now + self.frame_interval();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

impl App {
    fn draw(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let scale = window.scale_factor() as f32;
        let logical = [
            size.width as f32 / scale,
            size.height as f32 / scale,
        ];
        let time = self.clock.secs();
        let scene = match self.mode {
            Mode::Gallery => {
                let layout = layout::gallery(logical[0], logical[1], self.scroll);
                self.scroll = layout.scroll;
                Scene::Gallery {
                    hover: layout.hit(self.cursor.0, self.cursor.1),
                    scroll: layout.scroll,
                }
            }
            Mode::Saver(index) => Scene::Saver {
                index,
                hud_opacity: self.hud.opacity(),
            },
        };
        let show_cursor = match self.mode {
            Mode::Gallery => true,
            Mode::Saver(_) => self.hud.opacity() > 0.04,
        };
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        renderer.draw(logical, scale, time, scene);
        self.sync_cursor(show_cursor);
    }
}

fn logical_size(window: &Window) -> (f32, f32) {
    let size = window.inner_size();
    let scale = window.scale_factor() as f32;
    if scale <= 0.0 {
        return (size.width as f32, size.height as f32);
    }
    (size.width as f32 / scale, size.height as f32 / scale)
}

fn digit_index(key: PhysicalKey) -> Option<usize> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    Some(match code {
        KeyCode::Digit1 => 0,
        KeyCode::Digit2 => 1,
        KeyCode::Digit3 => 2,
        KeyCode::Digit4 => 3,
        KeyCode::Digit5 => 4,
        KeyCode::Digit6 => 5,
        KeyCode::Digit7 => 6,
        KeyCode::Digit8 => 7,
        KeyCode::Digit9 => 8,
        KeyCode::Digit0 => 9,
        _ => return None,
    })
}

fn is_gallery_key(event: &winit::event::KeyEvent) -> bool {
    if let PhysicalKey::Code(KeyCode::Backquote) = event.physical_key {
        return true;
    }
    matches!(
        &event.logical_key,
        Key::Character(text) if matches!(text.as_str(), "`" | "~" | "½" | "§")
    )
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        window: None,
        renderer: None,
        mode: Mode::Gallery,
        clock: Clock {
            epoch: Instant::now(),
            paused: Duration::ZERO,
            hidden_at: None,
        },
        scroll: 0.0,
        hud: Hud {
            hold_until: Instant::now(),
        },
        next_frame: Instant::now(),
        cursor: (0.0, 0.0),
        cursor_shown: true,
        occluded: false,
    };
    event_loop.run_app(&mut app).expect("run");
}
