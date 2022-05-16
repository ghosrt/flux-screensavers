use glow::HasContext;
use flux::{settings::*, *};
use takeable_option::Takeable;
use std::rc::Rc;

fn main() {
    env_logger::init();
    log::error!("{:?}", std::env::args());

    // if let Ok(raw_window_id) = std::env::var("XSCREENSAVER_WINDOW") {
    //   let window_id = raw_window_id.parse::<u64>().unwrap();
    //   log::error!("{:?}", window_id);
    // }

    let raw_window_id = std::env::var("XSCREENSAVER_WINDOW").unwrap();
    let xwindow = {
        if (raw_window_id).starts_with("0x") {
            u64::from_str_radix(raw_window_id.trim_start_matches("0x"), 16)
        } else {
            raw_window_id.parse::<u64>()
        }
    }.unwrap();

    let (raw_context, event_loop) = {
        let event_loop = glutin::event_loop::EventLoop::new();
        // let wb = glutin::window::WindowBuilder::new().with_title("A fantastic window!");

        unsafe {
            use glutin::platform::unix::{
              EventLoopWindowTargetExtUnix, RawContextExt, WindowExtUnix,
            };

            // if event_loop.is_wayland() {
            //     log::error!("Wayland");
            //     let win = wb.build(&event_loop).unwrap();
            //     let size = win.inner_size();
            //     let (width, height): (u32, u32) = size.into();

            //     let display_ptr = win.wayland_display().unwrap() as *const _;
            //     let surface = win.wayland_surface().unwrap();

            //     let raw_context = glutin::ContextBuilder::new()
            //         .build_raw_wayland_context(display_ptr, surface, width, height)
            //         .unwrap();

            //     (win, raw_context, event_loop)
            // } else {
            //     log::error!("X11");
            //     let win = wb.build(&event_loop).unwrap();
            //     let xconnection = event_loop.xlib_xconnection().unwrap();
            //     let xwindow = win.xlib_window().unwrap();
            //     let raw_context =
            //       glutin::ContextBuilder::new().build_raw_x11_context(xconnection, xwindow as std::os::raw::c_ulong).unwrap();

            //     (win, raw_context, event_loop)
            // }
            let xconnection = event_loop.xlib_xconnection().unwrap();
            let raw_context =
                glutin::ContextBuilder::new().build_raw_x11_context(xconnection, xwindow as std::os::raw::c_ulong).unwrap();
            (raw_context, event_loop)
        }
    };

    let raw_context = unsafe { raw_context.make_current().unwrap() };

    log::error!("Pixel format of GL context: {:?}", raw_context.get_pixel_format());

    let gl = unsafe { glow::Context::from_loader_function(|ptr| raw_context.get_proc_address(ptr) as *const _) };

    log::error!("OpenGL version: {:?}", gl.version());
    let context = Rc::new(gl);


    let settings = Settings {
        mode: Mode::Normal,
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
        grid_spacing: 12,
        view_scale: 1.6,
        noise_channels: vec![Noise {
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
        }]
    };

    let mut flux = Flux::new(
        &context,
        800,
        600,
        800,
        600,
        &Rc::new(settings),
    ).unwrap();

    let mut raw_context = Takeable::new(raw_context);
    let start = std::time::Instant::now();
    event_loop.run(move |event, _, control_flow| {
        use glutin::event::Event;

        let next_frame_time =
            std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        match event {
            Event::LoopDestroyed => {
                log::error!("Loop destroyed");
                Takeable::take(&mut raw_context);
                return;
            },

            _ => (),
        }

        flux.animate(start.elapsed().as_millis() as f32);
        raw_context.swap_buffers().unwrap();
    });
}

