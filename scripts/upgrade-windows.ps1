# Run from a separate PowerShell session, outside any TokenSave agent.
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$InstallRoot,

    [string[]]$CargoArguments = @('tokensave', '--locked'),

    [switch]$ForceStop
)

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') {
    throw 'This helper is for Windows. Run Cargo directly on this platform.'
}
if ($CargoArguments -match '^--root(?:=|$)') {
    throw 'Specify the Cargo install root with -InstallRoot.'
}
if ('--force' -notin $CargoArguments -and '-f' -notin $CargoArguments) {
    $CargoArguments += '--force'
}

$cargo = (Get-Command cargo -CommandType Application).Source
$InstallRoot = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallRoot)
$bin = Join-Path $InstallRoot 'bin'
$exe = Join-Path $bin 'tokensave.exe'
# Shared with src/windows_upgrade/constants.rs. An open exclusive handle
# signals shutdown and blocks launches. Windows releases it on process exit.
$marker = Join-Path $bin 'tokensave.cargo-upgrade.lock'
$waitMilliseconds = 15000
$lockWaitMilliseconds = 1000
$pollMilliseconds = 200
$sharingViolation = 32
$accessDenied = 5
$backup = Join-Path $bin 'tokensave.exe.cargo-upgrade-backup'

function Get-TargetProcesses {
    foreach ($candidate in Get-Process -Name tokensave -ErrorAction SilentlyContinue) {
        $keep = $false
        try {
            # Retain the process handle while inspecting and stopping it so
            # Windows cannot recycle this PID for another process.
            $null = $candidate.Handle
            if ([string]::Equals($candidate.MainModule.FileName, $exe, 'OrdinalIgnoreCase') -and -not $candidate.HasExited) {
                $keep = $true
                $candidate
            }
        }
        catch {
            if (-not $candidate.HasExited) {
                throw "Cannot inspect TokenSave PID $($candidate.Id). Check process permissions, Windows integrity level, and PowerShell bitness. $($_.Exception.Message)"
            }
        }
        finally {
            if (-not $keep) { $candidate.Dispose() }
        }
    }
}

# A running client's probe can briefly have the lock file open for reading.
$lockTimer = [Diagnostics.Stopwatch]::StartNew()
do {
    try {
        $guard = [IO.File]::Open($marker, 'OpenOrCreate', 'ReadWrite', 'None')
        break
    }
    catch {
        $reason = $_.Exception.GetBaseException()
        if (($reason.HResult -band 0xffff) -ne $sharingViolation) {
            throw "Cannot open the upgrade lock at $marker. Check permissions on the Cargo bin directory. $($reason.Message)"
        }
        if ($lockTimer.ElapsedMilliseconds -ge $lockWaitMilliseconds) {
            throw "The upgrade lock at $marker is in use. Wait for the other operation to finish."
        }
        Start-Sleep -Milliseconds $pollMilliseconds
    }
} while ($true)

$processes = [Collections.Generic.List[Diagnostics.Process]]::new()
try {
    if ([IO.File]::Exists($backup)) {
        throw "An earlier upgrade left a backup at $backup. Resolve it before retrying. If $exe is missing, move the backup back to that path."
    }
    if (-not [IO.File]::Exists($exe)) {
        throw "Cannot find an accessible $exe. Check -InstallRoot and your permissions. For a new installation, run Cargo directly."
    }

    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        foreach ($target in Get-TargetProcesses) { $processes.Add($target) }
        if ($processes.Count -eq 0) { break }

        if ($timer.ElapsedMilliseconds -ge $waitMilliseconds) {
            if (-not $ForceStop) {
                $ids = ($processes | ForEach-Object Id) -join ', '
                throw "TokenSave did not exit in time (PIDs: $ids). Close the owning agents or rerun with -ForceStop to terminate these instances explicitly."
            }
            foreach ($target in $processes) {
                if (-not $target.HasExited) {
                    try { $target.Kill() }
                    catch {
                        if (-not $target.HasExited) { throw }
                    }
                }
                if (-not $target.WaitForExit($waitMilliseconds)) {
                    throw "TokenSave PID $($target.Id) still holds the executable. Stop its supervisor, then retry."
                }
            }
            foreach ($target in $processes) { $target.Dispose() }
            $processes.Clear()
            # Refuse to install if a legacy agent has already restarted.
            foreach ($target in Get-TargetProcesses) { $processes.Add($target) }
            if ($processes.Count -ne 0) {
                throw 'TokenSave restarted during shutdown. Pause the agent or supervisor and retry.'
            }
            break
        }

        foreach ($target in $processes) { $target.Dispose() }
        $processes.Clear()
        Start-Sleep -Milliseconds $pollMilliseconds
    } while ($true)

    # Remove the old image from the launch path before Cargo starts. This
    # closes the restart window during Cargo's potentially lengthy build.
    try {
        [IO.File]::Move($exe, $backup)
    }
    catch {
        $reason = $_.Exception.GetBaseException()
        foreach ($target in Get-TargetProcesses) { $processes.Add($target) }
        if ($processes.Count -ne 0) {
            throw "TokenSave is running again at $exe. Pause its supervisor and retry."
        }
        switch ($reason.HResult -band 0xffff) {
            $sharingViolation { throw "Windows reports a sharing violation on $exe. Close the program holding the file and retry." }
            $accessDenied { throw "Windows denied moving $exe. Check directory permissions, integrity level, and file locks. Access denied alone does not identify the cause." }
        }
        throw
    }

    try {
        & $cargo install @CargoArguments --root $InstallRoot
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo install exited with code $LASTEXITCODE. See the Cargo output above."
        }
        if (-not [IO.File]::Exists($exe)) {
            throw "Cargo finished without installing $exe. Check -CargoArguments."
        }
        [IO.File]::Delete($backup)
    }
    finally {
        if (-not [IO.File]::Exists($exe)) {
            try { [IO.File]::Move($backup, $exe) }
            catch { throw "Could not restore $exe. The previous binary is at $backup. $($_.Exception.Message)" }
        }
    }
}
finally {
    foreach ($target in $processes) { $target.Dispose() }
    $guard.Dispose()
}
