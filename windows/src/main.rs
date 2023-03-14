// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod config;
mod settings_window;
mod surface;
mod wallpaper;

use cli::Mode;
use config::Config;
use flux::Flux;

use std::collections::HashMap;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::{fs, path, process, rc::Rc};

use glow as GL;
use glow::HasContext;
use winit::monitor::MonitorHandle;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle, RawWindowHandle};
use winit::window::WindowBuilder;
use winit::window::{Window, WindowId};

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext, Version};
use glutin::display::{Display, DisplayApiPreference, GetGlDisplay};
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};

#[cfg(windows)]
use winit::platform::windows::WindowBuilderExtWindows;

const MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER: f64 = 10.0;

// In milliseconds
const FADE_TO_BLACK_DURATION: f64 = 300.0;

struct Instance {
    flux: Flux,
    gl_context: PossiblyCurrentContext,
    gl_surface: Surface<WindowSurface>,
    gl: Rc<glow::Context>,
    window: Window,
}

impl Instance {
    pub fn draw(&mut self, timestamp: f64) {
        self.gl_context
            .make_current(&self.gl_surface)
            .expect("make OpenGL context current");

        self.flux.animate(timestamp);

        self.gl_surface
            .swap_buffers(&self.gl_context)
            .expect("swap OpenGL buffers");
    }

    pub fn fade_to_black(&mut self, timestamp: f64) {
        self.gl_context
            .make_current(&self.gl_surface)
            .expect("make OpenGL context current");

        let progress = (timestamp / FADE_TO_BLACK_DURATION).clamp(0.0, 1.0) as f32;
        unsafe {
            self.gl.clear_color(0.0, 0.0, 0.0, progress);
            self.gl.clear(GL::COLOR_BUFFER_BIT);
        }

        self.gl_surface
            .swap_buffers(&self.gl_context)
            .expect("swap OpenGL buffers");
    }
}

fn main() {
    let project_dirs = directories::ProjectDirs::from("me", "sandydoo", "Flux");
    let log_dir = project_dirs.as_ref().map(|dirs| dirs.data_local_dir());
    let config_dir = project_dirs.as_ref().map(|dirs| dirs.preference_dir());

    init_logging(log_dir);

    let config = Config::load(config_dir);

    match cli::read_flags().and_then(|mode| {
        if mode == Mode::Settings {
            settings_window::run(config)
                .map_err(|err| log::error!("{}", err))
                .unwrap();
            return Ok(());
        }

        run_flux(mode, config)
    }) {
        Ok(_) => process::exit(0),
        Err(err) => {
            log::error!("{}", err);
            process::exit(1)
        }
    };
}

fn init_logging(optional_log_dir: Option<&path::Path>) {
    use simplelog::*;

    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )];

    if let Some(log_dir) = optional_log_dir {
        let maybe_log_file = {
            fs::create_dir_all(log_dir).unwrap();
            let log_path = log_dir.join("flux_screensaver.log");
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(log_path)
        };

        if let Ok(log_file) = maybe_log_file {
            loggers.push(WriteLogger::new(
                LevelFilter::Warn,
                Config::default(),
                log_file,
            ));
        }
    }

    let _ = CombinedLogger::init(loggers);
    log_panics::init();
}

fn run_flux(mode: Mode, config: Config) -> Result<(), String> {
    let event_loop = winit::event_loop::EventLoop::new();

    match mode {
        Mode::Preview(raw_window_handle) => {
            #[cfg(not(windows))]
            panic!("Preview window unsupported");

            let mut instance = new_preview_window(&event_loop, raw_window_handle, &config)?;

            let start = std::time::Instant::now();

            event_loop.run(move |event, window_target, control_flow| {
                run_preview_loop(event, window_target, control_flow, &mut instance, start)
            });
        }

        Mode::Screensaver => {
            let monitors = event_loop
                .available_monitors()
                .map(|monitor| (monitor.clone(), wallpaper::get(&monitor).ok()))
                .collect::<Vec<(MonitorHandle, Option<std::path::PathBuf>)>>();
            log::debug!("Available monitors: {:?}", monitors);

            let surfaces = surface::combine_monitors(&monitors);
            log::debug!("Creating windows: {:?}", surfaces);

            let mut instances = surfaces
                .iter()
                .map(|surface| {
                    new_instance(&event_loop, &config, surface)
                        .map(|instance| (instance.window.id(), instance))
                })
                .collect::<Result<HashMap<WindowId, Instance>, String>>()?;

            let start = std::time::Instant::now();

            // Unhide windows after context setup
            for instance in instances.values() {
                instance.window.set_visible(true);
            }

            event_loop.run(move |event, window_target, control_flow| {
                run_main_loop(event, window_target, control_flow, &mut instances, start)
            });
        }

        _ => unreachable!(),
    };
}

