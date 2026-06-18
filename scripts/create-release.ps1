# Explorador de archivos — Script interactivo para crear releases
# Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

# Colores para output
$ErrorActionPreference = "Stop"

function Write-Color {
    param([string]$Text, [string]$Color = "White")
    Write-Host $Text -ForegroundColor $Color
}

function Write-Title {
    param([string]$Text)
    Write-Host ""
    Write-Color "═══════════════════════════════════════════════════════" "Cyan"
    Write-Color " $Text" "Cyan"
    Write-Color "═══════════════════════════════════════════════════════" "Cyan"
    Write-Host ""
}

function Confirm-Action {
    param([string]$Message)
    Write-Host ""
    Write-Color "$Message (s/n): " "Yellow" -NoNewline
    $response = Read-Host
    return $response -match "^[sS]$"
}

# ============================================================================
Write-Title "🚀 Naygo - Script de Release"

# 1. Verificar que estamos en la raíz del proyecto
if (-not (Test-Path "Cargo.toml")) {
    Write-Color "❌ Error: Este script debe ejecutarse desde la raíz del proyecto (donde está Cargo.toml)" "Red"
    exit 1
}

# 2. Verificar que estamos en la rama main
$currentBranch = git rev-parse --abbrev-ref HEAD
if ($currentBranch -ne "main") {
    Write-Color "⚠️  Atención: Estás en la rama '$currentBranch'" "Yellow"
    if (-not (Confirm-Action "¿Querés continuar de todos modos?")) {
        Write-Color "Operación cancelada." "Gray"
        exit 0
    }
}

# 3. Verificar que no hay cambios sin commitear
$status = git status --porcelain
if ($status) {
    Write-Color "❌ Hay cambios sin commitear:" "Red"
    git status --short
    Write-Host ""
    Write-Color "Por favor, commiteá o descartá los cambios antes de crear un release." "Yellow"
    exit 1
}

# 4. Verificar que estamos sincronizados con el remoto
Write-Color "Verificando estado del repositorio remoto..." "Gray"
git fetch --tags
$localCommit = git rev-parse HEAD
$remoteCommit = git rev-parse "@{u}" 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Color "⚠️  No se pudo verificar el remoto. ¿Estás conectado?" "Yellow"
} elseif ($localCommit -ne $remoteCommit) {
    Write-Color "❌ Tu rama local no está sincronizada con el remoto." "Red"
    Write-Color "Ejecutá: git pull" "Yellow"
    exit 1
}

Write-Color "✅ Repositorio limpio y sincronizado" "Green"

# 5. Obtener versión actual del Cargo.toml
$cargoContent = Get-Content "Cargo.toml" -Raw
if ($cargoContent -match 'version\s*=\s*"([^"]+)"') {
    $currentVersion = $matches[1]
    Write-Host ""
    Write-Color "Versión actual en Cargo.toml: " "White" -NoNewline
    Write-Color "v$currentVersion" "Cyan"
} else {
    Write-Color "❌ No se pudo leer la versión de Cargo.toml" "Red"
    exit 1
}

# 6. Solicitar nueva versión
Write-Host ""
Write-Color "Ingresá la nueva versión (formato: X.Y.Z, ej: 1.0.0):" "Yellow"
Write-Host "  Sugerencias SemVer:"
Write-Host "    - Incremento MAYOR (1.0.0 → 2.0.0): cambios incompatibles"
Write-Host "    - Incremento MENOR (1.0.0 → 1.1.0): nuevas características compatibles"
Write-Host "    - Incremento PATCH (1.0.0 → 1.0.1): correcciones de bugs"
Write-Host ""
Write-Color "Nueva versión: " "Yellow" -NoNewline
$newVersion = Read-Host

# Validar formato de versión
if ($newVersion -notmatch '^\d+\.\d+\.\d+$') {
    Write-Color "❌ Formato inválido. Debe ser X.Y.Z (ej: 1.0.0)" "Red"
    exit 1
}

# Verificar que el tag no exista
$tagExists = git tag -l "v$newVersion"
if ($tagExists) {
    Write-Color "❌ El tag v$newVersion ya existe." "Red"
    Write-Color "Tags existentes:" "Gray"
    git tag -l | Sort-Object -Descending | Select-Object -First 5
    exit 1
}

# 7. Mostrar resumen
Write-Title "📋 Resumen del Release"
Write-Host "  Rama actual:       $currentBranch"
Write-Host "  Versión actual:    v$currentVersion"
Write-Color "  Nueva versión:     v$newVersion" "Green"
Write-Host "  Tag a crear:       v$newVersion"
Write-Host ""
Write-Color "  GitHub Actions automáticamente:" "Cyan"
Write-Host "    ✓ Ejecutará tests y lints"
Write-Host "    ✓ Compilará el proyecto"
Write-Host "    ✓ Creará Naygo-portable.zip"
Write-Host "    ✓ Publicará la release en GitHub"
Write-Host ""

