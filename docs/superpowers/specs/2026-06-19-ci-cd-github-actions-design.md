# CI/CD con GitHub Actions — diseño

> Spec de diseño. Fecha: 2026-06-19. Autor: Nicolás Groth / ISGroth.

## Objetivo

Automatizar dos cosas en GitHub:

1. **Integración continua (CI):** correr el gate de calidad (formato + tests + clippy) en
   cada `push` y `pull_request` a `main`, para detectar regresiones temprano y dar un
   check verde/rojo automático en los aportes externos.
2. **Publicación de releases:** al crear un tag `v*.*.*`, compilar y publicar
   automáticamente el **portable** y el **instalador** en un GitHub Release, con un link
   de descarga estable que siempre apunta a la última versión.

Inspirado en el PR #1 (moisesnks), que se descarta por: duplicar `bump.ps1` con un script
nuevo que pide la versión a mano y no toca el CHANGELOG; publicar solo el portable (sin el
instalador); no usar `CARGO_BUILD_JOBS=2` (rompería el CI); y estar redactado en voseo.
Este diseño toma la idea pero la integra con lo que el repo ya tiene.

## Contexto del repo (verificado)

- `scripts/build-release.ps1` ya compila release, arma el **portable** (`Naygo-<ver>-portable.zip`)
  y, si encuentra `ISCC.exe`, el **instalador** Inno Setup (`Naygo-<ver>-setup.exe`). Lee la
  versión de `[workspace.package].version` del `Cargo.toml` raíz (fuente única).
- `scripts/bump.ps1` sube la versión automáticamente según los commits convencionales
  (feat→minor, fix→patch, BREAKING→major), mueve el CHANGELOG y crea el tag. NO pushea.
- Restricción dura: el proyecto compila con `CARGO_BUILD_JOBS=2` (las dependencias de
  SVG/PDF crashean el compilador de Slint con alta paralelización; los runners tienen varios
  cores).
- Convención: español neutral sin voseo en código y documentación. Headers de autoría MIT.
- El README ya tiene una sección "Instalación" + "Uso por línea de comandos" que NO debe
  reescribirse.

## Componentes

### 1. `.github/workflows/ci.yml`

Disparadores: `push` a `main` y `pull_request` a `main`.

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
env:
  CARGO_BUILD_JOBS: "2"
jobs:
  gate:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo test --workspace
      - run: cargo clippy --workspace --all-targets -- -D warnings
```

- Runner `windows-latest` (Naygo es Windows-only; `platform` usa Win32).
- NO compila release ni toca artefactos. Rápido y barato.
- `rust-cache` cachea las deps (Slint/SVG/PDF son lentas de compilar).

### 2. `.github/workflows/release.yml`

Disparador: `push` de tag `v*.*.*`.

```yaml
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
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      # Validar que el tag coincide con la versión del Cargo.toml (fuente única).
      - name: Validar versión vs tag
        shell: pwsh
        run: <ver Task del plan: extrae version de Cargo.toml, compara con github.ref_name sin la 'v', falla si difieren>
      - run: cargo test --workspace
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - name: Instalar Inno Setup
        run: choco install innosetup -y --no-progress
      - name: Empaquetar (portable + instalador)
        shell: pwsh
        run: powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
      - name: Copias con nombre estable
        shell: pwsh
        run: <copia dist\Naygo-<ver>-portable.zip -> Naygo-portable.zip y el setup -> Naygo-setup.exe>
      - name: Crear GitHub Release
        env:
          GH_TOKEN: ${{ github.token }}
        shell: pwsh
        run: <gh release create con los 4 archivos: versionados + estables>
```

- **Gate antes de publicar:** test + clippy; si fallan, no hay release a medias.
- **Inno Setup en el runner:** `choco install innosetup` (choco viene preinstalado en
  windows-latest). Así `build-release.ps1` encuentra `ISCC.exe` y genera el setup.
- **Versión = `Cargo.toml`** (no el nombre del tag), con validación de que coinciden. Si el
  tag es `v1.2.0`, el `Cargo.toml` debe decir `1.2.0`; si no, el workflow falla con mensaje
  claro ANTES de compilar.

### 3. `scripts/bump.ps1` (+ `-Push`)

Nuevo parámetro switch `-Push`. Sin él: comportamiento actual (commit + tag local, no
pushea). Con él: tras crear commit y tag, `git push` y `git push --tags`. La lógica de
auto-versionado y CHANGELOG queda intacta. Es lo único rescatable del `create-release.ps1`
del PR, integrado en el script que ya existe.

### 4. Link de descarga estable

El link `/releases/latest/download/<archivo>` requiere un **nombre de asset fijo**, pero
`build-release.ps1` genera nombres versionados. Solución: el release sube AMBOS:

- Versionados (histórico/referencia): `Naygo-<ver>-portable.zip`, `Naygo-<ver>-setup.exe`.
- Nombre fijo (link estable): `Naygo-portable.zip`, `Naygo-setup.exe` (copias).

Links estables resultantes:
```
https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip
https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-setup.exe
```

## Manejo de errores

- `$ErrorActionPreference = "Stop"` en pasos PowerShell (`build-release.ps1` ya lo usa).
- Gate falla → job falla antes de publicar.
- `build-release.ps1` falla (Inno no compiló, etc.) → paso falla, no se crea el release.
- Tag ≠ Cargo.toml → falla explícito antes de compilar.
- `gh release create` usa `${{ github.token }}` (token del runner, sin secretos extra).

## Pruebas

- Los workflows no son unit-testeables. Verificación:
  - `ci.yml`: debe correr verde en el PR que lo introduce.
  - `release.yml`: probar con un tag de prueba (p. ej. `v0.1.1-rc1`, NO `v0.1.0` que ya
    existe) y luego borrarlo. Esta prueba la hace Nicolás (requiere push a su repo).
- El gate del CI es el mismo que se corre localmente, así que el comportamiento es conocido.

## Documentación

- `docs/DISTRIBUTION.md`: sección nueva "Publicar una versión" en español neutral —
  `bump.ps1 -Push` + qué hace el workflow + el link estable.
- `README.md`: AGREGAR una línea con el link estable de descarga arriba de la sección de
  instalación existente. NO reescribir "Instalación" ni "Uso por línea de comandos".
- NO crear `SETUP_NOTES.md` (andamiaje del PR).
- Headers de autoría MIT en los `.yml` (comentario de cabecera).

## Archivos

- **Crear:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- **Modificar:** `scripts/bump.ps1` (+`-Push`), `README.md` (línea de link),
  `docs/DISTRIBUTION.md` (sección release)

## Fuera de alcance

- Firma de código del `.exe` (recomendación aparte; SmartScreen sigue avisando).
- Releases multi-plataforma (Naygo es Windows-only).
- Publicar en winget/choco (futuro, si el proyecto crece).
- Proteger la rama `main` (ya documentado en `docs/PROTEGER-RAMA-MAIN.md`; el CI es su
  prerrequisito natural — los checks requeridos serían los de `ci.yml`).