fn run_preview_loop(
    event: winit::event::Event<()>,
    _window_target: &winit::event_loop::EventLoopWindowTarget<()>,
    control_flow: &mut winit::event_loop::ControlFlow,
    instance: &mut Instance,
    start: std::time::Instant,
) {
    use winit::event::{Event, WindowEvent};
    use winit::event_loop::ControlFlow;

    *control_flow = ControlFlow::Poll;

    match event {
        Event::MainEventsCleared => {
            let timestamp = start.elapsed().as_secs_f64() * 1000.0;
            instance.draw(timestamp);
        }

        Event::LoopDestroyed => *control_flow = ControlFlow::Exit,

        Event::WindowEvent { event, .. } => {
            if event == WindowEvent::CloseRequested {
                *control_flow = ControlFlow::Exit
            }
        }

        _ => (),
    }
}

fn run_main_loop(
    event: winit::event::Event<()>,
    _window_target: &winit::event_loop::EventLoopWindowTarget<()>,
    control_flow: &mut winit::event_loop::ControlFlow,
    instances: &mut HashMap<WindowId, Instance>,
    start: std::time::Instant,
) {
    use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, WindowEvent};
    use winit::event_loop::ControlFlow;

    *control_flow = ControlFlow::Poll;

    match event {
        Event::MainEventsCleared => {
            for (_, instance) in instances.iter_mut() {
                instance.window.request_redraw();
            }
        }

        Event::RedrawRequested(window_id) => {
            let instance = instances.get_mut(&window_id).expect("cannot find window");
            let timestamp = start.elapsed().as_secs_f64() * 1000.0;

            if timestamp < FADE_TO_BLACK_DURATION {
                instance.fade_to_black(timestamp);
            } else {
                instance.draw(timestamp);
            }
        }

        Event::LoopDestroyed => *control_flow = ControlFlow::Exit,

        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested { .. }
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        ..
                    },
                is_synthetic: false,
                ..
            }
            | WindowEvent::MouseInput { .. } => *control_flow = ControlFlow::Exit,
            _ => (),
        },

        Event::DeviceEvent {
            event: DeviceEvent::MouseMotion {
                delta: (xrel, yrel),
            },
            ..
        } if f64::max(xrel.abs(), yrel.abs()) > MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER => {
            *control_flow = ControlFlow::Exit
        }

        _ => (),
    }
}

#[cfg(windows)]
fn new_preview_window(
    event_loop: &winit::event_loop::EventLoop<()>,
    raw_window_handle: RawWindowHandle,
    config: &Config,
) -> Result<Instance, String> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    use winit::dpi::{PhysicalSize, Size};

    let win32_handle = match raw_window_handle {
        RawWindowHandle::Win32(handle) => handle,
        _ => return Err("This platform is not supported yet".to_string()),
    };

    let preview_hwnd = HWND(win32_handle.hwnd as _);

    let mut rect = RECT::default();
    unsafe {
        GetClientRect(preview_hwnd, &mut rect);
    }

    let inner_size = PhysicalSize::new(rect.right as u32, rect.bottom as u32);

    let window = unsafe {
        WindowBuilder::new()
            .with_title("Flux Preview")
            .with_parent_window(Some(raw_window_handle))
            .with_inner_size(Size::Physical(inner_size))
            .with_decorations(false)
            .with_visible(false)
            .build(event_loop)
            .unwrap()
    };

    let (gl_context, gl_surface, glow_context) = new_gl_context(
        event_loop,
        raw_window_handle,
        inner_size,
        Some(window.raw_window_handle()),
    );

    let wallpaper = window
        .current_monitor()
        .and_then(|monitor| wallpaper::get(&monitor).ok());

    let physical_size = window.inner_size();
    let scale_factor = window.scale_factor();
    let logical_size = physical_size.to_logical(scale_factor);
    let settings = config.to_settings(wallpaper);
    let flux = Flux::new(
        &glow_context,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Rc::new(settings),
    )
    .map_err(|err| err.to_string())?;

    Ok(Instance {
        flux,
        gl_context,
        gl_surface,
        gl: Rc::clone(&glow_context),
        window,
    })
}

fn new_instance(
    event_loop: &winit::event_loop::EventLoop<()>,
    config: &Config,
    surface: &surface::Surface,
) -> Result<Instance, String> {
    let window = WindowBuilder::new()
        .with_title("Flux")
        .with_inner_size(surface.size)
        .with_position(surface.position)
        // Skips the default Windows window animation
        .with_maximized(true)
        // Hide the window until we've initialized Flux
        .with_visible(false)
        // Enable transparency to draw the fade in
        .with_transparent(true)
        .with_decorations(false)
        .with_undecorated_shadow(false)
        .with_skip_taskbar(true)
        .with_window_level(winit::window::WindowLevel::AlwaysOnTop)
        .build(event_loop)
        .unwrap();

    let (gl_context, gl_surface, glow_context) = new_gl_context(
        event_loop,
        window.raw_window_handle(),
        window.inner_size(),
        None,
    );

    let physical_size = surface.size;
    let logical_size = physical_size.to_logical(surface.scale_factor);
    let settings = config.to_settings(surface.wallpaper.clone());
    let flux = Flux::new(
        &glow_context,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Rc::new(settings),
    )
    .map_err(|err| err.to_string())?;

    window.set_cursor_visible(false);

    Ok(Instance {
        flux,
        gl_context,
        gl_surface,
        gl: Rc::clone(&glow_context),
        window,
    })
}

