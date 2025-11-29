// meta-hybrid_mount/src/main.rs
mod cli;
mod config;
mod defs;
mod modules;
mod nuke;
mod storage;
mod utils;

#[path = "magic_mount/mod.rs"]
mod magic_mount;
mod overlay_mount;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use anyhow::Result; // Removed unused `Context`
use clap::Parser;
use rustix::mount::{unmount, UnmountFlags};

use cli::{Cli, Commands};
use config::{Config, CONFIG_FILE_DEFAULT};

fn load_config(cli: &Cli) -> Result<Config> {
    if let Some(config_path) = &cli.config {
        return Config::from_file(config_path);
    }
    match Config::load_default() {
        Ok(config) => Ok(config),
        Err(e) => {
            if Path::new(CONFIG_FILE_DEFAULT).exists() {
                eprintln!("Error loading config: {:#}", e);
            }
            Ok(Config::default())
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle Subcommands
    if let Some(command) = &cli.command {
        match command {
            Commands::GenConfig { output } => { 
                Config::default().save_to_file(output)?; 
                return Ok(()); 
            },
            Commands::ShowConfig => { 
                let config = load_config(&cli)?;
                println!("{}", serde_json::to_string(&config)?); 
                return Ok(()); 
            },
            Commands::Storage => { 
                storage::print_status()?; 
                return Ok(()); 
            },
            Commands::Modules => { 
                let config = load_config(&cli)?;
                modules::print_list(&config)?; 
                return Ok(()); 
            }
        }
    }

    // Initialize Daemon Logic
    let mut config = load_config(&cli)?;
    config.merge_with_cli(
        cli.moduledir.clone(), 
        cli.tempdir.clone(), 
        cli.mountsource.clone(), 
        cli.verbose, 
        cli.partitions.clone()
    );

    utils::init_logger(config.verbose, Path::new(defs::DAEMON_LOG_FILE))?;

    // [STEALTH] Camouflage process name
    if let Err(e) = utils::camouflage_process("kworker/u9:1") {
        log::warn!("Failed to camouflage process: {}", e);
    }

    log::info!("Hybrid Mount Starting (True Hybrid Mode)...");

    if config.disable_umount {
        log::warn!("Namespace Detach (try_umount) is DISABLED via config.");
    }

    utils::ensure_dir_exists(defs::RUN_DIR)?;

    // 1. Static Mount Point Strategy
    let mnt_base = PathBuf::from(defs::FALLBACK_CONTENT_DIR);
    log::info!("Using fixed mount point at {}", mnt_base.display());
    utils::ensure_dir_exists(&mnt_base)?;

    // Save mount point state for CLI tools
    if let Err(e) = fs::write(defs::MOUNT_POINT_FILE, mnt_base.to_string_lossy().as_bytes()) {
        log::error!("Failed to write mount state: {}", e);
    }

    // Clean up previous mounts if necessary
    if mnt_base.exists() { let _ = unmount(&mnt_base, UnmountFlags::DETACH); }

    // 2. Smart Storage Setup (Tmpfs vs Ext4)
    let img_path = Path::new(defs::BASE_DIR).join("modules.img");
    let storage_mode = storage::setup(&mnt_base, &img_path, config.force_ext4)?;
    
    // Persist storage mode state
    if let Err(e) = fs::write(defs::STORAGE_MODE_FILE, &storage_mode) {
        log::warn!("Failed to write storage mode state: {}", e);
    }
    
    // 3. Populate Storage (Sync active modules)
    if let Err(e) = modules::sync_active(&config.moduledir, &mnt_base) {
        log::error!("Critical: Failed to sync modules: {:#}", e);
    }

    // 4. Scan & Group Modules
    let module_modes = config::load_module_modes();
    let mut active_modules: HashMap<String, PathBuf> = HashMap::new();
    if let Ok(entries) = fs::read_dir(&mnt_base) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Filter out system directories like 'lost+found' (common in ext4)
                // and our own directory to prevent miscounting modules
                if name != "lost+found" && name != "meta-hybrid" {
                    active_modules.insert(name, entry.path());
                }
            }
        }
    }
    log::info!("Loaded {} modules from storage ({})", active_modules.len(), storage_mode);

    // 5. Partition Grouping & True Hybrid Logic
    let mut partition_overlay_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut magic_mount_modules: HashSet<PathBuf> = HashSet::new();
    
    let mut all_partitions = defs::BUILTIN_PARTITIONS.to_vec();
    let extra_parts: Vec<&str> = config.partitions.iter().map(|s| s.as_str()).collect();
    all_partitions.extend(extra_parts);

    // Iterate modules to decide Overlay vs Magic
    for (module_id, content_path) in &active_modules {
        let mode = module_modes.get(module_id).map(|s| s.as_str()).unwrap_or("auto");
        if mode == "magic" {
            magic_mount_modules.insert(content_path.clone());
            log::info!("Module '{}' assigned to Magic Mount", module_id);
        } else {
            for &part in &all_partitions {
                if content_path.join(part).is_dir() {
                    partition_overlay_map.entry(part.to_string()).or_default().push(content_path.clone());
                }
            }
        }
    }

    // Phase A: OverlayFS
    for (part, modules) in &partition_overlay_map {
        let target_path = format!("/{}", part);
        let overlay_paths: Vec<String> = modules.iter().map(|m| m.join(part).display().to_string()).collect();
        log::info!("Mounting {} [OVERLAY] ({} layers)", target_path, overlay_paths.len());
        if let Err(e) = overlay_mount::mount_overlay(&target_path, &overlay_paths, None, None, config.disable_umount) {
            log::error!("OverlayFS mount failed for {}: {:#}. Fallback to Magic.", target_path, e);
            for m in modules { magic_mount_modules.insert(m.clone()); }
        }
    }

    // Capture magic count before execution
    let magic_count = magic_mount_modules.len();

    // Phase B: Magic Mount
    if !magic_mount_modules.is_empty() {
        let tempdir = if let Some(t) = &config.tempdir { t.clone() } else { utils::select_temp_dir()? };
        log::info!("Starting Magic Mount Engine for {} modules...", magic_mount_modules.len());
        utils::ensure_temp_dir(&tempdir)?;
        let module_list: Vec<PathBuf> = magic_mount_modules.into_iter().collect();
        if let Err(e) = magic_mount::mount_partitions(&tempdir, &module_list, &config.mountsource, &config.partitions, config.disable_umount) {
            log::error!("Magic Mount failed: {:#}", e);
        }
        utils::cleanup_temp_dir(&tempdir);
    }

    // Phase C: Nuke LKM (Stealth)
    let mut nuke_active = false;
    if storage_mode == "ext4" && config.enable_nuke {
        nuke_active = nuke::try_load(&mnt_base);
    }

    // Update module description with stats (Catgirl Mode 🐱)
    let overlay_count = active_modules.len().saturating_sub(magic_count);
    modules::update_description(&storage_mode, nuke_active, overlay_count, magic_count);

    log::info!("Hybrid Mount Completed");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        log::error!("Fatal Error: {:#}", e);
        eprintln!("Fatal Error: {:#}", e);
        std::process::exit(1);
    }
}
