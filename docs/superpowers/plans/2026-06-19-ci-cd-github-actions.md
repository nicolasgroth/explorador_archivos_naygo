# CI/CD con GitHub Actions — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dos workflows de GitHub Actions: CI (gate en push/PR a main) y Release (compilar + publicar portable e instalador en un tag v*.*.*), con link de descarga estable.

**Architecture:** Workflows YAML en `.github/workflows/`. Ambos corren en `windows-latest` con `CARGO_BUILD_JOBS=2` (limitación de Slint). El de release reusa `scripts/build-release.ps1` (ya genera portable + instalador) e instala Inno Setup en el runner. `scripts/bump.ps1` ya tiene `-Push` (no se toca). Docs en español neutral.

**Tech Stack:** GitHub Actions, PowerShell, `gh` CLI (preinstalado en runners), chocolatey (preinstalado), `dtolnay/rust-toolchain`, `Swatinem/rust-cache`.

> **Nota sin gate de cargo:** este trabajo NO toca código Rust, así que no hay `cargo test` nuevo. La verificación de cada YAML es: parsearlo (PowerShell + `ConvertFrom-Yaml` si está, o validación estructural) y revisión humana. La prueba real de los workflows la hace Nicolás en GitHub (push). El plan lo documenta.

---

### Task 1: Workflow de CI (gate en push/PR a main)

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Crear el archivo del workflow de CI**

Crear `.github/workflows/ci.yml` con este contenido exacto:

```yaml
# Naygo — integración continua: corre el gate (formato + tests + clippy) en cada push y
# pull request a main. No compila release ni publica nada.
# Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

# Naygo compila con CARGO_BUILD_JOBS=2: las dependencias de SVG/PDF crashean el compilador
# de Slint con alta paralelización (los runners tienen varios cores).
env:
  CARGO_BUILD_JOBS: "2"

jobs:
  gate:
    runs-on: windows-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache de dependencias
        uses: Swatinem/rust-cache@v2

      - name: Formato
        run: cargo fmt --all -- --check

      - name: Tests
        run: cargo test --workspace

      - name: Clippy (warnings = error)
        run: cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 2: Validar que el YAML es sintácticamente correcto**

Run (PowerShell):
```
powershell -Command "if (Get-Command ConvertFrom-Yaml -ErrorAction SilentlyContinue) { Get-Content .github/workflows/ci.yml -Raw | ConvertFrom-Yaml | Out-Null; 'YAML OK' } else { 'ConvertFrom-Yaml no disponible: validar por revisión' }"
```
Expected: "YAML OK" (o el aviso de que no hay parser → revisar a ojo que la indentación de 2 espacios sea consistente y no haya tabs).

- [ ] **Step 3: Revisar que no haya voseo en los comentarios**

Run:
```
powershell -Command "Select-String -Path .github/workflows/ci.yml -Pattern 'avisás|decís|querés|asegurate|hacé|tené|elegí|fijate|mirá|podés|tenés|commiteá|ejecutá|descargá' -ErrorAction SilentlyContinue"
```
Expected: sin coincidencias.

- [ ] **Step 4: Commit**

```
git add .github/workflows/ci.yml
git commit -m "ci: workflow de integracion continua (gate en push/PR a main)"
```

---

### Task 2: Workflow de Release (compilar + publicar en tag)

**Files:**
- Create: `.github/workflows/release.yml`

**Contexto:** `scripts/build-release.ps1` lee la versión de `Cargo.toml`, compila release, y genera `dist/Naygo-<ver>-portable.zip` + (si hay `ISCC.exe`) `dist/Naygo-<ver>-setup.exe`. El workflow instala Inno Setup para que el setup también se genere, valida que el tag coincida con la versión del Cargo.toml, y sube los 4 archivos (2 versionados + 2 con nombre estable) al release.

- [ ] **Step 1: Crear el archivo del workflow de release**

Crear `.github/workflows/release.yml` con este contenido exacto:

```yaml
# Naygo — publicación de releases: al crear un tag vX.Y.Z compila el portable y el
# instalador y los publica en un GitHub Release, con un link de descarga estable.
# Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
name: Release

on:
  push:
    tags: ["v*.*.*"]

permissions:
  contents: write

env:
  CARGO_BUILD_JOBS: "2"

