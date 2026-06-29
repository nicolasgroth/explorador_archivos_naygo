# Comprimir / descomprimir .zip — diseño

> Naygo — explorador de archivos. Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
> SPDX-License-Identifier: MIT
> Fase mayor reservada como futura, ahora en marcha. Rama `feat/comprimir-descomprimir`.

## Motivación

El preview de comprimidos ya lee el ÍNDICE de un `.zip`/`.tar`/`.tar.gz` (lista en árbol, sin
extraer). Falta lo principal: **crear** y **extraer** archivos comprimidos de verdad. Es una de
las operaciones que un explorador estilo Commander debe hacer bien.

## Alcance (1ª entrega)

- **Solo `.zip`**, crear y extraer. Es universal en Windows, puro-Rust (el crate `zip` ya está
  en `core` y `ui-slint`), sin libs nativas. Cubre el 95% de los casos.
- **Crear**: «Comprimir en .zip…» sobre la selección → pide nombre (default sensato) → crea el
  `.zip` en la carpeta actual.
- **Extraer**: «Extraer aquí» (subcarpeta con el nombre del zip, en la carpeta actual) y
  «Extraer en…» (elegir carpeta con diálogo nativo).
- **Integración total con el sistema de ops**: worker async, panel de progreso (barra, %,
  velocidad, ETA, cancelar) — el MISMO que copiar/mover. Conflictos con el diálogo existente.
- **Deshacible** (Ctrl+Z): comprimir → el `.zip` a papelera; extraer → lo extraído a papelera.
- **i18n** en los 10 idiomas.

### No-objetivos (YAGNI)

- Otros formatos para CREAR (.7z/.rar/.tar.gz). `.7z`/`.rar` no tienen impl puro-Rust libre.
- Niveles de compresión configurables, contraseñas, comprimir-y-enviar.
- «Extraer al otro panel» (posible extra menor futuro, muy estilo Commander).

## Arquitectura (3 capas)

### `core` — lógica pura y testeable

Módulo nuevo `crates/core/src/archive_ops.rs` (el crate ya depende de `zip`). Dos funciones
puras, sin tocar la UI ni Windows, cada una con `CancellationToken` y callback de progreso:

```rust
/// Resultado de una entrada procesada (para el resumen y el deshacer).
pub struct ArchiveOpItem {
    pub path: PathBuf,        // ruta REAL escrita (el .zip al comprimir; el archivo extraído al extraer)
    pub outcome: ArchiveOutcome, // Done / Skipped / Failed(String)
}

pub enum ArchiveOutcome { Done, Skipped, Failed(String) }

/// Decisión ante un conflicto al extraer (un archivo destino ya existe).
pub enum ExtractConflict { Overwrite, Skip, KeepBoth, Cancel }

/// Comprime `sources` (archivos y/o carpetas, recursivo) en `dest_zip`.
/// - Progreso por BYTES (suma de tamaños de los sources como total).
/// - Cancelar → aborta y BORRA el `.zip` parcial (no deja basura).
/// - Un source ilegible a mitad → se registra Failed y la op CONTINÚA con el resto.
/// Devuelve los items procesados (para el resumen y el deshacer).
pub fn compress_zip(
    sources: &[PathBuf],
    dest_zip: &Path,
    on_progress: &mut dyn FnMut(u64 /*bytes_done*/, u64 /*bytes_total*/),
    token: &CancellationToken,
) -> Result<Vec<ArchiveOpItem>, ArchiveError>;

/// Extrae `zip` dentro de `dest_dir`.
/// - PROTECCIÓN ZIP-SLIP: entradas con `..` o rutas absolutas que escapen de `dest_dir` se
///   RECHAZAN (Skipped) — nunca se escribe fuera del destino. (Mismo patrón que el import de
///   .naygoset.)
/// - Progreso por BYTES (suma de tamaños descomprimidos del índice).
/// - Conflicto (archivo destino existe) → `on_conflict` decide (Overwrite/Skip/KeepBoth/Cancel).
/// - Entrada corrupta/ilegible → Failed, la op CONTINÚA.
/// - Cancelar → aborta; lo YA extraído PERMANECE (extraer no es destructivo; borrar a ciegas
///   archivos mezclados con preexistentes sería riesgoso).
/// Devuelve los items extraídos (rutas creadas → para el deshacer: solo lo que la op creó).
pub fn extract_zip(
    zip: &Path,
    dest_dir: &Path,
    on_conflict: &mut dyn FnMut(&Path) -> ExtractConflict,
    on_progress: &mut dyn FnMut(u64, u64),
    token: &CancellationToken,
) -> Result<Vec<ArchiveOpItem>, ArchiveError>;
```

`ArchiveError` es un enum tipado (`Io`, `Zip`, `NoSpace`, `Cancelled`, …). Ningún `panic`.

