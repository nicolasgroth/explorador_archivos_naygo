# Naygo — regresión: evitar mezclar un setup previo con un portable nuevo.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

$ErrorActionPreference = 'Stop'
$fixture = Join-Path ([System.IO.Path]::GetTempPath()) ("naygo-preflight-test-" + [guid]::NewGuid())
$savedPath = $env:PATH
try {
    New-Item -ItemType Directory -Path (Join-Path $fixture 'scripts'), (Join-Path $fixture 'dist') | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'build-release.ps1') -Destination (Join-Path $fixture 'scripts')
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'NaygoSigning.psm1') -Destination (Join-Path $fixture 'scripts')
    [IO.File]::WriteAllText((Join-Path $fixture 'Cargo.toml'), "[workspace.package]`nversion = `"0.5.4`"`n")
    $oldSetup = Join-Path $fixture 'dist\Naygo-0.5.4-setup.exe'
    $oldZip = Join-Path $fixture 'dist\Naygo-0.5.4-portable.zip'
    $oldManifest = Join-Path $fixture 'dist\SHA256SUMS.txt'
    [IO.File]::WriteAllText($oldSetup, 'old setup')
    [IO.File]::WriteAllText($oldZip, 'old portable')
    [IO.File]::WriteAllText($oldManifest, 'old manifest')
    $env:PATH = ''
    function Test-Path {
        param([string]$LiteralPath, [string]$Path)
        $candidate = if ($LiteralPath) { $LiteralPath } else { $Path }
        if ($candidate -match 'Inno Setup 6[\\/]ISCC\.exe$') { return $false }
        Microsoft.PowerShell.Management\Test-Path -LiteralPath $candidate
    }
    $caught = $false
    try { & (Join-Path $fixture 'scripts\build-release.ps1') -SkipBuild }
    catch {
        if ($_.Exception.Message -notmatch 'existe un setup previo') { throw }
        $caught = $true
    }
    if (-not $caught) { throw 'El preflight no rechazó un setup previo sin ISCC.' }
    if ([IO.File]::ReadAllText($oldSetup) -ne 'old setup' -or
        [IO.File]::ReadAllText($oldZip) -ne 'old portable' -or
        [IO.File]::ReadAllText($oldManifest) -ne 'old manifest') { throw 'El preflight modificó la distribución anterior.' }
    Write-Host 'Preflight: setup y portable previos preservados; mezcla rechazada.'
} finally {
    $env:PATH = $savedPath
    Remove-Item function:Test-Path -ErrorAction SilentlyContinue
    if (Microsoft.PowerShell.Management\Test-Path -LiteralPath $fixture) {
        $resolvedFixture = (Get-Item -LiteralPath $fixture).FullName
        $resolvedTemp = (Get-Item -LiteralPath ([IO.Path]::GetTempPath())).FullName.TrimEnd('\') + '\'
        if (-not $resolvedFixture.StartsWith($resolvedTemp, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([IO.Path]::GetFileName($resolvedFixture).StartsWith('naygo-preflight-test-'))) {
            throw "No se limpió una ruta temporal inesperada: $resolvedFixture"
        }
        Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
    }
}
