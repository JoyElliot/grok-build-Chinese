Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (!$Condition) {
        throw "断言失败：$Message"
    }
}

function Assert-Throws {
    param(
        [Parameter(Mandatory = $true)][scriptblock]$Action,
        [Parameter(Mandatory = $true)][string]$Message
    )
    $threw = $false
    try {
        & $Action
    } catch {
        $threw = $true
    }
    Assert-True $threw $Message
}

# Test fixtures stand in for the Windows signature API; production code always
# uses Get-AuthenticodeSignature and has no fixture or trust-bypass parameter.
function Get-AuthenticodeSignature {
    param([string]$LiteralPath)
    $isFixture = (Get-Content -LiteralPath $LiteralPath -Raw -Encoding Ascii).StartsWith('fixture-xai-signed:')
    return [pscustomobject]@{
        Status = if ($isFixture) { 'Valid' } else { 'NotSigned' }
        SignerCertificate = if ($isFixture) {
            [pscustomobject]@{ Subject = 'CN=X.AI LLC, O=X.AI LLC, C=US' }
        } else { $null }
    }
}

function New-OfficialFixture {
    param([string]$SharedHome, [string[]]$Names = @('grok.exe'))
    $bin = Join-Path $SharedHome 'bin'
    New-Item -ItemType Directory -Path $bin -Force | Out-Null
    foreach ($name in $Names) {
        Set-Content -LiteralPath (Join-Path $bin $name) -Value "fixture-xai-signed:$name" -Encoding Ascii
    }
}

function Invoke-InteractiveInstaller {
    param(
        [Parameter(Mandatory = $true)][string]$InstallerPath,
        [Parameter(Mandatory = $true)][string]$PackagePath,
        [Parameter(Mandatory = $true)][string]$InstallPath,
        [Parameter(Mandatory = $true)][string]$SharedHome,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$InputLines
    )

    $powerShellExe = "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe"
    $startInfo = New-Object Diagnostics.ProcessStartInfo
    $startInfo.FileName = $powerShellExe
    $scriptedAnswers = $InputLines -join ';'
    $arguments = @($InstallerPath, $PackagePath, $InstallPath, $SharedHome, $scriptedAnswers) |
        ForEach-Object { "'" + $_.Replace("'", "''") + "'" }
    $childScript = '[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)' + "`n" +
        'function Get-AuthenticodeSignature {' + ${function:Get-AuthenticodeSignature}.ToString() + "}`n" +
        ('& {0} -PackageDir {1} -InstallDir {2} -GrokHome {3} -NoPathUpdate -InteractiveCommandSetup -ScriptedCommandSetupAnswers {4} -ShowProgress' -f $arguments)
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($childScript))
    $startInfo.Arguments = "-NoLogo -NoProfile -ExecutionPolicy Bypass -EncodedCommand $encoded"
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = [Text.Encoding]::UTF8
    $startInfo.StandardErrorEncoding = [Text.Encoding]::UTF8
    # Process.Start inherits pwsh's module path without the compatibility fix
    # applied by native PowerShell invocation. Use the PS5 system modules only.
    $startInfo.EnvironmentVariables['PSModulePath'] = "$env:SystemRoot\System32\WindowsPowerShell\v1.0\Modules"

    $process = [Diagnostics.Process]::Start($startInfo)
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
    foreach ($line in $InputLines) {
        $process.StandardInput.WriteLine($line)
    }
    $process.StandardInput.Close()
    $process.WaitForExit()
    return [pscustomobject]@{
        ExitCode = $process.ExitCode
        Stdout = $stdoutTask.Result
        Stderr = $stderrTask.Result
    }
}

$windowsDir = Split-Path -Parent $PSScriptRoot
$installer = Join-Path $windowsDir 'Install-GrokZh.ps1'
$agentShim = Join-Path $windowsDir 'agent-zh.cmd'
$oneClickLauncher = Join-Path $windowsDir '一键安装.cmd'
$commandSetupLauncher = Join-Path $windowsDir '[可选]替换原始启动方式.cmd'
$guide = Join-Path $windowsDir 'INSTALL-WINDOWS.md'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("grok-zh-installer-test-" + [Guid]::NewGuid().ToString('N'))

