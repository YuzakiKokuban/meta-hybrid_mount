use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{LazyLock, Mutex, RwLock},
    thread,
};

use anyhow::Result;
use rustix::mount::{MountFlags, mount as rustix_mount};

use crate::{
    conf::config::Config,
    mount::{magic_mount::magic_mount, overlay::mount_partition},
};

mod scanner;

pub static MAGIC_MOUNT_ID: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
pub static OVERLAYFS_ID: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
pub static PARTITIONS: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| RwLock::new(Vec::new()));
pub static CONFIG: LazyLock<RwLock<Config>> = LazyLock::new(|| RwLock::new(Config::default()));

pub fn mount(config: Config) -> Result<()> {
    let files =
        scanner::ModuleScanner::new(&config.moduledir, config.partitions.clone()).scanner()?;

    if files.is_empty() {
        log::info!("no modules need mount!!");
        return Ok(());
    }

    CONFIG.set(config)?;

    for (id, info) in files {
        if info.magic_mount {
            log::debug!("module {} will use magic_mount to mount", id.clone());
            MAGIC_MOUNT_ID.lock().unwrap().insert(id.clone());
        }

        if info.overlayfs {
            log::debug!("module {} will use overlayfs to mount", id.clone());
            OVERLAYFS_ID.lock().unwrap().insert(id.clone());
        }
    }

    let overlayfs = thread::Builder::new()
        .name("Moount-Overlayfs".to_string())
        .spawn(move || {
            let mut system_lowerdir: Vec<String> = Vec::new();
            let mut partition_lowerdir: HashMap<String, Vec<String>> = HashMap::new();
            let mut config = CONFIG.read().unwrap();

            for part in config.partitions.iter() {
                partition_lowerdir.insert((*part).to_string(), Vec::new());
            }

            for i in OVERLAYFS_ID.lock().unwrap().iter() {
                let path = Path::new(&config.moduledir).join(i);

                if path.is_dir()
                    && let Some(v) = partition_lowerdir.get_mut(i)
                {
                    v.push(path.display().to_string());
                    log::info!("  + {}/", i);
                }
            }
            if let Err(e) = mount_partition(
                "system",
                &system_lowerdir,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                config.disable_umount,
            ) {
                log::warn!("mount system failed: {e:#}");
            }

            for (k, v) in partition_lowerdir {
                if let Err(e) = mount_partition(
                    &k,
                    &v,
                    #[cfg(any(target_os = "linux", target_os = "android"))]
                    config.disable_umount,
                ) {
                    log::warn!("mount {k} failed: {e:#}");
                }
            }
        })?;

    let magic_mount = thread::Builder::new()
        .name("Magic-Mount".to_string())
        .spawn(|| {
            let mut config = CONFIG.read().unwrap();
            let need_id = MAGIC_MOUNT_ID.lock().unwrap().clone();

            if let Err(e) = rustix_mount(
                &config.mountsource,
                &config.hybrid_mnt_dir,
                "tmpfs",
                MountFlags::empty(),
                None,
            ) {
                log::error!("mount tmpfs failed: {e}");
            }

            magic_mount(
                &config.hybrid_mnt_dir,
                &config.moduledir,
                &config.mountsource,
                &config.partitions,
                need_id,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                !config.disable_umount,
            )
        })?;

    magic_mount.join().unwrap()?;
    overlayfs.join().unwrap();
    Ok(())
}
