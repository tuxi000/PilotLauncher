# PilotLauncher

> A Liquid Glass styled Minecraft launcher. 玻璃质感 UI，本体/整合包/模组/资源包/地图下载管理，内置免配置联机

> **改名说明**：本项目原名 GlassLauncher，现更名为 **PilotLauncher**（程序文件 `pilot-launcher.exe`，应用标识 `dev.pilotlauncher.launcher`）。
> 首次启动新版本时会自动把旧数据目录 `.minecraft\GlassLauncher\`（或 `%APPDATA%\dev.glasslauncher.launcher\`）改名迁移过来，设置、账户、Java 运行时和联机组件都会保留。
> 实例内部的元数据文件名（`glass-instance.json`、`glass-resources.json`、`.glass-vanilla.json` 等）保持不变，以兼容已有实例。

> 技术栈：**Tauri 2 + Rust**（后端）、**Vite + TypeScript + Vue 3**（前端），仅支持 **Windows 11 x64**。

设计思路参考了 [PCL](https://github.com/Meloong-Git/PCL)（版本隔离、BMCLAPI 下载源、外置登录等）与 [HMCL](https://github.com/HMCL-dev/HMCL)（OptiFine 安装方式、MCIM 镜像规则），布局参考 LauncherX，**没有复制任何代码**。

---

## 快速开始（Windows 11）

1. **安装构建环境**：双击 `scripts\setup-env.bat`。
   它会用 winget 检查并安装 Rustup（以及 `stable-x86_64-pc-windows-msvc` 工具链）、Node.js LTS、
   Visual Studio 2022 Build Tools（C++ 桌面开发工作负载 + Windows 11 SDK），并检查 WebView2。
   安装过程中会弹出 UAC 授权窗口；装完后**关闭并重新打开终端**，新的 PATH 才会生效。
2. **构建**：双击 `scripts\build.bat`（它会以 `ExecutionPolicy Bypass` 调用 `scripts\build.ps1`）。
   脚本依次执行 `npm install` → `npm run tauri -- build`，构建完成后把产物复制到项目根目录下的 `release\`：
   - `pilot-launcher.exe`：免安装版本（需要系统已有 WebView2，Windows 11 自带）
   - `PilotLauncher_0.1.0_x64-setup.exe`：NSIS 安装包
   - `PilotLauncher_0.1.0_x64_zh-CN.msi`：MSI 安装包

`build.bat` 可带参数，会原样传给 `build.ps1`：

```bat
scripts\build.bat -NpmMirror          :: 使用 npmmirror 国内镜像安装前端依赖
scripts\build.bat -Bundle nsis        :: 只打 NSIS 安装包（nsis / msi / none / all）
scripts\build.bat -Debug              :: 调试版本，编译更快
scripts\build.bat -SkipInstall        :: 跳过 npm install
```

开发模式（热更新）：在项目根目录执行 `npm install` 后运行 `npm run tauri dev`。

> 首次构建需要下载并编译约 400 个 Rust crate，通常需要 5～15 分钟；之后的增量构建会快很多。

---

## 项目结构

```
PilotLauncher/
├─ index.html / vite.config.ts / tsconfig.json / package.json
├─ config.local.example.json        # 本地私密配置模板（复制为 config.local.json，已被 .gitignore 忽略）
├─ scripts/
│  ├─ setup-env.bat / setup-env.ps1 # 一键安装构建环境（winget）
│  └─ build.bat / build.ps1         # 一键构建（build.ps1 为 UTF-8 BOM，兼容 PowerShell 5.1）
├─ src/                             # 前端（Vue 3）
│  ├─ main.ts / App.vue             # 入口；整体布局：自绘标题栏 + 左侧导航 + 内容面板
│  ├─ api.ts                        # 后端命令（invoke）与事件的类型定义（含 LOADER_ADDONS：Fabric API / QFAPI）
│  ├─ instanceIcons.ts              # 图标：原版风格方块图片（assets/icons/*.png）+ 原创像素图标，自动图标规则
│  ├─ aprilFools.ts                 # 愚人节版本名单与识别规则（与后端 minecraft/april_fools.rs 一致）
│  ├─ assets/icons/                 # 版本类型 / 加载器图标（64×64 透明 PNG，见“图标来源与授权”）
│  ├─ loaderVersions.ts             # 加载器版本列表的前端缓存、请求合并、并行预取
│  ├─ selectOption.ts               # GlassSelect 的选项类型
│  ├─ markdown.ts                   # 社区资源简介渲染：marked（markdown）+ DOMPurify（净化）
│  ├─ store.ts                      # 全局状态、事件监听（task-update / game-state / game-log）
│  ├─ theme.ts                      # 深色 / 浅色 / 跟随系统；窗口材质；背景配色（强调色微调）
│  ├─ glass/refract.ts              # 液态玻璃指令：v-refract（SVG 位移折射）、v-light（高光跟随）
│  ├─ styles/base.css, glass.css    # 设计变量与玻璃组件样式、弹性动画曲线
│  ├─ components/                   # TitleBar、SideNav、GlassDialog、Segmented、GlassSwitch、TaskCard、
│  │                                #   AccountDialog、LogDrawer、CrashDialog、ToastHost、Avatar、InstanceIcon、Icon
│  │                                #   第二阶段：LoaderPicker（加载器卡片）、CommunityBrowser（社区资源搜索）、
│  │                                #   ResourceDetail（详情 / 版本 / 依赖确认）、InstanceDetail（实例资源管理）、
│  │                                #   ImportDialog（本地整合包 / 地图导入，含拖放）、MissingFilesDialog
│  │                                #   第三阶段：GlassSelect（玻璃下拉框）、GlassCheck（玻璃复选框）
│  │                                #   第四阶段：AmbientBackground（内置简约背景，三套配色）
│  ├─ terracotta.ts                 # 联机：状态 / 异常 / NAT 难度的中文文案，邀请码校验（与陶瓦源码规则一致）
│  └─ pages/                        # HomePage、DownloadPage、InstancesPage、MultiplayerPage（联机）、SettingsPage
└─ src-tauri/                       # 后端（Rust）
   ├─ Cargo.toml / build.rs / tauri.conf.json
   ├─ capabilities/default.json     # Tauri 2 权限（窗口拖动、最小化、关闭、主题、文件选择对话框）
   ├─ icons/                        # 应用图标
   └─ src/
      ├─ main.rs / lib.rs           # 入口、命令注册、启动时应用 Mica
      ├─ state.rs                   # 全局状态 Core
      ├─ commands.rs                # 所有 #[tauri::command]
      ├─ settings.rs                # settings.json / config.local.json
      ├─ paths.rs                   # 目录布局
      ├─ tasks.rs                   # 后台任务与进度事件
      ├─ download/                  # 多线程下载器（并发、重试、多源回退、SHA-1）+ 下载源替换规则
      ├─ minecraft/                 # 版本清单、版本 JSON（rules、inheritsFrom）、依赖库、资源、安装、愚人节版本识别（april_fools.rs）
      ├─ java/                      # 官方 Java 运行时下载、本机 Java 扫描
      ├─ auth/                      # 离线 UUID、Yggdrasil（ALI / authenticate / refresh / validate）、authlib-injector
      ├─ instance.rs                # 实例增删改查（版本隔离）
      ├─ launch/                    # 启动参数、natives 解压、进程与日志、崩溃分析
      ├─ modloader/                 # 加载器：mod.rs（合并 / 阶段 / Java 进程）、fabric.rs（Fabric + Quilt）、
      │                             #   forge.rs（Forge + NeoForge，新旧安装器）、optifine.rs、
      │                             #   meta.rs（版本列表缓存：内存 + 磁盘、请求合并、官方 / 镜像竞速）
      └─ community/                 # 社区资源：mod.rs（统一模型、源策略、依赖计划、安装）、modrinth.rs、curseforge.rs、
                                    #   mcim.rs（镜像规则）、modpack.rs（mrpack / CurseForge 整合包）、
                                    #   world.rs（地图解压）、local.rs（实例内资源管理）
      └─ terracotta/                # 联机（Terracotta | 陶瓦联机）：mod.rs（进程管理、状态轮询、房主 / 房客操作）、
                                    #   metadata.rs（固定版本、哈希、下载地址）、install.rs（下载、校验、解压）、
                                    #   client.rs（本机 HTTP 接口封装、邀请码校验）
