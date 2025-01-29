use std::{
    fmt::{Debug, Formatter},
    path::Path,
};

use modrinth::Loaders;
use serde::{Deserialize, Serialize};

pub enum Error {
    Reqwest(reqwest::Error),
    NotFound,
    StatusCode(reqwest::StatusCode),
    NoVersionsFound,
    InvalidIndex,
    NoFilesFound,
    Io(std::io::Error),
    Yaml(serde_yaml::Error),
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
        }
    }
}

mod modrinth;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    loader: Loaders,
    version: String,
    mods: Vec<String>,
}

impl Config {
    pub fn try_load<P>(file_path: P) -> Result<Config, Error>
    where
        P: AsRef<Path>,
    {
        let file = std::fs::File::open(file_path)?;
        Ok(serde_yaml::from_reader(file)?)
    }
}
