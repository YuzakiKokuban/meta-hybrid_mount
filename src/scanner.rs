use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME};

#[derive(Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub disabled: bool,
    pub skip: bool,
}

fn read_prop<P>(path: P, key: &str) -> Option<String>
where
    P: AsRef<Path>,
{
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        if line.starts_with(key)
            && let Some((_, value)) = line.split_once('=')
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

pub fn scan_modules<P>(module_dir: P) -> Vec<ModuleInfo>
where
    P: AsRef<Path>,
{
    let mut modules = Vec::new();

    if let Ok(entries) = module_dir.as_ref().read_dir() {
        for entry in entries.flatten() {
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            if !path.join("module.prop").exists() {
                continue;
            }

            let id = entry.file_name().to_string_lossy().to_string();
            let prop_path = path.join("module.prop");

            let name = read_prop(&prop_path, "name").unwrap_or_else(|| id.clone());
            let version = read_prop(&prop_path, "version").unwrap_or_default();
            let description = read_prop(&prop_path, "description").unwrap_or_default();

            let disabled =
                path.join(DISABLE_FILE_NAME).exists() || path.join(REMOVE_FILE_NAME).exists();
            let skip = path.join(SKIP_MOUNT_FILE_NAME).exists();

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

    modules
}
