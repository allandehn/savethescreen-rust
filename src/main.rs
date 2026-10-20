mod layout;
mod render;
mod savers;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

use layout::Move;
use render::{FrameClock, Renderer, Scene};
use savers::{shader_dir, Saver};

/// Shelf playback stays here. A running saver uses the pace remembered for that card.
const SAVER_FRAME: Duration = Duration::from_nanos(1_000_000_000 / 30);
const GALLERY_FRAME: Duration = Duration::from_nanos(1_000_000_000 / 12);

fn pace_interval(fps: u32) -> Duration {
    Duration::from_nanos(1_000_000_000 / u64::from(fps.max(1)))
}

enum Mode {
    Gallery,
    Saver(usize),
    /// A still note for a shader that did not compile. Windowed, with no picture.
    Hold(usize),
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
    savers: Vec<Saver>,
    shader_dir: String,
    mode: Mode,
    clock: Clock,
    hud: Hud,
    next_frame: Instant,
    cursor: (f32, f32),
    pointer_down: bool,
    /// False while the click that opened a saver is still held, so that click is not iMouse.
    mouse_live: bool,
    cursor_shown: bool,
    occluded: bool,
    scroll: f32,
    /// Highlighted card. Follows the pointer, and the arrow keys and hjkl.
    selected: usize,
    /// Detail percent for each saver id, from 10 to 100.
    levels: HashMap<String, u8>,
    /// Pace in frames per second, when it is not the opening 30.
    paces: HashMap<String, u32>,
    quality_path: PathBuf,
    shader_frame: i32,
    drawn_at: Instant,
}

impl App {
    fn frame_interval(&self) -> Option<Duration> {
        match self.mode {
            Mode::Saver(index) => Some(pace_interval(self.displayed_pace(index))),
            Mode::Hold(_) => None,
            Mode::Gallery => {
                if self.playing_card().is_some() {
                    Some(SAVER_FRAME)
                } else if self
                    .renderer
                    .as_ref()
                    .is_none_or(|renderer| renderer.thumbnails_pending())
                {
                    Some(GALLERY_FRAME)
                } else {
                    None
                }
            }
        }
    }

    /// The card under the pointer, if the shelf is up and the pointer is on one.
    fn playing_card(&self) -> Option<usize> {
        if !matches!(self.mode, Mode::Gallery) {
            return None;
        }
        let window = self.window.as_ref()?;
        let (width, height) = logical_size(window);
        let laid = layout::gallery(
            &self.savers,
            &self.shader_dir,
            width,
            height,
            self.scroll,
            None,
            None,
            None,
            self.screen_aspect(),
        );
        let index = laid.hit(self.cursor.0, self.cursor.1)?;
        self.savers
            .get(index)
            .is_some_and(|saver| saver.hold.is_none())
            .then_some(index)
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
        let (name, held) = {
            let Some(saver) = self.savers.get(index) else {
                return;
            };
            (saver.name.clone(), saver.hold.is_some())
        };
        self.selected = index;
        if held {
            window.set_fullscreen(None);
            self.mode = Mode::Hold(index);
        } else {
            self.mode = Mode::Saver(index);
            self.mouse_live = !self.pointer_down;
            self.hud.poke(Duration::from_millis(4200));
        }
        window.set_title(&format!("{name} · Save the Screen"));
        window.request_redraw();
    }

