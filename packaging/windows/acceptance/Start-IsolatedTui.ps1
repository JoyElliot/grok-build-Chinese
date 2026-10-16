#requires -Version 5.1
[CmdletBinding()]
param([string]$BundleDirectory = $PSScriptRoot, [string]$ResultsDirectory = [IO.Path]::GetTempPath())
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# Explicit interactive opt-in. First verify all artifacts and run the isolated checks.
& (Join-Path $PSScriptRoot 'Run-WindowsAcceptance.ps1') -BundleDirectory $BundleDirectory -ResultsDirectory $ResultsDirectory -InteractiveTui
