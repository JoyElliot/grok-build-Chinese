[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$BaselineInstallerPath,
    [Parameter(Mandatory = $true)][string]$EvidencePath
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# CI materializes this exact Git blob as bytes. Never edit the historical
# installer or substitute the fixed script to manufacture a reproduction.
$commit = 'b3ca1daa84ec7f82a4fb82b0143868a802bf85c9'
$blob = 'cb857d881b1c0350842b3c5aabaceaa909b0e1e8'
$sha256 = 'c400683ff673c7d0111519c1bb4bc355006e62f45cb609ede0183496b85afdab'
$bytes = [IO.File]::ReadAllBytes($BaselineInstallerPath)
if ($bytes.Length -ne 81382 -or (Get-FileHash -LiteralPath $BaselineInstallerPath -Algorithm SHA256).Hash -ine $sha256) {
    throw 'Historical installer bytes do not match the pinned baseline.'
}
$header = [Text.Encoding]::ASCII.GetBytes('blob ' + $bytes.Length + [char]0)
$blobBytes = New-Object byte[] ($header.Length + $bytes.Length)
[Array]::Copy($header, 0, $blobBytes, 0, $header.Length)
[Array]::Copy($bytes, 0, $blobBytes, $header.Length, $bytes.Length)
$sha1 = [Security.Cryptography.SHA1]::Create()
try { $actualBlob = ([BitConverter]::ToString($sha1.ComputeHash($blobBytes))).Replace('-', '').ToLowerInvariant() }
finally { $sha1.Dispose() }
if ($actualBlob -cne $blob) { throw 'Historical installer Git blob identity differs from the pinned source.' }

$worker = Join-Path $PSScriptRoot 'Test-Install-GrokZhCancellation.ps1'
$root = Join-Path ([IO.Path]::GetTempPath()) ('grok installer baseline ' + [guid]::NewGuid().ToString('N'))
$report = [ordered]@{
    schema = 1
    risk_reproduced = $false
    case = 'stop-after-backup'
    baseline_commit = $commit
    baseline_git_blob = $blob
    baseline_sha256 = $sha256
    baseline_bytes = $bytes.Length
    harness_sha256 = (Get-FileHash -LiteralPath $worker -Algorithm SHA256).Hash.ToLowerInvariant()
    powershell_version = $PSVersionTable.PSVersion.ToString()
}
$process = $null
try {
    New-Item -ItemType Directory -Path $root | Out-Null
    $quoted = @($worker, ([IO.Path]::GetFullPath($BaselineInstallerPath)), $root) | ForEach-Object { "'" + $_.Replace("'", "''") + "'" }
    $invoke = '& {0} -Worker -Case stop-after-backup -InstallerPath {1} -TestRoot {2}' -f $quoted
    $command = '$ProgressPreference = ''SilentlyContinue''; [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); try { ' + $invoke + ' } catch { [Console]::Error.WriteLine($_.ToString() + [Environment]::NewLine + $_.ScriptStackTrace); exit 1 }'
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = [Diagnostics.Process]::GetCurrentProcess().MainModule.FileName
    $start.Arguments = '-NoLogo -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand ' + [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
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
        $process.Kill(); $null = $process.WaitForExit(5000)
        throw 'Baseline reproduction exceeded its 60-second process deadline.'
    }
    $out = $stdout.GetAwaiter().GetResult()
    $err = $stderr.GetAwaiter().GetResult()
    $evidenceDirectory = Split-Path -Parent ([IO.Path]::GetFullPath($EvidencePath))
    New-Item -ItemType Directory -Path $evidenceDirectory -Force | Out-Null
    [IO.File]::WriteAllText($EvidencePath + '.stdout.log', $out, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($EvidencePath + '.stderr.log', $err, [Text.UTF8Encoding]::new($false))
    $report['child_exit_code'] = $process.ExitCode
    # This exact assertion is reached only after the shared harness verifies
    # a real Stopped pipeline and EndInvoke cancellation, not a synthetic throw.
    $expectedFailure = 'Completed recovery left the old installation inactive.'
    $report['real_stop_assertions_passed'] = $err.Contains($expectedFailure)
    if ($process.ExitCode -ne 1 -or !$report.real_stop_assertions_passed) {
        throw "Baseline did not reach the precise post-stop recovery failure.`n$out`n$err"
    }
    $active = Join-Path $root 'active'
    $backups = @(Get-ChildItem -LiteralPath $root -Directory -Filter 'active.previous.*')
    $stages = @(Get-ChildItem -LiteralPath $root -Directory -Filter 'active.stage.*')
    $report['active_missing'] = !(Test-Path -LiteralPath $active)
    $report['backup_count'] = $backups.Count
    $report['stage_count'] = $stages.Count
    if (!$report.active_missing -or $backups.Count -ne 1 -or $stages.Count -ne 0) {
        throw 'Baseline filesystem does not show missing active entry, one preserved backup and deleted stage.'
    }
    $oldExe = Join-Path $backups[0].FullName 'grok-zh.exe'
    $marker = [IO.File]::ReadAllText((Join-Path $backups[0].FullName '.grok-zh-install.json')) | ConvertFrom-Json
    $report['old_program_bytes_intact'] = [IO.File]::ReadAllText($oldExe) -ceq 'cancellation-fixture:grok-zh.exe'
    $report['old_backup_bytes_intact'] = [IO.File]::ReadAllText((Join-Path $backups[0].FullName 'official-backup/grok.exe')) -ceq 'historical-official-backup'
    $report['old_ownership_intact'] = $marker.product -ceq 'grok-build-zh' -and $marker.install_dir -ieq $active
    $report['shared_data_intact'] = [IO.File]::ReadAllText((Join-Path $root 'shared-home/auth.json')) -ceq 'shared-auth-must-not-change' -and
        [IO.File]::ReadAllText((Join-Path $root 'shared-home/settings.json')) -ceq 'shared-settings-must-not-change'
    $report['baseline_source_unchanged'] = (Get-FileHash -LiteralPath $BaselineInstallerPath -Algorithm SHA256).Hash -ieq $sha256
    if (!$report.old_program_bytes_intact -or !$report.old_backup_bytes_intact -or !$report.old_ownership_intact -or
        !$report.shared_data_intact -or !$report.baseline_source_unchanged) {
        throw 'Baseline reproduction did not preserve expected original/source/shared-data bytes.'
    }
    $report['risk_reproduced'] = $true
    Write-Host "Reproduced pinned baseline cancellation loss (PowerShell $($PSVersionTable.PSVersion)); evidence: $EvidencePath"
} catch {
    $report['failure'] = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process) { $process.Dispose() }
    $directory = Split-Path -Parent ([IO.Path]::GetFullPath($EvidencePath))
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    [IO.File]::WriteAllText($EvidencePath, ($report | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
