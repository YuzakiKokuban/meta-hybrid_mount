use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rustix::fs::statvfs;
use serde::Serialize;

use crate::defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME};

#[derive(Debug, Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub disabled: bool,
    pub skip: bool,
}

#[derive(Debug, Serialize)]
pub struct StorageInfo {
    pub size: String,
    pub used: String,
    pub percent: String,
}

fn read_prop<P: AsRef<Path>>(path: P, key: &str) -> Option<String> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().flatten() {
        if line.starts_with(key) {
            if let Some((_, value)) = line.split_once('=') {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

pub fn scan_modules(module_dir: &PathBuf) -> Result<Vec<ModuleInfo>> {
    let mut modules = Vec::new();

    if let Ok(entries) = module_dir.read_dir() {
        for entry in entries.flatten() {
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            if !path.join("module.prop").exists() {
                continue;
            }

            // Filter logic matching magic_mount/mod.rs
            if !path.join("system").is_dir() {
                continue;
            }

            let disabled = path.join(DISABLE_FILE_NAME).exists() 
                || path.join(REMOVE_FILE_NAME).exists();
            let skip = path.join(SKIP_MOUNT_FILE_NAME).exists();

            if disabled || skip {
                continue;
            }

            let id = entry.file_name().to_string_lossy().to_string();
            let prop_path = path.join("module.prop");

            let name = read_prop(&prop_path, "name").unwrap_or_else(|| id.clone());
            let version = read_prop(&prop_path, "version").unwrap_or_default();
            let description = read_prop(&prop_path, "description").unwrap_or_default();

            modules.push(ModuleInfo {
                id,
                name,
                version,
                description,
                disabled,
                skip,
            });
        }
    }
    modules.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(modules)
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

pub fn get_storage_usage(path: &Path) -> Result<StorageInfo> {
    let stat = statvfs(path).with_context(|| format!("failed to statvfs {}", path.display()))?;

    let total_bytes = stat.f_blocks * stat.f_frsize;
    let avail_bytes = stat.f_bavail * stat.f_frsize;
    let used_bytes = total_bytes.saturating_sub(avail_bytes);

    let percent = if total_bytes > 0 {
        (used_bytes as f64 / total_bytes as f64) * 100.0
    } else {
        0.0
    };

    Ok(StorageInfo {
        size: format_size(total_bytes),
        used: format_size(used_bytes),
        percent: format!("{:.0}%", percent),
    })
}
