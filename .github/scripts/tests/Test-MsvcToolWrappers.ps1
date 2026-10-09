param(
    [Parameter(Mandatory)][string]$WrapperDirectory,
    [switch]$CargoProbe,
    # The probe must follow the product lockfile actually being built (a PR merge ref
    # can lock newer cc/find-msvc-tools than this branch), so versions are never fixed here.
    [string]$LockFile = (Join-Path $PSScriptRoot '../../../Cargo.lock')
)
$ErrorActionPreference = 'Stop'

function Get-LockedRegistryVersion([string]$Lock, [string]$Name) {
    $pattern = '(?m)^name = "' + [regex]::Escape($Name) + '"\r?\nversion = "([^"]+)"\r?\nsource = "registry\+'
    $versions = @([regex]::Matches($Lock, $pattern) | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
    if ($versions.Count -ne 1) {
        throw "Cargo.lock must lock exactly one crates.io version of $Name for the MSVC probe; found $($versions.Count): $($versions -join ', ')."
    }
    return $versions[0]
}
$root = Join-Path $WrapperDirectory 'wrapper probes 中文'
New-Item -ItemType Directory -Path $root -Force | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
if ($CargoProbe) {
    if (!(Test-Path -LiteralPath $LockFile -PathType Leaf)) { throw "Product Cargo.lock not found for the MSVC probe: $LockFile" }
    $lock = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $LockFile).Path)
    $locked = [ordered]@{}
    foreach ($name in 'cc', 'find-msvc-tools', 'blake3') { $locked[$name] = Get-LockedRegistryVersion $lock $name }
    if ($env:CC_x86_64_pc_windows_msvc -cne 'cl.exe' -or
        (Get-Command cl.exe -CommandType Application | Select-Object -First 1).Source -ine (Resolve-Path -LiteralPath (Join-Path $WrapperDirectory 'compiler/cl.exe')).Path) {
        throw 'BLAKE3 cross builds require the bare cl.exe name resolving to the target wrapper.'
    }
    # Use the exact cc/find-msvc-tools/blake3 versions in the product lockfile, after its fetch.
    # This exercises compiler-family detection and optimized assembly selection.
    [IO.File]::WriteAllText((Join-Path $root 'Cargo.toml'), @"
[workspace]
[package]
name = "msvc-wrapper-probe"
version = "0.0.0"
edition = "2021"
[build-dependencies]
cc = "=$($locked['cc'])"
find-msvc-tools = "=$($locked['find-msvc-tools'])"
[dependencies]
blake3 = "=$($locked['blake3'])"
[[bin]]
name = "msvc-wrapper-probe"
path = "main.rs"
"@, $utf8)
    [IO.File]::WriteAllText((Join-Path $root 'build.rs'), @'
fn main() {
    assert!(cc::Build::new().get_compiler().is_like_msvc(), "wrapper must select MSVC, including MASM");
    cc::Build::new().file("probe.c").compile("cc_probe");
}
'@, $utf8)
    [IO.File]::WriteAllText((Join-Path $root 'probe.c'), 'int cc_probe(void) { return 42; }', $utf8)
    [IO.File]::WriteAllText((Join-Path $root 'main.rs'), 'fn main() { println!("{}", blake3::hash(b"probe")); }', $utf8)
    $manifest = Join-Path $root 'Cargo.toml'
    & cargo generate-lockfile --offline --manifest-path $manifest
    if ($LASTEXITCODE -ne 0) { throw 'cc/blake3 probe dependency resolution failed.' }
    & cargo build --frozen --manifest-path $manifest --target x86_64-pc-windows-msvc --target-dir (Join-Path $root 'cargo-target')
    if ($LASTEXITCODE -ne 0) { throw 'Actual cc/blake3 compiler-wrapper probe failed.' }
    Write-Host "Locked cc $($locked['cc']), find-msvc-tools $($locked['find-msvc-tools']) and blake3 $($locked['blake3']) compiled with the target MSVC wrappers."
    return
}
$compiler = Join-Path $WrapperDirectory 'compiler/cl.exe'
$archiver = Join-Path $WrapperDirectory 'lib.exe'
$linker = Join-Path $WrapperDirectory 'link.exe'
$source = Join-Path $root 'probe.c'
$object = Join-Path $root 'probe.obj'
$library = Join-Path $root 'probe.lib'
$exe = Join-Path $root 'probe.exe'
[IO.File]::WriteAllText($source, '#include <stdio.h>' + "`n" + 'int main(void) { puts("wrapper probe"); return 0; }', $utf8)
$savedInclude = $env:INCLUDE
$savedLib = $env:LIB
try {
    # The target wrapper must restore its own environment, not inherit host LIB.
    $env:INCLUDE = Join-Path $root 'invalid-host-include'
    $env:LIB = Join-Path $root 'invalid-host-lib'
    & $compiler /nologo /MT /c $source "/Fo:$object"
    if ($LASTEXITCODE -ne 0) { throw 'Wrapper compile/environment/Unicode argument probe failed.' }
    $archiveArgs = @('/nologo') * 1400 + @("/OUT:$library", $object)
    if (($archiveArgs -join ' ').Length -le 8191) { throw 'Archive probe must exceed cmd.exe limit.' }
    & $archiver @archiveArgs
    if ($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $library)) { throw 'Long native archive command failed.' }
    $response = Join-Path $root 'link args.rsp'
    [IO.File]::WriteAllLines($response, @('/nologo', '/MACHINE:X64', ('/OUT:"{0}"' -f $exe), ('"{0}"' -f $object)), [Text.Encoding]::Unicode)
    & $linker "@$response"
    if ($LASTEXITCODE -ne 0) { throw 'Wrapper response-file/link environment probe failed.' }
    $bytes = [IO.File]::ReadAllBytes($exe)
    $pe = [BitConverter]::ToInt32($bytes, 0x3c)
    if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x8664) { throw 'Wrapper probe must produce x64 PE.' }
    # Run only on native x64; cross CI has a separate mandatory native consumer.
    if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() -ceq 'X64') {
        & $exe
        if ($LASTEXITCODE -ne 0) { throw 'Native x64 wrapper probe execution failed.' }
    }
    $missing = Join-Path $root 'missing.obj'
    $null = & $archiver /nologo "/OUT:$(Join-Path $root 'failure.lib')" $missing 2>&1
    if ($LASTEXITCODE -eq 0) { throw 'Wrapper swallowed the native tool failure.' }
} finally {
    $env:INCLUDE = $savedInclude
    $env:LIB = $savedLib
}
Write-Host 'MSVC wrapper environment, Unicode/space paths, >8191 command, response file, PE and failure-exit probes passed.'
