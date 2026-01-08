# memex-env.ps1: Cross-platform environment loader for .memex directory
# Version: 1.0.0
# Usage: memex-env.ps1 [env_name|list]
#
# This script loads environment variables from $HOME\.memex\.env files
# and opens a new PowerShell session with those variables loaded.

[CmdletBinding()]
param(
    [Parameter(Position=0)]
    [string]$Environment = "",

    [Parameter()]
    [switch]$Source,

    [Parameter()]
    [switch]$List,

    [Parameter()]
    [switch]$Help
)

# ============================================================================
# Configuration
# ============================================================================

$ErrorActionPreference = "Stop"
$MemexDir = Join-Path $env:USERPROFILE ".memex"
$DefaultEnvFile = ".env"
$EnvPrefix = ".env."

# ============================================================================
# Helper Functions
# ============================================================================

function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Type = "Info"
    )

    switch ($Type) {
        "Info"    { Write-Host "[INFO] " -ForegroundColor Blue -NoNewline; Write-Host $Message }
        "Success" { Write-Host "[SUCCESS] " -ForegroundColor Green -NoNewline; Write-Host $Message }
        "Warning" { Write-Host "[WARNING] " -ForegroundColor Yellow -NoNewline; Write-Host $Message }
        "Error"   { Write-Host "[ERROR] " -ForegroundColor Red -NoNewline; Write-Host $Message }
    }
}

function Show-Help {
    @"
memex-env - Environment Variable Loader

USAGE:
    memex-env.ps1 [ENVIRONMENT] [OPTIONS]

OPTIONS:
    -Source         Load in current session (recommended)
    -List           List all available .env files
    -Help           Show this help message

COMMANDS:
    (none)          Load default .env file
    dev             Load .env.dev
    prod            Load .env.prod
    test            Load .env.test

EXAMPLES:
    .\memex-env.ps1                   # Load default .env in current session
    .\memex-env.ps1 dev               # Load .env.dev in current session
    .\memex-env.ps1 dev -Source       # Load .env.dev (explicit)
    .\memex-env.ps1 -List             # List all .env files

BEHAVIOR:
    Loads environment variables into the CURRENT PowerShell session.
    Variables persist until you close the terminal window.

ENVIRONMENT:
    USERPROFILE\.memex            Directory containing .env files

FILES:
    %USERPROFILE%\.memex\.env      Default environment file
    %USERPROFILE%\.memex\.env.*    Environment-specific files

SECURITY:
    - Files are validated for path traversal attacks
    - Invalid variable names are rejected
    - Protected system variables (PATH, USERPROFILE, etc.) are skipped
    - UTF-8 encoding is enforced

"@
}

# ============================================================================
# Security Functions
# ============================================================================

function Test-ValidEnvName {
    param([string]$EnvName)

    # Check for path traversal attempts
    if ($EnvName -match '\.\.|[\\/]') {
        Write-ColorOutput "Invalid environment name: '$EnvName' (path traversal detected)" -Type Error
        return $false
    }

    # Check for invalid characters
    if ($EnvName -notmatch '^[a-zA-Z0-9_-]+$') {
        Write-ColorOutput "Invalid environment name: '$EnvName' (only alphanumeric, underscore, and hyphen allowed)" -Type Error
        return $false
    }

    return $true
}

function Test-FilePermissions {
    param([string]$FilePath)

    # Get file ACL
    try {
        $acl = Get-Acl -Path $FilePath
        $everyone = [System.Security.Principal.SecurityIdentifier]::new("S-1-1-0")

        # Check if Everyone has write access
        $everyoneRules = $acl.Access | Where-Object {
            $_.IdentityReference.Translate([System.Security.Principal.SecurityIdentifier]) -eq $everyone
        }

        if ($everyoneRules | Where-Object { $_.FileSystemRights -match "Write|FullControl" }) {
            Write-ColorOutput "File '$FilePath' has overly permissive access (Everyone can write)" -Type Warning
            Write-ColorOutput "Consider restricting file permissions" -Type Warning
        }
    }
    catch {
        # Silently ignore permission check errors
    }
}

