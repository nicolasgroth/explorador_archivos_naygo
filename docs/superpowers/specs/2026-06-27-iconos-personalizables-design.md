# Íconos personalizables — diseño

> Naygo — explorador de archivos. Copyright (c) 2026 Nicolás Groth / ISGroth. MIT.
> Spec del rediseño del sistema de íconos del toolbar: sets de fábrica con
> personalidad real, personalización por-objeto (mezclando sets + PNG propios) e
> import/export de sets en un archivo `.naygoset`.

## Motivación

Hoy hay 3 sets de íconos (`flat`, `fluent`, `mono`) y un combobox global en
Configuración para elegir uno. El problema reportado por el usuario: **al cambiar
de set no se nota la diferencia**.

Diagnóstico verificado (íconos a 24px, tamaño real de toolbar):

- `flat` y `fluent` salieron de fuentes parecidas y son **casi indistinguibles**.
- `mono` sí difiere pero es un gris plano apagado.
- En `assets/icons/otros/` hay íconos de línea (tipo Tabler/Lucide) de mucha mejor
  calidad, pero el set está incompleto (14 de 33).
- Hay 6 librerías descargadas y **sin explotar** en `assets/icons/*.zip`: Lucide,
  Tabler, Material Design, VSCode-icons, Flat Color Icons, Fluent Emoji.

Causa de fondo: los PNG están **pre-coloreados** y no se tiñen al tema, y dos de
los tres sets son visualmente equivalentes.

## Objetivos

1. Reemplazar los 3 sets actuales por **5 sets de fábrica con personalidad
   radicalmente distinta**, generados desde las librerías reales.
2. Hacer que los sets tintables se **tiñan al color del tema** (máscara alfa), para
   que cambiar de set *y* de tema se note.
3. Una **sección nueva de personalización** (pestaña "Íconos" en Configuración)
   donde el usuario elige qué ícono usa cada objeto, mezclando sets y pudiendo
   aportar PNG propios.
4. **Import/export** de sets personalizados en un archivo autocontenido
   `.naygoset` (zip), para compartir entre equipos con un clic.

### No-objetivos (YAGNI)

- No es un editor de SVG ni un dibujador de íconos.
- No se incluye VSCode-icons como set de toolbar (solo trae íconos de *tipos de
  archivo*, no acciones). Puede alimentar los íconos de archivos en una fase
  futura, fuera de este spec.
- No se incluye Fluent Emoji (emojis 3D, demasiado cargados/pesados para una barra).
- No hay edición por-tema de overrides (los overrides son globales, no por tema).

## Sets de fábrica

Cinco sets embebidos, cada uno cubre las 33 `IconKey`:

| ID           | Familia            | Estilo                       | Tintable | Licencia    |
|--------------|--------------------|------------------------------|----------|-------------|
| `lucide`     | Lucide             | línea fina 2px (default)     | sí       | ISC         |
| `tabler`     | Tabler             | línea, esquinas redondeadas  | sí       | MIT         |
| `material`   | Material Design    | relleno macizo (filled)      | sí       | Apache-2.0  |
| `flat-color` | Flat Color Icons   | color plano multicolor       | **no**   | MIT/GoodWare|
| `mono`       | Lucide teñido gris | monocromo de un tono         | sí       | ISC         |

`lucide` es el set por defecto. Todas las licencias son permisivas y compatibles
con `THIRD-PARTY-NOTICES.md`; el generador copia cada licencia al notices.

## Arquitectura

Sigue las 3 capas del proyecto. El grueso de la lógica vive en `core` (testeable
sin UI ni Windows).

### Capa `core`

**De enum a IDs (data-driven).** Se elimina el acoplamiento del `enum IconSet
{ Flat, Fluent, Mono }` a 3 `include_bytes!` fijos. El set activo ya es un
`String` (`Settings.icon_set`) y `IconSetCatalog` ya maneja IDs; se completa la
transición quitando el enum como fuente de verdad.

**Conceptos:**

- **Set base** — familia completa identificada por ID. Sets de fábrica embebidos +
  packs sueltos del usuario en `<config_dir>/icons/<id>/` (ya existe).
- **`tintable`** — propiedad por set (en su `manifest.json`). Los tintables se
  pintan con el color del tema; `flat-color` trae su color y no se tiñe.
- **`IconSource`** — fuente de un ícono para una key:
  - `Builtin { set_id: String, key: IconKey }` — un ícono de cualquier set.
  - `UserPng { rel_path: String }` — un PNG propio bajo `<config_dir>/icons/_user/`.
