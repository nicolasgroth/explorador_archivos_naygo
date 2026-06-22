# Footer por panel — Diseño

> Naygo (explorador de archivos Rust + Slint, render software). Autor: Nicolás Groth / ISGroth.
> Fecha: 2026-06-22.

## Objetivo

Cada panel de archivos muestra al pie una barra (footer) con información de ESE panel:
selección, bytes marcados y espacio de disco de su unidad. El formato es una **plantilla global
configurable** elegida entre predefinidas o una personalizada con tokens, manteniendo siempre
plantillas por defecto.

## Decisiones tomadas (con el usuario)

1. **Campos por defecto:** los 5 — seleccionados/total, bytes marcados, espacio libre del disco,
   espacio total + % usado, recuento de la carpeta. Todos togglables.
2. **Configuración:** enfoque híbrido (C) — combo de plantillas predefinidas + opción
   "Personalizada…" con editor de tokens. Vive en **Configuración → Avanzado**.
3. **Ámbito:** plantilla **global** (todos los paneles usan el mismo formato). Cada panel muestra
   SUS propios datos (su selección, su disco).

## Arquitectura (3 capas, respetando la regla de oro)

### Capa `core` — módulo nuevo `core::footer`

Lógica pura, sin UI ni Windows. 100% testeable.

```rust
/// Los 5 campos disponibles en el footer.
pub enum FooterField {
    Selected,    // "3 de 12 sel"
    MarkedBytes, // "4,2 MB"
    DiskFree,    // "112 GB libres"
    DiskTotal,   // "500 GB (77%)"
    ItemCount,   // "12 elementos (8 archivos, 4 carpetas)"
}

/// Plantilla elegida. Custom lleva el template de tokens del usuario.
pub enum FooterPreset {
    Compact,        // "{sel}/{total} · {marked}"
    Full,           // "{sel} de {total} sel · {marked} · {free} libres / {disk_total} ({pct})"
    DiskOnly,       // "{free} libres / {disk_total} ({pct})"
    SelectionOnly,  // "{sel} de {total} sel · {marked}"
    Custom(String), // template del usuario
}

/// Datos crudos que el UI calcula por panel y pasa a render().
pub struct FooterData {
    pub sel_count: usize,
    pub total_count: usize,
    pub marked_bytes: u64,
    pub disk: Option<naygo_core::disk::DiskUsage>, // None = disco no disponible (red caída, etc.)
    pub file_count: usize,
    pub dir_count: usize,
}

/// Sustituye tokens en el template y devuelve el string final.
pub fn render(preset: &FooterPreset, data: &FooterData, size_fmt: SizeFormat) -> String;
```

**Tokens** (para `Custom` y para definir los presets internamente):

| Token | Significado | Ejemplo |
|---|---|---|
| `{sel}` | nº seleccionados | `3` |
| `{total}` | nº total de filas visibles (tras filtro) | `12` |
| `{marked}` | bytes marcados (formateado) | `4,2 MB` |
| `{free}` | espacio libre | `112 GB` |
| `{disk_total}` | capacidad de la unidad | `500 GB` |
| `{pct}` | % usado del disco | `77%` |
| `{items}` | nº de elementos en la carpeta (sin filtrar) | `12` |
| `{files}` | nº de archivos | `8` |
| `{dirs}` | nº de carpetas | `4` |

**Reglas de sustitución (sin ambigüedad):**
- Token de disco (`{free}`, `{disk_total}`, `{pct}`) cuando `data.disk == None` → se reemplaza
  por `—`. (El disco puede no estar disponible: unidad de red caída, panel especial.)
- `{marked}` usa el mismo `SizeFormat` que la app (configurable, ya existe en `core::format`).
- Tokens desconocidos en un template Custom → se dejan literales (no se borra texto del usuario).
- `render` nunca falla (devuelve String siempre); el filesystem hostil no lo tumba.

**Presets** mapean a su template string internamente (constantes), así `render` es un solo camino
de código: preset → template → sustitución.

### Capa `config` — `Settings`

