/// This module contains the logic for
/// 1. Keeping track of the UI state, e.g. scrolling
/// 2. Code for rendering the file-tree
/// 3. Logic for handling mouse clicks
/// 4. (De)serialize code

use crate::files::{DirEntry, Action};
use crate::text::TextHandler;

use std::path::Path;
use std::io::BufRead;
use std::sync::Mutex;

/// Keeps track of the UI state
pub struct UIManager  {
    // Filesystem roots, i.e. top-most DirEntry
    // On Linux, this is the '/' root
    // On Windows, this is all dummy object 'root element' containing the drives i.e. C:\, D:\, E:\ etc.
    // See the files module for further explanation
    pub fileroot: DirEntry,
    // Config, i.e. font size and other persistent info
    pub config: UIConfig,
    // Text handler to draw text
    pub text_handler: Mutex<TextHandler>,
    // How far down the list we've scrolled
    pub scroll: f32,

    // Cursor x and y
    pub cx: f32,
    pub cy: f32,
}

/// Contains the settings for the UI, i.e. colors, size and other persistent data
/// TODO Make serializable so we can read/write to/from a file
pub struct UIConfig {
    // Height and width of file tree, in pixels
    // Note that this is the entire space available for the tree
    pub tree_width: f32,
    pub tree_height: f32,

    // Size of the font (in pixels)
    // Note that the size of an element is determined from this
    pub font_size: f32,
}

impl UIManager  {
    // Renders the file tree
    // The top-left corner is at (0,0) (in screen coordinates)
    // Translate the viewport before if it needs to be elsewhere
    pub fn render_file_tree(&self, device: &wgpu::Device, mut encoder: &mut wgpu::CommandEncoder, frame: &wgpu::SwapChainOutput, size: (u32,u32)) {
        let mut y = self.scroll;
        let mut indent = 0f32;

        // Render background, note that font_size determines height
        for entry in self.fileroot.children.lock().unwrap().iter() {
            y = self.render_subtree(entry, y, indent);
        }

        // TODO Flush before drawing text

        // Render text, note that font_size determines height
        let mut y = self.scroll;
        for entry in self.fileroot.children.lock().unwrap().iter() {
            y = self.render_subtree_text(entry, y, indent);
        }

        self.text_handler.lock().unwrap().flush(&device,&mut encoder, frame, size)
    }

    fn render_subtree(&self, root: &DirEntry, mut y: f32, mut indent: f32) -> f32 {
        // Render self, though only if within visible area
        if y >= -self.config.font_size && y <= self.config.tree_height {
            if *root.action.lock().unwrap() == Action::Exclude {
                // TODO: Draw red rectangle (indent,y,1024.0-indent, self.config.font_size) (x,y,w,h)
            } else if *root.action.lock().unwrap() == Action::Upload {
                // TODO: Draw green rectangle (indent,y,1024.0-indent, self.config.font_size) (x,y,w,h)
            }
        } else if y > self.config.tree_height {
            // We will never return to the visible area, stop drawing
            return y;
        }

        // Note: step size determined by font_size
        y += self.config.font_size;

        // Render children
        if *root.expanded.lock().unwrap() {
            indent += 24.0f32;
            for entry in root.children.lock().unwrap().iter() {
                y = self.render_subtree(entry, y, indent);
            }
        }
        y
    }

    fn render_subtree_text(&self, root: &DirEntry, mut y: f32, mut indent: f32) -> f32 {
        // Draw self if within visible area
        if y >= -self.config.font_size && y <= self.config.tree_height {
            // TODO Draw text (&root.name, indent+2.0, y+self.config.font_size/2.0, self.config.font_size) (text,x,y,size)
            self.text_handler.lock().unwrap().draw(&root.name, indent+2.0, y, self.config.font_size, [1.0,1.0,1.0,1.0]);
        } else if y > self.config.tree_height {
            // We will never return to the visible area, stop drawing
            return y;
        }

        // Note: step size determined by font_size
        y += self.config.font_size;

        // Render children
        if *root.expanded.lock().unwrap() {
            indent += 24.0f32;
            for entry in root.children.lock().unwrap().iter() {
                y = self.render_subtree_text(entry, y, indent);
            }
        }
        y
    }

    // Scroll an amount, uses +/- to scroll up/down
    pub fn scroll(&mut self, amount: f32) {
        self.scroll = (self.scroll+amount).min(0.0);
    }

    // Handle a mouse click, given it's location relative to the top-left corner of the tree
    // Positive x is right, positive y is down
    pub fn on_click(&self, button: u8) {
        // Offset 'y' based on scroll
        let mut y = self.cy-self.scroll;
        println!("Start search {}, button {}", y, button);
        let mut indent = 0f32;
        // Render background
        let mut done;
        for entry in self.fileroot.children.lock().unwrap().iter() {
            let temp = self.handle_click(entry, self.cx, y, button);
            y = temp.0;
            done = temp.1;
            if done {
                return
            }
        }
    }

    pub fn cursor_moved(&mut self, x: f32, y: f32) {
        self.cx = x;
        self.cy = y;
    }

    // Recursive part of click handling
    // Each (visible) entry decrement 'y' by font_size (it's height)
    // Once 'y' is <= font_size, it means we found our entry
    fn handle_click(&self, entry: &DirEntry, x: f32, mut y: f32, button: u8) -> (f32, bool) {
        // Check if we found our entry, if we did, handle the click and stop
        if y <= self.config.font_size {
            println!("Click {:?}, button {:?}", entry.name, button);
            if button == 1 {
                // Toggle visibility
                if *entry.expanded.lock().unwrap() {
                    *entry.expanded.lock().unwrap() = false;
                } else {
                    // This refreshes the dir and expands it
                    if !*entry.indexed.lock().unwrap() {
                        entry.expand();
                    }
                    *entry.expanded.lock().unwrap() = true;
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
        y -= self.config.font_size;

        // Notice: Only search expanded (visible) entries, as we cant click invisible ones
        if *entry.expanded.lock().unwrap() {
            let mut done;
            for entry in entry.children.lock().unwrap().iter() {
                let temp = self.handle_click(entry, x, y, button);
                y = temp.0;
                done = temp.1;
                if done {
                    return (y, true);
                }
            }
        }
        (y,false)
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
        let mut file = std::fs::File::open(path).unwrap();
        let mut reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            if line.is_err() {
                println!("Malformed entry - {}", line.err().unwrap());
                continue;
            }
            let line = line.unwrap();
            if line.starts_with("UPLOAD ") {
                // offset 7 for "UPLOAD " (note the space)
                println!("Trying to expand (upload) - {}" , &line[7..]);
                self.fileroot.expand_for_path(&line[7..], Action::Upload);
            } else if line.starts_with("EXCLUDE ") {
                // offset 8 for "EXCLUDE " (note the space)
                println!("Trying to expand (exclude) - {}" , &line[8..]);
                self.fileroot.expand_for_path(&line[8..], Action::Exclude);
            } else {
                println!("Malformed entry - {}", line);
            }
        }
    }
}