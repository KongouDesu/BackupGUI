use std::io::Cursor;
use std::sync::{Arc, Mutex};

use nanoserde::SerJson;
use wgpu::vertex_attr_array;
use zerocopy::{AsBytes, FromBytes};

use crate::files;
use crate::framework;
use crate::text;
use crate::ui;
use crate::ui::{filetree, UIState};
use crate::ui::{GUIConfig, GUIConfigStrings};
use crate::ui::align::AlignConfig;
use std::sync::atomic::{AtomicBool, Ordering};

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
            Self::new([x,y+h],color),
            Self::new([x+w,y],color),
            Self::new([x+w,y+h],color),
        ]
    }
}

#[repr(C)]
#[derive(Clone, Copy, AsBytes, FromBytes, Debug)]
pub struct TexVertex {
    pos: [f32; 4], // First 2 indices are (x,y), second are texture (u,v)
}

impl TexVertex {
    pub fn new(xy: (f32,f32),u: f32,v: f32) -> Self {
        TexVertex {
            pos: [xy.0,xy.1,u,v],
        }
    }

    // size is the (w,h) of the texture used
    // section is the top-left (x,y) and (w,h) for what part of the texture to draw
    pub fn rect(x: f32, y: f32, w: f32, h: f32, angle: f32, size: (f32, f32), section: [f32;4]) -> Vec<Self> {
        // Compute center
        let cx = x+w/2.0;
        let cy = y+h/2.0;
        let uv_left = section[0]/size.0;
        let uv_top = section[1]/size.1;
        let uv_right = (section[0]+section[2])/size.0;
        let uv_bottom = (section[1]+section[3])/size.1;
        vec![
            Self::new(rotate_around(x, y, cx, cy, angle), uv_left, uv_top),
            Self::new(rotate_around(x+w, y, cx, cy, angle), uv_right, uv_top),
            Self::new(rotate_around(x, y+h, cx, cy, angle), uv_left, uv_bottom),
            Self::new(rotate_around(x, y+h, cx, cy, angle), uv_left, uv_bottom),
            Self::new(rotate_around(x+w, y, cx, cy, angle), uv_right, uv_top),
            Self::new(rotate_around(x+w, y+h, cx, cy, angle), uv_right, uv_bottom),
        ]
    }
}

fn rotate_around(x: f32, y: f32, cx: f32, cy: f32, a: f32) -> (f32,f32) {
    let sin = a.sin();
    let cos = a.cos();
    let x = x - cx;
    let y = y - cy;
    let newx = x * cos - y * sin;
    let newy = x * sin + y * cos;
    (newx+cx,newy+cy)
}

pub struct GuiProgram {
    pub vs_module: wgpu::ShaderModule,
    pub fs_module: wgpu::ShaderModule,
    pub pipeline_layout: wgpu::PipelineLayout,
    pub pipeline: wgpu::RenderPipeline,
    pub uniforms: wgpu::BindGroup,
    pub transform: wgpu::Buffer,
    pub rebuild_pipeline: bool,
    pub sc_desc: wgpu::SwapChainDescriptor,
    pub state_manager: ui::StateManager,
    pub tex_vs_module: wgpu::ShaderModule,
    pub tex_fs_module: wgpu::ShaderModule,
    pub tex_pipeline: wgpu::RenderPipeline,
    pub texture_bind_group: wgpu::BindGroup,
    pub align: AlignConfig,
    pub timer: f32,
}

