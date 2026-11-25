#Requires -Version 5.1
<#
.SYNOPSIS
  PilotLauncher 构建脚本：npm install → tauri build，产出 exe / msi / nsis 安装包。

.PARAMETER Debug
  构建调试版本（不优化，速度更快）。

.PARAMETER Bundle
  打包格式：none（默认，只编译免安装 exe）、all（nsis + msi）、nsis、msi。
  注意：打包安装程序需要从 GitHub 下载 NSIS/WiX，国内网络可能超时。

.PARAMETER SkipInstall
  跳过 npm install。

.PARAMETER NpmMirror
  使用 npmmirror 国内镜像安装前端依赖。

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\build.ps1
  powershell -ExecutionPolicy Bypass -File scripts\build.ps1 -Bundle nsis -NpmMirror
#>
param(
    [switch]$Debug,
    [ValidateSet('all', 'nsis', 'msi', 'none')]
    [string]$Bundle = 'none',
    [switch]$SkipInstall,
    [switch]$NpmMirror
)

$ErrorActionPreference = 'Stop'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Write-Step([string]$text) { Write-Host "`n==> $text" -ForegroundColor Cyan }
function Write-Ok([string]$text) { Write-Host "  [OK] $text" -ForegroundColor Green }
function Write-Warn([string]$text) { Write-Host "  [!] $text" -ForegroundColor Yellow }
function Fail([string]$text) {
    Write-Host "`n[错误] $text" -ForegroundColor Red
    exit 1
}

function Update-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $env:Path = "$machine;$user"
    $extra = @("$env:USERPROFILE\.cargo\bin", 'D:\Git\cmd', 'D:\Git\bin', "$env:ProgramFiles\nodejs")
    foreach ($p in $extra) {
        if ((Test-Path $p) -and ($env:Path -notlike "*$p*")) { $env:Path = "$p;$env:Path" }
    }
}

function Require-Command([string]$name, [string]$hint) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if (-not $cmd) { Fail "未找到 $name。$hint" }
    return $cmd
}

Write-Host "PilotLauncher 构建脚本" -ForegroundColor White
Write-Host "项目目录：$Root"

# ---------- 1. 环境检查 ----------
Write-Step '检查构建环境'
Update-SessionPath
$hint = '请先双击 scripts\setup-env.bat 安装环境，然后重新打开终端。'
Require-Command 'node' $hint | Out-Null
Require-Command 'npm' $hint | Out-Null
Require-Command 'cargo' $hint | Out-Null
Require-Command 'rustc' $hint | Out-Null
Write-Ok ("Node.js " + (node --version))
Write-Ok ("npm " + (npm --version))
Write-Ok ((rustc --version) -join ' ')

$host_triple = (rustc -vV | Select-String '^host:').ToString().Replace('host:', '').Trim()
if ($host_triple -ne 'x86_64-pc-windows-msvc') {
    Write-Warn "当前 Rust 默认工具链为 $host_triple，建议执行：rustup default stable-x86_64-pc-windows-msvc"
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($vs) { Write-Ok "MSVC 生成工具：$vs" } else { Write-Warn '未检测到 MSVC C++ 生成工具，链接阶段可能失败。' }
} else {
    Write-Warn '未找到 vswhere，无法确认 Visual Studio Build Tools 是否已安装。'
}
$sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Lib'
if (Test-Path $sdk) { Write-Ok 'Windows SDK 已安装' } else { Write-Warn '未检测到 Windows SDK（Windows Kits\10）。' }

# ---------- 2. 前端依赖 ----------
if ($NpmMirror) {
    $env:npm_config_registry = 'https://registry.npmmirror.com'
    Write-Ok '使用 npmmirror 镜像'
}
if (-not $SkipInstall) {
    Write-Step '安装前端依赖（npm install）'
    npm install
    if ($LASTEXITCODE -ne 0) { Fail 'npm install 失败，请检查网络（可加 -NpmMirror 使用国内镜像）。' }
}

# ---------- 3. Tauri 构建 ----------
Write-Step '构建 PilotLauncher（tauri build）'
$tauriArgs = @('run', 'tauri', '--', 'build')
if ($Debug) { $tauriArgs += '--debug' }
switch ($Bundle) {
    'nsis' { $tauriArgs += @('--bundles', 'nsis') }
    'msi' { $tauriArgs += @('--bundles', 'msi') }
    'none' { $tauriArgs += '--no-bundle' }
    default { }
}
Write-Host ("  npm " + ($tauriArgs -join ' '))
& npm @tauriArgs
if ($LASTEXITCODE -ne 0) {
    $profileDir = if ($Debug) { 'debug' } else { 'release' }
    $builtExe = Join-Path $Root "src-tauri\target\$profileDir\pilot-launcher.exe"
    if (($Bundle -ne 'none') -and (Test-Path $builtExe) -and ((Get-Item $builtExe).LastWriteTime -gt (Get-Date).AddMinutes(-30))) {
        Write-Warn '程序已编译成功，但打包安装程序失败（多半是从 GitHub 下载 NSIS/WiX 超时）。继续收集免安装 exe。'
    } else {
        Fail "tauri build 失败（退出码 $LASTEXITCODE）。请查看上方 Rust / Node 输出。"
    }
}

# ---------- 4. 收集产物 ----------
Write-Step '收集构建产物'
$buildProfile = if ($Debug) { 'debug' } else { 'release' }
$target = Join-Path $Root "src-tauri\target\$buildProfile"
$out = Join-Path $Root 'release'
New-Item -ItemType Directory -Force -Path $out | Out-Null
# 软件已从 GlassLauncher 改名为 PilotLauncher：删掉 release 里旧名字的 exe，避免误开
$oldExe = Join-Path $out 'glass-launcher.exe'
if (Test-Path $oldExe) { Remove-Item $oldExe -Force -ErrorAction SilentlyContinue; Write-Ok '已删除旧的 glass-launcher.exe' }

$files = @()
$exe = Join-Path $target 'pilot-launcher.exe'
if (Test-Path $exe) { $files += Get-Item $exe }
foreach ($kind in @('msi', 'nsis')) {
    $dir = Join-Path $target "bundle\$kind"
    if (Test-Path $dir) { $files += Get-ChildItem $dir -File | Where-Object { $_.Extension -in '.msi', '.exe' } }
}
if ($files.Count -eq 0) { Fail '没有找到任何构建产物。' }
foreach ($f in $files) {
    Copy-Item $f.FullName -Destination $out -Force
    $size = '{0:N1} MB' -f ($f.Length / 1MB)
    Write-Ok "$($f.Name)  ($size)"
}
$example = Join-Path $Root 'config.local.example.json'
if (Test-Path $example) { Copy-Item $example -Destination $out -Force }
$localCfg = Join-Path $Root 'config.local.json'
if (Test-Path $localCfg) { Copy-Item $localCfg -Destination $out -Force; Write-Ok 'config.local.json（CurseForge Key）已复制' }

Write-Host "`n构建完成！产物已复制到：$out" -ForegroundColor Green
Write-Host '提示：pilot-launcher.exe 为免安装版本（需要系统已有 WebView2）；*-setup.exe / *.msi 为安装包。'