```

### 数据目录

便携模式（类似 PCL）：所有东西都放在 `pilot-launcher.exe` 所在文件夹。

| 位置 | 内容 |
| --- | --- |
| `<exe 所在文件夹>\.minecraft\`（默认游戏目录，可在设置中改） | `libraries\`、`assets\`（所有实例共享）、`versions\<实例名>\` |
| `<exe 所在文件夹>\.minecraft\PilotLauncher\`（数据目录，固定） | `settings.json`、`accounts.json`、`runtime\<component>\`（Java）、`authlib\`、`cache\`（含 `cache\loader-meta\` 加载器版本列表缓存）、`terracotta\<版本>\`（联机组件，见“联机”一节） |

- exe 所在文件夹不可写（例如装在 `C:\Program Files`）时，整体回退到 `%APPDATA%\dev.pilotlauncher.launcher\`。
- 从旧版本升级时，首次启动会把旧数据目录里的 `settings.json`、`accounts.json`、`authlib\` 复制过来；Java 运行时不复制，需要时自动重新下载。
- 在设置里改了游戏目录后，数据目录仍留在 exe 旁边的 `.minecraft\PilotLauncher\`。
- WebView2 的浏览器缓存仍由系统放在 `%LOCALAPPDATA%\dev.pilotlauncher.launcher\`。

**版本隔离**：每个实例就是 `versions\<实例名>\`，里面有 `<实例名>.json`、`<实例名>.jar`、`natives\`、
`glass-instance.json`（实例元数据，含加载器类型与版本）、`.glass-vanilla.json`（原版 JSON 备份，安装 / 更换加载器时使用）、
`glass-resources.json`（从社区安装的资源记录）以及 `mods\ saves\ resourcepacks\ shaderpacks\ config\`。
启动时 `--gameDir` 指向这个目录。其他启动器复制过来的版本文件夹（只要有 `<名称>.json`）也会显示为“外部版本”。

---

## 功能状态

| 模块 | 功能 | 状态 |
| --- | --- | --- |
| 版本清单 | 正式版 / 快照 / 远古版本 / 愚人节版本筛选、搜索、最新版标记、离线缓存 | ✅ 已完成 |
| 愚人节版本 | 内置名单 + 规则兜底识别，单独分类（不再混在快照里）、金块图标、禁用加载器；官方未提供下载的 2.0 显示为灰色条目并链接 Wiki | ✅ 第四阶段 |
| 安装原版 | 版本 JSON、客户端 jar、libraries（rules 判断、旧式 classifiers 与 1.19+ 新式 natives）、asset index 与 objects | ✅ 已完成 |
| 下载器 | 多线程并发、失败重试、多源回退、SHA-1 / 大小校验、`.part` 临时文件、进度与速度事件、取消 | ✅ 已完成 |
| 下载源 | 官方 / BMCLAPI / 官方优先失败换镜像（含 Java 运行时清单、authlib-injector 镜像） | ✅ 已完成 |
| Java | 按 `javaVersion.component` 下载 Mojang 官方运行时（找不到按 majorVersion 兜底），逐文件 SHA-1 校验；本机 Java 扫描；全局 / 实例覆盖 | ✅ 已完成 |
| 启动 | classpath 拼接与去重、`arguments.game/jvm` 的 rules / features、旧版 `minecraftArguments`、`inheritsFrom` 合并、内存（自动 / 手动 / 实例覆盖）、窗口大小、旧版资源（virtual / map_to_resources）、超长命令行自动改用 `@argfile`（Java 9+） | ✅ 已完成 |
| 日志与崩溃 | 实时采集 stdout / stderr、令牌脱敏、崩溃报告 / hs_err 识别、常见错误中文提示 | ✅ 已完成 |
| 账户 | 离线登录（`OfflinePlayer:<名字>` MD5，nameUUIDFromBytes 规则） | ✅ 已完成 |
| 账户 | 外置登录：ALI 检测、authenticate / refresh / validate、多角色选择、自动下载 authlib-injector、`-javaagent` 与 `-Dauthlibinjector.yggdrasil.prebuilt` | ✅ 已完成 |
| 实例 | 新建、重命名、删除、编辑（图标、备注、内存、Java、JVM / 游戏参数、窗口大小）、修复文件、一键打开实例文件夹 | ✅ 已完成 |
| 图标 | 原版风格方块图片：正式版草方块 / 快照命令方块 / 远古版本原石 / 愚人节金块，Forge 铁砧 / Fabric 布卷 / NeoForge 狐狸 / Quilt logo / OptiFine 土径；实例默认“自动”图标；石头等原创像素图标可选 | ✅ 第四阶段 |
| 加载器 | 新建或补装 Fabric / Quilt 时默认同时安装 Fabric API / QFAPI（可取消、可改选版本） | ✅ 第三阶段 |
| 加载器 | 版本列表内存 + 磁盘缓存（1 小时）、请求合并、“官方优先”官方与镜像竞速、选好游戏版本后并行预取 | ✅ 第三阶段 |
| 界面 | GlassSelect 玻璃下拉框（搜索、虚拟滚动、键盘操作、自动上下展开、标签）、GlassCheck 复选框，替换全部原生 select / datalist / checkbox | ✅ 第三阶段 |
| 设置 | 下载源、并发数、重试、内存、Java 路径覆盖、JVM / 游戏参数、游戏目录、主题、窗口材质、折射开关、自定义背景 | ✅ 已完成 |
| 背景 | 内置简约背景（渐变曲线 + 细线 + 少量几何形状，矢量绘制）、蓝紫 / 薄荷 / 珊瑚三套配色（各有浅色、深色版）、缓慢流动动画开关、强调色随配色微调 | ✅ 第四阶段 |
| 界面 | 液态玻璃（backdrop-filter + 高光描边 + SVG feDisplacementMap 边缘折射）、弹性动画、无边框自绘标题栏、Mica / Acrylic、深浅色跟随系统 | ✅ 已完成 |
| 加载器 | Fabric / Quilt：meta 服务 profile JSON 合并进实例 JSON（Fabric 走 BMCLAPI 镜像，Quilt 只走官方） | ✅ 已完成 |
| 加载器 | Forge / NeoForge 新版安装器：install_profile + version.json、libraries（maven 镜像）、data 变量（坐标 / 字面量 / `/data/` 解包）、processors（sides 过滤、MANIFEST Main-Class、outputs SHA-1 校验、DOWNLOAD_MOJMAPS 走下载源） | ✅ 已完成 |
| 加载器 | Forge 旧版（1.12.2 及以前）：versionInfo + universal jar | ✅ 已完成 |
| 加载器 | OptiFine：单独安装（optifine.Patcher + launchwrapper）、与 Forge 共存（放入 mods）；版本列表与下载走 BMCLAPI | ✅ 已完成 |
| 加载器 | 版本列表标记推荐版 / 最新版；Fabric 与 Forge / NeoForge / Quilt 互斥；OptiFine 不能搭配 Fabric / Quilt / NeoForge（提示 OptiFabric）；新建实例时选择、已有原版实例补装 / 更换 | ✅ 已完成 |
| 社区资源 | Modrinth API v2：facets 过滤（类型 / 版本 / 加载器 / 分类）、排序、分页；详情、版本、依赖 | ✅ 已完成 |
| 社区资源 | CurseForge API v1：搜索、分类、详情（HTML 描述）、文件列表、依赖（relationType 3）；无 Key 时提示只能用 Modrinth | ✅ 已完成 |
| 社区资源 | MCIM 镜像（官方优先 / 镜像优先 / 只用官方，设置中单独配置）；Key 只发给 CurseForge 官方 | ✅ 已完成 |
| 社区资源 | 安装到实例：mod / 资源包 / 光影包 / 数据包（选存档）/ 地图（解压到 saves）；默认按实例版本与加载器过滤；必需依赖自动列出、确认后一起安装、已装跳过；更新时替换旧文件 | ✅ 已完成 |
| 整合包 | Modrinth .mrpack（sha1 / sha512 校验、跳过 env.client=unsupported、overrides + client-overrides）、CurseForge zip（批量取下载地址、不允许第三方下载的文件列出并给出链接）；在线安装与本地导入 | ✅ 已完成 |
| 地图 | 只接 CurseForge（Modrinth 没有地图分类）；本地导入 zip（对话框或拖进窗口），自动定位 level.dat（多层嵌套、多存档），重名自动改名，防 zip slip | ✅ 已完成 |
| 实例资源 | 实例详情页：mods / 资源包 / 光影包 / 存档列表；读取 fabric.mod.json、quilt.mod.json、(neoforge.)mods.toml、mcmod.info 的名称 / 版本 / 图标；启用 / 禁用（.disabled）、删除、在资源管理器中定位 | ✅ 已完成 |
| 联机 | 内置 Terracotta | 陶瓦联机 0.4.2：首次使用时下载官方原版程序（GitHub / Gitee / CNB，遵守下载源设置，SHA-512 + SHA-256 校验），房主扫描局域网世界生成邀请码，房客输入邀请码加入；玩家列表、NAT 难度、异常提示、组件崩溃检测与重启；退出启动器时自动关闭 | ✅ 联机 |
| 微软登录 | — | ❌ 不在计划内 |

### CurseForge API Key

CurseForge 的搜索、下载与整合包都需要它（没有 Key 时只能使用 Modrinth；地图只来自 CurseForge，但可以本地导入）。Key **只**从本地文件 `config.local.json` 读取（已加入 `.gitignore`，源码中没有任何真实 Key）：

1. 复制 `config.local.example.json` 为 `config.local.json`；
2. 填写 `curseforgeApiKey`；
3. 放在 `pilot-launcher.exe` 同目录，或数据目录 `.minecraft\PilotLauncher\`（开发模式下也可放项目根目录）。

设置页的“关于”会显示是否已读取到 Key。

---

## 使用说明（第二阶段）

### 安装模组加载器
- **新建实例**：下载 → 游戏版本 → 选择版本点“安装”，在弹窗的加载器卡片中勾选 Forge / NeoForge / Fabric / Quilt / OptiFine 并选版本
  （默认选中推荐版，没有推荐版时选最新版；“只看稳定版”可过滤测试版）。Forge 与 OptiFine 可以同时勾选，OptiFine 会作为 mod 放进 `mods`。
- **给已有实例补装 / 更换**：实例 → 卡片上的拼图按钮（资源管理）→“加载器”标签。会从 `.glass-vanilla.json` 重新合并实例 JSON，mods / 存档 / 配置不受影响。
- Forge / NeoForge 安装器与 OptiFine Patcher 使用实例对应的 Java 运行；没有时自动下载 Mojang 官方运行时。整个过程显示在“下载 → 任务”中。

### 社区资源
- 下载页的“模组 / 整合包 / 资源包 / 光影包 / 数据包 / 地图”标签：顶部搜索框 + 平台（Modrinth / CurseForge / 全部）+ 游戏版本 / 加载器 / 分类 / 排序；
  “适配 xxx”按钮会按当前实例的版本与加载器筛选。结果支持无限滚动。
- 点开卡片：简介（Modrinth markdown / CurseForge HTML，均经 DOMPurify 净化，链接在系统浏览器打开）、截图、版本列表。
  选择目标实例（数据包还要选存档）后点“安装”；模组的必需依赖会列出确认，已安装的自动跳过。
- 整合包：详情里填写新实例名后安装；或点“导入本地整合包”选择 `.mrpack` / CurseForge zip。CurseForge 整合包中作者禁止第三方下载的文件会弹窗列出，
  之后也能在实例的资源管理中看到下载链接。
- 地图：只来自 CurseForge（Modrinth 没有地图分类）。没有 Key 也可以“导入本地地图”，或者**直接把 zip 拖进窗口**（整合包同样支持拖放）。
- 设置 → 下载 →“社区资源源”：官方优先失败换 MCIM / MCIM 优先 / 只用官方。

### 新增的 Tauri 权限 / 配置
| 位置 | 内容 | 用途 |
| --- | --- | --- |
| `Cargo.toml` | `tauri-plugin-dialog = "2"` | 文件选择对话框 |
| `package.json` | `@tauri-apps/plugin-dialog`、`marked`、`dompurify` | 对话框 JS API；简介渲染与净化 |
| `lib.rs` | `.plugin(tauri_plugin_dialog::init())` | 注册插件 |
| `capabilities/default.json` | `dialog:allow-open`、`dialog:allow-message` | 允许前端打开文件选择框 / 消息框 |
| `tauri.conf.json` | `"dragDropEnabled": true` | 接收拖进窗口的文件路径（`getCurrentWebview().onDragDropEvent`，使用 `core:default` 中的事件权限） |
| `tauri.conf.json` | `"csp": null`（保持不变） | 不限制图片来源，Modrinth / CurseForge 的 https 图标与截图可以直接显示；远程 HTML 一律先经 DOMPurify 净化（去掉 script / iframe / 表单 / 内联事件） |

---

## 使用说明（第三阶段）

### 图标
第三阶段的程序绘制像素图标已在第四阶段换成原版风格的方块图片，见下文“使用说明（第四阶段）→ 图标”。

### Fabric API / QFAPI 自动安装
- 新建实例或在“资源管理 → 加载器”中补装时，勾选 Fabric 后卡片下方会出现一行 **Fabric API**（Modrinth 项目 `fabric-api`，ID `P7dR8mSH`），
  默认勾选，并自动选好适配该游戏版本的最新版（按 `game_versions` 与 `loaders=fabric` 过滤），也可以在下拉框里改选。
- 勾选 Quilt 时对应的是 **Quilted Fabric API（QFAPI）**（Modrinth slug `qsl`，ID `qvIfYCYJ`，已于 2026-11 核实；它只发布到 1.21 左右，更新的游戏版本会显示“没有适配此版本”）。
- 找不到适配版本时显示“没有适配此版本的 Fabric API”，不影响安装加载器。
- 加载器装好后在**同一个任务**里多一个阶段“下载 Fabric API”，复用社区资源的安装代码：遵守“社区资源源”策略（MCIM 镜像）、
  校验哈希、写入 `glass-resources.json`。实例里已有其他版本的 Fabric API（包括手动放进去的）会被替换，避免重复加载。
  这一步失败时任务显示失败，但加载器已经装好、实例可以正常启动，可以稍后在“下载 → 模组”中手动安装。

### 加载器版本列表提速
找到的耗时原因与对应的优化：

| 原因 | 优化 |
| --- | --- |
| 每次打开安装弹窗、每次点开卡片都重新请求，没有缓存 | 后端内存 + 磁盘缓存（`cache\loader-meta\`，TTL 1 小时），网络失败时用过期缓存兜底；前端再缓存 10 分钟 |
| 只有点开卡片才开始请求；Forge 先等版本列表再串行请求推荐版信息 | 选好游戏版本（点“安装”或打开资源管理）时并行预取全部加载器与 Fabric API / QFAPI；Forge 版本列表与推荐版并行 |
| “官方优先”模式先连官方，国内常常要等连接超时（15 s）或读超时（60 s）加重试后才换镜像；NeoForge 官方接口没有镜像候选 | 官方优先时官方与 BMCLAPI **同时请求、谁先成功用谁**；只用官方时照常；BMCLAPI 模式镜像优先 |
| Forge 官方 maven-metadata.xml 包含所有版本；Fabric / Quilt 按版本请求的 `/versions/loader/<mc>` 每项都带完整 launcherMeta | Forge：BMCLAPI `/forge/minecraft/<mc>`（小）与官方 metadata（缓存后所有游戏版本共用）竞速；NeoForge：官方 `/api/maven/versions/releases/...` JSON 接口；Fabric / Quilt：改用 `/versions/loader` + `/versions/game` 两个小接口并缓存，不再按游戏版本重新拉大 JSON |
| 同一时间的重复请求各发各的 | 后端同一个缓存 key 同时只有一个请求，其余等待复用；前端同一 (加载器, 版本) 复用同一个 Promise |

每张加载器卡片单独显示加载状态（骨架屏 / 转圈），不用等全部加载完；加载失败的卡片再次点击会跳过缓存重新请求。

### 液态玻璃控件
- `GlassSelect`：玻璃触发按钮，弹出层通过 Teleport 挂到 body（不会被对话框裁切），`backdrop-filter` 玻璃背景、圆角、弹簧动画；
  选项悬停 / 选中高亮，可带“推荐 / 最新 / 测试版”标签、图标与第二行说明；选项多于 10 个时显示搜索框，并做了虚拟滚动（只渲染可见的几十项）；
  支持 ↑ ↓ / PageUp / PageDown / Home / End / 回车 / Esc（Esc 只关闭下拉框，不会关闭外层对话框），点外部关闭，按可用空间自动向上或向下展开。
- 已替换：加载器版本、实例设置的 Java（输入框 + 已检测 Java 下拉）、社区资源筛选（版本 / 加载器 / 分类 / 排序）、安装到实例、数据包选存档、导入地图的目标实例。
- `GlassCheck` 替换了原生 checkbox（“只看稳定版”、依赖确认列表、Fabric API）；原生 number 输入框的上下箭头已隐藏；滚动条统一为细玻璃样式。
- 实例卡片上的“打开文件夹”按钮直接打开该实例的游戏目录 `versions\<实例名>\`（去掉了二级菜单；资源管理里各标签的文件夹按钮仍打开对应子目录）。

---

## 使用说明（第四阶段）

### 图标
版本类型与加载器图标换成了原版风格的方块图片（`src/assets/icons/`，去白底的 64×64 透明 PNG，
在 `instanceIcons.ts` 里用 Vite 的 `import` 引入，构建时带哈希打包，不放在 `public/`）：

| 图标 | 文件 | 用在 |
| --- | --- | --- |
| 草方块 | `grass.png` | 正式版（release） |
| 命令方块 | `command_block.png` | 快照（snapshot） |
| 原石 | `cobblestone.png` | 远古版本（old_beta / old_alpha） |
| 金块 | `gold_block.png` | 愚人节版本 |
| 铁砧 | `anvil.png` | Forge（Forge + OptiFine 也显示铁砧） |
| 布卷 | `fabric.png` | Fabric |
| 狐狸 | `neoforge.png` | NeoForge |
| Quilt logo | `quilt.png` | Quilt |
| 土径方块 | `optifine.png` | 单独安装的 OptiFine（沿用 PCL 的惯例） |

- **自动图标规则不变**：有加载器显示加载器图标；没有加载器时正式版草方块、快照命令方块、远古版本原石、**愚人节版本金块**。
  “实例设置 → 图标”里列出全部图片图标，“自动”选项保留；第三阶段原创的石头 / 钻石块 / 红石块 / 紫水晶块仍可手动选择
  （它们还是程序生成的 SVG）。旧数据里手动选过的“金块”（`gold`）现在显示金块图片。
- 所有图标都是 256×256 的高清 PNG：方块按原版 16×16 贴图重新渲染（最近邻采样，像素清晰），显示时由浏览器平滑缩小，任何尺寸都不糊。
- 原来给程序绘制图标准备的玻璃底座已去掉，图标直接放在卡片上，只加一层很轻的投影；尺寸大于 48px 的地方（实例卡片）投影稍重。
- 用到图标的地方：下载页版本列表与版本类型筛选、加载器卡片、实例卡片与图标选择器、首页当前实例与实例弹出列表、
  社区资源“安装到实例”与导入地图的下拉框、社区资源的加载器筛选。

#### 图标来源与授权
- 方块图标（草方块、命令方块、原石、金块、铁砧、土径、石头、钻石块、红石块、紫水晶块）：用 Minecraft 原版 16×16 方块贴图
  （取自 [InventivetalentDev/minecraft-assets](https://github.com/InventivetalentDev/minecraft-assets) 的 1.21.4 分支）按物品栏视角渲染成等距方块。
  贴图**版权归 Mojang Studios 所有**，本项目只在启动器界面里用作图标，不对其主张任何权利；公开发布前请自行评估。
- Fabric 布卷：[FabricMC/fabric-loader](https://github.com/FabricMC/fabric-loader) 的官方图标（32×32，8 倍最近邻放大）。
- NeoForge 狐狸：[NeoForged](https://github.com/neoforged) 的官方像素 logo，去掉灰色底板后整数倍放大。
- Quilt logo：[QuiltMC/art](https://github.com/QuiltMC/art) 的 `brand/1024png/quilt_logo_transparent.png`，以 **CC0** 发布。
- 渲染脚本没有放进仓库；需要重新生成时，按 README 上面的来源下载贴图即可。

### 愚人节版本
- 下载 → 游戏版本 → 类型筛选里新增 **“愚人节”**（金块图标）。愚人节版本**不再出现在“快照”分类里**。
- 识别方式参考 PCL：**内置名单优先**，规则只用来兜底未来的新版本。名单（2026-11 用官方
  `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` 核实，全部是官方清单里实际存在的 ID）：

  | 年份 | ID | 名称 |
  | --- | --- | --- |
  | 2015 | `15w14a` | The Love and Hugs Update |
  | 2016 | `1.RV-Pre1` | Trendy Update（官方 releaseTime 是 3 月 31 日） |
  | 2019 | `3D Shareware v1.34` | 3D Shareware |
  | 2020 | `20w14infinite` | 20w14∞（清单中的 ID 是 `20w14infinite`） |
  | 2022 | `22w13oneblockatatime` | One Block at a Time |
  | 2023 | `23w13a_or_b` | The Vote Update |
  | 2024 | `24w14potato` | Poisonous Potato Update |
  | 2025 | `25w14craftmine` | Craftmine Update |
  | 2026 | `26w14a` | Herdcraft Update（2026 年起正式快照改为 `26.x-snapshot-N`，这个愚人节版本沿用了旧的周快照格式） |

  `15w14a` 与 `26w14a` 的 ID 与普通周快照格式相同，只能靠名单识别。
- **规则兜底**：类型是 `snapshot`、`releaseTime`（UTC）在 3 月 31 日～4 月 2 日之间，并且 ID 不符合任何常规快照格式
  （`\d\dw\d\d[a-z]`、`1.20.5-pre1`、`1.21-rc1`、`26.1-snapshot-3`、`26.4-pre-1`、`1.14 Pre-Release 2`、纯版本号）的版本也算愚人节版本。
  已用 2026-11 的官方清单验证：规则只命中名单里的版本，3 月 31 日～4 月 3 日发布的 22w13a、20w14a、26.1.1-rc-1 等正常快照不会被误判。
- 前后端一致：后端 `minecraft/april_fools.rs` 在版本清单的每一项上加了 `aprilFools` 字段（官方 `type` 保持 `snapshot` 不变），
  实例（包括其他启动器复制过来的外部版本）的 `versionType` 识别为 `april_fools`，自动图标显示金块；前端 `aprilFools.ts`
  有同样的名单与规则，用于字段缺失时兜底，以及只有版本号的场合（加载器卡片）。
- 官方清单里没有的早期愚人节版本（2013 年的 **2.0**）显示在愚人节分类末尾：灰色、不能安装，标注“官方未提供下载”，
  点击在浏览器中打开中文 Minecraft Wiki 的 [Java版2.0](https://zh.minecraft.wiki/w/Java%E7%89%882.0) 页面。
- 下载与安装流程和普通版本完全一样，走现有的下载源（官方 / BMCLAPI / 官方优先）。愚人节版本不支持任何加载器：
  安装对话框与“资源管理 → 加载器”中的加载器卡片全部禁用，并提示“愚人节版本不支持模组加载器”；不会去请求加载器版本列表；
  后端 `create_instance` 也会拒绝给名单中的版本附带加载器。

### 背景与配色
- 默认背景不再是方块风景图（`public/backgrounds/default.jpg` 已删除），换成代码绘制的现代简约背景 `components/AmbientBackground.vue`：
  底色渐变 + 三团柔和的彩色光晕 + 两道半透明渐变波浪 + 三条细曲线 + 一个半透明圆、一个细圆环、一个圆角方块，没有 Minecraft 元素，
  留白多，适合衬在液态玻璃下面。全部是 CSS / SVG 矢量绘制（`preserveAspectRatio="xMidYMid slice"` 铺满，细线 `non-scaling-stroke`），任何窗口尺寸都清晰。
- **设置 → 外观 → 背景配色**：三张玻璃色卡（直接渲染缩小的静态背景作为预览）——
  **蓝紫**（冷静、现代，默认）、**薄荷**（青绿，清新）、**珊瑚**（暖橙 / 珊瑚粉，温暖）。每套都有浅色和深色两个版本，跟随主题自动切换；
  深色版降低了光晕的饱和度与不透明度，浅色版用带色相的浅底色，不会发灰。
- **强调色随配色微调**：薄荷 `#0d9488`、珊瑚 `#e05a33`（蓝紫保持默认 `#0a84ff`）。两个颜色的亮度与默认蓝相当：
  白字按钮对比度约 3.7（默认蓝 3.65），作为文字放在深色底上约 4.8（默认蓝 4.9），按钮与链接的可读性不变。
