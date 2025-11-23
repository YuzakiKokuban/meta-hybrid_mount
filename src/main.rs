mod config;
mod magic_mount;
mod utils;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use config::Config;

const CONFIG_FILE_DEFAULT: &str = "/data/adb/magic_mount/config.toml";

#[derive(Parser, Debug)]
#[command(name = "magic_mount", version, about = "Magic Mount Metamodule")]
struct Cli {
    /// Config file path
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,

    /// Module directory path
    #[arg(short = 'm', long = "moduledir")]
    moduledir: Option<PathBuf>,

    /// Temporary directory path (auto-selected if not specified)
    #[arg(short = 't', long = "tempdir")]
    tempdir: Option<PathBuf>,

    /// Mount source name
    #[arg(short = 's', long = "mountsource")]
    mountsource: Option<String>,

    /// Enable verbose (debug) logging
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Extra partitions, comma-separated, eg: -p mi_ext,my_stock
    #[arg(short = 'p', long = "partitions", value_delimiter = ',')]
    partitions: Vec<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate example config file
    GenConfig {
        /// Output path for config file
        #[arg(short = 'o', long = "output", default_value = CONFIG_FILE_DEFAULT)]
        output: PathBuf,
    },
    /// Show current effective configuration
    ShowConfig,
}

fn load_config(cli: &Cli) -> Result<Config> {
    // 1. 尝试从指定的配置文件加载
    if let Some(config_path) = &cli.config {
        log::info!("Loading config from: {}", config_path.display());
        return Config::from_file(config_path).context("failed to load specified config file");
    }

    // 2. 尝试从默认位置加载
    if let Some(config) = Config::load_default() {
        log::info!(
            "Loaded config from default location: {}",
            CONFIG_FILE_DEFAULT
        );
        return Ok(config);
    }

    // 3. 使用默认配置
    log::info!("Using default configuration (no config file found)");
    Ok(Config::default())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // 处理子命令
    if let Some(command) = &cli.command {
        match command {
            Commands::GenConfig { output } => {
                return generate_config(output);
            }
            Commands::ShowConfig => {
                let config = load_config(&cli)?;
                return show_config(&config);
            }
        }
    }

    // 加载配置
    let mut config = load_config(&cli)?;

    // 命令行参数覆盖配置文件
    config.merge_with_cli(
        cli.moduledir,
        cli.tempdir,
        cli.mountsource,
        cli.verbose,
        cli.partitions,
    );

    // 初始化日志
    utils::init_logger(config.verbose)?;

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
    if !config.partitions.is_empty() {
        log::info!("extra partitions: {:?}", config.partitions);
    }

    utils::ensure_temp_dir(&tempdir)?;

    let result = magic_mount::magic_mount(
        &tempdir,
        &config.moduledir,
        &config.mountsource,
        &config.partitions,
    );

    utils::cleanup_temp_dir(&tempdir);

    match result {
        Ok(_) => {
            log::info!("Magic Mount Completed Successfully");
            Ok(())
        }
        Err(e) => {
            log::error!("Magic Mount Failed");
            log::error!("error: {:#}", e);
            Err(e)
        }
    }
}

fn generate_config(output: &PathBuf) -> Result<()> {
    let config = Config::default();
    config
        .save_to_file(output)
        .context("failed to generate config file")?;

    println!("✓ Config file generated at: {}", output.display());
    println!("\nExample content:");
    println!("{}", Config::example());
    Ok(())
}

fn show_config(config: &Config) -> Result<()> {
    println!("Current Configuration:");
    println!("=====================");
    println!("Module Dir    : {}", config.moduledir.display());
    println!(
        "Temp Dir      : {}",
        config
            .tempdir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(auto)".to_string())
    );
    println!("Mount Source  : {}", config.mountsource);
    println!("Verbose       : {}", config.verbose);
    println!("Partitions    : {:?}", config.partitions);
    Ok(())
}
