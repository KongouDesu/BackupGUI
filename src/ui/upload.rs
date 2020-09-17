use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raze::api::{Sha1Variant};
use scoped_pool::Pool;
use wgpu::BufferUsage;
use zerocopy::AsBytes;
use std::sync::mpsc::Sender;

use crate::files::tracked_reader::TrackedReader;
use crate::gui::{GuiProgram, Vertex};
use crate::ui::UploadInstance;

pub fn render(
    gui: &mut GuiProgram,
    frame: &wgpu::SwapChainOutput,
    device: &wgpu::Device,
) -> Vec<wgpu::CommandBuffer> {

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

    gui.state_manager.text_handler.lock().unwrap().draw_centered("Uploading", gui.align.win_width/2.0, gui.align.win_height/2.0 - 300.0,
                                                                 128.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);


    // Generate vertices and write text for progress bar
    let mut vertices: Vec<Vertex> = Vec::with_capacity(6*8*2); // 8 bars w/ 2 rectangles of 6 points each
    let mut instance_vec = gui.state_manager.upload_state.instances.lock().unwrap();
    const BAR_WIDTH: f32 = 700.0;
    const BAR_HEIGHT: f32 = 40.0;
    const BAR_SPACING: f32 = 8.0;
    let bar_start_y = (gui.sc_desc.height as f32)/2.0 - (4.0 * (BAR_HEIGHT + BAR_SPACING)) + ((BAR_HEIGHT+BAR_SPACING)/2.0);
    for i in 0..8 {
        // Back bar
        vertices.append(&mut super::Vertex::rect((gui.sc_desc.width as f32)/2.0-BAR_WIDTH/2.0,bar_start_y+(BAR_SPACING+BAR_HEIGHT)*i as f32,
                                                 BAR_WIDTH, BAR_HEIGHT, [0.05,0.05,0.05,1.0]));

        // Fill
        while let Ok(amount) = instance_vec[i].receiver.try_recv() {
            instance_vec[i].progress += amount;
        }
        let width = (BAR_WIDTH-2.0)*instance_vec[i].progress as f32/instance_vec[i].size as f32;
        vertices.append(&mut super::Vertex::rect((gui.sc_desc.width as f32)/2.0-BAR_WIDTH/2.0 + 1.0,bar_start_y+(BAR_SPACING+BAR_HEIGHT)*i as f32 + 1.0,
                                                 width, BAR_HEIGHT - 2.0, [0.1,0.3,0.1,1.0]));

        // Text (showing file name)
        if !gui.state_manager.config.hide_file_names {
            gui.state_manager.text_handler.lock().unwrap().draw_centered(&instance_vec[i].name, (gui.sc_desc.width as f32) / 2.0,
                                                                         bar_start_y + (BAR_SPACING + BAR_HEIGHT) * i as f32 + BAR_HEIGHT / 2.0,
                                                                         20.0, BAR_WIDTH - 2.0, [0.5, 0.9, 0.8, 1.0]);
        }
    }

    // Write number of files remaining
    let rem = {
        gui.state_manager.upload_state.queue.lock().unwrap().len()
    };
    gui.state_manager.text_handler.lock().unwrap().draw_centered(&format!("Remaining: {}",rem), gui.align.win_width/2.0, gui.align.win_height/2.0 + 300.0,
                                                                 64.0, f32::INFINITY, [0.05,0.05,0.05,1.0]);

    gui.state_manager.text_handler.lock().unwrap().flush(&device,&mut encoder, frame, (gui.sc_desc.width,gui.sc_desc.height));
    let cb2 = encoder.finish();


    // Progress bars
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

    // Runs _before_ text, so this has LoadOp::Clear
    if !vertices.is_empty() {
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
    let cb3 = encoder.finish();

    vec![cb3,cb2]
}

// Start uploading files
pub fn start(gui: &mut GuiProgram) {
    // Create a new queue
    // The 'get_files_for_upload' is not stopped even if the main upload thread ends
    // That means if we get an auth error, it would still be counting
    // Since counting can sometimes take minutes (e.g. cold drives) we could end up having
    // more than one 'get_files_for_upload' threads running, resulting in duplicate files in the queue
    // So, to fix it we make a new queue here
    // TODO Stop 'get_files_for_upload' if the other upload thread exits
    gui.state_manager.upload_state.queue = Arc::new(Mutex::new(vec![]));

    // Start the thread that queues files for upload
    // First, make sure the file-tree is read
    let root = crate::files::get_roots().unwrap();
    match root.deserialize("backuplist.dat") {
        Ok(_) => {},
        Err(e) => {
            println!("Error reading backuplist.dat - Reason: {:?}", e);
            gui.state_manager.status_channel_tx.send("Got no files for upload, make sure you've selected some".to_string()).unwrap();
            return
        },
    }
    let q = gui.state_manager.upload_state.queue.clone();
    std::thread::spawn(move || root.get_files_for_upload(&q));

    // Start the upload threads
    let q = gui.state_manager.upload_state.queue.clone();
    let i = gui.state_manager.upload_state.instances.clone();
    let bid = gui.state_manager.config.bucket_id.clone();
    let bw = gui.state_manager.config.bandwidth_limit;
    let keystring = format!("{}:{}", gui.state_manager.config.app_key_id, gui.state_manager.config.app_key);
    let tx = gui.state_manager.status_channel_tx.clone();
    std::thread::spawn(move || start_upload_threads(q, i, &bid, bw, keystring, tx));
}

fn start_upload_threads(queue: Arc<Mutex<Vec<PathBuf>>>, instances: Arc<Mutex<Vec<UploadInstance>>>, bucket_id: &str, bw: u32, keystring: String, tx: Sender<String>) {
    println!("Starting upload, getting file info on stored files");

    // Bandwidth per thread
    // 0 = unlimited, otherwise we need at least 1 for each thread
    let bandwidth;
    if bw > 0 {
        bandwidth = ((bw as usize)/8).max(1);
    } else {
        bandwidth = 0;
    }

    // Init backup and authenticate
    let client = reqwest::blocking::Client::builder().timeout(None).build().unwrap();

    let auth = match raze::api::b2_authorize_account(&client,keystring) {
        Ok(a) => a,
        Err(_e) => {
            tx.send("Authentication failed".to_string()).unwrap();
            return;
        },
    };

    // Get all files stored on the server
    // We need this to get the 'last changed' metatdata, which we use to determine
    // if the file has changed and needs to be re-uploaded
    let mut stored_file_list = match raze::util::list_all_files(&client, &auth, &bucket_id, 1000) {
        Ok(f) => Arc::new(f),
        Err(e) => {
            println!("Failed to get remote files - {:?}", e);
            tx.send("Failed talking to B2 - Check your Bucket ID".to_string()).unwrap();
            return
        },
    };
    // Sort so we can binary search later
    Arc::get_mut(&mut stored_file_list).unwrap().sort();
    println!("Got {} files from remote", stored_file_list.len());

    println!("Starting upload threads");
    // TODO(?) don't hardcode thread count as 8
    let pool = Pool::new(8); // Number of upload threads = number of concurrent uploads
    pool.scoped(|scope| {
        // Spawn 1 upload task per worker
        for i in 0..pool.workers() {
            let q = queue.clone();
            let sfl = stored_file_list.clone();
            let client = &client;
            let auth = &auth;
            let instance_handle = instances.clone();
            let instance_num = i;
            scope.execute(move || {
                let upauth = raze::api::b2_get_upload_url(&client, &auth, bucket_id).unwrap();
                loop {
                    // Try and get work, if it fails, sleep and check again
                    let p = {
                        q.lock().unwrap().pop()
                    };
                    let path = match p {
                        Some(p) => p,
                        None => {
                            std::thread::sleep(Duration::from_millis(5000));
                            continue;
                        }
                    };
                    let path_str = path.to_string_lossy().replace("\\", "/");

                    // Under Unix, all paths are naturally prefix with '/' (the root)
                    // B2 will not emulate folders if we start the path with a slash,
                    // so we strip it here to make it behave correctly
                    let name_in_b2 = if cfg!(windows) {
                        &path_str
                    } else {
                        &path_str[1..]
                    };

                    // Construct a StoredFile with the target name so we can binary search for it
                    // If found, check if it has been modified since it was uploaded
                    // If it has: upload it, if it hasn't: skip it
                    // Note that only file_name matters for comparing
                    let sf = raze::api::B2FileInfo {
                        file_name: name_in_b2.to_string(),
                        file_id: None,
                        account_id: auth.account_id.clone(),
                        bucket_id: bucket_id.to_string(),
                        content_length: 0,
                        content_sha1: None,
                        content_type: None,
                        action: "".to_owned(),
                        upload_timestamp: 0
                    };
                    // Compare modified time
                    let do_upload: bool;
                    let metadata = match std::fs::metadata(&path) {
                        Ok(m) => m,
                        Err(e) => {
                            println!("Failed to get metadata, skipping file ({:?})", e);
                            continue;
                        }
                    };
                    let modified_time = match metadata.modified().unwrap().duration_since(std::time::UNIX_EPOCH) {
                        Ok(v) => v.as_secs() * 1000, // Convert seconds to milliseconds
                        Err(_e) => 0u64
                    };
                    let filesize = metadata.len(); // Used later as well

                    match sfl.binary_search(&sf) {
                        Ok(v) => { // A file with the same path+name exists
                            // Check if the local file was modified since it was last uploaded
                            if modified_time > sfl[v].upload_timestamp {
                                do_upload = true;
                            } else {
                                do_upload = false;
                            }
                        },
                        Err(_e) => { // No matching path+name exists
                            do_upload = true;
                        }
                    }
                    if !do_upload {
                        //println!("Skipping {:?}", path_str);
                        continue;
                    }
                    println!("Uploading {:?}", path_str);


                    // Try uploading up to 5 times
                    for attempts in 0..5 {
                        let file = match std::fs::File::open(&path) {
                            Ok(f) => f,
                            Err(e) => {
                                println!("Failed to open file {:?} ({:?}) - It will not be uploaded", path, e);
                                break;
                            }
                        };
                        // Send info back to the UI thread by updating the UploadInstance
                        // Update info, reset counter, get a copy of the tx
                        let tx = {
                            let inst = &mut instance_handle.lock().unwrap()[instance_num];
                            inst.name = path_str.clone();
                            inst.size = filesize;
                            inst.progress = 0;
                            inst.sender.clone()
                        };

                        let params = raze::api::FileParameters {
                            file_path: name_in_b2,
                            file_size: filesize,
                            content_type: None, // auto
                            content_sha1: Sha1Variant::HexAtEnd,
                            last_modified_millis: modified_time
                        };
                        // Note that 'TrackedReader' has to be _after_ 'HashAtEnd' or it would read 40 bytes extra from the hash!
                        // If bandwidth == 0, do not throttle
                        let result = if bandwidth > 0 {
                            let file = raze::util::ReadThrottled::wrap(
                                raze::util::ReadHashAtEnd::wrap(
                                    TrackedReader::wrap(file, tx),
                                ),
                                bandwidth
                            );
                            raze::api::b2_upload_file(&client, &upauth, file, params)
                        } else {
                            let file = raze::util::ReadHashAtEnd::wrap(
                                TrackedReader::wrap(file, tx),
                            );
                            raze::api::b2_upload_file(&client, &upauth, file, params)
                        };

                        match result {
                            Ok(_) => break,
                            Err(e) => {
                                println!("Upload failed: {:?}", e);
                                match e {
                                    raze::Error::ReqwestError(e) => {println!("Reason: {:?}", e);},
                                    raze::Error::IOError(e) => {println!("Reason: {:?}", e);},
                                    raze::Error::SerdeError(e) => {println!("Reason: {:?}", e);},
                                    raze::Error::B2Error(e) => {println!("Reason: {:?}", e);},
                                }

                                if attempts == 4 {
                                    println!("Failed to upload {:?} after 5 attempts", path);
                                } else {
                                    // Sleep and retry
                                    std::thread::sleep(Duration::from_millis(5000));
                                    continue;
                                }
                            }
                        }
                    }
                }
            });

        }
    });
}