impl GuiProgram {
    fn create_pipeline(
        device: &wgpu::Device,
        sc_desc: &wgpu::SwapChainDescriptor,
        vs_module: &wgpu::ShaderModule,
        fs_module: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
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
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
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
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        })
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

        let mut init_encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Init CE") });


        // Orthographic transform, allows us to render in screen coordinates
        let transform = device.create_buffer_with_data(
            ortho(0.0,sc_desc.width as f32, 0.0, sc_desc.height as f32, 1.0, -1.0).as_bytes(),
            wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        );

        // Uniforms for transform matrix
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

        // Bind groups for textures
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        multisampled: false,
                        component_type: wgpu::TextureComponentType::Float,
                        dimension: wgpu::TextureViewDimension::D2,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                },
            ],
            label: Some("Texture BGL"),
        });

        let texture_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&uniform_layout, &texture_layout],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&uniform_layout],
        });

        // Create the texture
        let img_data = include_bytes!("../spritesheet.png");
        let img = image::load(Cursor::new(&img_data[..]), image::ImageFormat::Png)
            .unwrap()
            .to_rgba();
        let (width, height) = img.dimensions();
        println!("{}x{}", width, height);
        let img = img.into_raw();

        let texels = img;
        let texture_extent = wgpu::Extent3d {
            width,
            height,
            depth: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: texture_extent,
            array_layer_count: 1,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
            label: None,
        });
        let texture_view = texture.create_default_view();
        let temp_buf =
            device.create_buffer_with_data(texels.as_slice(), wgpu::BufferUsage::COPY_SRC);
        init_encoder.copy_buffer_to_texture(
            wgpu::BufferCopyView {
                buffer: &temp_buf,
                offset: 0,
                bytes_per_row: 4 * width,
                rows_per_image: 0,
            },
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                array_layer: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            texture_extent,
        );

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            compare: wgpu::CompareFunction::Undefined,
        });
        // Create bind group
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: None,
        });
        let tex_vs_bytes = framework::load_glsl(include_str!("texture.vert"),
                                                framework::ShaderStage::Vertex);
        let tex_fs_bytes = framework::load_glsl(include_str!("texture.frag"),
                                                framework::ShaderStage::Fragment,
        );
        let tex_vs_module = device.create_shader_module(&tex_vs_bytes);
        let tex_fs_module = device.create_shader_module(&tex_fs_bytes);

        let texture_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &texture_pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &tex_vs_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &tex_fs_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: sc_desc.format,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<TexVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float4],
                }],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let vs_bytes =
            framework::load_glsl(include_str!("shader.vert"), framework::ShaderStage::Vertex);
        let fs_bytes = framework::load_glsl(
            include_str!("shader.frag"),
            framework::ShaderStage::Fragment,
        );
        let vs_module = device.create_shader_module(&vs_bytes);
        let fs_module = device.create_shader_module(&fs_bytes);

        let pipeline = GuiProgram::create_pipeline(
            device,
            &sc_desc,
            &vs_module,
            &fs_module,
            &pipeline_layout,
        );

        let cfg = GUIConfig::from_file("config.cfg");
        let strings = GUIConfigStrings::from_cfg(&cfg);
        let start_state = match cfg.consented {
            true => UIState::Main,
            false => UIState::Consent,
        };
        let this = GuiProgram {
            vs_module,
            fs_module,
            pipeline_layout,
            pipeline,
            uniforms,
            transform,
            rebuild_pipeline: false,
            sc_desc: sc_desc.clone(),
            state_manager: ui::StateManager {
                fileroot: files::get_roots().unwrap(),
                config: cfg,
                strings,
                text_handler: Mutex::new(text::TextHandler::init(&device, sc_desc.format)),
                scroll: 0.0,
                state: start_state,
                upload_state: Default::default(),
                is_purge_done: Arc::new(AtomicBool::new(false)),
                cx: 0.0,
                cy: 0.0,
            },
            tex_vs_module,
            tex_fs_module,
            tex_pipeline: texture_pipeline,
            texture_bind_group,
            align: AlignConfig {
                scale: 1.0,
                win_width: sc_desc.width as f32,
                win_height: sc_desc.height as f32,
                tex_width: width as f32,
                tex_height: height as f32,
            },
            timer: 0.0,
        };

        (this, Some(init_encoder.finish()))
    }

    pub fn resize(
        &mut self,
        sc_desc: &wgpu::SwapChainDescriptor,
        device: &wgpu::Device,
    ) -> Option<wgpu::CommandBuffer> {
        self.sc_desc = sc_desc.clone();
        self.align.resize(sc_desc.width as f32, sc_desc.height as f32);

        // Update the transform matrix
        // 1. Generate new matrix
        let transform = ortho(0.0, sc_desc.width as f32, 0.0, sc_desc.height as f32, 1.0, -1.0);
        // 2. Create buffer
        let transform_buffer = device.create_buffer_with_data(
            transform.as_bytes(),
            wgpu::BufferUsage::COPY_SRC,
        );

        // 3. Create encoder to copy
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Resize encoder") });

        // 4. Copy to transform buffer
        encoder.copy_buffer_to_buffer(
            &transform_buffer,
            0,
            &self.transform,
            0,
            16 * 4,
        );

        Some(encoder.finish())
    }

    pub fn update(&mut self, event: winit::event::WindowEvent) {
        match event {
            winit::event::WindowEvent::KeyboardInput { input, .. } => {
                if let winit::event::ElementState::Pressed = input.state {
                    if input.virtual_keycode.is_some() {
                        ui::options::handle_keypress(self, &input.virtual_keycode.unwrap(), &input.modifiers)
                    }
                }
            },
            winit::event::WindowEvent::MouseWheel {
                delta: winit::event::MouseScrollDelta::LineDelta(_, y),
                ..
            } => {
                let max = filetree::compute_max_scroll(self);

                self.state_manager.scroll(y, self.align.scale, max);
            },
            winit::event::WindowEvent::MouseInput {device_id: _, state, button, modifiers: _} => {
                if state == winit::event::ElementState::Pressed  {
                    let but = match button {
                        winit::event::MouseButton::Left => 1,
                        winit::event::MouseButton::Right => 2,
                        winit::event::MouseButton::Middle => 3,
                        winit::event::MouseButton::Other(n) => n,
                    };
                    let state = match self.state_manager.state {
                        UIState::FileTree => ui::filetree::handle_click(self, but),
                        UIState::Main => ui::mainmenu::handle_click(self),
                        UIState::Options => ui::options::handle_click(self),
                        UIState::Consent => ui::consent::handle_click(self),
                        _ => None,
                    };
                    if let Some(state) = state {
                        if let UIState::Upload = state { ui::upload::start(self); }
                        self.state_manager.state = state;
                    }
                }
            },
            winit::event::WindowEvent::CursorMoved {device_id: _, position, modifiers: _} => {
                self.state_manager.cursor_moved(position.x as f32, position.y as f32);
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
            );
            self.rebuild_pipeline = false;
        }

        //// Check if we should swap state
        let next = match &self.state_manager.state {
            UIState::Purge => {
                if self.state_manager.is_purge_done.load(Ordering::Relaxed) {
                    Some(UIState::Main)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(next) = next { self.state_manager.state = next; }

        match &self.state_manager.state {
            UIState::FileTree => crate::ui::filetree::render(self, frame, device),
            UIState::Main => crate::ui::mainmenu::render(self, frame, device),
            UIState::Upload => crate::ui::upload::render(self, frame, device),
            UIState::Purge => crate::ui::purge::render(self, frame, device),
            UIState::Options => crate::ui::options::render(self, frame, device),
            UIState::Consent => crate::ui::consent::render(self, frame, device),
        }

    }

    // Saves the config when exiting
    pub fn exit(&mut self) {
        self.state_manager.strings.destring(&mut self.state_manager.config);
        let json = SerJson::serialize_json(&self.state_manager.config);
        std::fs::write("config.cfg",json).unwrap();
    }
}