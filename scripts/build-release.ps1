# Naygo - orquestador de empaquetado: compila release, arma el ZIP portable y
# (si Inno Setup esta instalado) genera el instalador.
# Copyright (c) 2026 Nicolas Groth / ISGroth. MIT License.
# Autor: Nicolás Groth <ngroth@gmail.com> — ISGroth.
#
# Uso:  powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
#       powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1 `
#         -SignToolPath 'C:\...\signtool.exe' -CertificateThumbprint '<SHA1>'
# Prerequisitos: Rust (toolchain MSVC). Inno Setup (ISCC.exe) opcional: si falta,
# se genera solo el ZIP portable y se avisa.

[CmdletBinding()]
param(
    # Omitir ambos mantiene el build sin firmar (comportamiento apto para desarrollo).
    [string]$SignToolPath = $env:NAYGO_SIGNTOOL,
    [string]$CertificateThumbprint = $env:NAYGO_SIGN_CERT_SHA1,
    [string]$TimestampUrl = $env:NAYGO_SIGN_TIMESTAMP_URL
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot           # raiz del repo (scripts/ esta un nivel abajo)
$dist = Join-Path $repo "dist"

if ([string]::IsNullOrWhiteSpace($TimestampUrl)) {
    $TimestampUrl = 'http://timestamp.digicert.com'
}

# PowerShell 7 normalmente expone Get-FileHash, pero algunos runtimes embebidos/minimalistas no
# incluyen ese cmdlet. El empaquetado no debe fallar DESPUÉS de crear ambos artefactos solo por
# imprimir sus checksums: usamos el cmdlet cuando existe y un fallback .NET compatible si no.
function Get-Sha256Hex([string]$path) {
    $getFileHash = Get-Command Get-FileHash -ErrorAction SilentlyContinue
    if ($null -ne $getFileHash) {
        return (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant()
    }

    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($path)
    try {
        # BitConverter existe tanto en Windows PowerShell clásico como en PowerShell moderno.
        return ([BitConverter]::ToString($sha256.ComputeHash($stream))).Replace("-", "").ToLowerInvariant()
    } finally {
        $stream.Dispose()
        $sha256.Dispose()
    }
}

function Invoke-SignedInstaller([string]$path) {
    $hasTool = -not [string]::IsNullOrWhiteSpace($SignToolPath)
    $hasCert = -not [string]::IsNullOrWhiteSpace($CertificateThumbprint)
    if (-not $hasTool -and -not $hasCert) {
        Write-Warning 'Instalador sin firma: no se configuró signtool ni certificado.'
        return
    }
    if (-not $hasTool -or -not $hasCert) {
        throw 'Para firmar, entrega SignToolPath y CertificateThumbprint (o NAYGO_SIGNTOOL y NAYGO_SIGN_CERT_SHA1).'
    }
    if (-not (Test-Path -LiteralPath $SignToolPath)) {
        throw "No existe signtool.exe: $SignToolPath"
    }

    Write-Host "Firmando instalador con SHA-256..."
    & $SignToolPath sign /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 $path
    if ($LASTEXITCODE -ne 0) { throw "signtool sign falló para $path." }
    & $SignToolPath verify /pa /all /v $path
    if ($LASTEXITCODE -ne 0) { throw "signtool verify falló para $path." }
    Write-Host 'Firma Authenticode verificada.'
}

# --- 1. Version: fuente unica = workspace.package.version del Cargo.toml raiz ---
$cargoToml = Get-Content (Join-Path $repo "Cargo.toml") -Raw
if ($cargoToml -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
    throw "No pude leer la version del Cargo.toml raiz."
}
$version = $Matches[1]
Write-Host "Naygo version $version"

# --- 2. Compilar release (solo el binario del producto, no todo el workspace) ---
Write-Host "Compilando release..."
& cargo build --release -p naygo-ui-slint
if ($LASTEXITCODE -ne 0) { throw "cargo build --release fallo." }
$exe = Join-Path $repo "target\release\naygo.exe"
if (-not (Test-Path $exe)) { throw "No se encontro $exe tras compilar." }

# --- 3. Preparar dist/ ---
if (-not (Test-Path $dist)) { New-Item -ItemType Directory -Path $dist | Out-Null }

# --- 4. ZIP portable: ejecutable y documentos (los símbolos no viajan por defecto) ---
Write-Host "Armando ZIP portable..."
$stage = Join-Path $dist "portable-stage"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item $exe (Join-Path $stage "naygo.exe")
# El PDB se conserva en target\release para diagnóstico, pero no se agrega al ZIP portable:
# pesa cientos de MB y el usuario normal no lo necesita para navegar archivos.
$pdb = Join-Path $repo "target\release\naygo.pdb"
if (Test-Path $pdb) {
    Write-Host "naygo.pdb generado (no se incluye en el ZIP portable)."
} else {
    Write-Warning "No se encontró $pdb; el instalador no ofrecerá símbolos de depuración."
}
Copy-Item (Join-Path $repo "LICENSE") (Join-Path $stage "LICENSE")
Copy-Item (Join-Path $repo "installer\LEEME.txt") (Join-Path $stage "LEEME.txt")
Copy-Item (Join-Path $repo "THIRD-PARTY-NOTICES.md") (Join-Path $stage "THIRD-PARTY-NOTICES.md")
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
    Write-Warning "y vuelve a correr este script."
} else {
    Write-Host "Generando instalador con Inno Setup ($isccPath)..."
    & $isccPath "/DMyAppVersion=$version" (Join-Path $repo "installer\naygo.iss")
    if ($LASTEXITCODE -ne 0) { throw "ISCC fallo al compilar el instalador." }
    $setup = Join-Path $dist "Naygo-$version-setup.exe"
    Invoke-SignedInstaller $setup
    Write-Host "Instalador: $setup"
}

# Checksums reproducibles también en builds LOCALES (CI ya hacía esto por separado).
# Así el contenido de dist/ queda completo y verificable sin depender del workflow remoto.
$artifacts = @($zip)
$setup = Join-Path $dist "Naygo-$version-setup.exe"
if (Test-Path -LiteralPath $setup) { $artifacts += $setup }
$checksumPath = Join-Path $dist "SHA256SUMS.txt"
$checksumLines = foreach ($artifact in $artifacts) {
    $hash = Get-Sha256Hex $artifact
    "$hash  $(Split-Path -Leaf $artifact)"
}
[System.IO.File]::WriteAllLines(
    $checksumPath,
    $checksumLines,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Host "Checksums: $checksumPath"

Write-Host "Listo. Artefactos en: $dist"
