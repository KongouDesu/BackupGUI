use wgpu::BufferUsage;
use zerocopy::AsBytes;

use crate::files::{Action, DirEntry};
use crate::gui::{GuiProgram, Vertex};
use crate::ui::align::Anchor;
use crate::ui::UIState;
use std::sync::atomic::Ordering;

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

    // Draw the tree itself
    // This function returns a list of vertices that when drawn makes up the background of the tree
    // It will also fill the text buffer with the appropriate sections - all we need to do is flush it
    let mut vertices = render_file_tree(gui);
    vertices.append(&mut super::Vertex::rect(0.0, 0.0, gui.align.win_width, 32.0, [0.0,0.0,0.0,1.0]));

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    if !vertices.is_empty() {
        let buffer = device.create_buffer_with_data(vertices.as_bytes(), BufferUsage::VERTEX);

        let rpass_color_attachment = {
            wgpu::RenderPassColorAttachmentDescriptor {
                attachment: &frame.view,
                resolve_target: None,
                load_op: wgpu::LoadOp::Clear,
                store_op: wgpu::StoreOp::Store,
                clear_color: wgpu::Color::BLACK,
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

    ////// Images
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    let vertices = gui.align.image(Anchor::TopRight, 0.0, 0.0, 64.0, 32.0, 0.0, Some([0.0,588.0,128.0,64.0]));
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

    gui.state_manager.text_handler.lock().unwrap().draw("File tree", 0.0, 0.0, 32.0, f32::INFINITY, [1.0,1.0,1.0,1.0]);
    let cb2 = encoder.finish();


    ///// Render text
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Text") });

    // Draw on top of previous (i.e. on the background of the tree)
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

    gui.state_manager.text_handler.lock().unwrap().flush(&device,&mut encoder, frame, (gui.sc_desc.width,gui.sc_desc.height));

    let cb3 = encoder.finish();

    vec![cb1,cb2,cb3]
}

// Renders the file tree
// The top-left corner is at (0,0) (in screen coordinates)
// Note: text handler must be flushed manually to render the text
// Returns a vec of vertices representing what it wants to draw
fn render_file_tree(gui: &crate::GuiProgram) -> Vec<Vertex> {
    let mut y = gui.state_manager.scroll + 32.0;
    let indent = 0f32;
    let mut vertices: Vec<Vertex> = Vec::new();

    // Render background, note that font_size determines height
    for entry in gui.state_manager.fileroot.children.lock().unwrap().iter() {
        let res = render_subtree(gui, entry, y, indent, vertices);
        y = res.0;
        vertices = res.1;
    }

    // Render text, note that font_size determines height
    let mut y = gui.state_manager.scroll + 32.0;
    for entry in gui.state_manager.fileroot.children.lock().unwrap().iter() {
        y = render_subtree_text(gui, entry, y, indent);
    }
    vertices
}

fn render_subtree(gui: &crate::GuiProgram, root: &DirEntry, mut y: f32, mut indent: f32, mut vertex_buffer: Vec<Vertex>) -> (f32,Vec<Vertex>) {
    // Render gui, though only if within visible area
    if y >= -gui.state_manager.config.font_size && y <= gui.align.win_height {
        if *root.action.lock().unwrap() == Action::Exclude {
            vertex_buffer.append(&mut gui.align.rectangle(Anchor::TopLeft, indent, y,
                                                          gui.align.win_width-indent, gui.state_manager.config.font_size, [0.8,0.0,0.0,1.0]));
        } else if *root.action.lock().unwrap() == Action::Upload {
            vertex_buffer.append(&mut gui.align.rectangle(Anchor::TopLeft, indent, y,
                                                          gui.align.win_width-indent, gui.state_manager.config.font_size, [0.0,0.8,0.0,1.0]));
        }
    } else if y > gui.align.win_height {
        // We will never return to the visible area, stop drawing
        return (y,vertex_buffer);
    }

    // Note: step size determined by font_size
    y += gui.state_manager.config.font_size;

    // Render children
    if root.expanded.load(Ordering::Relaxed) {
        indent += 24.0f32;
        for entry in root.children.lock().unwrap().iter() {
            let res = render_subtree(gui, entry, y, indent, vertex_buffer);
            y = res.0;
            vertex_buffer = res.1;
        }
    }
    (y,vertex_buffer)
}

fn render_subtree_text(gui: &crate::GuiProgram, root: &DirEntry, mut y: f32, mut indent: f32) -> f32 {
    // Draw gui if within visible area
    if y >= 32.0 && y <= gui.align.win_height {
        gui.state_manager.text_handler.lock().unwrap().draw(&root.name, indent+2.0, y,
                                                            gui.state_manager.config.font_size, gui.align.win_width-indent-2.0, [1.0,1.0,1.0,1.0]);
    } else if y > gui.align.win_height {
        // We will never return to the visible area, stop drawing
        return y;
    }

    // Note: step size determined by font_size
    y += gui.state_manager.config.font_size;

    // Render children
    if root.expanded.load(Ordering::Relaxed) {
        indent += 24.0f32;
        for entry in root.children.lock().unwrap().iter() {
            y = render_subtree_text(gui, entry, y, indent);
        }
    }
    y
}

pub fn handle_click(gui: &GuiProgram, button: u8) -> Option<UIState> {
    if gui.align.was_area_clicked(Anchor::TopRight, gui.state_manager.cx, gui.state_manager.cy, 0.0, 0.0, 64.0, 32.0) {
        println!("Return to Main -- Saving tree");
        gui.state_manager.fileroot.serialize("backuplist.dat");
        Some(UIState::Main)
    } else if gui.state_manager.cy >= 32.0 { // Only check for y>32 to exclude the top bar
        // Check if we clicked on an item in the tree
        // First we offset 'y' to match the 'scroll' value
        let mut y = gui.state_manager.cy - gui.state_manager.scroll - 32.0;
        println!("Start search {}, button {}", y, button);

        // Iterate over items in the root node, stopping if we click was handled
        for entry in gui.state_manager.fileroot.children.lock().unwrap().iter() {
            let temp = handle_click_rec(gui, entry, 0.0, y, button);
            y = temp.0;
            if temp.1 { // If we found what we clicked on, stop
                break;
            }
        }
        None
    } else {
        None
    }
}

// Recursive part of click handling
// Each (visible) entry decrement 'y' by font_size (it's height)
// Once 'y' is <= font_size, it means we found our entry
fn handle_click_rec(gui: &GuiProgram, entry: &DirEntry, x: f32, mut y: f32, button: u8) -> (f32, bool) {
    // Check if we found our entry, if we did, handle the click and stop

    if y <= gui.state_manager.config.font_size {
        println!("Click {:?}, button {:?}", entry.name, button);
        if button == 1 {
            // Toggle visibility
            if entry.expanded.load(Ordering::Relaxed) {
                entry.expanded.swap(false, Ordering::Relaxed);
            } else {
                // This refreshes the dir and expands it
                if !entry.indexed.load(Ordering::Relaxed) {
                    entry.expand();
                }
                entry.expanded.swap(true, Ordering::Relaxed);
            }
        } else if button == 2 {
            // Change action
            if *entry.action.lock().unwrap() == Action::Exclude {
                entry.change_action(Action::Upload);
            } else if *entry.action.lock().unwrap() == Action::Upload {
                entry.change_action(Action::Exclude);
            }
        }
        println!("{:?}", entry.action.lock().unwrap());
        return (y,true)
    }

    // If we didn't find it, search further
    y -= gui.state_manager.config.font_size;

    // Notice: Only search expanded (visible) entries, as we cant click invisible ones
    if entry.expanded.load(Ordering::Relaxed) {
        let mut done;
        for entry in entry.children.lock().unwrap().iter() {
            let temp = handle_click_rec(gui, entry, x, y, button);
            y = temp.0;
            done = temp.1;
            if done {
                return (y, true);
            }
        }
    }
    (y,false)
}

// Returns the maximum amount that we can scroll down
// This value is equal to the total visible size of the tree minus the size of one entry
pub fn compute_max_scroll(gui: &GuiProgram) -> f32 {
    let mut height = 0.0;
    for entry in gui.state_manager.fileroot.children.lock().unwrap().iter() {
        height += get_height_rec(gui, entry, 0.0);
    }
    height - gui.state_manager.config.font_size
}

// Recursive part of 'compute_max_scroll'
fn get_height_rec(gui: &GuiProgram, entry: &DirEntry, mut y: f32) -> f32 {
    y += gui.state_manager.config.font_size;
    if entry.expanded.load(Ordering::Relaxed) {
        for entry in entry.children.lock().unwrap().iter() {
            y += get_height_rec(gui, entry, 0.0);
        }
    }
    y
}

