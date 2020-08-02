/// This module contains code for the following:
/// (Windows only) Getting list of drives via winapi
/// Managing the file-tree, i.e. state of the file-browser
/// Logic for operating on the file-tree
/// Serialization and deserialization of the file-tree state

use std::path::{Path, PathBuf};

use std::fs;
use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use std::sync::atomic::AtomicBool;

pub mod tracked_reader;

// On Linux, we have the single root '/' instead of drives
// On Windows, there can be any number of drives, so we need to fetch them all
// This code uses winapi to get all the drives
// It also renames them to i.e. C:/ instead of C:\ for consistency
#[cfg(windows)]
pub fn get_roots() -> Result<DirEntry,&'static str> {
    use winapi::um::fileapi::GetLogicalDriveStringsW;
    use std::os::windows::ffi::OsStringExt;

    // Returns paths as e.g. "C:\\\u{0}" ('C' ':' '\' '<null>')
    // This means 4 bytes per drive
    const BUF_SIZE: usize = 64;
    let mut buffer = [0u16; BUF_SIZE];

    // Try to fill the buffer
    let res = unsafe {
        GetLogicalDriveStringsW(BUF_SIZE as u32,  buffer.as_mut_ptr())
    };

    // If we got any drives, process them, otherwise raise an error
    if res > 0 {
        let os_string = OsString::from_wide(&buffer);
        let drive_string = os_string.to_string_lossy();

        // Dummy root element
        // This is the root node of the file tree, which is invisible to the user and holds the drives
        // It can be seen as a list of drives, e.g. [C:/,D:/] used to "emulate" the Linux '/' root
        let root_element = DirEntry {
            kind: EntryKind::Directory,
            name: "".to_string(),
            path: "".to_string(),
            action: Arc::new(Mutex::new(Action::Exclude)),
            children: Arc::new(Mutex::new(vec![])),
            indexed: Arc::new(AtomicBool::new(true)),
            expanded: Arc::new(AtomicBool::new(true))
        };
        // Add found rives to the root element
        for x in drive_string.split('\0').filter(|x| !x.is_empty()) {
            let name = x.replace("\\","/");
            root_element.children.lock().unwrap().push(
                DirEntry {
                    kind: EntryKind::Directory,
                    name: name.to_owned(),
                    path: name.to_owned(),
                    action: Arc::new(Mutex::new(Action::Exclude)),
                    children: Arc::new(Mutex::new(vec![])),
                    indexed: Arc::new(AtomicBool::new(false)),
                    expanded: Arc::new(AtomicBool::new(false)),
                }
            );
        }

        // Write out found drives
        for x in root_element.children.lock().unwrap().iter() {
            println!("Detected Drive: {:?}",x);
        }
        Ok(root_element)
    } else {
        Err("No drives detected!?")
    }
}

// On Linux the root element is just '/'
// TODO Test this code
// TODO What about macOS?
#[cfg(not(windows))]
pub fn get_roots() -> Result<Vec<DirEntry>,&'static str> {
    Ok(vec!(DirEntry {
        kind: EntryKind::Directory,
        name: "/".to_owned(),
        path: "/".to_owned(),
        action: Arc::new(Mutex::new(Action::Nothing)),
        children: Arc::new(Mutex::new(vec![])),
        indexed: Arc::new(Mutex::new(false)),
        expanded: Arc::new(Mutex::new(false)),
    }))
}

/// Type of a 'DirEntry'
/// Each entry in the filesystem either a directory or a file
#[derive(Debug,Clone,PartialEq)]
pub enum EntryKind {
    Directory,
    File
}

impl EntryKind {
    /// Returns 'Directory' if the boolean is true, 'File' otherwise
    fn from_bool(is_dir: bool) -> EntryKind {
        if is_dir {
            EntryKind::Directory
        } else {
            EntryKind::File
        }
    }
}

/// Used to represent what to do with a DirEntry when uploading starts
/// Upload will upload the file or (recursively) the directory
/// Exclude means it does not get synced
/// Exclude has priority over an 'Upload' action inherited from a parent directory
#[derive(Debug,PartialEq,Clone,Copy)]
pub enum Action {
    Upload,
    Exclude,
}

