# Naygo - orquestador de empaquetado: compila release, arma el ZIP portable y
# (si Inno Setup esta instalado) genera el instalador.
# Copyright (c) 2026 Nicolas Groth / ISGroth. MIT License.
# Autor: Nicolás Groth <ngroth@gmail.com> — ISGroth.
#
# Uso:  powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
# Prerequisitos: Rust (toolchain MSVC). Inno Setup (ISCC.exe) opcional: si falta,
# se genera solo el ZIP portable y se avisa.

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot           # raiz del repo (scripts/ esta un nivel abajo)
$dist = Join-Path $repo "dist"

# --- 1. Version: fuente unica = workspace.package.version del Cargo.toml raiz ---
$cargoToml = Get-Content (Join-Path $repo "Cargo.toml") -Raw
if ($cargoToml -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
    throw "No pude leer la version del Cargo.toml raiz."
}
$version = $Matches[1]
Write-Host "Naygo version $version"

# --- 2. Compilar release ---
Write-Host "Compilando release..."
& cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build --release fallo." }
$exe = Join-Path $repo "target\release\naygo.exe"
if (-not (Test-Path $exe)) { throw "No se encontro $exe tras compilar." }

# --- 3. Preparar dist/ ---
if (-not (Test-Path $dist)) { New-Item -ItemType Directory -Path $dist | Out-Null }

# --- 4. ZIP portable: naygo.exe + LICENSE + LEEME.txt ---
Write-Host "Armando ZIP portable..."
$stage = Join-Path $dist "portable-stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item $exe (Join-Path $stage "naygo.exe")
Copy-Item (Join-Path $repo "LICENSE") (Join-Path $stage "LICENSE")
Copy-Item (Join-Path $repo "installer\LEEME.txt") (Join-Path $stage "LEEME.txt")
$zip = Join-Path $dist "Naygo-$version-portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Host "Portable: $zip"

# --- 5. Imagenes del asistente: BMP desde logo_naygo.png (Inno consume BMP) ---
# Usa System.Drawing para redimensionar. Tamanos tipicos de Inno: 164x314 y 55x58.
Add-Type -AssemblyName System.Drawing
function Convert-LogoToBmp([string]$dst, [int]$w, [int]$h) {
    $src = Join-Path $repo "assets\icons\logo_naygo.png"
    $img = [System.Drawing.Image]::FromFile($src)
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.Clear([System.Drawing.Color]::White)
    # Encaja el logo cuadrado centrado dentro del area.
    $side = [Math]::Min($w, $h)
    $x = [int](($w - $side) / 2); $y = [int](($h - $side) / 2)
    $g.DrawImage($img, $x, $y, $side, $side)
    $g.Dispose(); $img.Dispose()
    $bmp.Save($dst, [System.Drawing.Imaging.ImageFormat]::Bmp)
    $bmp.Dispose()
}
$wizLarge = Join-Path $repo "installer\wizard-large.bmp"
$wizSmall = Join-Path $repo "installer\wizard-small.bmp"
Convert-LogoToBmp $wizLarge 164 314
Convert-LogoToBmp $wizSmall 55 58
Write-Host "Imagenes del asistente generadas."

# --- 6. Instalador Inno (opcional): usa ISCC.exe del PATH o de la ruta estandar ---
# Busca ISCC.exe primero en el PATH; si no, en las ubicaciones tipicas de Inno Setup 6
# (instalacion por defecto, 64 y 32 bits). Asi no hace falta tenerlo en el PATH.
$isccPath = $null
$fromPath = Get-Command ISCC.exe -ErrorAction SilentlyContinue
if ($null -ne $fromPath) {
    $isccPath = $fromPath.Source
} else {
    foreach ($cand in @(
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe"
    )) {
        if (Test-Path $cand) { $isccPath = $cand; break }
    }
}
if ($null -eq $isccPath) {
    Write-Warning "Inno Setup (ISCC.exe) no encontrado (ni en PATH ni en las rutas estandar)."
    Write-Warning "Se genero solo el ZIP portable. Para el instalador, instala Inno Setup:"
    Write-Warning "  https://jrsoftware.org/isdl.php"
    Write-Warning "y volve a correr este script."
} else {
    Write-Host "Generando instalador con Inno Setup ($isccPath)..."
    & $isccPath "/DMyAppVersion=$version" (Join-Path $repo "installer\naygo.iss")
    if ($LASTEXITCODE -ne 0) { throw "ISCC fallo al compilar el instalador." }
    Write-Host "Instalador: $dist\Naygo-$version-setup.exe"
}

Write-Host "Listo. Artefactos en: $dist"
