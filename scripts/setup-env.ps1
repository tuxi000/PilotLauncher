# PilotLauncher 开发环境一键安装脚本
# 安装：Rust (MSVC 工具链)、Node.js LTS、VS 2022 生成工具 (C++ 桌面开发 + Windows 11 SDK)、WebView2
$ErrorActionPreference = 'Continue'

# ---- 自动提权 ----
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host '需要管理员权限，正在请求提权...' -ForegroundColor Yellow
    Start-Process powershell -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path $cargoBin) { $env:Path += ";$cargoBin" }
}
function Has($cmd) { return [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }
function Step($t) { Write-Host "`n==== $t ====" -ForegroundColor Cyan }

Step '检查 winget'
if (-not (Has 'winget')) {
    Write-Host '未找到 winget。请在 Microsoft Store 搜索并安装/更新 “应用安装程序 (App Installer)” 后重新运行本脚本。' -ForegroundColor Red
    Start-Process 'ms-windows-store://pdp/?ProductId=9NBLGGH4NNS1'
    Read-Host '按回车退出'; exit 1
}
$wg = @('--accept-source-agreements','--accept-package-agreements','--silent','-e')

Step '1/4 Visual Studio 2022 生成工具 (C++ / Windows 11 SDK)'
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasVC = $false
if (Test-Path $vswhere) {
    $r = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($r) { $hasVC = $true }
}
if ($hasVC) { Write-Host '已安装 MSVC 工具，跳过。' -ForegroundColor Green }
else {
    Write-Host '正在安装（约 3-6GB，耗时较长，请耐心等待）...'
    winget install --id Microsoft.VisualStudio.2022.BuildTools @wg --override "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621 --includeRecommended"
}

Step '2/4 WebView2 运行时'
$wv = Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue
if ($wv -and $wv.pv) { Write-Host "已安装 WebView2 $($wv.pv)，跳过。" -ForegroundColor Green }
else { winget install --id Microsoft.EdgeWebView2Runtime @wg }

Step '3/4 Node.js LTS'
Refresh-Path
if (Has 'node') { Write-Host "已安装 Node $(node --version)，跳过。" -ForegroundColor Green }
else { winget install --id OpenJS.NodeJS.LTS @wg; Refresh-Path }

Step '4/4 Rust (stable-x86_64-pc-windows-msvc)'
Refresh-Path
if (-not (Has 'rustup')) { winget install --id Rustlang.Rustup @wg; Refresh-Path }
if (Has 'rustup') {
    rustup default stable-x86_64-pc-windows-msvc
    rustup update stable
} else {
    Write-Host 'rustup 安装后未能在当前窗口找到，请关闭窗口后重新运行本脚本。' -ForegroundColor Yellow
}

Step '结果检查'
Refresh-Path
foreach ($c in 'node','npm','rustc','cargo') {
    if (Has $c) { Write-Host ("{0,-6} OK  {1}" -f $c, (& $c --version)) -ForegroundColor Green }
    else { Write-Host ("{0,-6} 缺失" -f $c) -ForegroundColor Red }
}
if (Test-Path $vswhere) {
    $r = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property displayName
    if ($r) { Write-Host "MSVC   OK  $r" -ForegroundColor Green } else { Write-Host 'MSVC   缺失' -ForegroundColor Red }
}
Write-Host "`n完成。若有缺失项，请重启电脑后再运行一次本脚本。" -ForegroundColor Cyan
Read-Host '按回车退出'
