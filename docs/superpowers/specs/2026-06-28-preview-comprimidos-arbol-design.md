# Preview de comprimidos: árbol + totales + tar/gz — diseño

> Naygo — explorador de archivos. Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
> SPDX-License-Identifier: MIT
> Feature #4 de 4 pedidas el 2026-06-28. Rama `feat/iconos-personalizables`.

## Motivación

El panel de vista previa YA muestra el contenido de un `.zip` (`read_zip_listing` en
`crates/ui-slint/src/preview.rs`): una lista plana de entradas con su tamaño
descomprimido, hasta 500, tolerante a zip dañado. Funciona, pero:

1. La lista es **plana** (rutas completas repetidas: `proyecto/`, `proyecto/src/`,
   `proyecto/src/main.rs`…) — poco legible para zips con estructura.
2. No hay **resumen** (cuántos archivos/carpetas, tamaño total).
3. Solo `.zip` — no `.tar` ni `.tar.gz`.

Nicolás pidió mejorar el preview de comprimidos aprovechando que el crate `zip` ya
está. Decidido en brainstorming: árbol ASCII + encabezado de totales + soporte tar/gz.
Los íconos por tipo quedan FUERA (el preview es texto monoespaciado; meter íconos
rediseñaría el panel — no vale el costo).

## Objetivos

1. **Árbol ASCII** (estilo `tree`: `├─ └─ │`) en vez de lista plana, con tamaños
   alineados. Sigue siendo texto monoespaciado (cero cambio en cómo se pinta el preview).
2. **Encabezado** con totales: "N archivos, M carpetas · TAMAÑO sin comprimir".
3. **Más formatos**: además de `.zip`, leer `.tar` y `.tar.gz` (y `.tgz`).

### No-objetivos (YAGNI)

- No íconos por tipo (el preview es texto).
- No `.rar` / `.7z` (requieren libs no-libres / no-Rust-puro).
- No extraer/descomprimir (sigue siendo solo lectura del índice).
- No un widget de árbol expandible (sigue siendo texto).

## Arquitectura

Sigue las 3 capas. **La construcción del árbol es lógica pura en `core`** (testeable sin
I/O); las capas de lectura (zip/tar/gz) viven donde ya está el worker (ui-slint), y
producen una lista de entradas que se pasa a la función pura de core.

### Capa `core` — `archive_tree` (nuevo módulo, puro y testeable)

`crates/core/src/archive_tree.rs`:

```rust
/// Una entrada de un archivo comprimido (zip/tar): ruta interna + tamaño descomprimido.
pub struct ArchiveEntry {
    pub path: String,   // ruta interna con '/' como separador, p.ej. "proyecto/src/main.rs"
    pub is_dir: bool,
    pub size: u64,      // tamaño descomprimido (0 para carpetas)
}

/// Resumen de un archivo comprimido.
pub struct ArchiveSummary {
    pub files: usize,
    pub dirs: usize,
    pub total_uncompressed: u64,
    pub truncated: bool,   // si se listaron menos de las reales (tope)
    pub total_entries: usize,
}

/// Construye el texto del preview: encabezado de totales + árbol ASCII indentado.
/// Puro: recibe las entradas ya leídas (sin tocar disco). Determinista y testeable.
pub fn render_archive_tree(
    entries: &[ArchiveEntry],
    summary: &ArchiveSummary,
    name: &str,           // nombre del archivo (para el encabezado)
    size_fmt: SizeFormat, // para formatear tamaños
) -> String;
```

`render_archive_tree`:
- **Construye un árbol** a partir de las rutas planas: parte cada `path` por `/`, crea
  nodos intermedios (carpetas) aunque no haya una entrada explícita para ellos (algunos
  tar/zip no listan las carpetas como entradas propias). Ordena: carpetas primero, luego
  archivos, alfabético dentro de cada nivel.
- **Dibuja el árbol ASCII**: cada nivel con `├─ ` / `└─ ` (último hijo) y `│  ` / `   `
  para la continuación de las líneas verticales. Los archivos muestran su tamaño alineado
  a la derecha (con `format_size`).
- **Encabezado**: una primera línea con el nombre + el resumen
  ("8 archivos, 4 carpetas · 21.6 KB sin comprimir"), una línea separadora, luego el árbol.
- Respeta el tope de entradas (constante, p.ej. 500 — el mismo de hoy): si `truncated`,
  agrega "… y N más" al final.