/// Represents an entry in a directory
/// This is either a file or a directory
///
/// Note that the path "" (blank) is RESERVED, as it is used by the root node
/// This means a path on windows is represented (blank)/C:/Users/(...)
/// Calling any functions other than get_files_for_upload() the root node is Undefined Behavior
#[derive(Debug,Clone)]
pub struct DirEntry {
    pub kind: EntryKind,
    pub name: String, // Readable Display Name
    pub path: String, // Full path to this entry
    pub action: Arc<Mutex<Action>>,
    pub children: Arc<Mutex<Vec<DirEntry>>>,
    // Whether or not the entry has had it's children vector populated yet
    pub indexed: Arc<AtomicBool>,
    // Whether or not to show children in the tree
    pub expanded: Arc<AtomicBool>,
}


/// Ordering implementation for a dir entry
/// Directories ALWAYS sort before files
/// Directories are sorted by name
/// Files are left in whatever order the OS gave them, i.e. this implementation does not change it
impl Ord for DirEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.kind, &other.kind) {
            (EntryKind::Directory, EntryKind::File) => Ordering::Less,
            (EntryKind::File, EntryKind::Directory) => Ordering::Greater,
            (EntryKind::Directory, EntryKind::Directory) => self.name.cmp(&other.name),
            // Don't compare files, as it takes excessive amounts of time in dirs with many files
            (EntryKind::File, EntryKind::File) => Ordering::Equal,
        }
    }
}

