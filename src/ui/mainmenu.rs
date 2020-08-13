use wgpu::BufferUsage;
use zerocopy::AsBytes;

use crate::gui::GuiProgram;
use crate::ui::align::Anchor;
use crate::ui::UIState;

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    let mut vertices = gui.align.image(Anchor::CenterGlobal, 196.0, 100.0, 196.0, 196.0, gui.timer, Some([0.0,0.0,256.0,256.0]));
    vertices.append(&mut gui.align.image(Anchor::CenterGlobal, 0.0, 100.0, 180.0, 180.0, 0.0,Some([0.0,406.0,180.0,180.0])));
    vertices.append(&mut gui.align.image(Anchor::CenterGlobal, -196.0, 100.0, 179.0, 148.0, 0.0,Some([0.0,257.0,179.0,148.0])));

    let buffer = device.create_buffer_with_data(vertices.as_bytes(), BufferUsage::VERTEX);

    let rpass_color_attachment =  {
        wgpu::RenderPassColorAttachmentDescriptor {
            attachment: &frame.view,
            resolve_target: None,
            load_op: wgpu::LoadOp::Clear,
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

    let cb1 = encoder.finish();


    ///// Text
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Text") });

    // Draw on top of previous
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

    gui.state_manager.text_handler.lock().unwrap().draw_centered("Backup", gui.align.win_width/2.0, gui.align.win_height/2.0 - 200.0, 128.0, f32::INFINITY, [0.0,0.0,0.0,1.0]);

    if let Some(s) = &gui.state_manager.status_message {
        gui.state_manager.text_handler.lock().unwrap().draw_centered(s, gui.align.win_width/2.0, gui.align.win_height/2.0 - 75.0, 48.0, f32::INFINITY, [0.7,0.0,0.0,1.0]);
    }

    gui.state_manager.text_handler.lock().unwrap().flush(&device,&mut encoder, frame, (gui.sc_desc.width,gui.sc_desc.height));
    let cb2 = encoder.finish();


    vec![cb1,cb2]
}

// We have 3 buttons each taking us to different states
pub fn handle_click(gui: &mut GuiProgram) -> Option<UIState> {
    if gui.align.was_area_clicked(Anchor::CenterGlobal, gui.state_manager.cx, gui.state_manager.cy, -196.0, 100.0, 179.0, 148.0) {
        println!("Swapping state to FileTree");
        if std::path::Path::new("backuplist.dat").exists() {
            gui.state_manager.fileroot.deserialize("backuplist.dat");
        }
        gui.state_manager.status_message = None;
        Some(UIState::FileTree)
    } else if gui.align.was_area_clicked(Anchor::CenterGlobal, gui.state_manager.cx, gui.state_manager.cy, 0.0, 100.0, 180.0, 180.0) {
        println!("Swapping state to Upload");
        gui.state_manager.status_message = None;
        Some(UIState::Upload)
    } else if gui.align.was_area_clicked(Anchor::CenterGlobal, gui.state_manager.cx, gui.state_manager.cy, 196.0, 100.0, 196.0, 148.0) {
        println!("Swapping state to Options");
        gui.state_manager.status_message = None;
        Some(UIState::Options)
    } else {
        None
    }
}