function Test-EnvFile {
    param([string]$FilePath)

    # Check if directory exists
    if (-not (Test-Path -Path $MemexDir -PathType Container)) {
        Write-ColorOutput "Directory '$MemexDir' does not exist" -Type Error
        Write-ColorOutput "Creating directory: $MemexDir" -Type Info
        New-Item -Path $MemexDir -ItemType Directory -Force | Out-Null
        return $false
    }

    # Check if file exists
    if (-not (Test-Path -Path $FilePath -PathType Leaf)) {
        Write-ColorOutput "Environment file '$FilePath' not found" -Type Error
        return $false
    }

    # Check if file is readable
    try {
        Get-Content -Path $FilePath -TotalCount 1 | Out-Null
    }
    catch {
        Write-ColorOutput "Cannot read file '$FilePath' (access denied)" -Type Error
        return $false
    }

    # Check file permissions
    Test-FilePermissions -FilePath $FilePath

    return $true
}

# ============================================================================
# Core Functions
# ============================================================================

function Show-EnvFiles {
    Write-ColorOutput "Available environment files in '$MemexDir':" -Type Info
    Write-Host ""

    $count = 0

    # List default .env
    $defaultPath = Join-Path $MemexDir $DefaultEnvFile
    if (Test-Path -Path $defaultPath -PathType Leaf) {
        Write-Host "  • default ($DefaultEnvFile)"
        $count++
    }

    # List all .env.* files
    $envFiles = Get-ChildItem -Path $MemexDir -Filter "$EnvPrefix*" -File -ErrorAction SilentlyContinue
    foreach ($file in $envFiles) {
        $envName = $file.Name -replace "^$([regex]::Escape($EnvPrefix))", ""
        Write-Host "  • $envName ($($file.Name))"
        $count++
    }

    if ($count -eq 0) {
        Write-ColorOutput "No .env files found in '$MemexDir'" -Type Warning
        Write-ColorOutput "Create a .env file with: echo 'VAR=value' > $(Join-Path $MemexDir '.env')" -Type Info
    }

    Write-Host ""
}

function Get-EnvFilePath {
    param([string]$EnvName)

    if ([string]::IsNullOrEmpty($EnvName)) {
        return Join-Path $MemexDir $DefaultEnvFile
    }
    else {
        return Join-Path $MemexDir "$EnvPrefix$EnvName"
    }
}

function Load-EnvInCurrentSession {
    param([string]$EnvFile)

    Write-ColorOutput "Loading environment in CURRENT session..." -Type Info
    Write-Host ""

    # UTF-8 encoding
    [Console]::OutputEncoding = [Text.UTF8Encoding]::UTF8
    $OutputEncoding = [Text.UTF8Encoding]::UTF8

    # Protected system variables
    $protected = @("PATH", "HOME", "USERPROFILE", "TEMP", "TMP", "COMPUTERNAME", "USERNAME")
    $script:count = 0

    # Load variables (simplified pattern from start_with_env.ps1)
    Get-Content $EnvFile -Encoding UTF8 |
        Where-Object { $_ -match "^\s*[^#].+=.+$" } |
        ForEach-Object {
            $k, $v = ($_ -split "=", 2)
            $k = $k.Trim()
            $v = $v.Trim()

            if ($protected -contains $k) {
                Write-ColorOutput "Skipping protected: $k" -Type Warning
                return
            }

            Set-Item -Path "env:$k" -Value $v
            Write-Host "  ✓ $k" -ForegroundColor Green
            $script:count++
        }

    Write-Host ""
    Write-ColorOutput "Loaded from: $EnvFile" -Type Success
    Write-ColorOutput "$script:count variables exported to CURRENT session" -Type Success

    # Set tracking variables
    $env:MEMEX_ENV_LOADED = Split-Path -Leaf $EnvFile
    $env:MEMEX_ENV_FILE = $EnvFile
    $env:MEMEX_ENV_MODE = "source"
}

# ============================================================================
# Main Function
# ============================================================================

function Main {
    # Show help
    if ($Help) {
        Show-Help
        exit 0
    }

    # List environments
    if ($List) {
        Show-EnvFiles
        exit 0
    }

    # Validate environment name
    if (-not [string]::IsNullOrEmpty($Environment)) {
        if (-not (Test-ValidEnvName -EnvName $Environment)) {
            exit 1
        }
    }

    # Get and validate environment file
    $envFile = Get-EnvFilePath -EnvName $Environment
    if (-not (Test-EnvFile -FilePath $envFile)) {
        $sq = [char]39
        $msg = 'Run ' + $sq + 'memex-env.ps1 -List' + $sq + ' to see available environments'
        Write-ColorOutput $msg -Type Info
        exit 1
    }

    # Load environment in current session
    Load-EnvInCurrentSession -EnvFile $envFile
}

# ============================================================================
# Entry Point
# ============================================================================

Main
