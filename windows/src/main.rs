// Disable the console window that pops up when you launch the .exe 
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use flux::{settings::*, *};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::video::GLProfile;
use std::rc::Rc;

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
}

fn main() {
    match read_flags() {
        Ok(Mode::Screensaver) => run_flux(),

        Err(err) => {
            println!("{}", err);
            std::process::exit(1)
        }
    };
}

fn run_flux() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_context_version(4, 6); // TODO

    let display_mode = video_subsystem.current_display_mode(0).unwrap();
    let physical_width = display_mode.w as u32;
    let physical_height = display_mode.h as u32;
    let (_, dpi, _) = video_subsystem.display_dpi(0).unwrap();
    let scale_factor = dpi as f64 / BASE_DPI as f64;
    let logical_width = (physical_width as f64 / scale_factor) as u32;
    let logical_height = (physical_height as f64 / scale_factor) as u32;

    // Debug scaling
    println!("pw: {}, ph: {}, lw: {}, lh: {}, dpi: {}", physical_width, physical_height, logical_width, logical_height, dpi);

    let window = video_subsystem
        .window("Flux", physical_width, physical_height)
        .fullscreen()
        .opengl()
        .build()
        .unwrap_or_else(|e| {
            println!("{}", e.to_string());
            std::process::exit(1)
        });

    // Hide mouse cursor
    sdl_context.mouse().show_cursor(false);

    let _ctx = window.gl_create_context().unwrap();
    let gl = unsafe {
        glow::Context::from_loader_function(|s| video_subsystem.gl_get_proc_address(s) as *const _)
    };
    let mut flux = Flux::new(
        &Rc::new(gl),
        physical_width,
        physical_height,
        logical_width,
        logical_height,
        &Rc::new(SETTINGS),
    )
    .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let start = std::time::Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
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
        Some(s) => {
            return Err(format!("I don’t know what the argument {} is.", s));
        }
        None => {
            return Err(format!("{}", "You need to provide at least on flag."));
        }
    }
}

// let sdl_context = sdl2::init()?;
// let w: *mut sdl2_sys::SDL_Window =
//     unsafe { sdl2_sys::SDL_CreateWindowFrom(parent as *const c_void) };

// let window: sdl2::video::Window = {
//     let video_subsystem = sdl_context.video()?;
//     unsafe { sdl2::video::Window::from_ll(video_subsystem, w) }
// };
