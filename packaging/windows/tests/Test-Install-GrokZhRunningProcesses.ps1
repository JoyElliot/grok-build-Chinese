[CmdletBinding()]
param([string]$InstallerPath)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (!$InstallerPath) { $InstallerPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'Install-GrokZh.ps1' }
. $InstallerPath -LibraryOnly

function Assert-ProcessGate {
    param([bool]$Condition, [string]$Message)
    if (!$Condition) { throw $Message }
}

function Assert-ProcessBlocked {
    param([string]$Install, [int]$ProcessId)
    $failure = $null
    try { Assert-NoRunningCommunityInstall -InstallDir $Install } catch { $failure = $_.Exception.Message }
    Assert-ProcessGate ($null -ne $failure) 'A live related process was not blocked.'
    Assert-ProcessGate ($failure.Contains("PID $ProcessId")) "Blocked-process message omitted PID $ProcessId`: $failure"
}

$root = Join-Path ([IO.Path]::GetTempPath()) ('grok process gate ' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
try {
    $install = Join-Path $root 'active'
    New-Item -ItemType Directory -Path $install | Out-Null
    # Mock only OS enumeration. Exercise the real production guard function.
    & {
        $fixture = @{ Processes = @(); Fail = $false }
        function Get-Process {
            [CmdletBinding()]
            param()
            if ($fixture.Fail) { throw 'Injected enumeration access failure.' }
            return $fixture.Processes
        }
        Assert-NoRunningCommunityInstall -InstallDir $install
        foreach ($name in @('grok-zh.exe', 'grok-zh.exe.old', 'grok-zh.exe.old.1234-5.old')) {
            $fixture.Processes = @([pscustomobject]@{ ProcessName = 'grok-zh'; Id = 1234; Path = (Join-Path $install $name); HasExited = $false })
            Assert-ProcessBlocked $install 1234
        }
        $prior = $install + '.previous.20260101-120000-0123abcd'
        $fixture.Processes = @([pscustomobject]@{ ProcessName = 'grok-zh'; Id = 1500; Path = (Join-Path $prior 'grok-zh.exe'); HasExited = $false })
        Assert-ProcessBlocked $install 1500
        $fixture.Processes = @([pscustomobject]@{ ProcessName = 'grok-zh'; Id = 1600; Path = (Join-Path ($prior + '-unrelated') 'grok-zh.exe'); HasExited = $false })
        Assert-NoRunningCommunityInstall -InstallDir $install
        $fixture.Processes = @([pscustomobject]@{ ProcessName = 'grok-zh'; Id = 2000; Path = (Join-Path ($install + '-other') 'grok-zh.exe'); HasExited = $false })
        Assert-NoRunningCommunityInstall -InstallDir $install
        $fixture.Processes = @([pscustomobject]@{ ProcessName = 'not-grok'; Id = 3000; Path = $null; HasExited = $false })
        Assert-NoRunningCommunityInstall -InstallDir $install
        $fixture.Processes = @([pscustomobject]@{ ProcessName = 'grok-zh'; Id = 4000; Path = $null; HasExited = $false })
        Assert-ProcessBlocked $install 4000
        $fixture.Processes[0].HasExited = $true
        Assert-NoRunningCommunityInstall -InstallDir $install
        $inaccessible = [pscustomobject]@{ ProcessName = 'grok-zh'; Id = 5000; HasExited = $false }
        $inaccessible | Add-Member -MemberType ScriptProperty -Name Path -Value { throw 'Injected image access failure.' }
        $fixture.Processes = @($inaccessible)
        Assert-ProcessBlocked $install 5000
        $fixture.Fail = $true
        $failedClosed = $false
        try { Assert-NoRunningCommunityInstall -InstallDir $install } catch { $failedClosed = $true }
        Assert-ProcessGate $failedClosed 'Process enumeration failure must fail closed.'
    }

    # Build a tiny, bounded, locally owned Windows test process with the
    # installed Microsoft .NET Framework compiler. This does not download
    # software, change execution policy or terminate user processes.
    $compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
    if (!(Test-Path -LiteralPath $compiler -PathType Leaf)) {
        $compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework/v4.0.30319/csc.exe'
    }
    Assert-ProcessGate (Test-Path -LiteralPath $compiler -PathType Leaf) 'The Windows process test requires the installed .NET Framework C# compiler.'
    $source = Join-Path $root 'ProcessFixture.cs'
    $executable = Join-Path $root 'ProcessFixture.exe'
    [IO.File]::WriteAllText($source, @'
using System;
using System.IO;
using System.Threading;
internal static class ProcessFixture {
    private static void Main(string[] args) {
        File.WriteAllText(args[0], "ready");
        Thread.Sleep(30000);
    }
}
'@)
    & $compiler /nologo /target:exe "/out:$executable" $source
    Assert-ProcessGate ($LASTEXITCODE -eq 0) 'Could not compile the bounded running-process fixture.'
    foreach ($name in @('grok-zh.exe', 'grok-zh.exe.old', 'grok-zh.exe.old.1234-5.old')) {
        $image = Join-Path $install $name
        Copy-Item -LiteralPath $executable -Destination $image
        $ready = Join-Path $root ($name + '.ready')
        $start = [Diagnostics.ProcessStartInfo]::new()
        $start.FileName = $image
        $start.Arguments = '"' + $ready + '"'
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $child = [Diagnostics.Process]::Start($start)
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(5)
            while (!(Test-Path -LiteralPath $ready) -and !$child.HasExited -and [DateTime]::UtcNow -lt $deadline) {
                Start-Sleep -Milliseconds 20
            }
            Assert-ProcessGate (Test-Path -LiteralPath $ready) "Fixture did not start: $name"
            Assert-ProcessBlocked $install $child.Id
            Assert-ProcessGate (!$child.HasExited) 'The process guard terminated a process.'
            Assert-NoRunningCommunityInstall -InstallDir (Join-Path $root 'unrelated-install')
            Write-Host "PASS actual running image $name (PowerShell $($PSVersionTable.PSVersion))"
        } finally {
            if (!$child.HasExited) { $child.Kill(); $null = $child.WaitForExit(5000) }
            $child.Dispose()
        }
    }
    Write-Host 'Running-process preflight passed; scan-to-activation race remains outside this guard.'
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force
}
