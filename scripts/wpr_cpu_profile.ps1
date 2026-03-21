<#
.SYNOPSIS
    WPR CPU Profiling Script for memex-cli
.DESCRIPTION
    Uses Windows Performance Recorder (WPR) to capture CPU hot spots
    during memex-cli multi-task execution
.NOTES
    Requires administrator privileges for WPR
#>

param(
    [string]$OutputDir = ".\wpr_traces",
    [string]$TraceName = "memex_cli_cpu_profile",
    [int]$WarmupRuns = 1,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

# Colors for output
function Write-Info { Write-Host "[INFO] $args" -ForegroundColor Cyan }
function Write-Success { Write-Host "[OK] $args" -ForegroundColor Green }
function Write-Warn { Write-Host "[WARN] $args" -ForegroundColor Yellow }
function Write-Err { Write-Host "[ERROR] $args" -ForegroundColor Red }

# Check admin privileges
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Err "This script requires administrator privileges for WPR."
    Write-Info "Please run PowerShell as Administrator and try again."
    exit 1
}

# Setup
$ProjectRoot = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $ProjectRoot "Cargo.toml"))) {
    # Fallback to current directory if script is run from project root
    $ProjectRoot = Get-Location
}
if (-not (Test-Path (Join-Path $ProjectRoot "Cargo.toml"))) {
    Write-Err "Cannot find Cargo.toml. Please run from memex_cli directory."
    exit 1
}
Set-Location $ProjectRoot
Write-Info "Project root: $ProjectRoot"

# Create output directory
$OutputDir = Join-Path $ProjectRoot $OutputDir
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}
Write-Info "Output directory: $OutputDir"

# Build release binary (with debug info for profiling)
if (-not $SkipBuild) {
    Write-Info "Building release binary with debug symbols..."
    $env:RUSTFLAGS = "-C debuginfo=2"
    cargo build -p memex-cli --profile profiling 
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Build failed"
        exit 1
    }
    Write-Success "Build completed"
}

$BinaryPath = Join-Path $ProjectRoot "target\profiling\memex-cli.exe"
# if (-not (Test-Path $BinaryPath)) {
#     Write-Warn "Release binary not found, using debug binary..."
#     $BinaryPath = Join-Path $ProjectRoot "target\debug\memex-cli.exe"
# }
Write-Info "Using binary: $BinaryPath"

# Create test input file
$TestInput = @"
---TASK---
id: task1-fib
backend: codex
workdir: $ProjectRoot
---CONTENT---
Calculate fibonacci(10) step by step
---END---

---TASK---
id: task2-dfs
backend: codex
workdir: $ProjectRoot
---CONTENT---
Explain DFS algorithm with pseudocode
---END---
"@

$TestInputFile = Join-Path $OutputDir "test_input.txt"
$TestInput | Out-File -FilePath $TestInputFile -Encoding UTF8
Write-Info "Test input written to: $TestInputFile"

# Warmup runs
Write-Info "Performing $WarmupRuns warmup run(s)..."
for ($i = 1; $i -le $WarmupRuns; $i++) {
    Write-Info "  Warmup run $i/$WarmupRuns..."
    $null = Get-Content $TestInputFile | & $BinaryPath run --backend "codex" --stream-format "text" --stdin 2>&1
}
Write-Success "Warmup completed"