- **背景动画**：默认开启，轻微缓慢地流动（每层 48～82 秒一个往复周期，几乎察觉不到）；设置里可以关闭；
  系统开启“减少动画”（`prefers-reduced-motion`）时自动停止。动画只改各层的 `transform`，不逐帧改 SVG path、不对动画层用模糊滤镜，
  由 WebView2 的合成器处理；离开首页时动画暂停（保留当前位置），避免其它页面大量 `backdrop-filter` 玻璃卡片每帧重新模糊。
- **自定义背景图优先**（`settings.backgroundImage`）：设置了图片就显示图片（与以前一样首页清晰、其它页面虚化）；
  内置背景下首页问候语改用主题文字颜色（不再是白字 + 暗角）。
- 新增设置字段：`backgroundPalette: "blue" | "mint" | "coral"`（默认 `blue`）、`backgroundAnimation: bool`（默认 `true`）。
  `settings.rs` 中带 serde 默认值，旧的 `settings.json` 可以直接读取。

---

## 使用说明（联机 · Terracotta | 陶瓦联机）

左侧导航栏新增 **联机** 页（在“实例”和“设置”之间）。联机功能由 [Terracotta | 陶瓦联机](https://github.com/burningtnt/Terracotta)
提供：它基于 [EasyTier](https://github.com/EasyTier/EasyTier) 建立 P2P 虚拟网络，把房主“对局域网开放”的世界转发给房客。
本启动器**不包含、不修改、不链接**陶瓦的代码，只在首次使用时下载官方发行的原版程序，并通过它自带的本机 HTTP 接口控制它。

### 使用方法

1. 打开联机页，点击 **安装联机组件**（约 7.8 MB，进度显示在任务卡片里，也可在 下载 → 任务 中查看）。安装后打开联机页会自动启动联机组件。
2. **房主**：在“创建房间”卡片里选择实例 → **启动游戏** → 进入单人世界后按 `Esc` → **对局域网开放** → **创建局域网世界** →
   回到启动器点 **创建房间**（先点创建房间再开局域网也可以）。检测到局域网世界后会显示大号邀请码（`U/XXXX-XXXX-XXXX-XXXX`），
   自动复制到剪贴板，也可以点“复制邀请码”。点 **关闭房间** 结束。
3. **房客**：在“加入房间”卡片里粘贴邀请码（可以直接粘贴整段聊天消息，会自动找出邀请码并校验）→ **加入**。连接成功后
   打开游戏 → **多人游戏**，局域网列表里会出现“陶瓦联机大厅”，双击进入；也可以用页面上显示的备用地址（如 `127.0.0.1:25565`）直接连接。
   点 **退出房间** 结束。
4. 房间进行中时，首页问候语下方会显示一个小胶囊（房间号 / 人数），导航栏“联机”图标上有小圆点，点击即可回到联机页。
5. 联机组件异常退出时，页面顶部会显示原因并提供 **重启联机组件** / **重新安装**；“复制日志”可以复制联机组件日志的最后 400 行用于反馈。

### 实现方式（参考 HMCL 的集成流程，没有复制 HMCL 的代码）

- **固定版本**：Terracotta `0.4.2`（与 HMCL 当前 main 分支 `assets/terracotta.json` 的 `version_latest` 相同）。
  Windows x86_64 发行包：`terracotta-0.4.2-windows-x86_64-pkg.tar.gz`，8,135,914 字节，
  SHA-256 `07ebe139e3ca5f74576e58b1a96efe59abdfbe148d3f1a49bfdca8b6f70745f0`；包内 `terracotta-0.4.2-windows-x86_64.exe`（9,402,880 字节）
  与 `VCRUNTIME140.DLL`（97,656 字节），三者的 SHA-512 写在 `src-tauri/src/terracotta/metadata.rs`。
- **下载源**：官方 = GitHub Releases；国内镜像 = Gitee（陶瓦 README 中的国内镜像）与 CNB（HMCL 元数据中的 downloads_CN）。
  设置里的下载源：“官方”只用 GitHub；“BMCLAPI”（陶瓦没有 BMCLAPI 镜像）按“镜像优先”处理：Gitee → CNB → GitHub；“官方优先”：GitHub → Gitee → CNB。
- **启动**：`terracotta-0.4.2-windows-x86_64.exe --hmcl2 <端口文件>`，`CREATE_NO_WINDOW`，`kill_on_drop`。
  陶瓦的 HTTP 服务就绪后把 `{"port": 端口}` 写到 `<端口文件>.tmp` 再改名。HMCL 用的 `--hmcl` 在 Windows 上只是一个引导进程
  （它以 DETACHED_PROCESS 另起 `--hmcl2` 工作进程后自己退出）；这里直接运行工作模式，陶瓦就是启动器的子进程，便于检测意外退出和退出时清理。
- **单实例**：陶瓦在 `%TEMP%\terracotta\terracotta.lock` 中记录正在运行实例的端口；HMCL / PCL CE 已经启动陶瓦时，本启动器直接复用它（页面会提示），且不会在退出时关闭它。
- **状态**：轮询 `GET /state`（联机页打开时约 0.7 秒一次，房间进行中约 1 秒一次，其余时间不请求），忽略 `index` 未增大的响应，变化时通过 `terracotta-state` 事件推给前端；
  连续 4 次请求失败视为联机组件已退出。
- **操作**：房主 `GET /state/scanning?player=<当前账户名>`；房客 `GET /state/guesting?room=<邀请码>&player=<当前账户名>`；
  退出 / 关闭房间 / 清除异常 `GET /state/ide`（开房 / 加入前若不在 waiting 会先调用它）；日志 `GET /log?fetch=true`。
  HMCL 还会从自己的服务拉取额外的 EasyTier 公共节点（`public_nodes` 参数），本启动器不使用它，只用陶瓦内置的公共节点。
- **退出**：启动器退出（`RunEvent::Exit`）时依次调用 `/state/ide`（关闭房间、结束 EasyTier）→ `/panic?peaceful=true`（正常退出）→ 结束子进程。

### 数据存放位置

| 位置 | 内容 |
| --- | --- |
| `<数据目录>\terracotta\0.4.2\` | 联机组件：`terracotta-0.4.2-windows-x86_64.exe`、`VCRUNTIME140.DLL`（原样解压，未修改） |
| `<数据目录>\terracotta\run\` | 启动时的临时端口文件（读取后删除） |
| `<数据目录>\terracotta\owned-port` | 本启动器启动的陶瓦端口（用于识别上次没关掉的实例） |
| `<数据目录>\cache\terracotta-0.4.2-windows-x86_64-pkg.tar.gz` | 下载中的安装包，安装完成后删除 |
| `%TEMP%\terracotta\` | 陶瓦自己的工作目录：全局锁、machine-id、每次运行的日志 `application.log` 与内嵌的 EasyTier（由陶瓦管理，超过 1 天自动清理） |

`<数据目录>` 在便携模式下是 `<exe 所在文件夹>\.minecraft\PilotLauncher\`。

### 版权与许可

- Terracotta | 陶瓦联机：© burningtnt 及贡献者，基于 EasyTier，使用 **GNU AGPL-3.0-or-later** 许可，并附有例外条款：
  通过陶瓦提供的进程间通信接口（如 HTTP API）与**未经修改**的陶瓦程序交互、且在用户界面明显处标识陶瓦版权信息的程序，不因此被 AGPL 覆盖。
- 本启动器的做法：只下载官方发布的二进制并校验哈希、**不修改、不静态 / 动态链接**；只通过本机 HTTP 接口交互；
  联机页底部固定显示版权与许可卡片（含 GitHub、许可证、EasyTier 链接），设置 → 关于 中也有致谢与链接。
- 陶瓦的二进制不随本启动器分发，由用户在使用时从官方 / 镜像下载。

### 已知限制

- 仅支持 Windows x86_64（陶瓦要求 Windows 10 及以上）。arm64 等其他平台的发行包没有接入。
- 版本固定为 0.4.2；升级陶瓦需要修改 `metadata.rs` 中的版本、文件名、大小与哈希（可对照 HMCL 的 `assets/terracotta.json`）。旧版本目录会在安装新版本后自动删除。
- 直接运行 `--hmcl2` 是陶瓦标注为内部用途的启动方式（HMCL 的引导进程最终也是运行它）；如果以后的陶瓦版本改了这一点，需要相应调整。
- 陶瓦只提供成员的名称、身份（房主 / 房客 / 你）与来源，**不提供延迟**，所以页面上没有延迟显示。
- 共用 HMCL / PCL CE 启动的陶瓦时，两个启动器看到的是同一个房间状态；在任一启动器里关闭房间都会影响另一个。
- 启动器被强制结束（任务管理器）时无法通知陶瓦退出；下次打开联机页会识别并接管上次留下的实例。
- 首次联机时 Windows 防火墙可能弹窗询问是否允许 EasyTier 访问网络（它会在本机监听 TCP / UDP 端口），请允许；部分安全软件可能拦截从临时目录运行的 EasyTier。
- 离线账户的 UUID 由用户名决定，房间里有同名玩家会互相挤下线；房客使用外置登录（皮肤站）账户经“陶瓦联机大厅”进入时可能出现“无效的玩家档案公钥签名”（陶瓦作者说明的已知情况），改用离线账户即可。
  本启动器没有离线皮肤功能，所以不需要像 HMCL 那样在联机时关闭离线皮肤。

---

## 主要设计

- **下载源替换规则**（`src-tauri/src/download/source.rs`）：
  `piston-meta / launchermeta / piston-data / launcher.mojang.com → bmclapi2.bangbang93.com`，
  `resources.download.minecraft.net → bmclapi2.bangbang93.com/assets`，
  `libraries.minecraft.net → bmclapi2.bangbang93.com/maven`。
  每个文件都会生成按优先级排列的候选地址：官方源只用官方；BMCLAPI 模式镜像优先、官方兜底；“官方优先”模式失败后自动换镜像。
- **文件校验**：安装 / 修复时完整校验 SHA-1；启动前只做“存在 + 大小”的快速检查，缺失文件会自动补全。
- **natives**：旧式（`natives` + `classifiers`）按原目录结构解压并遵守 `extract.exclude`；
  1.19+ 的 `natives-windows` 库放进 classpath 的同时把 dll 平铺解压到 `versions\<实例>\natives`，x86 / arm64 natives 自动跳过。
- **加载器 JSON 合并**（`modloader::merge_json`）：加载器 profile / version.json 作为 inheritsFrom 子版本合并进实例 JSON：
  libraries 子在前（同名去重，classpath 再按 group:artifact 去重），`arguments.game/jvm` 父在前子在后，其余字段子版本覆盖，
  去掉 inheritsFrom / jar，使实例 JSON 自包含（客户端 jar 就是 `<实例名>.jar`）。从其他启动器复制来的外部版本仍由 `load_resolved` 递归合并 inheritsFrom。
  Forge 1.17+ 的 jvm 参数 `${library_directory}`、`${classpath_separator}`、`${version_name}`（= 实例名，与 ignoreList 中的 `<实例名>.jar` 对应）均已支持。
- **Forge / NeoForge 安装器**：同一套代码（`forge::run_modern`）。`DOWNLOAD_MOJMAPS` 处理器不让 Java 直连 Mojang，而是由启动器按下载源下载
  `client_mappings`；已存在且 SHA-1 正确的 outputs 会跳过对应处理器（重装很快）。
- **OptiFine 安装方式的选择**：采用 HMCL 的方式——单独安装时运行安装器内的 `optifine.Patcher` 生成补丁库 `optifine:OptiFine:<mc>_<type>_<patch>`，
  再写 launchwrapper 版本 JSON（`--tweakClass optifine.OptiFineTweaker`，优先使用安装器自带的 launchwrapper-of 以支持 Java 9+）。
  理由：Patcher 是无界面的命令行入口，输入输出是确定的文件路径，不需要像 PCL 那样伪造 `.minecraft` / `launcher_profiles.json` 再复制版本目录，
  与版本隔离的目录结构无冲突，失败时有明确的退出码与输出。与 Forge 共存时直接把 OptiFine jar 放进 mods。
- **社区资源源策略**：`community::mcim` 生成候选地址（已核对 MCIM 官网与 HMCL 2026-11 的规则：`api.modrinth.com → mod.mcimirror.top/modrinth`、
  `cdn.modrinth.com → mod.mcimirror.top`、`api.curseforge.com → mod.mcimirror.top/curseforge`、`edge.forgecdn.net → mod.mcimirror.top`；
  `mediafilez.forgecdn.net` 同样映射，作为候选之一，失败自动回退）。`x-api-key` 只发给 CurseForge 官方，不会发给镜像。
- **依赖解析**：Modrinth `dependency_type = required`（支持只给 version_id 的依赖）、CurseForge `relationType = 3`，递归最多 6 层；
  已安装的判断来自 `glass-resources.json` 记录，Modrinth 还会用 mods 目录中文件的 SHA-1 调用 `/version_files` 反查（能识别手动放入的 mod）。
- **zip 安全**：地图、整合包 overrides 解压时条目名先经 `enclosed_name()` 规范化，再拒绝绝对路径 / 盘符 / `..`，并检查目标位于目的目录内。
- **液态玻璃**：`v-refract` 为每个玻璃元素生成与其尺寸一致的位移贴图（R 通道控制 x、B 通道控制 y，中心 50% 灰不位移），
  通过 `backdrop-filter: blur() url(#filter) saturate()` 实现边缘折射；WebView2 基于 Chromium，支持 SVG 滤镜用于 backdrop-filter。
  长列表使用不带 backdrop-filter 的轻量玻璃，避免滚动卡顿；性能较弱的设备可在设置里关闭折射。

---

## 已知限制

- **联机功能的代码尚未在本地编译验证**：此前各阶段已在 Windows 11 上编译通过；联机（`src-tauri/src/terracotta/`、`MultiplayerPage.vue` 等）
  在没有 Rust / Node 的环境中编写，未执行过 `cargo build` 与 `npm run build`，如遇编译错误请把报错贴出来。
  联机功能新增了一个 Rust 依赖 `flate2 = "1"`（解压 tar.gz 的 gzip 部分；zip / reqwest 本来就间接依赖它）。新增依赖需要联网：`npm install`（plugin-dialog、marked、dompurify）
  与 cargo 拉取 `tauri-plugin-dialog`。新版 tauri-plugin-dialog 要求较新的 Rust（官方文档写明 1.90+），请先 `rustup update`。
- **Tauri 版本一致性**：`Cargo.toml` 中 `tauri = "2"`、`package.json` 中 `@tauri-apps/api` / `@tauri-apps/cli` 为 `^2.2.0`，
  都会解析到最新的 2.x。Tauri CLI 会检查 Rust crate 与 npm 包的主/次版本是否一致；如果提示 “version mismatched Tauri packages”，
  执行 `npm install @tauri-apps/api@<与 tauri crate 相同的 2.x 版本>` 即可（或在 `src-tauri` 中 `cargo update`）。
- **MSI 打包**依赖 WiX（Tauri 自动下载），WiX 需要 Windows 的 VBScript 可选功能；若 MSI 失败，可先用 `-Bundle nsis`。
- 账户令牌以明文保存在 `accounts.json`（仅本机用户目录可读），不保存密码。
- Java 运行时下载使用清单里的原始文件（不使用 lzma 压缩版本），体积略大。
- 旧版本（1.12.2 及以下）的完整启动参数在 Java 8 下不能使用 `@argfile`，若 classpath 超过 Windows 命令行长度上限（约 32K 字符）会启动失败；原版不会触发。
- 用户名或游戏目录含中文时，个别远古版本的 LWJGL 2 可能无法加载 natives，建议在设置中把游戏目录改到纯英文路径。
- 崩溃分析基于关键字匹配，只覆盖常见情况。
- 第四阶段的限制：
  - 愚人节名单需要每年手动补充；规则兜底只覆盖“ID 不像常规快照”的新愚人节版本，像 `15w14a` / `26w14a` 这样沿用常规格式的需要加进名单（前后端两处）。
  - 外部版本（其他启动器复制来的）如果版本 JSON 里既没有 `clientVersion` 也没有 `inheritsFrom`，就按文件夹名判断；把 4 月 1 日前后发布的普通快照放进名字很特别的文件夹时，规则可能把它误判为愚人节版本（只影响图标与标签）。
  - 愚人节版本的显示名称使用官方英文名。
- 第三阶段的限制：
  - Fabric / Quilt 的版本列表改用 `/versions/loader` + `/versions/game` 判断是否支持该游戏版本；这两个接口失败时回退到按版本的接口。
  - “官方优先”模式下 Forge 版本列表可能来自 BMCLAPI（谁先返回用谁），刚发布的 Forge 构建可能要等镜像同步后才出现；需要时可切到“官方”。
  - 加载器版本缓存 1 小时，期间新发布的版本需要等缓存过期；加载失败的卡片再次点击会强制刷新。
- 第二阶段的限制：
  - Forge 远古版本（没有安装器的 1.5.x 以前）不支持；1.7.10 等旧版的部分 Scala 库在 Forge maven 上只有 `.pack.xz`，BMCLAPI 有 jar，只用官方源时可能下载失败。
  - OptiFine 单独安装使用 launchwrapper，旧版官方 `launchwrapper:1.12` 只能在 Java 8 上运行（OptiFine 1.13+ 自带的 launchwrapper-of 无此问题）。
  - OptiFine 与 NeoForge 不能共存（OptiFine 官方不支持）；Fabric / Quilt 上请使用 OptiFabric 或 Sodium / Iris。
  - CurseForge 的版本列表只取最近 50 个文件；“全部”平台模式下两边各取一页交替显示。
  - 地图只来自 CurseForge（Modrinth 没有地图类型）；没有 Key 时可以本地导入。
  - mod 图标从 jar 中读取（≤ 256 KB），mod 数量很多时首次打开资源列表需要一两秒。
  - mods.toml 使用极简解析（只读取字符串键），特殊写法的 TOML 可能读不到名称，此时显示文件名。
  - 拖放导入需要 `dragDropEnabled: true`；在 Windows 上开启后 WebView 内的 HTML5 拖放不可用（本启动器没有用到）。

---

## 自动构建与发布（GitHub Actions）

工作流文件：`.github/workflows/build.yml`，在 GitHub 的 Windows 环境里编译。

| 触发方式 | 结果 |
| --- | --- |
| 推送到 `main`、提交 PR、在 Actions 页面手动运行 | 编译，并在该次运行的页面上提供构建产物下载（保留 14 天） |
| 推送 `v*` 标签（如 `v0.1.0`） | 另外创建一个 **Release 草稿**，附上 `pilot-launcher.exe`（免安装）和 `*-setup.exe`（安装包） |

**第一次使用前：**
1. 仓库 → Settings → Secrets and variables → Actions → New repository secret，名称填 `CURSEFORGE_API_KEY`，值填你的 CurseForge API Key。
   构建时 Key 会内置进程序（和其他启动器的做法一样，二进制里的 Key 可被提取，这是 CurseForge 第三方启动器 Key 的正常用法）。
   **源码里不含真实 Key。** 没配置这个 Secret 时照样能构建，只是发布版里 CurseForge 不可用。
2. 发布新版本时：先把 `src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`、`package.json` 里的版本号改成一样的（如 `0.2.0`），提交后：
   ```powershell
   git tag v0.2.0
   git push origin v0.2.0
   ```
   标签和版本号不一致时，工作流会直接报错。构建完成后到 Releases 页面检查草稿，点 Publish 正式发布。

本地构建仍然用 `scripts\build.bat`；本地的 `config.local.json` 优先于内置 Key。

## 许可证

- 源代码以 [MIT 许可证](LICENSE) 发布。
- `src/assets/icons/` 里的方块图标用 Minecraft 原版贴图渲染，**版权归 Mojang Studios / Microsoft**，不适用 MIT 许可；其他第三方素材、运行时下载的程序（陶瓦联机、authlib-injector）和使用的在线服务见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
- NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## 致谢

- [PCL / Plain Craft Launcher](https://github.com/Meloong-Git/PCL)：功能与交互思路参考
- [BMCLAPI](https://bmclapidoc.bangbang93.com/)：下载镜像
- [MCIM](https://www.mcimirror.top)：Modrinth / CurseForge 镜像
- [HMCL](https://github.com/HMCL-dev/HMCL)：OptiFine 安装方式、镜像规则与陶瓦联机的集成流程参考
- [Modrinth](https://modrinth.com/)、[CurseForge](https://www.curseforge.com/)：社区资源
- [authlib-injector](https://github.com/yushijinhun/authlib-injector)：外置登录
- [Terracotta | 陶瓦联机](https://github.com/burningtnt/Terracotta)（© burningtnt 及贡献者，AGPL-3.0-or-later）：联机功能；通过其 HTTP 接口与未修改的程序交互
- [EasyTier](https://github.com/EasyTier/EasyTier)：陶瓦联机使用的 P2P 组网
- [Tauri](https://tauri.app/)、[Vue](https://vuejs.org/)
- [QuiltMC/art](https://github.com/QuiltMC/art)：Quilt logo（CC0）
- [中文 Minecraft Wiki](https://zh.minecraft.wiki/)：愚人节版本资料

本项目与 Mojang Studios / Microsoft 无关。Minecraft 是 Mojang Synergies AB 的商标；界面中使用的原版方块图片由用户提供，版权归 Mojang Studios 所有。
