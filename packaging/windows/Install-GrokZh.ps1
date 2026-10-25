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
额外创建 grok 和 agent 兼容命令，但不移动已有官方可执行文件。重新安装时默认保留先前选择；显式传入 -OverrideOfficialCommands:$false 可停用兼容命令。

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

.PARAMETER AllowDowngrade
明确允许安装包版本低于现有受管版本。请先自行备份共享 GROK_HOME；旧版本的数据格式兼容性无法保证。

.PARAMETER LibraryOnly
仅加载离线验证函数，不执行安装、联网或环境修改。供经验证的内嵌迁移入口使用。

.PARAMETER ExtractOnly
只验证完整外包并解出内嵌 MSVC 完整包，返回包目录，不安装。

.PARAMETER ExtractionDirectory
ExtractOnly 使用的全新临时目录。

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
    [switch]$Force,
    [switch]$AllowDowngrade,
    [switch]$LibraryOnly,
    [switch]$ExtractOnly,
    [string]$ExtractionDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Remove-PreviousCommunityInstall {
    param([string]$Backup, [string]$Current, [int]$Depth = 0)
    if (!$Backup -or !(Test-Path -LiteralPath $Backup)) { return $null }
    if ($Depth -ge 32) { return $Backup }
    try {
        $resolved = Resolve-FullPath $Backup
        $currentPath = Resolve-FullPath $Current
        $parent = Split-Path -Parent $currentPath
        $leaf = Split-Path -Leaf $currentPath
        if ((Split-Path -Parent $resolved) -ine $parent -or
            (Split-Path -Leaf $resolved) -notmatch ('^' + [regex]::Escape($leaf) + '\.previous\.\d{8}-\d{6}-[a-f0-9]{8}$')) { return $Backup }
        Assert-NoReparsePointInPath -Path $resolved -Label '旧版本目录'
        Assert-NoReparsePointTree -Path $resolved -Label '旧版本目录'
        $markerPath = Join-Path $resolved '.grok-zh-install.json'
        $marker = Get-Content -LiteralPath $markerPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($marker.product -cne 'grok-build-zh' -or !(Test-SamePath $marker.install_dir $currentPath)) { return $Backup }
        # A marker alone does not authorize removing personal files. Only the
        # old manifest and known generated launch/update files are disposable.
        $known = @('.grok-zh-install.json', '.grok-zh-update-result.json', 'grok-zh.exe.update.lock', 'SHA256SUMS.txt', 'grok.cmd', 'agent.cmd', 'BUILD-INFO.txt', 'LICENSE-grok-build.txt',
            'licenses\ripgrep\COPYING', 'licenses\ripgrep\LICENSE-MIT', 'licenses\ripgrep\UNLICENSE',
            'licenses\project\THIRD-PARTY-NOTICES', 'licenses\project\THIRD_PARTY_NOTICES.md', 'licenses\project\NOTICE')
        foreach ($line in Get-Content -LiteralPath (Join-Path $resolved 'SHA256SUMS.txt') -Encoding UTF8) {
            if ($line -match '^[a-fA-F0-9]{64}  (.+)$') { $known += $matches[1].Replace('/', '\') }
        }
        # Match only the updater's exact top-level backup/download/candidate
        # names; unrelated archives and files in nested directories stay protected.
        foreach ($file in Get-ChildItem -LiteralPath $resolved -File -Recurse -Force) {
            $relative = $file.FullName.Substring($resolved.Length + 1)
            if ($relative.StartsWith('official-backup\', [StringComparison]::OrdinalIgnoreCase)) {
                $preserved = Join-Path $currentPath $relative
                if (!(Test-Path -LiteralPath $preserved -PathType Leaf) -or
                    (Get-FileHash -LiteralPath $preserved).Hash -cne (Get-FileHash -LiteralPath $file.FullName).Hash) { return $Backup }
            } elseif ($relative -notin $known -and
                $relative -cnotmatch '^grok-zh\.exe\.(?:old(?:\.[0-9]+-[0-9]+\.old)?|[0-9]+-[0-9]+\.(?:download\.zip|candidate\.exe))$') { return $Backup }
            # Avoid partially deleting a backup still held by a running process.
            $handle = [IO.File]::Open($file.FullName, 'Open', 'Read', 'None')
            $handle.Dispose()
        }
        $previousProperty = $marker.PSObject.Properties['previous_install_backup']
        $pendingOlder = if ($previousProperty -and $previousProperty.Value) {
            Remove-PreviousCommunityInstall -Backup $previousProperty.Value -Current $currentPath -Depth ($Depth + 1)
        } else { $null }
        # Resolved sibling, strict name, ownership, full tree and preserved
        # official backup have all been checked before this recursive deletion.
        Remove-Item -LiteralPath $resolved -Recurse -Force
        return $pendingOlder
    } catch { return $Backup }
}

function Open-GrokInstallerLock {
    param([Parameter(Mandatory = $true)][string]$InstallDir)

    # A stable sibling lock survives both directory renames. Open the lock
    # itself (not a reparse target), pin its parent, and never unlink the file:
    # deleting an unlocked name could split contenders across different files.
    if (!('GrokZh.InstallDirectoryLock' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using Microsoft.Win32.SafeHandles;
namespace GrokZh {
    public sealed class InstallDirectoryLock : IDisposable {
        [StructLayout(LayoutKind.Sequential)]
        private struct FileInformation {
            public uint Attributes;
            public System.Runtime.InteropServices.ComTypes.FILETIME CreationTime;
            public System.Runtime.InteropServices.ComTypes.FILETIME AccessTime;
            public System.Runtime.InteropServices.ComTypes.FILETIME WriteTime;
            public uint VolumeSerial, SizeHigh, SizeLow, Links, IndexHigh, IndexLow;
        }
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern SafeFileHandle CreateFile(string name, uint access, uint share,
            IntPtr security, uint creation, uint flags, IntPtr template);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool GetFileInformationByHandle(SafeFileHandle file, out FileInformation info);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern uint GetFinalPathNameByHandle(SafeFileHandle file, StringBuilder path, uint size, uint flags);
        private SafeFileHandle parent;
        private SafeFileHandle file;
        private InstallDirectoryLock(SafeFileHandle parentHandle, SafeFileHandle fileHandle) {
            parent = parentHandle; file = fileHandle;
        }
        private static string FinalPath(SafeFileHandle handle) {
            StringBuilder path = new StringBuilder(32768);
            uint length = GetFinalPathNameByHandle(handle, path, (uint)path.Capacity, 0);
            if (length == 0 || length >= path.Capacity) throw new Win32Exception(Marshal.GetLastWin32Error());
            string result = path.ToString();
            if (result.StartsWith(@"\\?\UNC\", StringComparison.OrdinalIgnoreCase)) result = @"\\" + result.Substring(8);
            else if (result.StartsWith(@"\\?\", StringComparison.Ordinal)) result = result.Substring(4);
            return result.TrimEnd('\\', '/');
        }
        private static void CheckHandle(SafeFileHandle handle, bool directory, string expected) {
            if (handle.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error());
            FileInformation info;
            if (!GetFileInformationByHandle(handle, out info)) throw new Win32Exception(Marshal.GetLastWin32Error());
            if ((info.Attributes & 0x400) != 0 || ((info.Attributes & 0x10) != 0) != directory || (!directory && info.Links != 1))
                throw new IOException("Installer lock path must not be a reparse point, hard link or unexpected file type.");
            if (!String.Equals(FinalPath(handle), expected, StringComparison.OrdinalIgnoreCase))
                throw new IOException("Installer lock requires a canonical path with no redirected or short-name components: " + expected);
        }
        public static InstallDirectoryLock Acquire(string installDirectory) {
            string target = Path.GetFullPath(installDirectory).TrimEnd('\\', '/');
            string leaf = Path.GetFileName(target);
            if (String.IsNullOrWhiteSpace(leaf) || leaf.EndsWith(".", StringComparison.Ordinal) || leaf.EndsWith(" ", StringComparison.Ordinal) ||
                leaf.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 ||
                System.Text.RegularExpressions.Regex.IsMatch(leaf, @"^(CON|PRN|AUX|NUL|COM[1-9\u00B9\u00B2\u00B3]|LPT[1-9\u00B9\u00B2\u00B3])(?:\..*)?$", System.Text.RegularExpressions.RegexOptions.IgnoreCase))
                throw new IOException("Installer target must use an unambiguous ordinary directory name.");
            string parentPath = Path.GetDirectoryName(target);
            SafeFileHandle parentHandle = null, fileHandle = null;
            try {
                // FILE_READ_ATTRIBUTES; share read/write but not delete; open
                // the directory itself with BACKUP_SEMANTICS|OPEN_REPARSE_POINT.
                parentHandle = CreateFile(parentPath, 0x80, 3, IntPtr.Zero, 3, 0x02200000, IntPtr.Zero);
                CheckHandle(parentHandle, true, parentPath.TrimEnd('\\', '/'));
                string lockPath = Path.Combine(parentPath, Path.GetFileName(target) + ".install.lock");
                // OPEN_ALWAYS + no sharing is an immediate exclusive lock.
                fileHandle = CreateFile(lockPath, 0xC0000000, 0, IntPtr.Zero, 4, 0x00200080, IntPtr.Zero);
                CheckHandle(fileHandle, false, lockPath);
                // Existing target aliases must not manufacture another lock
                // name for the same installation. No target handle survives
                // this validation, so the subsequent rename stays possible.
                if (Directory.Exists(target)) {
                    using (SafeFileHandle targetHandle = CreateFile(target, 0x80, 7, IntPtr.Zero, 3, 0x02200000, IntPtr.Zero)) {
                        CheckHandle(targetHandle, true, target);
                    }
                }
                InstallDirectoryLock result = new InstallDirectoryLock(parentHandle, fileHandle);
                parentHandle = null; fileHandle = null;
                return result;
            } finally {
                if (fileHandle != null) fileHandle.Dispose();
                if (parentHandle != null) parentHandle.Dispose();
            }
        }
        public void Dispose() {
            if (file != null) { file.Dispose(); file = null; }
            if (parent != null) { parent.Dispose(); parent = null; }
        }
    }
}
'@
    }
    Assert-NoReparsePointInPath -Path $InstallDir -Label '安装锁目标'
    try {
        return [GrokZh.InstallDirectoryLock]::Acquire($InstallDir)
    } catch {
        throw "无法独占安装目录（可能有另一安装器正在运行，或路径不安全）：$InstallDir。未创建暂存目录或移动现有安装。$($_.Exception.Message)"
    }
}

function Assert-NoRunningCommunityInstall {
    param([Parameter(Mandatory = $true)][string]$InstallDir)

    $installPath = Resolve-FullPath $InstallDir
    $blocked = [Collections.Generic.List[string]]::new()
    try {
        # Read the running image path, never the version of the on-disk EXE:
        # an old process may still be running after that EXE was replaced.
        $processes = @(Get-Process -ErrorAction Stop | Where-Object { $_.ProcessName -like 'grok-zh*' })
    } catch {
        throw "无法检查正在运行的 grok-zh 进程，未切换安装目录。请退出旧会话后重试。$($_.Exception.Message)"
    }
    foreach ($process in $processes) {
        try {
            $imagePath = $null
            try { $imagePath = $process.Path } catch { }
            if ([string]::IsNullOrWhiteSpace($imagePath)) {
                # A process exiting during enumeration is harmless. Any live
                # candidate whose image cannot be read must fail closed.
                $exited = $false
                try { $exited = $process.HasExited } catch { }
                if (!$exited) { $blocked.Add("PID $($process.Id)（无法读取运行映像路径）") }
                continue
            }
            try {
                $imagePath = Resolve-FullPath $imagePath
                $relatedBackup = $imagePath.StartsWith($installPath + '.previous.', [StringComparison]::OrdinalIgnoreCase) -and
                    $imagePath.Substring($installPath.Length) -match '^\.previous\.\d{8}-\d{6}-[a-f0-9]{8}[\\/]'
                # A process in an exact historical sibling may have cached the
                # original install destination before its directory was moved.
                if ($relatedBackup -or (Test-PathsOverlap $installPath $imagePath)) {
                    $blocked.Add("PID $($process.Id)（$imagePath）")
                }
            } catch {
                $blocked.Add("PID $($process.Id)（无法确认运行映像路径）")
            }
        } finally {
            if ($process -is [Diagnostics.Process]) { $process.Dispose() }
        }
    }
    if ($blocked.Count -gt 0) {
        throw "安装目录仍有运行中的 grok-zh，或存在无法确认路径的相关进程：$($blocked -join '；')。请自行退出这些会话后重试，并且安装期间不要重新启动旧版本。安装器不会强制结束进程。"
    }
    # This late preflight reduces legacy-writer risk, but cannot prevent a new
    # process starting after the scan. Keep old sessions closed during install.
}

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
        [StringComparer]::OrdinalIgnoreCase
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
    $nativeProtocol = $null
    $buildInfoPath = Join-Path $Root 'BUILD-INFO.txt'
    if (Test-Path -LiteralPath $buildInfoPath -PathType Leaf) {
        if ((Get-Item -LiteralPath $buildInfoPath).Length -gt 65536) { throw '包内元数据超过大小限制。' }
        $candidateProtocol = Read-GrokBundleProtocol (Get-Content -LiteralPath $buildInfoPath -Raw -Encoding UTF8) '' ''
        if ($candidateProtocol -and $candidateProtocol.platform -in @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')) {
            if ($candidateProtocol.executable -cne 'grok-zh.exe' -or $candidateProtocol.installer -cne 'Install-GrokZh.ps1' -or
                $null -ne $candidateProtocol.PSObject.Properties['embedded_runtime']) { throw '原生完整包入口或内嵌描述无效。' }
            $nativeProtocol = $candidateProtocol
        }
    }
    if ((Get-Item -LiteralPath $manifestPath).Length -gt 65536) { throw '包内校验清单超过大小限制。' }
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
        Assert-GrokBundleRelativePath $name
        if (!$nativeProtocol -and $completeRequired -cnotcontains $name) {
            throw "安装包校验清单包含未批准的文件：$name"
        }
        $isAscii = $name -cmatch '^[\x00-\x7F]+$'
        if (!$nativeProtocol -and !$isAscii -and $allowedUnicodeNames -cnotcontains $name) {
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
    $completeManifest = (($hashes.Count -eq $completeRequired.Count) -or ($nativeProtocol -and $hashes.Count -ge $completeRequired.Count -and $hashes.Count -le 127)) -and
        @($completeRequired | Where-Object { !$hashes.ContainsKey($_) }).Count -eq 0
    if (!$legacyManifest -and !$completeManifest) {
        throw '安装包校验清单不是受支持的桥接、15 项 GNU 或原生完整格式。'
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

# Canonical offline embedded-runtime validator, also embedded in the GNU launcher.
# No downloader, network fallback, environment update or user-data access lives here.
function Assert-GrokBundleJsonKeys {
    param([string]$Json)
    $stack = [Collections.Generic.Stack[object]]::new()
    $tokens = [regex]::Matches($Json, '"(?:\\.|[^"\\])*"|[{}\[\]:,]|true|false|null|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?')
    foreach ($match in $tokens) {
        $token = $match.Value
        if ($token -eq '{' -or $token -eq '[') {
            $stack.Push([pscustomobject]@{ Object = $token -eq '{'; Key = $token -eq '{'; Names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase) })
        } elseif ($token -eq '}' -or $token -eq ']') {
            if ($stack.Count -eq 0) { throw '更新协议 JSON 结构无效。' }
            $null = $stack.Pop()
        } elseif ($stack.Count -gt 0) {
            $frame = $stack.Peek()
            if ($token -eq ',') { $frame.Key = $frame.Object }
            elseif ($token -eq ':') { $frame.Key = $false }
            elseif ($frame.Object -and $frame.Key -and $token.StartsWith('"')) {
                $name = (ConvertFrom-Json ('{"value":' + $token + '}')).value
                if (!$frame.Names.Add($name)) { throw "更新协议字段重复：$name" }
                $frame.Key = $false
            }
        }
    }
}

function Read-GrokBundleProtocol {
    param([string]$Text, [string]$Version, [string]$Platform)
    $begin = 'GROK-UPDATE-PROTOCOL-BEGIN'; $end = 'GROK-UPDATE-PROTOCOL-END'
    if (!$Text.Contains($begin) -and !$Text.Contains($end)) { return $null }
    $lines = @($Text -split '\r?\n')
    $starts = @(for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -ceq $begin) { $i } })
    $ends = @(for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i] -ceq $end) { $i } })
    if ($starts.Count -ne 1 -or $ends.Count -ne 1 -or $starts[0] + 1 -ge $ends[0]) { throw '包内更新协议块不完整或重复。' }
    $json = $lines[($starts[0] + 1)..($ends[0] - 1)] -join "`n"
    Assert-GrokBundleJsonKeys $json
    $protocol = ConvertFrom-Json -InputObject $json
    foreach ($field in @('schema', 'version', 'platform', 'mode', 'manifest', 'executable', 'installer')) {
        $value = Get-GrokBundleProperty $protocol $field
        if ($field -eq 'schema') {
            if (($value -isnot [int] -and $value -isnot [long]) -or $value -ne 1) { throw '内嵌包只支持 schema 1。' }
        } elseif ($value -isnot [string]) { throw "更新协议字段类型无效：$field" }
    }
    if ($protocol.mode -cne 'executable-only' -or $protocol.manifest -cne 'SHA256SUMS.txt') { throw '内嵌包更新协议无效。' }
    if ($Version -and $protocol.version -cne $Version) { throw '内嵌包版本与所选版本不一致。' }
    if ($Platform -and $protocol.platform -cne $Platform) { throw '内嵌包平台不一致。' }
    if ($protocol.version -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$') { throw '包内版本格式无效。' }
    foreach ($part in (($protocol.version -split '[-+]')[0]).Split('.')) { $null = [uint64]::Parse($part, [Globalization.CultureInfo]::InvariantCulture) }
    $withoutBuild = $protocol.version.Split('+')[0]
    if ($withoutBuild.Contains('-')) {
        foreach ($identifier in $withoutBuild.Substring($withoutBuild.IndexOf('-') + 1).Split('.')) {
            if ($identifier -cmatch '^0[0-9]+$') { throw '版本预发布数字标识符不能有前导零。' }
        }
    }
    Assert-GrokBundleRelativePath $protocol.executable
    Assert-GrokBundleRelativePath $protocol.installer
    if ($protocol.executable -in @('BUILD-INFO.txt', 'SHA256SUMS.txt') -or
        $protocol.installer -in @('BUILD-INFO.txt', 'SHA256SUMS.txt') -or
        $protocol.executable -ieq $protocol.installer) { throw '更新协议入口无效或重叠。' }
    return $protocol
}

function Get-GrokEmbeddedDescriptor {
    param($Protocol)
    $property = $Protocol.PSObject.Properties['embedded_runtime']
    if ($null -eq $property) { return $null }
    $descriptor = $property.Value
    $fields = @('schema', 'format', 'version', 'platform', 'archive_size', 'archive_sha256')
    if ($null -eq $descriptor -or @($descriptor.PSObject.Properties.Name).Count -ne $fields.Count) { throw '内嵌运行时描述字段无效。' }
    foreach ($field in $fields) { $null = Get-GrokBundleProperty $descriptor $field }
    if (($descriptor.schema -isnot [int] -and $descriptor.schema -isnot [long]) -or $descriptor.schema -ne 1 -or
        $descriptor.format -isnot [string] -or $descriptor.format -cne 'grok-msvc-bundle-v1' -or
        $descriptor.version -isnot [string] -or $descriptor.version -cne $Protocol.version -or
        $descriptor.platform -isnot [string] -or $descriptor.platform -cne 'x86_64-pc-windows-msvc' -or
        ($descriptor.archive_size -isnot [int] -and $descriptor.archive_size -isnot [long]) -or
        $descriptor.archive_size -le 0 -or $descriptor.archive_size -gt 536870912 -or
        $descriptor.archive_sha256 -isnot [string] -or $descriptor.archive_sha256 -cnotmatch '^[0-9a-fA-F]{64}$' -or
        $Protocol.platform -cne 'x86_64-pc-windows-gnu' -or $Protocol.executable -cne 'grok-zh.exe' -or
        $Protocol.installer -cne 'Install-GrokZh.ps1') { throw '内嵌运行时描述与外包版本、平台或摘要不一致。' }
    return $descriptor
}

function Assert-GrokFreeSpace {
    param([Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][long]$RequiredBytes)
    if ($RequiredBytes -lt 0) { throw '空间需求不能为负数。' }
    $root = [IO.Path]::GetPathRoot((Resolve-FullPath $Path))
    $drive = [IO.DriveInfo]::new($root)
    if (!$drive.IsReady -or $drive.AvailableFreeSpace -lt $RequiredBytes) { throw "磁盘剩余空间不足，至少需要 $RequiredBytes 字节：$root" }
}

function Assert-GrokPe64Prefix {
    param([Parameter(Mandatory = $true)][string]$Path, [long]$PrefixLength = -1)
    Assert-NoReparsePointInPath -Path $Path -Label 'PE 程序'
    $stream = [IO.File]::Open($Path, 'Open', 'Read', 'Read')
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($PrefixLength -lt 0) { $PrefixLength = $stream.Length }
        if ($PrefixLength -lt 256 -or $PrefixLength -gt $stream.Length -or $reader.ReadUInt16() -ne 0x5a4d) { throw '程序不是完整 x64 PE 文件。' }
        $null = $stream.Seek(0x3c, [IO.SeekOrigin]::Begin)
        $peOffset = [uint64]$reader.ReadUInt32()
        if ($peOffset -lt 64 -or $peOffset -gt [uint64]($PrefixLength - 26)) { throw 'PE 头超出程序前缀边界。' }
        $null = $stream.Seek([long]$peOffset, [IO.SeekOrigin]::Begin)
        if ($reader.ReadUInt32() -ne 0x00004550 -or $reader.ReadUInt16() -ne 0x8664) { throw '程序必须是 x64 PE。' }
        $sections = $reader.ReadUInt16()
        $null = $stream.Seek([long]$peOffset + 20, [IO.SeekOrigin]::Begin)
        $optionalSize = $reader.ReadUInt16(); $null = $reader.ReadUInt16()
        if ($sections -lt 1 -or $optionalSize -lt 112 -or $peOffset + 24 + $optionalSize + 40L * $sections -gt [uint64]$PrefixLength -or
            $reader.ReadUInt16() -ne 0x20b) { throw 'PE32+ 可选头或区段表不完整。' }
    } finally { $reader.Dispose(); $stream.Dispose() }
}

function Expand-GrokEmbeddedArchive {
    param([Parameter(Mandatory = $true)][string]$Launcher,
        [Parameter(Mandatory = $true)][string]$Destination,
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][long]$ExpectedSize,
        [Parameter(Mandatory = $true)][string]$ExpectedSha256)
    if ($ExpectedSize -le 0 -or $ExpectedSize -gt 536870912 -or $ExpectedSha256 -cnotmatch '^[0-9a-fA-F]{64}$' -or
        $Version -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') { throw '内嵌运行时固定版本、大小或摘要无效。' }
    foreach ($part in $Version.Split('.')) { $null = [uint64]::Parse($part, [Globalization.CultureInfo]::InvariantCulture) }
    $Launcher = Resolve-FullPath $Launcher
    $Destination = Resolve-FullPath $Destination
    Assert-NoReparsePointInPath -Path $Launcher -Label '内嵌启动器'
    Assert-NoReparsePointInPath -Path $Destination -Label '内嵌临时目录'
    if (Test-Path -LiteralPath $Destination) { throw '内嵌运行时目标必须是新的临时目录。' }
    $input = $null; $reader = $null; $output = $null; $sha = $null; $created = $false
    try {
        # Hold the authenticated source against writes/replacement while extracting.
        $input = [IO.File]::Open($Launcher, 'Open', 'Read', 'Read')
        if ($input.Length -lt 66 -or $input.Length -gt 536870912) { throw '内嵌启动器大小无效或缺少完整尾部。' }
        if ($input.ReadByte() -ne 0x4d -or $input.ReadByte() -ne 0x5a) { throw '内嵌启动器不是 Windows PE 文件。' }
        $null = $input.Seek(-64, [IO.SeekOrigin]::End)
        $reader = [IO.BinaryReader]::new($input, [Text.Encoding]::ASCII, $true)
        $magic = [Text.Encoding]::ASCII.GetString($reader.ReadBytes(16))
        $offset = $reader.ReadUInt64(); $size = $reader.ReadUInt64()
        $digest = ([BitConverter]::ToString($reader.ReadBytes(32))).Replace('-', '').ToLowerInvariant()
        # Subtraction and range checks avoid UInt64 overflow on hostile footer values.
        if ($magic -cne 'GROKMSVCBUNDLE01' -or $size -ne $ExpectedSize -or $size -gt 536870912 -or
            $offset -lt 256 -or $offset -gt [uint64]($input.Length - 64) -or
            $size -ne [uint64]($input.Length - 64) - $offset -or $digest -ine $ExpectedSha256) { throw '内嵌运行时尾部边界或摘要与固定信息不一致。' }
        Assert-GrokPe64Prefix -Path $Launcher -PrefixLength ([long]$offset)
        Assert-GrokFreeSpace -Path $Destination -RequiredBytes ($ExpectedSize + 16777216L)
        $null = New-Item -ItemType Directory -Path $Destination -ErrorAction Stop -WhatIf:$false -Confirm:$false
        $created = $true
        $archive = Join-Path $Destination 'native-msvc.zip'
        $output = [IO.File]::Open($archive, 'CreateNew', 'Write', 'None')
        $sha = [Security.Cryptography.SHA256]::Create()
        $null = $input.Seek([long]$offset, [IO.SeekOrigin]::Begin)
        $remaining = [long]$size; $buffer = [byte[]]::new(65536)
        while ($remaining -gt 0) {
            $count = $input.Read($buffer, 0, [int][Math]::Min($remaining, $buffer.Length))
            if ($count -le 0) { throw '内嵌运行时未完整读取。' }
            $output.Write($buffer, 0, $count)
            $null = $sha.TransformBlock($buffer, 0, $count, $buffer, 0)
            $remaining -= $count
        }
        $null = $sha.TransformFinalBlock([byte[]]::new(0), 0, 0)
        $actual = ([BitConverter]::ToString($sha.Hash)).Replace('-', '').ToLowerInvariant()
        if ($actual -ine $ExpectedSha256) { throw '内嵌运行时 SHA-256 不匹配。' }
        $output.Dispose(); $output = $null
        $contract = [pscustomobject]@{ Legacy = $false; Version = [pscustomobject]@{ Text = $Version }; PackageRoot = "grok-zh-$Version-windows-x86_64-msvc" }
        return Expand-VerifiedGrokBundlePackage -ArchivePath $archive -Destination (Join-Path $Destination 'package') -Contract $contract
    } catch {
        if ($output) { $output.Dispose(); $output = $null }
        if ($created -and (Test-Path -LiteralPath $Destination)) {
            Assert-NoReparsePointTree -Path $Destination -Label '内嵌临时目录'
            Remove-Item -LiteralPath $Destination -Recurse -Force -WhatIf:$false -Confirm:$false
        }
        throw
    } finally {
        if ($output) { $output.Dispose() }
        if ($reader) { $reader.Dispose() }
        if ($input) { $input.Dispose() }
        if ($sha) { $sha.Dispose() }
    }
}

function Expand-GrokEmbeddedPackage {
    param([Parameter(Mandatory = $true)][string]$Package,
        [Parameter(Mandatory = $true)][string]$Destination)
    $manifest = Read-AndVerifyManifest $Package
    if ($manifest.Count -ne 15) { throw '内嵌运行时外包必须使用完整清单。' }
    $infoPath = Join-Path $Package 'BUILD-INFO.txt'; $launcherPath = Join-Path $Package 'grok-zh.exe'
    $infoHandle = $null; $launcherHandle = $null
    try {
        $infoHandle = [IO.File]::Open($infoPath, 'Open', 'Read', 'Read')
        $launcherHandle = [IO.File]::Open($launcherPath, 'Open', 'Read', 'Read')
        if ((Get-FileHash -LiteralPath $infoPath -Algorithm SHA256).Hash -ine $manifest['BUILD-INFO.txt'] -or
            (Get-FileHash -LiteralPath $launcherPath -Algorithm SHA256).Hash -ine $manifest['grok-zh.exe']) { throw '外包在完整校验后发生变化。' }
        $protocol = Read-GrokBundleProtocol (Get-Content -LiteralPath $infoPath -Raw -Encoding UTF8) '' 'x86_64-pc-windows-gnu'
        if (!$protocol) { throw '安装包没有内嵌运行时协议。' }
        $descriptor = Get-GrokEmbeddedDescriptor $protocol
        if (!$descriptor) { throw '安装包没有内嵌运行时描述。' }
        return Expand-GrokEmbeddedArchive -Launcher $launcherPath -Destination $Destination `
            -Version $protocol.version -ExpectedSize $descriptor.archive_size -ExpectedSha256 $descriptor.archive_sha256
    } finally {
        if ($launcherHandle) { $launcherHandle.Dispose() }
        if ($infoHandle) { $infoHandle.Dispose() }
    }
}

function Get-GrokPackageVersion {
    param([string]$Root)
    $path = Join-Path $Root 'BUILD-INFO.txt'
    if (!(Test-Path -LiteralPath $path -PathType Leaf)) { return $null }
    $protocol = Read-GrokBundleProtocol ([IO.File]::ReadAllText($path)) '' ''
    $matches = @([IO.File]::ReadAllLines($path) | Where-Object { $_ -cmatch '^Version:\s*(\S+)\s*$' })
    if ($protocol) {
        if ($matches.Count -gt 1 -or ($matches.Count -eq 1 -and ($matches[0] -replace '^Version:\s*', '').Trim() -cne $protocol.version)) { throw '版本说明与包内协议不一致。' }
        return $protocol.version
    }
    if ($matches.Count -ne 1) { return $null }
    $text = ($matches[0] -replace '^Version:\s*', '').Trim()
    if ($text -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$') { return $null }
    return $text
}

function Get-GrokExecutableVersion {
    param([string]$Executable)
    Assert-NoReparsePointInPath -Path $Executable -Label '版本检查程序'
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Executable; $info.Arguments = '--version'
    $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new(); $process.StartInfo = $info
    try {
        if (!$process.Start()) { throw '无法启动版本检查。' }
        $stdout = $process.StandardOutput.ReadToEndAsync(); $stderr = $process.StandardError.ReadToEndAsync()
        if (!$process.WaitForExit(10000)) { $process.Kill(); $process.WaitForExit(); throw '程序版本检查超时。' }
        $text = $stdout.GetAwaiter().GetResult().Trim(); $null = $stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0 -or $text.Length -gt 4096 -or
            $text -cnotmatch '^grok-zh ((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)(?: \([^()\r\n]{1,128}\))?(?: \[[^\[\]\r\n]{1,32}\])?$') { throw '程序没有返回有效的 grok-zh 版本。' }
        return $matches[1]
    } finally { $process.Dispose() }
}

function Compare-GrokInstallerVersion {
    param([string]$Left, [string]$Right)
    $leftBase = $Left.Split('+')[0]; $rightBase = $Right.Split('+')[0]
    $leftParts = $leftBase.Split('-')[0].Split('.'); $rightParts = $rightBase.Split('-')[0].Split('.')
    for ($i = 0; $i -lt 3; $i++) {
        if ([uint64]$leftParts[$i] -gt [uint64]$rightParts[$i]) { return 1 }
        if ([uint64]$leftParts[$i] -lt [uint64]$rightParts[$i]) { return -1 }
    }
    $leftPre = if ($leftBase.Contains('-')) { $leftBase.Substring($leftBase.IndexOf('-') + 1) } else { '' }
    $rightPre = if ($rightBase.Contains('-')) { $rightBase.Substring($rightBase.IndexOf('-') + 1) } else { '' }
    if ($leftPre -and !$rightPre) { return -1 }
    if (!$leftPre -and $rightPre) { return 1 }
    if (!$leftPre -and !$rightPre) { return 0 }
    $leftIds = $leftPre.Split('.'); $rightIds = $rightPre.Split('.')
    for ($i = 0; $i -lt [Math]::Min($leftIds.Length, $rightIds.Length); $i++) {
        $a = $leftIds[$i]; $b = $rightIds[$i]
        $aNumeric = $a -cmatch '^[0-9]+$'; $bNumeric = $b -cmatch '^[0-9]+$'
        if ($aNumeric -and !$bNumeric) { return -1 }
        if (!$aNumeric -and $bNumeric) { return 1 }
        if ($aNumeric -and $bNumeric -and $a.Length -ne $b.Length) {
            if ($a.Length -lt $b.Length) { return -1 } else { return 1 }
        }
        $comparison = [StringComparer]::Ordinal.Compare($a, $b)
        if ($comparison -lt 0) { return -1 }
        if ($comparison -gt 0) { return 1 }
    }
    if ($leftIds.Length -lt $rightIds.Length) { return -1 }
    if ($leftIds.Length -gt $rightIds.Length) { return 1 }
    return 0
}

function Get-GrokBundleProperty {
    param($Object, [string]$Name)
    if ($null -eq $Object -or $null -eq $Object.PSObject.Properties[$Name]) {
        throw "发布信息缺少字段：$Name"
    }
    return $Object.$Name
}


function ConvertFrom-GrokBundleUtf8 {
    param([byte[]]$Bytes)
    $text = [Text.UTF8Encoding]::new($false, $true).GetString($Bytes)
    if ($text.Length -gt 0 -and $text[0] -eq [char]0xfeff) { $text = $text.Substring(1) }
    return $text
}


function Assert-GrokBundleRelativePath {
    param([string]$Path)
    if (!$Path -or $Path.Trim() -cne $Path -or $Path -match '[\\:<>"|?*\x00-\x1f\x7f]') { throw "包内路径无效：$Path" }
    foreach ($part in $Path.Split('/')) {
        if (!$part -or $part -in @('.', '..') -or $part.EndsWith('.') -or $part.EndsWith(' ') -or
            $part -match '^(?i:CON|PRN|AUX|NUL|COM[1-9\u00b9\u00b2\u00b3]|LPT[1-9\u00b9\u00b2\u00b3])(?:\.|$)') { throw "包内路径无效：$Path" }
    }
}


function Read-GrokBundleZipText {
    param($Entry)
    if ($Entry.Length -gt 65536) { throw '包内元数据超过大小限制。' }
    $stream = $Entry.Open()
    $memory = [IO.MemoryStream]::new()
    try {
        $chunk = [byte[]]::new(4096)
        while (($read = $stream.Read($chunk, 0, $chunk.Length)) -gt 0) {
            if ($memory.Length + $read -gt $Entry.Length) { throw '包内元数据实际大小超过声明。' }
            $memory.Write($chunk, 0, $read)
        }
        if ($memory.Length -ne $Entry.Length) { throw '包内元数据未完整解压。' }
        return ConvertFrom-GrokBundleUtf8 ($memory.ToArray())
    } finally { $stream.Dispose(); $memory.Dispose() }
}


function Expand-VerifiedGrokBundlePackage {
    param([string]$ArchivePath, [string]$Destination, $Contract)
    if ($Contract.Legacy) { throw '内嵌包不支持旧版平铺格式。' }
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    if (Test-Path -LiteralPath $Destination) { throw '解压目标必须是新的临时目录。' }
    $archive = [IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        if ($archive.Entries.Count -eq 0 -or $archive.Entries.Count -gt 128) { throw 'ZIP 文件数量超出限制。' }
        $entries = [Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        $total = 0L
        $directories = @()
        foreach ($entry in $archive.Entries) {
            $name = $entry.FullName
            $directory = $name.EndsWith('/')
            $unixType = ([uint32]([long]$entry.ExternalAttributes -band 0xffffffffL) -shr 16) -band 0xf000
            if ($name.Contains('\') -or $name.Contains('//') -or $name.Contains(':') -or $name.StartsWith('/') -or
                ($unixType -ne 0 -and $unixType -ne 0x8000 -and $unixType -ne 0x4000) -or
                ($entry.ExternalAttributes -band 0x400) -ne 0 -or
                ($unixType -eq 0x4000 -and !$directory) -or ($unixType -eq 0x8000 -and $directory)) { throw "ZIP 包含不安全的成员：$name" }
            $parts = $name.TrimEnd('/').Split('/')
            if (@($parts | Where-Object { $_ -in @('', '.', '..') }).Count -gt 0) { throw "ZIP 路径无效：$name" }
            if (!$Contract.Legacy) {
                if ($name -ceq "$($Contract.PackageRoot)/") {
                    if (!$seen.Add($name) -or $entry.Length -ne 0) { throw 'ZIP 顶层目录重复或不为空。' }
                    continue
                }
                $prefix = "$($Contract.PackageRoot)/"
                if (!$name.StartsWith($prefix, [StringComparison]::Ordinal)) { throw "ZIP 必须只有指定顶层目录：$($Contract.PackageRoot)" }
                $name = $name.Substring($prefix.Length)
            }
            $logical = $name.TrimEnd('/')
            Assert-GrokBundleRelativePath $logical
            if (!$seen.Add($logical)) { throw "ZIP 包含重复路径：$logical" }
            if ($directory) {
                if ($entry.Length -ne 0) { throw "ZIP 目录包含文件内容：$logical" }
                $directories += $logical
            } else {
                if ($entry.Length -gt 536870912 -or $entry.Length -lt 0) { throw 'ZIP 单文件超过大小限制。' }
                $total += $entry.Length
                if ($total -gt 805306368) { throw 'ZIP 解压后总大小超过限制。' }
                $entries.Add($logical, $entry)
            }
        }
        foreach ($name in @('BUILD-INFO.txt', 'SHA256SUMS.txt')) {
            if (!$entries.ContainsKey($name)) { throw "ZIP 缺少文件：$name" }
        }
        $protocol = Read-GrokBundleProtocol (Read-GrokBundleZipText $entries['BUILD-INFO.txt']) $Contract.Version.Text 'x86_64-pc-windows-msvc'
        if (!$protocol -or $protocol.executable -cne 'grok-zh.exe' -or $protocol.installer -cne 'Install-GrokZh.ps1' -or
            $null -ne $protocol.PSObject.Properties['embedded_runtime']) { throw '内嵌包必须是原生 MSVC 完整包，不能再次包含兼容启动器。' }
        if ($entries['SHA256SUMS.txt'].Length -gt 65536) { throw '包内校验清单超过大小限制。' }
        $manifestStream = $entries['SHA256SUMS.txt'].Open()
        try {
            $memory = [IO.MemoryStream]::new()
            try {
                $chunk = [byte[]]::new(4096)
                while (($read = $manifestStream.Read($chunk, 0, $chunk.Length)) -gt 0) {
                    if ($memory.Length + $read -gt $entries['SHA256SUMS.txt'].Length) { throw '包内清单实际大小超过声明。' }
                    $memory.Write($chunk, 0, $read)
                }
                if ($memory.Length -ne $entries['SHA256SUMS.txt'].Length) { throw '包内清单未完整解压。' }
                $manifestText = ConvertFrom-GrokBundleUtf8 ($memory.ToArray())
            } finally { $memory.Dispose() }
        } finally { $manifestStream.Dispose() }
        $manifestNames = @()
        $hashes = [Collections.Generic.Dictionary[string, string]]::new([StringComparer]::Ordinal)
        if ($manifestText.EndsWith("`n")) {
            $manifestText = $manifestText.Substring(0, $manifestText.Length - 1)
            if ($manifestText.EndsWith("`r")) { $manifestText = $manifestText.Substring(0, $manifestText.Length - 1) }
        }
        $lines = $manifestText -split '\r?\n'
        foreach ($line in $lines) {
            if ($line -cnotmatch '^([0-9A-Fa-f]{64})  (.+)$') { throw '包内 SHA256SUMS.txt 格式无效。' }
            $expected = $matches[1]; $name = $matches[2]
            Assert-GrokBundleRelativePath $name
            if ((!$protocol -and $manifestNames -cnotcontains $name) -or $name -ieq 'SHA256SUMS.txt' -or $hashes.ContainsKey($name)) { throw "包内清单包含额外或重复文件：$name" }
            $hashes.Add($name, $expected)
        }
        if ($protocol) {
            foreach ($required in @('BUILD-INFO.txt', $protocol.executable, $protocol.installer)) {
                if (!$hashes.ContainsKey($required)) { throw "新协议清单缺少必要文件：$required" }
            }
            $packageFiles = @($hashes.Keys)
        } else {
            if ($hashes.Count -ne $manifestNames.Count) { throw '包内 SHA256SUMS.txt 未覆盖完整文件集合。' }
            $packageFiles = @()
        }
        $fileNames = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($name in $entries.Keys) { $null = $fileNames.Add($name) }
        $allowedDirs = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($name in $packageFiles) {
            if (!$entries.ContainsKey($name)) { throw "ZIP 缺少清单文件：$name" }
            $parent = $name
            while ($parent.Contains('/')) {
                $parent = $parent.Substring(0, $parent.LastIndexOf('/'))
                if ($fileNames.Contains($parent)) { throw "ZIP 文件和目录冲突：$parent" }
                $null = $allowedDirs.Add($parent)
            }
        }
        if ($entries.Count -ne $packageFiles.Count + 1) { throw 'ZIP 包含清单之外的文件。' }
        foreach ($directory in $directories) {
            if (!$allowedDirs.Contains($directory)) { throw "ZIP 包含额外目录：$directory" }
        }
        Assert-GrokFreeSpace -Path $Destination -RequiredBytes ($total + 16777216L)
        $null = [IO.Directory]::CreateDirectory($Destination)
        $package = if ($Contract.Legacy) { $Destination } else { Join-Path $Destination $Contract.PackageRoot }
        foreach ($name in $entries.Keys) {
            $target = Join-Path $package $name
            $null = [IO.Directory]::CreateDirectory((Split-Path -Parent $target))
            $inputStream = $entries[$name].Open()
            $output = [IO.File]::Open($target, [IO.FileMode]::CreateNew)
            try {
                $buffer = [byte[]]::new(65536); $written = 0L
                while (($count = $inputStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    $written += $count
                    if ($written -gt $entries[$name].Length) { throw "ZIP 文件实际大小与声明不符：$name" }
                    $output.Write($buffer, 0, $count)
                }
                if ($written -ne $entries[$name].Length) { throw "ZIP 文件尚未完整解压：$name" }
            } finally { $inputStream.Dispose(); $output.Dispose() }
            if ($hashes.ContainsKey($name) -and (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ine $hashes[$name]) {
                throw "包内文件 SHA-256 不匹配：$name"
            }
        }
        $null = Read-AndVerifyManifest $package
        Assert-GrokPe64Prefix -Path (Join-Path $package 'grok-zh.exe')
        return $package
    } finally { $archive.Dispose() }
}


# The compiler embeds this same complete script; library loading must stay above
# all path defaults, prompts, installation and environment changes.
if ($LibraryOnly) { return }
if ($ExtractOnly) {
    if (!$ExtractionDirectory) { throw '-ExtractOnly 需要 -ExtractionDirectory。' }
    if (!$PackageDir) { $PackageDir = $PSScriptRoot }
    Expand-GrokEmbeddedPackage -Package (Resolve-FullPath $PackageDir) -Destination $ExtractionDirectory
    return
}

if ([string]::IsNullOrWhiteSpace($PackageDir)) {
    $PackageDir = $PSScriptRoot
}
$PackageDir = Resolve-FullPath $PackageDir
$packageBuildInfo = Join-Path $PackageDir 'BUILD-INFO.txt'
if (Test-Path -LiteralPath $packageBuildInfo -PathType Leaf) {
    $packageProtocol = Read-GrokBundleProtocol (Get-Content -LiteralPath $packageBuildInfo -Raw -Encoding UTF8) '' ''
    if ($packageProtocol -and $null -ne $packageProtocol.PSObject.Properties['embedded_runtime']) {
        # Validate all outer hashes before reading authenticated embedded bytes.
        $null = Read-AndVerifyManifest $PackageDir
        $bundleHome = if ($GrokHome) { $GrokHome } else { Get-DefaultGrokHome }
        $bundleHome = Resolve-FullPath ([Environment]::ExpandEnvironmentVariables($bundleHome))
        $bundleWork = Join-Path ([IO.Path]::GetTempPath()) ('grok-zh-embedded-install-' + [guid]::NewGuid().ToString('N'))
        if (Test-PathsOverlap $bundleWork $bundleHome) { throw '内嵌临时目录不能与共享 GROK_HOME 重叠。' }
        $bundleCreated = $false
        try {
            $nativePackage = Expand-GrokEmbeddedPackage -Package $PackageDir -Destination $bundleWork
            $bundleCreated = $true
            $forward = @{}
            foreach ($key in $PSBoundParameters.Keys) {
                if ($key -notin @('PackageDir', 'LibraryOnly', 'ExtractOnly', 'ExtractionDirectory')) { $forward[$key] = $PSBoundParameters[$key] }
            }
            $forward.PackageDir = $nativePackage
            & (Join-Path $nativePackage 'Install-GrokZh.ps1') @forward
        } finally {
            if ($bundleCreated -and (Test-Path -LiteralPath $bundleWork)) {
                Assert-NoReparsePointTree -Path $bundleWork -Label '内嵌临时目录'
                Remove-Item -LiteralPath $bundleWork -Recurse -Force -WhatIf:$false -Confirm:$false
            }
        }
        return
    }
}
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

$packageVersion = Get-GrokPackageVersion $PackageDir
if (Test-Path -LiteralPath $installMarker -PathType Leaf) {
    $existingMarker = Get-Content -LiteralPath $installMarker -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($existingMarker.product -cne 'grok-build-zh' -or !(Test-SamePath $existingMarker.install_dir $InstallDir)) {
        throw '现有安装的归属记录与目标目录不一致。'
    }
    $existingVersion = if ($packageVersion) { Get-GrokExecutableVersion (Join-Path $InstallDir 'grok-zh.exe') } else { $null }
    if ($packageVersion -and $existingVersion -and (Compare-GrokInstallerVersion $existingVersion $packageVersion) -gt 0) {
        if (!$AllowDowngrade) { throw "当前版本 $existingVersion 高于安装包 $packageVersion；如需明确降级，请先备份共享数据，再加 -AllowDowngrade。" }
        Write-Warning "正在明确降级到 $packageVersion。请先自行备份共享 GROK_HOME：$GrokHome。旧版数据格式兼容性未知；安装器不会回滚或修改共享账号、配置和会话。"
    }
    # Preserve the user's command choice across reinstall, downgrade and return
    # to MSVC unless this invocation explicitly chooses command setup. Do not
    # carry over migration sidecars or readiness markers.
    if (!$PSBoundParameters.ContainsKey('OverrideOfficialCommands') -and
        !$InteractiveCommandSetup.IsPresent -and
        $null -ne $existingMarker.PSObject.Properties['commands'] -and
        @($existingMarker.commands | Where-Object { $_ -in @('grok', 'agent') }).Count -gt 0) { $provideOfficialNames = $true }
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
$installerLock = $null
try {
$installerLock = Open-GrokInstallerLock -InstallDir $InstallDir
# Planning above is read-only so -WhatIf never creates a lock. Revalidate
# mutable ownership, version and command choices after obtaining exclusivity:
# another installer may have finished between planning and lock acquisition.
Assert-NoReparsePointInPath -Path $InstallDir -Label '安装锁目标'
if ((Test-Path -LiteralPath $InstallDir) -and
    !(Test-Path -LiteralPath $installMarker -PathType Leaf) -and !$Force.IsPresent) {
    throw "InstallDir 已存在，但不归本安装器管理：$InstallDir。"
}
if (Test-Path -LiteralPath $installMarker -PathType Leaf) {
    $lockedMarker = Get-Content -LiteralPath $installMarker -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($lockedMarker.product -cne 'grok-build-zh' -or !(Test-SamePath $lockedMarker.install_dir $InstallDir)) {
        throw '现有安装的归属记录与目标目录不一致。'
    }
    if ($packageVersion) {
        $lockedVersion = Get-GrokExecutableVersion (Join-Path $InstallDir 'grok-zh.exe')
        if ((Compare-GrokInstallerVersion $lockedVersion $packageVersion) -gt 0 -and !$AllowDowngrade) {
            throw "当前版本 $lockedVersion 高于安装包 $packageVersion；请明确确认 -AllowDowngrade。"
        }
    }
    if (!$PSBoundParameters.ContainsKey('OverrideOfficialCommands') -and !$InteractiveCommandSetup.IsPresent) {
        $provideOfficialNames = $requestedUninstallOfficial -or
            ($null -ne $lockedMarker.PSObject.Properties['commands'] -and
             @($lockedMarker.commands | Where-Object { $_ -in @('grok', 'agent') }).Count -gt 0)
    }
}
$token = [Guid]::NewGuid().ToString('N').Substring(0, 8)
$stage = "$InstallDir.stage.$PID-$token"
$stageCreated = $false
$previous = $null
$activationStarted = $false
$installationCommitted = $false
$removedOfficial = @()

try {
    Write-Host '[2/4] 正在复制并安装程序文件...' -ForegroundColor Cyan
    New-Item -ItemType Directory -Path $stage | Out-Null
    $stageCreated = $true
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
    Assert-GrokFreeSpace -Path $stage -RequiredBytes ($copyTotalBytes + 16777216L)
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

    if ($packageVersion -and (Get-GrokExecutableVersion (Join-Path $stage 'grok-zh.exe')) -cne $packageVersion) {
        throw '暂存程序版本与完整包版本不一致，未修改现有安装。'
    }

    if ($requestedUninstallOfficial) {
        Assert-OfficialInstallationRemovable -OfficialInstallation $officialInstallation
    }

    if (Test-Path -LiteralPath $InstallDir) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $previous = "$InstallDir.previous.$stamp-$token"
    }
    # Finish all staged metadata before retiring the current entrypoint.
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

    Assert-NoRunningCommunityInstall -InstallDir $InstallDir
    if ($previous) {
        Move-Item -LiteralPath $InstallDir -Destination $previous
    }
    $activationStarted = $true
    Move-Item -LiteralPath $stage -Destination $InstallDir
    $stageCreated = $false
    if ($packageVersion -and (Get-GrokExecutableVersion (Join-Path $InstallDir 'grok-zh.exe')) -cne $packageVersion) {
        throw '安装后版本检查失败，正在恢复原安装。'
    }
    $installationCommitted = $true
} finally {
    # Ctrl+C / PowerShell.Stop bypasses catch, but runs finally. Reconcile the
    # actual directory state: cancellation can land after either rename and
    # before the following assignment. Process termination/power loss cannot
    # run finally and is not covered by this in-process recovery.
    $canRemoveStage = $true
    if (!$installationCommitted) {
        try {
            if ($activationStarted -and !(Test-Path -LiteralPath $stage) -and
                (Test-Path -LiteralPath $InstallDir)) {
                Move-Item -LiteralPath $InstallDir -Destination $stage
                $stageCreated = $true
            }
            if ($previous -and (Test-Path -LiteralPath $previous)) {
                if (Test-Path -LiteralPath $InstallDir) {
                    throw "恢复目标已存在，未覆盖该目录：$InstallDir"
                }
                Move-Item -LiteralPath $previous -Destination $InstallDir
            } elseif ($previous -and !(Test-Path -LiteralPath $InstallDir)) {
                throw "找不到原安装或其备份：$previous"
            }
        } catch {
            # A failed restore must not discard the staged replacement, which
            # may be the only usable recovery copy. Never remove the backup.
            $canRemoveStage = $false
            Write-Warning "安装未完成，自动恢复失败：$($_.Exception.Message) 原安装备份：$previous；暂存目录：$stage。已保留恢复文件，请勿删除。"
        }
    }
    if ($canRemoveStage -and $stageCreated -and (Test-Path -LiteralPath $stage)) {
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
if ($previous) {
    # The new files were hash-verified in staging and activated successfully.
    # A failed activation above restores the old directory before reaching here.
    $pendingBackup = Remove-PreviousCommunityInstall -Backup $previous -Current $InstallDir
    $installedMarker = Get-Content -LiteralPath $installMarker -Raw -Encoding UTF8 | ConvertFrom-Json
    $installedMarker.previous_install_backup = $pendingBackup
    $installedMarker | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $installMarker -Encoding UTF8
    if ($pendingBackup) { Write-Warning "旧版本目录暂未清理（文件占用或含需保留的内容），下次更新将重试：$pendingBackup" }
}
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

} finally {
    if ($null -ne $installerLock) { $installerLock.Dispose() }
}
