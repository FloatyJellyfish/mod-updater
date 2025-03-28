use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter},
    io::ErrorKind,
};

use modrinth::Loaders;
use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};
use clap::{Parser, Subcommand};

pub mod modrinth;

pub enum Error {
    Reqwest(reqwest::Error),
    NotFound,
    StatusCode(reqwest::StatusCode),
    NoVersionsFound,
    InvalidIndex,
    NoFilesFound,
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
    JoinError(tokio::task::JoinError),
    NoGameVersions,
    InvalidRequest,
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}

impl From<reqwest::StatusCode> for Error {
    fn from(value: reqwest::StatusCode) -> Self {
        Self::StatusCode(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(value: serde_yaml::Error) -> Self {
        Self::Yaml(value)
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(value: tokio::task::JoinError) -> Self {
        Self::JoinError(value)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reqwest(arg0) => f.debug_tuple("Reqwest").field(arg0).finish(),
            Self::NotFound => write!(f, "Mod not found"),
            Self::StatusCode(arg0) => f.debug_tuple("StatusCode").field(arg0).finish(),
            Self::NoVersionsFound => write!(f, "No mod versions found"),
            Self::InvalidIndex => write!(f, "Invalid index"),
            Self::NoFilesFound => write!(f, "No files found"),
            Self::Io(arg0) => f.debug_tuple("IO").field(arg0).finish(),
            Self::Yaml(arg0) => f.debug_tuple("YAML").field(arg0).finish(),
            Self::JoinError(arg0) => f.debug_tuple("JoinError").field(arg0).finish(),
            Self::NoGameVersions => write!(f, "Failed to get game versions"),
            Self::InvalidRequest => write!(f, "Invalid request"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub loader: Loaders,
    pub version: String,
    pub mods: Vec<String>,
}

impl Config {
    const CONFIG_PATH: &str = "mods.yaml";

    pub async fn try_load() -> Result<Config, Error> {
        match tokio::fs::File::open(Self::CONFIG_PATH).await {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents).await?;
                Ok(serde_yaml::from_str(&contents)?)
            }
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    eprintln!("mods.yaml config file not found in current directory. Maybe you forgot to 'pack init'?");
                }
                Err(err.into())
            }
        }
    }

    pub async fn try_save(&mut self) -> Result<(), Error> {
        self.mods.sort();
        let contents = serde_yaml::to_string(&self)?;
        let mut file = File::create(Self::CONFIG_PATH).await?;
        file.write_all(contents.as_bytes()).await?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstalledMod {
    pub version: String,
    pub file: String,
}

#[derive(Serialize, Deserialize)]
pub struct ModManifest {
    pub installed: BTreeMap<String, InstalledMod>,
}

impl ModManifest {
    const CONFIG_PATH: &str = ".installed.yaml";

    pub async fn try_load() -> Result<ModManifest, Error> {
        match tokio::fs::File::open(Self::CONFIG_PATH).await {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents).await?;
                Ok(serde_yaml::from_str(&contents)?)
            }
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    Ok(ModManifest {
                        installed: BTreeMap::new(),
                    })
                } else {
                    Err(err.into())
                }
            }
        }
    }

    pub async fn try_save(&self) -> Result<(), Error> {
        let contents = serde_yaml::to_string(&self)?;
        let mut file = File::create(Self::CONFIG_PATH).await?;
        file.write_all(contents.as_bytes()).await?;
        Ok(())
    }
}

#[derive(Parser)]
#[command(name = "Mod Updater")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// List all versions for a mod
    Versions {
        /// Mod slug or id
        mod_name: String,
        /// Filter by mod loader
        #[arg(short, long)]
        loader: Option<Loaders>,
        /// Filter by game version (e.g. 1.21.4)
        #[arg(short, long)]
        game_version: Option<String>,
    },
    /// Get latest version of a mod for a given mod loader
    Latest {
        /// Mod slug or id
        mod_name: String,
        /// Filter by mod loader
        loader: Loaders,
        /// Filter by game version (e.g. 1.21.4)
        game_version: Option<String>,
    },
    /// Download mod given a loader and game version
    Download {
        /// Mod slug or id
        mod_name: String,
        /// Filter by mod loader
        loader: Loaders,
        /// Filter by game version (e.g. 1.21.4)
        game_version: String,
        /// Download latest mod version (skip mod version selection)
        #[arg(short, long)]
        latest: bool,
    },
    /// Operate on a mod pack specified in 'mods.yaml'
    Pack {
        #[command(subcommand)]
        command: PackCommand,
    },
}

#[derive(Subcommand, Clone)]
pub enum PackCommand {
    /// Download the latest version of all mods in pack
    Download,
    /// Update mods to their latest versions
    Update,
    /// Check for compatible game versions and update all mods to selected version
    Upgrade,
    /// Create modpack definition
    Init {
        loader: Loaders,
        game_version: String,
    },
    /// Add mod to modpack
    Add { mod_name: String },
    /// Remove mod from modpack
    Remove { mod_name: String },
    /// List mods in modpack
    List,
    /// List the latest game version for all mods in pack
    LatestGameVersion,
}
