use std::{collections::HashSet, fs, path::Path};

use anyhow::Result;

use crate::{
    defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME},
    framework::node::Node,
    utils::validate_module_id,
};

pub fn collect_module_files(
    module_dir: &Path,
    extra_partitions: &[String],
) -> Result<Option<Node>> {
    let mut root = Node::new_root("");
    let mut system = Node::new_root("system");
    let module_root = module_dir;
    let mut has_file = HashSet::new();

    log::debug!("begin collect module files: {}", module_root.display());

    for entry in module_root.read_dir()?.flatten() {
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let id = entry.file_name().to_str().unwrap().to_string();
        log::debug!("processing new module: {id}");

        let prop = entry.path().join("module.prop");
        if !prop.exists() {
            log::debug!("skipped module {id}, because not found module.prop");
            continue;
        }
        let string = fs::read_to_string(prop)?;
        for line in string.lines() {
            if line.starts_with("id")
                && let Some((_, value)) = line.split_once('=')
            {
                validate_module_id(value)?;
            }
        }

        if entry.path().join(DISABLE_FILE_NAME).exists()
            || entry.path().join(REMOVE_FILE_NAME).exists()
            || entry.path().join(SKIP_MOUNT_FILE_NAME).exists()
        {
            log::debug!("skipped module {id}, due to disable/remove/skip_mount");
            continue;
        }

        let mut modified = false;
        let mut partitions = HashSet::new();
        partitions.insert("system".to_string());
        partitions.extend(extra_partitions.iter().cloned());

        for p in &partitions {
            if entry.path().join(p).is_dir() {
                modified = true;
                break;
            }
            log::debug!("{id} due not modify {p}");
        }

        if !modified {
            continue;
        }

        log::debug!("collecting {}", entry.path().display());

        for p in partitions {
            if !entry.path().join(&p).exists() {
                continue;
            }

            has_file.insert(system.collect_module_files(entry.path().join(&p))?);
        }
    }

    if has_file.contains(&true) {
        const BUILTIN_PARTITIONS: [(&str, bool); 4] = [
            ("vendor", true),
            ("system_ext", true),
            ("product", true),
            ("odm", false),
        ];

        for (partition, require_symlink) in BUILTIN_PARTITIONS {
            let path_of_root = Path::new("/").join(partition);
            let path_of_system = Path::new("/system").join(partition);
            if path_of_root.is_dir() && (!require_symlink || path_of_system.is_symlink()) {
                let name = partition.to_string();
                if let Some(node) = system.children.remove(&name) {
                    root.children.insert(name, node);
                }
            }
        }

        for partition in extra_partitions {
            if BUILTIN_PARTITIONS.iter().any(|(p, _)| p == partition) {
                continue;
            }
            if partition == "system" {
                continue;
            }

            let path_of_root = Path::new("/").join(partition);
            let path_of_system = Path::new("/system").join(partition);
            let require_symlink = false;

            if path_of_root.is_dir() && (!require_symlink || path_of_system.is_symlink()) {
                let name = partition.clone();
                if let Some(node) = system.children.remove(&name) {
                    log::debug!("attach extra partition '{name}' to root");
                    root.children.insert(name, node);
                }
            }
        }

        root.children.insert("system".to_string(), system);
        Ok(Some(root))
    } else {
        Ok(None)
    }
}
