# Naygo — crea datos sintéticos aislados para capturas de documentación.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$root = Join-Path $repo 'target\agent-out\screenshots'
$id = '{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), ([guid]::NewGuid().ToString('N').Substring(0, 8))
$run = Join-Path $root $id
$documents = Join-Path $run 'Proyecto Aurora\Documentos'
$delivery = Join-Path $run 'Proyecto Aurora\Entregas'
New-Item -ItemType Directory -Path $documents, $delivery -Force | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $documents 'Guia.txt'), "Proyecto Aurora`n`nEjemplo de documentación para una captura de Naygo.`n", $utf8)
[IO.File]::WriteAllText((Join-Path $documents 'Tareas.csv'), "tarea,estado`nDiseno,completo`nRevision,pendiente`n", $utf8)
[IO.File]::WriteAllText((Join-Path $delivery 'Entrega.txt'), "Contenido ficticio para la demostración.`n", $utf8)
New-Item -ItemType Directory -Path (Join-Path $documents 'Recursos') | Out-Null
Write-Output "Fixture: $run"
Write-Output "Panel izquierdo: $documents"
Write-Output "Panel derecho: $delivery"
Write-Output 'No se ha abierto Naygo ni modificado la instancia en ejecución.'