**Default del nombre al comprimir** (función pura `default_zip_name(sources) -> String`):
- 1 archivo o 1 carpeta → su nombre + `.zip` (`informe.txt` → `informe.zip`; carpeta `proyecto`
  → `proyecto.zip`).
- varios ítems → `archivos.zip`.

### `ui-slint` — worker async + cableado (enfoque (b))

Worker PROPIO de zip (no pasa por el `plan/exec_step` del engine de copiar — ese asume
«paso = archivo» y forzar zip ahí lo enturbiaría). El worker:
- Corre en un hilo async; el hilo de UI NUNCA bloquea.
- Reusa el MISMO canal/panel de progreso que las ops (emite los `OpMsg::Progress` equivalentes o
  un canal espejo que el panel ya sabe pintar) y el MISMO `CancellationToken`.
- Para el conflicto al extraer, reusa el `ConflictPrompt`/diálogo lado a lado existente.
- Al terminar, registra UNA entrada en el historial de deshacer con las rutas creadas.

Handlers nuevos del menú contextual (en `ui-slint`, capa de cableado):
- «Comprimir en .zip…» (sobre la selección) → modal de nombre (reusa el patrón de «nueva
  carpeta»/«pegar como») con el default → lanza el worker de compresión.
- «Extraer aquí» (visible solo si la selección es 1 `.zip`) → subcarpeta `<nombre-zip>/` en la
  carpeta actual → lanza el worker de extracción.
- «Extraer en…» → diálogo nativo de carpeta → lanza el worker.

### `platform`

No se toca: `zip` es puro-Rust. El diálogo «elegir carpeta» usa el FileDialog nativo que ya existe.

## Componentes y responsabilidades

| Unidad | Responsabilidad | Depende de |
|--------|-----------------|------------|
| `core::archive_ops::compress_zip` | comprimir sources → .zip, progreso, cancelación | zip, cancel |
| `core::archive_ops::extract_zip` | extraer .zip → dir, zip-slip, conflicto, progreso | zip, cancel |
| `core::archive_ops::default_zip_name` | nombre por defecto del .zip | — |
| `ui::<worker de zip>` | correr compress/extract async, emitir progreso, cancelar | archive_ops, panel ops |
| `ui::<handlers ctx menu>` | armar la op desde el menú + modal de nombre / diálogo de carpeta | worker |
| deshacer (historial) | comprimir/extraer → a papelera lo creado | rutas registradas |

## Errores (filesystem hostil)

**Comprimir**: cancelar borra el parcial; source ilegible → Failed + continúa; nombre destino ya
existe → conflicto previo (sobrescribir/renombrar/cancelar); sin espacio → error claro + borra
parcial.

**Extraer**: zip-slip → entrada rechazada (Skipped), nunca escribe fuera del destino; conflicto →
diálogo existente; entrada corrupta → Failed + continúa; cancelar → lo extraído PERMANECE.

Transversal: todo I/O en worker async, `Result` tipado, sin `panic`.

## Deshacer

Comprimir/extraer registran las rutas que la op CREÓ (provenance explícita, como el undo de Move
en I-3). Deshacer las manda a PAPELERA (nunca borrado permanente), una entrada en el historial:
- Comprimir → el `.zip` creado a papelera.
- Extraer → SOLO los archivos/carpetas que la extracción escribió (no lo preexistente) a papelera.

## Testing (core puro, tempdirs)

- Round-trip: comprimir un árbol → extraer → comparar bytes idénticos.
- Carpetas anidadas, archivos vacíos, nombres unicode.
- **Zip-slip**: `.zip` con entrada `../escape.txt` → rechazada, nada fuera del destino.
- Cancelación: token cancelado a media compresión → `.zip` parcial borrado; a media extracción →
  lo extraído permanece.
- Conflicto: extraer sobre archivo existente → `on_conflict` se invoca; cada decisión
  (Overwrite/Skip/KeepBoth) hace lo correcto.
- `default_zip_name`: 1 archivo → su nombre; 1 carpeta → su nombre; varios → `archivos.zip`.
- Deshacer: las rutas creadas se registran; comprimir/extraer → van a papelera (lo creado, no más).
- Integración liviana del worker (crear un .zip real, leerlo, extraerlo en tempdir).

## Trade-offs decididos

- **Solo .zip**: universal, puro-Rust, simple. tar.gz crear y 7z/rar quedan fuera (raro en
  Windows / no hay impl libre).
- **Worker propio (b), no `exec_step` (a)**: aísla la lógica de zip del modelo «paso = archivo»
  del engine de copiar; reusa panel/canal/cancelación sin forzar el encaje.
- **Cancelar extracción deja lo extraído**: más seguro que borrar a ciegas archivos que pueden
  mezclarse con preexistentes.
- **Deshacer a papelera**: consistente con copiar/crear (nunca borrado permanente).

## Fuera de alcance / fases futuras

- Otros formatos para crear, niveles/contraseñas, «extraer al otro panel», comprimir-y-enviar.
