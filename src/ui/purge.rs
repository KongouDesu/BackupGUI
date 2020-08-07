use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use scoped_pool::Pool;
use wgpu::BufferUsage;
use zerocopy::AsBytes;
use std::sync::mpsc::Sender;

use crate::gui::GuiProgram;
use crate::ui::align::Anchor;

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

    let q = gui.state_manager.upload_state.queue.clone();
    let bid = gui.state_manager.config.bucket_id.clone();
    let tx = gui.state_manager.status_channel_tx.clone();
    let keystring = format!("{}:{}", gui.state_manager.config.app_key_id, gui.state_manager.config.app_key);

    std::thread::spawn(move || purge_task(q, bid, tx, keystring));
}

fn purge_task(q: Arc<Mutex<Vec<PathBuf>>>, bid: String, tx: Sender<String>, keystring: String) {
    // Get local files
    // Make sure the filetree is exactly the stored list
    let root = crate::files::get_roots().unwrap();
    if std::path::Path::new("backuplist.dat").exists() {
        root.deserialize("backuplist.dat");
    }
    root.get_files_for_upload(&q);

    // Collect all files that are supposed to be uploaded
    // On Unix, all paths start with '/' (the root). B2 will not emulate folders if we start file
    // paths with a slash, so we remove it during the upload process. 
    // This naturally means we have to remove it here to compare
    let lf = q.lock().unwrap();
    let mut local_files: Vec<String>;
    if cfg!(windows) {
        local_files = lf.iter().map(|x| x.to_string_lossy().replace("\\", "/")).collect();
    } else {
        local_files = lf.iter().map(|x| x.to_string_lossy().replace("\\", "/")[1..].to_string()).collect();
    }
    local_files.sort();
    println!("Collected local files");

    // Get list of files on server
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs_f32(30.0)).build().unwrap();

    let auth = match raze::api::b2_authorize_account(&client,keystring) {
        Ok(a) => a,
        Err(_e) => {
            tx.send("Authentication Failed".to_string()).unwrap();
            return;
        },
    };

    // Get list of files stored
    let remote_files = match raze::util::list_all_files(&client, &auth, &bid, 1000) {
        Ok(f) => f,
        Err(e) => {
            println!("Failed to get remote files - {:?}", e);
            tx.send("Failed talking to B2 - Check your Bucket ID".to_string()).unwrap();
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
    tx.send("Purge completed".to_string()).unwrap();
}