jobs:
  release:
    runs-on: windows-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy

      - name: Cache de dependencias
        uses: Swatinem/rust-cache@v2

      # La versión es fuente única en Cargo.toml. El tag debe coincidir (vX.Y.Z == X.Y.Z),
      # si no, abortamos antes de compilar para no publicar un release inconsistente.
      - name: Validar tag contra Cargo.toml
        shell: pwsh
        run: |
          $cargo = Get-Content Cargo.toml -Raw
          if ($cargo -notmatch '(?m)^\s*version\s*=\s*"([^"]+)"') {
            Write-Error "No pude leer la version de Cargo.toml."; exit 1
          }
          $cargoVer = $Matches[1]
          $tag = "${{ github.ref_name }}"
          $tagVer = $tag -replace '^v', ''
          Write-Host "Tag: $tag  ->  $tagVer"
          Write-Host "Cargo.toml: $cargoVer"
          if ($tagVer -ne $cargoVer) {
            Write-Error "El tag ($tagVer) no coincide con la version de Cargo.toml ($cargoVer)."
            exit 1
          }
          Write-Host "Version validada: $cargoVer"

      - name: Tests
        run: cargo test --workspace

      - name: Clippy (warnings = error)
        run: cargo clippy --workspace --all-targets -- -D warnings

      - name: Instalar Inno Setup
        run: choco install innosetup -y --no-progress

      - name: Empaquetar (portable + instalador)
        shell: pwsh
        run: powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1

      # build-release.ps1 genera nombres versionados; el link estable necesita un nombre
      # fijo, así que subimos también una copia sin versión.
      - name: Copias con nombre estable
        shell: pwsh
        run: |
          $ver = (Select-String -Path Cargo.toml -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
          Copy-Item "dist\Naygo-$ver-portable.zip" "dist\Naygo-portable.zip" -Force
          if (Test-Path "dist\Naygo-$ver-setup.exe") {
            Copy-Item "dist\Naygo-$ver-setup.exe" "dist\Naygo-setup.exe" -Force
          }
          echo "VER=$ver" >> $env:GITHUB_ENV

      - name: Crear GitHub Release
        env:
          GH_TOKEN: ${{ github.token }}
        shell: pwsh
        run: |
          $notes = "Build automático de Naygo $env:VER.`n`nDescarga el ZIP portable (o el instalador), descomprime y ejecuta naygo.exe. Si ya tienes Naygo instalado, el instalador actualiza sin perder tu configuración.`n`nEn la primera ejecución, Windows SmartScreen puede advertir ""editor desconocido"" (el ejecutable no está firmado): haz clic en ""Más información"" y luego ""Ejecutar de todos modos""."
          gh release create "${{ github.ref_name }}" `
            "dist\Naygo-$env:VER-portable.zip" `
            "dist\Naygo-$env:VER-setup.exe" `
            "dist\Naygo-portable.zip" `
            "dist\Naygo-setup.exe" `
            --title "Naygo $env:VER" `
            --notes "$notes"
```

- [ ] **Step 2: Validar el YAML**

Run:
```
powershell -Command "if (Get-Command ConvertFrom-Yaml -ErrorAction SilentlyContinue) { Get-Content .github/workflows/release.yml -Raw | ConvertFrom-Yaml | Out-Null; 'YAML OK' } else { 'ConvertFrom-Yaml no disponible: validar por revisión' }"
```
Expected: "YAML OK" o aviso de revisión manual. Si falla el parser, revisar indentación (2 espacios, sin tabs) y que los bloques `run: |` estén bien anidados.

- [ ] **Step 3: Revisar voseo**

Run:
```
powershell -Command "Select-String -Path .github/workflows/release.yml -Pattern 'avisás|decís|querés|asegurate|hacé|tené|elegí|fijate|mirá|podés|tenés|commiteá|ejecutá|descargá' -ErrorAction SilentlyContinue"
```
Expected: sin coincidencias.

- [ ] **Step 4: Commit**

```
git add .github/workflows/release.yml
git commit -m "ci: workflow de release (portable + instalador en tag vX.Y.Z)"
```

---

### Task 3: Link de descarga estable en el README

**Files:**
- Modify: `README.md` (insertar antes de `## Instalación`, ~línea 56)

**Contexto:** NO reescribir la sección "Instalación" ni "Uso por línea de comandos". Solo agregar una línea destacada con el link estable, arriba de la sección de instalación.

- [ ] **Step 1: Insertar la línea del link estable**

En `README.md`, localizar la línea `## Instalación` (~56). Insertar JUSTO ANTES de ella:

```markdown
## Descargar

**[⬇ Descargar Naygo (portable)](https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip)** — siempre apunta a la última versión publicada. También está el [instalador](https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-setup.exe).

```

(Queda: `## Descargar` con los dos links → luego la sección `## Instalación` existente, intacta, que explica portable vs instalador y el aviso de SmartScreen.)

- [ ] **Step 2: Verificar que la sección "Instalación" y "Uso por línea de comandos" siguen intactas**

Run:
```
powershell -Command "Select-String -Path README.md -Pattern '## Instalación|## Uso por línea de comandos|## Descargar'"
```
Expected: las 3 secciones presentes (Descargar nueva + las 2 que ya estaban).

- [ ] **Step 3: Commit**

```
git add README.md
git commit -m "docs: link de descarga estable en el README"
```

---

### Task 4: Documentar el proceso de release

**Files:**
- Modify: `docs/DISTRIBUTION.md` (agregar sección al final)

- [ ] **Step 1: Agregar la sección "Publicar una versión"**

Al final de `docs/DISTRIBUTION.md`, agregar:

```markdown

---

## Publicar una versión (para mantenedores)

Naygo publica los releases con **GitHub Actions**: al crear un tag `vX.Y.Z`, un workflow
compila el portable y el instalador y los adjunta a un GitHub Release.

### Paso 1: subir la versión y el tag

Usa el script de versionado, que infiere el nivel (patch/minor/major) de los commits
convencionales desde el último tag, mueve el CHANGELOG y crea el commit y el tag:

```powershell
# Local (no publica): crea commit + tag
powershell -ExecutionPolicy Bypass -File scripts\bump.ps1

# Publicar directamente (crea commit + tag y hace push):
powershell -ExecutionPolicy Bypass -File scripts\bump.ps1 -Push
```

Opciones útiles: `-Level minor` fuerza el nivel; `-DryRun` muestra qué haría sin tocar nada.

Si corres `bump.ps1` sin `-Push`, publica el tag a mano cuando estés listo:

```powershell
git push ; git push --tags
```

### Paso 2: el workflow hace el resto

Al llegar el tag a GitHub, el workflow `Release`:

1. Valida que el tag coincida con la versión de `Cargo.toml`.
2. Corre los tests y clippy.
3. Instala Inno Setup y ejecuta `scripts\build-release.ps1` (portable + instalador).
4. Crea el GitHub Release y adjunta los cuatro archivos: los versionados
   (`Naygo-X.Y.Z-portable.zip`, `Naygo-X.Y.Z-setup.exe`) y las copias con nombre estable
   (`Naygo-portable.zip`, `Naygo-setup.exe`).

Puedes seguir el progreso en la pestaña **Actions** del repositorio.

### Link de descarga estable

Las copias con nombre fijo permiten un link que siempre apunta a la última versión:

```
https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip
https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-setup.exe
```

### Compilar sin publicar

Para generar los artefactos en `dist\` sin crear un release:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

### Integración continua

Aparte del release, el workflow `CI` corre el gate (formato + tests + clippy) en cada push
y pull request a `main`. Es el conjunto de checks natural para exigir en la protección de la
rama `main` (ver `docs/PROTEGER-RAMA-MAIN.md`).
```

- [ ] **Step 2: Revisar voseo en lo agregado**

Run:
```
powershell -Command "Select-String -Path docs/DISTRIBUTION.md -Pattern 'avisás|decís|querés|asegurate|hacé|tené|elegí|fijate|mirá|podés|tenés|commiteá|ejecutá|descargá'"
```
Expected: sin coincidencias.

- [ ] **Step 3: Commit**

```
git add docs/DISTRIBUTION.md
git commit -m "docs: documentar la publicacion de versiones con GitHub Actions"
```

---

### Task 5: Verificación final

**Files:** (ninguno — solo revisión)

- [ ] **Step 1: Revisar que los archivos creados existen y nada de cargo se rompió**

Run:
```
powershell -Command "Test-Path .github/workflows/ci.yml, .github/workflows/release.yml"
```
Expected: `True` `True`.

- [ ] **Step 2: Confirmar que bump.ps1 NO se modificó**

Run:
```
git -C . diff --stat HEAD~5 -- scripts/bump.ps1
```
Expected: sin cambios en `scripts/bump.ps1` (este plan no lo toca; ya tenía `-Push`).

- [ ] **Step 3: Actualizar el grafo (toca archivos del repo)**

Run:
```
graphify update .
```
Expected: actualiza sin error (los `.yml` y `.md` no aportan nodos de código, pero mantiene el grafo coherente).

- [ ] **Step 4: Commit (si graphify dejó cambios versionables) — normalmente no**

graphify-out/ está en .gitignore, así que no hay nada que commitear. Saltar si `git status` está limpio salvo CLAUDE.md.

---

## Verificación de Nicolás (en GitHub, no automatizable)

1. **CI:** tras el push de estos commits a `main`, ir a **Actions** → el workflow `CI` debe
   correr verde. Cualquier PR futuro mostrará el check automáticamente.
2. **Release:** probar con un tag de prueba que NO sea `v0.1.0` (ya existe). Por ejemplo:
   ```
   git tag v0.1.1-rc1 ; git push origin v0.1.1-rc1
   ```
   Ojo: el tag de prueba `v0.1.1-rc1` NO coincide con la versión `0.1.0` del Cargo.toml, así
   que el workflow fallará en el paso de validación (es lo esperado para un -rc de prueba). Para
   una prueba REAL de punta a punta, usar `bump.ps1` (que sube Cargo.toml + tag juntos) con una
   versión nueva, o ajustar el Cargo.toml a `0.1.1` antes de taggear `v0.1.1`.
   Tras la prueba, borrar el tag y el release de prueba:
   ```
   gh release delete v0.1.1-rc1 -y ; git push origin :refs/tags/v0.1.1-rc1 ; git tag -d v0.1.1-rc1
   ```
3. **Link estable:** tras un release real, verificar que
   `https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip`
   descarga el ZIP.
