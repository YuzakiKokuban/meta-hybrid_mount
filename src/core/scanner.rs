use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::defs::{DISABLE_FILE_NAME, REMOVE_FILE_NAME, SKIP_MOUNT_FILE_NAME, STATE_FILE};

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Clone, PartialOrd)]
pub struct ModuleInfo {
    pub magic_mount: bool,
    pub overlayfs: bool,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct ModuleConfig {
    modules: HashMap<String, ModuleInfo>,
}

pub struct ModuleScanner {
    extra: Vec<String>,
    path: PathBuf,
}

impl ModuleScanner {
    pub fn new<P>(path: P, extra: Vec<String>) -> Self
    where
        P: AsRef<Path>,
    {
        Self {
            extra: extra,
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn scanner(&mut self) -> Result<HashMap<String, ModuleInfo>> {
        log::info!("Starting san modules!");
        let mut modules = HashMap::new();
        let file = fs::read_to_string(STATE_FILE)?;
        let json_raw: ModuleConfig = serde_json::from_str(&file)?;

        if let Ok(entries) = self.path.read_dir() {
            for p in entries.flatten() {
                let path = p.path();
                if !path.is_dir() {
                    continue;
                }

                let mut modified = false;
                let mut partitions = HashSet::new();
                partitions.insert("system".to_string());
                partitions.extend(self.extra.clone());

                for p in &partitions {
                    if path.join(p).is_dir() {
                        modified = true;
                        break;
                    }
                }

                if !modified {
                    continue;
                }

                let disabled =
                    path.join(DISABLE_FILE_NAME).exists() || path.join(REMOVE_FILE_NAME).exists();
                let skip = path.join(SKIP_MOUNT_FILE_NAME).exists();
                if disabled || skip {
                    continue;
                }

                let id = p.file_name();
                let id = id.to_str().unwrap_or("unknown");
                if !json_raw.modules.contains_key(id) {
                    continue;
                }

                modules.insert(
                    id.to_string(),
                    ModuleInfo {
                        magic_mount: false,
                        overlayfs: false,
                    },
                );
            }
        }

        Ok(modules)
    }
}
