#![allow(unused)]

use core::fmt::Error;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::io::{prelude::*, BufReader};
use tokio;

#[derive(Debug, Deserialize)]
struct Version {
    name: String,
    version_number: String,
    changelog: Option<String>,
    dependencies: Vec<Dependency>,
    game_versions: Vec<String>,
    version_type: String,
    loaders: Vec<String>,
    featured: bool,
    status: String,
    requested_status: Option<String>,
    id: String,
    project_id: String,
    author_id: String,
    date_published: String,
    downloads: i32,
    changelog_url: Option<String>,
    files: Vec<File>,
}

#[derive(Debug, Deserialize)]
struct Dependency {
    version_id: Option<String>,
    project_id: Option<String>,
    file_name: Option<String>,
    dependency_type: String,
}

#[derive(Debug, Deserialize)]
struct File {
    hashes: Hash,
    url: String,
    filename: String,
    primary: bool,
    size: i32,
    file_type: Option<String>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct GameVersion {
    major: usize,
    minor: usize,
    patch: usize,
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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        if (self.patch != 0) {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        } else if (self.minor != 0) {
            write!(f, "{}.{}", self.major, self.minor)
        } else {
            write!(f, "{}", self.major)
        }
    }
}

impl std::fmt::Debug for GameVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        std::fmt::Display::fmt(&self, f)
    }
}

#[derive(Debug, Deserialize)]
struct Hash {
    sha512: String,
    sha1: String,
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let file = std::fs::File::open("./modlist.txt").unwrap();
    let reader = BufReader::new(file);

    let client = Client::new();

    let mut version_support = HashMap::new();
    let lines: Vec<std::io::Result<String>> = reader.lines().collect();
    for line in lines.iter() {
        let line = line.as_ref().unwrap();
        let res = client
            .get(format!(
                "https://api.modrinth.com/v2/project/{}/version",
                line
            ))
            .send()
            .await?;
        if res.status().is_success() {
            let versions: Vec<Version> = res.json().await?;

            let mut max_version = "0.0.0".to_string().into();
            let mut game_versions = Vec::new();
            for version in versions {
                for game_version in version.game_versions {
                    let game_version: GameVersion = game_version.into();
                    if !game_versions.contains(&game_version) {
                        game_versions.push(game_version.clone());
                    }
                    if game_version > max_version {
                        max_version = game_version;
                    }
                }
            }

            for version in game_versions {
                version_support
                    .entry(version)
                    .and_modify(|count| *count += 1)
                    .or_insert(1);
            }

            println!("{line} - max version: {max_version}");
        } else {
            println!("Error getting version info for '{line}'");
        }
    }
    let mut compatible_versions = Vec::new();
    for (i, (version, count)) in version_support.iter().enumerate() {
        if *count == lines.len() {
            compatible_versions.push(version);
        }
    }
    compatible_versions.sort();
    compatible_versions.reverse();
    print!("Compatible version: ");
    // print!(
    //     "{}",
    //     compatible_versions
    //         .iter()
    //         .map(|ver| ver.to_string())
    //         .collect::<Vec<String>>()
    //         .join(", ")
    // );
    println!("Max compatible version {}", compatible_versions[0]);
    println!();

    Ok(())
}