- **Override por objeto** — `BTreeMap<IconKey, IconSource>`. Vacío por defecto.
- **Set efectivo** — set base + overrides aplicados encima. Es lo que la app pinta.

**`Settings`** (campos nuevos, todos `#[serde(default)]`, retro-compatibles):

```rust
/// Set base activo (ya existe). Migrado a IDs de fábrica.
pub icon_set: String,            // default "lucide"
/// Overrides por objeto sobre el set base. Vacío = solo el set base.
#[serde(default)]
pub icon_overrides: BTreeMap<IconKey, IconSource>,
```

`IconKey` debe serializar a una clave estable de string (p.ej. `"action_back"`,
`"file_image"`) — se reutiliza `icons::file_name(key)` que ya existe, más el parse
inverso.

**Migración del config viejo.** Al cargar `settings.json`:

- `icon_set == "flat"`  → `"flat-color"`
- `icon_set == "fluent"`→ `"lucide"`
- `icon_set == "mono"`  → `"mono"` (se queda)
- cualquier otro id desconocido → resuelve vía `IconSetCatalog` (cae a `lucide` si
  no existe en disco; hoy cae a `flat`).

Sin pérdida del resto de la config (`#[serde(default)]` honra `CONFIG_VERSION`).

**Resolución (extiende `icons::resolve_bytes`):**

```
resolve_bytes(settings, key, config_dir) -> Vec<u8>:
  1. ¿hay override para key en settings.icon_overrides?
       Builtin{set,key} -> bytes del set indicado (embebido o pack suelto)
       UserPng{path}    -> lee <config_dir>/icons/_user/<path>
  2. si no hay override -> set base settings.icon_set (embebido o pack suelto)
  3. si falta el archivo -> unknown embebido (lucide). Nunca devuelve vacío.
```

