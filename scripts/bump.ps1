# Naygo — versionado: infiere el nivel SemVer de los commits y publica una version.
# Copyright (c) 2026 Nicolas Groth / ISGroth. MIT License.
#
# Uso:
#   scripts\bump.ps1                 # infiere patch/minor/major de los commits
#   scripts\bump.ps1 -Level minor    # fuerza el nivel
#   scripts\bump.ps1 -DryRun         # muestra que haria, sin tocar nada
#   scripts\bump.ps1 -Push           # tras commit+tag, hace git push y push --tags
#
# Reglas (Conventional Commits) desde el ultimo tag vX.Y.Z:
#   feat: -> minor   fix: -> patch   "BREAKING CHANGE" o "!" -> major
#   si hay commits pero ninguno aporta nivel -> patch (fallback)
#   si no hay commits nuevos y ya habia tag -> no versiona (avisa)
# NO hace push salvo -Push. Pensado para correrlo Nicolas; Claude no lo ejecuta.

[CmdletBinding()]
param(
    [ValidateSet('patch', 'minor', 'major')]
    [string]$Level,
    [switch]$DryRun,
    [switch]$Push
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$cargoPath = Join-Path $repo "Cargo.toml"
$changelogPath = Join-Path $repo "CHANGELOG.md"

# --- Funciones puras (testeables sin git) ---

# Decide el nivel a partir de los "subjects" y "bodies" de los commits.
# Devuelve 'major'|'minor'|'patch'|$null (null = ningun commit aporto nivel).
function Get-BumpLevel {
    param([string[]]$Subjects, [string[]]$Bodies)
    $level = $null
    for ($i = 0; $i -lt $Subjects.Count; $i++) {
        $s = $Subjects[$i]
        $b = if ($i -lt $Bodies.Count) { $Bodies[$i] } else { "" }
        $isBreaking = ($s -match '^[a-z]+(\(.+\))?!:') -or ($b -match 'BREAKING CHANGE')
        if ($isBreaking) { return 'major' }
        if ($s -match '^feat(\(.+\))?:') { if ($level -ne 'minor') { $level = 'minor' } }
        elseif ($s -match '^fix(\(.+\))?:') { if ($null -eq $level) { $level = 'patch' } }
    }
    return $level
}

# Sube una version "X.Y.Z" segun el nivel. Devuelve la nueva "X.Y.Z".
function Step-Version {
    param([string]$Current, [string]$BumpLevel)
    if ($Current -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
        throw "Version actual con formato inesperado: '$Current'"
    }
    $maj = [int]$Matches[1]; $min = [int]$Matches[2]; $pat = [int]$Matches[3]
    switch ($BumpLevel) {
        'major' { $maj++; $min = 0; $pat = 0 }
        'minor' { $min++; $pat = 0 }
        'patch' { $pat++ }
        default { throw "Nivel invalido: '$BumpLevel'" }
    }
    return "$maj.$min.$pat"
}

# --- Flujo principal ---

# 0) Working tree limpio (no versionar a medias).
$dirty = git -C $repo status --porcelain
if ($dirty -and -not $DryRun) {
    throw "El working tree tiene cambios sin commitear. Haz commit o stash antes de versionar."
}

# 1) Version actual desde Cargo.toml (la linea de [workspace.package]).
$cargoRaw = Get-Content $cargoPath -Raw
if ($cargoRaw -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
    throw "No pude leer la version de Cargo.toml."
}
$current = $Matches[1]

# 2) Ultimo tag vX.Y.Z (si hay).
$lastTag = (git -C $repo tag --list "v*" --sort=-v:refname | Select-Object -First 1)
$hasTag = [bool]$lastTag

# 3) Determinar el nivel.
if ($Level) {
    $bump = $Level
} else {
    if ($hasTag) {
        $range = "$lastTag..HEAD"
    } else {
        $range = "HEAD"  # primer release: considerar todos los commits
    }
    $subjects = @(git -C $repo log $range --format="%s")
    $bodies = @(git -C $repo log $range --format="%b")
    if ($subjects.Count -eq 0 -and $hasTag) {
        Write-Host "Nada que versionar: no hay commits nuevos desde $lastTag."
        return
    }
    $inferred = Get-BumpLevel -Subjects $subjects -Bodies $bodies
    if ($null -eq $inferred) { $inferred = 'patch' }  # fallback
    $bump = $inferred
}

# 4) Nueva version.
$new = Step-Version -Current $current -BumpLevel $bump
$newTag = "v$new"
if (git -C $repo tag --list $newTag) {
    throw "El tag $newTag ya existe."
}
$today = (Get-Date -Format "yyyy-MM-dd")

Write-Host "Version actual : $current"
Write-Host "Nivel          : $bump"
Write-Host "Nueva version  : $new   (tag $newTag, fecha $today)"

if ($DryRun) {
    Write-Host "[DryRun] No se modifico nada."
    return
}

# 5) Reescribir version en Cargo.toml (solo la primera ocurrencia, la del workspace).
$cargoNew = [regex]::Replace(
    $cargoRaw,
    '(?m)^(\s*version\s*=\s*")[^"]+(")',
    "`${1}$new`${2}",
    1
)
Set-Content -Path $cargoPath -Value $cargoNew -Encoding utf8 -NoNewline

# 6) Mover "## [Sin publicar]" -> "## [X.Y.Z] - fecha" y crear un "Sin publicar" vacio.
$cl = Get-Content $changelogPath -Raw
if ($cl -notmatch '## \[Sin publicar\]') {
    throw "No encontre '## [Sin publicar]' en el CHANGELOG."
}
$replacement = "## [Sin publicar]`r`n`r`n## [$new] - $today"
$cl = $cl -replace '## \[Sin publicar\]', $replacement
Set-Content -Path $changelogPath -Value $cl -Encoding utf8

# 7) Refrescar el lock (los crates naygo-* heredan la version del workspace).
try {
    cargo update --workspace --offline 2>$null
} catch {
    Write-Warning "cargo update fallo (offline); el lock se regenerara al compilar."
}

# 8) Commit + tag.
git -C $repo add Cargo.toml Cargo.lock CHANGELOG.md
git -C $repo commit -m "chore(release): $newTag"
git -C $repo tag $newTag
Write-Host "Commit y tag $newTag creados."

# 9) Push solo con -Push.
if ($Push) {
    git -C $repo push
    git -C $repo push --tags
    Write-Host "Push hecho (commit + tags)."
} else {
    Write-Host "Recuerda publicar:  git push && git push --tags"
}
