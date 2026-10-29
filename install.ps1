$ErrorActionPreference = 'Stop'
trap { [Console]::Error.WriteLine($_); exit 1 }
$repository = if ($env:CUEMOL_RELEASE_REPOSITORY) { $env:CUEMOL_RELEASE_REPOSITORY } else { 'mogura-rs/cuemol-rs' }
$version = if ($env:CUEMOL_VERSION) { $env:CUEMOL_VERSION } else { 'latest' }
if ($version -notmatch '^[a-zA-Z0-9._-]+$') { throw 'Invalid release version' }
$base = if ($version -eq 'latest') { "https://github.com/$repository/releases/latest/download" } else { "https://github.com/$repository/releases/download/$version" }
if ($env:CUEMOL_RELEASE_URL) { $base = $env:CUEMOL_RELEASE_URL }
$destination = if ($env:CUEMOL_INSTALL_DIR) { $env:CUEMOL_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'CueMol/bin' }
$installName = if ($env:CUEMOL_INSTALL_NAME) { $env:CUEMOL_INSTALL_NAME } else { 'cuemol.exe' }
if ($installName -notmatch '^[a-zA-Z0-9._-]+\.exe$') { throw 'Invalid executable name' }
$asset = 'cuemol-x86_64-pc-windows-msvc.zip'
$work = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Force $work, $destination | Out-Null
function Download($source, $target) {
    if ($source.StartsWith('file://')) { Copy-Item ([Uri]$source).LocalPath $target }
    else { Invoke-WebRequest -UseBasicParsing $source -OutFile $target }
}
try {
    Download "$base/$asset" (Join-Path $work $asset)
    Download "$base/SHA256SUMS" (Join-Path $work 'SHA256SUMS')
    $line = Get-Content (Join-Path $work 'SHA256SUMS') | Where-Object { $_ -match "^[a-fA-F0-9]{64}\s+$([Regex]::Escape($asset))$" }
    if (@($line).Count -ne 1) { throw 'Missing or invalid checksum' }
    $expected = ($line -split '\s+')[0]
    $actual = (Get-FileHash (Join-Path $work $asset) -Algorithm SHA256).Hash
    if ($expected -ne $actual) { throw 'Checksum mismatch' }
    Expand-Archive (Join-Path $work $asset) (Join-Path $work 'unpacked')
    $incoming = Join-Path $work 'unpacked/cuemol.exe'
    if (-not (Test-Path $incoming -PathType Leaf)) { throw 'Release contains no cuemol.exe' }
    $binary = Join-Path $destination $installName
    $previous = Join-Path $destination ([IO.Path]::GetFileNameWithoutExtension($installName) + '.previous.exe')
    Remove-Item $previous -Force -ErrorAction SilentlyContinue
    if (Test-Path $binary) { Move-Item $binary $previous -Force }
    $tools = Join-Path $work 'unpacked/cuemol-tools'
    $installedTools = Join-Path $destination 'cuemol-tools'
    $previousTools = Join-Path $destination '.cuemol-previous-tools'
    $replacedTools = $false
    try {
        if (Test-Path $tools) {
            if (Test-Path $previousTools) { Remove-Item $previousTools -Recurse -Force }
            if (Test-Path $installedTools) { Move-Item $installedTools $previousTools }
            Move-Item $tools $installedTools
            $replacedTools = $true
        }
        Move-Item $incoming $binary -Force
    }
    catch {
        if ($replacedTools -and (Test-Path $installedTools)) { Remove-Item $installedTools -Recurse -Force }
        if (Test-Path $previousTools) { Move-Item $previousTools $installedTools }
        if (Test-Path $previous) { Move-Item $previous $binary -Force }
        throw
    }
    if (Test-Path $previousTools) { Remove-Item $previousTools -Recurse -Force }
    Write-Output "Installed $installName in $destination"
}
finally { Remove-Item $work -Recurse -Force }
