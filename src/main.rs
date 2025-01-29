#![allow(unused)]

use clap::{Parser, Subcommand, ValueEnum};
use mod_updater::{Config, Error};
use modrinth::{GameVersion, Loaders, Version};
use reqwest::{get, Client, ClientBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io::{prelude::*, BufReader};
use tokio::task::JoinSet;

mod modrinth;

static APP_USER_AGENT: &str = concat!(
    "FloatyJellyfish",
    "/",
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);

#[derive(Parser)]
#[command(name = "Mod Updater")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Clone)]
enum Commands {
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
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    let client = ClientBuilder::new().user_agent(APP_USER_AGENT).build()?;

    match cli.command {
        Commands::Versions {
            mod_name,
            loader,
            game_version,
        } => {
            list_versions(client.clone(), mod_name, loader, game_version).await?;
        }
        Commands::Latest {
            mod_name,
            loader,
            game_version,
        } => {
            get_latest_version(client.clone(), mod_name, loader, game_version).await?;
        }
        Commands::Download {
            mod_name,
            loader,
            game_version,
            latest,
        } => {
            download_mod(client.clone(), mod_name, loader, game_version, latest).await?;
        }
    }

    Ok(())
}

async fn list_versions(
    client: Client,
    mod_name: String,
    loader: Option<Loaders>,
    game_version: Option<String>,
) -> Result<(), Error> {
    let versions = get_versions(client, mod_name.clone(), loader, game_version).await?;
    println!("Mod versions for '{mod_name}':");
    for version in versions {
        println!(
            "\t{} - {} {}",
            version.name,
            version.game_versions.join(", "),
            version.loaders.join(", ")
        );
    }

    Ok(())
}

async fn get_latest_version(
    client: Client,
    mod_name: String,
    loader: Loaders,
    game_version: Option<String>,
) -> Result<(), Error> {
    let versions = get_versions(client, mod_name.clone(), Some(loader), game_version).await?;
    let latest = versions.first();
    println!("Latest version for mod '{}':", mod_name.clone());
    if let Some(latest) = latest {
        println!("\t{} - {}", latest.name, latest.game_versions.join(", "));
    } else {
        println!("No versions found for mod '{mod_name}'");
    }

    Ok(())
}

async fn download_mod(
    client: Client,
    mod_name: String,
    loader: Loaders,
    game_version: String,
    latest: bool,
) -> Result<(), Error> {
    let versions = get_versions(client.clone(), mod_name, Some(loader), Some(game_version)).await?;
    if versions.is_empty() {
        return Err(Error::NoVersionsFound);
    }

    let stdin = std::io::stdin();
    let version = if versions.len() == 1 || latest {
        &versions[0]
    } else {
        let mut buffer = String::new();

        println!("Available versions:");
        for (i, version) in versions.iter().enumerate() {
            println!("\t{i} - {}", version.name);
        }
        println!("Select version (0-{}):", versions.len() - 1);
        stdin.read_line(&mut buffer);
        let version_i: usize = if let Ok(version_i) = buffer.trim().parse() {
            if version_i >= versions.len() {
                return Err(Error::InvalidIndex);
            }
            version_i
        } else {
            return Err(Error::InvalidIndex);
        };

        &versions[version_i]
    };

    let files = &version.files;

    if files.is_empty() {
        return Err(Error::NoFilesFound);
    }

    let file = if files.len() == 1 {
        &files[0]
    } else {
        println!("Available files:");
        for (i, file) in version.files.iter().enumerate() {
            println!("\t{i} - {}", file.filename);
        }

        println!("Select file (0-{}):", version.files.len() - 1);
        let mut buffer = String::new();
        stdin.read_line(&mut buffer);
        let file_i: usize = if let Ok(file_i) = buffer.trim().parse() {
            if file_i >= versions.len() {
                return Err(Error::InvalidIndex);
            }
            file_i
        } else {
            return Err(Error::InvalidIndex);
        };

        &files[file_i]
    };

    let request = client.get(file.url.clone());

    print!("Downloading '{}'...", file.filename);
    std::io::stdout().flush();
    let res = request.send().await?;

    let bytes = res.bytes().await?;
    println!("Done");

    print!("Writing file '{}'...", file.filename);
    std::io::stdout().flush();
    let mut file = std::fs::File::create(file.filename.clone())?;

    file.write_all(&bytes)?;
    println!("Done");

    Ok(())
}

async fn get_versions(
    client: Client,
    mod_name: String,
    loader: Option<Loaders>,
    game_version: Option<String>,
) -> Result<Vec<Version>, Error> {
    let request = client.get(format!(
        "https://api.modrinth.com/v2/project/{mod_name}/version"
    ));
    let request = if let Some(loader) = loader {
        request.query(&[("loaders", format!("[\"{loader}\"]"))])
    } else {
        request
    };
    let request = if let Some(game_version) = game_version {
        request.query(&[("game_versions", format!("[\"{game_version}\"]"))])
    } else {
        request
    };
    let res = request.send().await?;
    if res.status().is_success() {
        Ok(res.json().await?)
    } else if res.status().as_u16() == 404 {
        Err(Error::NotFound)
    } else {
        Err(res.status().into())
    }
}

async fn compatible_versions(
    client: Client,
    mods: Vec<String>,
    loader: Loaders,
) -> Result<Vec<GameVersion>, Error> {
    let mut set = JoinSet::new();

    for m in mods.iter() {
        set.spawn(get_versions(
            client.clone(),
            m.clone(),
            Some(loader.clone()),
            None,
        ));
    }

    let mut mods_supported_versions = Vec::new();
    while let Some(res) = set.join_next().await {
        mods_supported_versions.push(res??);
    }

    let mut version_support = HashMap::new();
    for mod_supported_versions in mods_supported_versions {
        let mut max_version = "0.0.0".to_string().into();
        let mut game_versions = Vec::new();
        for version in mod_supported_versions {
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
    }

    let mut compatible_versions = Vec::new();
    let mods_count = mods.len();
    for (i, (version, count)) in version_support.iter().enumerate() {
        if *count == mods_count {
            compatible_versions.push(version.clone());
        }
    }
    compatible_versions.sort();
    compatible_versions.reverse();
    Ok(compatible_versions)
}

async fn latest_compatible_version(
    client: Client,
    mods: Vec<String>,
    loader: Loaders,
) -> Result<GameVersion, Error> {
    let compatible_versions = compatible_versions(client, mods, loader).await?;
    Ok(compatible_versions[0].clone())
}
