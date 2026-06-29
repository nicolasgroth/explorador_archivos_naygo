# Auditoría i18n + idiomas nuevos — diseño

> Naygo — explorador de archivos. Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
> SPDX-License-Identifier: MIT
> Feature #3 de 4 pedidas el 2026-06-28. Rama `feat/iconos-personalizables`.

## Motivación

Nicolás quiere sumar idiomas además de ES/EN. PERO primero exige **auditar** que TODOS los
textos visibles (errores, popups, títulos, toasts, avisos) estén cubiertos por clave i18n,
no hardcodeados — para que al traducir no queden huecos en español.

Estado verificado: ~29 strings sospechosos en `.rs` + 5 en `.slint`, sobre 792 claves i18n.
El proyecto está mayormente internacionalizado, pero hay una cola de hardcodes (incluido el
`archive_tree` de la feature #4). El sistema ya soporta cargar `lang/*.json` (embebidos +
sueltos) y el fallback es robusto (idioma activo → ES → la clave misma, nunca panic).

## Objetivos

1. **Auditoría exhaustiva**: barrer todo el código, mover a clave i18n (ES+EN) cualquier
   texto visible al usuario que hoy esté hardcodeado. Cero huecos.
2. **8 idiomas nuevos**: pt, fr, de, it (latinos, completos y probados) + zh, ja, ko, hi
   (CJK/hindi, marcados experimentales — su render depende de la fuente del SO).
3. **Árabe queda FUERA** (requiere soporte RTL real, feature aparte futura).
4. Traducir las ~800 claves a cada idioma; **parity estricto** (todos los idiomas con las
   mismas claves que es.json).

### No-objetivos (YAGNI)

- No soporte RTL (árabe/hebreo) — feature aparte.
- No fuente embebida CJK/Devanagari — se depende de la del SO (por eso "experimental").
- No traducción de comentarios de código ni logs (no son visibles al usuario).

## Decisiones (brainstorming)

- Auditoría exhaustiva + arreglar todo (deja la base 100% lista).
- Latinos completos; CJK/hindi experimental; árabe fuera.
- Yo (Claude) traduzco las ~800 claves a cada idioma; validación por parity + revisión.
- `archive_tree` (core puro) recibe los textos traducidos como parámetros (no los hardcodea).

## Arquitectura

### Parte 1 — Auditoría (mover hardcodes a clave)

**Inventario**: localizar cada texto visible hardcodeado:
- `.rs`: literales que terminan en la UI — `Payload::Message("...")`, `.set_*("...")`,
  `MessageVm { body: "..." }`, toasts `invoke_show_toast("...")`, etc. (NO logs/eprintln/
  tracing/panic — esos no son del usuario).
- `.slint`: `text: "..."` con texto en español (default de propiedades de UI).
- Excluir: nombres de archivo, claves técnicas, comentarios, tests.

**Por cada hardcode**: crear la clave en es.json + en.json, reemplazar el literal por
`c.t("clave")` (Rust, vía el accesor `I18n` ya existente) o un campo `Tr.<clave>` (Slint,
cableado en i18n_keys.rs). Seguir el patrón ya usado en el resto del código.

**Caso especial `core::archive_tree`** (feature #4): hoy embebe strings ES ("archivo(s)",
"carpeta(s)", "… y más entradas", "sin comprimir"). Como `core` no conoce el `I18n` de UI,
la solución: `render_archive_tree` recibe un struct de etiquetas ya traducidas:
```rust
pub struct ArchiveLabels {
    pub files: String,        // "archivo(s)" / "file(s)"
    pub folders: String,      // "carpeta(s)" / "folder(s)"
    pub uncompressed: String, // "sin comprimir" / "uncompressed"
    pub more_entries: String, // "… y más entradas" / "… and more entries"
    pub and_more: String,     // plantilla "… y {n} más" / "… and {n} more"
}
```
La capa ui-slint (`preview.rs`) arma `ArchiveLabels` desde `c.t(...)` y se lo pasa. Así core
sigue puro y el texto es traducible. Las claves van en los JSON (`archive.files`, etc.).

### Parte 2 — Idiomas nuevos

**Archivos**: `crates/core/src/i18n/{pt,fr,de,it,zh,ja,ko,hi}.json`, embebidos con
`include_str!` junto a es/en. Registrarlos en el catálogo embebido (donde hoy se hace
`include_str!("es.json")` / `en.json`).