# Create WPR profile for CPU analysis
$WprProfile = Join-Path $OutputDir "cpu_profile.wprp"
@"
<?xml version="1.0" encoding="utf-8"?>
<WindowsPerformanceRecorder Version="1.0">
  <Profiles>
    <SystemCollector Id="SystemCollector" Name="NT Kernel Logger">
      <BufferSize Value="1024"/>
      <Buffers Value="64"/>
    </SystemCollector>

    <EventCollector Id="EventCollector" Name="Event Collector">
      <BufferSize Value="1024"/>
      <Buffers Value="64"/>
    </EventCollector>

    <SystemProvider Id="SystemProvider">
      <Keywords>
        <Keyword Value="CpuConfig"/>
        <Keyword Value="CSwitch"/>
        <Keyword Value="DiskIO"/>
        <Keyword Value="DPC"/>
        <Keyword Value="HardFaults"/>
        <Keyword Value="Interrupt"/>
        <Keyword Value="Loader"/>
        <Keyword Value="MemoryInfo"/>
        <Keyword Value="ProcessCounter"/>
        <Keyword Value="ProcessThread"/>
        <Keyword Value="ReadyThread"/>
        <Keyword Value="SampledProfile"/>
        <Keyword Value="ThreadPriority"/>
      </Keywords>
      <Stacks>
        <Stack Value="CSwitch"/>
        <Stack Value="ReadyThread"/>
        <Stack Value="SampledProfile"/>
      </Stacks>
    </SystemProvider>

    <Profile Id="CpuProfile.Verbose.File" Name="CpuProfile" Description="CPU Profiling" LoggingMode="File" DetailLevel="Verbose">
      <Collectors>
        <SystemCollectorId Value="SystemCollector">
          <SystemProviderId Value="SystemProvider"/>
        </SystemCollectorId>
      </Collectors>
    </Profile>
  </Profiles>
</WindowsPerformanceRecorder>
"@ | Out-File -FilePath $WprProfile -Encoding UTF8
Write-Info "WPR profile created: $WprProfile"

# Start WPR tracing
$TraceFile = Join-Path $OutputDir "$TraceName.etl"
Write-Info "Starting WPR trace..."
wpr -start $WprProfile -filemode
if ($LASTEXITCODE -ne 0) {
    Write-Err "Failed to start WPR"
    exit 1
}
Write-Success "WPR trace started"

# Run the actual profiling
Write-Info "Running memex-cli for profiling..."
$sw = [Diagnostics.Stopwatch]::StartNew()

try {
    Get-Content $TestInputFile | & $BinaryPath run --backend "claude" --stream-format "text" --stdin
}
catch {
    Write-Warn "Command exited with error (may be expected for test): $_"
}

$sw.Stop()
Write-Info "Execution time: $($sw.ElapsedMilliseconds)ms"

# Stop WPR and save trace
Write-Info "Stopping WPR trace and saving to: $TraceFile"
wpr -stop $TraceFile
if ($LASTEXITCODE -ne 0) {
    Write-Err "Failed to stop WPR"
    exit 1
}
Write-Success "Trace saved to: $TraceFile"

# Generate summary report
$ReportFile = Join-Path $OutputDir "analysis_report.txt"
@"
================================================================================
                      MEMEX-CLI CPU PROFILING REPORT
================================================================================
Date: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
Binary: $BinaryPath
Trace File: $TraceFile

EXECUTION METRICS
-----------------
Total Execution Time: $($sw.ElapsedMilliseconds)ms
Task Count: 2 (fibonacci + DFS)

NEXT STEPS
----------
1. Open trace in Windows Performance Analyzer (WPA):
   wpa "$TraceFile"

2. Navigate to: Computation > CPU Usage (Sampled)

3. Look for hot spots in:
   - memex_cli.exe module
   - tokio runtime functions
   - serde serialization/deserialization
   - reqwest HTTP client operations

4. Key columns to analyze:
   - Weight (% CPU)
   - Count (sample hits)
   - Module + Function

TYPICAL OPTIMIZATION TARGETS FOR RUST CLI
------------------------------------------
- Synchronous I/O in async context
- Excessive allocations in hot paths
- String formatting/parsing overhead
- Serialization/deserialization bottlenecks
- Lock contention in concurrent code

KNOWN HOT PATHS IN MEMEX-CLI
-----------------------------
Based on code analysis:
1. cli/src/flow/flow_standard.rs:179 - execute_stdio_tasks
2. core/src/engine/run.rs:40 - pre_run (memory search)
3. core/src/executor/engine.rs - parallel task execution
4. plugins/src/backend/ - backend strategy execution
================================================================================
"@ | Out-File -FilePath $ReportFile -Encoding UTF8

Write-Success "Analysis report: $ReportFile"
Write-Info ""
Write-Info "To analyze the trace:"
Write-Host "  wpa `"$TraceFile`"" -ForegroundColor Yellow
Write-Info ""
Write-Info "Or use xperf for command-line analysis:"
Write-Host "  xperf -i `"$TraceFile`" -o `"$OutputDir\summary.txt`" -a dumper" -ForegroundColor Yellow
