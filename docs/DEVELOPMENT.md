# 开发文档

这里记录 PilotLauncher 的实现细节，给想改代码或排查问题的人看。普通使用请看 [README](../README.md)。

## 技术栈

- 后端：Rust + Tauri 2（tokio、reqwest + rustls、serde、zip、flate2）
- 前端：Vue 3 + TypeScript + Vite
- 平台：Windows x64，界面由 WebView2 渲染

## 构建

环境要求和基本命令见 README。补充几点：

- `scripts\setup-env.ps1` 用 winget 安装 Rustup、Node.js LTS、VS 2022 生成工具（C++ 和 Windows 11 SDK），并检查 WebView2。装完要重新打开终端，新的 PATH 才会生效
- `scripts\build.ps1` 依次执行 `npm install` 和 `npm run tauri -- build`，再把产物复制到 `release\`。默认只编译免安装 exe，不打安装包，因为打包工具 NSIS / WiX 要从 GitHub 下载，国内网络经常超时
- 第一次编译要下载并编译几百个 crate，大约 5 到 15 分钟，之后增量编译会快很多
- `tauri-plugin-dialog` 需要较新的 Rust，编译报错时先 `rustup update`
- Tauri CLI 会检查 Rust 端 `tauri` crate 和 npm 端 `@tauri-apps/api` 的版本是否一致，提示 mismatch 时把两边对齐即可

### GitHub Actions

`.github/workflows/build.yml` 在 `windows-latest` 上构建：

- 推送到 main、PR、手动触发：编译并上传构建产物（保留 14 天）
- 推送 `v*` 标签：额外创建 Release 草稿，附上免安装 exe 和 NSIS 安装包。标签必须和 `tauri.conf.json` 里的版本号一致，否则直接失败

CurseForge Key 来自仓库 Secret `CURSEFORGE_API_KEY`，构建时通过环境变量 `PILOT_CURSEFORGE_API_KEY` 写进程序（`option_env!`）。运行时 `config.local.json` 里的 Key 优先。

发布新版本时，先把 `src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`package.json` 三处的版本号改成一样，再打标签推送。

## 项目结构

```
├─ scripts/                 环境安装、构建脚本
├─ src/                     前端
│  ├─ api.ts                后端命令与事件的类型定义
│  ├─ store.ts              全局状态、事件监听
│  ├─ theme.ts              主题、窗口材质、背景配色
│  ├─ instanceIcons.ts      图标与自动图标规则
│  ├─ aprilFools.ts         愚人节版本识别（与后端一致）
│  ├─ loaderVersions.ts     加载器版本列表的缓存与预取
│  ├─ terracotta.ts         联机状态文案、邀请码校验
│  ├─ glass/refract.ts      液态玻璃的折射与高光指令
│  ├─ styles/               全局样式和玻璃组件样式
│  ├─ components/           通用组件（GlassSelect、GlassDialog、LoaderPicker 等）
│  └─ pages/                首页、下载、实例、联机、设置
└─ src-tauri/src/           后端
   ├─ commands.rs           所有 Tauri 命令
   ├─ state.rs / paths.rs / settings.rs
   ├─ tasks.rs              后台任务与进度事件
   ├─ download/             下载器、下载源替换规则
   ├─ minecraft/            版本清单、版本 JSON、依赖库、资源、安装、愚人节版本
   ├─ java/                 Java 运行时下载、本机 Java 扫描
   ├─ auth/                 离线 UUID、Yggdrasil、authlib-injector
   ├─ instance.rs           实例管理
   ├─ launch/               启动参数、natives、进程与日志、崩溃分析
   ├─ modloader/            Fabric / Quilt、Forge / NeoForge、OptiFine、版本列表缓存
   ├─ community/            Modrinth、CurseForge、MCIM、整合包、地图、实例内资源
   └─ terracotta/           陶瓦联机：安装、进程管理、HTTP 接口
```

## 目录布局

默认是便携模式，所有东西放在 exe 所在文件夹：

