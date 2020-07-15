use crate::gui::GuiProgram;
use wgpu::{vertex_attr_array, BufferDescriptor, BufferUsage};
use zerocopy::{AsBytes, FromBytes};
use crate::gui::TexVertex;

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    let mut vertices = TexVertex::rect(700.0, 200.0, 600.0, 800.0, gui.timer);
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


        rpass.draw(0..vertices.len() as u32, 0..3);
    }

    let cb = encoder.finish();


    vec![cb]
}