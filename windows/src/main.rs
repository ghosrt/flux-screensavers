// Disable the console window that pops up when you launch the .exe
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use core::ffi::c_void;
use flux::{settings::*, *};
use glow::HasContext;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use sdl2::event::Event;
use sdl2::video::GLProfile;
use std::rc::Rc;

#[cfg(windows)]
use winapi::shared::windef::HWND;

const BASE_DPI: u32 = 96;
const MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER: i32 = 10;

enum Mode {
    Screensaver,
    Preview(RawWindowHandle),
}

enum Window<W: HasRawWindowHandle> {
    MainWindow(W),
    PreviewWindow {
        #[allow(unused)]
        handle: W, // Keep this handle alive
        parent_handle: W,
    },
}

impl<W: HasRawWindowHandle> Window<W> {
    fn target_window(&self) -> &W {
        match self {
            Window::MainWindow(ref handle) => handle,
            Window::PreviewWindow {
                ref parent_handle, ..
            } => parent_handle,
        }
    }
}

fn main() {
    // env_logger::init();
    let env = env_logger::Env::default().filter_or("MY_LOG_LEVEL", "debug");

    env_logger::init_from_env(env);

    match read_flags().and_then(run_flux) {
        Ok(_) => std::process::exit(0),
        Err(err) => {
            log::error!("{}", err);
            std::process::exit(1)
        }
    };
}

