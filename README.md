# Modrinth Mod Updater

A CLI tool to manage Minecraft mods downloaded from Modrinth.

# Quick Start

```
$ mod-updater pack init fabric 1.21.4
$ mod-updater pack add "sodium"
```

# Usage

Loaders: `fabric`, `forge`, `neo-forge`, `quilt` or `lite-loader`

Version (examples): `1.21`, `1.21.4`

## Pack Commands

### Init

Initialize pack in current folder.

**Usage**: `mod-updater pack init <LOADER> <GAME_VERSION>`

Example: `mod-updater pack init fabric 1.21.4`

### Add

Add mod to modpack and download it. You will be prompted to select an option if no exact match is found.

**Usage**: `mod-updater pack add <MOD_NAME>`

Example: `mod-updater pack add "sodium"`

### Update

Download updates to mods if available.

**Usage**: `mod-updater pack update`

### Upgrade

Check what the latest compatible game version is for the mods currently in the pack. If there is a version higher than the current game version, it will prompt you to upgrade all mods to the selected game version.

> [!WARNING]
> This does not change the minecraft version the game uses. You will have to change this in your launcher.

**Usage**: `mod-updater pack upgrade`

### Download

Download all the mods in the pack (if they aren't already present). Useful if you delete a mod file, or when copying `mods.yaml`.

**Usage**: `mod-updater pack upgrade`

### Remove

Remove mod from modpack.

**Usage**: `mod-updater pack remove <MOD_NAME>`

Example `mod-updater pack remove "sodium"`

## Other Commands

These commands don't operate on a pack. They require the slug or id of the mod on Modrinth (e.g. the slug for https://modrinth.com/mod/sodium is `sodium`).

### Versions

List all game versions supported by a mod.

**Usage**: `mod-updater versions [OPTIONS] <MOD_NAME>`

Options:

- `--loader <LOADER>`
- `--game-version <GAME_VERSION>`

Example: `mod-updater versions --loader fabric --game-version 1.21 sodium`

### Latest

Get latest version of a mod for a given mod loader.

**Usage**: `mod-updater latest <MOD_NAME> <LOADER> [GAME_VERSION]`

Example: `mod-updater latest sodium fabric 1.21`

### Download

Download mod given a loader and game version

**Usage**: `mod-updater download [OPTIONS] <MOD_NAME> <LOADER> <GAME_VERSION>`

Options:

- `--latest` - Download latest mod version (skip mod version selection)

Example: `mod-updater download sodium fabric 1.21`
