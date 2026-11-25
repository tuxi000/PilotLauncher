# 第三方素材与服务声明

PilotLauncher 的**源代码**以 [MIT 许可证](LICENSE) 发布。以下素材和服务**不属于** MIT 许可的范围，版权归各自所有者。

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.
> 本项目不是 Minecraft 官方产品，未经 Mojang 或 Microsoft 批准，也与它们没有关联。

## 仓库中包含的素材（`src/assets/icons/`）

| 文件 | 内容 | 版权 / 许可 |
| --- | --- | --- |
| `grass.png`、`command_block.png`、`cobblestone.png`、`gold_block.png`、`anvil.png`、`optifine.png`、`stone.png`、`diamond.png`、`redstone.png`、`amethyst.png` | 用 Minecraft 原版 16×16 方块贴图渲染的等距方块图标 | 贴图 © Mojang Studios / Microsoft。仅用作启动器界面图标，本项目不对其主张任何权利，**不适用 MIT 许可**。如权利人要求，将立即移除。 |
| `fabric.png` | FabricMC 的项目图标 | 归 FabricMC 项目所有 |
| `neoforge.png` | NeoForged 的项目 logo（去掉底板） | 归 NeoForged 项目所有 |
| `quilt.png` | QuiltMC 官方 logo（[QuiltMC/art](https://github.com/QuiltMC/art)） | CC0 1.0 |

“Minecraft” 是 Mojang Synergies AB 的商标；Forge、NeoForge、Fabric、Quilt、OptiFine 等名称归各自项目所有，在本项目中仅用于说明兼容性。

## 运行时下载、不随仓库分发的程序

| 程序 | 用途 | 许可 |
| --- | --- | --- |
| [Terracotta \| 陶瓦联机](https://github.com/burningtnt/Terracotta)（基于 [EasyTier](https://github.com/EasyTier/EasyTier)） | 联机功能。启动器下载**未修改**的官方二进制并校验哈希，只通过它的本地 HTTP 接口交互，并在界面中标注版权 | AGPL-3.0-or-later（附例外条款） |
| [authlib-injector](https://github.com/yushijinhun/authlib-injector) | 外置登录 | AGPL-3.0 |
| Mojang 官方 Java 运行时、Minecraft 游戏文件 | 运行游戏 | 归 Mojang / Microsoft 所有，按需从官方或镜像下载 |

## 使用的在线服务

- [BMCLAPI](https://bmclapidoc.bangbang93.com/)（bangbang93）：游戏文件、Java、加载器镜像
- [MCIM](https://www.mcimirror.top/)：Modrinth / CurseForge 镜像
- [Modrinth](https://modrinth.com/)、[CurseForge](https://www.curseforge.com/)：社区资源

## 设计参考

设计思路参考了 [PCL](https://github.com/Meloong-Git/PCL)（龙腾猫跃）、[HMCL](https://github.com/HMCL-dev/HMCL) 与 LauncherX，没有复制它们的代码或素材。

## 依赖库

前端（npm）与后端（Rust crates）依赖各自遵循其许可证，详见 `package-lock.json` 与 `src-tauri/Cargo.lock` 中列出的包。
