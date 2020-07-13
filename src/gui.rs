use crate::framework;
use zerocopy::{AsBytes, FromBytes};
use wgpu::{vertex_attr_array, BufferDescriptor, BufferUsage};
use crate::text;
use crate::ui;
use crate::files;
use crate::ui::UIConfig;
use std::sync::Mutex;

#[repr(C)]
#[derive(Clone, Copy, AsBytes, FromBytes, Debug)]
pub struct Vertex {
    _pos: [f32; 2],
    _color: [f32; 4],
}

impl Vertex {
    pub fn new(pos: [f32; 2], color: [f32; 4]) -> Self {
        Vertex {
            _pos: pos,
            _color: color,
        }
    }

    pub fn rect(x: f32, y: f32, w: f32, h: f32, color: [f32;4]) -> Vec<Self> {
        vec![
            Self::new([x,y],color),
            Self::new([x+w,y],color),
            Self::new([x,y+h],color),
            Self::new([x+w,y+h],color),
        ]
    }
}

pub struct GuiProgram {
    vs_module: wgpu::ShaderModule,
    fs_module: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::BindGroup,
    transform: wgpu::Buffer,
    current_transform: [f32;16],
    multisampled_framebuffer: wgpu::TextureView,
    rebuild_pipeline: bool,
    sample_count: u32,
    sc_desc: wgpu::SwapChainDescriptor,
    ui_manager: ui::UIManager,
}

impl GuiProgram {
    fn create_pipeline(
        device: &wgpu::Device,
        sc_desc: &wgpu::SwapChainDescriptor,
        vs_module: &wgpu::ShaderModule,
        fs_module: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
        sample_count: u32,
    ) -> wgpu::RenderPipeline {
        println!("sample_count: {}", sample_count);
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: fs_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleStrip,
            color_states: &[wgpu::ColorStateDescriptor {
                format: sc_desc.format,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float2, 1 => Float4],
                }],
            },
            sample_count,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        })
    }

    fn create_multisampled_framebuffer(
        device: &wgpu::Device,
        sc_desc: &wgpu::SwapChainDescriptor,
        sample_count: u32,
    ) -> wgpu::TextureView {
        let multisampled_texture_extent = wgpu::Extent3d {
            width: sc_desc.width,
            height: sc_desc.height,
            depth: 1,
        };
        let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
            size: multisampled_texture_extent,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: sc_desc.format,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            label: None,
        };

        device
            .create_texture(multisampled_frame_descriptor)
            .create_default_view()
    }
}

fn ortho(left: f32, right: f32, top: f32, bottom: f32, near: f32, far: f32) -> [f32; 16] {
    let tx = -(right + left) / (right - left);
    let ty = -(top + bottom) / (top - bottom);
    let tz = -(far + near) / (far - near);
    [
        2.0 / (right - left), 0.0, 0.0, 0.0,
        0.0, 2.0 / (top - bottom), 0.0, 0.0,
        0.0, 0.0, -2.0 / (far - near), 0.0,
        tx, ty, tz, 1.0,
    ]
}

