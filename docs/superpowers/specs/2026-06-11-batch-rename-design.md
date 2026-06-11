# Batch-rename avanzado (R3) — Diseño

> Fase R3 del plan rename+undo. Requisitos explícitos de Nicolás: vista previa en
> vivo + comodines (fecha del archivo: día/mes/año/horas/minutos/segundos, contador,
> incluir o no la extensión) + ideas adicionales de UX aprobadas en bloque.

## Disparo

- **F2 con 2+ ítems seleccionados** en el panel activo → abre el diálogo de
  batch-rename (con 1 solo ítem, F2 sigue siendo el rename inline de R1).
- Menú contextual «Renombrar» con multi-selección → ídem.

## Modelo (core, puro — `crates/core/src/batch_rename.rs`)

```rust
pub enum CaseTransform { None, Lower, Upper, Title }

pub struct BatchSpec {
    pub template: String,     // patrón con comodines
    pub include_ext: bool,    // false (default): el patrón transforma solo el STEM
                              //   y la extensión original se conserva tal cual;
                              // true: el patrón produce el nombre COMPLETO ({ext} disponible)
    pub find: String,         // buscar (vacío = paso desactivado)
    pub replace: String,
    pub use_regex: bool,      // find/replace como regex (crate `regex`, MIT/Apache)
    pub case: CaseTransform,
    pub counter_start: i64,   // contador {n}: inicio…
    pub counter_step: i64,    // …y paso
}

pub struct BatchItem { pub path: PathBuf, pub modified_epoch_secs: Option<u64> }

pub enum RowStatus { Ok, Unchanged, Invalid(String), Collision }
pub struct PreviewRow { pub path: PathBuf, pub old_name: String,
                        pub new_name: String, pub status: RowStatus }

/// `existing_names`: nombres actuales del directorio (para colisiones).
/// `utc_offset_secs`: offset local (lo provee platform; core queda puro y testeable).
pub fn preview(items: &[BatchItem], spec: &BatchSpec,
               existing_names: &[String], utc_offset_secs: i64) -> Vec<PreviewRow>;
pub fn can_apply(rows: &[PreviewRow]) -> bool; // sin Invalid/Collision y ≥1 cambio real
```

### Comodines (alias ES y EN; desconocido → queda literal, como `{fecha}` del pegado)

| Token | Expande a |
|---|---|
| `{nombre}` / `{name}` | nombre original sin extensión |
| `{ext}` | extensión original sin punto (solo tiene sentido con `include_ext`) |
| `{n}`, `{n:K}` | contador (inicio/paso del spec); `:K` = padding a K dígitos (`{n:3}` → 001) |
| `{dia}` / `{day}` | día (2 dígitos) de la fecha de modificación del archivo |
| `{mes}` / `{month}` | mes (2 dígitos) |
| `{año}` / `{anio}` / `{year}` | año (4 dígitos) |
| `{hora}` / `{hour}` | hora local (2 dígitos) |
| `{min}` | minutos (2 dígitos) |
| `{seg}` / `{sec}` | segundos (2 dígitos) |

Fechas: epoch del archivo + `utc_offset_secs`, convertido con el `civil_from_epoch`
(Hinnant) ya existente en `clipboard/naming.rs` (se extiende a segundos y se comparte).
Sin fecha de modificación → esos tokens expanden a "" (fila sigue válida).

### Pipeline por archivo (en este orden)

1. Expansión del patrón (sobre stem o nombre completo según `include_ext`).
2. Buscar/reemplazar (texto plano, o regex con grupos `$1`; regex inválida → TODAS
   las filas `Invalid` con el error, Aplicar bloqueado).
3. Transformación de mayúsculas (`Title` = primera letra de cada palabra).
4. Si `!include_ext`, re-adjuntar la extensión original.

### Validación y colisiones (en el preview, en vivo)

- `is_valid_name` (reutilizado de ops) → `Invalid`.
- Nombre nuevo == viejo → `Unchanged` (fila atenuada; no cuenta como cambio).
- Duplicados dentro del propio batch (case-insensitive, Windows) → `Collision`.
- Choque con `existing_names` → `Collision`, SALVO que ese nombre lo libere otro
  ítem del mismo batch (shift foto1→foto2, foto2→foto3 es válido). Los ciclos
  (swap a↔b) quedan como `Collision` en v1.
- `Aplicar` se habilita solo con `can_apply`.

## Ejecución como UNA operación deshacible (ops)

- Nuevo `OpKind::BatchRename { new_names: Vec<String> }`, paralelo a `sources`.
- `plan()`: valida cada nombre; un paso `from → parent.join(new_name)` por ítem,
  **ordenados por dependencia** (un paso cuyo destino está ocupado por un source
  aún no procesado va después; sin progreso = ciclo → `PlanError::InvalidName`).
- `engine`: el brazo `BatchRename` reusa `exec_rename` por paso (igual que Rename).
- `undo::build_undo`: brazo nuevo que empareja sources↔Done **en orden inverso de
  ejecución** y emite `MoveBack` por par → deshacer un shift funciona. El resto
  (validate/to_requests/Historial/Ctrl+Z) cae solo por R2: UNA entrada en el journal.
- Etiqueta de la op: `op.batch_rename` («Renombrar en lote»).

## UI (`crates/ui/src/batch_rename_dialog.rs`)

Ventana centrada (patrón de diálogos existente): campos plantilla / incluir
extensión / buscar / reemplazar / regex / mayúsculas (combo) / contador inicio y
paso; ayuda de comodines plegable; **preview en vivo** (tabla Antes → Después,
rojo = colisión o inválido con motivo en tooltip, atenuado = sin cambio);
Aplicar (deshabilitado si no `can_apply`) / Cancelar (Esc). El preview se
recalcula al editar cualquier campo (puro, en memoria; barato hasta miles de filas).

`platform`: `local_utc_offset_secs()` (GetTimeZoneInformation) — la UI lo pasa a core.

## Tests (core)

Tokens (ES/EN, padding, fecha conocida, sin fecha), pipeline (orden patrón→
find/replace→case→ext), include_ext on/off, regex con grupos y regex inválida,
colisiones (duplicado interno, contra existentes, shift permitido, swap bloqueado),
can_apply, plan de BatchRename (orden por dependencia, ciclo = error), build_undo
del batch (orden inverso). UI manual en vivo.

## Fuera de alcance v1

Resolución de swaps con nombre temporal; perfiles/presets guardados; comodines de
metadatos EXIF; aplicar a resultados de filtro recursivo.