if (-not (Confirm-Action "¿Continuar con la creación del release?")) {
    Write-Color "Operación cancelada." "Gray"
    exit 0
}

# 8. Actualizar Cargo.toml
Write-Host ""
Write-Color "📝 Actualizando Cargo.toml..." "Cyan"
$cargoContent = $cargoContent -replace 'version\s*=\s*"[^"]+"', "version = `"$newVersion`""
Set-Content "Cargo.toml" -Value $cargoContent -NoNewline
Write-Color "✅ Cargo.toml actualizado" "Green"

# 9. Crear commit con la nueva versión
Write-Color "📝 Creando commit..." "Cyan"
git add Cargo.toml
git commit -m "chore: bump version to v$newVersion" --quiet
if ($LASTEXITCODE -ne 0) {
    Write-Color "❌ Error al crear el commit" "Red"
    exit 1
}
Write-Color "✅ Commit creado" "Green"

# 10. Crear el tag
Write-Color "🏷️  Creando tag v$newVersion..." "Cyan"
git tag -a "v$newVersion" -m "Release v$newVersion"
if ($LASTEXITCODE -ne 0) {
    Write-Color "❌ Error al crear el tag" "Red"
    Write-Color "Deshaciendo cambios..." "Yellow"
    git reset --hard HEAD~1
    exit 1
}
Write-Color "✅ Tag creado" "Green"

# 11. Confirmar push
Write-Title "🚀 Listo para Publicar"
Write-Host "Los siguientes cambios serán enviados al repositorio remoto:"
Write-Host ""
Write-Host "  1. Commit con la nueva versión en Cargo.toml"
Write-Host "  2. Tag v$newVersion (esto disparará el workflow de release)"
Write-Host ""
Write-Color "Una vez que hagas push del tag:" "Yellow"
Write-Host "  → GitHub Actions comenzará a compilar y publicar automáticamente"
Write-Host "  → Podés seguir el progreso en: Actions → Build and Release"
Write-Host "  → En ~5-10 minutos aparecerá en: Releases"
Write-Host ""

if (-not (Confirm-Action "¿Hacer push ahora?")) {
    Write-Host ""
    Write-Color "Push cancelado. Los cambios están en local." "Yellow"
    Write-Host ""
    Write-Host "Para publicar más tarde, ejecutá:"
    Write-Color "  git push && git push origin v$newVersion" "Cyan"
    Write-Host ""
    Write-Host "Para deshacer (si cambiás de idea):"
    Write-Color "  git reset --hard HEAD~1" "Gray"
    Write-Color "  git tag -d v$newVersion" "Gray"
    exit 0
}

# 12. Push del commit y tag
Write-Host ""
Write-Color "📤 Enviando cambios al repositorio..." "Cyan"

Write-Color "  → Pushing commit..." "Gray"
git push --quiet
if ($LASTEXITCODE -ne 0) {
    Write-Color "❌ Error al hacer push del commit" "Red"
    exit 1
}

Write-Color "  → Pushing tag..." "Gray"
git push origin "v$newVersion" --quiet
if ($LASTEXITCODE -ne 0) {
    Write-Color "❌ Error al hacer push del tag" "Red"
    Write-Color "El commit ya fue enviado. Intentá manualmente:" "Yellow"
    Write-Color "  git push origin v$newVersion" "Cyan"
    exit 1
}

# 13. Éxito
Write-Title "✅ Release Publicado"
Write-Color "El tag v$newVersion fue enviado exitosamente." "Green"
Write-Host ""
Write-Host "Próximos pasos:"
Write-Host ""
Write-Host "  1. Verificar el workflow en GitHub:"
$repoUrl = (git remote get-url origin) -replace '\.git$', '' -replace 'git@github\.com:', 'https://github.com/'
Write-Color "     $repoUrl/actions" "Cyan"
Write-Host ""
Write-Host "  2. Una vez completado (~5-10 min), verificar la release:"
Write-Color "     $repoUrl/releases/tag/v$newVersion" "Cyan"
Write-Host ""
Write-Host "  3. Link de descarga permanente (después de publicado):"
Write-Color "     $repoUrl/releases/latest/download/Naygo-portable.zip" "Cyan"
Write-Host ""
Write-Color "¡Listo! 🎉" "Green"
Write-Host ""
