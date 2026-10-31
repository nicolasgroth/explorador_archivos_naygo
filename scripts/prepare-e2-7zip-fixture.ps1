# Naygo — prepara un fixture aislado para validar drops desde 7-Zip.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param(
    [string] $OutputRoot,
    [ValidateRange(0, 10000)] [int] $ExtraFiles = 0
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$allowedRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot 'target\agent-out'))
if (-not $OutputRoot) { $OutputRoot = Join-Path $allowedRoot 'e2-7zip-fixtures' }
$root = [System.IO.Path]::GetFullPath($OutputRoot)
$allowedPrefix = $allowedRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $root.StartsWith($allowedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "La salida debe quedar dentro de '$allowedRoot' para mantener el fixture aislado."
}
$runId = '{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), ([guid]::NewGuid().ToString('N').Substring(0, 8))
$runDir = Join-Path $root $runId
$archivePath = Join-Path $runDir 'e2-drop-fixture.zip'
$destinationPath = Join-Path $runDir 'destination'
$sourceDir = Join-Path $runDir 'archive-content'

if (Test-Path -LiteralPath $runDir) {
    throw "La carpeta de salida ya existe; no se sobrescribe: $runDir"
}

New-Item -ItemType Directory -Path $sourceDir -Force | Out-Null
New-Item -ItemType Directory -Path $destinationPath -Force | Out-Null
$modelDir = Join-Path $sourceDir 'E2 fixture'
New-Item -ItemType Directory -Path $modelDir | Out-Null
$nestedDir = Join-Path $modelDir 'nested\level'
New-Item -ItemType Directory -Path $nestedDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $modelDir 'empty-folder') | Out-Null
[System.IO.File]::WriteAllText(
    (Join-Path $nestedDir 'deep.txt'),
    "Naygo nested folder regression fixture.`n",
    [System.Text.UTF8Encoding]::new($false)
)
if ($ExtraFiles -gt 0) {
    $manyDir = Join-Path $modelDir 'many'
    New-Item -ItemType Directory -Path $manyDir | Out-Null
    for ($index = 0; $index -lt $ExtraFiles; $index++) {
        [System.IO.File]::WriteAllText(
            (Join-Path $manyDir ('item-{0:D5}.txt' -f $index)),
            "fixture $index`n",
            [System.Text.UTF8Encoding]::new($false)
        )
    }
}

$modelText = @(
    '# Naygo E-2 drop regression fixture'
    'o fixture'
    'v 0.000000 0.000000 0.000000'
    'v 1.000000 0.000000 0.000000'
    'v 0.000000 1.000000 0.000000'
    'f 1 2 3'
) -join "`n"
$modelPath = Join-Path $modelDir 'sample.obj'
[System.IO.File]::WriteAllText($modelPath, $modelText + "`n", [System.Text.UTF8Encoding]::new($false))
[System.IO.File]::WriteAllText(
    (Join-Path $modelDir 'readme.txt'),
    "Naygo E-2 fixture. Expected to be copied from the archive into the empty destination.`n",
    [System.Text.UTF8Encoding]::new($false)
)
[System.IO.Compression.ZipFile]::CreateFromDirectory($sourceDir, $archivePath)

$hash = (Get-FileHash -LiteralPath $modelPath -Algorithm SHA256).Hash.ToLowerInvariant()
$instructions = @"
Fixture de regresión E-2 — drop desde 7-Zip

Archivo: $archivePath
Destino vacío: $destinationPath
Contenido esperado al soltar la carpeta "E2 fixture":
  E2 fixture/sample.obj
  E2 fixture/readme.txt
  E2 fixture/nested/level/deep.txt
  E2 fixture/empty-folder/ (vacía)
  E2 fixture/many/ ($ExtraFiles archivos adicionales, si se solicitó)
SHA-256 esperado de sample.obj: $hash

Abre el ZIP con 7-Zip, arrastra la carpeta "E2 fixture" al panel de Naygo que muestra
el destino vacío y acepta la copia. Comprueba que Naygo siga respondiendo, que aparezcan
los dos archivos, y que el SHA-256 de sample.obj coincida. Repite con el archivo OBJ solo.
Este fixture no contiene datos personales y se crea en una carpeta nueva; no modifica ni
borra archivos existentes.
"@
Set-Content -LiteralPath (Join-Path $runDir 'LEEME.txt') -Value $instructions -Encoding utf8

[pscustomobject]@{
    FixtureDirectory = $runDir
    Archive = $archivePath
    EmptyDestination = $destinationPath
    ExpectedObjSha256 = $hash
} | Format-List
