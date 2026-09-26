param(
    [Parameter(Mandatory)][string]$HostCompiler,
    [Parameter(Mandatory)][string]$TargetDirectory,
    [Parameter(Mandatory)][hashtable]$TargetEnvironment,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$root = (Resolve-Path -LiteralPath $OutputDirectory).Path
$envFile = Join-Path $root 'target-env.txt'
$lines = foreach ($key in ($TargetEnvironment.Keys | Sort-Object)) {
    $value = [string]$TargetEnvironment[$key]
    if ($key -match '[=\r\n]' -or $value -match '[\r\n]') { throw 'Invalid compiler environment.' }
    "$key=$value"
}
[IO.File]::WriteAllLines($envFile, [string[]]$lines, [Text.Encoding]::Unicode)
function C-WideString([string]$Value) { 'L"' + $Value.Replace('\', '\\').Replace('"', '\"') + '"' }
$header = @(
    '#define WRAPPER_ENV_FILE ' + (C-WideString $envFile)
    '#define WRAPPER_TARGET_DIR ' + (C-WideString $TargetDirectory)
)
[IO.File]::WriteAllLines((Join-Path $root 'msvc-wrapper-config.h'), $header, [Text.UTF8Encoding]::new($false))
$compilerDirectory = Join-Path $root 'compiler'
New-Item -ItemType Directory -Path $compilerDirectory -Force | Out-Null
$compiler = Join-Path $compilerDirectory 'cl.exe'
& $HostCompiler /nologo /W4 /WX /O2 /MT /utf-8 (Join-Path $PSScriptRoot 'msvc-tool-wrapper.c') "/I$root" "/Fe:$compiler" "/Fo:$(Join-Path $root 'wrapper.obj')"
if ($LASTEXITCODE -ne 0) { throw 'Native MSVC tool wrapper compilation failed.' }
foreach ($tool in @('link.exe', 'lib.exe')) {
    Copy-Item -LiteralPath $compiler -Destination (Join-Path $root $tool)
}
