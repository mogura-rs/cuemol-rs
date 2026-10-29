param([Parameter(Mandatory=$true)][string]$Binary)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$work = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
$release = Join-Path $work 'release'
$inputDir = Join-Path $work 'input'
$installDir = Join-Path $work 'bin'
$oldUrl = $env:CUEMOL_RELEASE_URL
$oldDestination = $env:CUEMOL_INSTALL_DIR
try {
    New-Item -ItemType Directory -Force $release, $inputDir, $installDir, (Join-Path $inputDir 'cuemol-tools') | Out-Null
    Copy-Item $Binary (Join-Path $inputDir 'cuemol.exe')
    $overlay = [Text.Encoding]::ASCII.GetBytes('cuemol-installer-fixture')
    $fixture = [IO.File]::Open((Join-Path $inputDir 'cuemol.exe'), [IO.FileMode]::Append)
    try { $fixture.Write($overlay, 0, $overlay.Length) } finally { $fixture.Dispose() }
    Set-Content (Join-Path $inputDir 'cuemol-tools/marker.txt') 'bundled' -Encoding Ascii
    $asset = 'cuemol-x86_64-pc-windows-msvc.zip'
    Compress-Archive (Join-Path $inputDir '*') (Join-Path $release $asset)
    $checksum = (Get-FileHash (Join-Path $release $asset) -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content (Join-Path $release 'SHA256SUMS') "$checksum  $asset" -Encoding Ascii
    $env:CUEMOL_RELEASE_URL = ([Uri]($release + [IO.Path]::DirectorySeparatorChar)).AbsoluteUri.TrimEnd('/')
    $env:CUEMOL_INSTALL_DIR = $installDir
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'install.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'Installation failed' }
    $installed = Join-Path $installDir 'cuemol.exe'
    & $installed --version
    if ($LASTEXITCODE -ne 0) { throw 'Installed command failed' }
    if ((Get-Content (Join-Path $installDir 'cuemol-tools/marker.txt')) -ne 'bundled') { throw 'Tools were not installed' }
    $before = (Get-FileHash $installed -Algorithm SHA256).Hash
    Set-Content (Join-Path $release 'SHA256SUMS') ((('0' * 64) + "  $asset")) -Encoding Ascii
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'install.ps1')
    if ($LASTEXITCODE -eq 0) { throw 'Installer accepted a bad checksum' }
    if ((Get-FileHash $installed -Algorithm SHA256).Hash -ne $before) { throw 'Failed update replaced the command' }
    Set-Content (Join-Path $release 'SHA256SUMS') "$checksum  $asset" -Encoding Ascii
    & $installed self-upgrade
    if ($LASTEXITCODE -ne 0) { throw 'Self-upgrade failed' }
    & $installed --version
    if ($LASTEXITCODE -ne 0) { throw 'Upgraded command failed' }
    $renamedDir = Join-Path $work 'renamed application'
    New-Item -ItemType Directory $renamedDir | Out-Null
    $renamed = Join-Path $renamedDir 'cuemol-rs.exe'
    Copy-Item $Binary $renamed
    if ((Get-FileHash $renamed -Algorithm SHA256).Hash -eq $before) { throw 'Replacement fixture must differ from the installed command' }
    & $renamed self-upgrade
    if ($LASTEXITCODE -ne 0) { throw 'Renamed command self-upgrade failed' }
    if (Test-Path (Join-Path $renamedDir 'cuemol.exe')) { throw 'Update created a different executable' }
    if ((Get-FileHash $renamed -Algorithm SHA256).Hash -ne $before) { throw 'Renamed command differs from the release' }
    if ((Get-Content (Join-Path $renamedDir 'cuemol-tools/marker.txt')) -ne 'bundled') { throw 'Renamed command tools were not updated' }
}
finally {
    $env:CUEMOL_RELEASE_URL = $oldUrl
    $env:CUEMOL_INSTALL_DIR = $oldDestination
    Remove-Item $work -Recurse -Force
}
