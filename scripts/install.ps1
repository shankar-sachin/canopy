# Install Canopy on Windows from a GitHub release.
#
#   irm https://raw.githubusercontent.com/shankar-sachin/canopy/main/scripts/install.ps1 | iex
#
# To pick a version or uninstall, download the script and run it:
#   .\install.ps1 -Version v1.0.0
#   .\install.ps1 -Uninstall
#
# Installs to %LOCALAPPDATA%\Programs\canopy (override with $env:CANOPY_INSTALL_DIR)
# and adds that folder to your user PATH. No admin rights needed.
param([string]$Version = "latest", [switch]$Uninstall)
$ErrorActionPreference = "Stop"

$Repo = "shankar-sachin/canopy"
$Dir = if ($env:CANOPY_INSTALL_DIR) { $env:CANOPY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\canopy" }
$Target = "x86_64-pc-windows-msvc"

function Remove-FromPath($d) {
    $p = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($p) {
        $parts = $p.Split(";") | Where-Object { $_ -and ($_ -ne $d) }
        [Environment]::SetEnvironmentVariable("Path", ($parts -join ";"), "User")
    }
}

if ($Uninstall) {
    if (Test-Path $Dir) { Remove-Item -Recurse -Force $Dir; Write-Host "canopy: removed $Dir" }
    Remove-FromPath $Dir
    Write-Host "canopy: your config (%USERPROFILE%\.config\canopy) was left alone"
    return
}

if ($Version -eq "latest") {
    $rel = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "canopy-installer" }
    $Version = $rel.tag_name
}

$name = "canopy-$Version-$Target"
$url = "https://github.com/$Repo/releases/download/$Version/$name.zip"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory $tmp | Out-Null
try {
    Write-Host "canopy: downloading $name"
    Invoke-WebRequest $url -OutFile "$tmp\$name.zip" -UseBasicParsing
    Invoke-WebRequest "$url.sha256" -OutFile "$tmp\$name.zip.sha256" -UseBasicParsing
    $want = ((Get-Content "$tmp\$name.zip.sha256" -Raw).Trim() -split "\s+")[0].ToLower()
    $got = (Get-FileHash "$tmp\$name.zip" -Algorithm SHA256).Hash.ToLower()
    if ($want -ne $got) { throw "checksum mismatch (expected $want, got $got); not installing" }

    Expand-Archive "$tmp\$name.zip" -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force $Dir | Out-Null
    Copy-Item "$tmp\$name\canopy.exe" $Dir -Force
    Write-Host "canopy: installed $Version to $Dir\canopy.exe"

    $p = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not ($p -split ";" | Where-Object { $_ -eq $Dir })) {
        [Environment]::SetEnvironmentVariable("Path", (($p, $Dir) | Where-Object { $_ }) -join ";", "User")
        Write-Host "canopy: added $Dir to your PATH; open a new terminal, then run: canopy"
    } else {
        Write-Host "canopy: run: canopy"
    }
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) { Write-Host "canopy: note: Canopy needs git (winget install --id Git.Git -e)" }
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
