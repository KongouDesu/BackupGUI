use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use nanoserde::{DeJson, SerJson};

/// This module contains the logic for
/// 1. Keeping track of the UI state, e.g. scrolling
/// 2. Code for rendering the file-tree
/// 3. Logic for handling mouse clicks
/// 4. (De)serialize code

use crate::files::{Action, DirEntry};
use crate::gui::Vertex;
use crate::text::TextHandler;
use std::sync::atomic::AtomicBool;

pub mod filetree;
pub mod mainmenu;
pub mod align;
pub mod upload;
pub mod purge;
pub mod options;
pub mod consent;

/// Keeps track of the UI state
pub struct StateManager {
    // Filesystem roots, i.e. top-most DirEntry
    // On Linux, this is the '/' root
    // On Windows, this is all dummy object 'root element' containing the drives i.e. C:\, D:\, E:\ etc.
    // See the files module for further explanation
    pub fileroot: DirEntry,
    // Config, i.e. font size and other persistent info
    pub config: GUIConfig,
    pub strings: GUIConfigStrings,
    // Text handler to draw text
    pub text_handler: Mutex<TextHandler>,
    // How far down the list we've scrolled
    pub scroll: f32,
    // Which state we're in (and thus, what should be shown/reacted to)
    pub state: UIState,
    pub upload_state: UploadState,

    pub is_purge_done: Arc<AtomicBool>,

    // Cursor x and y
    pub cx: f32,
    pub cy: f32,
}

pub struct UploadState {
    // Whether or not we're currently uploading
    pub running: bool,
    // Whether or not purge is running
    pub purging: bool,
    // Each of the concurrent upload threads
    pub instances: Arc<Mutex<Vec<UploadInstance>>>,
    // Queue of files to be uploaded, shared between threads
    pub queue: Arc<Mutex<Vec<PathBuf>>>,
}

impl Default for UploadState {
    fn default() -> Self {
        let mut instances = Vec::with_capacity(8);
        for _i in 0..8 {
            let (tx, rx) = std::sync::mpsc::channel();
            let instance = UploadInstance {
                name: "Starting...".to_string(),
                size: 0,
                progress: 0,
                sender: tx,
                receiver: rx,
            };
            instances.push(instance);
        }
        UploadState {
            running: false,
            purging: false,
            instances: Arc::new(Mutex::new(instances)),
            queue: Arc::new(Mutex::new(vec![])),
        }
    }
}

// name: filename
// size: total bytes to upload
// progress: how much has been uploaded
// receiver: used to receive progress updates
// sender: sender, cloned to each reader
pub struct UploadInstance {
    pub name: String,
    pub size: u64,
    pub progress: usize,
    pub sender: std::sync::mpsc::Sender<usize>,
    pub receiver: std::sync::mpsc::Receiver<usize>,
}


/// Represents what state the program is in
/// This means what to display and how to react to input
/// Consent: Inform the user about the program, terms, liability, warranty, affiliation etc.
///     This is skipped if consent has been already granted
/// Main: The main menu, when we are not selecting files and not uploading
///     Contains buttons to go to different states + options menu
/// FileTree: File tree browser, for selecting what files to upload/exclude
/// Upload: Displays upload progress + some settings to limit bandwidth usage while uploading
/// Purge: Switched to after upload, gets rid of files in the cloud that are no longer on the drive (B2 hide)
#[allow(dead_code)]
pub enum UIState {
    Consent,
    Main,
    FileTree,
    Upload,
    Options,
    Purge,
}

/// Contains the settings for the UI, i.e. colors, size and other persistent data
#[derive(Debug,DeJson,SerJson)]
pub struct GUIConfig {
    // Size of the font (in pixels)
    // Note that the size of an element is determined by this
    pub font_size: f32,
    // How fast we scroll in the file-tree
    pub scroll_factor: u8,
    // Bucket used for backups
    pub bucket_id: String,
    // Bandwidth limit (bytes/s)
    pub bandwidth_limit: i32,
    // Whether or not to show file paths while uploading
    pub hide_file_names: bool,
    // Whether or not the user has marked that they understand the consequences of using the program
    pub consented: bool,
}

/// Used by the options menu to hold user input
#[derive(Debug)]
pub struct GUIConfigStrings {
    pub active_field: usize,
    pub font_size: String,
    pub scroll_factor: String,
    pub bucket_id: String,
    pub bandwidth_limit: String,
}

