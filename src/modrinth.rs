use std::fmt::{Display, Formatter};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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

#[derive(Deserialize, Hash, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VersionType {
    Release,
    Snapshot,
    Alpha,
    Beta,
}

#[derive(Deserialize, Clone)]
pub struct GameVersion {
    pub version: String,
    pub version_type: VersionType,
    #[serde(with = "time::serde::iso8601")]
    pub date: OffsetDateTime,
    pub major: bool,
}

impl PartialEq for GameVersion {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
    }
}

impl std::hash::Hash for GameVersion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.version.hash(state);
    }
}

impl Eq for GameVersion {}

impl Display for GameVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version)
    }
}

#[derive(Deserialize)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Deserialize)]
pub struct Hit {
    pub title: String,
    pub description: String,
    pub slug: String,
    pub project_id: String,
    pub author: String,
    pub display_categories: Vec<String>,
    pub versions: Vec<String>,
    pub follows: u32,
    pub date_created: String,
    pub date_modified: String,
    pub latest_version: String,
    pub license: String,
    pub gallery: Vec<String>,
    pub featured_gallery: Option<String>,
}
