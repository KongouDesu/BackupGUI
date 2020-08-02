use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raze;
use raze::api::Sha1Variant;
use reqwest;
use scoped_pool::Pool;
use wgpu::{BufferDescriptor, BufferUsage, vertex_attr_array};
use zerocopy::{AsBytes, FromBytes};

use crate::files::{Action, DirEntry, EntryKind};
use crate::files::tracked_reader::TrackedReader;
use crate::gui::{GuiProgram, Vertex};
use crate::gui::TexVertex;
use crate::ui::{UIState, UploadInstance};
use crate::ui::align::Anchor;
use winit::event::{VirtualKeyCode, ModifiersState};

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

    ///// Polygons
    let mut vertices = &mut Vertex::rect(gui.align.win_width/2.0 - 300.0, gui.align.win_height/2.0 - 300.0, 600.0, 600.0, [0.7,0.7,0.7,1.0]);

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
    th.draw_centered("Notice", gui.align.win_width/2.0, gui.align.win_height/2.0 - 300.0,
                     96.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    // Draw options
    th.draw("This program is a backup tool", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 200.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("It is NOT a synchronization tool - It can only upload files", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 170.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("This program targets Backblaze's B2 API", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 140.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("https://www.backblaze.com/b2/cloud-storage.html", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 110.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("Note that this project is not affiliated with Backblaze", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 80.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("Be aware of the costs. Refer to License.md for full terms of use.", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 50.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw("In order to work, you must provide a file named 'credentials'", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 10.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("It must consist of one line with the format:", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 20.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered("applicationKeyId:applicationKey", gui.align.win_width/2.0, gui.align.win_height/2.0 + 70.0 ,
            36.0, f32::INFINITY, [0.25,0.05,0.05,1.0]);
    th.draw("Additionally, a bucket ID has to be specified in the options", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 100.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw("By clicking 'I Understand' you confirm that you have read and", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 140.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("agree to the terms specified in License.md - Use at your own risk", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 170.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);





    // Flush text
    th.flush(&device,&mut encoder, frame, (gui.sc_desc.width,gui.sc_desc.height));


    let cb2 = encoder.finish();


    ///// Images
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    let mut vertices;
    if gui.timer < 10.0 {
        vertices = gui.align.image(Anchor::CenterGlobal, 0.0, 250.0, 200.0, 62.0, 0.0, Some([0.0,781.0,200.0,62.0]));
    } else {
        vertices = gui.align.image(Anchor::CenterGlobal, 0.0, 250.0, 200.0, 62.0, 0.0, Some([0.0,718.0,200.0,62.0]));
    }

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
    if gui.align.was_area_clicked(Anchor::CenterGlobal, gui.state_manager.cx, gui.state_manager.cy,
                                  0.0, 250.0,
                                  200.0, 62.0) && gui.timer >= 10.0 {
        gui.state_manager.config.consented = true;
        Some(UIState::Main)
    } else {
        None
    }
}