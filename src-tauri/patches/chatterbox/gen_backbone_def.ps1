param(
    [Parameter(Mandatory=$true)]
    [string]$Archives,

    [Parameter(Mandatory=$true)]
    [string]$DefPath
)

$ErrorActionPreference = "Stop"

# Split comma-separated archive paths
$ArchiveList = $Archives -split ','

# Find dumpbin.exe from the same toolchain that CMake discovered.
$dumpbinCandidates = @()
$vsRoot = "C:\Program Files\Microsoft Visual Studio\2022"
if (Test-Path $vsRoot) {
    $dumpbinCandidates += Get-ChildItem "$vsRoot\*\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName
}
$dumpbinCandidates += "dumpbin"

$dumpbin = $null
foreach ($c in $dumpbinCandidates) {
    try {
        & $c /? 2>&1 | Out-Null
        $dumpbin = $c
        break
    } catch { }
}
if (-not $dumpbin) { throw "dumpbin.exe not found" }

$symbols = [System.Collections.Generic.SortedSet[string]]::new()

foreach ($arch in $ArchiveList) {
    if (-not (Test-Path -LiteralPath $arch)) {
        Write-Warning "Archive not found: $arch"
        continue
    }
    Write-Host "Scanning $arch ..."
    $output = & $dumpbin /symbols $arch 2>&1
    foreach ($line in $output) {
        $lineStr = "$line"
        # Only DEFINED external symbols (SECT* section, not UNDEF)
        if ($lineStr -notmatch 'External') { continue }
        if ($lineStr -match 'UNDEF') { continue }
        # Extract symbol name after the pipe character
        if ($lineStr -match '\|\s+(.+)$') {
            $sym = $matches[1].Trim()
            # Split on space to get just the decorated name (before any parenthesized demangled text)
            $sym = $sym -split '\s+'
            # Match: llama_* (undecorated C), ?common_sampler* (C++ mangled), ?common_grammar* (C++ mangled)
            if ($sym[0] -match '^llama_\w+' -or $sym[0] -match '^\?common_sampler' -or $sym[0] -match '^\?common_grammar') {
                [void]$symbols.Add($sym[0])
            }
        }
    }
}

$lines = @("LIBRARY ttsbackbone", "EXPORTS")
foreach ($s in $symbols) {
    $lines += "    $s"
}
$lines += ""

Set-Content -Path $DefPath -Value $lines -Encoding ASCII
Write-Host "Generated $DefPath with $($symbols.Count) exports"
