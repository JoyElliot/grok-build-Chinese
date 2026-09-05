<#
.SYNOPSIS
安装或升级 Grok Build 中文社区版的 Windows 完整安装包。

.DESCRIPTION
安装前校验 SHA256SUMS.txt 及必需文件，将程序部署到独立安装目录，并可选择更新用户 Path、提供 grok/agent 兼容命令，或卸载已验证的官方程序。共享的 GROK_HOME 用户数据不会被删除。

.PARAMETER PackageDir
已解压安装包的根目录。默认使用本脚本所在目录。

.PARAMETER InstallDir
程序安装目录。默认使用当前用户 LocalAppData 下的 Programs\grok-zh\bin。

.PARAMETER GrokHome
共享 GROK_HOME 数据目录。默认读取 GROK_HOME 环境变量，否则使用当前用户目录下的 .grok。

.PARAMETER OverrideOfficialCommands
额外创建 grok 和 agent 兼容命令，但不移动已有官方可执行文件。

.PARAMETER UninstallOfficial
删除已验证的官方 grok.exe/agent.exe，不创建备份，并创建对应兼容命令。默认路径安装还会卸载已识别的官方 npm 包。不会删除共享用户数据。

.PARAMETER InteractiveCommandSetup
先检查官方版；没有官方版时直接接管命令，有官方版时显示保留或卸载官方版的数字菜单。

.PARAMETER ScriptedCommandSetupAnswers
仅供安装器自动化验证使用。以分号分隔菜单答案；正常安装和双击入口不要设置。

.PARAMETER ShowProgress
显示安装包校验和程序复制的百分比进度。用于双击入口；自动化调用可不启用。

.PARAMETER NoPathUpdate
不修改当前用户 Path。

.PARAMETER Force
允许安装到缺少本安装器归属标记的现有目录。使用前请先检查目录内容。

.EXAMPLE
& .\Install-GrokZh.ps1

使用默认路径安装，并将安装目录置于当前用户 Path 首位。

.EXAMPLE
& .\Install-GrokZh.ps1 -NoPathUpdate -WhatIf

预览安装操作，不修改用户 Path，也不写入文件。
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [string]$PackageDir,
    [string]$InstallDir,
    [string]$GrokHome,
    [switch]$OverrideOfficialCommands,
    [switch]$UninstallOfficial,
    [switch]$InteractiveCommandSetup,
    [string]$ScriptedCommandSetupAnswers,
    [switch]$ShowProgress,
    [switch]$NoPathUpdate,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $expanded = $Path.Trim().Trim('"')
    if ([string]::IsNullOrWhiteSpace($expanded)) {
        throw '路径不能为空。'
    }
    $full = [IO.Path]::GetFullPath($expanded)
    $root = [IO.Path]::GetPathRoot($full)
    if ([StringComparer]::OrdinalIgnoreCase.Equals($full, $root)) {
        return $full
    }
    return $full.TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
}

function Test-SamePath {
    param(
        [Parameter(Mandatory = $true)][string]$Left,
        [Parameter(Mandatory = $true)][string]$Right
    )

    return [StringComparer]::OrdinalIgnoreCase.Equals(
        (Resolve-FullPath $Left),
        (Resolve-FullPath $Right)
    )
}

function Test-PathsOverlap {
    param(
        [Parameter(Mandatory = $true)][string]$Left,
        [Parameter(Mandatory = $true)][string]$Right
    )

    $leftPath = Resolve-FullPath $Left
    $rightPath = Resolve-FullPath $Right
    if ([StringComparer]::OrdinalIgnoreCase.Equals($leftPath, $rightPath)) {
        return $true
    }
    $separator = [IO.Path]::DirectorySeparatorChar
    $leftPrefix = if ($leftPath.EndsWith($separator)) { $leftPath } else { "$leftPath$separator" }
    $rightPrefix = if ($rightPath.EndsWith($separator)) { $rightPath } else { "$rightPath$separator" }
    return $leftPath.StartsWith($rightPrefix, [StringComparison]::OrdinalIgnoreCase) -or
        $rightPath.StartsWith($leftPrefix, [StringComparison]::OrdinalIgnoreCase)
}

function Assert-NoReparsePointInPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $current = $Path
    while (![string]::IsNullOrWhiteSpace($current)) {
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "$Label 的路径链包含符号链接或重解析点，为避免绕过数据目录边界，安装已停止：$current"
            }
        }
        $parent = Split-Path -Parent $current
        if ([string]::IsNullOrWhiteSpace($parent) -or
            [StringComparer]::OrdinalIgnoreCase.Equals($parent, $current)) {
            break
        }
        $current = $parent
    }
}

function Assert-NoReparsePointTree {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $pending = [Collections.Generic.Queue[string]]::new()
    $pending.Enqueue($Path)
    while ($pending.Count -gt 0) {
        $current = $pending.Dequeue()
        $item = Get-Item -LiteralPath $current -Force
        if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "$Label 不能包含符号链接或重解析点：$current"
        }
        if ($item.PSIsContainer) {
            foreach ($child in Get-ChildItem -LiteralPath $current -Force) {
                if (($child.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                    throw "$Label 不能包含符号链接或重解析点：$($child.FullName)"
                }
                if ($child.PSIsContainer) {
                    $pending.Enqueue($child.FullName)
                }
            }
        }
    }
}

