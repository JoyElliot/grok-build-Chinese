# Keep Cargo build scripts/proc macros native; isolate only target C/link tools.
$ErrorActionPreference = 'Stop'
if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() -cne 'Arm64' -or
    $env:TARGET -cne 'x86_64-pc-windows-msvc' -or
    $env:RUSTUP_TOOLCHAIN -cne '1.94.0-aarch64-pc-windows-msvc') {
    throw 'Expected ARM64 Windows/Rust host and x64 MSVC target.'
}
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsRoot = (& $vswhere -latest -products '*' -property installationPath | Select-Object -First 1).Trim()
if (!$vsRoot) { throw 'Missing Visual Studio.' }
$dev = Join-Path $vsRoot 'Common7\Tools\VsDevCmd.bat'
$root = Join-Path $env:RUNNER_TEMP 'grok-msvc-cross-tools'
New-Item -ItemType Directory -Path $root -Force | Out-Null
# Capture only compiler environment keys. Never persist runner credentials.
$keys = @('PATH', 'INCLUDE', 'LIB', 'LIBPATH', 'WindowsSdkDir', 'WindowsSDKVersion',
    'WindowsLibPath', 'UCRTVersion', 'UniversalCRTSdkDir', 'VCToolsInstallDir',
    'VCToolsVersion', 'VCINSTALLDIR', 'VSINSTALLDIR', 'VSCMD_ARG_TGT_ARCH',
    'VSCMD_ARG_HOST_ARCH', 'VSCMD_VER', 'ExtensionSdkDir')
