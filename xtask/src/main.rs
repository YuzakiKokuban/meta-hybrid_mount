// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use fs_extra::{
    dir::{self},
    file::{self},
};
use tempfile::NamedTempFile;
use zip::{write::FileOptions, CompressionMethod};

mod zip_ext;
use crate::zip_ext::zip_create_from_directory_with_options;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
enum Arch {
    #[value(name = "arm64")]
    Arm64,
    #[value(name = "arm")]
    Arm,
    #[value(name = "x86_64")]
    X86_64,
    #[value(name = "riscv64")]
    Riscv64,
}

impl Arch {
    fn target(&self) -> &'static str {
        match self {
            Arch::Arm64 => "aarch64-linux-android",
            Arch::Arm => "armv7-linux-androideabi",
            Arch::X86_64 => "x86_64-linux-android",
            Arch::Riscv64 => "riscv64-linux-android",
        }
    }
    fn android_abi(&self) -> &'static str {
        match self {
            Arch::Arm64 => "arm64-v8a",
            Arch::Arm => "armeabi-v7a",
            Arch::X86_64 => "x86_64",
            Arch::Riscv64 => "riscv64",
        }
    }
    fn api_level(&self) -> &'static str {
        match self {
            Arch::Riscv64 => "35",
            _ => "29",
        }
    }
}

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        #[arg(long)]
        release: bool,
        #[arg(long)]
        skip_webui: bool,
        #[arg(long, value_enum)]
        arch: Option<Arch>,

        /// Path to the encrypted private key (default: private.enc)
        #[arg(long, default_value = "private.enc")]
        key_enc: PathBuf,

        /// Path to the certificate (default: cert.pem)
        #[arg(long, default_value = "cert.pem")]
        cert: PathBuf,
    },
    Lint,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = project_root();
    match cli.command {
        Commands::Build {
            release,
            skip_webui,
            arch,
            key_enc,
            cert,
        } => {
            build_full(&root, release, skip_webui, arch, &key_enc, &cert)?;
        }
        Commands::Lint => {
            run_clippy(&root)?;
        }
    }
    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn run_clippy(root: &Path) -> Result<()> {
    println!(":: Running Clippy...");

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let status = Command::new(cargo)
        .current_dir(root)
        .args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ])
        .status()
        .context("Failed to run cargo clippy")?;

    if !status.success() {
        anyhow::bail!("Clippy found issues! Please fix them before committing.");
    }

    println!(":: Clippy checks passed!");
    Ok(())
}

fn build_full(
    root: &Path,
    release: bool,
    skip_webui: bool,
    target_arch: Option<Arch>,
    key_enc_path: &Path,
    cert_path: &Path,
) -> Result<()> {
    let output_dir = root.join("output");
    let stage_dir = output_dir.join("staging");
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir)?;
    }
    fs::create_dir_all(&stage_dir)?;
    let version = get_version(root)?;
    if !skip_webui {
        println!(":: Building WebUI...");
        build_webui(root, &version)?;
    }

    let archs_to_build = if let Some(selected) = target_arch {
        vec![selected]
    } else {
        vec![Arch::Arm64, Arch::X86_64, Arch::Riscv64]
    };

    for arch in archs_to_build {
        println!(":: Compiling Core for {:?}...", arch);
        compile_core(root, release, arch)?;
        let bin_name = "meta-hybrid";
        let profile = if release { "release" } else { "debug" };
        let src_bin = root
            .join("target")
            .join(arch.target())
            .join(profile)
            .join(bin_name);
        let stage_bin_dir = stage_dir.join("binaries").join(arch.android_abi());
        fs::create_dir_all(&stage_bin_dir)?;
        if src_bin.exists() {
            file::copy(
                &src_bin,
                stage_bin_dir.join(bin_name),
                &file::CopyOptions::new().overwrite(true),
            )?;
        } else {
            println!("Warning: Binary not found at {}", src_bin.display());
        }
    }
    println!(":: Copying module scripts...");
    let module_src = root.join("module");
    let options = dir::CopyOptions::new().overwrite(true).content_only(true);
    dir::copy(&module_src, &stage_dir, &options)?;
    let gitignore = stage_dir.join(".gitignore");
    if gitignore.exists() {
        fs::remove_file(gitignore)?;
    }
    println!(":: Injecting version: {}", version);
    update_module_prop(&stage_dir.join("module.prop"), &version)?;
    println!(":: Creating Zip...");
    let zip_file = output_dir.join(format!("Meta-Hybrid-{}.zip", version));
    let zip_options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9));
    zip_create_from_directory_with_options(&zip_file, &stage_dir, |_| zip_options)?;
    println!(":: Build Complete: {}", zip_file.display());
    if let Ok(password) = env::var("META_HYBRID_SIGN_PASSWORD") {
        if !password.is_empty() {
            let abs_key_enc = root.join(key_enc_path);
            let abs_cert = root.join(cert_path);

            if abs_key_enc.exists() && abs_cert.exists() {
                decrypt_and_sign(&zip_file, &abs_key_enc, &abs_cert, &password)?;
            } else {
                println!(":: Skipping signature: private.enc or cert.pem not found at root.");
            }
        }
    } else {
        println!(":: Skipping signature: META_HYBRID_SIGN_PASSWORD not set.");
    }

    Ok(())
}

