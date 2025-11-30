// src/core/sync.rs
use std::fs;
use std::path::Path;
use anyhow::Result;
use crate::{defs, utils, core::inventory::Module};

/// Synchronizes enabled modules from source to the prepared storage.
/// Also handles SELinux context repair.
pub fn perform_sync(modules: &[Module], target_base: &Path) -> Result<()> {
    log::info!("Starting module sync to {}", target_base.display());

    // 1. Wipe storage (Clean slate)
    wipe_storage(target_base)?;

    // 2. Sync each module
    for module in modules {
        let dst = target_base.join(&module.id);
        
        // Recursively check if the module has actual content for known partitions
        let has_content = defs::BUILTIN_PARTITIONS.iter().any(|p| {
            let part_path = module.source_path.join(p);
            part_path.exists() && has_files_recursive(&part_path)
        });

        if has_content {
            log::debug!("Syncing module: {}", module.id);
            if let Err(e) = utils::sync_dir(&module.source_path, &dst) {
                log::error!("Failed to sync module {}: {}", module.id, e);
            } else {
                // 3. Context Mirroring
                repair_module_contexts(&dst, &module.id);
            }
        } else {
            log::debug!("Skipping empty module: {}", module.id);
        }
    }
    
    Ok(())
}

fn wipe_storage(target: &Path) -> Result<()> {
    if !target.exists() { return Ok(()); }
    
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name == "lost+found" || name == "meta-hybrid" { continue; }

        if path.is_dir() {
            if let Err(e) = fs::remove_dir_all(&path) {
                log::warn!("Failed to delete dir {}: {}", name, e);
            }
        } else {
            if let Err(e) = fs::remove_file(&path) {
                log::warn!("Failed to delete file {}: {}", name, e);
            }
        }
    }
    Ok(())
}

fn repair_module_contexts(module_root: &Path, module_id: &str) {
    for part in defs::BUILTIN_PARTITIONS {
        let part_root = module_root.join(part);
        if part_root.exists() {
            if let Err(e) = recursive_context_repair(module_root, &part_root) {
                log::warn!("Context repair failed for {}/{}: {}", module_id, part, e);
            }
        }
    }
}

fn recursive_context_repair(base: &Path, current: &Path) -> Result<()> {
    if !current.exists() { return Ok(()); }
    
    // Calculate path relative to module root to find system equivalent
    // e.g. /mnt/modA/system/bin/app -> /system/bin/app
    let relative = current.strip_prefix(base)?;
    let system_path = Path::new("/").join(relative);

    if system_path.exists() {
        // Copy context from real system file
        let _ = utils::copy_path_context(&system_path, current);
    }

    if current.is_dir() {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            recursive_context_repair(base, &entry.path())?;
        }
    }
    Ok(())
}

fn has_files_recursive(path: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    if has_files_recursive(&entry.path()) { return true; }
                } else {
                    return true; // Found a file/symlink/device
                }
            }
        }
    }
    false
}