Tolerante: un PNG ilegible/ausente cae al fallback, no falla (regla "el filesystem
es hostil").

### Generación de los sets de fábrica (build-time)

Generador en `xtask`/script Rust **fuera del binario final** (como el `gen_icons`
previo, pero idempotente y no destructivo del config del usuario):

- Lee los SVG de las librerías y rasteriza cada `IconKey` a PNG a 2-3 tamaños
  (24 / 48 @2x) para nitidez en distintas escalas.
- Sets tintables (lucide/tabler/material/mono): renderiza el glifo como **máscara
  alfa de un solo color** (blanco), para que la app aplique el color del tema
  multiplicando por alfa en runtime.
- `flat-color`: conserva el color original.
- Escribe `assets/icons/<set>/manifest.json`: `{ id, label, tintable, version,
  license }`.
- Escribe a `assets/icons/<set>/` de forma reversible por git. **Nunca** toca
  `<config_dir>/icons/` del usuario.

Mapeo de `IconKey` → nombre de ícono en cada librería: tabla explícita en el
generador (cada librería nombra distinto: `arrow-left` vs `arrow_back` vs `left`).

### Tinte en runtime

El render de Slint es por software (sin GPU). El tinte se aplica **al decodificar**
en el `IconCache`, una sola vez por `(set_efectivo, key, color_tema)`:

- Set tintable → se multiplica el color del tema por el canal alfa de la máscara.
- Set no tintable (`flat-color`) → se usa el PNG tal cual.

El cache se invalida al cambiar set base, overrides o tema.

### Capa `ui-slint`

**Pestaña "Íconos"** nueva en `config-window.slint`, junto a las existentes. Tres
bloques (ver mockup `03-mockup-ui.html`):

1. **Set base** — chips para los 5 sets de fábrica + cualquier pack importado.
   Cambiar el set base **no borra** los overrides (se reaplican encima). Cambio en
   caliente (patrón ya existente).
2. **Íconos por objeto** — grilla con las 33 filas, agrupadas: primero acciones de
   barra, luego tipos de archivo. Cada fila: ícono actual, nombre, origen
   ("Lucide (base)" / "Material · override"), botón "Cambiar ▾". Las filas con
   override llevan un badge. Sin sub-pestañas: scroll simple.
3. **Panel "Set personalizado"** — Importar `.naygoset`, Exportar set actual,
   Restablecer al set base (limpia overrides).

**Selector de ícono** (popup al pulsar "Cambiar ▾"): el objeto renderizado en cada
set (un set por columna, scroll horizontal si sobran) + opción "PNG propio…" que
abre un `FileDialog` nativo. El PNG elegido se copia a
`<config_dir>/icons/_user/<hash>.png` y el override referencia esa ruta relativa.

El `IconCache` de `icons.rs` se extiende a clave por set efectivo + tinte e
invalida al cambiar set/override/tema.

### Formato `.naygoset` (import/export)

Un `.zip` con extensión propia:

```
mi-set.naygoset
├── manifest.json   { schema:1, name, author, base_set_id, tintable_default,
│                     overrides:[{ key, source }] }
├── icons/          PNG que el set aporta:
│   ├── folder.png        (overrides Builtin de un set no-fábrica, o base no-fábrica)
│   └── _user/ab12.png    (overrides UserPng)
└── LICENSE.txt     (opcional; atribución de librerías para sets derivados)
```

- **Exportar**: serializa el set efectivo (base + overrides) y empaqueta los PNG
  necesarios — los del usuario siempre; los del base solo si el base no es de
  fábrica. Resultado autocontenido.
- **Importar**: valida `schema`, copia a `<config_dir>/icons/<nombre>/`, registra
  el set en el catálogo, queda disponible para activar. Si `base_set_id` es de
  fábrica y existe, se reutiliza embebido (no se duplica). Tolerante: PNG corrupto
  o key desconocida no rompe la importación — cae al `unknown` embebido y se avisa
  discreto vía `MessageModal`. Sin panics.

### i18n

Todas las cadenas nuevas (nombres de objetos, botones, avisos de import/export)
por clave en `lang/es.json` y `lang/en.json`. Ningún texto hardcoded.

## Componentes y responsabilidades

| Unidad | Responsabilidad | Depende de |
|--------|-----------------|------------|
| `core::icons` (mod) | tabla embebida + `resolve_bytes` con overrides + tinte-spec | `icon_kind`, `config` |
| `core::icon_set` (catalog) | catálogo de sets (fábrica + sueltos + importados) | fs read_dir |
| `core::icon_source` (nuevo) | `IconSource`, serde, parse de key↔string | `icon_kind` |
| `core::icon_pack` (nuevo) | export/import `.naygoset` (zip + manifest), tolerante | `zip`, `serde_json` |
| `core::config` | `Settings.icon_overrides` + migración del enum viejo | serde |
| `xtask gen_icons` | rasterizar SVG → PNG máscara/color + manifest (build-time) | resvg/usvg |
| `ui-slint icons.rs` | `IconCache` por set efectivo + tinte | `core::icons` |
| `ui-slint config-window.slint` | pestaña "Íconos": set base + grilla + import/export | VM |
| `ui-slint config_ctrl.rs` | cablear set base, overrides, import/export a `core` | core |

## Errores

Conforme a "el filesystem es hostil" y "la app nunca cae":

- PNG ausente/ilegible (override o base) → fallback `unknown` embebido.
- `.naygoset` corrupto, schema inválido, zip ilegible → import abortado con aviso
  discreto (`MessageModal`), config intacta.
- Key desconocida en un manifest importado → se ignora esa entrada, el resto entra.
- PNG propio que el usuario borró del disco → el override cae al fallback.

## Testing (capa core, sin UI)

- Round-trip `.naygoset`: export → import produce el mismo set efectivo.
- `resolve_bytes` con y sin override (Builtin y UserPng), y fallback a unknown.
- Migración: `icon_set` `"flat"`→`"flat-color"`, `"fluent"`→`"lucide"`,
  `"mono"`→`"mono"`; settings v1 sin `icon_overrides` cae a vacío.
- Import tolerante: zip con un PNG corrupto entra igual (esa key al fallback).
- Cada set de fábrica cubre las 33 keys (extiende el test existente).
- Serde de `IconSource` y parse key↔string ida y vuelta.

Objetivo: ~25-30 tests nuevos. Mantener el build limpio + clippy antes de commit.

## Trade-offs decididos

- **Máscara alfa vs PNG pre-coloreado** para tintables: se elige máscara alfa.
  Es la causa de que hoy "no se note" el cambio de set/tema; cuesta poco código en
  el `IconCache` y resuelve el problema de raíz.
- **5 sets** (no 3): Lucide + Tabler + Material + Flat Color + Mono dan 2 de línea,
  1 de relleno, 1 de color y 1 mono — máxima diferencia perceptible.
- **`.naygoset` autocontenido** (no solo JSON): un JSON de overrides se rompe si el
  destino no tiene el set/PNG referenciado. El zip lo evita.
- **Overrides globales** (no por tema): simplicidad; YAGNI hasta que se pida.

## Fuera de alcance / fases futuras

- Íconos de tipos de archivo alimentados por VSCode-icons en los paneles.
- Eyedropper/cuentagotas para el editor de temas (ya hay idea aparte).
- Overrides por-tema.
