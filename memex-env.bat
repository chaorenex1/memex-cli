@echo off
REM memex-env.bat: Environment loader wrapper for PowerShell script
REM Version: 1.0.0
REM
REM This is a simple wrapper that calls the PowerShell version.
REM For full functionality, use memex-env.ps1 directly.

setlocal enabledelayedexpansion

REM Get the directory where this batch file is located
set "SCRIPT_DIR=%~dp0"

REM Check if PowerShell is available
where powershell >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] PowerShell is not found in PATH
    echo Please ensure PowerShell is installed and accessible
    exit /b 1
)

REM Call the PowerShell script with all arguments
powershell.exe -ExecutionPolicy Bypass -File "%SCRIPT_DIR%memex-env.ps1" %*

exit /b %errorlevel%