function Get-CompilerEnvironment([string]$Arch) {
    $lines = & $env:ComSpec /d /s /c "call `"$dev`" -host_arch=arm64 -arch=$Arch >nul && set"
    if ($LASTEXITCODE -ne 0) { throw "VsDevCmd failed for $Arch." }
    $result = @{}
    foreach ($line in $lines) {
        if ($line -match '^([^=]+)=(.*)$' -and $matches[1] -in $keys) {
            $result[$matches[1]] = $matches[2]
        }
    }
    if ($result.VSCMD_ARG_TGT_ARCH -cne $Arch -or $result.VSCMD_ARG_HOST_ARCH -cne 'arm64' -or
        !$result.LIB -or !$result.INCLUDE -or !$result.VCToolsInstallDir) {
        throw "Incomplete MSVC environment for $Arch."
    }
    return $result
}
$hostEnv = Get-CompilerEnvironment 'arm64'
$targetEnv = Get-CompilerEnvironment 'x64'
$hostBin = Join-Path $hostEnv.VCToolsInstallDir 'bin\Hostarm64\arm64'
$targetBin = Join-Path $targetEnv.VCToolsInstallDir 'bin\Hostarm64\x64'
foreach ($bin in @($hostBin, $targetBin)) {
    foreach ($tool in @('cl.exe', 'link.exe', 'lib.exe')) {
        if (!(Test-Path -LiteralPath (Join-Path $bin $tool) -PathType Leaf)) {
            throw "Missing ARM64-hosted compiler component: $bin\$tool"
        }
    }
}
$utf8 = [Text.UTF8Encoding]::new($false)
foreach ($entry in $hostEnv.GetEnumerator()) {
    [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, 'Process')
    "$($entry.Key)=$($entry.Value)" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
}
# Native wrappers avoid cmd.exe's 8191-character archive limit. BLAKE3's
# cross-build detection additionally requires the bare CC name cl.exe.
& "$PSScriptRoot/build-msvc-tool-wrappers.ps1" -HostCompiler (Join-Path $hostBin 'cl.exe') `
    -TargetDirectory $targetBin -TargetEnvironment $targetEnv -OutputDirectory $root
$overrides = @{
    # Only cl.exe is exposed on PATH; host tools must not find target link/lib.
    PATH = "$(Join-Path $root 'compiler');$($hostEnv.PATH)"
    CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_LINKER = (Join-Path $hostBin 'link.exe')
    CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = (Join-Path $root 'link.exe')
    CC_aarch64_pc_windows_msvc = (Join-Path $hostBin 'cl.exe')
    CXX_aarch64_pc_windows_msvc = (Join-Path $hostBin 'cl.exe')
    AR_aarch64_pc_windows_msvc = (Join-Path $hostBin 'lib.exe')
    CC_x86_64_pc_windows_msvc = 'cl.exe'
    CXX_x86_64_pc_windows_msvc = 'cl.exe'
    AR_x86_64_pc_windows_msvc = (Join-Path $root 'lib.exe')
    # The locked cmake-rs predates VS18; keep its MSBuild target selection.
    CMAKE_GENERATOR_x86_64_pc_windows_msvc = 'Visual Studio 18 2026'
}
foreach ($entry in $overrides.GetEnumerator()) {
    [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, 'Process')
    "$($entry.Key)=$($entry.Value)" | Out-File $env:GITHUB_ENV -Append -Encoding utf8
}

# Fail before the full Cargo graph if either environment links the wrong CRT.
$cSource = Join-Path $root 'probe.c'
$rustSource = Join-Path $root 'probe.rs'
[IO.File]::WriteAllText($cSource, '#include <stdio.h>' + "`n" + 'int main(void) { puts("probe"); return 0; }', $utf8)
[IO.File]::WriteAllText($rustSource, 'fn main() { println!("probe"); }', $utf8)
function Assert-Machine([string]$File, [int]$Expected) {
    $bytes = [IO.File]::ReadAllBytes($File)
    $pe = [BitConverter]::ToInt32($bytes, 0x3c)
    if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne $Expected) { throw "Wrong probe architecture: $File" }
}
foreach ($arch in @('arm64', 'x64')) {
    $target = if ($arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
    $machine = if ($arch -eq 'arm64') { 0xaa64 } else { 0x8664 }
    $compiler = if ($arch -eq 'arm64') { Join-Path $hostBin 'cl.exe' } else { Join-Path $root 'compiler/cl.exe' }
    $linker = if ($arch -eq 'arm64') { Join-Path $hostBin 'link.exe' } else { Join-Path $root 'link.exe' }
    $cExe = Join-Path $root "$arch-c.exe"
    & $compiler /nologo /MT $cSource "/Fe:$cExe" "/Fo:$(Join-Path $root "$arch.obj")"
    if ($LASTEXITCODE -ne 0) { throw "C $arch link probe failed." }
    Assert-Machine $cExe $machine
    $rustExe = Join-Path $root "$arch-rust.exe"
    & rustc $rustSource --target $target -C "linker=$linker" -C target-feature=+crt-static -o $rustExe
    if ($LASTEXITCODE -ne 0) { throw "Rust $arch link probe failed." }
    Assert-Machine $rustExe $machine
    if ($arch -eq 'arm64') {
        & $rustExe
        if ($LASTEXITCODE -ne 0) { throw 'Native ARM64 host probe could not run.' }
    }
}
& "$PSScriptRoot/tests/Test-MsvcToolWrappers.ps1" -WrapperDirectory $root
# cmake-rs 0.1.54 selects this target/toolset with our explicit VS18 generator.
# Probe its MSBuild path too; unlike Rust, some VS tools may run under emulation.
$cmakeSource = Join-Path $root 'cmake-probe'
New-Item -ItemType Directory -Path $cmakeSource -Force | Out-Null
Copy-Item -LiteralPath $cSource -Destination (Join-Path $cmakeSource 'probe.c')
[IO.File]::WriteAllText((Join-Path $cmakeSource 'CMakeLists.txt'),
    "cmake_minimum_required(VERSION 3.21)`nproject(probe C)`nadd_executable(probe probe.c)`n", $utf8)
$cmakeBuild = Join-Path $root 'cmake-build'
& cmake -S $cmakeSource -B $cmakeBuild -G 'Visual Studio 18 2026' -A x64 -T host=x64
if ($LASTEXITCODE -ne 0) { throw 'CMake x64 configure probe failed.' }
& cmake --build $cmakeBuild --config Release
if ($LASTEXITCODE -ne 0) { throw 'CMake x64 build probe failed.' }
Assert-Machine (Join-Path $cmakeBuild 'Release\probe.exe') 0x8664
$report = Join-Path $env:RUNNER_TEMP 'grok-msvc-cross-report'
New-Item -ItemType Directory -Path $report -Force | Out-Null
[ordered]@{
    rust = @(& rustc -Vv); host_linker = (Join-Path $hostBin 'link.exe')
    target_linker = (Join-Path $targetBin 'link.exe'); msvc_version = $hostEnv.VCToolsVersion
    host_lib = $hostEnv.LIB; target_lib = $targetEnv.LIB
    cpu = @(Get-CimInstance Win32_Processor | Select-Object Name, NumberOfLogicalProcessors)
    image = $env:ImageVersion; host = 'aarch64-pc-windows-msvc'; target = $env:TARGET
} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $report 'toolchain-and-host.json') -Encoding utf8
$fingerprint = @($hostEnv.VCToolsVersion, $hostEnv.WindowsSDKVersion)
foreach ($bin in @($hostBin, $targetBin)) {
    foreach ($tool in @('cl.exe', 'link.exe', 'lib.exe')) {
        $fingerprint += (Get-FileHash -LiteralPath (Join-Path $bin $tool) -Algorithm SHA256).Hash
    }
}
$fingerprintPath = Join-Path $report 'compiler-fingerprint.txt'
[IO.File]::WriteAllLines($fingerprintPath, $fingerprint, $utf8)
"toolchain_key=$((Get-FileHash -LiteralPath $fingerprintPath -Algorithm SHA256).Hash.ToLowerInvariant())" |
    Out-File $env:GITHUB_OUTPUT -Append -Encoding utf8
Write-Host 'ARM64 host and x64 target Rust/C link probes passed; product execution remains gated on native x64.'
