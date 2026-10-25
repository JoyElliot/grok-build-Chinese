[CmdletBinding()]
param(
    [string]$InstallerPath,
    [switch]$Worker,
    [string]$Case,
    [string]$TestRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$InstallerPath) {
    $InstallerPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'Install-GrokZh.ps1'
}

function Assert-CancellationTest {
    param([bool]$Condition, [string]$Message)
    if (!$Condition) { throw $Message }
}

function Get-TreeSnapshot {
    param([string]$Root)
    return (@(Get-ChildItem -LiteralPath $Root -File -Recurse -Force | Sort-Object FullName | ForEach-Object {
        $_.FullName.Substring($Root.Length) + ':' + (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
    }) -join "`n")
}

function New-CancellationPackage {
    param([string]$Root, [string]$Installer)
    $names = @(
        'grok-zh.exe', 'agent-zh.cmd', 'rg.exe', '一键安装.cmd', '[可选]替换原始启动方式.cmd',
        'Install-GrokZh.ps1', 'INSTALL-WINDOWS.md', 'LICENSE-grok-build.txt', 'BUILD-INFO.txt',
        'licenses/ripgrep/COPYING', 'licenses/ripgrep/LICENSE-MIT', 'licenses/ripgrep/UNLICENSE',
        'licenses/project/THIRD-PARTY-NOTICES', 'licenses/project/THIRD_PARTY_NOTICES.md', 'licenses/project/NOTICE'
    )
    foreach ($name in $names) {
        $path = Join-Path $Root $name
        New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
        [IO.File]::WriteAllText($path, "cancellation-fixture:$name")
    }
    Copy-Item -LiteralPath $Installer -Destination (Join-Path $Root 'Install-GrokZh.ps1') -Force
    # A legacy full-package fixture deliberately has no executable-version
    # contract. These tests exercise real filesystem activation, not EXE launch.
    [IO.File]::WriteAllText((Join-Path $Root 'BUILD-INFO.txt'), 'Version: cancellation-fixture')
    return $names
}

function Write-CancellationManifest {
    param([string]$Root, [string[]]$Names)
    $lines = foreach ($name in $Names) {
        (Get-FileHash -LiteralPath (Join-Path $Root $name) -Algorithm SHA256).Hash + '  ' + $name
    }
    [IO.File]::WriteAllLines((Join-Path $Root 'SHA256SUMS.txt'), [string[]]$lines, [Text.UTF8Encoding]::new($false))
}

if (!$Worker) {
    # Isolate each controller in a child of the current engine, so even a
    # broken/hung finally or runspace disposal cannot hang the test job.
    $engine = [Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
    $cases = @(
        'stop-before-backup', 'stop-after-backup', 'stop-before-activate', 'stop-after-activate',
        'stop-first-before-activate', 'stop-first-after-activate',
        'throw-after-backup', 'throw-after-activate',
        'stop-restore-failure', 'throw-restore-failure', 'blocked-running-process', 'stop-concurrent-installer', 'success'
    )
    foreach ($scenario in $cases) {
        $root = Join-Path ([IO.Path]::GetTempPath()) ('grok installer cancellation ' + [guid]::NewGuid().ToString('N'))
        $process = $null
        try {
            New-Item -ItemType Directory -Path $root | Out-Null
            $quoted = @($PSCommandPath, $InstallerPath, $scenario, $root) | ForEach-Object { "'" + $_.Replace("'", "''") + "'" }
            $invoke = '& {0} -InstallerPath {1} -Worker -Case {2} -TestRoot {3}' -f $quoted
            $command = '$ProgressPreference = ''SilentlyContinue''; [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); try { ' + $invoke + ' } catch { [Console]::Error.WriteLine($_.ToString() + [Environment]::NewLine + $_.ScriptStackTrace); exit 1 }'
            $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
            $start = [Diagnostics.ProcessStartInfo]::new()
            $start.FileName = $engine
            $start.Arguments = "-NoLogo -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand $encoded"
            $start.UseShellExecute = $false
            $start.CreateNoWindow = $true
            $start.RedirectStandardOutput = $true
            $start.RedirectStandardError = $true
            $start.StandardOutputEncoding = [Text.Encoding]::UTF8
            $start.StandardErrorEncoding = [Text.Encoding]::UTF8
            $process = [Diagnostics.Process]::Start($start)
            $stdout = $process.StandardOutput.ReadToEndAsync()
            $stderr = $process.StandardError.ReadToEndAsync()
            if (!$process.WaitForExit(60000)) {
                $process.Kill()
                $null = $process.WaitForExit(5000)
                throw "Cancellation case exceeded its 60-second process deadline: $scenario"
            }
            $output = $stdout.GetAwaiter().GetResult()
            $errors = $stderr.GetAwaiter().GetResult()
            Assert-CancellationTest ($process.ExitCode -eq 0) "$scenario failed in PowerShell $($PSVersionTable.PSVersion):`n$output`n$errors"
            Write-Host "PASS $scenario (PowerShell $($PSVersionTable.PSVersion))"
        } finally {
            if ($null -ne $process) { $process.Dispose() }
            if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
        }
    }
    Write-Host 'Installer cancellation recovery passed. Abrupt process termination/power loss is outside finally recovery.'
    return
}

Assert-CancellationTest (![string]::IsNullOrWhiteSpace($TestRoot)) 'Worker requires an isolated test root.'
$package = Join-Path $TestRoot 'package'
$install = Join-Path $TestRoot 'active'
$sharedHome = Join-Path $TestRoot 'shared-home'
$names = @(New-CancellationPackage -Root $package -Installer $InstallerPath)
Write-CancellationManifest -Root $package -Names $names
New-Item -ItemType Directory -Path $sharedHome | Out-Null
[IO.File]::WriteAllText((Join-Path $sharedHome 'auth.json'), 'shared-auth-must-not-change')
[IO.File]::WriteAllText((Join-Path $sharedHome 'settings.json'), 'shared-settings-must-not-change')
$homeBefore = Get-TreeSnapshot $sharedHome
$isFresh = $Case.StartsWith('stop-first-', [StringComparison]::Ordinal)
$oldSnapshot = $null
if (!$isFresh) {
    & $InstallerPath -PackageDir $package -InstallDir $install -GrokHome $sharedHome -NoPathUpdate -Confirm:$false 6>$null
    # Include historical recovery data, not only the main EXE, in the exact
    # byte-for-byte rollback assertion.
    $officialBackup = Join-Path $install 'official-backup'
    New-Item -ItemType Directory -Path $officialBackup | Out-Null
    [IO.File]::WriteAllText((Join-Path $officialBackup 'grok.exe'), 'historical-official-backup')
    $oldSnapshot = Get-TreeSnapshot $install
}
[IO.File]::WriteAllText((Join-Path $package 'grok-zh.exe'), 'new-candidate-must-not-replace-old-on-cancel')
Write-CancellationManifest -Root $package -Names $names
$newHash = (Get-FileHash -LiteralPath (Join-Path $package 'grok-zh.exe') -Algorithm SHA256).Hash

$state = [hashtable]::Synchronized(@{
    Case = $Case
    Install = $install
    Hit = [Threading.ManualResetEventSlim]::new($false)
    RestoreAttempted = $false
    BackupAttempted = $false
    Trace = [Collections.Generic.List[string]]::new()
})
$runner = [PowerShell]::Create()
$invocation = $null
try {
    # Run the whole unmodified production script. Only filesystem command
    # boundaries are intercepted; no production test hooks or copied rollback.
    $null = $runner.AddScript({
        param($Installer, $Package, $Install, $SharedHome, $State)
        Set-StrictMode -Version Latest
        $ErrorActionPreference = 'Stop'
        $ProgressPreference = 'SilentlyContinue'
        $State.Trace.Add('Runspace entered; case=' + $State.Case)
        function Wait-CancellationBoundary {
            $State.Trace.Add('Cancellation boundary entered')
            $State.Hit.Set()
            # BeginStop interrupts a real running cmdlet. Throwing a synthetic
            # PipelineStoppedException would incorrectly exercise catch.
            Start-Sleep -Seconds 30
            throw 'Controller failed to stop the pipeline at the requested boundary.'
        }
        function Get-Process {
            [CmdletBinding()]
            param()
            if ($State.Case -eq 'blocked-running-process') {
                return [pscustomobject]@{ ProcessName = 'grok-zh'; Id = 1234; Path = (Join-Path $State.Install 'grok-zh.exe'); HasExited = $false }
            }
            Microsoft.PowerShell.Management\Get-Process @PSBoundParameters
        }
        function Move-Item {
            param([string]$LiteralPath, [string]$Destination)
            $State.Trace.Add('Move-Item: ' + $LiteralPath + ' -> ' + $Destination)
            $backup = $LiteralPath -eq $State.Install -and $Destination.StartsWith($State.Install + '.previous.', [StringComparison]::OrdinalIgnoreCase)
            $activate = $Destination -eq $State.Install -and $LiteralPath.StartsWith($State.Install + '.stage.', [StringComparison]::OrdinalIgnoreCase)
            $restore = $Destination -eq $State.Install -and $LiteralPath.StartsWith($State.Install + '.previous.', [StringComparison]::OrdinalIgnoreCase)
            $State.Trace.Add('Expected install=' + $State.Install + '; backup=' + $backup + '; activate=' + $activate + '; restore=' + $restore)
            if ($backup) { $State.BackupAttempted = $true }
            if ($restore) {
                $State.RestoreAttempted = $true
                if ($State.Case -in @('stop-restore-failure', 'throw-restore-failure')) { throw 'Injected backup restore failure.' }
            }
            if ($backup -and $State.Case -in @('stop-before-backup', 'stop-concurrent-installer')) { Wait-CancellationBoundary }
            if ($activate -and $State.Case -in @('stop-before-activate', 'stop-first-before-activate')) { Wait-CancellationBoundary }
            Microsoft.PowerShell.Management\Move-Item -LiteralPath $LiteralPath -Destination $Destination
            if ($backup -and $State.Case -in @('stop-after-backup', 'stop-restore-failure')) { Wait-CancellationBoundary }
            if ($backup -and $State.Case -in @('throw-after-backup', 'throw-restore-failure')) { throw 'Injected failure after retiring old installation.' }
            if ($activate -and $State.Case -in @('stop-after-activate', 'stop-first-after-activate')) { Wait-CancellationBoundary }
            if ($activate -and $State.Case -eq 'throw-after-activate') { throw 'Injected failure after activating new installation.' }
        }
        try {
            & $Installer -PackageDir $Package -InstallDir $Install -GrokHome $SharedHome -NoPathUpdate -Confirm:$false
        } catch {
            $State.Trace.Add($_.ToString() + [Environment]::NewLine + $_.ScriptStackTrace)
            throw
        }
    }.ToString()).AddArgument($InstallerPath).AddArgument($package).AddArgument($install).AddArgument($sharedHome).AddArgument($state)
    $invocation = $runner.BeginInvoke()
    $isStop = $Case.StartsWith('stop-', [StringComparison]::Ordinal)
    if ($isStop) {
        if (!$state.Hit.Wait(15000)) {
            # PSDataCollection enumeration blocks while the producer is live.
            # Snapshot only on failure; never delay BeginStop to format logs.
            $earlyErrors = $runner.Streams.Error.ReadAll() | Out-String
            throw "Did not reach cancellation boundary: $Case. State=$($runner.InvocationStateInfo.State). Trace=$($state.Trace -join [Environment]::NewLine). Errors=$earlyErrors"
        }
        if ($Case -eq 'stop-concurrent-installer') {
            $activeBefore = Get-TreeSnapshot $install
            $heldStage = @(Get-ChildItem -LiteralPath $TestRoot -Directory -Filter 'active.stage.*')
            Assert-CancellationTest ($heldStage.Count -eq 1) 'First installer did not own exactly one stage.'
            $stageBefore = Get-TreeSnapshot $heldStage[0].FullName
            $competingError = $null
            try {
                & $InstallerPath -PackageDir $package -InstallDir $install.ToUpperInvariant() -GrokHome $sharedHome -NoPathUpdate -Confirm:$false 6>$null
            } catch { $competingError = $_.Exception.Message }
            Assert-CancellationTest ($null -ne $competingError -and $competingError.Contains('无法独占安装目录')) 'Competing installer did not fail immediately on the stable lock.'
            Assert-CancellationTest ((Get-TreeSnapshot $install) -ceq $activeBefore) 'Competing installer changed the active directory.'
            Assert-CancellationTest ((Get-TreeSnapshot $heldStage[0].FullName) -ceq $stageBefore) 'Competing installer changed the first stage.'
            Assert-CancellationTest (@(Get-ChildItem -LiteralPath $TestRoot -Directory -Filter 'active.stage.*').Count -eq 1) 'Competing installer created another stage.'
            Assert-CancellationTest (@(Get-ChildItem -LiteralPath $TestRoot -Directory -Filter 'active.previous.*').Count -eq 0) 'Competing installer moved the active directory.'
        }
        $stop = $runner.BeginStop($null, $null)
        Assert-CancellationTest ($stop.AsyncWaitHandle.WaitOne(10000)) "Pipeline did not stop in time: $Case"
        $runner.EndStop($stop)
    }
    Assert-CancellationTest ($invocation.AsyncWaitHandle.WaitOne(15000)) "Pipeline did not finish in time: $Case"
    $endedWithError = $false
    $endFailure = ''
    try { $null = $runner.EndInvoke($invocation) } catch { $endedWithError = $true; $endFailure = $_.Exception.ToString() }
    $errorText = $runner.Streams.Error.ReadAll() | Out-String
    $actualState = $runner.InvocationStateInfo.State.ToString()
    $expectedState = if ($isStop) { 'Stopped' } elseif ($Case -eq 'success') { 'Completed' } else { 'Failed' }
    Assert-CancellationTest ($actualState -ceq $expectedState) "Expected real $expectedState pipeline, got $actualState. Reason=$($runner.InvocationStateInfo.Reason). EndInvoke=$endFailure. Trace=$($state.Trace -join [Environment]::NewLine). Errors=$errorText"
    Assert-CancellationTest ($endedWithError -eq ($Case -ne 'success')) "Unexpected EndInvoke outcome: $Case"

    if ($Case -eq 'blocked-running-process') {
        Assert-CancellationTest (!$state.BackupAttempted) 'Running-process refusal happened after retiring the active installation.'
        $rejectionText = $errorText + $endFailure + [string]$runner.InvocationStateInfo.Reason
        Assert-CancellationTest ($rejectionText.Contains('PID 1234')) 'Running-process refusal omitted the blocking PID.'
    }

    $stages = @(Get-ChildItem -LiteralPath $TestRoot -Directory -Filter 'active.stage.*')
    $backups = @(Get-ChildItem -LiteralPath $TestRoot -Directory -Filter 'active.previous.*')
    if ($Case -in @('stop-restore-failure', 'throw-restore-failure')) {
        Assert-CancellationTest $state.RestoreAttempted 'Rollback was not attempted.'
        Assert-CancellationTest (!(Test-Path -LiteralPath $install)) 'Failed restore unexpectedly produced an active directory.'
        Assert-CancellationTest ($backups.Count -eq 1) 'Failed restore lost the original backup.'
        Assert-CancellationTest ((Get-TreeSnapshot $backups[0].FullName) -ceq $oldSnapshot) 'Failed restore changed the original backup.'
        Assert-CancellationTest ($stages.Count -eq 1) 'Failed restore deleted the staged recovery candidate.'
        Assert-CancellationTest ((Get-FileHash -LiteralPath (Join-Path $stages[0].FullName 'grok-zh.exe') -Algorithm SHA256).Hash -ceq $newHash) 'Failed restore changed the staged candidate.'
        Assert-CancellationTest (Test-Path -LiteralPath (Join-Path $stages[0].FullName '.grok-zh-install.json')) 'Recovery candidate lost its ownership marker.'
    } else {
        Assert-CancellationTest ($stages.Count -eq 0) 'Completed recovery leaked a staging directory.'
        Assert-CancellationTest ($backups.Count -eq 0) 'Completed recovery left the old installation inactive.'
        if ($isFresh) {
            Assert-CancellationTest (!(Test-Path -LiteralPath $install)) 'Cancelled first install left an uncommitted active directory.'
        } elseif ($Case -eq 'success') {
            Assert-CancellationTest ((Get-FileHash -LiteralPath (Join-Path $install 'grok-zh.exe') -Algorithm SHA256).Hash -ceq $newHash) 'Successful installation did not commit the candidate.'
            Assert-CancellationTest ([IO.File]::ReadAllText((Join-Path $install 'official-backup/grok.exe')) -ceq 'historical-official-backup') 'Successful install lost historical official backup.'
        } else {
            Assert-CancellationTest (Test-Path -LiteralPath $install -PathType Container) 'Cancellation left the active installation missing.'
            Assert-CancellationTest ((Get-TreeSnapshot $install) -ceq $oldSnapshot) 'Rollback did not restore every original file byte-for-byte.'
        }
    }
    Assert-CancellationTest ((Get-TreeSnapshot $sharedHome) -ceq $homeBefore) 'Installer modified shared user data.'
    if ($Case -eq 'stop-concurrent-installer') {
        & $InstallerPath -PackageDir $package -InstallDir $install -GrokHome $sharedHome -NoPathUpdate -Confirm:$false 6>$null
        Assert-CancellationTest ((Get-FileHash -LiteralPath (Join-Path $install 'grok-zh.exe') -Algorithm SHA256).Hash -ceq $newHash) 'Retry after pipeline stop could not acquire and commit under the same lock.'
        Assert-CancellationTest (Test-Path -LiteralPath ($install + '.install.lock') -PathType Leaf) 'Stable lock file was deleted after release.'
    }

} finally {
    # The parent also enforces a hard process deadline if a regression hangs
    # Stop/Dispose. Do not issue another stop during the recovery being tested.
    if ($null -ne $invocation -and !$invocation.IsCompleted) {
        $cleanupStop = $runner.BeginStop($null, $null)
        $null = $cleanupStop.AsyncWaitHandle.WaitOne(5000)
    }
    if ($null -eq $invocation -or $invocation.IsCompleted) { $runner.Dispose(); $state.Hit.Dispose() }
}
