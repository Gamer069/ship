use std::{
    io::ErrorKind,
    path::Path,
    process::{Command, Stdio},
};

use clap::Parser;

use crate::{
    appimage::AppImageGenerator,
    conf::{ShipConfig, Target},
    deb::DebGenerator,
};

pub mod conf;
pub mod deb;
pub mod appimage;
pub mod gen_;

use gen_::Generator as _;

#[derive(Parser, Debug)]
#[command(
    name = "ship",
    author = "Your Name <you@example.com>",
    version = "0.1.0",
    about = "Generates cross-platform installers from a Shipfile",
    long_about = "Ship reads a Shipfile TOML configuration, resolves variables, and produces platform-specific installers. Supports dry-run mode and CLI overrides for version and targets."
)]
pub struct Cli {
    /// Path to the Shipfile
    #[arg(short, long, default_value = "ship.toml", value_name = "FILE")]
    pub config: String,

    /// Dry run mode — prints what would be generated without building installers
    #[arg(short = 'd', long = "dry-run")]
    pub dry_run: bool,
}

fn main() {
    let cli = Cli::parse();

    let contents = std::fs::read_to_string(&cli.config).unwrap_or_else(|e| {
        match e.kind() {
            ErrorKind::NotFound => {
                eprintln!("error: no `{}` present, terminating...", cli.config);
            }
            ErrorKind::IsADirectory => {
                eprintln!("error: `{}` is a directory, terminating...", cli.config);
            }
            _ => {
                eprintln!("error: {e}");
            }
        }

        std::process::exit(-1);
    });

    println!("building...");

    let conf: ShipConfig = toml::from_str(&contents).unwrap_or_else(|e| {
        eprintln!("Failed to parse {}: {}", cli.config, e);
        std::process::exit(-1);
    });

    if conf.out.targets.is_empty() {
        eprintln!("no targets!");
        std::process::exit(0);
    }

    // execute build command
    if let Some(ref build) = conf.build
        && let Some(cmd_str) = &build.cmd
    {
        #[cfg(unix)]
        let mut cmd_builder = Command::new("sh");
        #[cfg(windows)]
        let mut cmd_builder = Command::new("cmd");

        #[cfg(unix)]
        cmd_builder.arg("-c").arg(cmd_str);
        #[cfg(windows)]
        cmd_builder.arg("/C").arg(cmd_str);

        cmd_builder
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // set current_dir if build.cwd is Some
        if let Some(cwd) = &build.cwd {
            cmd_builder.current_dir(Path::new(cwd));
        }

        let mut cmd = cmd_builder.spawn().unwrap_or_else(|err| {
            eprintln!(
                "error while spawning child process to execute build command: {err}, terminating..."
            );
            std::process::exit(-1);
        });

        let status = cmd.wait().unwrap();
        println!("exited build child process with status {}", status);
    }

    for target in &conf.out.targets {
        match target {
            Target::Deb => {
                let generator = DebGenerator::new(&conf);

                generator.run();
            }
            Target::AppImage => {
                let generator = AppImageGenerator::new(&conf);

                generator.run();
            }
            t => {
                eprintln!("target {:?} not yet supported; skipping...", t);
            }
        }
    }
}
