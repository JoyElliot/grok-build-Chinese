# Parse the real production entrypoint function; no copied resolver implementation.
$ErrorActionPreference = 'Stop'
$runner = Join-Path $PSScriptRoot 'Run-MigrationTransaction.ps1'
$tokens = $null; $errors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($runner, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Runner syntax errors.' }
$functionAst = $ast.Find({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Resolve-BundleDirectory' }, $true)
if ($null -eq $functionAst) { throw 'Missing real entrypoint resolver.' }
. ([ScriptBlock]::Create($functionAst.Extent.Text))
$root = Join-Path ([IO.Path]::GetTempPath()) ('grok runner entrypoint ' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $root
try {
    $script = Join-Path $root 'Run-MigrationTransaction.ps1'
    $expected = (Resolve-Path -LiteralPath $root).Path
    foreach ($actual in @((Resolve-BundleDirectory '' $script ''), (Resolve-BundleDirectory $null '' $script), (Resolve-BundleDirectory $root '' ''))) {
        if ($actual -cne $expected) { throw "Entrypoint resolved wrong bundle: $actual" }
    }
    $rejected = $false
    try { $null = Resolve-BundleDirectory '' '' '' } catch { $rejected = $true }
    if (!$rejected) { throw 'Empty origin must fail clearly before LiteralPath binding.' }
    Write-Host 'Real runner entrypoint resolver: absent/default/explicit paths and no-origin rejection passed.'
} finally { Remove-Item -LiteralPath $root -Force }