    fn nudge(&mut self, movement: Move) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let (width, height) = logical_size(&window);
        let cols = layout::gallery_cols(width);
        self.selected = layout::step_index(self.selected, self.savers.len(), cols, movement);
        let laid = layout::gallery(
            &self.savers,
            &self.shader_dir,
            width,
            height,
            self.scroll,
            None,
            None,
            None,
            self.screen_aspect(),
        );
        self.scroll = laid.scroll;
        if let Some(card) = laid.cards.iter().find(|card| card.index == self.selected) {
            let margin = 18.0;
            if card.y < margin {
                self.scroll = (self.scroll - (margin - card.y)).max(0.0);
            } else if card.y + card.span() > height - margin {
                self.scroll += card.y + card.span() - (height - margin);
            }
        }
        window.request_redraw();
    }

    fn cycle(&mut self, delta: isize) {
        let index = match self.mode {
            Mode::Saver(index) | Mode::Hold(index) => index,
            Mode::Gallery => return,
        };
        let count = self.savers.len() as isize;
        if count == 0 {
            return;
        }
        let next = (index as isize + delta).rem_euclid(count) as usize;
        self.open_saver(next);
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
        match Renderer::new(window.clone(), event_loop, &mut self.savers) {
            Ok(renderer) => {
                eprintln!(
                    "savethescreen on {}. Savers open at 30 fps and 50% of the window. Page Up and Page Down step the pace, up to the screen. {} loaded.",
                    renderer.adapter_name,
                    self.savers.len()
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
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => {
                window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = window.scale_factor() as f32;
                if scale > 0.0 {
                    self.cursor = (position.x as f32 / scale, position.y as f32 / scale);
                }
                if matches!(self.mode, Mode::Gallery) {
                    let (width, height) = logical_size(window);
                    let laid = layout::gallery(
                        &self.savers,
                        &self.shader_dir,
                        width,
                        height,
                        self.scroll,
                        None,
                        None,
                        None,
                        self.screen_aspect(),
                    );
                    if let Some(index) = laid.hit(self.cursor.0, self.cursor.1) {
                        self.selected = index;
                    }
                    window.request_redraw();
                }
                if matches!(self.mode, Mode::Saver(_)) {
                    self.hud.poke(Duration::from_millis(2800));
                }
            }
            WindowEvent::CursorLeft { .. } => {
                // A point no card contains, so the shelf returns to stills and can rest.
                self.cursor = (-1.0, -1.0);
                if matches!(self.mode, Mode::Gallery) {
                    window.request_redraw();
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
                if button == winit::event::MouseButton::Left {
                    self.pointer_down = state == ElementState::Pressed;
                    if !self.pointer_down {
                        self.mouse_live = true;
                    }
                }
                if state != ElementState::Pressed || button != winit::event::MouseButton::Left {
                    return;
                }
                match self.mode {
                    Mode::Gallery => {
                        let (width, height) = logical_size(window);
                        let layout = layout::gallery(
                            &self.savers,
                            &self.shader_dir,
                            width,
                            height,
                            self.scroll,
                            None,
                            None,
                            None,
                            self.screen_aspect(),
                        );
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
                    Mode::Hold(_) => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if let Some(movement) = nav_key(&event) {
                    match self.mode {
                        Mode::Gallery => self.nudge(movement),
                        Mode::Saver(_) => match movement {
                            Move::Previous => self.cycle(-1),
                            Move::Next => self.cycle(1),
                            Move::Up => self.nudge_quality(10),
                            Move::Down => self.nudge_quality(-10),
                        },
                        Mode::Hold(_) => match movement {
                            Move::Previous => self.cycle(-1),
                            Move::Next => self.cycle(1),
                            Move::Up | Move::Down => {}
                        },
                    }
                    return;
                }
                if let Some(up) = pace_key(&event) {
                    self.step_pace(up);
                    return;
                }
                if event.repeat {
                    return;
                }
                if is_back_key(&event) {
                    if matches!(self.mode, Mode::Gallery) {
                        event_loop.exit();
                    } else {
                        self.show_gallery();
                    }
                    return;
                }
                if is_gallery_key(&event) {
                    self.show_gallery();
                    return;
                }
                if let Some(percent) = quality_key(&event) {
                    let index = match self.mode {
                        Mode::Saver(index) | Mode::Hold(index) => index,
                        Mode::Gallery => self.selected,
                    };
                    self.set_level(index, percent);
                    return;
                }
                match self.mode {
                    Mode::Gallery => {
                        if matches!(event.logical_key, Key::Named(NamedKey::Enter)) {
                            self.open_saver(self.selected);
                        }
                    }
                    Mode::Saver(_) => {
                        if matches!(&event.logical_key, Key::Character(text) if text.eq_ignore_ascii_case("f"))
                        {
                            self.toggle_fullscreen();
                        }
                    }
                    Mode::Hold(_) => {}
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
        let Some(interval) = self.frame_interval() else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };
        if self.next_frame <= now {
            window.request_redraw();
            self.next_frame = now + interval;
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
        let logical = [size.width as f32 / scale, size.height as f32 / scale];
        let time = self.clock.secs();
        let now = Instant::now();
        let delta = now
            .saturating_duration_since(self.drawn_at)
            .as_secs_f32()
            .clamp(0.0, 0.1);
        self.drawn_at = now;
        let frame = self.shader_frame;
        self.shader_frame = self.shader_frame.saturating_add(1);
        let scene = match self.mode {
            Mode::Gallery => {
                let aspect = self.screen_aspect();
                let layout = layout::gallery(
                    &self.savers,
                    &self.shader_dir,
                    logical[0],
                    logical[1],
                    self.scroll,
                    None,
                    None,
                    None,
                    aspect,
                );
                self.scroll = layout.scroll;
                let hover = if self.savers.is_empty() {
                    None
                } else {
                    Some(self.selected.min(self.savers.len() - 1))
                };
                let detail = hover.map(|index| self.level_of(index));
                let pace = hover.map(|index| self.displayed_pace(index));
                let playing = layout.hit(self.cursor.0, self.cursor.1).filter(|&index| {
                    self.savers
                        .get(index)
                        .is_some_and(|saver| saver.hold.is_none())
                });
                Scene::Gallery {
                    hover,
                    scroll: layout.scroll,
                    detail,
                    pace,
                    playing,
                    aspect,
                }
            }
            Mode::Saver(index) => Scene::Saver {
                index,
                hud_opacity: self.hud.opacity(),
                detail: self.level_of(index),
                pace: self.displayed_pace(index),
            },
            Mode::Hold(index) => Scene::Hold { index },
        };
        let show_cursor = match self.mode {
            Mode::Gallery | Mode::Hold(_) => true,
            Mode::Saver(_) => self.hud.opacity() > 0.04,
        };
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        renderer.draw(
            logical,
            scale,
            &FrameClock {
                time,
                delta,
                frame,
                pointer: [self.cursor.0, self.cursor.1],
                pointer_down: self.pointer_down && self.mouse_live,
            },
            scene,
            &self.savers,
            &self.shader_dir,
        );
        self.sync_cursor(show_cursor);
    }
}

fn monitor_aspect(window: &Window) -> f32 {
    window
        .current_monitor()
        .or_else(|| window.primary_monitor())
        .map(|monitor| {
            let size = monitor.size();
            layout::screen_aspect(size.width as f32, size.height as f32)
        })
        .unwrap_or(16.0 / 9.0)
}

fn logical_size(window: &Window) -> (f32, f32) {
    let size = window.inner_size();
    let scale = window.scale_factor() as f32;
    if scale <= 0.0 {
        return (size.width as f32, size.height as f32);
    }
    (size.width as f32 / scale, size.height as f32 / scale)
}

impl App {
    fn screen_aspect(&self) -> f32 {
        self.window
            .as_deref()
            .map(monitor_aspect)
            .unwrap_or(16.0 / 9.0)
    }

    fn level_of(&self, index: usize) -> u8 {
        self.savers
            .get(index)
            .and_then(|saver| self.levels.get(&saver.id).copied())
            .unwrap_or(savers::DEFAULT_QUALITY)
    }

    fn set_level(&mut self, index: usize, percent: u8) {
        let Some(id) = self.savers.get(index).map(|saver| saver.id.clone()) else {
            return;
        };
        let percent = savers::snap_quality(percent);
        if self.levels.get(&id).copied() != Some(percent) {
            self.levels.insert(id, percent);
            if let Err(err) = savers::save_quality(&self.quality_path, &self.levels, &self.paces) {
                eprintln!("savethescreen: {}: {err}", self.quality_path.display());
            }
        }
        if matches!(self.mode, Mode::Saver(_)) {
            self.hud.poke(Duration::from_millis(4200));
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn refresh_hz(&self) -> Option<u32> {
        let window = self.window.as_deref()?;
        let mhz = window
            .current_monitor()
            .or_else(|| window.primary_monitor())
            .and_then(|monitor| monitor.refresh_rate_millihertz());
        savers::refresh_hz(mhz)
    }

    /// The pace this card draws at on the monitor it is on now.
    fn displayed_pace(&self, index: usize) -> u32 {
        let stored = self
            .savers
            .get(index)
            .and_then(|saver| self.paces.get(&saver.id).copied())
            .unwrap_or(savers::DEFAULT_PACE);
        savers::effective_pace(stored, self.refresh_hz())
    }

    fn step_pace(&mut self, up: bool) {
        if matches!(self.mode, Mode::Hold(_)) {
            return;
        }
        let index = match self.mode {
            Mode::Saver(index) => index,
            Mode::Gallery => self.selected,
            Mode::Hold(_) => return,
        };
        let Some(id) = self
            .savers
            .get(index)
            .and_then(|saver| saver.hold.is_none().then(|| saver.id.clone()))
        else {
            return;
        };
        let stored = self.paces.get(&id).copied().unwrap_or(savers::DEFAULT_PACE);
        let next = savers::step_pace(stored, self.refresh_hz(), up);
        if next != stored {
            if next == savers::DEFAULT_PACE {
                self.paces.remove(&id);
            } else {
                self.paces.insert(id, next);
            }
            if let Err(err) = savers::save_quality(&self.quality_path, &self.levels, &self.paces) {
                eprintln!("savethescreen: {}: {err}", self.quality_path.display());
            }
        }
        if matches!(self.mode, Mode::Saver(_)) {
            self.hud.poke(Duration::from_millis(4200));
        }
        self.next_frame = Instant::now();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn nudge_quality(&mut self, delta: i32) {
        let Mode::Saver(index) = self.mode else {
            return;
        };
        let next = i32::from(self.level_of(index)) + delta;
        let percent = if next < 10 { 10 } else { next as u8 };
        self.set_level(index, percent);
    }
}

fn nav_key(event: &winit::event::KeyEvent) -> Option<Move> {
    match &event.logical_key {
        Key::Named(NamedKey::ArrowLeft) => Some(Move::Previous),
        Key::Named(NamedKey::ArrowRight) => Some(Move::Next),
        Key::Named(NamedKey::ArrowUp) => Some(Move::Up),
        Key::Named(NamedKey::ArrowDown) => Some(Move::Down),
        Key::Character(text) => match text.to_ascii_lowercase().as_str() {
            "h" => Some(Move::Previous),
            "l" => Some(Move::Next),
            "k" => Some(Move::Up),
            "j" => Some(Move::Down),
            _ => None,
        },
        _ => None,
    }
}

fn quality_key(event: &winit::event::KeyEvent) -> Option<u8> {
    let percent = match event.physical_key {
        PhysicalKey::Code(code) => match code {
            KeyCode::Digit1 | KeyCode::Numpad1 => Some(10),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some(20),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some(30),
            KeyCode::Digit4 | KeyCode::Numpad4 => Some(40),
            KeyCode::Digit5 | KeyCode::Numpad5 => Some(50),
            KeyCode::Digit6 | KeyCode::Numpad6 => Some(60),
            KeyCode::Digit7 | KeyCode::Numpad7 => Some(70),
            KeyCode::Digit8 | KeyCode::Numpad8 => Some(80),
            KeyCode::Digit9 | KeyCode::Numpad9 => Some(90),
            KeyCode::Digit0 | KeyCode::Numpad0 => Some(100),
            _ => None,
        },
        _ => None,
    };
    if percent.is_some() {
        return percent;
    }
    match &event.logical_key {
        Key::Character(text) => match text.as_str() {
            "1" => Some(10),
            "2" => Some(20),
            "3" => Some(30),
            "4" => Some(40),
            "5" => Some(50),
            "6" => Some(60),
            "7" => Some(70),
            "8" => Some(80),
            "9" => Some(90),
            "0" => Some(100),
            _ => None,
        },
        _ => None,
    }
}

/// Page Up steps the pace toward the screen. Page Down steps it back toward 30.
fn pace_key(event: &winit::event::KeyEvent) -> Option<bool> {
    match event.physical_key {
        PhysicalKey::Code(KeyCode::PageUp) => return Some(true),
        PhysicalKey::Code(KeyCode::PageDown) => return Some(false),
        _ => {}
    }
    match &event.logical_key {
        Key::Named(NamedKey::PageUp) => Some(true),
        Key::Named(NamedKey::PageDown) => Some(false),
        _ => None,
    }
}

fn is_back_key(event: &winit::event::KeyEvent) -> bool {
    if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
        return true;
    }
    matches!(&event.logical_key, Key::Character(text) if text.eq_ignore_ascii_case("q"))
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
    let quality_path = savers::quality_path();
    let (levels, paces) = savers::load_quality(&quality_path);
    let mut app = App {
        window: None,
        renderer: None,
        savers: savers::load(),
        shader_dir: shader_dir().display().to_string(),
        levels,
        paces,
        quality_path,
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
        pointer_down: false,
        mouse_live: true,
        cursor_shown: true,
        occluded: false,
        selected: 0,
        shader_frame: 0,
        drawn_at: Instant::now(),
    };
    event_loop.run_app(&mut app).expect("run");
}
