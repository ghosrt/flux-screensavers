use flux::settings::*;
use flux::*;
use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::Window;
use glutin::PossiblyCurrent;
use std::rc::Rc;

const settings: Settings = Settings {
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

fn main() {
    std::process::exit(match run() {
        Ok(_) => 0,
        Err(err) => {
            print!("{}", err);
            // (format!("{}", err).into());
            1
        }
    });
}

fn run() -> Result<(), String> {
    match std::env::args().nth(1).as_mut().map(|s| s.as_str()) {
        Some("/s") => {
            let (gl, window, event_loop) = get_rendering_context();
            let mut flux = Flux::new(&Rc::new(gl), 800, 600, 800, 600, &Rc::new(settings)).unwrap();

            let start = std::time::Instant::now();

            event_loop.run(move |event, _, control_flow| {
                let next_frame_time =
                    std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
                *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

                match event {
                    Event::LoopDestroyed => {
                        return;
                    }

                    Event::MainEventsCleared => {
                        window.window().request_redraw();
                    }

                    Event::RedrawRequested(_) => {}

                    Event::WindowEvent { ref event, .. } => {
                        use WindowEvent::*;
                        match event {
                            MouseInput {
                                button: glutin::event::MouseButton::Left,
                                ..
                            } => *control_flow = ControlFlow::Exit,

                            _ => (),
                        }
                    }

                    _ => (),
                }

                flux.animate(start.elapsed().as_millis() as f32);
                window.swap_buffers().unwrap();
            });
        }
        Some(s) => {
            return Err(format!("I don’t know what the argument {} is.", s));
        }
        None => {
            return Err(format!("{}", "You need to provide at least on flag."));
        }
    }
}

pub fn get_rendering_context() -> (
    glow::Context,
    glutin::ContextWrapper<PossiblyCurrent, Window>,
    EventLoop<()>,
) {
    let event_loop = glutin::event_loop::EventLoop::new();
    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Flux")
        .with_fullscreen(Some(glutin::window::Fullscreen::Exclusive(
            get_best_videomode(&event_loop.primary_monitor().unwrap()),
        )));
    let window = unsafe {
        glutin::ContextBuilder::new()
            .with_vsync(true)
            .build_windowed(window_builder, &event_loop)
            .unwrap()
            .make_current()
            .unwrap()
    };
    let gl =
        unsafe { glow::Context::from_loader_function(|s| window.get_proc_address(s) as *const _) };

    (gl, window, event_loop)
}

pub fn get_best_videomode(monitor: &glutin::monitor::MonitorHandle) -> glutin::monitor::VideoMode {
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
