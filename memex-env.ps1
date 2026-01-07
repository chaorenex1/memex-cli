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
    memex-env.ps1 [ENVIRONMENT|COMMAND]

COMMANDS:
    (none)          Load default .env file
    dev             Load .env.dev
    prod            Load .env.prod
    test            Load .env.test
    -List           List all available .env files
    -Help           Show this help message

EXAMPLES:
    .\memex-env.ps1               # Load default .env
    .\memex-env.ps1 dev           # Load .env.dev
    .\memex-env.ps1 prod          # Load .env.prod
    .\memex-env.ps1 -List         # List all .env files

ENVIRONMENT:
    USERPROFILE\.memex            Directory containing .env files

FILES:
    %USERPROFILE%\.memex\.env      Default environment file
    %USERPROFILE%\.memex\.env.*    Environment-specific files

SECURITY:
    - Files are validated for path traversal attacks
    - Invalid variable names are rejected
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

function Read-EnvFile {
    param([string]$EnvFile)

    Write-ColorOutput "Loading environment from: $EnvFile" -Type Info

    $envVars = @{}
    $lineNum = 0

    Get-Content -Path $EnvFile -Encoding UTF8 | ForEach-Object {
        $lineNum++
        $line = $_.Trim()

        # Skip empty lines and comments
        if ([string]::IsNullOrWhiteSpace($line) -or $line.StartsWith('#')) {
            return
        }

        # Parse KEY=VALUE format
        if ($line -match '^([A-Za-z_][A-Za-z0-9_]*)=(.*)$') {
            $key = $matches[1]
            $value = $matches[2]

            # Remove surrounding quotes if present
            $value = $value -replace '^"(.*)"$', '$1'
            $value = $value -replace "^'(.*)'$", '$1'

            $envVars[$key] = $value
        }
        else {
            Write-ColorOutput "Skipping invalid line $lineNum`: $line" -Type Warning
        }
    }

    Write-ColorOutput "Environment loaded successfully" -Type Success

    return $envVars
}

function Start-NewPowerShellSession {
    param(
        [hashtable]$EnvVars,
        [string]$EnvFile
    )

    Write-ColorOutput "Starting new PowerShell session..." -Type Info

    # Create a temporary startup script
    $tempScript = [System.IO.Path]::GetTempFileName() + ".ps1"

    # Build startup script content
    $scriptContent = @"
# Set UTF-8 encoding
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
`$OutputEncoding = [System.Text.Encoding]::UTF8

# Set environment variables
"@

    foreach ($key in $EnvVars.Keys) {
        $value = $EnvVars[$key]
        $scriptContent += "`n`$env:$key = '$value'"
    }

    $scriptContent += @"


# Display loaded environment
Write-Host ""
Write-Host "╔════════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  memex-env: Environment Variables Loaded                      ║" -ForegroundColor Cyan
Write-Host "╚════════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "Loaded from: $EnvFile" -ForegroundColor Green
Write-Host "UTF-8 encoding: enabled" -ForegroundColor Green
Write-Host ""
Write-Host "Type 'exit' to return to the original shell" -ForegroundColor Yellow
Write-Host ""

# Change to original directory
Set-Location '$PWD'

# Clean up temp script on exit
Register-EngineEvent -SourceIdentifier PowerShell.Exiting -Action {
    Remove-Item -Path '$tempScript' -Force -ErrorAction SilentlyContinue
} | Out-Null

"@

    # Write script to temp file
    Set-Content -Path $tempScript -Value $scriptContent -Encoding UTF8

    # Start new PowerShell session with temp script
    try {
        Start-Process -FilePath "powershell.exe" -ArgumentList "-NoExit", "-ExecutionPolicy", "Bypass", "-File", "`"$tempScript`"" -WorkingDirectory $PWD
        Write-ColorOutput "New PowerShell session started successfully" -Type Success
        exit 0
    }
    catch {
        Write-ColorOutput "Failed to start new PowerShell session: $_" -Type Error
        Remove-Item -Path $tempScript -Force -ErrorAction SilentlyContinue
        exit 1
    }
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

    # Get environment file path
    $envFile = Get-EnvFilePath -EnvName $Environment

    # Validate environment file
    if (-not (Test-EnvFile -FilePath $envFile)) {
        Write-ColorOutput "Run 'memex-env.ps1 -List' to see available environments" -Type Info
        exit 1
    }

    # Read environment variables
    $envVars = Read-EnvFile -EnvFile $envFile

    # Start new PowerShell session
    Start-NewPowerShellSession -EnvVars $envVars -EnvFile $envFile
}

# ============================================================================
# Entry Point
# ============================================================================

Main
