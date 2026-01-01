use std::collections::HashSet;

use anyhow::Result;

use crate::framework::config::ConfigData;

pub mod config;
mod node;
mod utils;

pub fn executer(config: ConfigData) -> Result<()> {
    let mut magic_mount = HashSet::new();
    if let Some(files) = utils::collect_module_files(&config.moduledir, &config.partitions)? {
        if files.magic_mount {
            magic_mount.insert(&files);
        }
    }

    Ok(())
}