fn decrypt_and_sign(
    zip_path: &Path,
    enc_key_path: &Path,
    cert_path: &Path,
    password: &str,
) -> Result<()> {
    println!(":: Decrypting private key...");
    let temp_key = NamedTempFile::new()?;
    let temp_key_path = temp_key.path();
    let status = Command::new("openssl")
        .args(["aes-256-cbc", "-d", "-pbkdf2", "-in"])
        .arg(enc_key_path)
        .arg("-out")
        .arg(temp_key_path)
        .arg("-pass")
        .arg("env:OPENSSL_PASS_VAR")
        .env("OPENSSL_PASS_VAR", password)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to execute openssl for decryption")?;

    if !status.success() {
        anyhow::bail!("Failed to decrypt private key. Check password and openssl version.");
    }

    println!(":: Signing module with ksusig...");
    let output_path = zip_path.with_file_name(format!(
        "{}-signed.zip",
        zip_path.file_stem().unwrap_or_default().to_string_lossy()
    ));

    let status = Command::new("ksusig")
        .arg("sign")
        .arg("--key")
        .arg(temp_key_path)
        .arg("--cert")
        .arg(cert_path)
        .arg(zip_path)
        .arg(&output_path)
        .status()
        .context("Failed to execute ksusig")?;

    if !status.success() {
        anyhow::bail!("ksusig signing failed.");
    }
    fs::rename(&output_path, zip_path)?;
    println!(":: Signed successfully!");
    Ok(())
}

fn build_webui(root: &Path, version: &str) -> Result<()> {
    generate_webui_constants(root, version)?;
    let webui_dir = root.join("webui");
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let status = Command::new(npm)
        .current_dir(&webui_dir)
        .arg("install")
        .status()?;
    if !status.success() {
        anyhow::bail!("npm install failed");
    }
    let status = Command::new(npm)
        .current_dir(&webui_dir)
        .args(["run", "build"])
        .status()?;
    if !status.success() {
        anyhow::bail!("npm run build failed");
    }
    Ok(())
}

