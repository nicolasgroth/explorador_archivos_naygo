# Naygo — prepara datos desechables para validación manual W-07/W-13.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param(
    [ValidateRange(1, 100000)]
    [int[]] $FileCounts = @(10000, 100000),
    [string] $RootPath
)

$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$allowedRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot 'target\agent-out'))
if (-not $RootPath) {
    $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $RootPath = Join-Path $allowedRoot "w13-filesystem-$stamp"
}

$resolvedRoot = [System.IO.Path]::GetFullPath($RootPath)
$allowedPrefix = $allowedRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $resolvedRoot.StartsWith($allowedPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "La salida debe quedar dentro de '$allowedRoot' para mantener las fixtures aisladas del resto del equipo."
}
if (Test-Path -LiteralPath $resolvedRoot) {
    throw "La carpeta de salida ya existe; indica una ruta nueva para no sobrescribir datos: $resolvedRoot"
}

$counts = @($FileCounts | Sort-Object -Unique)
if ($counts.Count -ne $FileCounts.Count) {
    throw 'FileCounts no debe repetir cantidades.'
}

New-Item -ItemType Directory -Path $allowedRoot -Force | Out-Null
New-Item -ItemType Directory -Path $resolvedRoot | Out-Null

function Write-EmptyFiles {
    param(
        [Parameter(Mandatory)] [string] $Directory,
        [Parameter(Mandatory)] [int] $Count
    )

    New-Item -ItemType Directory -Path $Directory | Out-Null
    for ($index = 1; $index -le $Count; $index++) {
        $name = 'item-{0:D6}.txt' -f $index
        $path = Join-Path $Directory $name
        $stream = [System.IO.FileStream]::new(
            $path,
            [System.IO.FileMode]::CreateNew,
            [System.IO.FileAccess]::Write,
            [System.IO.FileShare]::None
        )
        $stream.Dispose()

        if (($index % 10000) -eq 0 -or $index -eq $Count) {
            Write-Progress -Activity "Creando $Count archivos vacíos" `
                -Status "$index / $Count" -PercentComplete (($index / $Count) * 100)
        }
    }
    Write-Progress -Activity "Creando $Count archivos vacíos" -Completed
}

foreach ($count in $counts) {
    Write-EmptyFiles -Directory (Join-Path $resolvedRoot "listing-$count") -Count $count
}

$source = Join-Path $resolvedRoot 'ops\source'
$destination = Join-Path $resolvedRoot 'ops\destination'
New-Item -ItemType Directory -Path (Join-Path $source 'nested') -Force | Out-Null
New-Item -ItemType Directory -Path $destination -Force | Out-Null
[System.IO.File]::WriteAllText((Join-Path $source 'copy-me.txt'), 'copy fixture')
[System.IO.File]::WriteAllText((Join-Path $source 'conflict.txt'), 'source version')
[System.IO.File]::WriteAllText((Join-Path $source 'unicode-café.txt'), 'UTF-8 filename fixture')
[System.IO.File]::WriteAllText((Join-Path $source 'nested\move-me.txt'), 'move fixture')
[System.IO.File]::WriteAllText((Join-Path $destination 'conflict.txt'), 'destination version')

$extensionRoot = Join-Path $resolvedRoot 'extensions'
New-Item -ItemType Directory -Path (Join-Path $extensionRoot '1.The Buddha') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $extensionRoot '2.3DBenchy') -Force | Out-Null
$stlFixture = @'
solid extension-fixture
endsolid extension-fixture
'@
[System.IO.File]::WriteAllText(
    (Join-Path $extensionRoot 'piece.stl'),
    $stlFixture,
    [System.Text.UTF8Encoding]::new($false)
)
[System.IO.File]::WriteAllText((Join-Path $extensionRoot 'no-extension'), 'extensionless fixture')

$dropSource = Join-Path $resolvedRoot 'drop-archive-source'
New-Item -ItemType Directory -Path $dropSource | Out-Null
$objFixture = @"
# Naygo 7-Zip external-drop fixture
o drop-fixture
v 0 0 0
v 1 0 0
v 0 1 0
f 1 2 3
"@
[System.IO.File]::WriteAllText(
    (Join-Path $dropSource 'drop-fixture.obj'),
    $objFixture,
    [System.Text.UTF8Encoding]::new($false)
)
[System.IO.File]::WriteAllText(
    (Join-Path $dropSource 'drop-fixture.txt'),
    'Synthetic 7-Zip drop fixture; no personal data.',
    [System.Text.UTF8Encoding]::new($false)
)
$dropArchive = Join-Path $resolvedRoot 'drop-7zip-fixture.zip'
Compress-Archive -LiteralPath @(
    (Join-Path $dropSource 'drop-fixture.obj'),
    (Join-Path $dropSource 'drop-fixture.txt')
) -DestinationPath $dropArchive

$readme = @"
Naygo W-07/W-13 — filesystem fixtures
Created: $(Get-Date -Format o)
Root: $resolvedRoot
Listing folders: $($counts -join ', ') empty files each
Operations: ops/source and ops/destination contain disposable copy/move/conflict fixtures.
Extensions: extensions contains dotted folders, an STL, and a file with no extension.
External drop: drop-7zip-fixture.zip contains a tiny OBJ and TXT with synthetic content.

These are generated test files. No user documents were copied or modified.
Use a dedicated test session and keep results in this folder. The extension fixture is read-only;
drop copied content into a separate empty folder under this fixture root. Remove this exact folder
manually after the acceptance run; this generator never deletes or overwrites existing paths.
"@
[System.IO.File]::WriteAllText(
    (Join-Path $resolvedRoot 'README.txt'),
    $readme,
    [System.Text.UTF8Encoding]::new($false)
)

$summary = [pscustomobject]@{
    schema = 'naygo.w13.filesystem-fixtures.v2'
    created_at = (Get-Date).ToString('o')
    root = $resolvedRoot
    listing_counts = $counts
    operations_source = Join-Path $resolvedRoot 'ops\source'
    operations_destination = $destination
    extensions = $extensionRoot
    drop_archive = $dropArchive
    drop_archive_source = $dropSource
    note = 'Synthetic disposable fixtures only; no cleanup or overwrite is performed.'
}
$summary | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $resolvedRoot 'manifest.json') -Encoding UTF8

Write-Output $resolvedRoot
