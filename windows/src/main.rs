// Disable the console window that pops up when you launch the .exe
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

const SETTINGS_COMING_SOON_MESSAGE: &'static str = r#"
    Coming soon!

    You’ll be able to personalise the screensaver here and make it your own, but it’s not quite ready yet.
    Follow me on Twitter @sandy_doo for updates!
"#;

#[derive(PartialEq)]
enum Mode {
    Screensaver,
    Preview(RawWindowHandle),
    Settings,
}

struct Instance {
    flux: Flux,
    context: sdl2::video::GLContext,
    window: sdl2::video::Window,
}

impl Instance {
    pub fn draw(&mut self, timestamp: f64) {
        // Don’t use `gl_set_context_to_current`. It doesn’t use the
        // corrent context!
        self.window.gl_make_current(&self.context).unwrap();
        self.flux.animate(timestamp);
        self.window.gl_swap_window();
    }
}

enum WindowMode<W: HasRawWindowHandle> {
    AllDisplays(Vec<Instance>),
    PreviewWindow {
        instance: Instance,
        #[allow(unused)]
        event_window: W, // Keep this handle alive
    },
}

fn main() {
    env_logger::init();

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

    if mode == Mode::Settings {
        use sdl2::messagebox::{show_simple_message_box, MessageBoxFlag};
        show_simple_message_box(
            MessageBoxFlag::INFORMATION,
            "Flux Settings",
            &SETTINGS_COMING_SOON_MESSAGE,
            None,
        )
        .map_err(|msg| format!("Can’t open a message box: {}", msg))?;
        return Ok(());
    }

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_context_version(3, 3);
    gl_attr.set_double_buffer(true);

    // Forcibly disable antialiasing. We take care of that internally.
    gl_attr.set_multisample_buffers(0);
    gl_attr.set_multisample_samples(0);

    #[cfg(debug_assertions)]
    gl_attr.set_context_flags().debug().set();

    // SDL, by default, disables the screensaver and doesn’t allow the display
    // to sleep. We want both of these things to happen in both screensaver and
    // preview modes.
    video_subsystem.enable_screen_saver();

    let settings = Rc::new(Settings {
        mode: settings::Mode::Normal,
        fluid_size: 128,
        fluid_frame_rate: 60.0,
        fluid_timestep: 1.0 / 60.0,
        viscosity: 5.0,
        velocity_dissipation: 0.0,
        clear_pressure: settings::ClearPressure::KeepPressure,
        diffusion_iterations: 3,
        pressure_iterations: 19,
        color_scheme: ColorScheme::Peacock,
        line_length: 550.0,
        line_width: 10.0,
        line_begin_offset: 0.4,
        line_variance: 0.45,
        grid_spacing: 15,
        view_scale: 1.6,
        noise_channels: vec![
            Noise {
                scale: 2.5,
                multiplier: 1.0,
                offset_increment: 0.0015,
            },
            Noise {
                scale: 15.0,
                multiplier: 0.7,
                offset_increment: 0.0015 * 6.0,
            },
            Noise {
                scale: 30.0,
                multiplier: 0.5,
                offset_increment: 0.0015 * 12.0,
            },
        ],
    });

    let mut window_mode = match mode {
        Mode::Preview(raw_window_handle) => {
            let preview_window_handle = match raw_window_handle {
                RawWindowHandle::Win32(handle) => handle.hwnd,
                _ => return Err("This platform is not supported yet".to_string()),
            };

            // Tell SDL that the window we’re about to adopt will be used with
            // OpenGL.
            sdl2::hint::set("SDL_VIDEO_FOREIGN_WINDOW_OPENGL", "1");
            let sdl_preview_window: *mut sdl2_sys::SDL_Window =
                unsafe { sdl2_sys::SDL_CreateWindowFrom(preview_window_handle as *const c_void) };

            if sdl_preview_window.is_null() {
                return Err(format!(
                    "Can’t create the preview window with the handle {:?}",
                    preview_window_handle
                ));
            }

            let preview_window: sdl2::video::Window = unsafe {
                sdl2::video::Window::from_ll(video_subsystem.clone(), sdl_preview_window)
            };

            // You need to create an actual window to listen to events. We’ll
            // then link this to the preview window as a child to cleanup when
            // the preview dialog is closed.
            let event_window = video_subsystem
                .window("Flux Preview", 0, 0)
                .position(0, 0)
                .borderless()
                .hidden()
                .build()
                .map_err(|err| err.to_string())?;

            match event_window.raw_window_handle() {
                #[cfg(target_os = "windows")]
                raw_window_handle::RawWindowHandle::Win32(event_window_handle) => {
                    if unsafe {
                        set_window_parent_win32(
                            event_window_handle.hwnd as HWND,
                            preview_window_handle as HWND,
                        )
                    } {
                        log::debug!("Linked preview window");
                    }
                }
                _ => (),
            }

            let (_, dpi, _) =
                video_subsystem.display_dpi(preview_window.display_index().unwrap_or(0))?;
            let scale_factor = dpi as f64 / BASE_DPI as f64;
            let (physical_width, physical_height) = preview_window.drawable_size();
            let logical_width = (physical_width as f64 / scale_factor) as u32;
            let logical_height = (physical_height as f64 / scale_factor) as u32;

            let context = preview_window.gl_create_context()?;
            let glow_context = unsafe {
                glow::Context::from_loader_function(|s| {
                    video_subsystem.gl_get_proc_address(s) as *const _
                })
            };
            log::debug!("{:?}", glow_context.version());

            preview_window.gl_make_current(&context)?;
            let flux = Flux::new(
                &Rc::new(glow_context),
                logical_width,
                logical_height,
                physical_width,
                physical_height,
                &settings,
            )
            .map_err(|err| err.to_string())?;

            let instance = Instance {
                flux,
                context,
                window: preview_window,
            };

            WindowMode::PreviewWindow {
                instance,
                event_window,
            }
        }
        Mode::Screensaver => {
            let display_count = video_subsystem.num_video_displays()?;
            log::debug!("Detected {} displays", display_count);

            let mut instances = Vec::with_capacity(display_count as usize);
            for display_index in 0..display_count {
                let (_, dpi, _) = video_subsystem.display_dpi(display_index)?;
                let scale_factor = dpi as f64 / BASE_DPI as f64;
                let bounds = video_subsystem.display_bounds(display_index)?;
                let (physical_width, physical_height) = bounds.size();
                let logical_width = (physical_width as f64 / scale_factor) as u32;
                let logical_height = (physical_height as f64 / scale_factor) as u32;

                log::debug!(
                    "Display: {}\nPhysical size: {}x{}, Logical size: {}x{}, Position: {} {}, DPI: {}",
                    display_index,
                    physical_width,
                    physical_height,
                    logical_width,
                    logical_height,
                    bounds.x(),
                    bounds.y(),
                    dpi
                );

                // Create the SDL window
                let window = video_subsystem
                    .window("Flux", physical_width, physical_height)
                    .position(bounds.x(), bounds.y())
                    .input_grabbed()
                    .fullscreen_desktop()
                    .allow_highdpi()
                    .opengl()
                    .build()
                    .map_err(|err| err.to_string())?;

                let context = window.gl_create_context()?;
                let glow_context = unsafe {
                    glow::Context::from_loader_function(|s| {
                        video_subsystem.gl_get_proc_address(s) as *const _
                    })
                };
                log::debug!("{:?}", glow_context.version());

                window.gl_make_current(&context)?;
                let flux = Flux::new(
                    &Rc::new(glow_context),
                    logical_width,
                    logical_height,
                    physical_width,
                    physical_height,
                    &settings,
                )
                .map_err(|err| err.to_string())?;

                let instance = Instance {
                    flux,
                    context,
                    window,
                };

                instances.push(instance)
            }

            // Hide the cursor and report relative mouse movements.
            sdl_context.mouse().set_relative_mouse_mode(true);

            WindowMode::AllDisplays(instances)
        }
        _ => unreachable!(),
    };

    // Try to enable vsync.
    if let Err(err) = video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync) {
        log::error!("Can’t enable vsync: {}", err);
    }

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
                _ => (),
            }
        }

        let timestamp = start.elapsed().as_millis() as f64;
        match window_mode {
            WindowMode::AllDisplays(ref mut instances) => {
                for instance in instances.iter_mut() {
                    instance.draw(timestamp);
                }
            }
            WindowMode::PreviewWindow {
                ref mut instance, ..
            } => instance.draw(timestamp),
        }
    }

    Ok(())
}

