//! A native window for the host: surfaces stacked in a column like the DOM page (#surfaces), painted by
//! the renderer at the screen's pixel density and presented with softbuffer. Pointer and keys go to the
//! host, which turns them into the same events the DOM renderer emits.
use crate::mcp::{self, Wake, Waker};
use mx_render::host::{Host, Placed, State, CLOSE, COL_W, GAP};
use mx_render::Hit;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

struct Proxy(EventLoopProxy<Wake>);

impl Waker for Proxy {
    fn wake(&self, w: Wake) {
        let _ = self.0.send_event(w);
    }
}

struct App {
    host: Arc<Mutex<Host>>,
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    scroll: f32,
    cursor: (f32, f32),
    placed: Vec<Placed>,
    focused: Option<String>,
}

pub fn run(host: Arc<Mutex<Host>>) {
    let event_loop = EventLoop::<Wake>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let h = host.clone();
    std::thread::spawn(move || mcp::serve(h, Some(Box::new(Proxy(proxy)))));
    let mut app = App { host, window: None, surface: None, scroll: 0.0, cursor: (0.0, 0.0), placed: vec![], focused: None };
    event_loop.run_app(&mut app).expect("run");
}

impl App {
    fn scale(&self) -> f32 {
        self.window.as_ref().map(|w| w.scale_factor() as f32).unwrap_or(1.0)
    }

    fn set_scale(&self) {
        let sc = self.scale();
        let mut h = self.host.lock().unwrap();
        if h.renderer.scale != sc {
            h.renderer.scale = sc;
            let ids: Vec<String> = h.entries.iter().filter(|e| e.state == State::Live).map(|e| e.surface.id.clone()).collect();
            for id in ids {
                let _ = h.repaint(&id);
            }
        }
    }

    fn redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn paint(&mut self) {
        let Some(window) = self.window.clone() else { return };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        let sc = window.scale_factor() as f32;
        let (pm, placed) = self.host.lock().unwrap().compose(w.get(), h.get(), sc, self.scroll);
        self.placed = placed;
        let Some(surface) = self.surface.as_mut() else { return };
        surface.resize(w, h).unwrap();
        let mut buf = surface.buffer_mut().unwrap();
        for (i, px) in pm.pixels().iter().enumerate() {
            buf[i] = ((px.red() as u32) << 16) | ((px.green() as u32) << 8) | px.blue() as u32;
        }
        buf.present().unwrap();
    }

    fn css_w(&self) -> f32 {
        self.window.as_ref().map(|w| w.inner_size().width as f32 / self.scale()).unwrap_or(COL_W)
    }

    /// The live surface under a window point, with the point in surface coordinates.
    fn under(&self, x: f32, y: f32) -> Option<(&Placed, f32, f32)> {
        Host::under(&self.placed, self.css_w(), x, y).map(|(i, rx, ry)| (&self.placed[i], rx, ry))
    }

    fn tick(&mut self) {
        let view_h = self.window.as_ref().map(|w| w.inner_size().height as f32 / self.scale()).unwrap_or(900.0);
        let mut h = self.host.lock().unwrap();
        for p in &self.placed {
            if p.live {
                let visible = (p.y + p.h).min(view_h) - p.y.max(0.0);
                h.set_in_view(&p.id, visible >= p.h / 2.0);
            }
        }
        let before: Vec<State> = h.entries.iter().map(|e| e.state).collect();
        h.tick(mcp::now_ms());
        let after: Vec<State> = h.entries.iter().map(|e| e.state).collect();
        drop(h);
        if before != after {
            self.redraw();
        }
    }
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Manifest").with_inner_size(LogicalSize::new(COL_W as f64, 900.0));
        let window = Rc::new(el.create_window(attrs).expect("window"));
        let ctx = Context::new(window.clone()).expect("softbuffer context");
        self.surface = Some(Surface::new(&ctx, window.clone()).expect("softbuffer surface"));
        self.window = Some(window);
        self.set_scale();
        self.redraw();
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: Wake) {
        match event {
            Wake::Redraw => self.redraw(),
            Wake::Quit => el.exit(),
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let animating = self.host.lock().unwrap().animating();
        el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(if animating { 16 } else { 250 })));
        self.tick();
        if animating {
            self.redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.paint(),
            WindowEvent::Resized(_) => self.redraw(),
            WindowEvent::ScaleFactorChanged { .. } => {
                self.set_scale();
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let sc = self.scale();
                self.cursor = (position.x as f32 / sc, position.y as f32 / sc);
                let icon = match self.under(self.cursor.0, self.cursor.1) {
                    Some((p, rx, ry)) if rx > p.w - CLOSE && ry < CLOSE => CursorIcon::Pointer,
                    Some((p, rx, ry)) => match self.host.lock().unwrap().hit(&p.id, rx, ry).map(|(h, _)| h) {
                        Some(Hit::Field(_)) => CursorIcon::Text,
                        Some(Hit::None) | None => CursorIcon::Default,
                        Some(_) => CursorIcon::Pointer,
                    },
                    None => CursorIcon::Default,
                };
                if let Some(w) = &self.window {
                    w.set_cursor(icon);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / self.scale(),
                };
                let total: f32 = self.placed.iter().map(|p| p.h + GAP).sum::<f32>() + GAP;
                let view = self.window.as_ref().map(|w| w.inner_size().height as f32 / self.scale()).unwrap_or(900.0);
                self.scroll = (self.scroll - dy).clamp(0.0, (total - view).max(0.0));
                self.redraw();
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                let (x, y) = self.cursor;
                let Some((p, rx, ry)) = self.under(x, y) else { return };
                let (id, w) = (p.id.clone(), p.w);
                let mut h = self.host.lock().unwrap();
                if rx > w - CLOSE && ry < CLOSE {
                    h.dissolve(&id, "user");
                } else {
                    h.click(&id, rx, ry);
                    self.focused = Some(id);
                }
                drop(h);
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let Some(id) = self.focused.clone() else { return };
                let text: String = match &event.logical_key {
                    Key::Named(NamedKey::Backspace) => "\u{8}".into(),
                    Key::Named(NamedKey::Delete) => "\u{7f}".into(),
                    Key::Named(NamedKey::ArrowLeft) => "\u{1}".into(),
                    Key::Named(NamedKey::ArrowRight) => "\u{2}".into(),
                    Key::Named(NamedKey::Home) => "\u{3}".into(),
                    Key::Named(NamedKey::End) => "\u{4}".into(),
                    Key::Named(NamedKey::Escape) => "\u{1b}".into(),
                    Key::Named(NamedKey::Enter) => "\n".into(),
                    Key::Named(NamedKey::Space) => " ".into(),
                    Key::Character(c) => c.to_string(),
                    _ => return,
                };
                self.host.lock().unwrap().key(&id, &text);
                self.redraw();
            }
            _ => {}
        }
    }
}
