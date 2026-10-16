#requires -Version 5.1
[CmdletBinding()]
param(
    [string]$BundleDirectory = $PSScriptRoot,
    [string]$ResultsDirectory = [IO.Path]::GetTempPath(),
    [switch]$InteractiveTui
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# This is an acceptance harness, never a default-path installer or updater.
$script:AcceptanceUtf8 = [Text.UTF8Encoding]::new($false, $true)
$script:AcceptanceLogDirectory = $null
$script:AcceptanceChecks = [Collections.Generic.List[string]]::new()
$script:AcceptanceCommands = 0

function Assert-Acceptance([bool]$Condition, [string]$Message) {
    if (!$Condition) { throw $Message }
}
function Write-AcceptanceStep([string]$Message) {
    $line = ('[{0}] {1}' -f [DateTime]::UtcNow.ToString('HH:mm:ss'), $Message)
    Write-Host $line
    if ($script:AcceptanceLogDirectory) {
        [IO.File]::AppendAllText((Join-Path $script:AcceptanceLogDirectory 'progress.log'), "$line`r`n", $script:AcceptanceUtf8)
    }
}
function Assert-AcceptancePath([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            Assert-Acceptance (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) "路径不能经过符号链接或重解析点：$cursor"
        }
        $next = [IO.Path]::GetDirectoryName($cursor)
        if ($next -eq $cursor) { break }
        $cursor = $next
    }
}
function Assert-AcceptanceRelative([string]$Name) {
    Assert-Acceptance ($Name -and $Name.Trim() -ceq $Name -and $Name -notmatch '[\\:<>"|?*\x00-\x1f\x7f]') "ZIP 路径无效：$Name"
    foreach ($part in $Name.Split('/')) {
        Assert-Acceptance ($part -and $part -notin @('.', '..') -and !$part.EndsWith('.') -and !$part.EndsWith(' ') -and
            $part -notmatch '^(?i:CON|PRN|AUX|NUL|COM[1-9\u00b9\u00b2\u00b3]|LPT[1-9\u00b9\u00b2\u00b3])(?:\.|$)') "ZIP 路径无效：$Name"
    }
}
function Get-AcceptanceHash([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Read-AcceptanceBundle([string]$Directory) {
    Assert-AcceptancePath $Directory
    $manifestPath = Join-Path $Directory 'SHA256SUMS.txt'
    Assert-Acceptance ((Get-Item -LiteralPath $manifestPath).Length -le 65536) '验收清单过大。'
    $hashes = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($line in Get-Content -LiteralPath $manifestPath -Encoding UTF8) {
        Assert-Acceptance ($line -cmatch '^([0-9a-f]{64})  ([A-Za-z0-9][A-Za-z0-9._-]*)$') '验收清单格式无效。'
        $digest = $matches[1]; $name = $matches[2]
        Assert-Acceptance (!$hashes.ContainsKey($name) -and $name -ine 'SHA256SUMS.txt') '验收清单含重复或自引用条目。'
        $hashes.Add($name, $digest)
    }
    $files = @(Get-ChildItem -LiteralPath $Directory -Force)
    Assert-Acceptance ($files.Count -eq $hashes.Count + 1) '验收包包含缺失或额外文件。请解压到一个全新目录。'
    foreach ($file in $files) {
        Assert-Acceptance (!$file.PSIsContainer -and ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -eq 0) '验收包只允许普通文件。'
        if ($file.Name -ceq 'SHA256SUMS.txt') { continue }
        Assert-Acceptance ($hashes.ContainsKey($file.Name)) "验收清单缺少：$($file.Name)"
        Assert-Acceptance ((Get-AcceptanceHash $file.FullName) -ceq $hashes[$file.Name]) "验收包 SHA-256 不匹配：$($file.Name)"
    }
    foreach ($required in @('ACCEPTANCE-PROVENANCE.json','Run-WindowsAcceptance.ps1','Start-IsolatedTui.ps1','README.zh-CN.md')) {
        Assert-Acceptance ($hashes.ContainsKey($required)) "验收包缺少依赖：$required"
    }
    $provenancePath = Join-Path $Directory 'ACCEPTANCE-PROVENANCE.json'
    Assert-Acceptance ((Get-Item -LiteralPath $provenancePath).Length -le 65536) '来源记录过大。'
    $record = Get-Content -LiteralPath $provenancePath -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Acceptance ($record.schema -eq 1 -and $record.scope.preview_only -eq $true) '来源记录不是预览验收包。'
    Assert-Acceptance ($record.version -cmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' -and
        $record.commit -cmatch '^[0-9a-f]{40}$' -and $record.repository -ceq 'JoyElliot/grok-build-Chinese') '来源版本或仓库标识无效。'
    $expectedFiles = @('ACCEPTANCE-PROVENANCE.json','Run-WindowsAcceptance.ps1','Start-IsolatedTui.ps1','README.zh-CN.md')
    foreach ($kind in @('native','embedded_gnu')) {
        $archive = $record.archives.$kind
        $suffix = if ($kind -eq 'native') { 'msvc' } else { 'gnu' }
        $expected = "grok-zh-$($record.version)-windows-x86_64-$suffix.zip"
        Assert-Acceptance ($archive.file -ceq $expected -and $archive.target -ceq "x86_64-pc-windows-$suffix") 'ZIP 名称/目标不匹配。'
        Assert-Acceptance ($archive.size -gt 0 -and $archive.size -le 536870912 -and $archive.sha256 -cmatch '^[0-9a-f]{64}$') 'ZIP 大小/摘要无效。'
        Assert-Acceptance ($hashes.ContainsKey($expected) -and $hashes[$expected] -ceq $archive.sha256 -and
            (Get-Item -LiteralPath (Join-Path $Directory $expected)).Length -eq $archive.size) 'ZIP 与来源记录不一致。'
        $expectedFiles += $expected
    }
    Assert-Acceptance ($hashes.Count -eq $expectedFiles.Count) '验收清单包含未声明的依赖。'
    Assert-Acceptance ($record.archives.embedded_gnu.embedded_native_sha256 -ceq $record.archives.native.sha256 -and
        $record.archives.embedded_gnu.embedded_native_size -eq $record.archives.native.size) 'GNU 内嵌包不是本轮原生 MSVC ZIP。'
    return $record
}
function Expand-AcceptanceZip([string]$ArchivePath, [string]$Destination, [string]$PackageRoot) {
    # Check every entry before writing; use slash-normalized names on PS 5.1/7.
    Assert-Acceptance (!(Test-Path -LiteralPath $Destination)) 'ZIP 解压目标必须是新目录。'
    Assert-AcceptancePath $Destination
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        Assert-Acceptance ($archive.Entries.Count -gt 0 -and $archive.Entries.Count -le 128) 'ZIP 文件数超限。'
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        $fileNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        $total = 0L
        foreach ($entry in $archive.Entries) {
            $name = $entry.FullName; $isDirectory = $name.EndsWith('/')
            $logical = $name.TrimEnd('/')
            Assert-Acceptance (!$name.Contains('//')) 'ZIP 路径含空组件。'
            Assert-AcceptanceRelative $logical
            Assert-Acceptance (($logical -ceq $PackageRoot -and $isDirectory) -or $logical.StartsWith($PackageRoot + '/', [StringComparison]::Ordinal)) 'ZIP 必须只有预期的顶层目录。'
            Assert-Acceptance ($seen.Add($logical)) 'ZIP 包含大小写重复路径。'
            $unixType = ($entry.ExternalAttributes -shr 16) -band 0xF000
            Assert-Acceptance ($unixType -in @(0,0x8000,0x4000) -and ($entry.ExternalAttributes -band 0x400) -eq 0 -and
                !($unixType -eq 0x8000 -and $isDirectory) -and !($unixType -eq 0x4000 -and !$isDirectory)) 'ZIP 包含不支持的文件类型。'
            Assert-Acceptance ($entry.Length -ge 0 -and $entry.Length -le 536870912 -and (!$isDirectory -or $entry.Length -eq 0)) 'ZIP 文件大小无效。'
            $total += $entry.Length
            Assert-Acceptance ($total -le 805306368) 'ZIP 解压大小超限。'
            if (!$isDirectory) { $null = $fileNames.Add($logical) }
        }
        foreach ($name in $seen) {
            $ancestor = $name
            while ($ancestor.Contains('/')) {
                $ancestor = $ancestor.Substring(0, $ancestor.LastIndexOf('/'))
                Assert-Acceptance (!$fileNames.Contains($ancestor)) 'ZIP 存在文件/目录冲突。'
            }
        }
        $drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($Destination))
        Assert-Acceptance ($drive.AvailableFreeSpace -gt $total + 16777216L) '磁盘空间不足，未开始解压。'
        $null = [IO.Directory]::CreateDirectory($Destination)
        foreach ($entry in $archive.Entries) {
            $target = Join-Path $Destination ($entry.FullName.Replace('/', [IO.Path]::DirectorySeparatorChar))
            if ($entry.FullName.EndsWith('/')) { $null = [IO.Directory]::CreateDirectory($target); continue }
            $null = [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target))
            $sourceStream = $entry.Open(); $targetStream = $null
            try {
                $targetStream = [IO.File]::Open($target, 'CreateNew', 'Write', 'None')
                $buffer = [byte[]]::new(65536); $written = 0L
                while (($count = $sourceStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    $written += $count
                    Assert-Acceptance ($written -le $entry.Length) 'ZIP 实际内容超过声明。'
                    $targetStream.Write($buffer, 0, $count)
                }
                Assert-Acceptance ($written -eq $entry.Length) 'ZIP 文件不完整。'
            } finally { $sourceStream.Dispose(); if ($targetStream) { $targetStream.Dispose() } }
        }
        return Join-Path $Destination $PackageRoot
    } finally { $archive.Dispose() }
}
function New-AcceptanceProcess([string]$Executable, [string]$Arguments) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable; $start.Arguments = $Arguments
    $start.UseShellExecute = $false; $start.WorkingDirectory = $script:AcceptanceWorkspace
    # Do not inherit provider keys, user auth, hooks, or project configuration.
    $start.EnvironmentVariables.Clear()
    foreach ($name in @('SystemRoot','WINDIR','ComSpec','PATHEXT','ProgramFiles','ProgramFiles(x86)','ProgramData','OS')) {
        $value = [Environment]::GetEnvironmentVariable($name, 'Process')
        if ($value) { $start.EnvironmentVariables[$name] = $value }
    }
    foreach ($name in $script:AcceptanceEnvironment.Keys) { $start.EnvironmentVariables[$name] = $script:AcceptanceEnvironment[$name] }
    return $start
}
function Invoke-AcceptanceCommand([string]$Executable, [string]$Arguments, [string]$Label) {
    $script:AcceptanceCommands++
    Write-AcceptanceStep $Label
    $start = New-AcceptanceProcess $Executable $Arguments
    $start.CreateNoWindow = $true; $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = $script:AcceptanceUtf8; $start.StandardErrorEncoding = $script:AcceptanceUtf8
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $start
    $watch = [Diagnostics.Stopwatch]::StartNew()
    try {
        $null = $process.Start(); $stdoutTask = $process.StandardOutput.ReadToEndAsync(); $stderrTask = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit(180000)) {
            # Only this harness-owned process tree, never a pre-existing program.
            & (Join-Path $env:SystemRoot 'System32/taskkill.exe') /PID $process.Id /T /F | Out-Null
            $null = $process.WaitForExit(10000)
            throw "命令超过 180 秒，已请求停止本次测试进程树：$Label"
        }
        Assert-Acceptance ($stdoutTask.Wait(10000) -and $stderrTask.Wait(10000)) "日志未正常关闭：$Label"
        $standardOut = $stdoutTask.GetAwaiter().GetResult(); $standardError = $stderrTask.GetAwaiter().GetResult()
        $log = "Executable: $Executable`r`nArguments: $Arguments`r`nExit: $($process.ExitCode)`r`nMilliseconds: $($watch.ElapsedMilliseconds)`r`nSTDOUT:`r`n$standardOut`r`nSTDERR:`r`n$standardError"
        [IO.File]::WriteAllText((Join-Path $script:AcceptanceLogDirectory ('command-{0:d2}.log' -f $script:AcceptanceCommands)), $log, $script:AcceptanceUtf8)
        Assert-Acceptance ($process.ExitCode -eq 0) "命令失败（退出码 $($process.ExitCode)）：$Label；见 logs。"
        return [pscustomobject]@{ Out=$standardOut; Err=$standardError; Milliseconds=$watch.ElapsedMilliseconds }
    } finally { $process.Dispose() }
}
function Assert-AcceptanceInstalled([string]$Directory, [string]$ExpectedExeHash, [string]$Version) {
    Assert-Acceptance ((Get-AcceptanceHash (Join-Path $Directory 'grok-zh.exe')) -ceq $ExpectedExeHash) '安装后的 EXE 与本轮 MSVC 产物不同。'
    $marker = Get-Content -LiteralPath (Join-Path $Directory '.grok-zh-install.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-Acceptance ($marker.product -ceq 'grok-build-zh' -and $marker.version -ceq $Version -and
        [StringComparer]::OrdinalIgnoreCase.Equals($marker.install_dir, $Directory)) '安装归属或版本记录不一致。'
    Assert-Acceptance (!(Test-Path -LiteralPath (Join-Path $Directory 'grok.cmd')) -and !(Test-Path -LiteralPath (Join-Path $Directory 'agent.cmd'))) '隔离安装不应接管官方命令。'
}