Tres campos nuevos (con `#[serde(default)]` para migrar settings.json viejos sin romper):

```rust
#[serde(default = "default_footer_enabled")] pub footer_enabled: bool,       // true
#[serde(default)]                            pub footer_preset: FooterPreset, // Compact (Default)
#[serde(default)]                            pub footer_custom_template: String, // ""
```

`FooterPreset` deriva `Default = Compact` y `Serialize/Deserialize`.

### Capa `ui-slint`

**Settings → Avanzado (`config-window.slint` + workspace_ctrl):** sección "Pie de panel":
- Casilla "Mostrar pie de panel" → `footer_enabled`.
- `ThemeCombo` (el propio; el ComboBox de std es invisible en tema claro) con: Compacta /
  Completa / Solo disco / Solo selección / Personalizada…
- Si "Personalizada…": aparece debajo un `LineEdit`/`TextEdit` con el template + lista de tokens
  al lado (referencia) + **línea de vista previa en vivo** con datos de ejemplo fijos
  (`FooterData` de muestra) renderizada por `core::footer::render`, para ver el resultado al teclear.

**Footer en el panel (`file-panel.slint`):** al pie del `VerticalLayout` del panel, condicional
`if footer.enabled`:
- Alto ~22px, fondo `Theme.row-alt-bg`, borde superior `Theme.border`, texto `Theme.text-dim` 12px.
- Una sola línea: el string ya renderizado (`pane.footer_text`). El UI NO formatea, solo pinta.
- `overflow: elide` si no cabe.
- Si el disco es crítico (`DiskUsage::is_critical()`), el footer se pinta en `Theme.error`
  (señal visual; reutiliza la lógica de la tira de discos del toolbar).

**Flujo de datos (campo nuevo `footer_text: string` en `PaneVm`):**
1. En `sync_rows` (donde ya se actualiza selección/path por tick), para cada panel Files:
   arma `FooterData` — cuenta `selected`, suma `marked_bytes` de los entries marcados, toma
   `file_count`/`dir_count` del listado, resuelve `DiskUsage` con `disk_usage(root)` (ya existe en
   `workspace_ctrl.rs:4208`).
2. Llama `core::footer::render(preset, &data, size_fmt)` → string.
3. Asigna a `pane.footer_text`.

**Caché de disco (rendimiento):** `DiskUsage` se resuelve por panel pero se **cachea** y solo se
refresca al cambiar la carpeta del panel o cada N segundos (mismo patrón que la tira de discos),
para no pegarle a WinAPI en cada pulsación de teclado. El conteo de selección/items es CPU puro
sobre memoria. **Cero I/O nuevo en el hilo de UI por pulsación.**

## i18n

Claves nuevas (triple: es/en + estructura), español neutral sin voseo:
- Etiquetas de Avanzado: "Pie de panel", "Mostrar pie de panel", "Plantilla", "Personalizada",
  nombres de presets, "Tokens disponibles", "Vista previa".
- Los nombres de preset en el combo.
- **No reutilizar nombres de claves existentes** (gotcha conocido: colisión de claves i18n).

## Testing

- `core::footer`: tests de `render` para cada preset, con disco presente y `None`, con selección
  0 y >0, tokens desconocidos, formato de bytes. (El grueso de la lógica vive aquí y es pura.)
- `config`: round-trip serde + migración (settings.json sin los campos → defaults).
- UI: el wiring se valida visualmente en la VM (no hay tests de render Slint).

## Errores / filesystem hostil

- Disco no disponible → `disk: None` → tokens de disco muestran `—`. El footer nunca tumba la app.
- Template Custom inválido o con tokens raros → se pinta lo que se pueda, literal para lo
  desconocido. Nunca panic.

## Fuera de alcance (YAGNI)

- Plantilla por panel (se decidió global).
- Campos extra (filtro activo, ruta) — descartados por redundantes con path-bar/header.
- Peso recursivo de la carpeta en el footer (costoso; ya existe ComputeSize bajo demanda con F3).
