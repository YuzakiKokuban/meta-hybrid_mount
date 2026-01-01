use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::defs::CONFIG;

#[derive(Serialize, Deserialize)]
pub struct ConfigData {
    #[serde(default = "default_moduledir")]
    pub moduledir: PathBuf,
    #[serde(default = "default_mountsource")]
    pub mountsource: String,
    pub verbose: bool,
    pub partitions: Vec<String>,
    pub tmpfsdir: Option<String>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub umount: bool,
}

fn default_moduledir() -> PathBuf {
    PathBuf::from("/data/adb/modules/")
}

fn default_mountsource() -> String {
    String::from("KSU")
}

impl ConfigData {
    pub fn load() -> Result<Self> {
        let content = fs::read_to_string(CONFIG).context("failed to read config file")?;

        let config: Self = toml::from_str(&content).context("failed to parse config file")?;

        Ok(config)
    }
}
