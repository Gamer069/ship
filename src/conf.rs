use clap::ValueEnum;
use deb::DebArchitecture;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Prog {
    pub name: String,   // required
    pub author: String, // required
    pub arch: Arch,
    pub version: Option<String>,     // optional
    pub description: Option<String>, // optional
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum Arch {
    All,
    Alpha,
    Armel,
    Armhf,
    Arm64,
    Hppa,
    I386,
    Amd64,
    Ia64,
    M68k,
    Mips,
    Mipsel,
    Mips64el,
    PowerPC,
    Ppc64,
    Ppc64el,
    Riscv64,
    S390x,
    Sh4,
    Sparc4,
    X32,
    HurdI386,
    KFreebsdI386,
    KFreebsdAmd64,
}

impl Arch {
    pub fn deb(&self) -> DebArchitecture {
        match self {
            Arch::All => DebArchitecture::All,
            Arch::Alpha => DebArchitecture::Alpha,
            Arch::Armel => DebArchitecture::Armel,
            Arch::Armhf => DebArchitecture::Armhf,
            Arch::Arm64 => DebArchitecture::Arm64,
            Arch::Hppa => DebArchitecture::Hppa,
            Arch::I386 => DebArchitecture::I386,
            Arch::Amd64 => DebArchitecture::Amd64,
            Arch::Ia64 => DebArchitecture::Ia64,
            Arch::M68k => DebArchitecture::M68k,
            Arch::Mips => DebArchitecture::Mips,
            Arch::Mipsel => DebArchitecture::Mipsel,
            Arch::Mips64el => DebArchitecture::Mips64el,
            Arch::PowerPC => DebArchitecture::PowerPC,
            Arch::Ppc64 => DebArchitecture::Ppc64,
            Arch::Ppc64el => DebArchitecture::Ppc64el,
            Arch::Riscv64 => DebArchitecture::Riscv64,
            Arch::S390x => DebArchitecture::S390x,
            Arch::Sh4 => DebArchitecture::Sh4,
            Arch::Sparc4 => DebArchitecture::Sparc4,
            Arch::X32 => DebArchitecture::X32,
            Arch::HurdI386 => DebArchitecture::HurdI386,
            Arch::KFreebsdI386 => DebArchitecture::KFreebsdI386,
            Arch::KFreebsdAmd64 => DebArchitecture::KFreebsdAmd64,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Files {
    pub paths: Vec<String>,      // required
    pub icon: Option<String>,    // optional
    pub license: Option<String>, // optional
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Build {
    pub cmd: Option<String>, // optional build command
    pub cwd: Option<String>, // optional working directory
}

/// Supported installer target types
#[derive(Serialize, Deserialize, ValueEnum, Clone, PartialEq, Eq, Debug)]
pub enum Target {
    Exe,
    Msi,
    Dmg,
    Pkg,
    Deb,
    AppImage,
    Rpm,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Out {
    pub targets: Vec<Target>, // required
    #[serde(default = "default_bin_dir")]
    pub bin: String,
}

fn default_bin_dir() -> String {
    "./bin/".to_string()
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Vars {
    pub env: Option<Vec<String>>,   // optional
    pub arg: Option<Vec<String>>,   // optional
    pub cmake: Option<Vec<String>>, // optional
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Scripts {
    pub preinstall: Option<String>,  // optional
    pub postinstall: Option<String>, // optional
}

/// Top-level config
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct ShipConfig {
    pub prog: Prog,
    pub files: Files,
    pub build: Option<Build>,
    pub out: Out,
    pub vars: Option<Vars>,
    pub scripts: Option<Scripts>,
}
