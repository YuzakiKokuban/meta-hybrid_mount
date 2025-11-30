// src/core/inventory.rs
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::{defs, conf::config};

#[derive(Debug, Clone)]
pub struct Module {
    pub id: String,
    pub source_path: PathBuf,
    pub mode: String, // "auto", "magic", etc.
}

/// Scans the source directory for enabled modules.
/// Does not access the runtime storage.
pub fn scan(source_dir: &Path, config: &config::Config) -> Result<Vec<Module>> {
    let mut modules = Vec::new();
    if !source_dir.exists() {
        return Ok(modules);
    }

    let module_modes = config::load_module_modes();

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if !path.is_dir() { continue; }
        
        let id = entry.file_name().to_string_lossy().to_string();
        
        // Skip internal or system directories
        if id == "meta-hybrid" || id == "lost+found" || id == ".git" { continue; }

        // Check for disable flags
        if path.join(defs::DISABLE_FILE_NAME).exists() || 
           path.join(defs::REMOVE_FILE_NAME).exists() || 
           path.join(defs::SKIP_MOUNT_FILE_NAME).exists() { 
            continue; 
        }

        let mode = module_modes.get(&id).cloned().unwrap_or_else(|| "auto".to_string());

        modules.push(Module {
            id,
            source_path: path,
            mode,
        });
    }
    modules.sort_by(|a, b| b.id.cmp(&a.id));

    Ok(modules)
}