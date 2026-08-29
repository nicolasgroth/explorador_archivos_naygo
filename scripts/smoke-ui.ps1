# Naygo — smoke interactivo post-build de la ventana real.
# Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
#
# Los runners de GitHub no exponen un escritorio Windows interactivo. Este gate se ejecuta en una
# sesión local: usa SendKeys/clics Win32, verifica que Naygo abre, conserva sus capturas y rechaza
# cualquier panic escrito durante la corrida.

[CmdletBinding()]
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\naygo.exe'),
    [string]$OutDir = (Join-Path (Split-Path -Parent $PSScriptRoot) ("target\ui-smoke\" + (Get-Date -Format 'yyyyMMdd-HHmmss')))
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not [Environment]::UserInteractive) {
    Write-Warning 'SMOKE UI OMITIDO: requiere una sesión Windows interactiva (SendKeys y capturas).'
    exit 0
}
if (-not (Test-Path -LiteralPath $Exe)) {
    throw "No existe Naygo: $Exe. Compila primero con cargo build -p naygo-ui-slint."
}

$exeDir = Split-Path -Parent (Resolve-Path -LiteralPath $Exe)
$log = Join-Path $exeDir ("naygo-" + (Get-Date -Format 'yyyy-MM-dd') + '.log')
$logBefore = if (Test-Path -LiteralPath $log) { (Get-Content -LiteralPath $log -Raw).Length } else { 0 }
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

function Invoke-Driver([string]$Script, [string]$Name) {
    $shots = Join-Path $OutDir $Name
    Write-Host "== $Name ==" -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot $Script) -Exe $Exe -Shots $shots
    $count = @(Get-ChildItem -LiteralPath $shots -Filter '*.png' -File -ErrorAction SilentlyContinue).Count
    if ($count -lt 3) {
        throw "$Name no produjo las tres capturas esperadas (en $shots)."
    }
}

Push-Location $repoRoot
try {
    Invoke-Driver 'repro-rename.ps1' 'rename'
    Invoke-Driver 'repro-scroll.ps1' 'scroll'

    $allShots = @(Get-ChildItem -LiteralPath $OutDir -Recurse -Filter '*.png' -File)
    if ($allShots.Count -lt 6) {
        throw "El smoke produjo $($allShots.Count) capturas; se esperaban al menos 6."
    }

    $logAfter = if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log -Raw } else { '' }
    $newLog = if ($logAfter.Length -gt $logBefore) { $logAfter.Substring($logBefore) } else { '' }
    if ($newLog -match '\*\*\* PANIC \*\*\*') {
        throw "Naygo registró un panic durante el smoke. Revisa: $log"
    }

    Write-Host "SMOKE UI OK: $($allShots.Count) capturas; sin panic nuevo." -ForegroundColor Green
    Write-Host "Artefactos: $OutDir"
}
finally {
    Pop-Location
}
