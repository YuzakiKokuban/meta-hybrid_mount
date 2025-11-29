// meta-hybrid_mount/src/modules.rs
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::Serialize;
use crate::{config, defs, utils, state};

#[derive(Serialize)]
struct ModuleInfo {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    mode: String,
}

fn read_prop(path: &Path, key: &str) -> Option<String> {
    if let Ok(file) = fs::File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if line.starts_with(key) && line.chars().nth(key.len()) == Some('=') {
                return Some(line[key.len() + 1..].to_string());
            }
        }
    }
    None
}

pub fn update_description(storage_mode: &str, nuke_active: bool, overlay_count: usize, magic_count: usize) {
    let path = Path::new(defs::MODULE_PROP_FILE);
    if !path.exists() { 
        log::warn!("module.prop not found at {}, skipping description update", path.display());
        return; 
    }

    // Catgirl Style Formatter
    let mode_str = if storage_mode == "tmpfs" { "Tmpfs" } else { "Ext4" };
    let status_emoji = if storage_mode == "tmpfs" { "🐾" } else { "💿" };
    
    let nuke_str = if nuke_active { " | 肉垫: 开启 ✨" } else { "" };
    
    // Construct the cute string
    let new_desc = format!(
        "description=😋 运行中喵～ ({}) {} | Overlay: {} | Magic: {}{}", 
        mode_str, status_emoji, overlay_count, magic_count, nuke_str
    );

    // Read and replace
    let mut new_lines = Vec::new();
    match fs::read_to_string(path) {
        Ok(content) => {
            for line in content.lines() {
                if line.starts_with("description=") {
                    new_lines.push(new_desc.clone());
                } else {
                    new_lines.push(line.to_string());
                }
            }
            // Write back
            if let Err(e) = fs::write(path, new_lines.join("\n")) {
                log::error!("Failed to update module.prop: {}", e);
            } else {
                log::info!("Updated module.prop description (Meow!).");
            }
        },
        Err(e) => log::error!("Failed to read module.prop: {}", e),
    }
}

pub fn scan_enabled_ids(metadata_dir: &Path) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    if !metadata_dir.exists() { return Ok(ids); }
    for entry in fs::read_dir(metadata_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() { continue; }
        let id = entry.file_name().to_string_lossy().to_string();
        if id == "meta-hybrid" || id == "lost+found" { continue; }
        if path.join(defs::DISABLE_FILE_NAME).exists() || path.join(defs::REMOVE_FILE_NAME).exists() || path.join(defs::SKIP_MOUNT_FILE_NAME).exists() { continue; }
        ids.push(id);
    }
    Ok(ids)
}

pub fn sync_active(source_dir: &Path, target_base: &Path) -> Result<()> {
    log::info!("Syncing modules from {} to {}", source_dir.display(), target_base.display());
    let ids = scan_enabled_ids(source_dir)?;
    log::debug!("Found {} enabled modules to sync.", ids.len());
    
    // 1. Prune stale modules from storage (e.g. from modules.img)
    // This fixes the issue where uninstalled/disabled modules persist in ext4 mode
    if target_base.exists() {
        for entry in fs::read_dir(target_base)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() { continue; }
            
            let id = entry.file_name().to_string_lossy().to_string();
            
            // Skip ext4 system folder and self
            if id == "lost+found" || id == "meta-hybrid" { continue; }

            // If module is not in the enabled list, remove it from storage
            if !ids.contains(&id) {
                log::info!("Pruning stale/disabled module from storage: {}", id);
                if let Err(e) = fs::remove_dir_all(&path) {
                    log::warn!("Failed to remove stale module {}: {}", id, e);
                }
            }
        }
    }

    // 2. Sync enabled modules
    for id in ids {
        let src = source_dir.join(&id);
        let dst = target_base.join(&id);
        let has_content = defs::BUILTIN_PARTITIONS.iter().any(|p| src.join(p).exists());
        if has_content {
            log::debug!("Syncing module: {}", id);
            if let Err(e) = utils::sync_dir(&src, &dst) {
                log::error!("Failed to sync module {}: {}", id, e);
            }
        }
    }
    Ok(())
}

pub fn print_list(config: &config::Config) -> Result<()> {
    let module_modes = config::load_module_modes();
    let modules_dir = &config.moduledir;
    let mut modules = Vec::new();

    // Load from state file
    let state = state::RuntimeState::load().unwrap_or_default();
    
    let mut mnt_base = PathBuf::from(defs::FALLBACK_CONTENT_DIR);
    if !state.mount_point.as_os_str().is_empty() {
        mnt_base = state.mount_point;
    }

    if modules_dir.exists() {
        for entry in fs::read_dir(modules_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() { continue; }
            let id = entry.file_name().to_string_lossy().to_string();
            if id == "meta-hybrid" || id == "lost+found" { continue; }
            if path.join(defs::DISABLE_FILE_NAME).exists() || path.join(defs::REMOVE_FILE_NAME).exists() || path.join(defs::SKIP_MOUNT_FILE_NAME).exists() { continue; }

            let has_content = defs::BUILTIN_PARTITIONS.iter().any(|p| {
                path.join(p).exists() || mnt_base.join(&id).join(p).exists()
            });

            if has_content {
                let prop_path = path.join("module.prop");
                let name = read_prop(&prop_path, "name").unwrap_or_else(|| id.clone());
                let version = read_prop(&prop_path, "version").unwrap_or_default();
                let author = read_prop(&prop_path, "author").unwrap_or_default();
                let description = read_prop(&prop_path, "description").unwrap_or_default();
                let mode = module_modes.get(&id).cloned().unwrap_or_else(|| "auto".to_string());
                modules.push(ModuleInfo { id, name, version, author, description, mode });
            }
        }
    }
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    println!("{}", serde_json::to_string(&modules)?);
    Ok(())
}
