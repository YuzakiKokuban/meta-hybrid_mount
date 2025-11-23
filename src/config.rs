use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const CONFIG_FILE_DEFAULT: &str = "/data/adb/magic_mount/config.toml";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_moduledir")]
    pub moduledir: PathBuf,

    #[serde(default)]
    pub tempdir: Option<PathBuf>,

    #[serde(default = "default_mountsource")]
    pub mountsource: String,

    #[serde(default)]
    pub verbose: bool,

    #[serde(default)]
    pub partitions: Vec<String>,
}

fn default_moduledir() -> PathBuf {
    PathBuf::from("/data/adb/modules/")
}

fn default_mountsource() -> String {
    String::from("MaGIcMounT")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            moduledir: default_moduledir(),
            tempdir: None,
            mountsource: default_mountsource(),
            verbose: false,
            partitions: Vec::new(),
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref()).context("failed to read config file")?;

        let config: Config = toml::from_str(&content).context("failed to parse config file")?;

        Ok(config)
    }

    pub fn load_default() -> Option<Self> {
        Self::from_file(CONFIG_FILE_DEFAULT).ok()
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self).context("failed to serialize config")?;

        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }

        fs::write(path.as_ref(), content).context("failed to write config file")?;

        Ok(())
    }

    pub fn example() -> String {
        let example = Self::default();
        toml::to_string_pretty(&example).unwrap_or_default()
    }

    pub fn merge_with_cli(
        &mut self,
        moduledir: Option<PathBuf>,
        tempdir: Option<PathBuf>,
        mountsource: Option<String>,
        verbose: bool,
        partitions: Vec<String>,
    ) {
        if let Some(dir) = moduledir {
            self.moduledir = dir;
        }
        if tempdir.is_some() {
            self.tempdir = tempdir;
        }
        if let Some(source) = mountsource {
            self.mountsource = source;
        }
        if verbose {
            self.verbose = true;
        }
        if !partitions.is_empty() {
            self.partitions = partitions;
        }
    }
}