fn read_flags() -> Result<Mode, String> {
    match std::env::args().nth(1).as_mut().map(|s| {
        s.make_ascii_lowercase();
        s.as_str()
    }) {
        // Settings panel
        //
        // /c -> you’re supposed to support this, but AFAIK the only way to get
        // this is to manually send it from the command line.
        //
        // /c:HWND -> the screensaver configuration window gives a window
        // handle. I’m not sure what it’s for. Maybe you’re supposed to use it
        // to close your settings window if the parent windows closes?
        //
        // No flags -> <right click + configure> sends no flags whatsoever.
        Some("/c") | None => Ok(Mode::Settings),
        Some(s) if s.starts_with("/c:") => Ok(Mode::Settings),

        // Run screensaver
        //
        // /s -> run the screensaver.
        //
        // /S -> <right click + test> sends an uppercase /S, which doesn’t
        // seem to be documented anywhere.
        Some("/s") => Ok(Mode::Screensaver),

        // Run preview
        //
        // /p HWND -> draw the screensaver in the preview window.
        //
        // /p:HWND -> TODO: apparently, this is also an option you need to
        // support.
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

// Specifying DPI awareness in the app manifest does not apply when running in a
// preview window.
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
        E_INVALIDARG => Err("Can’t enable support for high-resolution screens.".to_string()),
        // The app manifest settings, if applied, trigger this path.
        _ => {
            let mut awareness = PROCESS_DPI_UNAWARE;
            match unsafe { GetProcessDpiAwareness(ptr::null_mut(), &mut awareness) } {
                S_OK if awareness == PROCESS_PER_MONITOR_DPI_AWARE => Ok(()),
                _ => Err("Can’t enable support for high-resolution screens. The setting has been modified and set to an unsupported value.".to_string()),
            }
        }
    }
}
