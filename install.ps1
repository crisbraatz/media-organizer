[CmdletBinding()]
param([string] $InstallDir)

Add-Type -AssemblyName System.Net.Http
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$script:Repository = 'https://github.com/crisbraatz/media-organizer'

function Get-ReleaseResponse($Client, [Uri] $Url) {
    for ($redirect = 0; $redirect -lt 8; $redirect++) {
        if ($Url.Scheme -cne 'https') { throw 'Refusing a non-HTTPS release URL.' }
        $response = $Client.GetAsync($Url, [Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        if ([int] $response.StatusCode -in 301, 302, 303, 307, 308) {
            try {
                if ($null -eq $response.Headers.Location) { throw 'Missing redirect location.' }
                $Url = [Uri]::new($Url, $response.Headers.Location)
            } finally {
                $response.Dispose()
            }
            continue
        }
        if (!$response.IsSuccessStatusCode) {
            $status = [int] $response.StatusCode
            $response.Dispose()
            throw "GitHub download failed with HTTP $status."
        }
        return $response
    }
    throw 'Too many GitHub release redirects.'
}

function Get-StableVersion($Client) {
    $response = Get-ReleaseResponse $Client "$script:Repository/releases/latest"
    try {
        $url = $response.RequestMessage.RequestUri.AbsoluteUri
        $prefix = "$script:Repository/releases/tag/v"
        if (!$url.StartsWith($prefix, [StringComparison]::Ordinal)) { throw 'Unexpected GitHub release URL.' }
        $version = $url.Substring($prefix.Length)
        if ($version -cnotmatch '\A(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\z') {
            throw 'GitHub did not return a supported stable release tag.'
        }
        return $version
    } finally {
        $response.Dispose()
    }
}

function Save-ReleaseAsset($Client, [string] $Url, [string] $Destination) {
    $response = Get-ReleaseResponse $Client $Url
    try {
        $file = [IO.File]::Open($Destination, [IO.FileMode]::CreateNew)
        try { $response.Content.CopyToAsync($file).GetAwaiter().GetResult() | Out-Null }
        finally { $file.Dispose() }
    } finally {
        $response.Dispose()
    }
}

function Assert-ExecutableVersion([string] $Executable, [string] $Version) {
    $reported = & $Executable --version
    if ($LASTEXITCODE -ne 0 -or ($reported -join "`n") -cne "media-organizer $Version") {
        throw 'The executable version does not match the release.'
    }
}

function Add-PathEntry([string] $Current, [string] $Directory) {
    foreach ($entry in $Current.Split(';')) {
        $expanded = [Environment]::ExpandEnvironmentVariables($entry.Trim().Trim('"')).TrimEnd([char[]] '/\')
        if ($expanded -ieq $Directory.TrimEnd([char[]] '/\')) { return $Current }
    }
    if (!$Current) { return $Directory }
    return $Current.TrimEnd(';') + ';' + $Directory
}

function Install-MediaOrganizer($Client, [string] $Directory) {
    $version = Get-StableVersion $Client
    $asset = "media-organizer-$version-windows-x64.zip"
    $base = "$script:Repository/releases/download/v$version"
    $scratch = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString('N'))
    [IO.Directory]::CreateDirectory($scratch) | Out-Null
    $archivePath = Join-Path $scratch $asset
    $manifest = Join-Path $scratch 'SHA256SUMS'
    $stage = $null
    $backup = $null
    $committed = $false
    try {
        Save-ReleaseAsset $Client "$base/$asset" $archivePath
        Save-ReleaseAsset $Client "$base/SHA256SUMS" $manifest
        $records = @([IO.File]::ReadAllLines($manifest) | Where-Object {
            $fields = $_.Trim() -split '\s+'
            $fields.Length -ge 2 -and $fields[1] -ceq $asset
        })
        $pattern = '\A([0-9A-Fa-f]{64})[ \t]+' + [regex]::Escape($asset) + '\s*\z'
        if ($records.Count -ne 1 -or $records[0] -cnotmatch $pattern) {
            throw "Expected exactly one SHA-256 checksum for $asset."
        }
        $expected = $Matches[1]
        if ((Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash -ine $expected) {
            throw "SHA-256 mismatch for $asset."
        }

        [IO.Directory]::CreateDirectory($Directory) | Out-Null
        $destination = Join-Path $Directory 'media-organizer.exe'
        if (Test-Path -LiteralPath $destination) {
            $entry = Get-Item -LiteralPath $destination -Force
            if ($entry.PSIsContainer -or ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw 'The executable destination is a directory or a link.'
            }
        }
        $stage = Join-Path $Directory ('.media-organizer-' + [Guid]::NewGuid().ToString('N') + '.exe')
        $archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
        try {
            $entries = @($archive.Entries | Where-Object { $_.FullName -ceq 'media-organizer.exe' })
            if ($entries.Count -ne 1) { throw 'Expected exactly one executable in the archive.' }
            $inputStream = $entries[0].Open()
            try {
                $file = [IO.File]::Open($stage, [IO.FileMode]::CreateNew)
                try { $inputStream.CopyTo($file) }
                finally { $file.Dispose() }
            } finally { $inputStream.Dispose() }
        } finally { $archive.Dispose() }

        Assert-ExecutableVersion $stage $version
        if ([IO.File]::Exists($destination)) {
            $backup = Join-Path $Directory ('.media-organizer-' + [Guid]::NewGuid().ToString('N') + '.backup')
            try {
                [IO.File]::Replace($stage, $destination, $backup)
            } catch {
                if ([IO.File]::Exists($backup) -and ![IO.File]::Exists($destination)) {
                    [IO.File]::Move($backup, $destination)
                }
                throw
            }
        } else {
            [IO.File]::Move($stage, $destination)
        }
        $committed = $true
        Write-Host "Installed media-organizer $version at $destination"
    } finally {
        foreach ($file in @($stage, $archivePath, $manifest)) {
            if ($file) { Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue }
        }
        Remove-Item -LiteralPath $scratch -ErrorAction SilentlyContinue
        if ($backup -and [IO.File]::Exists($backup)) {
            if ($committed) {
                Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue
            } else {
                Write-Warning "Previous executable backup retained at $backup"
            }
        }
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    $ErrorActionPreference = 'Stop'
    if ($env:OS -ne 'Windows_NT') { throw 'This installer requires Windows x64.' }
    $processors = @(Get-CimInstance -ClassName Win32_Processor)
    if (!$processors.Count -or @($processors | Where-Object { $_.Architecture -ne 9 }).Count) {
        throw 'This installer requires a native Windows x64 processor.'
    }

    $addUserPath = !$InstallDir
    if ($addUserPath) {
        $InstallDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs\media-organizer'
    }
    if ($InstallDir -notmatch '\A(?:[A-Za-z]:[\\/]|[\\/]{2}[^\\/]+[\\/][^\\/]+(?:[\\/]|\z))' -or $InstallDir.Contains(';')) {
        throw 'The install directory must be an absolute path without semicolons.'
    }
    $InstallDir = [IO.Path]::GetFullPath($InstallDir)
    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [Net.Http.HttpClient]::new($handler)
    $client.DefaultRequestHeaders.UserAgent.ParseAdd('media-organizer-installer/1.0')
    try { Install-MediaOrganizer $client $InstallDir }
    finally { $client.Dispose() }
    if ($addUserPath) {
        try {
            $oldPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $newPath = Add-PathEntry $oldPath $InstallDir
            if ($newPath -cne $oldPath) { [Environment]::SetEnvironmentVariable('Path', $newPath, 'User') }
            $env:Path = Add-PathEntry $env:Path $InstallDir
            Write-Host 'Open a new terminal if media-organizer is not yet on your PATH.'
        } catch {
            Write-Warning "The executable is installed. Add $InstallDir to your user PATH manually."
        }
    }
}
