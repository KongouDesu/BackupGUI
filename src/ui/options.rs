use wgpu::BufferUsage;
use zerocopy::AsBytes;

use crate::gui::{GuiProgram, Vertex};
use crate::ui::UIState;
use crate::ui::align::Anchor;
use winit::event::{VirtualKeyCode, ModifiersState};

use clipboard::ClipboardProvider;
use clipboard::ClipboardContext;
use std::error::Error;

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

    ///// Polygons
    let mut vertices = vec![];
    for i in 0..7 {
        let col_left = match i % 2 {
            0 => [0.2,0.2,0.2,1.0],
            1 => [0.3,0.3,0.3,1.0],
            _ => [0.0,0.0,0.0,1.0]
        };
        let col_right = if gui.state_manager.strings.active_field == i+1 {
            [0.5,0.5,0.5,1.0]
        } else {
            match i % 2 {
                0 => [0.3,0.3,0.3,1.0],
                1 => [0.2,0.2,0.2,1.0],
                _ => [0.0,0.0,0.0,1.0]
            }
        };
        let i = i as f32;
        vertices.append(&mut Vertex::rect(gui.align.win_width/2.0 - 300.0, gui.align.win_height/2.0 - 225.0 + 50.0*i, 300.0, 50.0, col_left));
        vertices.append(&mut Vertex::rect(gui.align.win_width/2.0, gui.align.win_height/2.0 - 225.0 + 50.0*i, 300.0, 50.0, col_right));
    }

    vertices.append(&mut gui.align.rectangle(Anchor::CenterGlobal, 173.0, 248.0,173.0,175.0, [0.8,0.8,0.8,1.0]));



    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    {
        let buffer = device.create_buffer_with_data(vertices.as_bytes(), BufferUsage::VERTEX);

        let rpass_color_attachment = {
            wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &frame.view,
                resolve_target: None,
                load_op: wgpu::LoadOp::Clear,
                store_op: wgpu::StoreOp::Store,
                clear_color: wgpu::Color::WHITE,
            }
        };

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[rpass_color_attachment],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&gui.pipeline);
        rpass.set_bind_group(0, &gui.uniforms, &[]);
        rpass.set_vertex_buffer(0, &buffer, 0, 0);

        rpass.draw(0..vertices.len() as u32, 0..1);
    }

    let cb1 = encoder.finish();

    ///// Text
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Text") });

    {
        let _ = encoder.begin_render_pass(
            &wgpu::RenderPassDescriptor {
                color_attachments: &[
                    wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &frame.view,
                        resolve_target: None,
                        load_op: wgpu::LoadOp::Load,
                        store_op: wgpu::StoreOp::Store,
                        clear_color: wgpu::Color::WHITE,
                    },
                ],
                depth_stencil_attachment: None,
            },
        );
    }

    // Header text
    let mut th = gui.state_manager.text_handler.lock().unwrap();
    th.draw_centered("Options", gui.align.win_width/2.0, gui.align.win_height/2.0 - 300.0,
                                                                 96.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    // Draw options
    th.draw_centered("Font size", gui.align.win_width/2.0 - 150.0, gui.align.win_height/2.0 - 200.0 ,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered(&gui.state_manager.strings.font_size, gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 200.0,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw_centered("Scroll speed", gui.align.win_width/2.0 - 150.0, gui.align.win_height/2.0 - 150.0 ,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered(&gui.state_manager.strings.scroll_factor, gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 150.0,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw_centered("Application Key ID", gui.align.win_width/2.0 - 150.0, gui.align.win_height/2.0 - 100.0,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered(&gui.state_manager.strings.app_key_id, gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 100.0,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw_centered("Application Key", gui.align.win_width/2.0 - 150.0, gui.align.win_height/2.0 - 50.0,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered(&gui.state_manager.strings.app_key, gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 50.0,
                     20.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw_centered("Bucket ID", gui.align.win_width/2.0 - 150.0, gui.align.win_height/2.0,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered(&gui.state_manager.strings.bucket_id,
                                                                 gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw_centered("Bandwidth limit (KB/s)", gui.align.win_width/2.0 - 150.0, gui.align.win_height/2.0 + 50.0 ,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered(&gui.state_manager.strings.bandwidth_limit,
                                                                 gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 + 50.0,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw_centered("Hide file names", gui.align.win_width/2.0 - 150.0, gui.align.win_height/2.0 + 100.0 ,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    let bool_as_yes_no = match gui.state_manager.config.hide_file_names {
        true => "Yes",
        false => "No",
    };
    th.draw_centered(bool_as_yes_no, gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 + 100.0,
                                                                 24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);



    th.draw_centered("Start Purge", gui.align.win_width/2.0 - 225.0, gui.align.win_height/2.0 + 150.0,
                     48.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered("Cleans up the cloud, removing old files", gui.align.win_width/2.0 - 225.0, gui.align.win_height/2.0 + 180.0,
                     24.0, 460.0, [0.05,0.05,0.05,1.0]);
    th.draw_centered("Compares files on disk with files on cloud", gui.align.win_width/2.0 - 225.0, gui.align.win_height/2.0 + 210.0,
                     24.0, 460.0, [0.05,0.05,0.05,1.0]);
    th.draw_centered("Files in cloud that cant be found on disk are removed", gui.align.win_width/2.0 - 225.0, gui.align.win_height/2.0 + 240.0,
                     24.0, 460.0, [0.05,0.05,0.05,1.0]);
    th.draw_centered("This doesn't delete files but hides them", gui.align.win_width/2.0 - 225.0, gui.align.win_height/2.0 + 270.0,
                     24.0, 460.0, [0.05,0.05,0.05,1.0]);
    th.draw_centered("Configure the lifecycle settings to adjust behavior", gui.align.win_width/2.0 - 225.0, gui.align.win_height/2.0 + 300.0,
                     24.0, 460.0, [0.05,0.05,0.05,1.0]);
    th.draw_centered("Purging can take a few minutes", gui.align.win_width/2.0 - 225.0, gui.align.win_height/2.0 + 330.0,
                     24.0, 460.0, [0.05,0.05,0.05,1.0]);

    // Flush text
    th.flush(&device,&mut encoder, frame, (gui.sc_desc.width,gui.sc_desc.height));


    let cb2 = encoder.finish();


    ///// Images
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    let mut vertices = gui.align.image(Anchor::TopRight, 0.0, 0.0, 64.0, 32.0, 0.0, Some([0.0,651.0,128.0,64.0]));
    vertices.append(&mut gui.align.image(Anchor::CenterGlobal, 175.0, 250.0, 169.0, 171.0, 0.0, Some([180.0,234.0,169.0,171.0])));
    let buffer = device.create_buffer_with_data(vertices.as_bytes(), BufferUsage::VERTEX);

    let rpass_color_attachment =  {
        wgpu::RenderPassColorAttachmentDescriptor {
            attachment: &frame.view,
            resolve_target: None,
            load_op: wgpu::LoadOp::Load,
            store_op: wgpu::StoreOp::Store,
            clear_color: wgpu::Color::WHITE,
        }
    };

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[rpass_color_attachment],
            depth_stencil_attachment: None,
        });

        rpass.set_pipeline(&gui.tex_pipeline);
        rpass.set_bind_group(0, &gui.uniforms, &[]);
        rpass.set_bind_group(1, &gui.texture_bind_group, &[]);
        rpass.set_vertex_buffer(0, &buffer, 0, 0);

        rpass.draw(0..vertices.len() as u32, 0..1);
    }

    let cb3 = encoder.finish();


    vec![cb1,cb2,cb3]
}

pub fn handle_click(gui: &mut GuiProgram) -> Option<UIState> {
    if gui.align.was_area_clicked(Anchor::CenterLocal, gui.state_manager.cx, gui.state_manager.cy,
                                  gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 200.0,
                                    300.0, 50.0) {
        gui.state_manager.strings.active_field = 1;
    } else if gui.align.was_area_clicked(Anchor::CenterLocal, gui.state_manager.cx, gui.state_manager.cy,
                                         gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 150.0,
                                         300.0, 50.0) {
        gui.state_manager.strings.active_field = 2;
    } else if gui.align.was_area_clicked(Anchor::CenterLocal, gui.state_manager.cx, gui.state_manager.cy,
                                         gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 100.0,
                                         300.0, 50.0) {
        gui.state_manager.strings.active_field = 3;
    } else if gui.align.was_area_clicked(Anchor::CenterLocal, gui.state_manager.cx, gui.state_manager.cy,
                                         gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 - 50.0,
                                         300.0, 50.0) {
        gui.state_manager.strings.active_field = 4;
    } else if gui.align.was_area_clicked(Anchor::CenterLocal, gui.state_manager.cx, gui.state_manager.cy,
                                         gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0,
                                         300.0, 50.0) {
        gui.state_manager.strings.active_field = 5;
    } else if gui.align.was_area_clicked(Anchor::CenterLocal, gui.state_manager.cx, gui.state_manager.cy,
                                         gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 + 50.0,
                                         300.0, 50.0) {
        gui.state_manager.strings.active_field = 6;
    } else if gui.align.was_area_clicked(Anchor::CenterLocal, gui.state_manager.cx, gui.state_manager.cy,
                                         gui.align.win_width/2.0 + 150.0, gui.align.win_height/2.0 + 100.0,
                                         300.0, 50.0) {
        gui.state_manager.config.hide_file_names = !gui.state_manager.config.hide_file_names;
    } else {
        gui.state_manager.strings.active_field = 0;
    }
    gui.state_manager.strings.destring(&mut gui.state_manager.config);

    if gui.align.was_area_clicked(Anchor::TopRight, gui.state_manager.cx, gui.state_manager.cy, 0.0, 0.0, 64.0, 32.0) {
        gui.save_config();
        return Some(UIState::Main)
    } else if gui.align.was_area_clicked(Anchor::CenterGlobal, gui.state_manager.cx, gui.state_manager.cy, 173.0, 248.0, 173.0, 175.0,) {
        gui.save_config();
        crate::ui::purge::start_purge_thread(gui);
        return Some(UIState::Purge)
    }
    None
}

pub fn handle_keypress(gui: &mut GuiProgram, key: &VirtualKeyCode, mods: &ModifiersState) {
    match key {
        // Backspace key
        VirtualKeyCode::Back => {
            match gui.state_manager.strings.active_field {
                1 => {gui.state_manager.strings.font_size.pop();},
                2 => {gui.state_manager.strings.scroll_factor.pop();},
                3 => {gui.state_manager.strings.app_key_id.pop();},
                4 => {gui.state_manager.strings.app_key.pop();},
                5 => {gui.state_manager.strings.bucket_id.pop();},
                6 => {gui.state_manager.strings.bandwidth_limit.pop();},
                _ => ()
            }
        },
        _ => {
            // TODO Prettier way to handle this?
            let mut ch = match key {
                VirtualKeyCode::A => 'a',
                VirtualKeyCode::B => 'b',
                VirtualKeyCode::C => 'c',
                VirtualKeyCode::D => 'd',
                VirtualKeyCode::E => 'e',
                VirtualKeyCode::F => 'f',
                VirtualKeyCode::G => 'g',
                VirtualKeyCode::H => 'h',
                VirtualKeyCode::I => 'i',
                VirtualKeyCode::J => 'j',
                VirtualKeyCode::K => 'k',
                VirtualKeyCode::L => 'l',
                VirtualKeyCode::M => 'm',
                VirtualKeyCode::N => 'n',
                VirtualKeyCode::O => 'o',
                VirtualKeyCode::P => 'p',
                VirtualKeyCode::Q => 'q',
                VirtualKeyCode::R => 'r',
                VirtualKeyCode::S => 's',
                VirtualKeyCode::T => 't',
                VirtualKeyCode::U => 'u',
                VirtualKeyCode::V => 'v',
                VirtualKeyCode::W => 'w',
                VirtualKeyCode::X => 'x',
                VirtualKeyCode::Y => 'y',
                VirtualKeyCode::Z => 'z',
                VirtualKeyCode::Key0 => '0',
                VirtualKeyCode::Key1 => '1',
                VirtualKeyCode::Key2 => '2',
                VirtualKeyCode::Key3 => '3',
                VirtualKeyCode::Key4 => '4',
                VirtualKeyCode::Key5 => '5',
                VirtualKeyCode::Key6 => '6',
                VirtualKeyCode::Key7 => '7',
                VirtualKeyCode::Key8 => '8',
                VirtualKeyCode::Key9 => '9',
                _ => return,
            };
            if mods.ctrl() && ch == 'v' {
                let ctx: Result<ClipboardContext, Box<dyn Error>>  = ClipboardProvider::new();
                match ctx {
                    Ok(mut c) => {
                        match c.get_contents() {
                            Ok(s) => {
                                match gui.state_manager.strings.active_field {
                                    1 => {gui.state_manager.strings.font_size.push_str(&s);},
                                    2 => {gui.state_manager.strings.scroll_factor.push_str(&s);},
                                    3 => {gui.state_manager.strings.app_key_id.push_str(&s);},
                                    4 => {gui.state_manager.strings.app_key.push_str(&s);},
                                    5 => {gui.state_manager.strings.bucket_id.push_str(&s);},
                                    6 => {gui.state_manager.strings.bandwidth_limit.push_str(&s);},
                                    _ => ()
                                }
                            },
                            Err(_e) => ()
                        }
                    }
                    Err(_e) => (),
                };
            } else {
                if mods.shift() { ch = ch.to_ascii_uppercase(); }
                match gui.state_manager.strings.active_field {
                    1 => {gui.state_manager.strings.font_size.push(ch);},
                    2 => {gui.state_manager.strings.scroll_factor.push(ch);},
                    3 => {gui.state_manager.strings.app_key_id.push(ch);},
                    4 => {gui.state_manager.strings.app_key.push(ch);},
                    5 => {gui.state_manager.strings.bucket_id.push(ch);},
                    6 => {gui.state_manager.strings.bandwidth_limit.push(ch);},
                    _ => ()
                }
            }
        }
    }
}
