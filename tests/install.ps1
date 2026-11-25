$ErrorActionPreference = 'Stop'
. (Join-Path (Split-Path $PSScriptRoot) 'install.ps1')
$scratch = [IO.Path]::Combine([IO.Path]::GetTempPath(), [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($scratch) | Out-Null
$script:FixtureTag = '1.0.0'
$script:FixtureReportedVersion = '1.0.0'
$script:FixtureManifest = ''
$script:FixtureArchive = ''
$script:FixtureBinary = [byte[]](1, 2, 3)
function Get-StableVersion($Client) { return $script:FixtureTag }
function Save-ReleaseAsset($Client, $Url, $Destination) {
    if ($Url.EndsWith('/SHA256SUMS')) {
        [IO.File]::WriteAllText($Destination, $script:FixtureManifest)
    } else {
        [IO.File]::Copy($script:FixtureArchive, $Destination)
    }
}
function Assert-ExecutableVersion($Executable, $Version) {
    if ($script:FixtureReportedVersion -cne $Version) { throw 'Wrong executable version.' }
}
function Assert-Bytes($Path, $Expected) {
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($Path)) -cne [Convert]::ToBase64String($Expected)) {
        throw 'Unexpected installed bytes.'
    }
}
function Must-Fail($Operation) {
    try { & $Operation } catch { return }
    throw 'Expected the installer to fail.'
}
try {
    $install = Join-Path $scratch 'programs'
    $script:FixtureArchive = Join-Path $scratch 'fixture.zip'
    $zip = [IO.Compression.ZipFile]::Open($script:FixtureArchive, [IO.Compression.ZipArchiveMode]::Create)
    try {
        $stream = $zip.CreateEntry('media-organizer.exe').Open()
        try { $stream.Write($script:FixtureBinary, 0, $script:FixtureBinary.Length) }
        finally { $stream.Dispose() }
    } finally { $zip.Dispose() }
    $asset = 'media-organizer-1.0.0-windows-x64.zip'
    $hash = (Get-FileHash -LiteralPath $script:FixtureArchive -Algorithm SHA256).Hash
    $validManifest = "$hash  $asset`n"
    $script:FixtureManifest = $validManifest
    Install-MediaOrganizer $null $install
    $destination = Join-Path $install 'media-organizer.exe'
    Assert-Bytes $destination $script:FixtureBinary
    $previous = [byte[]](5, 6)
    [IO.File]::WriteAllBytes($destination, $previous)
    $script:FixtureReportedVersion = '0.9.0'
    Must-Fail { Install-MediaOrganizer $null $install }
    Assert-Bytes $destination $previous
    $script:FixtureReportedVersion = '1.0.0'
    $script:FixtureManifest = $validManifest + $validManifest
    Must-Fail { Install-MediaOrganizer $null $install }
    Assert-Bytes $destination $previous
    $script:FixtureManifest = ('0' * 64) + "  $asset`n"
    Must-Fail { Install-MediaOrganizer $null $install }
    Assert-Bytes $destination $previous
    $script:FixtureManifest = $validManifest
    Install-MediaOrganizer $null $install
    Assert-Bytes $destination $script:FixtureBinary
    $original = "C:\Existing;$install"
    if ((Add-PathEntry $original $install) -cne $original) { throw 'PATH entry was duplicated.' }
    if ((Add-PathEntry 'C:\Existing' $install) -cne $original) { throw 'PATH entry was not appended.' }
} finally {
    Remove-Item -LiteralPath $scratch -Recurse -Force
}
