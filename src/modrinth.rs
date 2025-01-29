use std::fmt::{Display, Formatter};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Version {
    pub name: String,
    pub version_number: String,
    pub changelog: Option<String>,
    pub dependencies: Vec<Dependency>,
    pub game_versions: Vec<String>,
    pub version_type: String,
    pub loaders: Vec<String>,
    pub featured: bool,
    pub status: String,
    pub requested_status: Option<String>,
    pub id: String,
    pub project_id: String,
    pub author_id: String,
    pub date_published: String,
    pub downloads: i32,
    pub changelog_url: Option<String>,
    pub files: Vec<File>,
}

#[derive(Debug, Deserialize)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Deserialize)]
pub struct File {
    pub hashes: Hash,
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: i32,
    pub file_type: Option<String>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameVersion {
    pub major: usize,
    pub minor: usize,
    pub patch: usize,
}

impl From<String> for GameVersion {
    fn from(value: String) -> Self {
        let parts: Vec<&str> = value.split(".").collect();
        let mut major = 0;
        let mut minor = 0;
        let mut patch = 0;

        if parts.len() >= 1 {
            major = parts[0].parse::<usize>().unwrap();
        }

        if parts.len() >= 2 {
            minor = parts[1].parse::<usize>().unwrap();
        }

        if parts.len() >= 3 {
            patch = parts[2].parse::<usize>().unwrap();
        }

        Self {
            major,
            minor,
            patch,
        }
    }
}

impl Display for GameVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        if self.patch != 0 {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        } else if self.minor != 0 {
            write!(f, "{}.{}", self.major, self.minor)
        } else {
            write!(f, "{}", self.major)
        }
    }
}

impl std::fmt::Debug for GameVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        std::fmt::Display::fmt(&self, f)
    }
}

#[derive(Debug, Deserialize)]
pub struct Hash {
    pub sha512: String,
    pub sha1: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Loaders {
    Fabric,
    Forge,
    NeoForge,
    Quilt,
    LiteLoader,
}

impl Display for Loaders {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Loaders::Fabric => "fabric",
            Loaders::Forge => "forge",
            Loaders::NeoForge => "neoforge",
            Loaders::Quilt => "quilt",
            Loaders::LiteLoader => "liteloader",
        };
        write!(f, "{str}")
    }
}