impl GUIConfigStrings {
    pub fn from_cfg(cfg: &GUIConfig) -> Self {
        Self {
            active_field: 0,
            font_size: cfg.font_size.to_string(),
            scroll_factor: cfg.scroll_factor.to_string(),
            bucket_id: cfg.bucket_id.to_string(),
            bandwidth_limit: (cfg.bandwidth_limit/1000).to_string(), // Divide by 1000 to get KB/s from B/s
        }
    }

    // Verifies input strings and updates the supplied config
    pub fn destring(&mut self, cfg: &mut GUIConfig) {
        let s = self.font_size.trim();
        let fs = f32::from_str(s);
        if let Ok(n) = fs {cfg.font_size = n.max(4.0).min(1024.0);}
        self.font_size = cfg.font_size.to_string();

        let s = self.scroll_factor.trim();
        let fs = u32::from_str(s);
        if let Ok(n) = fs {cfg.scroll_factor = n.max(1).min(128) as u8;}
        self.scroll_factor = cfg.scroll_factor.to_string();

        let s = self.bucket_id.trim();
        cfg.bucket_id = s.to_string();

        let s = self.bandwidth_limit.trim();
        let fs = i32::from_str(s);
        if let Ok(n) = fs {cfg.bandwidth_limit = n.min(1000000)*1000;} // Multiply by 1000 to get B/s from KB/s

        self.bandwidth_limit = (cfg.bandwidth_limit/1000).to_string();
    }
}


impl GUIConfig {
    /// Instance a UIConfig from the given file, or a default if no such file exists
    pub fn from_file<T: AsRef<str>>(path: T) -> Self {
        let json = match std::fs::read_to_string(path.as_ref()) {
            Ok(s) => s,
            Err(_e) => return Self::default(),
        };
        match DeJson::deserialize_json(&json) {
            Ok(s) => s,
            Err(_e) => Self::default()
        }
    }
}

impl Default for GUIConfig {
    fn default() -> Self {
        GUIConfig {
            font_size: 24.0,
            scroll_factor: 1,
            bucket_id: "".to_string(),
            bandwidth_limit: 0,
            hide_file_names: false,
            consented: false,
        }
    }
}

impl StateManager {
    // Scroll an amount, uses +/- to scroll up/down
    pub fn scroll(&mut self, amount: f32, scale: f32, max: f32) {
        self.scroll = (self.scroll+amount*self.config.font_size*scale).min(0.0).max(-max);
    }

    pub fn cursor_moved(&mut self, x: f32, y: f32) {
        self.cx = x;
        self.cy = y;

    }

    /// Write the current config to a file
    ///
    /// The output file is a minimal list of directories and their rules
    /// The idea is that there's rules like 'dir1/dir2/dir3 UPLOAD'
    /// When loading, first dir1 is expanded. If we find dir2 we expand that, and so on
    ///
    /// This runs through the current tree, depth-first
    /// If a node is marked 'upload' we add that to the output list
    /// Every node in the children will not be added, unless they are marked 'exclude'
    /// This rule is applied i.e. you can do
    ///  UPLOAD  root/
    /// EXCLUDE     dir1/
    /// INHERIT          dir2/
    ///  UPLOAD          dir3/
    /// INHERIT              dir4/
    /// INHERIT      dir5/
    /// Here, dir3, dir4 and dir5 will be uploaded
    /// Note dir3+dir4 work despite the parent being 'exclude'
    pub fn serialize<T: AsRef<Path>>(&self, file: T) {
        let path = file.as_ref();
        let mut file = std::fs::File::create(path).unwrap();

        for child in self.fileroot.children.lock().unwrap().iter() {
            child.serialize_rec(&mut file, false);
        }
    }

    /// Load the list of files to backup from a file
    /// Counterpart to serialize
    pub fn deserialize<T: AsRef<Path>>(&mut self, file: T) {
        let path = file.as_ref();
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            if line.is_err() {
                println!("Malformed entry - {}", line.err().unwrap());
                continue;
            }
            let line = line.unwrap();
            if line.starts_with("UPLOAD ") {
                // offset 7 for "UPLOAD " (note the space)
                self.fileroot.expand_for_path(&line[7..], Action::Upload);
            } else if line.starts_with("EXCLUDE ") {
                // offset 8 for "EXCLUDE " (note the space)
                self.fileroot.expand_for_path(&line[8..], Action::Exclude);
            } else {
                println!("Malformed entry - {}", line);
            }
        }
    }
}