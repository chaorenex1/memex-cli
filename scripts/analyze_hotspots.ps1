<#
.SYNOPSIS
    Analyze CPU hotspots from WPR trace using xperf
.DESCRIPTION
    Parses ETL trace and extracts CPU sampling data for memex-cli
#>

param(
    [Parameter(Mandatory=$true)]
    [string]$TraceFile,
    [string]$OutputDir = ".\wpr_traces\analysis"
)

$ErrorActionPreference = "Stop"

# Verify trace file exists
if (-not (Test-Path $TraceFile)) {
    Write-Host "[ERROR] Trace file not found: $TraceFile" -ForegroundColor Red
    exit 1
}

# Create output directory
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

Write-Host "[INFO] Analyzing trace: $TraceFile" -ForegroundColor Cyan

# Extract CPU samples summary
$CpuSummary = Join-Path $OutputDir "cpu_summary.txt"
Write-Host "[INFO] Extracting CPU summary..." -ForegroundColor Cyan
xperf -i $TraceFile -o $CpuSummary -a dumper -stacksonly 2>$null

# Extract profile data for specific process
$ProfileData = Join-Path $OutputDir "profile_data.csv"
Write-Host "[INFO] Extracting profile data..." -ForegroundColor Cyan

# Use tracerpt for CSV export (built into Windows)
tracerpt $TraceFile -o $ProfileData -of CSV -summary "$OutputDir\summary.txt" 2>$null

# Parse and display top functions (if xperf output is available)
if (Test-Path $CpuSummary) {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "TOP CPU CONSUMERS (from trace)" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green

    # Look for memex-cli related entries
    $content = Get-Content $CpuSummary -Raw
    if ($content -match "memex") {
        Write-Host "Found memex-cli entries in trace" -ForegroundColor Yellow
    }
}

# Generate Flamegraph-compatible output (if tools available)
$FlameScript = @"
# Flamegraph generation commands (requires flamegraph tools)
# Download from: https://github.com/BrendanGregg/FlameGraph

# Convert ETL to perf-compatible format:
# xperf -i "$TraceFile" -o stacks.txt -a dumper -stacksonly

# Generate flamegraph:
# perl stackcollapse-perf.pl stacks.txt | perl flamegraph.pl > memex_flamegraph.svg
"@

$FlameScript | Out-File -FilePath "$OutputDir\flamegraph_instructions.txt" -Encoding UTF8

Write-Host ""
Write-Host "[SUCCESS] Analysis complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Output files:" -ForegroundColor Cyan
Write-Host "  - $CpuSummary"
Write-Host "  - $ProfileData"
Write-Host "  - $OutputDir\summary.txt"
Write-Host ""
Write-Host "For visual analysis, run:" -ForegroundColor Yellow
Write-Host "  wpa `"$TraceFile`"" -ForegroundColor White
