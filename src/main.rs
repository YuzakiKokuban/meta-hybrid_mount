// meta-hybrid_mount/src/main.rs
mod conf;
mod core;
mod defs;
mod mount;
mod utils;

use std::path::{Path, PathBuf};
use anyhow::Result;
use clap::Parser;
use rustix::mount::{unmount, UnmountFlags};
use mimalloc::MiMalloc;

use conf::{
    cli::{Cli, Commands},
    config::{Config, CONFIG_FILE_DEFAULT},
};
use core::{
    executor,
    modules,
    planner,
    state::RuntimeState,
    storage,
};
use mount::{
    nuke,
};

// Set mimalloc as the global allocator for better performance
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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

    // Clean up previous mounts if necessary
    if mnt_base.exists() { let _ = unmount(&mnt_base, UnmountFlags::DETACH); }

    // 2. Smart Storage Setup (Tmpfs vs Ext4)
    let img_path = Path::new(defs::BASE_DIR).join("modules.img");
    let storage_mode = storage::setup(&mnt_base, &img_path, config.force_ext4)?;
    
    // 3. Populate Storage (Sync active modules)
    if let Err(e) = modules::sync_active(&config.moduledir, &mnt_base) {
        log::error!("Critical: Failed to sync modules: {:#}", e);
    }

    // --- ARCHITECTURE REFACTOR: PLANNER & EXECUTOR ---

    // 4. Generate Mount Plan
    log::info!("Generating mount plan...");
    let plan = planner::generate(&config, &mnt_base)?;
    
    log::info!("Plan: {} OverlayFS operations, {} Magic Mount modules", 
        plan.overlay_ops.len(), 
        plan.magic_module_paths.len()
    );

    // 5. Execute Plan
    // Execution result contains the FINAL list of active modules, accounting for fallbacks
    let exec_result = executor::execute(&plan, &config)?;

    // --------------------------------------------------

    // Phase C: Nuke LKM (Stealth)
    let mut nuke_active = false;
    if storage_mode == "ext4" && config.enable_nuke {
        nuke_active = nuke::try_load(&mnt_base);
    }

    // Update module description (Catgirl Mode 🐱)
    // Counts come from actual execution result
    modules::update_description(
        &storage_mode, 
        nuke_active, 
        exec_result.overlay_module_ids.len(), 
        exec_result.magic_module_ids.len()
    );

    // Save structured state
    // We save the ACTUAL mounted modules, not just the planned ones
    let state = RuntimeState::new(
        storage_mode,
        mnt_base,
        exec_result.overlay_module_ids,
        exec_result.magic_module_ids,
        plan.conflicts,
        nuke_active
    );
    if let Err(e) = state.save() {
        log::error!("Failed to save runtime state: {}", e);
    }

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