try {
    $package = Join-Path $testRoot 'package'
    New-Item -ItemType Directory -Path $package -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $package 'grok-zh.exe') -Value 'fake-grok-zh' -Encoding Ascii
    Set-Content -LiteralPath (Join-Path $package 'rg.exe') -Value 'fake-ripgrep' -Encoding Ascii
    Copy-Item -LiteralPath $agentShim -Destination (Join-Path $package 'agent-zh.cmd')
    Copy-Item -LiteralPath $oneClickLauncher -Destination (Join-Path $package '一键安装.cmd')
    Copy-Item -LiteralPath $commandSetupLauncher -Destination (Join-Path $package '[可选]替换原始启动方式.cmd')
    Copy-Item -LiteralPath $installer -Destination (Join-Path $package 'Install-GrokZh.ps1')
    Copy-Item -LiteralPath $guide -Destination (Join-Path $package 'INSTALL-WINDOWS.md')
    Set-Content -LiteralPath (Join-Path $package 'BUILD-INFO.txt') `
        -Value "Version: installer-test`nTarget: x86_64-pc-windows-gnu" -Encoding UTF8
    Set-Content -LiteralPath (Join-Path $package 'LICENSE-grok-build.txt') `
        -Value 'Apache-2.0 test license' -Encoding UTF8
    $licenseFiles = [ordered]@{
        'licenses/ripgrep/COPYING' = 'ripgrep COPYING'
        'licenses/ripgrep/LICENSE-MIT' = 'ripgrep MIT'
        'licenses/ripgrep/UNLICENSE' = 'ripgrep UNLICENSE'
        'licenses/project/THIRD-PARTY-NOTICES' = 'project third-party notices'
        'licenses/project/THIRD_PARTY_NOTICES.md' = 'project tool notices'
        'licenses/project/NOTICE' = 'project NOTICE'
    }
    foreach ($entry in $licenseFiles.GetEnumerator()) {
        $path = Join-Path $package $entry.Key
        New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
        Set-Content -LiteralPath $path -Value $entry.Value -Encoding UTF8
    }

    $names = @(
        'grok-zh.exe',
        'agent-zh.cmd',
        'rg.exe',
        '一键安装.cmd',
        '[可选]替换原始启动方式.cmd',
        'Install-GrokZh.ps1',
        'INSTALL-WINDOWS.md',
        'LICENSE-grok-build.txt',
        'BUILD-INFO.txt',
        'licenses/ripgrep/COPYING',
        'licenses/ripgrep/LICENSE-MIT',
        'licenses/ripgrep/UNLICENSE',
        'licenses/project/THIRD-PARTY-NOTICES',
        'licenses/project/THIRD_PARTY_NOTICES.md',
        'licenses/project/NOTICE'
    )
    $lines = foreach ($name in $names) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash
        "$hash  $name"
    }
    [IO.File]::WriteAllLines(
        (Join-Path $package 'SHA256SUMS.txt'),
        [string[]]$lines,
        (New-Object Text.UTF8Encoding($false))
    )

    $archiveSource = Join-Path $testRoot 'archive-source'
    $archivePackage = Join-Path $archiveSource 'grok-zh-installer-test-windows-x86_64-gnu'
    New-Item -ItemType Directory -Path $archiveSource -Force | Out-Null
    Copy-Item -LiteralPath $package -Destination $archivePackage -Recurse
    $archivePath = Join-Path $testRoot 'nested-package.zip'
    Compress-Archive -LiteralPath $archivePackage -DestinationPath $archivePath
    $archiveExtract = Join-Path $testRoot 'archive-extract'
    Expand-Archive -LiteralPath $archivePath -DestinationPath $archiveExtract
    $archiveRootFiles = @(Get-ChildItem -LiteralPath $archiveExtract -File -Force)
    $archiveRootDirectories = @(Get-ChildItem -LiteralPath $archiveExtract -Directory -Force)
    Assert-True ($archiveRootFiles.Count -eq 0 -and $archiveRootDirectories.Count -eq 1) `
        'Release ZIP 解压后必须只有一个顶层目录'
    $nestedInstall = Join-Path $testRoot 'nested-package-install'
    & (Join-Path $archiveRootDirectories[0].FullName 'Install-GrokZh.ps1') `
        -InstallDir $nestedInstall `
        -GrokHome (Join-Path $testRoot 'unused-nested-home') -NoPathUpdate -Confirm:$false
    Assert-True (Test-Path -LiteralPath (Join-Path $nestedInstall 'grok-zh.exe')) `
        '单一顶层目录 Release ZIP 无法手动安装'

    $legacyPackage = Join-Path $testRoot 'legacy-package'
    Copy-Item -LiteralPath $package -Destination $legacyPackage -Recurse
    $legacyNames = $names[0..6]
    $legacyLines = foreach ($name in $legacyNames) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $legacyPackage $name) -Algorithm SHA256).Hash
        "$hash  $name"
    }
    [IO.File]::WriteAllLines(
        (Join-Path $legacyPackage 'SHA256SUMS.txt'),
        [string[]]$legacyLines,
        (New-Object Text.UTF8Encoding($false))
    )
    $legacyInstall = Join-Path $testRoot 'legacy-package-install'
    & (Join-Path $legacyPackage 'Install-GrokZh.ps1') `
        -PackageDir $legacyPackage -InstallDir $legacyInstall `
        -GrokHome (Join-Path $testRoot 'unused-legacy-home') -NoPathUpdate -Confirm:$false
    Assert-True (Test-Path -LiteralPath (Join-Path $legacyInstall 'grok-zh.exe')) `
        '7 项桥接清单无法由新版安装器安装'
    Assert-True (Test-Path -LiteralPath (Join-Path $legacyInstall 'BUILD-INFO.txt')) `
        '7 项桥接安装丢失构建信息'
    Assert-True (Test-Path -LiteralPath (Join-Path $legacyInstall 'LICENSE-grok-build.txt')) `
        '7 项桥接安装丢失主许可证'
    foreach ($relativeLicense in $licenseFiles.Keys) {
        Assert-True (Test-Path -LiteralPath (Join-Path $legacyInstall $relativeLicense) -PathType Leaf) `
            "7 项桥接安装丢失许可证文件：$relativeLicense"
    }

    Assert-Throws {
        & $installer -PackageDir $package -InstallDir $testRoot `
            -GrokHome (Join-Path $testRoot 'unused-home') -NoPathUpdate -Force -Confirm:$false
    } '安装器错误地允许 PackageDir 与 InstallDir 重叠'

    $overlapHome = Join-Path $testRoot 'overlap-home'
    New-Item -ItemType Directory -Path $overlapHome -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $overlapHome 'auth.json') -Value 'must survive' -Encoding Ascii
    Assert-Throws {
        & $installer -PackageDir $package -InstallDir (Join-Path $overlapHome 'community-bin') `
            -GrokHome $overlapHome -NoPathUpdate -Force -Confirm:$false
    } '安装器错误地允许 InstallDir 与 GROK_HOME 重叠'
    Assert-True (Test-Path -LiteralPath (Join-Path $overlapHome 'auth.json')) '拒绝重叠路径时修改了共享认证数据'

    $defaultInstall = Join-Path $testRoot 'default-install'
    $defaultOutput = @(& $installer -PackageDir $package -InstallDir $defaultInstall `
        -GrokHome (Join-Path $testRoot 'unused-home') -NoPathUpdate -Confirm:$false 6>&1)
    $defaultOutputText = $defaultOutput -join "`n"
    Assert-True ($defaultOutputText.Contains('安装完成：')) '安装器未输出中文完成提示'
    Assert-True (!$defaultOutputText.Contains('Installation complete:')) '安装器仍输出旧英文完成提示'
    Assert-True ($defaultOutputText.Contains('新终端，输入 grok-zh')) '安装器未提示在新终端启动 grok-zh'
    Assert-True ($defaultOutputText.Contains('[可选]替换原始启动方式.cmd')) '安装器未提示可选命令接管入口'
    Assert-True (Test-Path -LiteralPath (Join-Path $defaultInstall 'grok-zh.exe')) '未安装 grok-zh.exe'
    Assert-True (Test-Path -LiteralPath (Join-Path $defaultInstall 'agent-zh.cmd')) '未安装 agent-zh.cmd'
    Assert-True (Test-Path -LiteralPath (Join-Path $defaultInstall 'rg.exe')) '未在 grok-zh.exe 同目录安装 rg.exe'
    Assert-True (Test-Path -LiteralPath (Join-Path $defaultInstall 'BUILD-INFO.txt')) '未安装受校验的构建信息'
    Assert-True (Test-Path -LiteralPath (Join-Path $defaultInstall 'LICENSE-grok-build.txt')) '未安装受校验的主许可证'
    foreach ($relativeLicense in $licenseFiles.Keys) {
        Assert-True (Test-Path -LiteralPath (Join-Path $defaultInstall $relativeLicense) -PathType Leaf) "未安装受校验的许可证文件：$relativeLicense"
    }
    Assert-True (!(Test-Path -LiteralPath (Join-Path $defaultInstall '一键安装.cmd'))) '一键入口只应保留在解压包根，不应复制到安装目录'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $defaultInstall '[可选]替换原始启动方式.cmd'))) '可选入口只应保留在解压包根，不应复制到安装目录'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $defaultInstall 'Install-GrokZh.ps1'))) '安装脚本只应保留在解压包根，不应复制到运行目录'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $defaultInstall 'INSTALL-WINDOWS.md'))) '安装说明只应保留在解压包根，不应复制到运行目录'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $defaultInstall 'grok.cmd'))) '默认安装不应创建 grok.cmd'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $defaultInstall 'agent.cmd'))) '默认安装不应创建 agent.cmd'
    $installedManifestPath = Join-Path $defaultInstall 'SHA256SUMS.txt'
    Assert-True (Test-Path -LiteralPath $installedManifestPath -PathType Leaf) '安装目录缺少自身文件校验清单'
    foreach ($line in Get-Content -LiteralPath $installedManifestPath -Encoding UTF8) {
        Assert-True ($line -match '^([0-9A-Fa-f]{64})  (.+)$') "安装目录校验清单格式无效：$line"
        $installedHash = $matches[1].ToUpperInvariant()
        $installedName = $matches[2]
        $installedPath = Join-Path $defaultInstall $installedName
        Assert-True (Test-Path -LiteralPath $installedPath -PathType Leaf) "安装目录校验清单引用了不存在的文件：$installedName"
        Assert-True ((Get-FileHash -LiteralPath $installedPath -Algorithm SHA256).Hash -eq $installedHash) "安装目录文件哈希不匹配：$installedName"
    }
    $installedManifestText = Get-Content -LiteralPath $installedManifestPath -Raw -Encoding UTF8
    Assert-True (!$installedManifestText.Contains('一键安装.cmd')) '安装目录校验清单不应引用仅位于解压包根的一键入口'
    Assert-True (!$installedManifestText.Contains('[可选]替换原始启动方式.cmd')) '安装目录校验清单不应引用仅位于解压包根的可选入口'

    $progressInstall = Join-Path $testRoot 'progress-install'
    $progressOutput = @(& $installer -PackageDir $package -InstallDir $progressInstall `
        -GrokHome (Join-Path $testRoot 'unused-progress-home') -NoPathUpdate `
        -ShowProgress -Confirm:$false 6>&1)
    $progressOutputText = $progressOutput -join "`n"
    Assert-True (Test-Path -LiteralPath (Join-Path $progressInstall 'grok-zh.exe')) '-ShowProgress 安装未完成'
    Assert-True ($progressOutputText.Contains('[1/4]')) '-ShowProgress 未输出校验阶段进度'
    Assert-True ($progressOutputText.Contains('[2/4]')) '-ShowProgress 未输出复制阶段进度'

    $ps5DefaultInstall = Join-Path $testRoot 'ps5-default-package-install'
    $packageInstaller = Join-Path $package 'Install-GrokZh.ps1'
    $ps5Output = @(& "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass `
        -File $packageInstaller `
        -InstallDir $ps5DefaultInstall `
        -GrokHome (Join-Path $testRoot 'unused-ps5-home') `
        -NoPathUpdate 2>&1)
    Assert-True ($LASTEXITCODE -eq 0) "Windows PowerShell 5.1 -File 默认 PackageDir 调用失败：$($ps5Output -join "`n")"
    Assert-True (Test-Path -LiteralPath (Join-Path $ps5DefaultInstall 'grok-zh.exe')) 'PS5.1 -File 未从脚本目录解析默认 PackageDir'

    $interactiveKeepInstall = Join-Path $testRoot 'interactive-keep-install'
    $interactiveKeepHome = Join-Path $testRoot 'interactive-keep-home'
    New-OfficialFixture $interactiveKeepHome
    $interactiveKeep = Invoke-InteractiveInstaller `
        -InstallerPath $packageInstaller `
        -PackagePath $package `
        -InstallPath $interactiveKeepInstall `
        -SharedHome $interactiveKeepHome `
        -InputLines @('1')
    Assert-True ($interactiveKeep.ExitCode -eq 0) "交互方案 1 执行失败：$($interactiveKeep.Stderr)"
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveKeepInstall 'grok.cmd')) '交互方案 1 未创建 grok.cmd'
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveKeepInstall 'agent.cmd')) '交互方案 1 未创建 agent.cmd'
    Assert-True ($interactiveKeep.Stdout.Contains('[1] 保留官方版，只接管 grok、agent 命令')) '已安装官方版时未显示简化后的菜单'
    Assert-True (!$interactiveKeep.Stdout.Contains('推荐，可恢复')) '选项 1 仍包含旧括号提示'
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveKeepHome 'bin/grok.exe')) '方案 1 删除了官方程序'
    Assert-True ($interactiveKeep.Stdout.Contains('grok --version; agent --help')) '交互方案 1 的验证命令未切换到 grok/agent'
    Assert-True (!$interactiveKeep.Stdout.Contains('grok-zh --version; agent-zh --help')) '交互方案 1 仍输出普通安装的验证命令'

    $interactiveEofInstall = Join-Path $testRoot 'interactive-eof-install'
    $interactiveEofHome = Join-Path $testRoot 'interactive-eof-home'
    New-OfficialFixture $interactiveEofHome
    $interactiveEof = Invoke-InteractiveInstaller `
        -InstallerPath $packageInstaller `
        -PackagePath $package `
        -InstallPath $interactiveEofInstall `
        -SharedHome $interactiveEofHome `
        -InputLines @()
    Assert-True ($interactiveEof.ExitCode -eq 0) "交互输入结束时未能安全取消：$($interactiveEof.Stderr)"
    Assert-True (!(Test-Path -LiteralPath $interactiveEofInstall)) '交互输入结束后仍创建了安装目录'

    $interactiveRemoveHome = Join-Path $testRoot 'interactive-remove-home'
    New-OfficialFixture $interactiveRemoveHome
    $interactiveRemoveInstall = Join-Path $testRoot 'interactive-remove-install'
    $unconfirmed = Invoke-InteractiveInstaller -InstallerPath $packageInstaller -PackagePath $package `
        -InstallPath $interactiveRemoveInstall -SharedHome $interactiveRemoveHome -InputLines @('2')
    Assert-True ($unconfirmed.ExitCode -eq 0 -and !(Test-Path -LiteralPath $interactiveRemoveInstall)) '未收到卸载确认仍执行了安装'
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveRemoveHome 'bin/grok.exe')) '未收到卸载确认仍删除了官方程序'
    $interactiveRemove = Invoke-InteractiveInstaller -InstallerPath $packageInstaller -PackagePath $package `
        -InstallPath $interactiveRemoveInstall -SharedHome $interactiveRemoveHome -InputLines @('2', '2')
    Assert-True ($interactiveRemove.ExitCode -eq 0) "交互卸载失败：$($interactiveRemove.Stderr)"
    Assert-True ($interactiveRemove.Stdout.Contains('[2] 卸载官方版本 grok.exe，并接管 grok、agent 命令')) '选项 2 文案未更新'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $interactiveRemoveHome 'bin/grok.exe'))) '交互确认后未卸载官方程序'
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveRemoveInstall 'grok.cmd')) '交互卸载后未创建命令接管'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $interactiveRemoveInstall 'official-backup'))) '交互卸载仍创建官方备份'

    $interactiveNoOfficialInstall = Join-Path $testRoot 'interactive-no-official-install'
    $interactiveNoOfficial = Invoke-InteractiveInstaller `
        -InstallerPath $packageInstaller `
        -PackagePath $package `
        -InstallPath $interactiveNoOfficialInstall `
        -SharedHome (Join-Path $testRoot 'interactive-no-official-home') `
        -InputLines @()
    Assert-True ($interactiveNoOfficial.ExitCode -eq 0) "无官方程序时直接接管失败：$($interactiveNoOfficial.Stderr)"
    Assert-True (!$interactiveNoOfficial.Stdout.Contains('[1]')) '无官方程序时仍显示选择菜单'
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveNoOfficialInstall 'grok.cmd')) '无官方程序时未自动创建 grok.cmd'
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveNoOfficialInstall 'agent.cmd')) '无官方程序时未自动创建 agent.cmd'

    $interactiveRejectHome = Join-Path $testRoot 'interactive-reject-home'
    $interactiveRejectBin = Join-Path $interactiveRejectHome 'bin'
    New-Item -ItemType Directory -Path $interactiveRejectBin -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $interactiveRejectBin 'grok.exe') -Value 'unsigned-program' -Encoding Ascii
    Set-Content -LiteralPath (Join-Path $interactiveRejectBin 'grok-zh.exe') -Value 'fixture-xai-signed:community-name-must-be-excluded' -Encoding Ascii
    Set-Content -LiteralPath (Join-Path $interactiveRejectBin 'grok.cmd') -Value '@grok-zh.exe %*' -Encoding Ascii
    $unknownHash = (Get-FileHash -LiteralPath (Join-Path $interactiveRejectBin 'grok.exe')).Hash
    $interactiveRejectInstall = Join-Path $testRoot 'interactive-reject-install'
    $interactiveReject = Invoke-InteractiveInstaller `
        -InstallerPath $packageInstaller `
        -PackagePath $package `
        -InstallPath $interactiveRejectInstall `
        -SharedHome $interactiveRejectHome `
        -InputLines @()
    Assert-True ($interactiveReject.ExitCode -eq 0) "未签名程序拒绝流程执行失败：$($interactiveReject.Stderr)"
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveRejectInstall 'grok.cmd')) '仅有中文版或未知程序时未直接接管'
    Assert-True (!$interactiveReject.Stdout.Contains('[1]')) '中文版或未验证程序被误识别为官方版'
    Assert-True ((Get-FileHash -LiteralPath (Join-Path $interactiveRejectBin 'grok.exe')).Hash -eq $unknownHash) '修改了未验证的同名程序'
    Assert-True (Test-Path -LiteralPath (Join-Path $interactiveRejectBin 'grok-zh.exe')) '删除了中文版程序'

    & $installer -PackageDir $package -InstallDir $defaultInstall `
        -GrokHome (Join-Path $testRoot 'unused-home') -NoPathUpdate -Confirm:$false
    Assert-True (Test-Path -LiteralPath (Join-Path $defaultInstall '.grok-zh-install.json')) '重新安装后未保留安装器归属标记'

    $fakeHome = Join-Path $testRoot 'shared-home'
    $fakeOfficialBin = Join-Path $fakeHome 'bin'
    New-Item -ItemType Directory -Path $fakeOfficialBin -Force | Out-Null
    New-OfficialFixture $fakeHome @('grok.exe', 'agent.exe', 'grok-1.0.12.exe')
    Set-Content -LiteralPath (Join-Path $fakeHome 'auth.json') -Value '{"token":"must-survive"}' -Encoding Ascii
    Set-Content -LiteralPath (Join-Path $fakeHome 'config.toml') -Value '# must survive' -Encoding Ascii

    $takeoverInstall = Join-Path $testRoot 'takeover-install'
    $takeoverOutput = @(& $installer -PackageDir $package -InstallDir $takeoverInstall -GrokHome $fakeHome `
        -UninstallOfficial -NoPathUpdate -Confirm:$false 6>&1)
    $takeoverOutputText = $takeoverOutput -join "`n"
    Assert-True ($takeoverOutputText.Contains('新终端，输入 grok 启动中文版')) '命令接管后仍未提示使用 grok 启动'
    Assert-True ($takeoverOutputText.Contains('输入 agent 启动代理模式')) '命令接管后仍未提示使用 agent 启动代理模式'
    Assert-True (!$takeoverOutputText.Contains('新终端，输入 grok-zh 启动中文版')) '命令接管后仍把 grok-zh 显示为主启动命令'
    Assert-True ($takeoverOutputText.Contains('验证命令：grok --version; agent --help')) '命令接管后的验证命令未切换到 grok/agent'
    Assert-True (Test-Path -LiteralPath (Join-Path $takeoverInstall 'grok.cmd')) '命令接管未创建 grok.cmd'
    Assert-True (Test-Path -LiteralPath (Join-Path $takeoverInstall 'agent.cmd')) '命令接管未创建 agent.cmd'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $fakeOfficialBin 'grok.exe'))) '未删除官方 grok.exe'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $fakeOfficialBin 'agent.exe'))) '未删除官方 agent.exe'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $fakeOfficialBin 'grok-1.0.12.exe'))) '未删除官方版本化程序'
    Assert-True (Test-Path -LiteralPath (Join-Path $fakeHome 'auth.json')) '修改了共享 auth.json'
    Assert-True (Test-Path -LiteralPath (Join-Path $fakeHome 'config.toml')) '修改了共享 config.toml'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $takeoverInstall 'official-backup'))) '卸载官方版仍创建了备份'

    & $installer -PackageDir $package -InstallDir $takeoverInstall -GrokHome $fakeHome `
        -NoPathUpdate -Confirm:$false
    Assert-True (!(Test-Path -LiteralPath (Join-Path $takeoverInstall 'grok.cmd'))) '未启用接管的重新安装仍保留 grok.cmd'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $takeoverInstall 'agent.cmd'))) '未启用接管的重新安装仍保留 agent.cmd'
    Assert-True (!(Test-Path -LiteralPath (Join-Path $takeoverInstall 'official-backup'))) '重新安装错误地创建了官方版备份'

    $whatIfInstall = Join-Path $testRoot 'whatif-install'
    $whatIfHome = Join-Path $testRoot 'whatif-home'
    $whatIfOfficialBin = Join-Path $whatIfHome 'bin'
    New-Item -ItemType Directory -Path $whatIfOfficialBin -Force | Out-Null
    New-OfficialFixture $whatIfHome
    Set-Content -LiteralPath (Join-Path $whatIfHome 'auth.json') -Value 'must survive WhatIf' -Encoding Ascii
    & $installer -PackageDir $package -InstallDir $whatIfInstall `
        -GrokHome $whatIfHome -UninstallOfficial -NoPathUpdate -WhatIf
    Assert-True (!(Test-Path -LiteralPath $whatIfInstall)) '-WhatIf 不应创建安装目录'
    Assert-True (Test-Path -LiteralPath (Join-Path $whatIfOfficialBin 'grok.exe')) '-WhatIf 不应移动官方命令'
    Assert-True (Test-Path -LiteralPath (Join-Path $whatIfHome 'auth.json')) '-WhatIf 不应修改共享认证数据'

    $lockedHome = Join-Path $testRoot 'locked-home'
    New-OfficialFixture $lockedHome
    $lockedInstall = Join-Path $testRoot 'locked-install'
    $lockedFile = Join-Path $lockedHome 'bin/grok.exe'
    $held = [IO.File]::Open($lockedFile, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        Assert-Throws {
            & $installer -PackageDir $package -InstallDir $lockedInstall -GrokHome $lockedHome `
                -UninstallOfficial -NoPathUpdate -Confirm:$false
        } '官方程序占用时未中止卸载'
        Assert-True (!(Test-Path -LiteralPath $lockedInstall)) '官方卸载失败后仍激活了新安装'
        Assert-True (Test-Path -LiteralPath $lockedFile) '官方卸载失败时删除了占用文件'
        Assert-True (@(Get-ChildItem -LiteralPath $testRoot -Directory -Filter 'locked-install.stage.*').Count -eq 0) '卸载失败后未清理暂存安装'
    } finally {
        $held.Dispose()
    }

    & {
        $activationHome = Join-Path $testRoot 'activation-failure-home'
        $activationInstall = Join-Path $testRoot 'activation-failure-install'
        New-OfficialFixture $activationHome
        function Move-Item {
            param([string]$LiteralPath, [string]$Destination)
            if ($Destination -eq $activationInstall) { throw 'fixture activation failure' }
            Microsoft.PowerShell.Management\Move-Item -LiteralPath $LiteralPath -Destination $Destination
        }
        Assert-Throws {
            & $installer -PackageDir $package -InstallDir $activationInstall -GrokHome $activationHome `
                -UninstallOfficial -NoPathUpdate -Confirm:$false
        } '模拟目录激活失败时未报告错误'
        Assert-True (Test-Path -LiteralPath (Join-Path $activationHome 'bin/grok.exe')) '中文版激活失败前已删除官方程序'
        Assert-True (!(Test-Path -LiteralPath $activationInstall)) '激活失败仍残留安装目录'
    }

    & {
        $removalHome = Join-Path $testRoot 'removal-failure-home'
        $removalInstall = Join-Path $testRoot 'removal-failure-install'
        New-OfficialFixture $removalHome
        $removalFile = Join-Path $removalHome 'bin/grok.exe'
        function Remove-Item {
            [CmdletBinding()]
            param([string]$LiteralPath, [switch]$Recurse, [switch]$Force)
            if ($LiteralPath -eq $removalFile) { throw 'fixture deletion failure' }
            Microsoft.PowerShell.Management\Remove-Item @PSBoundParameters
        }
        $failureText = ''
        try {
            & $installer -PackageDir $package -InstallDir $removalInstall -GrokHome $removalHome `
                -UninstallOfficial -NoPathUpdate -Confirm:$false
        } catch {
            $failureText = $_.Exception.Message
        }
        Assert-True ($failureText.Contains('官方版卸载未完成')) '删除失败未明确报告卸载未完成'
        Assert-True (Test-Path -LiteralPath (Join-Path $removalInstall 'grok-zh.exe')) '删除失败后丢失可用的中文版'
        Assert-True (Test-Path -LiteralPath (Join-Path $removalInstall 'grok.cmd')) '删除失败后丢失中文版兼容命令'
        Assert-True (Test-Path -LiteralPath $removalFile) '删除失败后官方文件状态异常'
    }

    # Load only function definitions, without executing the installer body, to
    # exercise a changed removal plan and a private npm stub (never real npm).
    $parseTokens = $null
    $parseErrors = $null
    $installerAst = [Management.Automation.Language.Parser]::ParseFile($installer, [ref]$parseTokens, [ref]$parseErrors)
    Assert-True ($parseErrors.Count -eq 0) '安装器存在 PowerShell 解析错误'
    $functionNames = @('Resolve-FullPath', 'Test-PathsOverlap', 'Assert-NoReparsePointInPath',
        'Test-XaiSignedExecutable', 'Get-OfficialInstallation', 'Get-OfficialNpmPackage',
        'Assert-OfficialInstallationRemovable', 'Remove-OfficialInstallation')
    foreach ($definition in $installerAst.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] }, $false)) {
        if ($functionNames -contains $definition.Name) {
            . ([scriptblock]::Create($definition.Extent.Text))
        }
    }
    $changedHome = Join-Path $testRoot 'changed-home'
    New-OfficialFixture $changedHome
    $changedPath = Join-Path $changedHome 'bin/grok.exe'
    $changedPlan = Get-OfficialInstallation -OfficialBin (Join-Path $changedHome 'bin') -CommunityInstallDir $defaultInstall
    Set-Content -LiteralPath $changedPath -Value 'fixture-xai-signed:changed-after-selection' -Encoding Ascii
    Assert-Throws { Remove-OfficialInstallation $changedPlan } '检测后文件改变仍执行了卸载'
    Assert-True (Test-Path -LiteralPath $changedPath) '删除了与检测结果不一致的程序'

    $npmBin = Join-Path $testRoot 'npm-stub'
    $npmPrefix = Join-Path $testRoot 'npm-prefix'
    $npmRoot = Join-Path $npmPrefix 'node_modules'
    $npmPackageRoot = Join-Path $npmRoot '@xai-official/grok'
    New-Item -ItemType Directory -Path $npmBin, $npmPackageRoot -Force | Out-Null
    $npmManifest = Join-Path $npmPackageRoot 'package.json'
    $npmStub = Join-Path $npmBin 'npm.cmd'
    [IO.File]::WriteAllText($npmStub, "@echo off`r`nif `"%~1`"==`"root`" (`r`n  echo $npmRoot`r`n  exit /b 0`r`n)`r`nexit /b 7`r`n", [Text.Encoding]::ASCII)
    $previousPath = $env:Path
    try {
        $env:Path = $npmBin
        Set-Content -LiteralPath $npmManifest -Value '{"name":"grok-build-zh"}' -Encoding Ascii
        Assert-True ($null -eq (Get-OfficialNpmPackage)) '仅路径相同的中文 npm 包被识别为官方包'
        Set-Content -LiteralPath $npmManifest -Value '{"name":"@xai-official/grok"}' -Encoding Ascii
        $npmPlan = Get-OfficialInstallation -OfficialBin (Join-Path $testRoot 'absent-bin') `
            -CommunityInstallDir $defaultInstall -IncludeGlobalCommands
        Assert-True ($npmPlan.Installed -and $null -ne $npmPlan.NpmPackage) '未识别官方 npm 包'
        Assert-True ($npmPlan.NpmPackage.Prefix -eq $npmPrefix) '官方 npm 卸载前缀与检测位置不一致'
        Assert-Throws { Remove-OfficialInstallation $npmPlan } 'npm 卸载失败仍报告成功'
        Assert-True (Test-Path -LiteralPath $npmManifest) 'npm 卸载失败后擅自删除了包文件'

        $npmSuccess = Join-Path $npmBin 'npm-success.ps1'
        [pscustomobject]@{ Prefix = $npmPrefix; Manifest = $npmManifest } | ConvertTo-Json |
            Set-Content -LiteralPath (Join-Path $npmBin 'npm-success.json') -Encoding UTF8
        @'
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$NpmArguments)
$fixture = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'npm-success.json') -Raw | ConvertFrom-Json
$expected = @('uninstall', '--global', '--prefix', $fixture.Prefix,
    '--ignore-scripts', '--no-audit', '--no-fund', '@xai-official/grok')
if (($NpmArguments -join "`n") -cne ($expected -join "`n")) { throw 'unexpected npm uninstall arguments' }
Remove-Item -LiteralPath $fixture.Manifest
$global:LASTEXITCODE = 0
'@ | Set-Content -LiteralPath $npmSuccess -Encoding UTF8
        $npmPlan.NpmPackage.NpmPath = $npmSuccess
        Remove-OfficialInstallation $npmPlan | Out-Null
        Assert-True (!(Test-Path -LiteralPath $npmManifest)) 'npm 成功卸载后仍存在官方包清单'
    } finally {
        $env:Path = $previousPath
        $global:LASTEXITCODE = 0
    }

    $oneClickText = Get-Content -LiteralPath $oneClickLauncher -Raw
    $commandSetupText = Get-Content -LiteralPath $commandSetupLauncher -Raw
    Assert-True ($oneClickText.Contains('ExecutionPolicy Bypass')) '一键入口未使用仅限子进程的 ExecutionPolicy Bypass'
    Assert-True ($oneClickText.Contains('%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe')) '一键入口未使用受信任的系统 Windows PowerShell 路径'
    Assert-True ($oneClickText.Contains('-PackageDir "%~dp0."')) '一键入口未安全传入解压包目录'
    Assert-True ($oneClickText.Contains('pause')) '一键入口未在安装结束后等待用户关闭窗口'
    Assert-True ($oneClickText.Contains('INSTALL_EXIT')) '一键入口未保留安装脚本退出码'
    Assert-True ($oneClickText.Contains('-ShowProgress')) '一键入口未启用安装进度'
    Assert-True ($oneClickText.Contains('%~dp0Install-GrokZh.ps1')) '一键入口未从自身目录定位安装器'
    Assert-True ($commandSetupText.Contains('-InteractiveCommandSetup')) '可选入口未启用交互式命令接管菜单'
    Assert-True ($commandSetupText.Contains('-ShowProgress')) '可选入口未启用安装进度'
    Assert-True ($commandSetupText.Contains('%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe')) '可选入口未使用受信任的系统 Windows PowerShell 路径'
    Assert-True ($commandSetupText.Contains('-PackageDir "%~dp0."')) '可选入口未安全传入解压包目录'
    Assert-True ($commandSetupText.Contains('pause')) '可选入口未在安装结束后等待用户关闭窗口'
    Assert-True ($commandSetupText.Contains('INSTALL_EXIT')) '可选入口未保留安装脚本退出码'
    foreach ($launcher in @($oneClickLauncher, $commandSetupLauncher)) {
        $nonAsciiBytes = @([IO.File]::ReadAllBytes($launcher) | Where-Object { $_ -gt 127 })
        Assert-True ($nonAsciiBytes.Count -eq 0) "CMD 启动器内容必须保持 ASCII，避免旧版控制台乱码：$launcher"
    }

    Write-Host 'Windows 安装器测试通过。'
} finally {
    if (Test-Path -LiteralPath $testRoot) {
        $resolvedTestRoot = [IO.Path]::GetFullPath($testRoot)
        $expectedPrefix = Join-Path ([IO.Path]::GetTempPath()) 'grok-zh-installer-test-'
        if (!$resolvedTestRoot.StartsWith($expectedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw "拒绝清理临时测试目录以外的路径：$resolvedTestRoot"
        }
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