impl GuiProgram {
    pub fn init(
        sc_desc: &wgpu::SwapChainDescriptor,
        device: &wgpu::Device,
    ) -> (Self, Option<wgpu::CommandBuffer>) {
        println!("Press left/right arrow keys to change sample_count.");
        let sample_count = 4;


        let transform = device.create_buffer_with_data(
            ortho(0.0,sc_desc.width as f32, 0.0, sc_desc.height as f32, 1.0, -1.0).as_bytes(),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        let uniform_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniforms"),
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::UniformBuffer { dynamic: false },
                    },
                ],
            });

        let uniforms = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms2"),
            layout: &uniform_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &transform,
                        range: 0..64,
                    },
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&uniform_layout],
        });

        let vs_bytes =
            framework::load_glsl(include_str!("shader.vert"), framework::ShaderStage::Vertex);
        let fs_bytes = framework::load_glsl(
            include_str!("shader.frag"),
            framework::ShaderStage::Fragment,
        );
        let vs_module = device.create_shader_module(&vs_bytes);
        let fs_module = device.create_shader_module(&fs_bytes);

        /*
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[],
        });*/

        let pipeline = GuiProgram::create_pipeline(
            device,
            &sc_desc,
            &vs_module,
            &fs_module,
            &pipeline_layout,
            sample_count,
        );
        let multisampled_framebuffer =
            GuiProgram::create_multisampled_framebuffer(device, sc_desc, sample_count);

        let vertex_count = 0 as u32;

        let this = GuiProgram {
            vs_module,
            fs_module,
            pipeline_layout,
            pipeline,
            uniforms,
            transform,
            current_transform: [0.0; 16],
            multisampled_framebuffer,
            rebuild_pipeline: false,
            sample_count,
            sc_desc: sc_desc.clone(),
            ui_manager: ui::UIManager {
                fileroot: files::get_roots().unwrap(),
                config: UIConfig {
                    tree_width: 1024.0,
                    tree_height: 1024.0,
                    font_size: 24.0,
                },
                text_handler: Mutex::new(text::TextHandler::init(&device, sc_desc.format)),
                scroll: 0.0,
                cx: 0.0,
                cy: 0.0,
            }
        };
        (this, None)
    }

    pub fn resize(
        &mut self,
        sc_desc: &wgpu::SwapChainDescriptor,
        device: &wgpu::Device,
    ) -> Option<wgpu::CommandBuffer> {
        self.sc_desc = sc_desc.clone();
        self.multisampled_framebuffer =
            GuiProgram::create_multisampled_framebuffer(device, sc_desc, self.sample_count);
        None
    }

    pub fn update(&mut self, event: winit::event::WindowEvent) {
        match event {
            winit::event::WindowEvent::KeyboardInput { input, .. } => {
                if let winit::event::ElementState::Pressed = input.state {
                    match input.virtual_keycode {
                        Some(winit::event::VirtualKeyCode::Left) => {
                            if self.sample_count >= 2 {
                                self.sample_count = self.sample_count >> 1;
                                self.rebuild_pipeline = true;
                            }
                        }
                        Some(winit::event::VirtualKeyCode::Right) => {
                            if self.sample_count <= 16 {
                                self.sample_count = self.sample_count << 1;
                                self.rebuild_pipeline = true;
                            }
                        }
                        _ => {}
                    }
                }
            },
            winit::event::WindowEvent::MouseWheel {
                delta: winit::event::MouseScrollDelta::LineDelta(_, y),
                ..
            } => {
                self.ui_manager.scroll(y*24.0);
            },
            winit::event::WindowEvent::MouseInput {device_id, state, button, modifiers} => {
                if state == winit::event::ElementState::Pressed  {
                    let but = match button {
                        winit::event::MouseButton::Left => 1,
                        winit::event::MouseButton::Right => 2,
                        winit::event::MouseButton::Middle => 3,
                        winit::event::MouseButton::Other(n) => n,
                    };
                    self.ui_manager.on_click(but);
                }
            },
            winit::event::WindowEvent::CursorMoved {device_id, position, modifiers} => {
                self.ui_manager.cursor_moved(position.x as f32, position.y as f32);
            }
            _ => {}
        }
    }

    pub fn render(
        &mut self,
        frame: &wgpu::SwapChainOutput,
        device: &wgpu::Device,
    ) -> Vec<wgpu::CommandBuffer> {
        if self.rebuild_pipeline {
            self.pipeline = GuiProgram::create_pipeline(
                device,
                &self.sc_desc,
                &self.vs_module,
                &self.fs_module,
                &self.pipeline_layout,
                self.sample_count,
            );
            self.multisampled_framebuffer =
                GuiProgram::create_multisampled_framebuffer(device, &self.sc_desc, self.sample_count);
            self.rebuild_pipeline = false;
        }

        let vertices = self.ui_manager.render_file_tree();

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        if !vertices.is_empty() {
            let buffer = device.create_buffer_with_data(vertices.as_bytes(), BufferUsage::VERTEX);

            let rpass_color_attachment = if self.sample_count == 1 {
                wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &frame.view,
                    resolve_target: None,
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::BLACK,
                }
            } else {
                wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: &self.multisampled_framebuffer,
                    resolve_target: Some(&frame.view),
                    load_op: wgpu::LoadOp::Clear,
                    store_op: wgpu::StoreOp::Store,
                    clear_color: wgpu::Color::BLACK,
                }
            };

            /*
            if self.transform != self.pipeline.current_transform {
                let transform_buffer = device.create_buffer_with_data(
                    transform.as_bytes(),
                    wgpu::BufferUsage::COPY_SRC,
                );

                encoder.copy_buffer_to_buffer(
                    &transform_buffer,
                    0,
                    &pipeline.transform,
                    0,
                    16 * 4,
                );

                pipeline.current_transform = transform;
            }
            */

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[rpass_color_attachment],
                depth_stencil_attachment: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_bind_group(0, &self.uniforms, &[]);
            rpass.set_vertex_buffer(0, &buffer, 0, 0);


            rpass.draw(0..vertices.len() as u32, 0..3);
        }
        let cb1 = encoder.finish();

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Text") });

        // Draw on top of previous (i.e., entry background rectangle)
        {
            let _ = encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    color_attachments: &[
                        wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &frame.view,
                            resolve_target: None,
                            load_op: wgpu::LoadOp::Load,
                            store_op: wgpu::StoreOp::Store,
                            clear_color: wgpu::Color::BLACK,
                        },
                    ],
                    depth_stencil_attachment: None,
                },
            );
        }

        self.ui_manager.text_handler.lock().unwrap().flush(&device,&mut encoder, frame, (self.sc_desc.width,self.sc_desc.height));

       let cb2 = encoder.finish();

        vec![cb1,cb2]
    }
}