<div align="center">

# PilotLauncher

A clean, modern launcher for Minecraft: Java Edition

[简体中文](README.md) | English

[![Build](https://github.com/tuxi000/PilotLauncher/actions/workflows/build.yml/badge.svg)](https://github.com/tuxi000/PilotLauncher/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/tuxi000/PilotLauncher?include_prereleases)](https://github.com/tuxi000/PilotLauncher/releases)
[![License](https://img.shields.io/github/license/tuxi000/PilotLauncher)](LICENSE)

</div>

<!-- Screenshot: put screenshots in docs/images/ and uncomment the line below -->
<!-- ![PilotLauncher](docs/images/home.png) -->

## About

PilotLauncher is a Minecraft launcher built with Tauri (Rust + Vue), with a clean interface and a translucent, glassy look.

Installing the game, setting up Java, adding a mod loader, finding mods and modpacks, playing with friends: you can do all of it from inside the launcher, without hunting for downloads or copying files around by hand.

Windows only for now.

> The launcher UI is currently in Simplified Chinese only.

## Features

**Game and Java**
- Install releases, snapshots, old alpha/beta versions and April Fools versions
- Downloads the right Java for each game version (Mojang's official runtimes), or use a Java you already have
- Every instance has its own mods, saves and config (version isolation)
- Download from the official servers or the BMCLAPI mirror, or try official first and fall back to the mirror automatically

**Mod loaders**
- Forge, NeoForge, Fabric, Quilt and OptiFine
- Pick one when creating an instance, or add one to an existing vanilla instance
- Fabric API is installed along with Fabric by default (QFAPI for Quilt)

**Community content**
- Search and download mods, modpacks, resource packs, shader packs, data packs and worlds from Modrinth and CurseForge
- Files are filtered by the instance's game version and loader, and required dependencies are listed and installed together
- Data packs go into the world you pick, and worlds are extracted into `saves`
- Optional MCIM mirror for faster downloads in mainland China
- Modpacks in Modrinth (`.mrpack`) and CurseForge format, online or from a local file
- See, enable, disable or remove installed mods from the instance page

**Accounts**
- Offline accounts
- authlib-injector accounts (LittleSkin and other skin sites)

**Multiplayer**
- Built-in [Terracotta](https://github.com/burningtnt/Terracotta): the host opens the world to LAN and shares an invite code, friends paste it to join
- Invite codes work across PilotLauncher, HMCL and PCL CE

**Interface**
- Translucent glass-style interface, light and dark themes that follow the system
- Three background color schemes, or use your own image
- A live view of your PC's memory usage next to the game memory setting
- Live game log, with hints for common crash causes

**Portable**
- Game files and launcher data live in a `.minecraft` folder next to the program by default, so you can move the whole folder anywhere

## Download

| Source | Notes |
| --- | --- |
| [GitHub Releases](https://github.com/tuxi000/PilotLauncher/releases/latest) | Official release page |
| [Gitee Releases](https://gitee.com/tuxi000/pilot-launcher/releases) | Mirror for mainland China |

Each release has two files:

- `pilot-launcher.exe`: portable, put it in an empty folder and run it
- `PilotLauncher_x.y.z_x64-setup.exe`: installer

**Requirements**: Windows 11 x64. Windows 10 should work but hasn't been tested. WebView2 is required and ships with Windows 11.

> The program isn't code-signed, so Windows SmartScreen may warn you the first time. Click "More info" → "Run anyway".

## Getting started

1. Put `pilot-launcher.exe` in a new, empty folder. A path without non-ASCII characters is safest
2. Click the avatar at the bottom left and add an offline or authlib-injector account
3. Go to "Download" → "Game versions", pick a version and install it. Choose a mod loader in the dialog if you want mods
4. Back on the home page, click the launch button. Java is downloaded on the first launch, so give it a moment

Multiplayer: open the "Multiplayer" page and install the multiplayer component first. The host creates a room; guests paste the invite code and join.

## FAQ

**CurseForge search returns nothing**

If you built the launcher yourself, you need a CurseForge API key (see "Building from source"). Builds from Releases already include one. Without a key, only Modrinth is available.

**Downloads are slow or keep failing**

In Settings → Download, switch the download source to BMCLAPI and the community source to "MCIM first".

**Windows Firewall pops up when I play online**

Click "Allow". Terracotta needs to listen on a network port. It doesn't need admin rights and doesn't install a virtual network adapter.

**Someone keeps getting kicked from the room**

Offline accounts are identified by their username, so two players with the same name will kick each other. Use different names.

**Very old versions (before 1.5) won't start**

If the game folder or your Windows username contains non-ASCII characters (such as Chinese), some old versions can't load their native libraries. Move the game folder to an ASCII-only path in Settings.

**Where is my data stored?**

In the `.minecraft` folder next to `pilot-launcher.exe`. The launcher's own settings, accounts and Java runtimes are in `.minecraft\PilotLauncher\`. If the program's folder isn't writable (for example under `C:\Program Files`), `%APPDATA%\dev.pilotlauncher.launcher\` is used instead.

## Building from source

You need:

- Rust (stable, `x86_64-pc-windows-msvc` toolchain)
- Node.js LTS
- Visual Studio 2022 Build Tools ("Desktop development with C++" and the Windows 11 SDK)

`scripts\setup-env.bat` installs all of them through winget.

```powershell
git clone https://github.com/tuxi000/PilotLauncher.git
cd PilotLauncher
npm install

# development mode with hot reload
npm run tauri dev

# release build, output goes to release\
scripts\build.bat
```

`build.bat` accepts `-NpmMirror` (use a mirror registry in China), `-Bundle nsis` (build the installer) and `-Debug` (debug build).

**CurseForge API key**: copy `config.local.example.json` to `config.local.json` and put your own key in it (get one from [CurseForge for Studios](https://console.curseforge.com/)). This file is ignored by Git.

Implementation notes (project layout, mirror rules, how mod loaders are installed and so on) are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (Chinese).

## Contributing

Bug reports and ideas are welcome in [Issues](https://github.com/tuxi000/PilotLauncher/issues), and so are pull requests.

When reporting a bug, please include the launcher version, the game version and loader, and the steps to reproduce it. For game crashes, attach the log window output or the report from `crash-reports`.

## License

The source code is released under the [MIT License](LICENSE).

The block icons in `src/assets/icons/` are rendered from Minecraft's original textures. They are © Mojang Studios and are not covered by the MIT License. Other third-party assets, programs downloaded at runtime and online services used are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Acknowledgements

- [PCL](https://github.com/Meloong-Git/PCL) and [HMCL](https://github.com/HMCL-dev/HMCL): many of the features and ideas here come from these two launchers
- [BMCLAPI](https://bmclapidoc.bangbang93.com/): mirror for game files, Java and mod loaders
- [MCIM](https://www.mcimirror.top/): mirror for Modrinth and CurseForge
- [Modrinth](https://modrinth.com/) and [CurseForge](https://www.curseforge.com/): community content
- [Terracotta](https://github.com/burningtnt/Terracotta) and [EasyTier](https://github.com/EasyTier/EasyTier): multiplayer
- [authlib-injector](https://github.com/yushijinhun/authlib-injector): third-party login
- [Tauri](https://tauri.app/) and [Vue](https://vuejs.org/)