function Get-DefaultInstallDir {
    $localAppData = [Environment]::GetFolderPath('LocalApplicationData')
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        $localAppData = $env:LOCALAPPDATA
    }
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        throw '无法确定当前用户的 LocalAppData 目录。'
    }
    return Join-Path $localAppData 'Programs\grok-zh\bin'
}

function Get-DefaultGrokHome {
    if (![string]::IsNullOrWhiteSpace($env:GROK_HOME)) {
        return $env:GROK_HOME
    }
    $profile = [Environment]::GetFolderPath('UserProfile')
    if ([string]::IsNullOrWhiteSpace($profile)) {
        $profile = $env:USERPROFILE
    }
    if ([string]::IsNullOrWhiteSpace($profile)) {
        throw '无法确定当前用户的个人资料目录。'
    }
    return Join-Path $profile '.grok'
}

function Get-FileSha256 {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$DisplayName,
        [Parameter(Mandatory = $true)][long]$BytesBefore,
        [Parameter(Mandatory = $true)][long]$TotalBytes,
        [switch]$ProgressEnabled
    )

    if (!$ProgressEnabled.IsPresent) {
        return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
    }

    $stream = $null
    $sha256 = $null
    try {
        $stream = [IO.File]::Open(
            $Path,
            [IO.FileMode]::Open,
            [IO.FileAccess]::Read,
            [IO.FileShare]::Read
        )
        $sha256 = [Security.Cryptography.SHA256]::Create()
        $buffer = New-Object byte[] 4194304
        $processed = [long]0
        $lastPercent = -1
        while (($read = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            [void]$sha256.TransformBlock($buffer, 0, $read, $buffer, 0)
            $processed += $read
            $overall = $BytesBefore + $processed
            $percent = [Math]::Min(99, [int][Math]::Floor(
                ($overall * 100.0) / [Math]::Max([long]1, $TotalBytes)
            ))
            if ($percent -ne $lastPercent) {
                Write-Progress -Id 1 -Activity '正在校验安装包完整性' `
                    -Status "$DisplayName（$percent%）" -PercentComplete $percent
                $lastPercent = $percent
            }
        }
        $empty = New-Object byte[] 0
        [void]$sha256.TransformFinalBlock($empty, 0, 0)
        return ([BitConverter]::ToString($sha256.Hash)).Replace('-', '').ToUpperInvariant()
    } finally {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
        if ($null -ne $sha256) {
            $sha256.Dispose()
        }
    }
}

function Copy-PackageFile {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [Parameter(Mandatory = $true)][string]$DisplayName,
        [Parameter(Mandatory = $true)][long]$BytesBefore,
        [Parameter(Mandatory = $true)][long]$TotalBytes,
        [switch]$ProgressEnabled
    )

    if (!$ProgressEnabled.IsPresent) {
        Copy-Item -LiteralPath $Source -Destination $Destination
        return (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToUpperInvariant()
    }

    $input = $null
    $output = $null
    $sha256 = $null
    $hashResult = $null
    try {
        $input = [IO.File]::Open(
            $Source,
            [IO.FileMode]::Open,
            [IO.FileAccess]::Read,
            [IO.FileShare]::Read
        )
        $output = [IO.File]::Open(
            $Destination,
            [IO.FileMode]::Create,
            [IO.FileAccess]::Write,
            [IO.FileShare]::None
        )
        $sha256 = [Security.Cryptography.SHA256]::Create()
        $buffer = New-Object byte[] 4194304
        $processed = [long]0
        $lastPercent = -1
        while (($read = $input.Read($buffer, 0, $buffer.Length)) -gt 0) {
            [void]$sha256.TransformBlock($buffer, 0, $read, $buffer, 0)
            $output.Write($buffer, 0, $read)
            $processed += $read
            $overall = $BytesBefore + $processed
            $percent = [Math]::Min(99, [int][Math]::Floor(
                ($overall * 100.0) / [Math]::Max([long]1, $TotalBytes)
            ))
            if ($percent -ne $lastPercent) {
                Write-Progress -Id 2 -Activity '正在复制程序文件' `
                    -Status "$DisplayName（$percent%）" -PercentComplete $percent
                $lastPercent = $percent
            }
        }
        $empty = New-Object byte[] 0
        [void]$sha256.TransformFinalBlock($empty, 0, 0)
        $hashResult = ([BitConverter]::ToString($sha256.Hash)).Replace('-', '').ToUpperInvariant()
    } finally {
        if ($null -ne $output) {
            $output.Dispose()
        }
        if ($null -ne $input) {
            $input.Dispose()
        }
        if ($null -ne $sha256) {
            $sha256.Dispose()
        }
    }
    [IO.File]::SetLastWriteTimeUtc($Destination, [IO.File]::GetLastWriteTimeUtc($Source))
    return $hashResult
}

function Test-XaiSignedExecutable {
    param([Parameter(Mandatory = $true)][string]$Path)

    try {
        $signature = Get-AuthenticodeSignature -LiteralPath $Path
        $subject = if ($null -ne $signature.SignerCertificate) {
            $signature.SignerCertificate.Subject
        } else {
            ''
        }
        return $signature.Status -eq 'Valid' -and
            $null -ne $signature.SignerCertificate -and
            $subject -match '(?i)(?:^|,\s*)CN=X\.AI LLC(?:,|$)' -and
            $subject -match '(?i)(?:^|,\s*)O=X\.AI LLC(?:,|$)'
    } catch {
        return $false
    }
}

