// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use core::ffi::c_void;
use flux::{settings::*, *};
use raw_window_handle::HasRawWindowHandle;
use sdl2::event::Event;
use sdl2::video::GLProfile;
use std::rc::Rc;

// #[cfg(target_os = "windows")]
use winapi::shared::windef::HWND;

const BASE_DPI: u32 = 96;

enum Mode {
    Screensaver,
    Preview(String),
}

fn main() {
    env_logger::init();

    match read_flags() {
        Ok(Mode::Screensaver) => run_flux(None),
        Ok(Mode::Preview(handle)) => run_flux(Some(handle)),
        Err(err) => {
            log::error!("{}", err);
            std::process::exit(1)
        }
    };
}

fn run_flux(window_handle: Option<String>) {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_context_version(4, 6); // TODO

    // FIX
    let _child_window: sdl2::video::Window;
    let _child_window_context: std::rc::Rc<sdl2::video::WindowContext>;

    let (window, physical_width, physical_height) = {
        if let Some(handle_id) = window_handle {
            let parent_handle = handle_id
                .parse::<usize>()
                .map_err(|e| e.to_string())
                .unwrap();
            sdl2::hint::set("SDL_VIDEO_FOREIGN_WINDOW_OPENGL", "1");
            let sdl_window: *mut sdl2_sys::SDL_Window =
                unsafe { sdl2_sys::SDL_CreateWindowFrom(parent_handle as *const c_void) };

            if sdl_window.is_null() {
                log::error!(
                    "Can’t create the preview window with the handle {:?}",
                    parent_handle
                );
                std::process::exit(1)
            }

            let parent_window: sdl2::video::Window =
                unsafe { sdl2::video::Window::from_ll(video_subsystem.clone(), sdl_window) };

            _child_window = video_subsystem
                .window("Flux Preview", 0, 0)
                .position(0, 0)
                .borderless()
                .hidden()
                .build()
                .unwrap();

            if let raw_window_handle::RawWindowHandle::Win32(handle) =
                _child_window.raw_window_handle()
            {
                if unsafe { set_window_parent_win32(handle.hwnd as HWND, parent_handle as HWND) } {
                    // Will render into parent window directly
                    // return Ok((parent_window, window.context()));
                    log::debug!("Linked preview window");
                    _child_window_context = _child_window.context();
                }
            }

            let (physical_width, physical_height) = parent_window.size();

            (parent_window, physical_width, physical_height)
        } else {
            let display_mode = video_subsystem.current_display_mode(0).unwrap();
            let physical_width = display_mode.w as u32;
            let physical_height = display_mode.h as u32;
            let window = video_subsystem
                .window("Flux", physical_width, physical_height)
                .fullscreen()
                .opengl()
                .build()
                .unwrap_or_else(|e| {
                    log::error!("{}", e.to_string());
                    std::process::exit(1)
                });
            (window, physical_width, physical_height)
        }
    };

    // Hide mouse cursor
    sdl_context.mouse().show_cursor(false);

    let _ctx = window.gl_create_context().unwrap();
    let gl = unsafe {
        glow::Context::from_loader_function(|s| video_subsystem.gl_get_proc_address(s) as *const _)
    };
    let (_, dpi, _) = video_subsystem.display_dpi(0).unwrap();
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
        color_scheme: ColorScheme::Plasma,
        line_length: 400.0,
        line_width: 7.0,
        line_begin_offset: 0.5,
        line_variance: 0.5,
        grid_spacing: 20,
        view_scale: 1.2,
        noise_channels: vec![
            Noise {
                scale: 2.3,
                multiplier: 1.0,
                offset_increment: 1.0 / 512.0,
            },
            Noise {
                scale: 13.8,
                multiplier: 0.7,
                offset_increment: 1.0 / 512.0,
            },
            Noise {
                scale: 27.6,
                multiplier: 0.5,
                offset_increment: 1.0 / 512.0,
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
    .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let start = std::time::Instant::now();

    'main: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                // | Event::Window {
                //     win_event: sdl2::event::WindowEvent::Close,
                //     ..
                // }
                | Event::KeyDown { .. }
                | Event::MouseMotion { .. }
                | Event::MouseButtonDown { .. } => break 'main,
                _ => {}
            }
        }

        flux.animate(start.elapsed().as_millis() as f32);
        window.gl_swap_window();
        ::std::thread::sleep(::std::time::Duration::new(0, 1_000_000_000u32 / 60));
    }
}

fn read_flags() -> Result<Mode, String> {
    match std::env::args().nth(1).as_mut().map(|s| s.as_str()) {
        Some("/s") => Ok(Mode::Screensaver),
        Some("/p") => {
            let handle = std::env::args()
                .nth(2)
                .ok_or_else(|| "I can’t find the window to show a screensaver preview.")?;
            Ok(Mode::Preview(handle))
        }
        Some(s) => {
            return Err(format!("I don’t know what the argument {} is.", s));
        }
        None => {
            return Err(format!("{}", "You need to provide at least on flag."));
        }
    }
}

// #[cfg(target_os = "windows")]
// unsafe fn get_window_handle_win32(sdl_window: *mut sdl2_sys::SDL_Window) -> Option<HWND> {
//     use sdl2_sys::{
//         SDL_GetWindowWMInfo, SDL_SysWMinfo, SDL_SysWMinfo__bindgen_ty_1, SDL_bool, SDL_version,
//         SDL_MAJOR_VERSION, SDL_MINOR_VERSION, SDL_PATCHLEVEL, SDL_SYSWM_TYPE,
//     };

//     let mut syswmi = SDL_SysWMinfo {
//         version: SDL_version {
//             major: SDL_MAJOR_VERSION as u8,
//             minor: SDL_MINOR_VERSION as u8,
//             patch: SDL_PATCHLEVEL as u8,
//         },
//         subsystem: SDL_SYSWM_TYPE::SDL_SYSWM_UNKNOWN,
//         info: SDL_SysWMinfo__bindgen_ty_1 { dummy: [0; 64] },
//     };

//     match SDL_GetWindowWMInfo(sdl_window, &mut syswmi) {
//         SDL_bool::SDL_TRUE => {
//             assert!(syswmi.subsystem == SDL_SYSWM_TYPE::SDL_SYSWM_WINDOWS);
//             let handle: HWND = syswmi.info.wl.display;
//             assert!(!handle.is_null());
//             Some(handle)
//         }
//         SDL_bool::SDL_FALSE => None,
//     }
// }

// #[cfg(target_os = "windows")]
unsafe fn set_window_parent_win32(handle: HWND, parent_handle: HWND) -> bool {
    use winapi::um::winuser::{SetParent, GWL_STYLE, WS_CHILD, WS_POPUP};
    if SetParent(handle, parent_handle).is_null() {
        return false;
    }
    // Make this a child window so it will close when the parent dialog closes
    #[cfg(target_arch = "x86_64")]
    {
        use winapi::shared::basetsd::LONG_PTR;
        winapi::um::winuser::SetWindowLongPtrA(
            handle,
            GWL_STYLE,
            (winapi::um::winuser::GetWindowLongPtrA(handle, GWL_STYLE) & !WS_POPUP as LONG_PTR)
                | WS_CHILD as LONG_PTR,
        );
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        use winapi::shared::ntdef::LONG;
        winapi::um::winuser::SetWindowLongA(
            handle,
            GWL_STYLE,
            (winapi::um::winuser::GetWindowLongA(handle, GWL_STYLE) & !WS_POPUP as LONG)
                | WS_CHILD as LONG,
        );
    }
    true
}
