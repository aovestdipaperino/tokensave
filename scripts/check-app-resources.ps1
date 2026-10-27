param([Parameter(Mandatory=$true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$info = (Get-Item -LiteralPath $Executable).VersionInfo
if ($info.ProductName -ne 'TokenSave Beta' -or
    $info.FileDescription -ne 'TokenSave Beta' -or
    $info.OriginalFilename -ne 'tokensave.exe' -or
    $info.ProductVersion -notmatch '-beta\.\d+' -or
    $info.FileVersion -notmatch '-beta\.\d+' -or
    -not $info.IsPreRelease) {
    throw "Incorrect beta version resource: $info"
}
Add-Type -AssemblyName System.Drawing
$temporaryDirectory = $null
$icon = $null
try {
    $iconPath = $Executable
    # ExtractAssociatedIcon rejects UNC paths, including WSL checkouts.
    if ($Executable.StartsWith('\\')) {
        $temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
        New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
        $iconPath = Join-Path $temporaryDirectory 'tokensave.exe'
        Copy-Item -LiteralPath $Executable -Destination $iconPath
    }
    $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($iconPath)
    if ($null -eq $icon) { throw 'No executable icon found' }
} finally {
    if ($null -ne $icon) { $icon.Dispose() }
    if ($null -ne $temporaryDirectory) { Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force }
}
Write-Output "$($info.ProductName) $($info.ProductVersion)"