function Get-OfficialNpmPackage {
    $npm = Get-Command npm.cmd -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($null -eq $npm) {
        return $null
    }
    try {
        $rootOutput = @(& $npm.Path root --global 2>$null)
        if ($LASTEXITCODE -ne 0 -or $rootOutput.Count -ne 1) {
            return $null
        }
        $root = ([string]$rootOutput[0]).Trim()
        if (![IO.Path]::IsPathRooted($root)) {
            return $null
        }
        $root = Resolve-FullPath $root
        if ((Split-Path -Leaf $root) -ine 'node_modules') {
            return $null
        }
        $packageRoot = Join-Path $root '@xai-official\grok'
        $manifestPath = Join-Path $packageRoot 'package.json'
        if (!(Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
            return $null
        }
        Assert-NoReparsePointInPath -Path $manifestPath -Label '官方 npm 包'
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if ($null -eq $manifest.PSObject.Properties['name'] -or
            $manifest.name -cne '@xai-official/grok') {
            return $null
        }
        return [pscustomobject]@{
            NpmPath = $npm.Path
            Prefix = Split-Path -Parent $root
            PackageRoot = $packageRoot
        }
    } catch {
        Write-Warning "无法确认官方 npm 包，未将其列入自动卸载范围：$($_.Exception.Message)"
        return $null
    }
}

function Get-OfficialInstallation {
    param(
        [Parameter(Mandatory = $true)][string]$OfficialBin,
        [Parameter(Mandatory = $true)][string]$CommunityInstallDir,
        [switch]$IncludeGlobalCommands
    )

    $directories = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    [void]$directories.Add($OfficialBin)
    if ($IncludeGlobalCommands.IsPresent) {
        if (![string]::IsNullOrWhiteSpace($env:GROK_BIN_DIR)) {
            $customBin = [Environment]::ExpandEnvironmentVariables($env:GROK_BIN_DIR)
            if ([IO.Path]::IsPathRooted($customBin)) {
                [void]$directories.Add((Resolve-FullPath $customBin))
            }
        }
        foreach ($name in @('grok.exe', 'agent.exe')) {
            foreach ($command in @(Get-Command $name -All -CommandType Application -ErrorAction SilentlyContinue)) {
                [void]$directories.Add((Split-Path -Parent $command.Path))
            }
        }
    }
    $files = [Collections.Generic.List[object]]::new()
    foreach ($directory in $directories) {
        if ((Test-PathsOverlap $directory $CommunityInstallDir) -or
            !(Test-Path -LiteralPath $directory -PathType Container)) {
            continue
        }
        Assert-NoReparsePointInPath -Path $directory -Label '官方程序目录'
        # Match official executable names only. In particular, neither grok-zh
        # nor the community grok.cmd/agent.cmd shims are official installations.
        foreach ($file in @(Get-ChildItem -LiteralPath $directory -File)) {
            if ($file.Name -notmatch '^(?:grok|agent)(?:-\d+\.\d+\.\d+(?:[.-][0-9A-Za-z-]+)*)?\.exe$') {
                continue
            }
            if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or
                !(Test-XaiSignedExecutable $file.FullName)) {
                continue
            }
            $files.Add([pscustomobject]@{
                Path = $file.FullName
                Sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
            })
        }
    }
    $npmPackage = if ($IncludeGlobalCommands.IsPresent) { Get-OfficialNpmPackage } else { $null }
    return [pscustomobject]@{
        Files = $files.ToArray()
        NpmPackage = $npmPackage
        Installed = $files.Count -gt 0 -or $null -ne $npmPackage
    }
}

function Read-InstallerInput {
    param(
        [Parameter(Mandatory = $true)][string]$Prompt,
        [AllowNull()][Collections.Generic.Queue[string]]$ScriptedAnswers
    )

    if ($null -ne $ScriptedAnswers) {
        Write-Host -NoNewline "$Prompt`: "
        if ($ScriptedAnswers.Count -eq 0) {
            Write-Host ''
            return $null
        }
        $value = $ScriptedAnswers.Dequeue()
        Write-Host $value
    } elseif ([Console]::IsInputRedirected) {
        Write-Host -NoNewline "$Prompt`: "
        $value = [Console]::In.ReadLine()
        Write-Host ''
    } else {
        $value = Read-Host $Prompt
    }

    if ($null -eq $value) {
        return $null
    }
    return $value.Trim()
}

