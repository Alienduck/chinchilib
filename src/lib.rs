use std::usize;

pub use pixels;
use pixels::{Pixels, SurfaceTexture};
pub use rgb;
use std::cell::RefCell;
use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;
pub use winit;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowExtWebSys;
use winit::window::{Window, WindowId};

/// Mapping for the keys that are recognized. They are centered an AZERTY keyboard's essential keys
/// needed for games.
/// TODO: for keys that correspondond to a character, use a unique enum variant that contains a
/// SmolStr.
#[derive(Eq, Hash, PartialEq)]
pub enum Key {
    KeyA,
    KeyZ,
    KeyE,
    KeyQ,
    KeyS,
    KeyD,
    KeyW,
    KeyX,
    KeyC,
    Up,
    Down,
    Left,
    Right,
}

impl std::convert::TryFrom<&winit::keyboard::Key> for Key {
    type Error = ();
    fn try_from(value: &winit::keyboard::Key) -> Result<Self, ()> {
        use winit::keyboard::{Key as WKey, NamedKey as WNamedKey};
        match value {
            WKey::Named(WNamedKey::ArrowLeft) => Some(Key::Left),
            WKey::Named(WNamedKey::ArrowRight) => Some(Key::Right),
            WKey::Named(WNamedKey::ArrowUp) => Some(Key::Up),
            WKey::Named(WNamedKey::ArrowDown) => Some(Key::Down),
            WKey::Character(name) if name == "q" => Some(Key::KeyQ),
            WKey::Character(name) if name == "d" => Some(Key::KeyD),
            WKey::Character(name) if name == "z" => Some(Key::KeyZ),
            WKey::Character(name) if name == "s" => Some(Key::KeyS),
            WKey::Character(name) if name == "a" => Some(Key::KeyA),
            WKey::Character(name) if name == "e" => Some(Key::KeyE),
            WKey::Character(name) if name == "w" => Some(Key::KeyW),
            WKey::Character(name) if name == "x" => Some(Key::KeyX),
            WKey::Character(name) if name == "c" => Some(Key::KeyC),
            _ => None,
        }
        .ok_or(())
    }
}

/// Everyting about the window. Pixels and Window are options because they
/// are constructed on "resume" and cannot be construted earlier
pub struct WinitHandler {
    winfbx: Rc<RefCell<Option<WinFbx>>>,
    init_done: bool,
    pub width: usize,
    pub height: usize,
    last_frame: Instant,
    tick: std::time::Duration,
    /// Set to true if your app has something special to do at every tick even if there are no user
    /// events. This can be used if you have physics or an animation to run. Defaults to false to
    /// preserve performance.
    always_tick: bool,
    app: Option<Box<dyn GfxApp>>,
    cursor_pos: (f64, f64),
}

fn hz_to_nanosec_period(hz: u16) -> u64 {
    let nano_period = 1.0 / hz as f64 * 1_000_000_000.0;
    nano_period as u64
}

#[cfg(test)]
mod test {
    #[test]
    fn hz_to_nanosec_period() {
        assert_eq!(super::hz_to_nanosec_period(60), 16_666_666);
        assert_eq!(super::hz_to_nanosec_period(1), 1_000_000_000);
    }
}

impl WinitHandler {
    /// Create a new handler with an app, a window size and a desired tick rate. Run app with
    /// `.run()`
    pub fn new(app: Box<dyn GfxApp>, size: (usize, usize), tick_per_second: u16) -> Self {
        let nsec_period = hz_to_nanosec_period(tick_per_second);
        Self {
            winfbx: Rc::new(RefCell::new(None)), // Initialise vide
            init_done: false,
            width: size.0,
            height: size.1,
            last_frame: Instant::now(),
            tick: std::time::Duration::from_nanos(nsec_period),
            app: Some(app),
            cursor_pos: (0.0, 0.0),
            always_tick: false,
        }
    }

    pub fn run(&mut self) -> Result<(), winit::error::EventLoopError> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);
        event_loop.run_app(self)?;
        Ok(())
    }

    /// Set to true if your app has something special to do at every tick even if there are no user
    /// events. This can be used if you have physics or an animation to run. Defaults to false to
    /// preserve performance.
    pub fn set_always_tick(&mut self, val: bool) {
        self.always_tick = val;
    }

    pub fn new_with_winfbx(winfbx: WinFbx, size: (usize, usize), tick_per_second: u16) -> Self {
        let nsec_period = hz_to_nanosec_period(tick_per_second);
        Self {
            winfbx: Rc::new(RefCell::new(Some(winfbx))), // Injection here
            init_done: false,
            width: size.0,
            height: size.1,
            last_frame: Instant::now(),
            tick: std::time::Duration::from_nanos(nsec_period),
            app: None, // App already in winfbx (could remove field)
            cursor_pos: (0.0, 0.0),
            always_tick: false,
        }
    }
}