```
<exe 所在文件夹>\
├─ pilot-launcher.exe
└─ .minecraft\                 游戏根目录，可在设置里修改
   ├─ PilotLauncher\           启动器数据目录（固定）
   │  ├─ settings.json
   │  ├─ accounts.json
   │  ├─ runtime\<component>\  Java 运行时
   │  ├─ authlib\              authlib-injector
   │  ├─ terracotta\<版本>\    陶瓦联机程序
   │  └─ cache\                版本清单、加载器版本列表等缓存
   ├─ versions\<实例名>\       实例目录，也是启动时的 --gameDir
   ├─ libraries\               所有实例共享
   └─ assets\                  所有实例共享
```

- exe 所在文件夹不可写时，整体回退到 `%APPDATA%\dev.pilotlauncher.launcher\`
- 实例目录里的 `glass-instance.json` 是实例元数据（加载器类型与版本、图标、内存等），`.glass-vanilla.json` 是原版 JSON 备份，`glass-resources.json` 记录从社区安装的资源。文件名里的 glass 是项目旧名 GlassLauncher 留下的，为了兼容已有实例没有改
- 程序从 GlassLauncher 改名后，第一次启动会把旧的 `.minecraft\GlassLauncher\` 改名为 `.minecraft\PilotLauncher\`
- 其他启动器的版本文件夹（只要有 `<名称>.json`）会显示为外部版本

## 下载

### 下载源替换

`download/source.rs`：

| 官方 | BMCLAPI |
| --- | --- |
| `piston-meta.mojang.com`、`launchermeta.mojang.com`、`piston-data.mojang.com`、`launcher.mojang.com` | `bmclapi2.bangbang93.com` |
| `resources.download.minecraft.net` | `bmclapi2.bangbang93.com/assets` |
| `libraries.minecraft.net` | `bmclapi2.bangbang93.com/maven` |

每个文件生成一组按优先级排好的候选地址：官方模式只用官方；BMCLAPI 模式镜像优先、官方兜底；官方优先模式失败后换镜像。

### 社区资源镜像

`community/mcim.rs`：

| 官方 | MCIM |
| --- | --- |
| `api.modrinth.com` | `mod.mcimirror.top/modrinth` |
| `cdn.modrinth.com` | `mod.mcimirror.top` |
| `api.curseforge.com` | `mod.mcimirror.top/curseforge` |
| `edge.forgecdn.net`、`mediafilez.forgecdn.net` | `mod.mcimirror.top` |

CurseForge 的 `x-api-key` 只发给官方接口，不发给镜像。

### 下载器

并发数可调，失败重试并换下一个候选地址。先写 `.part` 临时文件，边下边算 SHA-1 并检查大小。安装和修复时做完整校验，启动前只检查文件是否存在、大小是否一致，缺的自动补。

## Java

从 Mojang 的 Java 运行时清单（或 BMCLAPI 镜像）读取 windows-x64 列表，按版本 JSON 里的 `javaVersion.component` 下载，找不到就按 `majorVersion` 兜底。逐个文件校验 SHA-1。也会扫描本机已安装的 Java，全局和实例都可以覆盖。

## 启动

- 合并 `inheritsFrom` 链，classpath 按 group:artifact 去重
- 处理 `arguments.game` / `arguments.jvm` 里的 rules 和 features，兼容旧版 `minecraftArguments`
- natives：旧式库按原目录结构解压并遵守 `extract.exclude`；1.19 之后的 natives 库只平铺解压 dll，跳过 x86 和 arm64
- 旧版资源会复制到 `virtual/legacy` 或 `resources`
- Java 9 以上、命令行超过约 30000 字符时改用 `@argfile`
- 进程以 `CREATE_NO_WINDOW` 启动，日志分批推送到前端，访问令牌替换成 `******`
- 崩溃时查找 `crash-reports` 和 `hs_err` 文件，按关键字给出中文提示

## 账户

- 离线：UUID 按 Java `nameUUIDFromBytes` 规则由 `OfflinePlayer:<名字>` 计算
- 外置登录：先做 API 地址指示（ALI）检测，再走 Yggdrasil 的 authenticate / refresh / validate。authlib-injector 从 BMCLAPI 或官方下载并校验，启动时加 `-javaagent` 和 `-Dauthlibinjector.yggdrasil.prebuilt`
- 令牌目前明文存在 `accounts.json`，不保存密码

## 模组加载器

### JSON 合并

`modloader::merge_json`：先取原版 JSON（`.glass-vanilla.json`），再把加载器的 profile 或 version.json 作为子版本合并进实例 JSON，去掉 `inheritsFrom` 和 `jar`，让实例 JSON 自包含。重装或更换加载器时总是从原版重新合并。

### Fabric / Quilt

从 meta 服务获取 profile JSON。版本列表用 `/versions/loader` 和 `/versions/game` 两个小接口并缓存。Fabric 走 BMCLAPI 镜像，Quilt 没有镜像。

### Forge / NeoForge

共用 `forge::run_modern`：

- 下载安装器，读取 `install_profile.json` 和 `version.json`
- libraries 走 maven 镜像；url 为空的库从安装器的 `maven/` 目录解出
- data 变量支持 `[maven 坐标]`、`'字面量'`、`/data/` 路径，以及 SIDE、MINECRAFT_JAR、ROOT 等内置变量
- processors 过滤掉 server 端，用实例对应的 Java 运行，主类从 jar 的 MANIFEST 读取；outputs 已存在且 SHA-1 正确时跳过
- `DOWNLOAD_MOJMAPS` 不让 Java 直连 Mojang，由启动器按下载源下载
- 1.12.2 及以前的旧安装器走 versionInfo + universal jar

### OptiFine

单独安装时参考 HMCL：用实例的 Java 运行安装器里的 `optifine.Patcher` 生成补丁库，再写 launchwrapper 版本 JSON。Patcher 没有界面，输入输出都是确定的文件路径，不用伪造 `.minecraft`，和版本隔离不冲突。与 Forge 共存时直接放进 mods。版本列表和下载固定走 BMCLAPI。

### 互斥规则

Forge、NeoForge、Fabric、Quilt 四选一。OptiFine 不能搭配 Fabric、Quilt（提示改用 OptiFabric），也不能搭配 NeoForge。愚人节版本不能装任何加载器。

### 版本列表缓存

`modloader/meta.rs`：内存加磁盘缓存（`cache\loader-meta\`，1 小时），网络失败时用过期缓存兜底。官方优先模式下官方和镜像同时请求，谁先成功用谁。同一个缓存键同时只发一个请求。前端在选好游戏版本后并行预取所有加载器的列表。

## 社区资源

- Modrinth API v2，用 facets 按类型、版本、加载器、分类过滤
- CurseForge API v1，gameId 432；classId：6 模组、4471 整合包、12 资源包、6552 光影、17 地图、6945 数据包
- 依赖：Modrinth 的 `required`、CurseForge 的 `relationType = 3`，递归最多 6 层。已安装的判断依据是 `glass-resources.json`，Modrinth 还会用 mods 里文件的 SHA-1 反查
- Modrinth 没有地图分类，地图只从 CurseForge 搜索
- 简介渲染：Modrinth 的 markdown 用 marked，CurseForge 的 HTML 用 DOMPurify 净化

### 整合包

- `.mrpack`：读 `modrinth.index.json`，按 dependencies 建实例并装加载器，下载 files（校验 sha1 / sha512，跳过 `env.client = unsupported`），解压 overrides 和 client-overrides
- CurseForge：读 `manifest.json`，用 `/v1/mods/files` 批量取下载地址。作者不允许第三方下载的文件（`downloadUrl` 为空）列出来让用户手动下载

### 解压安全

地图和 overrides 解压时，条目名先经 `enclosed_name()` 规范化，再拒绝绝对路径、盘符和 `..`，并检查目标在目的目录内。

## 愚人节版本

前后端各有一份名单（`src/aprilFools.ts`、`src-tauri/src/minecraft/april_fools.rs`），名单优先：

`15w14a`、`1.RV-Pre1`、`3D Shareware v1.34`、`20w14infinite`、`22w13oneblockatatime`、`23w13a_or_b`、`24w14potato`、`25w14craftmine`、`26w14a`

名单之外用规则兜底：类型为 snapshot、发布时间在 3 月 31 日到 4 月 2 日（UTC），并且 ID 不符合任何常规快照格式。`15w14a`、`26w14a` 这种格式和普通快照一样的只能靠名单，以后的新版本需要手动加进两份名单。

2013 年的 2.0 不在官方清单里，只显示为不能安装的条目，链接到 Wiki。

## 陶瓦联机

固定使用 Terracotta 0.4.2，版本、文件名、大小和哈希写在 `terracotta/metadata.rs`。

- 安装：从 GitHub Releases、Gitee 或 CNB 下载 `terracotta-0.4.2-windows-x86_64-pkg.tar.gz`，校验整包的 SHA-512 和 SHA-256，解压后再逐个校验文件，不做任何修改
- 启动：直接运行 `--hmcl2 <端口文件>`，等陶瓦写出 `{"port": N}`，这样陶瓦是启动器的子进程，能检测意外退出。`--hmcl2` 是陶瓦标注为内部用途的参数，升级陶瓦时要确认它还在
- 单实例：陶瓦在 `%TEMP%\terracotta\terracotta.lock` 里记录端口。HMCL 或 PCL CE 已经开着陶瓦时直接共用，退出时不关闭它
- 接口：只监听 127.0.0.1，全部是 GET
  - `/state`：状态，共 waiting、host-scanning、host-starting、host-ok、guest-connecting、guest-starting、guest-ok、exception 八种
  - `/state/scanning?player=`：开房
  - `/state/guesting?room=&player=`：加入
  - `/state/ide`：回到待机
  - `/log?fetch=true`：日志
  - `/panic?peaceful=true`：退出
- 轮询：联机页打开时约 0.7 秒一次，房间进行中约 1 秒一次，其余时间不请求。连续 4 次失败视为组件已退出
- 邀请码：`U/XXXX-XXXX-XXXX-XXXX`，字符集是 0-9 和 A-Z 去掉 I、O，换算成 34 进制后必须是 7 的倍数
- 退出：启动器退出时依次调 `/state/ide`、`/panic?peaceful=true`，再结束子进程

许可证合规：陶瓦是 AGPL-3.0-or-later 加例外条款。启动器只下载未修改的官方程序，只通过 HTTP 接口交互，在联机页底部和关于页标注了版权，不随启动器分发陶瓦。

升级陶瓦：对照 [HMCL 的 `assets/terracotta.json`](https://github.com/HMCL-dev/HMCL) 改 `metadata.rs` 里的版本、文件名、大小和哈希。

## 界面

- 液态玻璃：`v-refract` 按元素尺寸生成位移贴图，通过 `backdrop-filter: blur() url(#filter) saturate()` 做边缘折射。长列表用不带 `backdrop-filter` 的轻量玻璃，避免滚动卡顿。设置里可以关掉折射
- 背景：`AmbientBackground.vue` 用 CSS 和 SVG 绘制，动画只改 `transform`。离开首页时暂停，系统开启"减少动画"时不动
- 窗口：无边框，自绘标题栏，背景用 Windows 11 的 Mica / Acrylic

## 已知限制

- 只支持 Windows x64
- 没有微软正版登录
- 账户令牌明文保存
- Java 8 不支持 `@argfile`，classpath 特别长时可能超过 Windows 命令行长度上限（原版不会碰到）
- 1.5 以前没有安装器的 Forge 不支持
- OptiFine 单独安装时如果只能用官方 `launchwrapper:1.12`，只能在 Java 8 上运行
- CurseForge 的文件列表只取最近 50 个
- `mods.toml` 是极简解析，读不到名称时显示文件名
- zip 只支持 deflate，用 LZMA 等压缩的地图会解压失败
- 陶瓦不提供延迟数据，所以联机页不显示延迟
