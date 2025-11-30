#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless
)]

mod config;
mod defs;
mod magic_mount;
mod scanner;
mod utils;

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use env_logger::Builder;
use mimalloc::MiMalloc;

use crate::{
    config::Config,
    defs::CONFIG_FILE_DEFAULT,
    magic_mount::UMOUNT,
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn load_config() -> Config {
    if let Ok(config) = Config::load_default() {
        log::info!("Loaded config from default location: {CONFIG_FILE_DEFAULT}");
        return config;
    }

    log::info!("Using default configuration (no config file found)");
    Config::default()
}

fn init_logger(verbose: bool) {
    let level = if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    let mut builder = Builder::new();

    builder.format(|buf, record| {
        writeln!(
            buf,
            "[{}] [{}] {}",
            record.level(),
            record.target(),
            record.args()
        )
    });
    builder.filter_level(level).init();

    log::info!("log level: {}", level.as_str());
}

fn main() -> Result<()> {
    
    let config = load_config();

    let args: Vec<_> = std::env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "scan" => {
                let json_output = args.len() > 2 && args[2] == "--json";
                
                let modules = scanner::scan_modules(&config.moduledir)?;

                if json_output {
                    let json = serde_json::to_string(&modules)?;
                    println!("{json}");
                } else {
                    for module in modules {
                        println!("{}", module.id);
                    }
                }
                return Ok(());
            },
            "storage" => {
                let check_path = Path::new("/data/adb");
                let storage = scanner::get_storage_usage(check_path)
                    .unwrap_or(scanner::StorageInfo {
                        size: "-".to_string(),
                        used: "-".to_string(),
                        percent: "0%".to_string(),
                    });
                let json = serde_json::to_string(&storage)?;
                println!("{json}");
                return Ok(());
            },
            _ => {}
        }
    }

    init_logger(config.verbose);

    log::info!("Magic Mount Starting");
    log::info!("module dir      : {}", config.moduledir.display());

    let tempdir = if let Some(temp) = config.tempdir {
        log::info!("temp dir (cfg)  : {}", temp.display());
        temp
    } else {
        let temp = utils::select_temp_dir().context("failed to select temp dir automatically")?;
        log::info!("temp dir (auto) : {}", temp.display());
        temp
    };

    log::info!("mount source    : {}", config.mountsource);
    log::info!("verbose mode    : {}", config.verbose);
    log::info!(
        "extra partitions: {}",
        if config.partitions.is_empty() {
            "None".to_string()
        } else {
            format!("{:?}", config.partitions)
        }
    );

    utils::ensure_temp_dir(&tempdir)?;

    if config.umount {
        UMOUNT.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let result = magic_mount::magic_mount(
        &tempdir,
        &config.moduledir,
        &config.mountsource,
        &config.partitions,
    );

    utils::cleanup_temp_dir(&tempdir);

    match result {
        Ok(()) => {
            log::info!("Magic Mount Completed Successfully");
            Ok(())
        }
        Err(e) => {
            log::error!("Magic Mount Failed");
            for cause in e.chain() {
                log::error!("{cause:#?}");
            }
            log::error!("{:#?}", e.backtrace());
            Err(e)
        }
    }
}
