@echo off
setlocal
rem PilotLauncher build helper (ASCII only). Calls build.ps1 with ExecutionPolicy Bypass.
rem Usage: scripts\build.bat [-Debug] [-Bundle nsis|msi|none] [-SkipInstall] [-NpmMirror]
cd /d "%~dp0.."
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
set "ERR=%ERRORLEVEL%"
echo.
if "%ERR%"=="0" (
  echo Build finished. See the "release" folder.
) else (
  echo Build failed with exit code %ERR%.
)
pause
exit /b %ERR%