impl winit::application::ApplicationHandler for WinitHandler {
    /// Resume gets called when window gets loaded for the first time
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.init_done {
            return;
        }
        self.init_done = true;

        log::info!("Moteur démarré. Création de la fenêtre...");

        if let Some(app) = self.app.take() {
            let width = self.width;
            let height = self.height;

            let winfbx_handle = self.winfbx.clone();

            let mut attr = Window::default_attributes()
                .with_inner_size(winit::dpi::LogicalSize::new(width as f64, height as f64)) // LogicalSize to auto
                .with_title("Chinchilib App");

            #[cfg(target_arch = "wasm32")]
            {
                use winit::platform::web::WindowAttributesExtWebSys;
                // Désactive le suivi automatique de la taille du canvas par winit
                attr = attr.with_prevent_default(true);
            }

            let window = event_loop
                .create_window(attr)
                .expect("Impossible de créer la fenêtre");

            #[cfg(target_arch = "wasm32")]
            {
                use web_sys::HtmlElement;
                if let Some(canvas) = window.canvas() {
                    let win = web_sys::window().unwrap();
                    let doc = win.document().unwrap();

                    if let Ok(canvas_el) = canvas.clone().dyn_into::<web_sys::HtmlCanvasElement>() {
                        canvas_el.set_width(width as u32);
                        canvas_el.set_height(height as u32);
                        let _ = canvas_el
                            .style()
                            .set_property("width", &format!("{}px", width));
                        let _ = canvas_el
                            .style()
                            .set_property("height", &format!("{}px", height));
                    }

                    let container = doc.get_element_by_id("wasm-canvas-container");

                    if let Some(dst) = container {
                        let _ = dst.append_child(&canvas);
                        log::info!("Canvas injecté dans #wasm-canvas-container");
                    } else {
                        let _ = doc.body().unwrap().append_child(&canvas);
                        log::info!("Canvas injecté dans body (container non trouvé)");
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen_futures::spawn_local;
                spawn_local(async move {
                    log::info!("Initialisation Pixels (Async)...");
                    let winfbx = WinFbx::new_async(window, width, height, app).await;

                    *winfbx_handle.borrow_mut() = Some(winfbx);
                    log::info!("Initialisation terminée !");
                });
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                let winfbx = WinFbx::new(event_loop, width, height, app);
                *winfbx_handle.borrow_mut() = Some(winfbx);
            }
        }
    }

    /// Instead of redrawing for every event, or every keyprss, we only try to
    /// render after all evens have been processed.
    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Ok(mut borrow) = self.winfbx.try_borrow_mut() {
            if let Some(fbx) = borrow.as_mut() {
                if fbx.done() {
                    event_loop.exit();
                    return;
                }

                let now = Instant::now();
                if now.duration_since(self.last_frame) >= self.tick {
                    self.last_frame = now;
                    fbx.on_tick();
                    fbx.window.request_redraw();
                } else {
                    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: WindowId,
        event: winit::event::WindowEvent,
    ) {
        if let Ok(mut borrow) = self.winfbx.try_borrow_mut() {
            if let Some(fbx) = borrow.as_mut() {
                use winit::event::WindowEvent;
                match event {
                    WindowEvent::CloseRequested => event_loop.exit(),
                    WindowEvent::Resized(size) => {
                        #[cfg(target_arch = "wasm32")]
                        {
                            if size.width > 0
                                && size.width <= 4096
                                && size.height > 0
                                && size.height <= 4096
                            {
                                fbx.process_resize(size);
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        fbx.process_resize(size);
                    }
                    WindowEvent::KeyboardInput { event, .. } if !event.repeat => {
                        fbx.process_kbd_input(event, event_loop)
                    }
                    WindowEvent::RedrawRequested => fbx.on_redraw(),
                    WindowEvent::CursorMoved { position, .. } => {
                        self.cursor_pos = (position.x, position.y);
                    }
                    WindowEvent::MouseInput { state, .. } if state.is_pressed() => {}
                    _ => {}
                }
            }
        }
    }
}

pub fn put_pixel(frame: &mut [u8], width: usize, x: usize, y: usize, color: rgb::RGBA8) {
    use rgb::*;
    let idx = width * y + x;
    frame.as_rgba_mut()[idx] = color;
}

/// Manages the actual winit::Window, the Pixels, handles resizes, records pressed keys into a
/// custom structure and call the given app tick and draw methods.
pub struct WinFbx {
    pub window: Window,
    pixels: Pixels,
    pause: bool,
    height: usize,
    width: usize,
    pressed_keys: std::collections::HashSet<Key>,
    released_keys: std::collections::HashSet<Key>,
    needs_render: bool,
    app: Box<dyn GfxApp>,
}

impl WinFbx {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        width: usize,
        height: usize,
        app: Box<dyn GfxApp>,
    ) -> Self {
        let mut attr = Window::default_attributes();
        let size = winit::dpi::PhysicalSize::new(width as u16, height as u16);
        attr = attr.with_inner_size(size).with_title("Box");
        let window = event_loop.create_window(attr).unwrap();

        let surface_texture = SurfaceTexture::new(width as u32, height as u32, &window);
        let mut pixels = Pixels::new(width as u32, height as u32, surface_texture).unwrap();

        pixels.clear_color(pixels::wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });

        Self {
            window,
            pixels,
            height,
            width,
            pause: false,
            pressed_keys: std::collections::HashSet::new(),
            released_keys: std::collections::HashSet::new(),
            needs_render: true,
            app,
        }
    }

    // VERSION WEB (Asynchrone) - Utilisée dans 'start_game' (runner)
    #[cfg(target_arch = "wasm32")]
    pub async fn new_async(
        window: Window, // On reçoit la fenêtre déjà créée
        width: usize,
        height: usize,
        app: Box<dyn GfxApp>,
    ) -> Self {
        let surface_texture = SurfaceTexture::new(width as u32, height as u32, &window);

        // C'est ici qu'on utilise le .await qui posait problème
        let mut pixels = Pixels::new_async(width as u32, height as u32, surface_texture)
            .await
            .expect("Pixels::new_async failed");

        pixels.clear_color(pixels::wgpu::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });

        Self {
            window,
            pixels,
            height,
            width,
            pause: false,
            pressed_keys: std::collections::HashSet::new(),
            released_keys: std::collections::HashSet::new(),
            needs_render: true,
            app,
        }
    }

    fn on_redraw(&mut self) {
        if self.needs_render {
            self.app.draw(&mut self.pixels, self.width);
        }

        if let Err(err) = self.pixels.render() {
            log::error!("failed to render with error {}", err);
            return;
        }

        self.needs_render = false;
    }

    fn done(&self) -> bool {
        self.app.done() == DoneStatus::Exit
    }

    fn on_tick(&mut self) {
        if self.app.done() == DoneStatus::NotDone {
            let app_requests_render = self
                .app
                .on_tick(&self.pressed_keys, (self.width, self.height));

            if app_requests_render {
                self.needs_render = true;
            }
        }
        self.pressed_keys
            .retain(|candidate| !self.released_keys.contains(candidate));
        self.released_keys.clear();
    }

    fn process_kbd_input(
        &mut self,
        event: winit::event::KeyEvent,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        use winit::keyboard::{Key, NamedKey};
        if let Ok(my_key) = (&event.logical_key).try_into() {
            if event.state == winit::event::ElementState::Pressed {
                self.pressed_keys.insert(my_key);
            } else if event.state == winit::event::ElementState::Released {
                self.released_keys.insert(my_key);
            }
        };
        if event.state == winit::event::ElementState::Pressed {
            match event.logical_key {
                Key::Named(NamedKey::Escape) => event_loop.exit(),
                Key::Named(NamedKey::Space) => {
                    self.pause = !self.pause;
                }
                _ => {}
            }
        }
    }

    fn process_resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if let Err(err) = self.pixels.resize_surface(size.width, size.height) {
            log::error!("resize_surface failed: {err}");
            return;
        }
        self.window.request_redraw();
        self.needs_render = true;
    }
}

#[derive(Eq, PartialEq)]
pub enum DoneStatus {
    /// The program should quit, the app has nothing left to do.
    Exit,
    /// The program should remain open, but the app is done. Useful when you want the result of the
    /// app to stay on the screen. On `Remain` the `draw` and `on_tick` methodes will not be called
    /// anymore.
    Remain,
    /// The program should continue, the app is not done.
    NotDone,
}

pub trait GfxApp {
    /// Every tick, this method gets called with currently pressed keys. Released keys during the tick are considered still pressed. But will be removed after this call.
    fn on_tick(
        &mut self,
        pressed_keys: &std::collections::HashSet<Key>,
        window_size: (usize, usize),
    ) -> bool;

    /// You get the pixel array, so you can draw on it before the render.
    fn draw(&mut self, pixels: &mut Pixels, width: usize);

    /// Indicate if the app logic is done and if the program should remain or exit. For oneshot
    /// drawing, return `DoneStatus::Remain` so that the result stays on screen.
    fn done(&self) -> DoneStatus;
}
