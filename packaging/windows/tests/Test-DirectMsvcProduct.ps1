[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ArtifactsDirectory,
    [Parameter(Mandatory = $true)][string]$PackageDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../..'))
$info = Get-Content -LiteralPath (Join-Path $PackageDirectory 'BUILD-INFO.txt') -Raw -Encoding UTF8
if ($info -cnotmatch '(?m)^Version:[ \t]*(\S+)[ \t]*\r?$') { throw '产品包缺少版本。' }
$version = $matches[1]
$parent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/')
$work = Join-Path $parent ('grok-direct-product-test-' + [guid]::NewGuid().ToString('N'))
$utf8 = [Text.UTF8Encoding]::new($false)
$names = @('GROK_HOME', 'GROK_DISABLE_AUTOUPDATER', 'GROK_TELEMETRY_ENABLED', 'GROK_CRASH_HANDLER', 'Path', 'HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA')
$old = @{}
foreach ($name in $names) { $old[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$script:CleanupSafe = $true
function Get-RawUserPathSnapshot {
    # GetEnvironmentVariable('Path', 'User') expands REG_EXPAND_SZ against
    # the current process environment. Isolating USERPROFILE/LOCALAPPDATA must
    # not look like a persistent user PATH change. Compare registry identity,
    # value kind, and unexpanded bytes instead; never write the registry.
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $false)
    try {
        $exists = $null -ne $key -and $key.GetValueNames() -icontains 'Path'
        $kind = $null; $value = $null
        if ($exists) {
            $kind = $key.GetValueKind('Path').ToString()
            $value = $key.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        }
        return ([ordered]@{ Exists = $exists; Kind = $kind; Value = $value } | ConvertTo-Json -Compress)
    } finally { if ($null -ne $key) { $key.Dispose() } }
}
$userPath = Get-RawUserPathSnapshot
function Run-Product([string]$Exe, [string]$Arguments) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Exe; $start.Arguments = $Arguments; $start.UseShellExecute = $false
    $start.CreateNoWindow = $true; $start.WorkingDirectory = $work
    $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = $utf8; $start.StandardErrorEncoding = $utf8
    $p = [Diagnostics.Process]::new(); $p.StartInfo = $start
    try {
        $null = $p.Start(); $stdout = $p.StandardOutput.ReadToEndAsync(); $stderr = $p.StandardError.ReadToEndAsync()
        if (!$p.WaitForExit(30000)) {
            # Keep the Process handle: never invoke taskkill against a bare PID
            # that may have exited and been reused. Descendant termination is
            # uncertain on this path, so preserve the isolated work directory.
            $script:CleanupSafe = $false
            if (!$p.HasExited) { $p.Kill() }
            if (!$p.WaitForExit(10000)) { throw '超时进程无法确认终止，隔离目录已保留。' }
            throw "CLI 超过旧更新器 30 秒期限；子进程清理未确认，隔离目录已保留：$Arguments"
        }
        # A descendant can retain the stdout/stderr handles after the root
        # exits. Do not block indefinitely waiting for inherited pipe handles.
        if (![Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdout, $stderr), 10000)) {
            $script:CleanupSafe = $false
            throw 'CLI 退出后输出管道未关闭；子进程清理未确认，隔离目录已保留。'
        }
        $out = $stdout.GetAwaiter().GetResult(); $err = $stderr.GetAwaiter().GetResult()
        if ($p.ExitCode -ne 0 -or !$out) { throw "实际 MSVC CLI 失败：$Arguments / $($p.ExitCode)`n$err" }
        return $out
    } catch {
        # Faulted pipe tasks and unexpected process API failures are uncertain
        # too; never turn an exceptional path into destructive cleanup.
        $script:CleanupSafe = $false
        throw
    } finally { $p.Dispose() }
}
try {
    $null = [IO.Directory]::CreateDirectory($work)
    $installed = Join-Path $work 'legacy-layout'
    Copy-Item -LiteralPath $PackageDirectory -Destination $installed -Recurse
    $env:GROK_HOME = Join-Path $work 'home'; $null = [IO.Directory]::CreateDirectory($env:GROK_HOME)
    $env:HOME = Join-Path $work 'isolated-user'; $env:USERPROFILE = $env:HOME
    $env:APPDATA = Join-Path $env:HOME 'AppData/Roaming'
    $env:LOCALAPPDATA = Join-Path $env:HOME 'AppData/Local'
    foreach ($directory in @($env:HOME, $env:APPDATA, $env:LOCALAPPDATA)) { $null = [IO.Directory]::CreateDirectory($directory) }
    $env:GROK_DISABLE_AUTOUPDATER = '1'; $env:GROK_TELEMETRY_ENABLED = '0'; $env:GROK_CRASH_HANDLER = '0'
    [IO.File]::WriteAllText((Join-Path $env:GROK_HOME 'preserve.txt'), 'retain', $utf8)
    [IO.File]::WriteAllText((Join-Path $installed 'personal.txt'), 'retain-original', $utf8)
    $program = Join-Path $installed 'grok-zh.exe'
    # Boundary fixture only: no claim that a released GNU updater was run.
    [IO.File]::WriteAllText($program, 'previous executable boundary fixture', $utf8)
    $before = @{}
    foreach ($file in Get-ChildItem -LiteralPath $installed -File -Recurse -Force) {
        if ($file.FullName -ine $program) { $before[$file.FullName] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash }
    }
    $candidate = Join-Path $work 'verified.exe'
    $archive = Join-Path $ArtifactsDirectory "grok-zh-$version-windows-x86_64-gnu.zip"
    $extract = @'
import importlib.util, shutil, sys, zipfile
from pathlib import Path
spec = importlib.util.spec_from_file_location('direct', sys.argv[1]); module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
result = module.validate_direct_archive(Path(sys.argv[2]), sys.argv[3])
with zipfile.ZipFile(sys.argv[2]) as zipped, zipped.open(result['root'] + '/grok-zh.exe') as source, open(sys.argv[4], 'xb') as destination:
    shutil.copyfileobj(source, destination, 1024 * 1024)
'@
    $extract | python -B - (Join-Path $repo '.github/scripts/build-windows-direct-msvc.py') $archive $version $candidate
    if ($LASTEXITCODE -ne 0) { throw '直接兼容包验证或 EXE 提取失败。' }
    $expected = (Get-FileHash -LiteralPath (Join-Path $PackageDirectory 'grok-zh.exe') -Algorithm SHA256).Hash
    if ((Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash -cne $expected) { throw '候选与实际 MSVC 构建不一致。' }
    $env:Path = "$env:SystemRoot\System32;$env:SystemRoot"
    $text = Run-Product $candidate '--version'
    if ($text -cnotmatch "^grok-zh $([regex]::Escape($version)) \(") { throw '候选版本检查失败。' }
    $backup = Join-Path $work 'previous.exe'
    [IO.File]::Replace($candidate, $program, $backup)
    if ([IO.File]::ReadAllText($backup, $utf8) -cne 'previous executable boundary fixture') { throw '替换未保留原始备份。' }
    foreach ($arguments in @('--version', '--help', 'agent --help', 'update --help')) { $null = Run-Product $program $arguments }
    if ((Get-FileHash -LiteralPath $program -Algorithm SHA256).Hash -cne $expected) { throw '运行后 EXE 身份改变。' }
    $afterNames = @(Get-ChildItem -LiteralPath $installed -File -Recurse -Force | ForEach-Object FullName)
    $expectedNames = @($before.Keys) + @($program)
    $fileChanges = @(Compare-Object -ReferenceObject $expectedNames -DifferenceObject $afterNames -CaseSensitive)
    if ($fileChanges.Count -ne 0) {
        $details = @($fileChanges | ForEach-Object {
            "$($_.SideIndicator) $($_.InputObject.Substring($installed.Length + 1))"
        }) -join '; '
        throw "直接启动改动安装目录文件集合（原 $($expectedNames.Count)，现 $($afterNames.Count)；=> 新增，<= 缺失）：$details"
    }
    foreach ($name in $before.Keys) {
        if ((Get-FileHash -LiteralPath $name -Algorithm SHA256).Hash -cne $before[$name]) { throw "EXE-only 替换改动伴随文件：$name" }
    }
    if (Test-Path -LiteralPath (Join-Path $installed '.grok-zh-msvc')) {
        throw '直接启动创建了不应存在的 .grok-zh-msvc 迁移运行时。'
    }
    if ([IO.File]::ReadAllText((Join-Path $env:GROK_HOME 'preserve.txt'), $utf8) -cne 'retain') {
        throw '直接启动改动了隔离 GROK_HOME 中的保留文件。'
    }
    if ((Get-RawUserPathSnapshot) -cne $userPath) {
        throw '直接启动改动了注册表用户 PATH 的存在性、值类型或未展开内容。'
    }
    Write-Host "Direct MSVC boundary smoke passed: $version; native EXE identity, backup, unchanged companions, four CLI commands."
    Write-Host 'This isolated boundary fixture is not a released GNU updater online migration or real-machine AV acceptance.'
} catch {
    $script:CleanupSafe = $false
    throw
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $old[$name], 'Process') }
    if ((Split-Path -Parent ([IO.Path]::GetFullPath($work))) -ine $parent -or !(Split-Path -Leaf $work).StartsWith('grok-direct-product-test-')) { throw '清理边界无效。' }
    if ($script:CleanupSafe -and (Test-Path -LiteralPath $work)) {
        Remove-Item -LiteralPath $work -Recurse -Force
    } elseif (Test-Path -LiteralPath $work) {
        Write-Warning "验证失败或子进程状态不确定；保留隔离目录供检查：$work"
    }
}
