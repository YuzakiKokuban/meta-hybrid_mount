use std::{
    collections::{HashMap, hash_map::Entry},
    fmt,
    fs::{DirEntry, FileType},
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use anyhow::Result;
use extattr::lgetxattr;
use rustix::path::Arg;

use crate::defs::{MAGIC_MOUNT, OVERLAYFS, REPLACE_DIR_FILE_NAME, REPLACE_DIR_XATTR};

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum NodeFileType {
    RegularFile,
    Directory,
    Symlink,
    Whiteout,
}

impl From<FileType> for NodeFileType {
    fn from(value: FileType) -> Self {
        if value.is_file() {
            Self::RegularFile
        } else if value.is_dir() {
            Self::Directory
        } else if value.is_symlink() {
            Self::Symlink
        } else {
            Self::Whiteout
        }
    }
}

#[derive(Clone)]
pub struct Node {
    pub name: String,
    pub file_type: NodeFileType,
    pub children: HashMap<String, Self>,
    // the module that owned this node
    pub module_path: Option<PathBuf>,
    pub replace: bool,
    pub skip: bool,
    pub overlayfs: bool,
    pub magic_mount: bool,
}

impl fmt::Display for NodeFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory => write!(f, "DIR"),
            Self::RegularFile => write!(f, "FILE"),
            Self::Symlink => write!(f, "LINK"),
            Self::Whiteout => write!(f, "WHT"),
        }
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print_tree(
            node: &Node,
            f: &mut fmt::Formatter<'_>,
            prefix: &str,
            is_last: bool,
            is_root: bool,
        ) -> fmt::Result {
            let connector = if is_root {
                ""
            } else if is_last {
                "└── "
            } else {
                "├── "
            };

            let name = if node.name.is_empty() {
                "/"
            } else {
                &node.name
            };

            let mut flags = Vec::new();

            if node.replace {
                flags.push("REPLACE");
            }

            if node.skip {
                flags.push("SKIP");
            }

            if node.magic_mount {
                flags.push("MAGIC_MOUNT");
            }

            if node.overlayfs {
                flags.push("OVERLAYFS");
            }

            let flag_str = if flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", flags.join("|"))
            };

            let source_str = if let Some(p) = &node.module_path {
                format!(" -> {}", p.display())
            } else {
                String::new()
            };

            writeln!(
                f,
                "{}{}{} [{}]{}{}",
                prefix, connector, name, node.file_type, flag_str, source_str
            )?;

            let child_prefix = if is_root {
                ""
            } else if is_last {
                "    "
            } else {
                "│   "
            };

            let new_prefix = format!("{}{}", prefix, child_prefix);

            let mut children: Vec<_> = node.children.values().collect();

            children.sort_by(|a, b| a.name.cmp(&b.name));

            for (i, child) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;

                print_tree(child, f, &new_prefix, is_last_child, false)?;
            }

            Ok(())
        }

        print_tree(self, f, "", true, true)
    }
}

impl Node {
    pub fn collect_module_files<P>(&mut self, module_dir: P) -> Result<bool>
    where
        P: AsRef<Path>,
    {
        let dir = module_dir.as_ref();
        let mut has_file = false;
        for entry in dir.read_dir()?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            let node = match self.children.entry(name.clone()) {
                Entry::Occupied(o) => Some(o.into_mut()),
                Entry::Vacant(v) => Self::new_module(&name, &entry).map(|it| v.insert(it)),
            };

            if let Some(node) = node {
                has_file |= if node.file_type == NodeFileType::Directory {
                    node.collect_module_files(dir.join(&node.name))? || node.replace
                } else {
                    true
                }
            }
        }

        Ok(has_file)
    }

    fn dir_is_replace<P>(path: P) -> bool
    where
        P: AsRef<Path>,
    {
        if let Ok(v) = lgetxattr(&path, REPLACE_DIR_XATTR)
            && String::from_utf8_lossy(&v) == "y"
        {
            return true;
        }

        path.as_ref().join(REPLACE_DIR_FILE_NAME).exists()
    }

    pub fn new_root<S>(name: S) -> Self
    where
        S: AsRef<str> + Into<String>,
    {
        Self {
            name: name.into(),
            file_type: NodeFileType::Directory,
            children: HashMap::default(),
            module_path: None,
            replace: false,
            skip: false,
            overlayfs: false,
            magic_mount: false,
        }
    }

    pub fn new_module<S>(name: &S, entry: &DirEntry) -> Option<Self>
    where
        S: ToString,
    {
        if let Ok(metadata) = entry.metadata() {
            let path = entry.path();
            let file_type = if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
                Some(NodeFileType::Whiteout)
            } else {
                Some(NodeFileType::from(metadata.file_type()))
            };
            if let Some(file_type) = file_type {
                let replace = file_type == NodeFileType::Directory && Self::dir_is_replace(&path);
                if replace {
                    log::debug!("{} need replace", path.display());
                }

                return Some(Self {
                    name: name.to_string(),
                    file_type,
                    children: HashMap::default(),
                    module_path: Some(path.clone()),
                    replace,
                    skip: false,
                    overlayfs: path.join(OVERLAYFS).exists(),
                    magic_mount: path.join(MAGIC_MOUNT).exists(),
                });
            }
        }

        None
    }
}
