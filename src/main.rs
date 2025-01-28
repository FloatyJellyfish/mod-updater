#![allow(unused)]

use clap::{Parser, Subcommand, ValueEnum};
use reqwest::{get, Client, ClientBuilder, StatusCode};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::io::{prelude::*, BufReader};
use tokio;

static APP_USER_AGENT: &str = concat!(
    "FloatyJellyfish",
    "/",
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);

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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        std::fmt::Display::fmt(&self, f)
    }
}

#[derive(Debug, Deserialize)]
struct Hash {
    sha512: String,
    sha1: String,
}

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
    },
}

#[derive(Clone, ValueEnum)]
enum Loaders {
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
            list_versions(&client, mod_name, loader, game_version).await?;
        }
        Commands::Latest {
            mod_name,
            loader,
            game_version,
        } => {
            get_latest_version(&client, mod_name, loader, game_version).await?;
        }
        Commands::Download {
            mod_name,
            loader,
            game_version,
        } => {
            download_mod(&client, mod_name, loader, game_version).await?;
        }
    }

    return Ok(());

    let file = std::fs::File::open("./modlist.txt").unwrap();
    let reader = BufReader::new(file);

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

async fn list_versions(
    client: &Client,
    mod_name: String,
    loader: Option<Loaders>,
    game_version: Option<String>,
) -> Result<(), Error> {
    let versions = get_versions(client, &mod_name, loader, game_version).await?;
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
    client: &Client,
    mod_name: String,
    loader: Loaders,
    game_version: Option<String>,
) -> Result<(), Error> {
    let versions = get_versions(client, &mod_name, Some(loader), game_version).await?;
    let latest = versions.first();
    println!("Latest version for mod '{mod_name}':");
    if let Some(latest) = latest {
        println!("\t{} - {}", latest.name, latest.game_versions.join(", "));
    } else {
        println!("No versions found for mod '{mod_name}'");
    }

    Ok(())
}

async fn download_mod(
    client: &Client,
    mod_name: String,
    loader: Loaders,
    game_version: String,
) -> Result<(), Error> {
    let versions = get_versions(client, &mod_name, Some(loader), Some(game_version)).await?;
    if versions.len() == 0 {
        println!("No versions found");
        return Ok(());
    }

    let mut buffer = String::new();
    let stdin = std::io::stdin();

    println!("Available versions:");
    for (i, version) in versions.iter().enumerate() {
        println!("\t{i} - {}", version.name);
    }
    println!("Select version (0-{}):", versions.len() - 1);
    stdin.read_line(&mut buffer);
    let version_i: usize = if let Ok(version_i) = buffer.trim().parse() {
        if version_i >= versions.len() {
            println!("Invalid index");
            return Ok(());
        }
        version_i
    } else {
        println!("Please enter a number");
        return Ok(());
    };

    let version = &versions[version_i];

    println!("Available files:");
    for (i, file) in version.files.iter().enumerate() {
        println!("\t{i} - {}", file.filename);
    }

    println!("Select file (0-{}):", version.files.len() - 1);
    let mut buffer = String::new();
    stdin.read_line(&mut buffer);
    let file_i: usize = if let Ok(file_i) = buffer.trim().parse() {
        if file_i >= versions.len() {
            println!("Invalid index");
            return Ok(());
        }
        file_i
    } else {
        println!("Please enter a number");
        return Ok(());
    };

    let file = &version.files[file_i];

    let request = client.get(file.url.clone());

    let res = request.send().await?;

    let bytes = res.bytes().await?;

    let file = std::fs::File::create(file.filename.clone());

    let mut file = match file {
        Err(err) => {
            println!("Unable to open file: {:?}", err);
            return Ok(());
        }
        Ok(file) => file,
    };

    if let Err(err) = file.write(&bytes) {
        println!("Unable to write to file: {:?}", err);
    }

    Ok(())
}

enum Error {
    Reqwest(reqwest::Error),
    NotFound,
    StatusCode(StatusCode),
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Reqwest(value)
    }
}

impl From<StatusCode> for Error {
    fn from(value: StatusCode) -> Self {
        Self::StatusCode(value)
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reqwest(arg0) => f.debug_tuple("Reqwest").field(arg0).finish(),
            Self::NotFound => write!(f, "Mod not found"),
            Self::StatusCode(arg0) => f.debug_tuple("StatusCode").field(arg0).finish(),
        }
    }
}

async fn get_versions(
    client: &Client,
    mod_name: &str,
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
    } else {
        if res.status().as_u16() == 404 {
            Err(Error::NotFound)
        } else {
            Err(res.status().into())
        }
    }
}
