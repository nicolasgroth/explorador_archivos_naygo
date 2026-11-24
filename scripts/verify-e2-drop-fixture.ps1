# Naygo — compara una copia de prueba de arrastre sin modificar el origen ni el destino.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)] [string] $FixtureDirectory
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$allowed = [IO.Path]::GetFullPath((Join-Path $repoRoot 'target\agent-out')).TrimEnd('\') + '\'
$fixture = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $FixtureDirectory).Path)
if (-not $fixture.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'El fixture debe estar dentro de target\agent-out del checkout actual.'
}
$source = Join-Path $fixture 'archive-content\E2 fixture'
$destination = Join-Path $fixture 'destination\E2 fixture'
foreach ($path in @($source, $destination)) {
    if (-not (Test-Path -LiteralPath $path -PathType Container)) { throw "No existe la carpeta esperada: $path" }
}

function Get-Manifest([string] $root) {
    $rootItem = Get-Item -LiteralPath $root -Force
    if ($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'No se admiten enlaces en el fixture.' }
    $entries = @{}
    $pending = [Collections.Generic.Queue[string]]::new()
    $pending.Enqueue($root)
    while ($pending.Count -gt 0) {
        $current = $pending.Dequeue()
        foreach ($item in Get-ChildItem -LiteralPath $current -Force) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Enlace no permitido: $($item.FullName)" }
            $relative = $item.FullName.Substring($root.Length).TrimStart('\')
            if ($item.PSIsContainer) {
                $entries[$relative] = 'directory'
                $pending.Enqueue($item.FullName)
            } else {
                $entries[$relative] = 'file:' + (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash
            }
        }
    }
    return ,$entries
}

$expected = Get-Manifest $source
$actual = Get-Manifest $destination
$differences = [Collections.Generic.List[string]]::new()
foreach ($key in $expected.Keys) {
    if (-not $actual.ContainsKey($key)) { $differences.Add("Falta: $key") }
    elseif ($actual[$key] -ne $expected[$key]) { $differences.Add("Tipo o contenido distinto: $key") }
}
foreach ($key in $actual.Keys) {
    if (-not $expected.ContainsKey($key)) { $differences.Add("Adicional: $key") }
}
if ($differences.Count -gt 0) {
    $differences | Sort-Object | ForEach-Object { Write-Host $_ }
    throw "La copia presenta $($differences.Count) diferencias."
}
$files = @($expected.Values | Where-Object { $_ -like 'file:*' }).Count
$directories = @($expected.Values | Where-Object { $_ -eq 'directory' }).Count
Write-Host "Coinciden $files archivos y $directories subcarpetas; SHA-256 verificado para cada archivo."
