// Disable the console window that pops up when you launch the .exe
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use flux::{settings::*, *};
use std::rc::Rc;
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::GetClientRect,
};
use std::os::raw;
use takeable_option::Takeable;
use glutin::platform::windows::WindowExtWindows;
use glutin::platform::windows::RawContextExt;

const SETTINGS: Settings = Settings {
    viscosity: 1.0,
    velocity_dissipation: 0.0,
    starting_pressure: 0.8,
    fluid_size: 128,
    fluid_simulation_frame_rate: 30.0,
    diffusion_iterations: 20,
    pressure_iterations: 60,
    color_scheme: ColorScheme::Plasma,
    line_length: 180.0,
    line_width: 6.0,
    line_begin_offset: 0.5,
    line_fade_out_length: 0.005,
    spring_stiffness: 0.2,
    spring_variance: 0.25,
    spring_mass: 2.0,
    spring_damping: 2.0,
    spring_rest_length: 0.0,
    advection_direction: 1.0,
    adjust_advection: 22.0,
    max_line_velocity: 0.02,
    grid_spacing: 20,
    view_scale: 1.2,
    noise_channel_1: Noise {
        scale: 0.9,
        multiplier: 0.20,
        offset_1: 2.0,
        offset_2: 8.0,
        offset_increment: 0.01,
        delay: 0.5,
        blend_duration: 3.5,
        blend_threshold: 0.4,
        blend_method: BlendMethod::Curl,
    },
    noise_channel_2: Noise {
        scale: 25.0,
        multiplier: 0.08,
        offset_1: 3.0,
        offset_2: 2.0,
        offset_increment: 0.02,
        delay: 0.15,
        blend_duration: 1.0,
        blend_threshold: 0.0,
        blend_method: BlendMethod::Curl,
    },
};

const BASE_DPI: u32 = 96;

enum Mode {
    Screensaver,
    Preview(HWND),
}

fn main() {
    let env = env_logger::Env::default()
        .filter_or("MY_LOG_LEVEL", "debug");
    env_logger::init_from_env(env);

    match read_flags() {
        Ok(Mode::Screensaver) => run_flux(None),
        Ok(Mode::Preview(handle)) => run_flux(Some(handle)),
        Err(err) => {
            log::error!("{}", err);
            std::process::exit(1)
        }
    };
}

fn run_flux(window_handle: Option<HWND>) {
    let event_loop = glutin::event_loop::EventLoop::new();
    
    let (window, physical_width, physical_height) = {
        if let Some(parent_handle) = window_handle {
            let mut rect: RECT = unsafe { std::mem::zeroed() };
            if unsafe { GetClientRect(parent_handle, &mut rect) } == false.into() {
                panic!("Unexpected GetClientRect failure: please report this error to https://github.com/rust-windowing/winit")
            }
            let physical_width = (rect.right - rect.left) as u32;
            let physical_height = (rect.bottom - rect.top) as u32;

            (parent_handle as *mut raw::c_void, physical_width, physical_height)
        } else {
            let window = glutin::window::WindowBuilder::new()
                .with_title("Flux")
                .with_fullscreen(Some(glutin::window::Fullscreen::Exclusive(
                    get_best_videomode(&event_loop.primary_monitor().unwrap()),
                )))
                .build(&event_loop)
                .unwrap_or_else(|e| {
                    log::error!("{}", e.to_string());
                    std::process::exit(1)
                });

            let (physical_width, physical_height): (u32, u32) = window.inner_size().into();

            (window.hwnd(), physical_width, physical_height)
        }
    };

    log::debug!("{}", physical_width);

    let context = unsafe {
        glutin::ContextBuilder::new()
            .build_raw_context(window)
            .unwrap_or_else(|e| {
                log::error!("{:?}", e);
                std::process::exit(1)
            })
    };
    let context = unsafe {
        context
            .make_current()
            .unwrap_or_else(|e| {
                log::error!("{:?}", e);
                std::process::exit(1)
            })
    };
    let gl = unsafe {
        glow::Context::from_loader_function(|s| context.get_proc_address(s) as *const _)
    };
    // let (_, dpi, _) = video_subsystem.display_dpi(0).unwrap();
    // let scale_factor = dpi as f64 / BASE_DPI as f64;
    // let logical_width = (physical_width as f64 / scale_factor) as u32;
    // let logical_height = (physical_height as f64 / scale_factor) as u32;
    let (logical_width, logical_height) = (physical_width, physical_height);
    let dpi = 96.0;
    log::debug!(
        "pw: {}, ph: {}, lw: {}, lh: {}, dpi: {}",
        physical_width,
        physical_height,
        logical_width,
        logical_height,
        dpi
    );
    let mut flux = Flux::new(
        &Rc::new(gl),
        logical_width,
        logical_height,
        physical_width,
        physical_height,
        &Rc::new(SETTINGS),
    )
    .unwrap();

    let start = std::time::Instant::now();
    let mut context = Takeable::new(context);
    event_loop.run(move |event, _, control_flow| {
        use glutin::event::{Event, WindowEvent};
        use glutin::event_loop::ControlFlow;

        let next_frame_time =
            std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        log::debug!("{:?}", event);

        match event {
            Event::LoopDestroyed => {
                Takeable::take(&mut context);
                return;
            }

            Event::WindowEvent { ref event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => ()
            },

            _ => (),
        }

        flux.animate(start.elapsed().as_millis() as f32);
        context.swap_buffers().unwrap();
    });
}

fn read_flags() -> Result<Mode, String> {
    match std::env::args().nth(1).as_mut().map(|s| s.as_str()) {
        Some("/s") => Ok(Mode::Screensaver),
        Some("/p") => {
            let handle_id = std::env::args()
                .nth(2)
                .ok_or_else(|| "I can’t find the window to show a screensaver preview.")?;
            let handle =
                handle_id.parse::<usize>().map_err(|e| e.to_string())? as HWND;
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

fn get_best_videomode(monitor: &glutin::monitor::MonitorHandle) -> glutin::monitor::VideoMode {
    let mut modes = monitor.video_modes().collect::<Vec<_>>();
    modes.sort_by(|a, b| {
        use std::cmp::Ordering::*;
        match b.size().width.cmp(&a.size().width) {
            Equal => match b.size().height.cmp(&a.size().height) {
                Equal => b.refresh_rate().cmp(&a.refresh_rate()),
                default => default,
            },
            default => default,
        }
    });

    modes.first().unwrap().clone()
}

