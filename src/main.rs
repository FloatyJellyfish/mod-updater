use clap::Parser;
use mod_updater::modrinth::{GameVersion, Loaders, SearchResult, Version, VersionType};
use mod_updater::{Config, Error, InstalledMod, ModManifest, Cli, Commands, PackCommand};
use reqwest::{Client, ClientBuilder};
use std::collections::{HashMap, HashSet};
use std::io::stdin;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{copy, create_dir, read_dir, remove_file, try_exists};
use tokio::io::{stdout, AsyncWriteExt};
use tokio::task::{spawn_blocking, JoinSet};

static APP_USER_AGENT: &str = concat!(
    "FloatyJellyfish",
    "/",
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
);

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
        Commands::Pack { command } => {
            let manifest = ModManifest::try_load().await?;
            match command {
                PackCommand::Download => {
                    download_mods(client.clone(), Config::try_load().await?, manifest).await?;
                }
                PackCommand::Update => {
                    update_mods(client.clone(), Config::try_load().await?).await?;
                }
                PackCommand::Upgrade => {
                    upgrade_mods(client.clone(), Config::try_load().await?, manifest).await?;
                }
                PackCommand::Init {
                    loader,
                    game_version,
                } => {
                    pack_init(loader, game_version).await?;
                }
                PackCommand::Add { mod_name } => {
                    add_mod(
                        client.clone(),
                        Config::try_load().await?,
                        manifest,
                        mod_name,
                    )
                    .await?;
                }
                PackCommand::Remove { mod_name } => {
                    remove_mod(Config::try_load().await?, manifest, mod_name).await?
                }
                PackCommand::List => list_mods(Config::try_load().await?).await?,
                PackCommand::LatestGameVersion => {
                    latest_game_version(client.clone(), Config::try_load().await?).await?
                }
            }
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
) -> Result<(String, InstalledMod), Error> {
    let versions = get_versions(
        client.clone(),
        mod_name.clone(),
        Some(loader),
        Some(game_version),
    )
    .await?;
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
        let buffer: String = spawn_blocking(move || {
            let mut buffer = String::new();

            match stdin().read_line(&mut buffer) {
                Ok(_) => Ok::<std::string::String, Error>(buffer),
                Err(err) => Err(err.into()),
            }
        })
        .await??;

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
            match stdin().read_line(&mut buffer) {
                Ok(_) => Ok::<std::string::String, Error>(buffer),
                Err(err) => Err(err.into()),
            }
        })
        .await??;
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

    Ok((
        mod_name,
        InstalledMod {
            version: version.name.clone(),
            file: file.filename.clone(),
        },
    ))
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
    for (version, count) in version_support.iter() {
        if *count == mods_count {
            compatible_versions.push((*version).clone());
        }
    }
    compatible_versions.sort_by(|a, b| a.date.cmp(&b.date));
    compatible_versions.reverse();
    Ok(compatible_versions)
}