> El árbol se construye desde las rutas; las carpetas implícitas (presentes en la ruta de
> un archivo pero sin entrada propia) se crean para que el árbol no tenga huecos. Esto es
> robusto frente a tar (que no siempre lista carpetas) y zip (que sí).

### Capa `ui-slint` — lectura de zip/tar/gz

`crates/ui-slint/src/preview.rs`:
- Reemplazar `read_zip_listing` por `read_archive_listing(path) -> Payload` que:
  1. Detecta el formato por extensión (`.zip` → zip; `.tar` → tar; `.tar.gz`/`.tgz` →
     gz+tar). Una función `archive_format(path) -> Option<ArchiveFormat>`.
  2. Lee las entradas (hasta el tope) a un `Vec<ArchiveEntry>` + arma el `ArchiveSummary`:
     - zip: `zip::ZipArchive` (como hoy).
     - tar: `tar::Archive` sobre el `File`.
     - tar.gz: `flate2::read::GzDecoder` envolviendo el `File`, luego `tar::Archive`.
  3. Llama `naygo_core::archive_tree::render_archive_tree(...)` y devuelve
     `Payload::Text { text, truncated, highlighted: None }`.
  - Tolerante: archivo ilegible/corrupto → `Payload::Message("… inválido o dañado")`,
    como hoy. Nunca panic.
- `build_payload` (línea ~228): cambiar `if is_zip(path)` por
  `if is_archive(path)` (zip/tar/tgz), llamando `read_archive_listing`.

### Dependencias

Agregar a `crates/ui-slint/Cargo.toml` (donde se leen los archivos):
- `tar = "0.4"` (MIT/Apache).
- `flate2 = "1"` (MIT/Apache, backend `miniz_oxide` puro-Rust por defecto — sin libs C).
`zip` ya está. `core` no necesita deps nuevas (`archive_tree` es puro).

## Componentes y responsabilidades

| Unidad | Responsabilidad | Depende de |
|--------|-----------------|------------|
| `core::archive_tree::ArchiveEntry/Summary` | tipos de datos del índice | — |
| `core::archive_tree::render_archive_tree` | texto = encabezado + árbol ASCII (puro) | format |
| `ui::preview::archive_format` | detectar zip/tar/tgz por extensión | — |
| `ui::preview::read_archive_listing` | leer el índice → entries → render | zip, tar, flate2, archive_tree |
| `ui::preview::build_payload` | enrutar archivos comprimidos al nuevo lector | read_archive_listing |

## Errores

- Archivo corrupto/ilegible (zip/tar/gz) → `Payload::Message` con aviso, no panic.
- tar.gz con gzip válido pero tar corrupto adentro → aviso.
- Entradas con rutas raras (vacías, solo `/`, con `..`) → el árbol las ignora o las pone
  en la raíz sin romper (no se extrae nada, así que `..` no es riesgo de escritura; pero
  el render no debe panicar).
- Tope de entradas: igual que hoy (constante), con "… y N más".

## Testing (core, puro)

`render_archive_tree` y la construcción del árbol son 100% testeables sin I/O:
- Un set de `ArchiveEntry` planas → el texto de árbol esperado (con `├─ └─ │`).
- Carpetas implícitas: entradas `a/b/c.txt` sin entrada `a/` ni `a/b/` → el árbol crea
  `a/` y `a/b/`.
- Orden: carpetas antes que archivos, alfabético.
- Encabezado: cuenta archivos/carpetas y suma tamaños correctamente.
- Truncado: con más entradas que el tope → "… y N más".
- Entrada vacía / lista vacía → texto sin panic (encabezado "0 archivos").
La lectura real de zip/tar/gz se verifica con un test de integración liviano (crear un
.zip y un .tar.gz de prueba en un tempdir, leerlos, comprobar que el texto contiene las
entradas) y en vivo en la VM.

## Trade-offs decididos

- **Árbol como texto ASCII** (no widget): cero rediseño del panel de preview, liviano,
  cumple "ver el contenido". Un widget expandible sería sobre-ingeniería.
- **tar/gz sí, rar/7z no**: tar+flate2 son puro-Rust y libres; rar/7z no.
- **Construcción del árbol en core**: pura y testeable; la lectura de formatos (con I/O)
  queda en la capa que ya tiene el worker.

## Fuera de alcance / fases futuras

- Comprimir/descomprimir de verdad (crear/extraer): fase mayor futura, ya acordada fuera.
- Íconos por tipo en el árbol: descartado (el preview es texto).
- La feature #3 (auditoría i18n + idiomas) va en su propio spec, después de esta.
