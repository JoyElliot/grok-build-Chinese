[CmdletBinding()]
param([string]$BundleDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$stage = 'resolve bundle directory'
trap {
    Write-Host ("验收入口失败；阶段={0}；行={1}；错误={2}" -f $stage, $_.InvocationInfo.ScriptLineNumber, $_.Exception.Message)
    Write-Host $_.InvocationInfo.PositionMessage
    Write-Host $_.ScriptStackTrace
    break
}
function Resolve-BundleDirectory([string]$Requested, [string]$ScriptPath, [string]$FallbackScriptPath) {
    if ([string]::IsNullOrWhiteSpace($Requested)) {
        if ([string]::IsNullOrWhiteSpace($ScriptPath)) { $ScriptPath = $FallbackScriptPath }
        if ([string]::IsNullOrWhiteSpace($ScriptPath)) { throw '无法确定脚本位置；请显式传入 -BundleDirectory。' }
        $Requested = [IO.Path]::GetDirectoryName($ScriptPath)
    }
    if ([string]::IsNullOrWhiteSpace($Requested)) { throw '验收包目录不能为空。' }
    return (Resolve-Path -LiteralPath $Requested -ErrorAction Stop).Path
}
if ($env:OS -ne 'Windows_NT') { throw '此验收必须在 Windows 原生运行。' }
$bundle = Resolve-BundleDirectory $BundleDirectory $PSCommandPath $MyInvocation.MyCommand.Path
Write-Host "验收包目录：$bundle"
$stage = 'read bundle manifest'
$manifest = Get-Content -LiteralPath (Join-Path $bundle 'SHA256SUMS.json') -Encoding UTF8 -Raw | ConvertFrom-Json
$stage = 'verify required manifest entries'
$required = @('bin/legacy-transaction.exe','bin/current-transaction.exe','inputs/historical-gnu.zip','inputs/native-msvc.zip','inputs/direct-msvc.zip')
foreach ($name in $required) {
    if ($null -eq $manifest.PSObject.Properties[$name]) { throw "验收清单缺少必要项：$name" }
}
$archivePins = @{
    'historical-gnu.zip' = 'f074aea7d5de99d469e4beb821407926a46076225f2bf986d789ce1b118159d8'
    'native-msvc.zip' = 'de9e0ea609a8494ccde3024b0b9b6a669f12387d5a92b39e0d73e34d5ab770ff'
    'direct-msvc.zip' = 'ff7ee4339f578b290d2f7ec42236f75889701ae0badc92d462910cb962c10433'
}
$stage = 'verify pinned input archives'
foreach ($name in $archivePins.Keys) {
    $path = Join-Path $bundle ('inputs/' + $name)
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $archivePins[$name]) { throw "固定产品归档 SHA-256 不符：$name" }
}
$stage = 'verify bundle manifest hashes'
foreach ($entry in $manifest.PSObject.Properties) {
    if ($entry.Name -match '(^[\\/]|:|(^|[\\/])\.\.([\\/]|$))') { throw '验收清单包含不安全路径。' }
    $path = Join-Path $bundle $entry.Name
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $entry.Value) { throw "验收文件 SHA-256 不匹配：$($entry.Name)" }
}
$stage = 'create isolated test directory'
$work = Join-Path ([IO.Path]::GetTempPath()) ('grok-migration-transaction-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $work
$results = Join-Path $work 'results'; $null = New-Item -ItemType Directory -Path $results
# Only these OS fields survive. No caller credentials, proxy, Cargo or developer PATH.
$isolated = @{}
foreach ($key in @('SystemRoot','WINDIR','COMSPEC','OS','PROCESSOR_ARCHITECTURE','NUMBER_OF_PROCESSORS')) {
    $value = [Environment]::GetEnvironmentVariable($key, 'Process')
    if ($null -ne $value) { $isolated[$key] = $value }
}
$isolated['PATH'] = (Join-Path $env:SystemRoot 'System32') + ';' + $env:SystemRoot
foreach ($key in @('HOME','USERPROFILE','APPDATA','LOCALAPPDATA','GROK_HOME','TEMP','TMP')) {
    $isolated[$key] = Join-Path $work $key
    $null = New-Item -ItemType Directory -Path $isolated[$key] -Force
}
$isolated['GROK_AUTH_PATH'] = Join-Path $isolated['GROK_HOME'] 'auth.json'
$isolated['GROK_DISABLE_AUTOUPDATER'] = '1'; $isolated['GROK_TELEMETRY_ENABLED'] = '0'
$isolated['GROK_CRASH_HANDLER'] = '0'; $isolated['GROK_MANAGED_CONFIG'] = '0'
function Stop-TestTree($Process) {
    # taskkill's own wait is bounded; no unbounded shell or output pipes.
    $killInfo = [Diagnostics.ProcessStartInfo]::new()
    $killInfo.FileName = Join-Path $env:SystemRoot 'System32/taskkill.exe'
    $killInfo.Arguments = '/PID ' + $Process.Id + ' /T /F'
    $killInfo.UseShellExecute = $false; $killInfo.CreateNoWindow = $true
    $killer = [Diagnostics.Process]::Start($killInfo)
    try {
        if (!$killer.WaitForExit(10000)) { $killer.Kill(); throw '进程树终止状态不确定。' }
        if ($killer.ExitCode -ne 0) {
            if (!$Process.HasExited) { $Process.Kill(); $null = $Process.WaitForExit(5000) }
            throw 'taskkill 未确认整个进程树已终止；请检查残留进程。'
        }
    }
    finally { $killer.Dispose() }
    if (!$Process.WaitForExit(5000)) { $Process.Kill(); if (!$Process.WaitForExit(5000)) { throw '测试主进程终止状态不确定。' } }
}
function Run-Test([string]$Name,[string]$Arguments,[int]$TimeoutSeconds) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = Join-Path $bundle ('bin/' + $Name + '.exe')
    $start.Arguments = $Arguments; $start.WorkingDirectory = $work
    $start.UseShellExecute = $false; $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $start.EnvironmentVariables.Clear()
    foreach ($key in $isolated.Keys) { $start.EnvironmentVariables[$key] = $isolated[$key] }
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $start
    $stdout = $null; $stderr = $null
    try {
        $null = $process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync(); $stderr = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit($TimeoutSeconds * 1000)) { Stop-TestTree $process; throw "$Name 测试超时。" }
        if (![Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdout,$stderr),10000)) { Stop-TestTree $process; throw "$Name 输出管道未结束，终止状态需检查。" }
        $output = $stdout.GetAwaiter().GetResult(); $errorOutput = $stderr.GetAwaiter().GetResult()
        [IO.File]::WriteAllText((Join-Path $results ($Name + '.log')), $output + "`n" + $errorOutput)
        if ($process.ExitCode -ne 0) { throw "$Name 失败，退出码 $($process.ExitCode)，详见日志。" }
        if ($output -notmatch 'test result: ok\. ([1-9][0-9]*) passed; 0 failed; 0 ignored;') { throw "$Name 未证明非零测试全部执行通过。" }
        $passed = [int]$Matches[1]
        $expected = if ($Name -eq "legacy-transaction") { 8 } else { 20 }
        if ($passed -ne $expected) { throw "$Name 实际测试数量不符：$passed，预期 $expected。" }
        Write-Host "$Name：$passed 项测试通过。"
    } catch {
        [IO.File]::WriteAllText((Join-Path $results ($Name + ".error.txt")), $_.Exception.Message)
        throw
    } finally {
        foreach ($capture in @(@('stdout',$stdout),@('stderr',$stderr))) {
            $task = $capture[1]
            if ($null -ne $task -and $task.Status -eq [Threading.Tasks.TaskStatus]::RanToCompletion) {
                [IO.File]::WriteAllText((Join-Path $results ($Name + '.' + $capture[0] + '.txt')), $task.GetAwaiter().GetResult())
            }
        }
        $process.Dispose()
    }
}
try {
    $stage = 'extract pinned archives'
    foreach ($kind in @('historical-gnu','native-msvc')) {
        Expand-Archive -LiteralPath (Join-Path $bundle ('inputs/' + $kind + '.zip')) -DestinationPath (Join-Path $work $kind)
    }
    $old = Join-Path $work 'historical-gnu/grok-zh-1.0.45-windows-x86_64-gnu/grok-zh.exe'
    $native = Join-Path $work 'native-msvc/grok-zh-1.0.104-windows-x86_64-msvc/grok-zh.exe'
    if ((Get-FileHash -LiteralPath $old -Algorithm SHA256).Hash.ToLowerInvariant() -cne '474918134a312ab5b2200e03092c5f34a99b5f2d3847057fe827e3ff193404ca') { throw '真实旧 GNU EXE 身份不符。' }
    if ((Get-FileHash -LiteralPath $native -Algorithm SHA256).Hash.ToLowerInvariant() -cne 'd7e2e35c9a18ec7d9415ce155cb1677d040925f208aff53f5d29fb7a8ed1d1ce') { throw '真实 MSVC EXE 身份不符。' }
    $isolated['GROK_DIRECT_HISTORICAL_EXE'] = $old
    $isolated['GROK_DIRECT_NATIVE_EXE'] = $native
    $isolated['GROK_DIRECT_NATIVE_ARCHIVE'] = Join-Path $bundle 'inputs/native-msvc.zip'
    $isolated['GROK_DIRECT_DIRECT_ARCHIVE'] = Join-Path $bundle 'inputs/direct-msvc.zip'
    $isolated['GROK_TRANSACTION_RESULTS'] = $results
    $stage = 'run legacy transactions'
    Run-Test 'legacy-transaction' 'community_release::transaction:: --test-threads=1 --nocapture' 480
    $stage = 'run current transactions'
    Run-Test 'current-transaction' 'community_release::transaction:: --test-threads=1 --nocapture' 480
    $oldHash = '474918134a312ab5b2200e03092c5f34a99b5f2d3847057fe827e3ff193404ca'
    $newHash = 'd7e2e35c9a18ec7d9415ce155cb1677d040925f208aff53f5d29fb7a8ed1d1ce'
    foreach ($kind in @('legacy','current')) {
        $cases = @('transaction','hash-rejection','truncation-rejection','real-version-rejection','lock-contention','sharing-lock','source-sharing-failure')
        if ($kind -eq 'current') { $cases += @('direct-reinstall','activation-rollback','atomic-live-self-update','atomic-process-kill','pre-fix-process-kill') }
        else { $cases += @('delayed-writer-overwrite') }
        foreach ($case in $cases) {
            $evidence = Get-Content -LiteralPath (Join-Path $results ($kind + '-' + $case + '.json')) -Encoding UTF8 -Raw | ConvertFrom-Json
            if ($evidence.status -cne 'PASSED') { throw "缺少成功的事务证据：$kind/$case" }
            if ($case -eq 'delayed-writer-overwrite') {
                if ($evidence.risk_reproduced -ne $true -or $evidence.initial_sha256 -cne $newHash -or $evidence.installed_sha256 -cne $oldHash -or $evidence.automatic_candidate_selection_tested -ne $false -or $evidence.whole_immutable_old_executable_update -ne $false) { throw '旧写入者风险复现证据不完整。' }
            }
            if ($case -eq 'pre-fix-process-kill') {
                if ($evidence.risk_reproduced -ne $true -or $evidence.fixed_behavior -ne $false -or $evidence.active_exe_missing -ne $true -or $evidence.backup_preserved -ne $true) { throw '修复前强制终止负向证据不完整。' }
            }
            if ($case -eq 'atomic-live-self-update') {
                if ($evidence.live_mapped_self_update -ne $true -or $evidence.public_entry_direct_launch -ne $true -or $evidence.locked_prior_backup_cases -ne 2 -or $evidence.power_loss_tested -ne $false) { throw '运行中映像原子替换证据不完整。' }
            }
            if ($case -eq 'atomic-process-kill') {
                if (@($evidence.cases).Count -ne 6 -or $evidence.process_kill -cne 'TerminateProcess' -or $evidence.power_loss_tested -ne $false) { throw '原子替换终止注入证据不完整。' }
                foreach ($entry in $evidence.cases) {
                    if ($entry.entry_present_and_launchable -ne $true -or $entry.next_update_succeeded -ne $true) { throw '终止后入口或后续更新未通过。' }
                }
            }
            if ($case -eq 'transaction') {
                $expectedInitial = if ($kind -eq 'legacy') { $oldHash } else { $newHash }
                $expectedVersion = if ($kind -eq 'legacy') { '1.0.45' } else { '1.0.104' }
                if ($evidence.initial_sha256 -cne $expectedInitial -or $evidence.installed_sha256 -cne $newHash -or $evidence.initial_version -cne $expectedVersion -or $evidence.installed_version -cne '1.0.104' -or $evidence.whole_immutable_old_executable_update -ne $false -or $evidence.live_https -ne $false) {
                    throw "事务源/目标或证据范围不符：$kind"
                }
            }
        }
    }
    Write-Host '迁移及恢复回归通过：真实文件事务、运行中测试映像原子替换与强制终止注入已执行。'
    Write-Host '包含旧 GNU 延迟写入覆盖风险和修复前入口缺失的负向复现；测试通过不表示历史旧更新器已修复。'
    Write-Host '网络传输为本地测试接缝；不是未修改旧 EXE 在线升级，也不是两个不同 MSVC 版本的连续升级。'
} finally {
    Write-Host "隔离日志及临时测试文件保留于：$work"
    Write-Host '未改动现有安装、账户、系统 PATH、执行策略或杀毒设置。请仅反馈 results 目录中的测试日志。'
}
