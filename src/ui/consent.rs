use wgpu::BufferUsage;
use zerocopy::AsBytes;

use crate::gui::{GuiProgram, Vertex};
use crate::ui::align::Anchor;
use crate::ui::UIState;

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

    ///// Polygons
    let vertices = &mut Vertex::rect(gui.align.win_width/2.0 - 300.0, gui.align.win_height/2.0 - 300.0, 600.0, 600.0, [0.7,0.7,0.7,1.0]);

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
    th.draw("This program is a backup tool", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 250.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("It is NOT a synchronization tool - It can only upload files", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 220.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("This program targets Backblaze's B2 API", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 190.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("https://www.backblaze.com/b2/cloud-storage.html", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 160.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("Note that this project is not affiliated with Backblaze", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 130.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("Be aware of the costs. Refer to License.md for full terms of use.", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 100.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw("Must be configured before use", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 60.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("Requires authentication info, specifically an", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 - 30.0 ,
                     24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw_centered("'Application Key ID' and an 'Application Key'", gui.align.win_width/2.0, gui.align.win_height/2.0 + 20.0,
            36.0, f32::INFINITY, [0.25,0.05,0.05,1.0]);
    th.draw("Additionally, a Bucket ID has to be specified", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 50.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("Be aware that this is stored in plaintext!", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 80.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    th.draw("By clicking 'I Understand' you confirm that you have read and", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 120.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    th.draw("agree to the terms specified in License.md - Use at your own risk", gui.align.win_width/2.0 - 295.0, gui.align.win_height/2.0 + 150.0 ,
            24.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);


    // Flush text
    th.flush(&device,&mut encoder, frame, (gui.sc_desc.width,gui.sc_desc.height));


    let cb2 = encoder.finish();


    ///// Images
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    let vertices;
    // Use greyed out 'accept' until 10 seconds have passed
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

// Handle 'accept' click - Can only be pressed after 10 seconds
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