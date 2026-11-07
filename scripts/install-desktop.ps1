# Install canopy desktop on Windows from a GitHub release.
#
#   irm https://gitcanopy.netlify.app/install-desktop.ps1 | iex
#
# To pick a version, download the script and run it:
#   .\install-desktop.ps1 -Version v1.0.0
#
# Downloads the installer for your CPU, checks its SHA-256, and runs it. Uninstall
# from Settings > Apps, or: winget uninstall shankars.canopy-desktop
param([string]$Version = "latest")
$ErrorActionPreference = "Stop"

$Repo = "shankar-sachin/canopy"
$Arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$Target = if ($Arch -eq "ARM64") { "aarch64" } else { "x86_64" }

if ($Version -eq "latest") {
    $rel = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "canopy-installer" }
    $Version = $rel.tag_name
}

$name = "canopy-desktop-$Version-$Target-setup.exe"
$url = "https://github.com/$Repo/releases/download/$Version/$name"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory $tmp | Out-Null
try {
    Write-Host "canopy: downloading $name"
    Invoke-WebRequest $url -OutFile "$tmp\$name" -UseBasicParsing
    Invoke-WebRequest "$url.sha256" -OutFile "$tmp\$name.sha256" -UseBasicParsing
    $want = ((Get-Content "$tmp\$name.sha256" -Raw).Trim() -split "\s+")[0].ToLower()
    $got = (Get-FileHash "$tmp\$name" -Algorithm SHA256).Hash.ToLower()
    if ($want -ne $got) { throw "checksum mismatch (expected $want, got $got); not installing" }
    Write-Host "canopy: running the installer"
    Start-Process "$tmp\$name" -Wait
    Write-Host "canopy: done. Find canopy in the Start menu."
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { Write-Host "canopy: note: canopy needs git (winget install --id Git.Git -e)" }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