fn generate_webui_constants(root: &Path, version: &str) -> Result<()> {
    let path = root.join("webui/src/lib/constants_gen.ts");
    let content = format!(
        r#"
export const APP_VERSION = "{version}";
export const RUST_PATHS = {{
  CONFIG: "/data/adb/meta-hybrid/config.toml",
  MODE_CONFIG: "/data/adb/meta-hybrid/module_mode.conf",
  IMAGE_MNT: "/data/adb/meta-hybrid/mnt",
  DAEMON_STATE: "/data/adb/meta-hybrid/run/daemon_state.json",
  DAEMON_LOG: "/data/adb/meta-hybrid/daemon.log",
}} as const;
export const BUILTIN_PARTITIONS = ["system", "vendor", "product", "system_ext", "odm", "oem", "apex"] as const;
"#
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    let old_path = root.join("webui/src/lib/constants_gen.js");
    if old_path.exists() {
        let _ = fs::remove_file(old_path);
    }
    Ok(())
}

fn compile_core(root: &Path, release: bool, arch: Arch) -> Result<()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    if !matches!(arch, Arch::Riscv64) {
        let _ = Command::new("rustup")
            .args(["target", "add", arch.target()])
            .status();
    }
    let ndk_home = env::var("ANDROID_NDK_HOME").context("ANDROID_NDK_HOME not set")?;
    let host_os = std::env::consts::OS;
    let host_tag = match host_os {
        "linux" => "linux-x86_64",
        "macos" => "darwin-x86_64",
        "windows" => "windows-x86_64",
        _ => panic!("Unsupported OS"),
    };
    let toolchain_bin = PathBuf::from(ndk_home)
        .join("toolchains/llvm/prebuilt")
        .join(host_tag)
        .join("bin");
    let api = arch.api_level();
    let cc_name = match arch {
        Arch::Arm64 => format!("aarch64-linux-android{}-clang", api),
        Arch::Arm => format!("armv7a-linux-androideabi{}-clang", api),
        Arch::X86_64 => format!("x86_64-linux-android{}-clang", api),
        Arch::Riscv64 => format!("riscv64-linux-android{}-clang", api),
    };
    let cc_path = toolchain_bin.join(&cc_name);
    let ar_path = toolchain_bin.join("llvm-ar");
    if !cc_path.exists() {
        anyhow::bail!("Compiler not found: {}", cc_path.display());
    }
    let mut cmd = Command::new(&cargo);
    cmd.current_dir(root);
    cmd.arg("build").arg("--target").arg(arch.target());
    if matches!(arch, Arch::Riscv64) {
        cmd.arg("-Z").arg("build-std=std,panic_abort");
    }
    if release {
        cmd.arg("--release");
    }
    let path_val = env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("{}:{}", toolchain_bin.display(), path_val));
    let env_target = arch.target().replace('-', "_");
    cmd.env(format!("CC_{}", env_target), &cc_path);
    cmd.env(format!("AR_{}", env_target), &ar_path);
    cmd.env("CC", &cc_path);
    cmd.env("AR", &ar_path);
    cmd.env(
        format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
        &cc_path,
    );
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("Compilation failed for {}", arch.target());
    }
    Ok(())
}

fn get_version(root: &Path) -> Result<String> {
    if let Ok(v) = env::var("META_HYBRID_VERSION") {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let output = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output();
    if let Ok(o) = output {
        if o.status.success() {
            return Ok(String::from_utf8(o.stdout)?.trim().to_string());
        }
    }
    let toml_path = root.join("module/config.toml");
    if toml_path.exists() {
        let content = fs::read_to_string(toml_path)?;
        for line in content.lines() {
            if line.trim().starts_with("version") {
                if let Some(v) = line.split('"').nth(1) {
                    return Ok(format!("{}-dev", v));
                }
            }
        }
    }
    Ok("v0.0.0-unknown".to_string())
}

fn update_module_prop(path: &Path, version: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(path)?;
    let mut new_lines = Vec::new();
    let code = if let Ok(env_code) = env::var("META_HYBRID_CODE") {
        env_code
    } else {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };
        let mut hasher = DefaultHasher::new();
        version.hash(&mut hasher);
        ((hasher.finish() % 100000) as u32).to_string()
    };
    for line in content.lines() {
        if line.starts_with("version=") {
            new_lines.push(format!("version={}", version));
        } else if line.starts_with("versionCode=") {
            new_lines.push(format!("versionCode={}", code));
        } else {
            new_lines.push(line.to_string());
        }
    }
    fs::write(path, new_lines.join("\n"))?;
    Ok(())
}