**Nombre nativo en el selector**: cada idioma tiene una clave `lang.<code>` (hoy
`"lang.es": "Español"`, `"lang.en": "English"`). Agregar `lang.pt`="Português",
`lang.fr`="Français", `lang.de`="Deutsch", `lang.it`="Italiano", `lang.zh`="中文",
`lang.ja`="日本語", `lang.ko`="한국어", `lang.hi`="हिन्दी" — EN TODOS los JSON (el nombre nativo
es el mismo en cualquier idioma activo; el parity exige que la clave exista en todos).

**Experimental**: los 4 CJK/hindi llevan un sufijo o nota en el selector ("中文
(experimental)" o una marca). Decisión simple: agregar al valor `lang.zh` etc. el sufijo
" (experimental)" NO — eso ensucia el nombre nativo. Mejor: una clave aparte o que el
selector añada la marca para esos códigos. Implementación: el código mantiene una lista de
"idiomas experimentales" `["zh","ja","ko","hi"]`; el selector, al construir la etiqueta,
añade un sufijo traducido (`lang.experimental_suffix` = " (experimental)") para esos códigos.

### Traducciones

Traducir las ~800 claves de es.json (y cotejar con en.json) a cada idioma:
- pt/fr/de/it: traducción directa, calidad buena (alfabeto latino).
- zh/ja/ko/hi: traducción razonable; los placeholders (`{drive}`, `{n}`, `{path}`, etc.) se
  conservan literales (no se traducen).
- Tono: neutral, consistente con el registro de la app.

## Componentes y responsabilidades

| Unidad | Responsabilidad | Depende de |
|--------|-----------------|------------|
| auditoría (varios .rs/.slint) | mover hardcodes a clave i18n | i18n catalog |
| `core::archive_tree` (refactor) | recibir ArchiveLabels (no hardcodear) | — |
| `ui::preview` | armar ArchiveLabels desde c.t | i18n |
| `i18n/{pt,fr,de,it,zh,ja,ko,hi}.json` | catálogos nuevos (≈800 claves c/u) | — |
| catálogo embebido (i18n/mod.rs) | include_str! de los 8 + registro | — |
| selector de idioma (main.rs) | listar con nombre nativo + marca experimental | lang.* keys |
| parity test (i18n/mod.rs) | todos los idiomas == claves de es.json | todos los JSON |

## Errores

- Fallback robusto ya existe (activo → ES → clave). No cambia. Un idioma con una clave
  faltante NO crashea (cae a ES), pero el parity test lo detecta para que no haya huecos.
- JSON inválido en un idioma → el catálogo lo ignora con log (comportamiento actual), pero
  el test de "JSON válido" lo detecta en CI.
- Placeholders mal copiados (`{drive}` traducido por error) → revisión + (si es viable) un
  test que verifique que cada valor con `{x}` en es.json tiene el mismo `{x}` en los demás.

## Testing

- **Parity estricto**: extender/duplicar `es_en_tienen_las_mismas_claves` a un test que
  compruebe que CADA idioma (pt/fr/de/it/zh/ja/ko/hi/en) tiene EXACTAMENTE el mismo conjunto
  de claves que es.json. Un idioma con clave de más o de menos → falla.
- **JSON válido**: cada archivo parsea (un test que cargue los 10).
- **Placeholders**: un test que, para cada clave con `{...}` en es.json, verifique que el
  mismo placeholder aparece en todos los idiomas (evita "… y {n} más" mal traducido).
- **archive_tree**: el test existente se adapta a `ArchiveLabels` (pasar labels de prueba).
- Verificación visual (render CJK, nombres nativos, marca experimental) en la VM por Nicolás.

## Trade-offs decididos

- **CJK/hindi experimental, no garantizado**: honesto sobre que el render depende de la
  fuente del SO; no embebemos fuentes (peso) en esta entrega.
- **Árabe fuera**: RTL es una feature grande aparte; prometer árabe sin RTL daría texto
  desordenado.
- **archive_tree con labels parametrizados**: mantiene core puro sin acoplarlo al I18n de UI.
- **parity estricto** (todos == es): garantiza cero huecos; el costo es traducir el 100%.

## Orden de implementación

1. **Auditoría**: inventario → mover hardcodes (incluido archive_tree con ArchiveLabels).
2. **Infraestructura**: los 8 JSON (inicialmente copia de en.json como placeholder para que
   compile + parity) + registro embebido + selector con nombre nativo + marca experimental +
   tests de parity/JSON/placeholders.
3. **Traducciones**: llenar cada idioma (reemplazar el placeholder en-copy por la traducción
   real), idioma por idioma, validando parity en cada uno.

## Fuera de alcance / fases futuras

- Árabe + soporte RTL.
- Fuente embebida CJK/Devanagari (para garantizar render sin depender del SO).
- Esta es la última de las 4 mejoras pedidas el 2026-06-28.