fn run_flux(mode: Mode) -> Result<(), String> {
    #[cfg(windows)]
    set_dpi_awareness()?;

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_context_version(3, 3);
    gl_attr.set_multisample_buffers(2);
    gl_attr.set_multisample_samples(4);
    #[cfg(debug_assertions)]
    gl_attr.set_context_flags().debug().set();

    let display_mode = video_subsystem.current_display_mode(0)?;
    log::debug!("Refresh rate: {}fps", display_mode.refresh_rate);

    let (window, physical_width, physical_height) = match mode {
        Mode::Preview(raw_window_handle) => {
            let parent_handle = match raw_window_handle {
                RawWindowHandle::Win32(handle) => handle.hwnd,
                _ => return Err("This platform is not supported".to_string()),
            };

            // SDL disables the screensaver by default. Make sure we let the
            // screensaver run whenever we’re showing the preview.
            video_subsystem.enable_screen_saver();

            // Tell SDL that the window we’re about to adopt will be used with
            // OpenGL.
            sdl2::hint::set("SDL_VIDEO_FOREIGN_WINDOW_OPENGL", "1");
            let sdl_window: *mut sdl2_sys::SDL_Window =
                unsafe { sdl2_sys::SDL_CreateWindowFrom(parent_handle as *const c_void) };

            if sdl_window.is_null() {
                return Err(format!(
                    "Can’t create the preview window with the handle {:?}",
                    parent_handle
                ));
            }

            let parent_window: sdl2::video::Window =
                unsafe { sdl2::video::Window::from_ll(video_subsystem.clone(), sdl_window) };

            let child_window = video_subsystem
                .window("Flux Preview", 0, 0)
                .position(0, 0)
                .borderless()
                .hidden()
                .build()
                .map_err(|err| err.to_string())?;

            match child_window.raw_window_handle() {
                #[cfg(target_os = "windows")]
                raw_window_handle::RawWindowHandle::Win32(child_handle) => {
                    if unsafe {
                        set_window_parent_win32(child_handle.hwnd as HWND, parent_handle as HWND)
                    } {
                        log::debug!("Linked preview window");
                    }
                }
                _ => (),
            }

            let (physical_width, physical_height) = parent_window.drawable_size();

            let window = Window::PreviewWindow {
                handle: child_window,
                parent_handle: parent_window,
            };
            (window, physical_width, physical_height)
        }
        Mode::Screensaver => {
            let physical_width = display_mode.w as u32;
            let physical_height = display_mode.h as u32;
            let window = video_subsystem
                .window("Flux", physical_width, physical_height)
                .fullscreen_desktop()
                .input_grabbed()
                .allow_highdpi()
                .opengl()
                .build()
                .map_err(|err| err.to_string())?;

            // Hide mouse cursor
            sdl_context.mouse().show_cursor(false);
            sdl_context.mouse().set_relative_mouse_mode(true);

            (Window::MainWindow(window), physical_width, physical_height)
        }
    };

    // Create the OpenGL context. We don’t use the context it returns, but make
    // sure it isn’t dropped.
    let _ctx = window.target_window().gl_create_context()?;
    let gl = unsafe {
        glow::Context::from_loader_function(|s| video_subsystem.gl_get_proc_address(s) as *const _)
    };
    log::debug!("{:?}", gl.version());

    let (_, dpi, _) = video_subsystem.display_dpi(0)?;
    let scale_factor = dpi as f64 / BASE_DPI as f64;
    let logical_width = (physical_width as f64 / scale_factor) as u32;
    let logical_height = (physical_height as f64 / scale_factor) as u32;

    log::debug!(
        "pw: {}, ph: {}, lw: {}, lh: {}, dpi: {}",
        physical_width,
        physical_height,
        logical_width,
        logical_height,
        dpi
    );

    let settings = Settings {
        mode: settings::Mode::Normal,
        viscosity: 5.0,
        velocity_dissipation: 0.0,
        starting_pressure: 0.0,
        fluid_size: 128,
        fluid_simulation_frame_rate: 60.0,
        diffusion_iterations: 4,
        pressure_iterations: 20,
        color_scheme: ColorScheme::Peacock,
        line_length: 400.0,
        line_width: 7.0,
        line_begin_offset: 0.5,
        line_variance: 0.5,
        grid_spacing: 12,
        view_scale: 1.6,
        noise_channels: vec![
            Noise {
                scale: 2.3,
                multiplier: 1.0,
                offset_increment: 1.0 / 1024.0,
            },
            Noise {
                scale: 13.8,
                multiplier: 0.7,
                offset_increment: 1.0 / 1024.0,
            },
            Noise {
                scale: 27.6,
                multiplier: 0.5,
                offset_increment: 1.0 / 1024.0,
            },
        ],
    };

    let mut flux = Flux::new(
        &Rc::new(gl),
        logical_width,
        logical_height,
        physical_width,
        physical_height,
        &Rc::new(settings),
    )
    .map_err(|err| err.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;
    let start = std::time::Instant::now();

    'main: loop {
        for event in event_pump.poll_iter() {
            match mode {
                Mode::Preview(_) => match event {
                    Event::Quit { .. }
                    | Event::Window {
                        win_event: sdl2::event::WindowEvent::Close,
                        ..
                    } => break 'main,
                    _ => (),
                },
                Mode::Screensaver => match event {
                    Event::Quit { .. }
                    | Event::Window {
                        win_event: sdl2::event::WindowEvent::Close,
                        ..
                    }
                    | Event::KeyDown { .. }
                    | Event::MouseButtonDown { .. } => break 'main,
                    Event::MouseMotion { xrel, yrel, .. } => {
                        if i32::max(xrel.abs(), yrel.abs())
                            > MINIMUM_MOUSE_MOTION_TO_EXIT_SCREENSAVER
                        {
                            break 'main;
                        }
                    }
                    _ => {}
                },
            }
        }

        flux.animate(start.elapsed().as_millis() as f32);
        window.target_window().gl_swap_window();
        ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}

fn read_flags() -> Result<Mode, String> {
    match std::env::args().nth(1).as_mut().map(|s| {
        // I think the test button sends an uppercase /S, which doesn’t seem to
        // be documented anywhere.
        s.make_ascii_lowercase();
        s.as_str()
    }) {
        Some("/s") => Ok(Mode::Screensaver),
        Some("/p") => {
            let handle_ptr = std::env::args()
                .nth(2)
                .ok_or_else(|| "I can’t find the window to show a screensaver preview.")?
                .parse::<usize>()
                .map_err(|e| e.to_string())?;

            let mut handle = raw_window_handle::Win32Handle::empty();
            handle.hwnd = handle_ptr as *mut c_void;
            Ok(Mode::Preview(RawWindowHandle::Win32(handle)))
        }
        Some(s) => {
            return Err(format!("I don’t know what the argument {} is.", s));
        }
        None => {
            return Err(format!("{}", "You need to provide at least on flag."));
        }
    }
}

#[cfg(windows)]
unsafe fn set_window_parent_win32(handle: HWND, parent_handle: HWND) -> bool {
    use winapi::shared::basetsd::LONG_PTR;
    use winapi::um::winuser::{
        GetWindowLongPtrA, SetParent, SetWindowLongPtrA, GWL_STYLE, WS_CHILD, WS_POPUP,
    };

    // Attach our window to the parent window.
    // You can get more error information with `GetLastError`
    // https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setparent
    if SetParent(handle, parent_handle).is_null() {
        return false;
    }

    // `SetParent` doesn’t actually set the window style flags. `WS_POPUP` and
    // `WS_CHILD` are mutually exclusive.
    SetWindowLongPtrA(
        handle,
        GWL_STYLE,
        (GetWindowLongPtrA(handle, GWL_STYLE) & !WS_POPUP as LONG_PTR) | WS_CHILD as LONG_PTR,
    );

    true
}

#[cfg(windows)]
pub fn set_dpi_awareness() -> Result<(), String> {
    use std::ptr;
    use winapi::{
        shared::winerror::{E_INVALIDARG, S_OK},
        um::shellscalingapi::{
            GetProcessDpiAwareness, SetProcessDpiAwareness, PROCESS_DPI_UNAWARE,
            PROCESS_PER_MONITOR_DPI_AWARE,
        },
    };

    match unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) } {
        S_OK => Ok(()),
        E_INVALIDARG => Err("Could not set DPI awareness.".into()),
        _ => {
            let mut awareness = PROCESS_DPI_UNAWARE;
            match unsafe { GetProcessDpiAwareness(ptr::null_mut(), &mut awareness) } {
                S_OK if awareness == PROCESS_PER_MONITOR_DPI_AWARE => Ok(()),
                _ => Err("Please disable DPI awareness override in program properties.".into()),
            }
        }
    }
}
