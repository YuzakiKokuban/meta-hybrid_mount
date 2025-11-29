// meta-hybrid_mount/src/config.rs
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const CONFIG_FILE_DEFAULT: &str = "/data/adb/meta-hybrid/config.toml";
pub const MODULE_MODE_FILE: &str = "/data/adb/meta-hybrid/module_mode.conf";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_moduledir")]
    pub moduledir: PathBuf,
    pub tempdir: Option<PathBuf>,
    #[serde(default = "default_mountsource")]
    pub mountsource: String,
    pub verbose: bool,
    #[serde(default, deserialize_with = "deserialize_partitions_flexible")]
    pub partitions: Vec<String>,
    
    // Stealth Configuration
    // If true, forces the use of ext4 loop image even if tmpfs supports xattr.
    #[serde(default)]
    pub force_ext4: bool,

    // If true, attempts to load the Nuke LKM to hide ext4 traces.
    #[serde(default)]
    pub enable_nuke: bool,

    // If true, skips the try_umount logic (namespace detaching).
    // Useful for debugging or if the kernel doesn't support it well.
    #[serde(default)]
    pub disable_umount: bool,
}

fn default_moduledir() -> PathBuf {
    PathBuf::from("/data/adb/modules/")
}

fn default_mountsource() -> String {
    String::from("HybridMount")
}

fn deserialize_partitions_flexible<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::Vec(v) => Ok(v),
        StringOrVec::String(s) => Ok(s
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()),
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            moduledir: default_moduledir(),
            tempdir: None,
            mountsource: default_mountsource(),
            verbose: false,
            partitions: Vec::new(),
            force_ext4: false,
            enable_nuke: false,
            disable_umount: false,
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref()).context("failed to read config file")?;
        let config: Config = toml::from_str(&content).context("failed to parse config file")?;
        Ok(config)
    }

    pub fn load_default() -> Result<Self> {
        Self::from_file(CONFIG_FILE_DEFAULT)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }
        fs::write(path.as_ref(), content).context("failed to write config file")?;
        Ok(())
    }

    pub fn merge_with_cli(
        &mut self,
        moduledir: Option<PathBuf>,
        tempdir: Option<PathBuf>,
        mountsource: Option<String>,
        verbose: bool,
        partitions: Vec<String>,
    ) {
        if let Some(dir) = moduledir { self.moduledir = dir; }
        if tempdir.is_some() { self.tempdir = tempdir; }
        if let Some(source) = mountsource { self.mountsource = source; }
        if verbose { self.verbose = true; }
        if !partitions.is_empty() { self.partitions = partitions; }
    }
}

pub fn load_module_modes() -> HashMap<String, String> {
    let mut modes = HashMap::new();
    if let Ok(content) = fs::read_to_string(MODULE_MODE_FILE) {
        for line in content.lines() {
            if line.trim().starts_with('#') { continue; }
            if let Some((id, mode)) = line.split_once('=') {
                modes.insert(id.trim().to_string(), mode.trim().to_lowercase());
            }
        }
    }
    modes
}