function Read-InteractiveCommandSetup {
    param(
        [Parameter(Mandatory = $true)]$OfficialInstallation,
        [AllowNull()][Collections.Generic.Queue[string]]$ScriptedAnswers
    )

    if (!$OfficialInstallation.Installed) {
        Write-Host '未检测到已验证的官方版本，将直接安装中文版并接管 grok、agent 命令。' -ForegroundColor Cyan
        return [pscustomobject]@{
            Cancelled = $false
            OverrideOfficial = $true
            RemoveOfficial = $false
        }
    }
    Write-Host ''
    Write-Host '=== 可选：替换原始启动方式 ===' -ForegroundColor Cyan
    Write-Host '此步骤会安装或升级中文版，并让 grok、agent 优先启动中文版。'
    Write-Host '无论选择哪种方案，都不会删除 ~/.grok 中的聊天记录、登录信息、配置、插件或 MCP 数据。'
    Write-Host ''
    Write-Host '[1] 保留官方版，只接管 grok、agent 命令'
    Write-Host '[2] 卸载官方版本 grok.exe，并接管 grok、agent 命令'
    Write-Host '[3] 取消'

    while ($true) {
        $choice = Read-InstallerInput '请输入 1、2 或 3' -ScriptedAnswers $ScriptedAnswers
        if ($null -eq $choice) {
            Write-Warning '没有收到输入，已安全取消可选命令设置。'
            return [pscustomobject]@{
                Cancelled = $true
                OverrideOfficial = $false
                RemoveOfficial = $false
            }
        }
        switch ($choice) {
            '1' {
                return [pscustomobject]@{
                    Cancelled = $false
                    OverrideOfficial = $true
                    RemoveOfficial = $false
                }
            }
            '2' {
                Write-Host ''
                Write-Host '将卸载以下官方程序，不创建备份：' -ForegroundColor Yellow
                foreach ($file in $OfficialInstallation.Files) {
                    Write-Host "  $($file.Path)"
                }
                if ($null -ne $OfficialInstallation.NpmPackage) {
                    Write-Host '  npm 全局包 @xai-official/grok'
                }
                Write-Host '聊天记录、登录信息、配置、插件和 MCP 数据会保留。'
                $confirm = Read-InstallerInput `
                    '如确认继续，请再次输入 2；输入其他内容返回菜单' `
                    -ScriptedAnswers $ScriptedAnswers
                if ($null -eq $confirm) {
                    Write-Warning '没有收到二次确认，已安全取消可选命令设置。'
                    return [pscustomobject]@{
                        Cancelled = $true
                        OverrideOfficial = $false
                        RemoveOfficial = $false
                    }
                }
                if ($confirm -ne '2') {
                    Write-Host '未执行卸载操作。'
                    continue
                }
                return [pscustomobject]@{
                    Cancelled = $false
                    OverrideOfficial = $true
                    RemoveOfficial = $true
                }
            }
            '3' {
                return [pscustomobject]@{
                    Cancelled = $true
                    OverrideOfficial = $false
                    RemoveOfficial = $false
                }
            }
            default {
                Write-Host '输入无效，请输入 1、2 或 3。' -ForegroundColor Yellow
            }
        }
    }
}

