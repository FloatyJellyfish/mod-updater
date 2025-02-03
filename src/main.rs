#![allow(unused)]

use clap::{Parser, Subcommand, ValueEnum};
use mod_updater::modrinth::{GameVersion, Loaders, Version};
use mod_updater::{Config, Error};
use reqwest::{get, Client, ClientBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::fs::FileType;
use std::io::{prelude::*, stdin};
use std::ops::Deref;
use tokio::fs::{copy, create_dir, read_dir, remove_file, try_exists, write, File};
use tokio::io::{stdout, AsyncWriteExt};
use tokio::task::{spawn_blocking, JoinSet};

// pub use mod_updater::Loaders;

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
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
enum PackCommand {
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
        Commands::Pack { command, path } => match command {
            PackCommand::Download => {
                let config = if let Some(path) = path {
                    Config::try_load(path).await
                } else {
                    Config::try_load("mods.yaml").await
                }?;
                download_mods(client.clone(), config).await?;
            }
            PackCommand::Update => {
                let config = if let Some(path) = path {
                    Config::try_load(path).await
                } else {
                    Config::try_load("mods.yaml").await
                }?;
                update_mods(client.clone(), config).await?;
            }
            PackCommand::Upgrade => {
                let config = if let Some(path) = path {
                    Config::try_load(path).await
                } else {
                    Config::try_load("mods.yaml").await
                }?;
                upgrade_mods(client.clone(), config).await?;
            }
            PackCommand::Init {
                loader,
                game_version,
            } => {
                pack_init(client.clone(), loader, game_version).await?;
            }
        },
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

    let version = if versions.len() == 1 || latest {
        &versions[0]
    } else {
        println!("Available versions:");
        for (i, version) in versions.iter().enumerate() {
            println!("\t{i} - {}", version.name);
        }
        println!("Select version (0-{}):", versions.len() - 1);
        let buffer = spawn_blocking(move || {
            let mut buffer = String::new();
            stdin().read_line(&mut buffer);
            buffer
        })
        .await?;

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
        let buffer = spawn_blocking(move || {
            let mut buffer = String::new();
            stdin().read_line(&mut buffer);
            buffer
        })
        .await?;
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

    download_file(client.clone(), file.url.clone(), file.filename.clone()).await?;

    Ok(())
}

async fn download_file(client: Client, url: String, path: String) -> Result<(), Error> {
    let request = client.get(url);

    println!("Downloading '{}'...", path);
    stdout().flush().await?;
    let res = request.send().await?;

    let bytes = res.bytes().await?;

    stdout().flush().await?;
    let mut file = tokio::fs::File::create(path.clone()).await?;

    file.write_all(&bytes).await?;
    println!("Wrote file '{}'...", path);

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
    let game_versions = get_game_versions(client.clone()).await?;

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
        let mut versions = Vec::new();
        for version in mod_supported_versions {
            for game_version in version.game_versions {
                let game_version = game_versions
                    .iter()
                    .find(|x| x.version == game_version)
                    .expect("Invalid game version");
                if !versions.contains(game_version) {
                    versions.push(game_version.clone());
                }
            }
        }
        for version in versions {
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
            compatible_versions.push((*version).clone());
        }
    }
    compatible_versions.sort_by(|a, b| a.date.cmp(&b.date));
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

async fn download_mods(client: Client, config: Config) -> Result<(), Error> {
    let mut set = JoinSet::new();

    for m in config.mods {
        set.spawn(download_mod(
            client.clone(),
            m,
            config.loader.clone(),
            config.version.clone(),
            true,
        ));
    }

    while let Some(res) = set.join_next().await {
        res??;
    }

    Ok(())
}

async fn update_mods(client: Client, config: Config) -> Result<(), Error> {
    let mut set = JoinSet::new();

    for m in config.mods {
        set.spawn(update_mod(
            client.clone(),
            m.clone(),
            config.loader.clone(),
            config.version.clone(),
        ));
    }

    let mut updates = Vec::new();
    while let Some(res) = set.join_next().await {
        updates.push(res??);
    }

    println!("The following updates have been completed:");
    for update in updates {
        println!("\t{update}");
    }
    Ok(())
}

async fn update_mod(
    client: Client,
    mod_name: String,
    loader: Loaders,
    game_version: String,
) -> Result<String, Error> {
    let mut entries = read_dir("./").await?;
    let versions = get_versions(
        client.clone(),
        mod_name.clone(),
        Some(loader),
        Some(game_version),
    )
    .await?;

    let mut up_to_date = false;
    let mut exsiting = Vec::new();
    let latest_file = &versions[0].files[0];
    while let Some(entry) = entries.next_entry().await? {
        if *entry.file_name() == *latest_file.filename {
            return Ok(format!("'{mod_name}' is already up to date"));
        }

        for version in &versions[1..] {
            if *entry.file_name() == *version.files[0].filename {
                exsiting.push(version.files[0].filename.clone());
            }
        }
    }

    for file in exsiting {
        println!("Removing {file}");
        remove_file(file).await?;
    }

    download_file(
        client.clone(),
        latest_file.url.clone(),
        latest_file.filename.clone(),
    )
    .await?;

    Ok(format!("Updated '{mod_name}' to '{}'", versions[0].name))
}

async fn upgrade_mods(client: Client, config: Config) -> Result<(), Error> {
    let game_versions = get_game_versions(client.clone()).await?;

    let current_version = config.version;
    let current_version_index = game_versions
        .iter()
        .position(|x| x.version == current_version)
        .expect("Invalid game version");
    let compatible_versions =
        compatible_versions(client.clone(), config.mods.clone(), config.loader.clone()).await?;

    let compatible_versions: Vec<GameVersion> = compatible_versions
        .into_iter()
        .filter(|ver| {
            game_versions
                .iter()
                .position(|x| x == ver)
                .expect("Invalid game version")
                < current_version_index
        })
        .collect();

    if compatible_versions.is_empty() {
        println!("No compatible versions available to upgrade to");
        return Ok(());
    }

    println!("Compatible game versions:");
    for (i, version) in compatible_versions.iter().enumerate() {
        println!("\t{i} - {version}");
    }

    println!("Select game version (0-{}):", compatible_versions.len() - 1);
    let buffer = spawn_blocking(move || {
        let mut buffer = String::new();
        stdin().read_line(&mut buffer);
        buffer
    })
    .await?;
    let i: usize = if let Ok(i) = buffer.trim().parse() {
        if i >= compatible_versions.len() {
            return Err(Error::InvalidIndex);
        }
        i
    } else {
        return Err(Error::InvalidIndex);
    };

    let version = &compatible_versions[i];

    // Move all .jar files to 'old' directory
    if !try_exists("./old/").await? {
        create_dir("./old/").await?;
    }
    let mut dir = read_dir("./").await?;
    while let Some(entry) = dir.next_entry().await? {
        if entry.file_type().await?.is_file()
            && entry.file_name().into_string().unwrap().ends_with(".jar")
        {
            copy(
                entry.path(),
                format!("./old/{}", entry.file_name().into_string().unwrap()),
            )
            .await?;
            remove_file(entry.path()).await?;
        }
    }

    let new_config = Config {
        loader: config.loader,
        version: version.to_string(),
        mods: config.mods,
    };

    download_mods(client.clone(), new_config.clone()).await?;

    let config_contents = serde_yaml::to_string(&new_config)?;
    write("mods.yaml", config_contents).await?;

    Ok(())
}

async fn get_game_versions(client: Client) -> Result<Vec<GameVersion>, Error> {
    let request = client.get("https://api.modrinth.com/v2/tag/game_version");
    let res = request.send().await?;

    if res.status().is_success() {
        Ok(res.json().await?)
    } else {
        Err(Error::StatusCode(res.status()))
    }
}

async fn pack_init(client: Client, loader: Loaders, game_version: String) -> Result<(), Error> {
    let config = Config {
        loader,
        version: game_version,
        mods: Vec::new(),
    };
    let contents = serde_yaml::to_string(&config)?;
    let mut file = File::create("mods.yaml").await?;
    file.write_all(contents.as_bytes()).await?;
    println!("Created pack config 'mods.yaml'");
    Ok(())
}
