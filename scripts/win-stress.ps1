<#
.SYNOPSIS
    Runs a test binary in a tight loop to surface intermittent Windows crashes.

.DESCRIPTION
    Issues #308 and #367 track a STATUS_ACCESS_VIOLATION (0xC0000005) that
    appears on roughly one Windows CI run in ten, so a single green run proves
    nothing either way. This script turns that into a measurable rate: it builds
    the test binaries once, runs one of them N times, and reports how many
    iterations died on a non-zero exit code together with the code itself.

    Run it before and after a candidate fix and compare the rates. Under
    Application Verifier's page heap (see -Verifier) a latent use-after-free
    faults on the first bad access instead of whenever the heap happens to
    recycle the block, which turns a statistical argument into a binary one.

.PARAMETER Test
    Integration test target to stress, e.g. db_query_test. Defaults to the whole
    suite ("--workspace"), which is slower but covers binaries beyond the two
    named in #367.

.PARAMETER Iterations
    How many times to run the binary. 200 is a reasonable starting point for a
    fast suite; raise it until the pre-fix rate is stable enough to compare.

.PARAMETER TestThreads
    Value for libtest's --test-threads. Leave unset for the default (one thread
    per core). Set to 1 to test whether serialising teardown hides the crash.

.PARAMETER Verifier
    Enable Application Verifier page heap for the binary for the duration of the
    run, then disable it again. Requires appverif.exe (Windows SDK) and an
    elevated shell.

.PARAMETER TargetDir
    Cargo target directory. Defaults to target-win so a Windows run does not
    invalidate a WSL/Linux build of the same tree.

.PARAMETER TempDir
    Directory to point TMP/TEMP at for the duration of the run. Each iteration
    of a database suite creates dozens of temp directories, and an iteration
    that crashes leaks every one it was holding, so a few hundred iterations
    against the default %TEMP% can put gigabytes on the system drive and start
    failing tests for lack of space — which looks like a crash but is not one.
    Point this at a volume with room to spare.

.EXAMPLE
    ./scripts/win-stress.ps1 -Test db_query_test -Iterations 200

.EXAMPLE
    ./scripts/win-stress.ps1 -Test db_query_test -Iterations 50 -Verifier
#>
[CmdletBinding()]
param(
    [string] $Test = '',
    [int]    $Iterations = 200,
    [int]    $TestThreads = 0,
    [switch] $Verifier,
    [string] $TargetDir = 'target-win',
    [string] $TempDir = ''
)

$ErrorActionPreference = 'Stop'

# Match the Windows CI job: RUST_MIN_STACK resizes spawned threads, and
# .cargo/config.toml's /STACK link arg covers the main thread on MSVC (#337).
$env:RUST_MIN_STACK = '8388608'
$env:CARGO_TARGET_DIR = $TargetDir
if ($TempDir) {
    New-Item -ItemType Directory -Force -Path $TempDir | Out-Null
    $env:TMP = $TempDir
    $env:TEMP = $TempDir
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $selector = if ($Test) { @('--test', $Test) } else { @('--workspace') }

    Write-Host "Building test binaries ($($selector -join ' '))..." -ForegroundColor Cyan
    $messages = & cargo test @selector --no-run --message-format=json 2>$null
    if ($LASTEXITCODE -ne 0) { throw "cargo test --no-run failed ($LASTEXITCODE)" }

    $binaries = @(
        $messages |
            ForEach-Object { try { $_ | ConvertFrom-Json } catch { $null } } |
            Where-Object { $_ -and $_.executable } |
            ForEach-Object { $_.executable } |
            Select-Object -Unique
    )
    if ($binaries.Count -eq 0) { throw 'no test executables reported by cargo' }
    if ($Test -and $binaries.Count -gt 1) {
        # --test <name> can still report the lib's unit-test binary; keep the one
        # whose file name matches the requested target.
        $binaries = @($binaries | Where-Object { (Split-Path $_ -Leaf) -like "$Test-*" })
    }

    $harnessArgs = @()
    if ($TestThreads -gt 0) { $harnessArgs += @('--test-threads', "$TestThreads") }

    if ($Verifier) {
        foreach ($binary in $binaries) {
            & appverif.exe -enable Heaps -for (Split-Path $binary -Leaf) | Out-Null
        }
        Write-Host 'Application Verifier page heap enabled.' -ForegroundColor Yellow
    }

    $failures = @()
    try {
        foreach ($binary in $binaries) {
            $name = Split-Path $binary -Leaf
            Write-Host "Stressing $name x$Iterations..." -ForegroundColor Cyan

            for ($i = 1; $i -le $Iterations; $i++) {
                & $binary @harnessArgs *> $null
                $code = $LASTEXITCODE
                if ($code -ne 0) {
                    # PowerShell surfaces NTSTATUS codes as negative Int32, and
                    # Windows PowerShell parses the bare literal 0xFFFFFFFF as
                    # Int32 -1, so both sides need widening or the mask is a
                    # no-op and the [uint32] cast throws on the negative value.
                    $hex = '0x{0:X8}' -f [uint32] ([int64] $code -band 0xFFFFFFFFL)
                    # An access violation is the failure this script exists to
                    # count. Everything else — a plain assertion failure (101), a
                    # missing fixture, a full disk — is a broken run, not a
                    # crash, and folding the two together silently inflates the
                    # rate the before/after comparison rests on.
                    $isCrash = $code -eq -1073741819   # 0xC0000005
                    $failures += [pscustomobject]@{
                        Binary    = $name
                        Iteration = $i
                        ExitCode  = $hex
                        IsCrash   = $isCrash
                    }
                    $label = if ($isCrash) { 'ACCESS VIOLATION' } else { 'failed run' }
                    Write-Host "  iteration $i exited $hex ($label)" -ForegroundColor (
                        if ($isCrash) { 'Red' } else { 'Yellow' })
                }
                elseif ($i % 25 -eq 0) {
                    Write-Host "  $i/$Iterations clean"
                }
            }
        }
    }
    finally {
        if ($Verifier) {
            foreach ($binary in $binaries) {
                & appverif.exe -disable * -for (Split-Path $binary -Leaf) | Out-Null
            }
            Write-Host 'Application Verifier page heap disabled.' -ForegroundColor Yellow
        }
    }

    $total = $Iterations * $binaries.Count
    $crashes = @($failures | Where-Object IsCrash)
    $broken = @($failures | Where-Object { -not $_.IsCrash })

    Write-Host ''
    Write-Host "=== $($crashes.Count)/$total access violations ===" -ForegroundColor (
        if ($crashes.Count -eq 0) { 'Green' } else { 'Red' })
    if ($broken.Count -gt 0) {
        # Not part of the crash rate, but not noise either: a run that failed for
        # another reason did not exercise the teardown path, so the denominator
        # above is optimistic until these are explained.
        Write-Host "=== $($broken.Count)/$total runs failed for other reasons ===" -ForegroundColor Yellow
    }
    if ($failures.Count -gt 0) {
        $failures | Group-Object Binary, ExitCode |
            ForEach-Object { Write-Host ("  {0}  x{1}" -f $_.Name, $_.Count) }
        exit 1
    }
}
finally {
    Pop-Location
}