Assert-Acceptance ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT -and [Environment]::Is64BitProcess) '请在 Windows x64 的 64 位 PowerShell 5.1 或 7 中运行。'
Assert-Acceptance ($env:PROCESSOR_ARCHITECTURE -ceq 'AMD64') '本验收包只支持原生 Windows x64；ARM64 仿真未覆盖。'
$bundlePath = [IO.Path]::GetFullPath($BundleDirectory)
$resultParent = [IO.Path]::GetFullPath($ResultsDirectory)
if ($bundlePath -ne [IO.Path]::GetPathRoot($bundlePath)) { $bundlePath = $bundlePath.TrimEnd('\','/') }
if ($resultParent -ne [IO.Path]::GetPathRoot($resultParent)) { $resultParent = $resultParent.TrimEnd('\','/') }
Assert-Acceptance (Test-Path -LiteralPath $resultParent -PathType Container) 'ResultsDirectory 必须是已有目录。'
Assert-AcceptancePath $resultParent
Assert-Acceptance (!$resultParent.Equals($bundlePath, [StringComparison]::OrdinalIgnoreCase) -and
    !$resultParent.StartsWith($bundlePath.TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) '结果目录不能位于只读验收包目录内。'
$sandbox = Join-Path $resultParent ('grok-windows-acceptance-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $sandbox
$script:AcceptanceLogDirectory = Join-Path $sandbox 'logs'; $null = [IO.Directory]::CreateDirectory($script:AcceptanceLogDirectory)
$script:AcceptanceWorkspace = Join-Path $sandbox 'workspace'; $null = [IO.Directory]::CreateDirectory($script:AcceptanceWorkspace)
$dataDirectory = Join-Path $sandbox 'shared-home'; $null = [IO.Directory]::CreateDirectory($dataDirectory)
$isolatedTemp = Join-Path $sandbox 'temp'; $null = [IO.Directory]::CreateDirectory($isolatedTemp)
$profileDirectory = Join-Path $sandbox 'profile'; $null = [IO.Directory]::CreateDirectory($profileDirectory)
$script:AcceptanceEnvironment = @{
    GROK_HOME=$dataDirectory; TEMP=$isolatedTemp; TMP=$isolatedTemp
    HOME=$profileDirectory; USERPROFILE=$profileDirectory
    APPDATA=(Join-Path $profileDirectory 'AppData/Roaming'); LOCALAPPDATA=(Join-Path $profileDirectory 'AppData/Local')
    PATH="$env:SystemRoot\System32;$env:SystemRoot"
}
foreach ($name in @('APPDATA','LOCALAPPDATA')) { $null = [IO.Directory]::CreateDirectory($script:AcceptanceEnvironment[$name]) }
$priorEnvironment = @{}
foreach ($name in $script:AcceptanceEnvironment.Keys) { $priorEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$userPathBefore = [Environment]::GetEnvironmentVariable('Path','User')
$machinePathBefore = [Environment]::GetEnvironmentVariable('Path','Machine')
$result = [ordered]@{ schema=1; status='failed'; sandbox=$sandbox; powershell=$PSVersionTable.PSVersion.ToString(); version=$null; commit=$null; checks=@(); excluded_checks=@(); error=$null; interactive_tui='not-run' }
try {
    Write-AcceptanceStep "结果与日志保留在：$sandbox"
    Write-AcceptanceStep '1/6 验证两个真实 ZIP、脚本依赖、SHA-256 和构建来源'
    $record = Read-AcceptanceBundle $bundlePath
    $testVersion = $record.version; $result.version=$testVersion; $result.commit=$record.commit; $result.excluded_checks=@($record.scope.excluded_checks)
    Write-AcceptanceStep "版本 $testVersion；提交 $($record.commit)；CI $($record.run_url)"
    $script:AcceptanceChecks.Add('bundle-integrity')
    foreach ($name in $script:AcceptanceEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name, $script:AcceptanceEnvironment[$name], 'Process') }
    $nativeRoot = Expand-AcceptanceZip (Join-Path $bundlePath $record.archives.native.file) (Join-Path $sandbox 'native-package') "grok-zh-$testVersion-windows-x86_64-msvc"
    $gnuRoot = Expand-AcceptanceZip (Join-Path $bundlePath $record.archives.embedded_gnu.file) (Join-Path $sandbox 'gnu-package') "grok-zh-$testVersion-windows-x86_64-gnu"
    # Reuse the checked-in production verifier from the authenticated package.
    . (Join-Path $nativeRoot 'Install-GrokZh.ps1') -LibraryOnly
    $null = Read-AndVerifyManifest $nativeRoot; $null = Read-AndVerifyManifest $gnuRoot
    $nativeInfo = Get-Content -LiteralPath (Join-Path $nativeRoot 'BUILD-INFO.txt') -Raw -Encoding UTF8
    $null = Read-GrokBundleProtocol $nativeInfo $testVersion 'x86_64-pc-windows-msvc'
    Assert-Acceptance ($nativeInfo -cmatch ('(?m)^Commit: ' + [regex]::Escape($record.commit) + '\r?$') -and
        $nativeInfo -cmatch '(?m)^Repository: JoyElliot/grok-build-Chinese\r?$') '原生程序 BUILD-INFO 来源不匹配。'
    $gnuProtocol = Read-GrokBundleProtocol (Get-Content -LiteralPath (Join-Path $gnuRoot 'BUILD-INFO.txt') -Raw -Encoding UTF8) $testVersion 'x86_64-pc-windows-gnu'
    $descriptor = Get-GrokEmbeddedDescriptor $gnuProtocol
    Assert-Acceptance ($descriptor.archive_sha256 -ceq $record.archives.native.sha256 -and $descriptor.archive_size -eq $record.archives.native.size) '内嵌运行时描述不是本轮原生 ZIP。'
    $expectedExeHash = Get-AcceptanceHash (Join-Path $nativeRoot 'grok-zh.exe')
    $null = [IO.Directory]::CreateDirectory((Join-Path $dataDirectory 'sessions'))
    $sentinels = @{'auth.json'='{"acceptance_fixture":true}'; 'config.toml'='# synthetic acceptance fixture'; 'sessions/preserve.json'='{"acceptance_fixture":true}'}
    $sentinelHashes = @{}
    foreach ($relative in $sentinels.Keys) {
        $sentinelPath = Join-Path $dataDirectory $relative
        [IO.File]::WriteAllText($sentinelPath, $sentinels[$relative], $script:AcceptanceUtf8)
        $sentinelHashes[$relative] = Get-AcceptanceHash $sentinelPath
    }
    Write-AcceptanceStep '2/6 原生 MSVC CLI 冒烟'
    $nativeVersion = Invoke-AcceptanceCommand (Join-Path $nativeRoot 'grok-zh.exe') '--version' '原生 --version'
    Assert-Acceptance ($nativeVersion.Out -match ('\bgrok-zh ' + [regex]::Escape($testVersion) + '(?:\s|$)')) '原生版本输出不匹配。'
    $null = Invoke-AcceptanceCommand (Join-Path $nativeRoot 'grok-zh.exe') '--help' '原生 --help'
    $script:AcceptanceChecks.Add('native-cli')
    Write-AcceptanceStep '3/6 模拟历史更新器的 EXE-only 提交边界，首次真实离线迁移'
    $legacyDirectory = Join-Path $sandbox 'legacy-entry'; $null = [IO.Directory]::CreateDirectory($legacyDirectory)
    $launcherPath = Join-Path $legacyDirectory 'grok-zh.exe'
    Copy-Item -LiteralPath (Join-Path $gnuRoot 'grok-zh.exe') -Destination $launcherPath
    $launcherHash = Get-AcceptanceHash $launcherPath
    $personalPath = Join-Path $legacyDirectory 'personal.txt'; [IO.File]::WriteAllText($personalPath, 'synthetic-personal-file', $script:AcceptanceUtf8)
    $runtimeDirectory = Join-Path $legacyDirectory '.grok-zh-msvc'
    $script:AcceptanceEnvironment.TEMP = Join-Path $sandbox 'unavailable-temp'; $script:AcceptanceEnvironment.TMP = $script:AcceptanceEnvironment.TEMP
    $candidateVersion = Invoke-AcceptanceCommand $launcherPath '--version' '迁移前的纯版本检查（TEMP 不存在）'
    Assert-Acceptance ($candidateVersion.Out.Contains("grok-zh $testVersion (GNU migration launcher)") -and
        $candidateVersion.Milliseconds -lt 20000 -and !(Test-Path -LiteralPath $runtimeDirectory)) '纯版本检查产生副作用、版本不匹配或超过 20 秒预算。'
    $script:AcceptanceEnvironment.TEMP=$isolatedTemp; $script:AcceptanceEnvironment.TMP=$isolatedTemp
    $firstLaunch = Invoke-AcceptanceCommand $launcherPath '--help' '首次正常启动（无 Python/MinGW PATH）'
    Assert-Acceptance ($firstLaunch.Out -and $firstLaunch.Err.Contains('离线迁移完成')) '首次启动没有完成真实离线迁移。'
    Assert-AcceptanceInstalled $runtimeDirectory $expectedExeHash $testVersion
    $readyPath = Join-Path $runtimeDirectory '.grok-zh-bootstrap-ready'
    Assert-Acceptance ((Get-Content -LiteralPath $readyPath -Raw -Encoding UTF8) -ceq "$testVersion`nwindows-x64-gnu-to-msvc-embedded-v1`n") '迁移 ready 标记不匹配。'
    $readyTime = (Get-Item -LiteralPath $readyPath).LastWriteTimeUtc
    $runtimeHashes = @{}
    foreach ($file in Get-ChildItem -LiteralPath $runtimeDirectory -File -Recurse -Force) { $runtimeHashes[$file.FullName] = Get-AcceptanceHash $file.FullName }
    $script:AcceptanceChecks.Add('embedded-offline-migration')
    Write-AcceptanceStep '4/6 第二次启动及 CLI 转交（TEMP 不存在，确认不重新迁移）'
    $script:AcceptanceEnvironment.TEMP = Join-Path $sandbox 'unavailable-temp'; $script:AcceptanceEnvironment.TMP = $script:AcceptanceEnvironment.TEMP
    foreach ($arguments in @('--help','--version','agent --help','update --help')) {
        $warmResult = Invoke-AcceptanceCommand $launcherPath $arguments "迁移后 $arguments"
        Assert-Acceptance (!$warmResult.Err.Contains('离线迁移完成') -and !$warmResult.Err.Contains('正在离线')) '后续启动重复迁移。'
        if ($arguments -eq '--version') { Assert-Acceptance ($warmResult.Out -match ('\bgrok-zh ' + [regex]::Escape($testVersion) + '(?:\s|$)') -and !$warmResult.Out.Contains('GNU migration launcher')) '迁移后未显示真实 MSVC 版本。' }
    }
    Assert-Acceptance ((Get-Item -LiteralPath $readyPath).LastWriteTimeUtc -eq $readyTime -and (Get-AcceptanceHash $launcherPath) -ceq $launcherHash) '第二次启动修改了 ready 或原 GNU 入口。'
    foreach ($filePath in $runtimeHashes.Keys) { Assert-Acceptance ((Get-AcceptanceHash $filePath) -ceq $runtimeHashes[$filePath]) '第二次启动修改了已安装运行时文件。' }
    $script:AcceptanceEnvironment.TEMP=$isolatedTemp; $script:AcceptanceEnvironment.TMP=$isolatedTemp
    $script:AcceptanceChecks.Add('restart-idempotence')
    Write-AcceptanceStep '5/6 完整 GNU 包的隔离便携安装（显式安装目录、GROK_HOME、NoPathUpdate）'
    $portableDirectory = Join-Path $sandbox 'portable-install'
    # Run the real installer under the same PowerShell edition, with the same
    # minimal child environment; its own EXE probes cannot inherit user secrets.
    $installerPath = (Join-Path $gnuRoot 'Install-GrokZh.ps1').Replace("'", "''")
    $packageArgument = $gnuRoot.Replace("'", "''")
    $installArgument = $portableDirectory.Replace("'", "''")
    $dataArgument = $dataDirectory.Replace("'", "''")
    $installCommand = "`$ErrorActionPreference = 'Stop'; [Console]::OutputEncoding = [Text.UTF8Encoding]::new(`$false); & '$installerPath' -PackageDir '$packageArgument' -InstallDir '$installArgument' -GrokHome '$dataArgument' -NoPathUpdate -OverrideOfficialCommands:`$false -Confirm:`$false; if (!`$?) { exit 1 }"
    $encodedCommand = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($installCommand))
    $shellName = if ($PSVersionTable.PSVersion.Major -ge 6) { 'pwsh.exe' } else { 'powershell.exe' }
    $null = Invoke-AcceptanceCommand (Join-Path $PSHOME $shellName) "-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand $encodedCommand" '完整包安装器（隔离进程）'
    Assert-AcceptanceInstalled $portableDirectory $expectedExeHash $testVersion
    $null = Invoke-AcceptanceCommand (Join-Path $portableDirectory 'grok-zh.exe') '--version' '便携安装 --version'
    $null = Invoke-AcceptanceCommand (Join-Path $portableDirectory 'grok-zh.exe') '--help' '便携安装 --help'
    $script:AcceptanceChecks.Add('portable-install'); $script:AcceptanceChecks.Add('install-marker')
    Write-AcceptanceStep '6/6 检查合成账号/配置/会话、个人文件和 PATH 保留'
    foreach ($relative in $sentinelHashes.Keys) { Assert-Acceptance ((Get-AcceptanceHash (Join-Path $dataDirectory $relative)) -ceq $sentinelHashes[$relative]) "合成数据发生改变：$relative" }
    Assert-Acceptance ([IO.File]::ReadAllText($personalPath) -ceq 'synthetic-personal-file') '个人哨兵文件发生改变。'
    Assert-Acceptance ($env:Path -ceq $script:AcceptanceEnvironment.PATH -and [Environment]::GetEnvironmentVariable('Path','User') -ceq $userPathBefore -and
        [Environment]::GetEnvironmentVariable('Path','Machine') -ceq $machinePathBefore) '测试意外修改了 PATH。'
    $script:AcceptanceChecks.Add('synthetic-data-and-path-preservation')
    $result.status='passed'
    Write-AcceptanceStep '自动验收通过。只覆盖本版本离线迁移/CLI/隔离安装；真实历史客户端、联网连续更新、降级和账号/TUI 尚未验收。'
    if ($InteractiveTui) {
        Write-AcceptanceStep '按显式参数启动可选交互 TUI；使用新的空账号目录和工作目录，不继承 API 密钥。交互内容不写入测试日志。'
        $interactiveHome = Join-Path $sandbox 'interactive-home'; $null = [IO.Directory]::CreateDirectory($interactiveHome)
        $script:AcceptanceEnvironment.GROK_HOME=$interactiveHome
        $interactive = [Diagnostics.Process]::new(); $interactive.StartInfo = New-AcceptanceProcess (Join-Path $portableDirectory 'grok-zh.exe') ''
        try { $null=$interactive.Start(); $interactive.WaitForExit(); $result.interactive_tui="exited:$($interactive.ExitCode)" }
        finally { $interactive.Dispose() }
        Write-AcceptanceStep '交互进程已退出；这只记录退出状态，不代表登录、provider 或 TUI 显示验收通过。'
    }
} catch {
    $result.status='failed'; $result.error=$_.Exception.Message
    Write-AcceptanceStep "失败：$($result.error)"
    throw
} finally {
    foreach ($name in $priorEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name, $priorEnvironment[$name], 'Process') }
    $result.checks=@($script:AcceptanceChecks.ToArray())
    [IO.File]::WriteAllText((Join-Path $sandbox 'RESULT.json'), ($result | ConvertTo-Json -Depth 8), $script:AcceptanceUtf8)
    Write-AcceptanceStep "日志：$(Join-Path $sandbox 'logs')；结果：$(Join-Path $sandbox 'RESULT.json')"
    Write-AcceptanceStep '已恢复测试进程的环境变量。保留全部沙盒文件供检查；确认无需保留后，可自行删除上面这个 grok-windows-acceptance-* 目录。'
}
