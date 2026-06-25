# Naygo — suite completa de regresion: corre TODOS los tests + gates de calidad.
# Copyright (c) 2026 Nicolas Groth / ISGroth. MIT License.
#
# Punto de entrada UNICO para verificar el sistema completo de un solo comando.
# Ejecuta, en orden, las tres puertas que deben estar verdes antes de cada commit:
#   1. Tests de todo el workspace  (cargo test --workspace)
#   2. Clippy con warnings = error (cargo clippy --workspace --all-targets -- -D warnings)
#   3. Formato                     (cargo fmt --all --check)
#
# Imprime un resumen legible (tests por crate, estado de clippy/fmt) y termina con
# codigo de salida 0 si TODO paso, o 1 si algo fallo (apto para CI o para un hook).
#
# Uso:
#   scripts\test-all.ps1            # corre las tres puertas y resume
#   scripts\test-all.ps1 -NoLint    # solo tests (salta clippy y fmt; iteracion rapida)
#   scripts\test-all.ps1 -FailFast  # se detiene en la primera puerta que falle
#
# Entorno: PowerShell 5.1 (sin && ni ||). Build con CARGO_BUILD_JOBS=2 (equipos modestos).

[CmdletBinding()]
param(
    [switch]$NoLint,
    [switch]$FailFast
)

$ErrorActionPreference = 'Continue'
# Limitar la paralelizacion del build: la prioridad del proyecto es correr en maquinas modestas.
$env:CARGO_BUILD_JOBS = '2'

# Ubicarse en la raiz del repo (el script vive en scripts\), sin depender del cwd del llamador.
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

# Acumuladores del resumen final.
$results = [ordered]@{}
$allOk = $true

function Write-Section($title) {
    Write-Host ''
    Write-Host ('=' * 64) -ForegroundColor DarkCyan
    Write-Host "  $title" -ForegroundColor Cyan
    Write-Host ('=' * 64) -ForegroundColor DarkCyan
}

# ─────────────────────────── 1. TESTS ───────────────────────────
Write-Section 'Tests del workspace (cargo test --workspace)'
# Capturar la salida para extraer las lineas "test result:" por crate, ademas de mostrarla.
$testOutput = & cargo test --workspace 2>&1
$testExit = $LASTEXITCODE
$testOutput | ForEach-Object { Write-Host $_ }

# Sumar passed/failed de todas las lineas "test result: ok. N passed; M failed; ...".
$passed = 0
$failed = 0
$ignored = 0
foreach ($line in $testOutput) {
    $text = [string]$line
    if ($text -match 'test result:.*?(\d+) passed.*?(\d+) failed.*?(\d+) ignored') {
        $passed += [int]$Matches[1]
        $failed += [int]$Matches[2]
        $ignored += [int]$Matches[3]
    }
}
if ($testExit -eq 0) {
    $results['Tests'] = "OK  ($passed passed, $ignored ignored)"
} else {
    $results['Tests'] = "FALLO  ($passed passed, $failed failed)"
    $allOk = $false
    if ($FailFast) {
        Write-Host ''
        Write-Host 'FailFast: los tests fallaron, abortando.' -ForegroundColor Red
        Pop-Location
        exit 1
    }
}

# ─────────────────────────── 2. CLIPPY ───────────────────────────
if (-not $NoLint) {
    Write-Section 'Clippy (warnings = error)'
    & cargo clippy --workspace --all-targets -- -D warnings 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -eq 0) {
        $results['Clippy'] = 'OK  (sin warnings)'
    } else {
        $results['Clippy'] = 'FALLO  (hay warnings/errores)'
        $allOk = $false
        if ($FailFast) {
            Write-Host ''
            Write-Host 'FailFast: clippy fallo, abortando.' -ForegroundColor Red
            Pop-Location
            exit 1
        }
    }

    # ─────────────────────────── 3. FORMATO ───────────────────────────
    Write-Section 'Formato (cargo fmt --check)'
    & cargo fmt --all -- --check 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -eq 0) {
        $results['Formato'] = 'OK  (todo formateado)'
    } else {
        $results['Formato'] = 'FALLO  (corre: cargo fmt --all)'
        $allOk = $false
    }
} else {
    $results['Clippy'] = 'OMITIDO  (-NoLint)'
    $results['Formato'] = 'OMITIDO  (-NoLint)'
}

# ─────────────────────────── RESUMEN ───────────────────────────
Write-Section 'Resumen'
foreach ($k in $results.Keys) {
    $v = $results[$k]
    $color = if ($v -like 'OK*') { 'Green' } elseif ($v -like 'OMITIDO*') { 'Yellow' } else { 'Red' }
    Write-Host ('  {0,-10} {1}' -f $k, $v) -ForegroundColor $color
}
Write-Host ''
if ($allOk) {
    Write-Host '  TODO VERDE: el sistema completo paso la verificacion.' -ForegroundColor Green
    Pop-Location
    exit 0
} else {
    Write-Host '  HAY FALLOS: revisa las secciones marcadas arriba.' -ForegroundColor Red
    Pop-Location
    exit 1
}
