use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use scoped_pool::Pool;
use wgpu::BufferUsage;
use zerocopy::AsBytes;

use crate::gui::GuiProgram;
use crate::ui::align::Anchor;
use std::sync::atomic::{Ordering, AtomicBool};

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

    // Images
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    //let mut vertices = TexVertex::rect(700.0, 200.0, 600.0, 800.0, gui.timer);
    let vertices = gui.align.image(Anchor::CenterGlobal, 0.0, 0.0, 256.0, 256.0, gui.timer, Some([0.0,0.0,256.0,256.0]));

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
                        clear_color: wgpu::Color::WHITE,
                    },
                ],
                depth_stencil_attachment: None,
            },
        );
    }

    gui.state_manager.text_handler.lock().unwrap().draw_centered("Clearing unused files...", gui.align.win_width/2.0, gui.align.win_height/2.0 - 300.0,
                                                                 96.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);
    gui.state_manager.text_handler.lock().unwrap().draw_centered("In progress, please wait", gui.align.win_width/2.0, gui.align.win_height/2.0 + 300.0,
                                                                 96.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    gui.state_manager.text_handler.lock().unwrap().flush(&device,&mut encoder, frame, (gui.sc_desc.width,gui.sc_desc.height));
    let cb2 = encoder.finish();


    vec![cb1,cb2]
}

// Start the purge thread to run in the background
pub fn start_purge_thread(gui: &mut GuiProgram) {
    println!("Start purge");
    gui.state_manager.is_purge_done.swap(false, Ordering::Relaxed);

    let q = gui.state_manager.upload_state.queue.clone();
    gui.state_manager.fileroot.get_files_for_upload(&q);
    let bid = gui.state_manager.config.bucket_id.clone();
    let done = gui.state_manager.is_purge_done.clone();

    std::thread::spawn(move || purge_task(q, bid, done));
}

fn purge_task(q: Arc<Mutex<Vec<PathBuf>>>, bid: String, done: Arc<AtomicBool>) {
    // Collect all files that are supposed to be uploaded
    let local_files = q.lock().unwrap();
    let mut local_files: Vec<String> = local_files.iter().map(|x| x.to_string_lossy().replace("\\", "/")).collect();
    local_files.sort();
    println!("Collected local files");

    // Get list of files on server
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs_f32(30.0)).build().unwrap();
    // TODO Handle missing auth gracefully
    let auth = raze::util::authenticate_from_file(&client,"credentials").unwrap();

    // Avoid crashing the program if it fails
    let remote_files = match raze::util::list_all_files(&client, &auth, &bid, 1000) {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to get remote files - {:?}", e);
            return
        },
    };
    println!("Collected remote files");

    // Compare the two lists:
    // Check each file in the cloud; if it isn't in the upload list, queue it for hiding
    let mut hide_list = vec![];
    for file in remote_files {
        match local_files.binary_search(&file.file_name) {
            Ok(_) => (),
            Err(_) => hide_list.push(file.file_name),
        }
    }
    println!("Ready to hide {} files", hide_list.len());
    let hide_list = Arc::new(Mutex::new(hide_list));


    let pool = Pool::new(16);
    // Spawn hide threads
    pool.scoped(|scope| {
        for _i in 0..pool.workers() {
            let hl = hide_list.clone();
            let bid = bid.clone();
            let client = &client;
            let auth = &auth;
            scope.execute(move || {
                loop {
                    let p = {
                        hl.lock().unwrap().pop()
                    };
                    let file = match p {
                        Some(f) => f,
                        None => break, // No more files to hide
                    };

                    println!("Hiding {:?}", file);
                    for _i in 0..5 {
                        let res = raze::api::b2_hide_file(&client, &auth, &bid, &file);
                        match res {
                            Ok(_) => break, // Break on success = do not retry
                            Err(e) => { // Continue on failure = retry
                                println!("Err {:?}, retrying {:?}", e, file);
                                continue
                            },
                        }
                    }
                }
            });
        }
    });

    println!("Done purging");
    done.swap(true, Ordering::Relaxed);
}