function Read-AndVerifyManifest {
    param([Parameter(Mandatory = $true)][string]$Root)

    Assert-NoReparsePointTree -Path $Root -Label '安装包'
    $manifestPath = Join-Path $Root 'SHA256SUMS.txt'
    if (!(Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "安装包缺少校验清单：$manifestPath"
    }
    $manifestItem = Get-Item -LiteralPath $manifestPath -Force
    if (($manifestItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "安装包校验清单不能是符号链接或重解析点：$manifestPath"
    }

    $hashes = [Collections.Generic.Dictionary[string, string]]::new(
        [StringComparer]::Ordinal
    )
    $normalizedNames = [Collections.Generic.HashSet[string]]::new(
        [StringComparer]::Ordinal
    )
    $legacyRequired = @(
        'grok-zh.exe',
        'agent-zh.cmd',
        'rg.exe',
        '一键安装.cmd',
        '[可选]替换原始启动方式.cmd',
        'Install-GrokZh.ps1',
        'INSTALL-WINDOWS.md'
    )
    $completeRequired = @(
        $legacyRequired
        'LICENSE-grok-build.txt',
        'BUILD-INFO.txt',
        'licenses/ripgrep/COPYING',
        'licenses/ripgrep/LICENSE-MIT',
        'licenses/ripgrep/UNLICENSE',
        'licenses/project/THIRD-PARTY-NOTICES',
        'licenses/project/THIRD_PARTY_NOTICES.md',
        'licenses/project/NOTICE'
    )
    $allowedUnicodeNames = @('一键安装.cmd', '[可选]替换原始启动方式.cmd')
    foreach ($line in Get-Content -LiteralPath $manifestPath -Encoding UTF8) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            throw 'SHA256SUMS.txt 不能包含空行。'
        }
        if ($line -notmatch '^([0-9A-Fa-f]{64})  (.+)$') {
            throw "SHA256SUMS.txt 行格式无效：$line"
        }
        $expected = $matches[1].ToUpperInvariant()
        $name = $matches[2]
        $pathParts = $name.Split('/')
        if ($name.Trim() -ne $name -or
            $name.StartsWith('/') -or
            $name.Contains('\') -or
            $name.Contains(':') -or
            $pathParts.Count -eq 0 -or
            @($pathParts | Where-Object { $_ -in @('', '.', '..') }).Count -ne 0) {
            throw "安装包校验清单包含不安全路径：$name"
        }
        if ($name -ieq 'SHA256SUMS.txt') {
            throw 'SHA256SUMS.txt 不能包含自身哈希条目。'
        }
        if ($completeRequired -cnotcontains $name) {
            throw "安装包校验清单包含未批准的文件：$name"
        }
        $isAscii = $name -cmatch '^[\x00-\x7F]+$'
        if (!$isAscii -and $allowedUnicodeNames -cnotcontains $name) {
            throw "安装包校验清单包含未批准的 Unicode 文件名：$name"
        }
        $normalizedName = if ($isAscii) {
            $name.ToLowerInvariant()
        } else {
            $name
        }
        if (!$normalizedNames.Add($normalizedName)) {
            throw "安装包校验清单包含重复条目：$name"
        }
        $hashes.Add($name, $expected)
    }

    # A manually extracted package has no trustworthy GitHub Tag metadata, so
    # this installer selects the compatibility profile by the exact manifest
    # shape. The Release workflow and Rust updater separately bind that profile
    # to legacy versions and reject it for release-v* updates.
    $legacyManifest = $hashes.Count -eq $legacyRequired.Count -and
        @($legacyRequired | Where-Object { !$hashes.ContainsKey($_) }).Count -eq 0
    $completeManifest = $hashes.Count -eq $completeRequired.Count -and
        @($completeRequired | Where-Object { !$hashes.ContainsKey($_) }).Count -eq 0
    if (!$legacyManifest -and !$completeManifest) {
        throw '安装包校验清单不是受支持的 7 项桥接格式或 15 项完整格式。'
    }

    $totalBytes = [long]0
    foreach ($name in $hashes.Keys) {
        $source = Join-Path $Root $name
        if (!(Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "安装包缺少文件：$source"
        }
        $sourceItem = Get-Item -LiteralPath $source -Force
        if (($sourceItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "安装包文件不能是符号链接或重解析点：$source"
        }
        $totalBytes += $sourceItem.Length
    }

    $verifiedBytes = [long]0
    foreach ($name in $hashes.Keys) {
        $source = Join-Path $Root $name
        $length = (Get-Item -LiteralPath $source).Length
        $actual = Get-FileSha256 -Path $source -DisplayName $name `
            -BytesBefore $verifiedBytes -TotalBytes $totalBytes `
            -ProgressEnabled:$ShowProgress.IsPresent
        if ($actual -ne $hashes[$name]) {
            throw "$name 的 SHA-256 不匹配。预期 $($hashes[$name])，实际 $actual。"
        }
        $verifiedBytes += $length
    }
    if ($ShowProgress.IsPresent) {
        Write-Progress -Id 1 -Activity '正在校验安装包完整性' -Completed
    }

    return $hashes
}

function Write-CommandShim {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][bool]$AgentMode
    )

    $content = if ($AgentMode) {
        @'
@echo off
"%~dp0grok-zh.exe" agent %*
exit /b %ERRORLEVEL%
'@
    } else {
        @'
@echo off
"%~dp0grok-zh.exe" %*
exit /b %ERRORLEVEL%
'@
    }
    Set-Content -LiteralPath $Path -Value $content.Trim() -Encoding Ascii
}

function Get-NormalizedPathEntry {
    param([string]$Entry)

    if ([string]::IsNullOrWhiteSpace($Entry)) {
        return $null
    }
    $trimmed = $Entry.Trim().Trim('"')
    $expanded = [Environment]::ExpandEnvironmentVariables($trimmed)
    try {
        return (Resolve-FullPath $expanded)
    } catch {
        return $expanded.TrimEnd('\', '/')
    }
}

function Add-UserPathEntry {
    param([Parameter(Mandatory = $true)][string]$Directory)

    $normalizedDirectory = Get-NormalizedPathEntry $Directory
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $oldProcessPath = $env:Path
    $kept = [Collections.Generic.List[string]]::new()
    $userEntries = if ($null -eq $userPath) { @() } else { @($userPath.Split([char]';')) }
    foreach ($entry in $userEntries) {
        if ([string]::IsNullOrWhiteSpace($entry)) {
            $kept.Add($entry)
            continue
        }
        $normalized = Get-NormalizedPathEntry $entry
        if (![StringComparer]::OrdinalIgnoreCase.Equals($normalized, $normalizedDirectory)) {
            $kept.Add($entry.Trim())
        }
    }
    $newUserEntries = @($Directory) + @($kept)

    $processKept = [Collections.Generic.List[string]]::new()
    $processEntries = if ($null -eq $env:Path) { @() } else { @($env:Path.Split([char]';')) }
    foreach ($entry in $processEntries) {
        if ([string]::IsNullOrWhiteSpace($entry)) {
            $processKept.Add($entry)
            continue
        }
        $normalized = Get-NormalizedPathEntry $entry
        if (![StringComparer]::OrdinalIgnoreCase.Equals($normalized, $normalizedDirectory)) {
            $processKept.Add($entry.Trim())
        }
    }

    try {
        [Environment]::SetEnvironmentVariable('Path', ($newUserEntries -join ';'), 'User')
        $env:Path = (@($Directory) + @($processKept)) -join ';'
    } catch {
        try {
            [Environment]::SetEnvironmentVariable('Path', $userPath, 'User')
            $env:Path = $oldProcessPath
        } catch {
            Write-Warning "恢复原用户 Path 失败：$($_.Exception.Message)"
        }
        throw
    }
}

function Assert-OfficialInstallationRemovable {
    param(
        [Parameter(Mandatory = $true)]$OfficialInstallation
    )

    # Validate every planned file before removing any of them. Do not let the
    # non-interactive switch bypass the same identity checks used by the menu.
    foreach ($file in $OfficialInstallation.Files) {
        Assert-NoReparsePointInPath -Path $file.Path -Label '待卸载官方程序'
        if (!(Test-Path -LiteralPath $file.Path -PathType Leaf) -or
            !(Test-XaiSignedExecutable $file.Path) -or
            (Get-FileHash -LiteralPath $file.Path -Algorithm SHA256).Hash -cne $file.Sha256) {
            throw "待卸载的官方程序已发生变化，操作已停止：$($file.Path)"
        }
        try {
            $stream = [IO.File]::Open($file.Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
            $stream.Dispose()
        } catch {
            throw "官方程序正在使用中或无法访问，请关闭后重试：$($file.Path)。$($_.Exception.Message)"
        }
    }
    $npmPackage = $OfficialInstallation.NpmPackage
    if ($null -ne $npmPackage) {
        $manifestPath = Join-Path $npmPackage.PackageRoot 'package.json'
        Assert-NoReparsePointInPath -Path $manifestPath -Label '待卸载官方 npm 包'
        $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
        if ($null -eq $manifest.PSObject.Properties['name'] -or
            $manifest.name -cne '@xai-official/grok') {
            throw '官方 npm 包身份已发生变化，卸载已停止。'
        }
    }
}

function Remove-OfficialInstallation {
    param([Parameter(Mandatory = $true)]$OfficialInstallation)

    Assert-OfficialInstallationRemovable -OfficialInstallation $OfficialInstallation
    $npmPackage = $OfficialInstallation.NpmPackage
    if ($null -ne $npmPackage) {
        $manifestPath = Join-Path $npmPackage.PackageRoot 'package.json'
        & $npmPackage.NpmPath uninstall --global --prefix $npmPackage.Prefix `
            --ignore-scripts --no-audit --no-fund '@xai-official/grok' | Out-Host
        if ($LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath $manifestPath)) {
            throw '官方 npm 包未能卸载，已停止后续删除；请检查 npm 输出后重试。'
        }
        Write-Host '已卸载 npm 全局包 @xai-official/grok。'
    }
    $removed = [Collections.Generic.List[string]]::new()
    foreach ($file in $OfficialInstallation.Files) {
        if (!(Test-Path -LiteralPath $file.Path -PathType Leaf)) {
            continue
        }
        Assert-NoReparsePointInPath -Path $file.Path -Label '待卸载官方程序'
        if (!(Test-XaiSignedExecutable $file.Path) -or
            (Get-FileHash -LiteralPath $file.Path -Algorithm SHA256).Hash -cne $file.Sha256) {
            throw "待卸载的官方程序已发生变化，未删除该文件：$($file.Path)"
        }
        Remove-Item -LiteralPath $file.Path
        $removed.Add($file.Path)
        Write-Host "已卸载官方程序：$($file.Path)"
    }
    return $removed.ToArray()
}

if ([string]::IsNullOrWhiteSpace($PackageDir)) {
    $PackageDir = $PSScriptRoot
}
$PackageDir = Resolve-FullPath $PackageDir
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Get-DefaultInstallDir
}
$InstallDir = Resolve-FullPath $InstallDir
$useDefaultGrokHome = [string]::IsNullOrWhiteSpace($GrokHome)
if ($useDefaultGrokHome) {
    $GrokHome = Get-DefaultGrokHome
}
if ($GrokHome -match '(?i)\$env:') {
    throw 'GrokHome/GROK_HOME 中不能包含未展开的 $env: 变量；请先在 PowerShell 中展开后再传入。'
}
$GrokHome = [Environment]::ExpandEnvironmentVariables($GrokHome)
if ($GrokHome -match '%[^%]+%') {
    throw 'GrokHome/GROK_HOME 中包含无法展开的环境变量。'
}
if (![IO.Path]::IsPathRooted($GrokHome)) {
    throw 'GrokHome/GROK_HOME 必须是绝对路径，且不得包含未展开的环境变量。'
}
$GrokHome = Resolve-FullPath $GrokHome
$officialBin = Resolve-FullPath (Join-Path $GrokHome 'bin')
Assert-NoReparsePointInPath -Path $PackageDir -Label 'PackageDir'
Assert-NoReparsePointInPath -Path $InstallDir -Label 'InstallDir'
Assert-NoReparsePointInPath -Path $GrokHome -Label 'GrokHome'
Assert-NoReparsePointInPath -Path $officialBin -Label 'GrokHome\bin'
$requestedOverrideOfficialCommands = $OverrideOfficialCommands.IsPresent
$requestedUninstallOfficial = $UninstallOfficial.IsPresent
$officialInstallation = $null
if ($InteractiveCommandSetup.IsPresent -or $requestedUninstallOfficial) {
    $officialInstallation = Get-OfficialInstallation -OfficialBin $officialBin `
        -CommunityInstallDir $InstallDir -IncludeGlobalCommands:$useDefaultGrokHome
}
if ($InteractiveCommandSetup.IsPresent) {
    $scriptedAnswers = $null
    if ($PSBoundParameters.ContainsKey('ScriptedCommandSetupAnswers')) {
        $scriptedAnswers = [Collections.Generic.Queue[string]]::new()
        if (![string]::IsNullOrEmpty($ScriptedCommandSetupAnswers)) {
            foreach ($answer in $ScriptedCommandSetupAnswers.Split(';')) {
                $scriptedAnswers.Enqueue($answer)
            }
        }
    }
    $interactiveChoice = Read-InteractiveCommandSetup `
        -OfficialInstallation $officialInstallation `
        -ScriptedAnswers $scriptedAnswers
    if ($interactiveChoice.Cancelled) {
        Write-Host '已取消，没有修改程序、Path 或共享用户数据。'
        return
    }
    $requestedOverrideOfficialCommands = $interactiveChoice.OverrideOfficial
    $requestedUninstallOfficial = $interactiveChoice.RemoveOfficial
}
$provideOfficialNames = $requestedOverrideOfficialCommands -or $requestedUninstallOfficial

if (Test-PathsOverlap $PackageDir $InstallDir) {
    throw 'PackageDir 与 InstallDir 不能相同，也不能互相包含。'
}
if (Test-PathsOverlap $InstallDir $GrokHome) {
    throw 'InstallDir 不能与共享的 GROK_HOME 数据目录重叠；请使用独立的默认程序目录。'
}

Write-Host ''
Write-Host '[1/4] 正在校验安装包完整性...' -ForegroundColor Cyan
$manifest = Read-AndVerifyManifest $PackageDir
$installMarker = Join-Path $InstallDir '.grok-zh-install.json'
if ((Test-Path -LiteralPath $InstallDir) -and
    !(Test-Path -LiteralPath $installMarker -PathType Leaf) -and
    !$Force.IsPresent) {
    throw "InstallDir 已存在，但不归本安装器管理：$InstallDir。请先检查目录内容，确认安全后再使用 -Force。"
}

$operationParts = [Collections.Generic.List[string]]::new()
$operationParts.Add('安装 grok-zh 和 agent-zh')
if ($provideOfficialNames) {
    $operationParts.Add('提供 grok 和 agent 兼容命令')
}
if (!$NoPathUpdate.IsPresent) {
    $operationParts.Add('将安装目录置于用户 Path 首位')
}
if ($requestedUninstallOfficial) {
    $operationParts.Add('卸载已验证的官方程序，不创建备份')
}
$operation = ($operationParts -join ', ')
if (!$PSCmdlet.ShouldProcess($InstallDir, $operation)) {
    return
}

$parent = Split-Path -Parent $InstallDir
New-Item -ItemType Directory -Path $parent -Force | Out-Null
$token = [Guid]::NewGuid().ToString('N').Substring(0, 8)
$stage = "$InstallDir.stage.$PID-$token"
$previous = $null
$removedOfficial = @()

try {
    Write-Host '[2/4] 正在复制并安装程序文件...' -ForegroundColor Cyan
    New-Item -ItemType Directory -Path $stage | Out-Null
    $packageOnlyNames = @(
        '一键安装.cmd',
        '[可选]替换原始启动方式.cmd',
        'Install-GrokZh.ps1',
        'INSTALL-WINDOWS.md'
    )
    $installNames = @($manifest.Keys | Where-Object { $packageOnlyNames -notcontains $_ })
    $copyTotalBytes = [long]0
    foreach ($name in $installNames) {
        $copyTotalBytes += (Get-Item -LiteralPath (Join-Path $PackageDir $name)).Length
    }
    $copiedBytes = [long]0
    foreach ($name in $installNames) {
        $source = Join-Path $PackageDir $name
        $destination = Join-Path $stage $name
        $destinationParent = Split-Path -Parent $destination
        if (!(Test-Path -LiteralPath $destinationParent -PathType Container)) {
            New-Item -ItemType Directory -Path $destinationParent -Force | Out-Null
        }
        $length = (Get-Item -LiteralPath $source).Length
        $copiedHash = Copy-PackageFile -Source $source -Destination $destination `
            -DisplayName $name -BytesBefore $copiedBytes -TotalBytes $copyTotalBytes `
            -ProgressEnabled:$ShowProgress.IsPresent
        if ($copiedHash -ne $manifest[$name]) {
            throw "$name 在复制过程中发生变化，安装已停止。预期 $($manifest[$name])，实际 $copiedHash。"
        }
        $copiedBytes += $length
    }
    if ($ShowProgress.IsPresent) {
        Write-Progress -Id 2 -Activity '正在复制程序文件' -Completed
    }
    $installedManifestLines = foreach ($name in $installNames | Sort-Object) {
        "$($manifest[$name])  $name"
    }
    [IO.File]::WriteAllLines(
        (Join-Path $stage 'SHA256SUMS.txt'),
        [string[]]$installedManifestLines,
        (New-Object Text.UTF8Encoding($false))
    )
    if ($manifest.Count -eq 7) {
        foreach ($name in @('BUILD-INFO.txt', 'LICENSE-grok-build.txt')) {
            $source = Join-Path $PackageDir $name
            if (!(Test-Path -LiteralPath $source -PathType Leaf)) {
                throw "7 项桥接安装包缺少随包文件：$source"
            }
            $sourceItem = Get-Item -LiteralPath $source -Force
            if (($sourceItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "桥接安装包随包文件不能是符号链接或重解析点：$source"
            }
            Copy-Item -LiteralPath $source -Destination (Join-Path $stage $name)
        }
        $licenseSource = Join-Path $PackageDir 'licenses'
        if (!(Test-Path -LiteralPath $licenseSource -PathType Container)) {
            throw "7 项桥接安装包缺少许可证目录：$licenseSource"
        }
        Assert-NoReparsePointTree -Path $licenseSource -Label '桥接安装包许可证目录'
        Copy-Item -LiteralPath $licenseSource -Destination (Join-Path $stage 'licenses') -Recurse
    }
    if ($provideOfficialNames) {
        Write-CommandShim -Path (Join-Path $stage 'grok.cmd') -AgentMode:$false
        Write-CommandShim -Path (Join-Path $stage 'agent.cmd') -AgentMode:$true
    }

    $existingOfficialBackup = Join-Path $InstallDir 'official-backup'
    if (Test-Path -LiteralPath $existingOfficialBackup -PathType Container) {
        Assert-NoReparsePointTree -Path $existingOfficialBackup -Label '现有官方命令备份目录'
        Copy-Item -LiteralPath $existingOfficialBackup `
            -Destination (Join-Path $stage 'official-backup') -Recurse
    }

    $version = 'unknown'
    $buildInfo = Join-Path $PackageDir 'BUILD-INFO.txt'
    if (Test-Path -LiteralPath $buildInfo -PathType Leaf) {
        $versionLine = Get-Content -LiteralPath $buildInfo | Where-Object { $_ -match '^Version:\s*(.+)$' } | Select-Object -First 1
        if ($versionLine -and $versionLine -match '^Version:\s*(.+)$') {
            $version = $matches[1].Trim()
        }
    }

    if ($requestedUninstallOfficial) {
        Assert-OfficialInstallationRemovable -OfficialInstallation $officialInstallation
    }

    if (Test-Path -LiteralPath $InstallDir) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $previous = "$InstallDir.previous.$stamp-$token"
        Move-Item -LiteralPath $InstallDir -Destination $previous
    }

    try {
        [ordered]@{
            product = 'grok-build-zh'
            version = $version
            installed_at = (Get-Date).ToString('o')
            install_dir = $InstallDir
            commands = if ($provideOfficialNames) {
                @('grok-zh', 'agent-zh', 'grok', 'agent')
            } else {
                @('grok-zh', 'agent-zh')
            }
            previous_install_backup = $previous
            official_command_home = $GrokHome
        } | ConvertTo-Json -Depth 4 | Set-Content `
            -LiteralPath (Join-Path $stage '.grok-zh-install.json') -Encoding UTF8

        Move-Item -LiteralPath $stage -Destination $InstallDir
    } catch {
        if ($previous -and !(Test-Path -LiteralPath $InstallDir) -and (Test-Path -LiteralPath $previous)) {
            Move-Item -LiteralPath $previous -Destination $InstallDir
        }
        throw
    }
} finally {
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}

Write-Host '[3/4] 正在更新当前用户的命令搜索路径...' -ForegroundColor Cyan
if (!$NoPathUpdate.IsPresent) {
    Add-UserPathEntry $InstallDir
}

Write-Host '[4/4] 正在完成启动命令配置...' -ForegroundColor Cyan
if ($requestedUninstallOfficial) {
    try {
        # Only remove official programs after the replacement and its Path are
        # ready. Keep the working community install if removal later fails.
        $removedOfficial = @(Remove-OfficialInstallation -OfficialInstallation $officialInstallation)
    } catch {
        throw "中文版已安装到 $InstallDir，但官方版卸载未完成。请关闭官方程序并检查错误后，重新运行可选入口重试。$($_.Exception.Message)"
    }
}

Write-Host ''
Write-Host "安装完成：$InstallDir"
if ($provideOfficialNames) {
    Write-Host '主启动命令：grok、agent'
    Write-Host '兼容命令：grok-zh、agent-zh'
    Write-Host '已启用命令接管：grok 和 agent 现在使用本安装目录中的可恢复兼容脚本。'
    Write-Host '当前安装进程中的命令解析结果：'
    foreach ($commandName in @('grok', 'agent')) {
        $resolved = Get-Command $commandName -All -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($null -eq $resolved) {
            Write-Warning "  $commandName -> 未找到。请确认安装目录已加入用户 Path。"
            continue
        }
        $isApplication = $resolved -is [System.Management.Automation.ApplicationInfo]
        $resolvedPath = if ($isApplication) {
            $resolved.Path
        } else {
            $resolved.Definition
        }
        Write-Host "  $commandName -> $resolvedPath"
        $installPrefix = "$InstallDir$([IO.Path]::DirectorySeparatorChar)"
        $isCommunityCommand = $isApplication -and
            ![string]::IsNullOrWhiteSpace($resolvedPath) -and
            [IO.Path]::IsPathRooted($resolvedPath) -and
            $resolvedPath.StartsWith($installPrefix, [StringComparison]::OrdinalIgnoreCase)
        if (!$isCommunityCommand) {
            Write-Warning "  $commandName 仍被其他程序、别名或 Machine Path 遮蔽；请继续使用 $commandName-zh，或检查 Get-Command $commandName -All。"
        }
    }
} else {
    Write-Host '默认命令：grok-zh、agent-zh'
}
if ($requestedUninstallOfficial) {
    Write-Host "官方命令处理结果：已删除 $($removedOfficial.Count) 个官方程序文件，未创建备份；共享数据目录 $GrokHome 未更改。"
}
if ($NoPathUpdate.IsPresent) {
    Write-Host '未修改用户 Path（-NoPathUpdate）。'
} else {
    Write-Host '安装目录已置于用户 Path 首位。请重新打开其他终端窗口。'
}
Write-Host ''
Write-Host '接下来怎么启动：' -ForegroundColor Green
Write-Host '  1. 完全关闭已有的 PowerShell / Windows Terminal 窗口。'
if ($provideOfficialNames) {
    Write-Host '  2. 打开一个新终端，输入 grok 启动中文版。'
    Write-Host '  3. 也可以输入 agent 启动代理模式。'
    Write-Host '  4. grok-zh 和 agent-zh 兼容命令仍可使用。'
} else {
    Write-Host '  2. 打开一个新终端，输入 grok-zh 启动中文版。'
    Write-Host '  3. 也可以输入 agent-zh 启动代理模式。'
    Write-Host '可选方案：如希望直接输入 grok 或 agent，请双击解压包中的 [可选]替换原始启动方式.cmd。' -ForegroundColor Yellow
}
if ($ShowProgress.IsPresent) {
    Write-Host '通过双击入口运行时，脚本结束后会等待按键再关闭窗口，安装结果不会一闪而过。'
}
if ($provideOfficialNames) {
    Write-Host '验证命令：grok --version; agent --help'
} else {
    Write-Host '验证命令：grok-zh --version; agent-zh --help'
}