async fn download_mods(
    client: Client,
    config: Config,
    mut manifest: ModManifest,
) -> Result<(), Error> {
    let mut set = JoinSet::new();

    for m in config.mods {
        if !manifest.installed.contains_key(&m) {
            set.spawn(download_mod(
                client.clone(),
                m,
                config.loader.clone(),
                config.version.clone(),
                true,
            ));
        }
    }

    while let Some(res) = set.join_next().await {
        let (name, installed_mod) = res??;
        manifest.installed.insert(name, installed_mod);
    }

    manifest.try_save().await?;

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

async fn upgrade_mods(client: Client, config: Config, manifest: ModManifest) -> Result<(), Error> {
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
        match stdin().read_line(&mut buffer) {
            Ok(_) => Ok::<std::string::String, Error>(buffer),
            Err(err) => Err(err.into()),
        }
    })
    .await??;
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

    for (_name, installed_mod) in manifest.installed.iter() {
        copy(
            ["./", &installed_mod.file].iter().collect::<PathBuf>(),
            ["./old", &installed_mod.file].iter().collect::<PathBuf>(),
        )
        .await?;
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

    let mut new_config = Config {
        version: version.to_string(),
        ..config
    };

    download_mods(client.clone(), new_config.clone(), manifest).await?;

    new_config.try_save().await?;

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

async fn pack_init(loader: Loaders, game_version: String) -> Result<(), Error> {
    let mut config = Config {
        loader,
        version: game_version,
        mods: Vec::new(),
    };
    config.try_save().await?;
    println!("Created pack config 'mods.yaml'");
    Ok(())
}

async fn add_mod(
    client: Client,
    mut config: Config,
    mut manifest: ModManifest,
    mod_name: String,
) -> Result<(), Error> {
    let request = client.get("https://api.modrinth.com/v2/search").query(&[
        ("query", mod_name.as_str()),
        (
            "facets",
            format!(
                "[[\"project_type:mod\"], [\"versions:{}\"], [\"categories:{}\"]]",
                config.version, config.loader
            )
            .as_str(),
        ),
        ("limit", "5"),
    ]);
    let res = request.send().await?;
    let mod_slug = if res.status().is_success() {
        let search_result: SearchResult = res.json().await?;
        if search_result.hits.is_empty() {
            return Err(Error::NotFound);
        } else if search_result.hits.len() == 1 {
            search_result.hits[0].slug.clone()
        } else {
            for (i, hit) in search_result.hits.iter().enumerate() {
                println!("\t{i} - {}: {}", hit.title, hit.description);
            }

            println!("Select mod (0-{}):", search_result.hits.len() - 1);
            let buffer = spawn_blocking(move || {
                let mut buffer = String::new();
                match stdin().read_line(&mut buffer) {
                    Ok(_) => Ok::<std::string::String, Error>(buffer),
                    Err(err) => Err(err.into()),
                }
            })
            .await??;
            let i: usize = if let Ok(i) = buffer.trim().parse() {
                if i >= search_result.hits.len() {
                    return Err(Error::InvalidIndex);
                }
                i
            } else {
                return Err(Error::InvalidIndex);
            };
            search_result.hits[i].slug.clone()
        }
    } else if res.status().as_u16() == 400 {
        println!("Invalid request");
        println!("{}", res.text().await?);
        return Err(Error::InvalidRequest);
    } else {
        return Err(res.status().into());
    };

    if config.mods.contains(&mod_slug) {
        println!("'{mod_slug}' already present in pack");
        return Ok(());
    }

    let (_name, installed_mod) = download_mod(
        client.clone(),
        mod_slug.clone(),
        config.loader.clone(),
        config.version.clone(),
        true,
    )
    .await?;
    config.mods.push(mod_slug.clone());
    config.try_save().await?;
    manifest.installed.insert(mod_slug.clone(), installed_mod);
    manifest.try_save().await?;
    println!("'{mod_slug}' added");
    Ok(())
}

async fn remove_mod(
    mut config: Config,
    mut manifest: ModManifest,
    mod_name: String,
) -> Result<(), Error> {
    if !config.mods.contains(&mod_name) {
        println!("No mod '{mod_name}' in pack");
        return Ok(());
    }

    config.mods.retain(|m| *m != mod_name);

    if let Some(installed_mod) = manifest.installed.get(&mod_name) {
        remove_file(&installed_mod.file).await?;
    }

    manifest.installed.remove(&mod_name);

    config.try_save().await?;
    manifest.try_save().await?;

    println!("Mod '{mod_name}' removed from pack");

    Ok(())
}

async fn list_mods(config: Config) -> Result<(), Error> {
    println!("Mods in pack:");
    for m in config.mods {
        println!("\t{m}");
    }

    Ok(())
}

async fn latest_game_version(client: Client, config: Config) -> Result<(), Error> {
    let game_versions: Vec<String> = get_game_versions(client.clone())
        .await?
        .into_iter()
        .filter(|v| v.version_type == VersionType::Release)
        .map(|v| v.version)
        .collect();

    let game_versions = Arc::new(game_versions);

    let mut set = JoinSet::new();

    for m in config.mods {
        set.spawn(get_latest_mod_game_version(
            client.clone(),
            m.clone(),
            game_versions.clone(),
            config.loader.clone(),
        ));
    }

    set.join_all().await;

    Ok(())
}

async fn get_latest_mod_game_version(
    client: Client,
    mod_name: String,
    game_versions: Arc<Vec<String>>,
    loader: Loaders,
) -> Result<(), Error> {
    let mod_versions = get_versions(client.clone(), mod_name.clone(), Some(loader), None).await?;
    let mut mod_game_versions = HashSet::new();
    for mod_version in mod_versions {
        for game_version in mod_version.game_versions {
            mod_game_versions.insert(game_version);
        }
    }
    let mut mod_game_versions: Vec<String> = mod_game_versions
        .into_iter()
        .filter(|v| game_versions.contains(v))
        .collect();
    mod_game_versions.sort_by(|a, b| {
        game_versions
            .iter()
            .position(|v| v == a)
            .expect("Invalid game version")
            .cmp(
                &game_versions
                    .iter()
                    .position(|v| v == b)
                    .expect("Invalid game version"),
            )
    });
    println!(
        "{} - {}",
        mod_name,
        mod_game_versions
            .first()
            .expect("Mod has no supported game versions")
    );

    Ok(())
}