impl PartialOrd for DirEntry {
    fn partial_cmp(&self, other: &DirEntry) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for DirEntry {

}

impl PartialEq for DirEntry {
    fn eq(&self, other: &DirEntry) -> bool {
        self.kind == other.kind && self.name == other.name
    }
}

impl DirEntry {
    /// Expands this entry's children
    /// This will populate the 'children' vector
    /// Only populates once, use 'refresh_children' to force repopulate
    ///
    /// Silently ignores most errors, as they're almost all permission-related
    /// Symlinks are IGNORED to prevent cycles
    /// Sorts elements, see Ord impl for DirEntry
    pub fn expand(&self) {
        // Only index once, see 'refresh_children'
        if self.indexed.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        let read = fs::read_dir(&self.path);
        if read.is_err() {
            eprintln!("{:?}", read.err().unwrap());
        } else {
            for entry in read.ok().unwrap() {
                if entry.is_err() {
                    println!("IO Error: {:?}", entry.err().unwrap());
                    continue;
                }
                let entry = entry.unwrap();
                // Ignore symlinks to prevent cycles
                if entry.file_type().unwrap().is_symlink() {
                    continue;
                }

                // Check if it is a directory
                // If it is, add a '/' to the end of the display name
                let is_dir = entry.path().is_dir();
                let entry_name;
                let path;
                if is_dir {
                    entry_name = format!("{}/",entry.file_name().to_string_lossy());
                    path = format!("{}{}",self.path,entry_name).replace("//","/");
                } else {
                    entry_name = format!("{}",entry.file_name().to_string_lossy());
                    path = format!("{}/{}",self.path,entry_name).replace("//","/");
                }

                self.children.lock().unwrap().push(
                    DirEntry {
                        kind: EntryKind::from_bool(is_dir),
                        name: entry_name,
                        path,
                        action: Arc::new(Mutex::new(*self.action.lock().unwrap())),
                        children: Arc::new(Mutex::new(vec![])),
                        indexed: Arc::new(AtomicBool::new(false)),
                        expanded: Arc::new(AtomicBool::from(false)),
                    }
                )
            }
        }
        // Sort the elements, see Ord impl for DirEntry for details
        self.children.lock().unwrap().sort();

        self.indexed.swap(true, std::sync::atomic::Ordering::Relaxed);
        self.expanded.swap(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Changes the action of an element
    /// Applies recursively to all children unless they are marked 'Ignore' and the new action is 'Upload'
    pub fn change_action(&self, new_action: Action) {
        *self.action.lock().unwrap() = new_action;
        for child in self.children.lock().unwrap().iter() {
            child.change_action(new_action);
        }
    }

    /// Recursive part of serialize
    /// 'mark' is true if we think the current dir should be uploaded based on the parents
    /// It is flipped to 'false' and the node written if we encounter an 'exclude' while it is true
    pub fn serialize_rec(&self, file: &mut File, mark: bool) {
        let mut mark = mark;
        if *self.action.lock().unwrap() == Action::Upload && !mark {
            file.write_all(format!("UPLOAD {}\n",self.path).as_bytes()).unwrap();
            mark = true;
        } else if *self.action.lock().unwrap() == Action::Exclude && mark {
            file.write_all(format!("EXCLUDE {}\n",self.path).as_bytes()).unwrap();
            mark = false;
        }

        for child in self.children.lock().unwrap().iter() {
            child.serialize_rec(file, mark);
        }
    }


    /// Keeps recursively expanding in an attempt to find the supplied path
    /// If found, it is set to the supplied action
    /// Used by deserialization
    ///
    /// Given a directory, i.e. /some/path/to/a/file it will:
    /// expand '/' and search for 'some'
    /// If 'some' is found, it is expanded and it'll search for 'path'
    /// etc.
    pub fn expand_for_path(&self, path: &str, action: Action) {
        let x = path.find('/');
        let name;
        let remainder;
        match x {
            Some(n)=> {
                name = &path[0..n];
                remainder = &path[n+1..];
            },
            None => {
                name = &path;
                remainder = "";
            },
        };

        for child in self.children.lock().unwrap().iter() {
            // Find name without trailing '/'
            let child_name = child.name.replace("/","");

            if child_name == name {
                // Nothing left: we have a file
                // Only consists of '/', assume it is trailing and change the action
                // Does not end with '/', it's a file
                if remainder.is_empty() || remainder == "/" {
                    child.change_action(action);
                } else {
                    child.expand();
                    child.expand_for_path(remainder, action);
                    return;
                }
            }
        }
    }


    /// Intended to be run on the root element
    /// Runs through the file-tree, appending all FILES marked 'Upload' to a queue
    pub fn get_files_for_upload(&self, queue: &Arc<Mutex<Vec<PathBuf>>>) {
        println!("Building upload file list...");
        use std::time::SystemTime;
        let t = SystemTime::now();
        for child in self.children.lock().unwrap().iter() {
            child.get_files(queue);
        }
        println!("Finished building list in {:?}", t.elapsed().unwrap());
    }

    /// Recursive part of 'get_files_for_upload'
    /// 'self' is always a directory
    fn get_files(&self, queue: &Arc<Mutex<Vec<PathBuf>>>) {
        let mut buffer = vec![]; // Buffer files to add to minimize locking

        // There are 3 cases for each child:
        // 1. File marked upload - add to upload queue
        // 2. Directory that's already indexed - Recursively resolve
        // 3. Directory not indexed but marked upload - Recursively add all sub-elements to queue
        // Note that if a directory is not indexed but the is marked upload, there can be no 'exclude' files in it
        // This is because an 'exclude' file is always indexed automatically on startup or when changed to 'exclude'
        for entry in self.children.lock().unwrap().iter() {
            if entry.kind == EntryKind::File && *entry.action.lock().unwrap() == Action::Upload {
                buffer.push(PathBuf::from(entry.path.clone()));
            } else if entry.kind == EntryKind::Directory {
                if entry.indexed.load(std::sync::atomic::Ordering::Relaxed) {
                    entry.get_files(queue);
                } else if *entry.action.lock().unwrap() == Action::Upload {
                    get_files_all(entry.path.clone(), queue);
                }
            }
        }
        {
            queue.lock().unwrap().append(&mut buffer);
        }
    }
}

/// Alternate recursive part of 'get_files_for_upload'
/// Used on non-indexed directories marked as upload
/// This effectively means all files in all subdirectories should be added to the queue
fn get_files_all<T: AsRef<Path>>(path: T, queue: &Arc<Mutex<Vec<PathBuf>>>) {
    let path = path.as_ref();
    // Attempt to read the current entry
    // This may fail due to any number reasons, typically missing permissions
    let read = fs::read_dir(path);
    if read.is_err() {
        eprintln!("{:?}, {:?}", read.err().unwrap(), path);
    } else {
        let mut buffer = vec![]; // Buffer files to add to minimize locking
        for entry in read.ok().unwrap() {
            // Skip bad entries, typically caused by permissions
            if entry.is_err() {
                println!("IO Error: {:?}", entry.err().unwrap());
                continue;
            }
            // Get the entry and discard symlinks
            // If we don't do this, we could have cyclic directories
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_symlink() {
                continue;
            }
            // For files: add upload queue if we need to
            // For directories: determine if we should check recursively
            let is_dir = entry.path().is_dir();
            if !is_dir {
                buffer.push(entry.path().to_owned());
            } else {
                get_files_all(entry.path(), queue);
            }
        }
        // Append collected files to queue
        {
            queue.lock().unwrap().append(&mut buffer);
        }
    }
}