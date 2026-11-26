<div align="center">

# PilotLauncher

一个液态玻璃风格的 Minecraft Java 版启动器

简体中文 | [English](README_EN.md)

[![Build](https://github.com/tuxi000/PilotLauncher/actions/workflows/build.yml/badge.svg)](https://github.com/tuxi000/PilotLauncher/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/tuxi000/PilotLauncher?include_prereleases)](https://github.com/tuxi000/PilotLauncher/releases)
[![License](https://img.shields.io/github/license/tuxi000/PilotLauncher)](LICENSE)

</div>

<!-- 截图：把截图放进 docs/images/ 后取消下面这行的注释 -->
<!-- ![PilotLauncher](docs/images/home.png) -->

## 简介

PilotLauncher 是一个用 Tauri（Rust + Vue）写的 Minecraft 启动器，界面参考了 iOS 26 的液态玻璃风格，布局参考 LauncherX。

装游戏、配 Java、装模组加载器、找模组和整合包、和朋友联机，这些常用的事情都能在启动器里直接完成，不用另外去网站下载、手动放文件。

目前只支持 Windows。

## 功能

**游戏与 Java**
- 安装正式版、快照、远古版本和愚人节版本
- 根据游戏版本自动下载对应的 Java（Mojang 官方运行时），也可以指定本机已有的 Java
- 每个实例的模组、存档、配置互相独立（版本隔离）
- 下载源可选官方或 BMCLAPI，也可以设成官方优先、失败时自动换镜像

**模组加载器**
- 支持 Forge、NeoForge、Fabric、Quilt、OptiFine
- 新建实例时直接选，已有的原版实例也能补装
- 装 Fabric 时默认一起装好 Fabric API，Quilt 对应装 QFAPI

**社区资源**
- 从 Modrinth 和 CurseForge 搜索、下载模组、整合包、资源包、光影包、数据包和地图
- 按实例的游戏版本和加载器自动筛选能用的文件，前置模组会列出来一起装
- 数据包装进你选的存档，地图自动解压到 saves
- 支持 MCIM 镜像，国内下载更快
- 整合包支持 Modrinth（.mrpack）和 CurseForge 两种格式，也能导入本地文件
- 实例详情里可以看已装的模组，启用、禁用或删除

**账户**
- 离线登录
- 外置登录（authlib-injector），可以用 LittleSkin 等皮肤站账户

**联机**
- 内置[陶瓦联机](https://github.com/burningtnt/Terracotta)：房主开局域网，把邀请码发给朋友就能加入
- 邀请码和 HMCL、PCL CE 的陶瓦联机通用

**界面**
- 液态玻璃效果，深色、浅色跟随系统
- 三套背景配色可选，也可以换成自己的图片
- 游戏内存设置旁边实时显示整台电脑的内存占用
- 游戏日志实时查看，崩溃时给出常见原因的提示

**便携**
- 游戏文件和启动器数据默认都放在程序旁边的 `.minecraft` 文件夹里，整个文件夹拷走就能用

## 下载

| 下载渠道 | 说明 |
| --- | --- |
| [GitHub Releases](https://github.com/tuxi000/PilotLauncher/releases/latest) | 官方发布页 |
| [Gitee 发行版](https://gitee.com/tuxi000/pilot-launcher/releases) | 国内镜像，访问 GitHub 慢时用这个，每次发布自动同步 |

每个版本里有两个文件：

- `pilot-launcher.exe`：免安装版，放进一个空文件夹直接运行
- `PilotLauncher_x.y.z_x64-setup.exe`：安装版

**系统要求**：Windows 11 x64。Windows 10 理论上可以运行，但没有测试过。程序依赖 WebView2，Windows 11 已经自带。

> 程序没有代码签名，第一次运行时 Windows 可能提示"已保护你的电脑"，点"更多信息"→"仍要运行"即可。

## 上手

1. 把 `pilot-launcher.exe` 放进一个新建的空文件夹，路径最好不要带中文
2. 打开后点左下角的头像，添加一个离线账户或外置登录账户
3. 进入"下载"→"游戏版本"，选一个版本安装，需要模组的话在弹窗里选好加载器
4. 回到首页点"启动游戏"。第一次启动会自动下载 Java，稍等一会儿

联机：左侧"联机"页里先安装联机组件，房主选"创建房间"，房客粘贴邀请码选"加入房间"。

## 常见问题

**CurseForge 搜不到东西**

自己编译的版本需要配置 CurseForge API Key，见下面的"从源码构建"。Releases 里下载的版本已经内置了 Key。没有 Key 时只能用 Modrinth。

**下载很慢或经常失败**

在"设置"→"下载"里把下载源换成 BMCLAPI，社区资源源换成"MCIM 优先"。

**联机时 Windows 弹出防火墙提示**

点"允许"。陶瓦联机需要监听网络端口，不需要管理员权限，也不会安装虚拟网卡。

**联机时有人被挤下线**

离线账户的身份由用户名决定，房间里不能有两个同名玩家，换个名字就好。

**很老的版本（1.5 以前）启动不了**

如果游戏目录或 Windows 用户名里有中文，部分远古版本加载不了本地库文件。在设置里把游戏目录改到纯英文路径再试。

**GitHub 打不开或下载很慢**

改用 [Gitee 发行版](https://gitee.com/tuxi000/pilot-launcher/releases) 下载，文件和 GitHub 上的一样。

**数据存在哪里**

默认在 `pilot-launcher.exe` 旁边的 `.minecraft` 文件夹里。启动器自己的设置、账户和下载的 Java 在 `.minecraft\PilotLauncher\`。如果程序所在文件夹没有写入权限（比如装在 `C:\Program Files`），会改用 `%APPDATA%\dev.pilotlauncher.launcher\`。

## 从源码构建

需要：

- Rust（stable，`x86_64-pc-windows-msvc` 工具链）
- Node.js LTS
- Visual Studio 2022 生成工具（"使用 C++ 的桌面开发"和 Windows 11 SDK）

这些都可以用 `scripts\setup-env.bat` 一键安装（通过 winget）。

```powershell
git clone https://github.com/tuxi000/PilotLauncher.git
cd PilotLauncher
npm install

# 开发模式（热更新）
npm run tauri dev

# 打包，产物在 release\ 里
scripts\build.bat
```

`build.bat` 可以加参数：`-NpmMirror` 用国内 npm 镜像，`-Bundle nsis` 生成安装包，`-Debug` 生成调试版本。

**CurseForge API Key**：复制 `config.local.example.json` 为 `config.local.json`，填入你自己的 Key（在 [CurseForge for Studios](https://console.curseforge.com/) 申请）。这个文件不会被提交到 Git。

项目结构、下载源规则、加载器安装流程等实现细节见 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)。

## 参与开发

欢迎提 [Issue](https://github.com/tuxi000/PilotLauncher/issues) 反馈问题或建议，也欢迎提交 Pull Request。

反馈问题时请附上：启动器版本、游戏版本和加载器、问题的复现步骤。游戏崩溃的话，把日志窗口里的内容或 `crash-reports` 里的崩溃报告一起贴上来。

## 许可证

源代码以 [MIT 许可证](LICENSE) 发布。

`src/assets/icons/` 里的方块图标是用 Minecraft 原版贴图渲染的，版权归 Mojang Studios 所有，不适用 MIT 许可。其他第三方素材、运行时下载的程序和使用的在线服务见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

本项目不是 Minecraft 官方产品，未经 Mojang 或 Microsoft 批准，也与它们没有关联。

## 致谢

- [PCL](https://github.com/Meloong-Git/PCL) 和 [HMCL](https://github.com/HMCL-dev/HMCL)：很多功能和交互的思路来自这两个启动器
- [BMCLAPI](https://bmclapidoc.bangbang93.com/)：游戏文件、Java 和加载器的下载镜像
- [MCIM](https://www.mcimirror.top/)：Modrinth 和 CurseForge 的镜像
- [Modrinth](https://modrinth.com/)、[CurseForge](https://www.curseforge.com/)：社区资源
- [Terracotta | 陶瓦联机](https://github.com/burningtnt/Terracotta) 和 [EasyTier](https://github.com/EasyTier/EasyTier)：联机功能
- [authlib-injector](https://github.com/yushijinhun/authlib-injector)：外置登录
- [Tauri](https://tauri.app/)、[Vue](https://vuejs.org/)
