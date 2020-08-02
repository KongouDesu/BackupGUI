use std::time::{Duration, Instant};

use winit::{
    event::{self, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

use crate::gui::GuiProgram;

#[allow(dead_code)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}


pub fn load_glsl(code: &str, stage: ShaderStage) -> Vec<u32> {
    let ty = match stage {
        ShaderStage::Vertex => glsl_to_spirv::ShaderType::Vertex,
        ShaderStage::Fragment => glsl_to_spirv::ShaderType::Fragment,
        ShaderStage::Compute => glsl_to_spirv::ShaderType::Compute,
    };

    wgpu::read_spirv(glsl_to_spirv::compile(&code, ty).unwrap()).unwrap()
}

struct Setup {
    window: winit::window::Window,
    event_loop: EventLoop<()>,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

async fn setup(title: &str) -> Setup {
    let event_loop = EventLoop::new();

    log::info!("Initializing the surface...");

    let (window, size, surface) = {
        let mut builder = winit::window::WindowBuilder::new();
        builder = builder.with_title(title);

        let window = builder.build(&event_loop).unwrap();
        let size = window.inner_size();
        let surface = wgpu::Surface::create(&window);
        (window, size, surface)
    };

    let adapter = wgpu::Adapter::request(
        &wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::Default,
            compatible_surface: Some(&surface),
        },
        wgpu::BackendBit::PRIMARY,
    )
        .await
        .unwrap();

    let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default(),
    })
        .await;

    Setup {
        window,
        event_loop,
        size,
        surface,
        device,
        queue,
    }
}

fn start(
    Setup {
        window,
        event_loop,
        size,
        surface,
        device,
        queue,
    }: Setup,
) {
    let (mut pool, _spawner) = {
        env_logger::init();

        let local_pool = futures::executor::LocalPool::new();
        let spawner = local_pool.spawner();
        (local_pool, spawner)
    };

    let mut sc_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        // TODO: Allow srgb unconditionally
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Mailbox,
    };
    let mut swap_chain = device.create_swap_chain(&surface, &sc_desc);

    log::info!("Initializing the example...");
    let (mut program, init_command_buf) = GuiProgram::init(&sc_desc, &device);

    if let Some(command_buf) = init_command_buf {
        queue.submit(&[command_buf]);
    }

    let mut last_update_inst = Instant::now();

    log::info!("Entering render loop...");
    event_loop.run(move |event, _, control_flow| {
        *control_flow = if cfg!(feature = "metal-auto-capture") {
            ControlFlow::Exit
        } else {
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(5))
        };
        match event {
            event::Event::MainEventsCleared => {
                if last_update_inst.elapsed() > Duration::from_millis(7) {
                    program.timer += last_update_inst.elapsed().as_secs_f32();
                    window.request_redraw();
                    last_update_inst = Instant::now();
                }

                pool.run_until_stalled();
            },
            event::Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                log::info!("Resizing to {:?}", size);
                sc_desc.width = u32::max(size.width,1);
                sc_desc.height = u32::max(1,size.height);
                swap_chain = device.create_swap_chain(&surface, &sc_desc);
                let command_buf = program.resize(&sc_desc, &device);
                if let Some(command_buf) = command_buf {
                    queue.submit(&[command_buf]);
                }
            }
            event::Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    input:
                    event::KeyboardInput {
                        virtual_keycode: Some(event::VirtualKeyCode::Escape),
                        state: event::ElementState::Pressed,
                        ..
                    },
                    ..
                }
                | WindowEvent::CloseRequested => {
                    program.exit();
                    *control_flow = ControlFlow::Exit;
                }
                _ => {
                    program.update(event);
                }
            }
            event::Event::RedrawRequested(_) => {
                let frame = swap_chain
                    .get_next_texture()
                    .expect("Timeout when acquiring next swap chain texture");
                let command_buf = program.render(&frame, &device);
                queue.submit(&command_buf);
            }
            _ => {}
        }
    });
}

pub fn run(title: &str) {
    let setup = futures::executor::block_on(setup(title));
    start(setup);
}