/// Create an OpenGL context, surface, and initialize the glow API.
///
/// Hacks
///
/// The optional attr_window should be used when rendering to the preview window. Instead of just
/// using the handle to the preview window, pass the window handle for the invisible event window
/// to work around a bug where Windows complains that it can't find the window class.
///
/// This code has been modified from glutin-winit and only supports WGL (Windows).
fn new_gl_context(
    event_loop: &winit::event_loop::EventLoop<()>,
    raw_window_handle: RawWindowHandle,
    inner_size: winit::dpi::PhysicalSize<u32>,

    // A hack to create the gl_display using the invisible event window
    // we create for the preview.
    attr_window: Option<RawWindowHandle>,
) -> (
    PossiblyCurrentContext,
    Surface<WindowSurface>,
    Rc<glow::Context>,
) {
    let template = ConfigTemplateBuilder::new()
        .with_alpha_size(8)
        .with_transparency(true)
        // Double buffering doesn't work with transparency
        .with_single_buffering(false)
        .compatible_with_native_window(raw_window_handle)
        .build();

    // Only WGL requires a window to create a full-fledged OpenGL context
    let attr_window = attr_window.unwrap_or(raw_window_handle);
    let preference = DisplayApiPreference::WglThenEgl(Some(attr_window));
    let gl_display = unsafe { Display::new(event_loop.raw_display_handle(), preference).unwrap() };

    let gl_config = unsafe {
        gl_display
            .find_configs(template)
            .unwrap()
            .reduce(|accum, config| {
                let transparency_check = config.supports_transparency().unwrap_or(false)
                    & !accum.supports_transparency().unwrap_or(false);

                if transparency_check || config.num_samples() > accum.num_samples() {
                    config
                } else {
                    accum
                }
            })
            .unwrap()
    };

    log::debug!(
        "Picked a config with {} samples and {:?} transparency",
        gl_config.num_samples(),
        gl_config.supports_transparency()
    );

    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(Some(raw_window_handle));

    let not_current_gl_context = unsafe {
        gl_display
            .create_context(&gl_config, &context_attributes)
            .expect("failed to create OpenGL context")
    };

    let (width, height) = inner_size.non_zero().expect("non-zero window size").into();
    let attrs =
        SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window_handle, width, height);

    let gl_surface = unsafe {
        gl_config
            .display()
            .create_window_surface(&gl_config, &attrs)
            .unwrap()
    };

    // Make it current.
    let gl_context = not_current_gl_context.make_current(&gl_surface).unwrap();

    // Try setting vsync.
    if let Err(res) =
        gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
    {
        log::error!("Failed to set vsync: {res:?}");
    }

    let glow_context = unsafe {
        glow::Context::from_loader_function(|s| {
            gl_display.get_proc_address(&CString::new(s).unwrap().as_c_str()) as *const _
        })
    };
    log::debug!("{:?}", glow_context.version());

    (gl_context, gl_surface, Rc::new(glow_context))
}

// Specifying DPI awareness in the app manifest does not apply when running in a
// preview window.
#[cfg(windows)]
pub fn set_dpi_awareness() -> Result<(), String> {
    use windows::Win32::Foundation::E_INVALIDARG;
    use windows::Win32::UI::HiDpi::{
        GetProcessDpiAwareness, SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE,
        PROCESS_SYSTEM_DPI_AWARE,
    };

    if let Err(err) = unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) } {
        match err.code() {
            E_INVALIDARG => return Err("Can’t enable support for high-resolution screens.".to_string()),
            // The app manifest settings, if applied, trigger this path.
            _ => {
                return match unsafe { GetProcessDpiAwareness(None) } {
                    Ok(awareness)
                        if awareness == PROCESS_PER_MONITOR_DPI_AWARE
                        || awareness == PROCESS_SYSTEM_DPI_AWARE => Ok(()),
                    _ => Err("Can’t enable support for high-resolution screens. The setting has been modified and set to an unsupported value.".to_string()),
                }
            }
        }
    }

    Ok(())
}

/// [`winit::dpi::PhysicalSize<u32>`] non-zero extensions.
trait NonZeroU32PhysicalSize {
    /// Converts to non-zero `(width, height)`.
    fn non_zero(self) -> Option<(NonZeroU32, NonZeroU32)>;
}
impl NonZeroU32PhysicalSize for winit::dpi::PhysicalSize<u32> {
    fn non_zero(self) -> Option<(NonZeroU32, NonZeroU32)> {
        let w = NonZeroU32::new(self.width)?;
        let h = NonZeroU32::new(self.height)?;
        Some((w, h))
    }
}
