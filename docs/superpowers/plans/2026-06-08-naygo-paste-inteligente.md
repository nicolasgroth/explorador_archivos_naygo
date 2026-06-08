# paste-inteligente — Pegar el portapapeles del SO según su tipo — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que `Ctrl+V` lea el portapapeles del SISTEMA y actúe según su contenido — archivos→copiar/mover (motor ops-A), texto→crear `.txt`, imagen→crear PNG/JPG — y que `Ctrl+C`/`Ctrl+X` escriban al portapapeles del SO (interop con Explorer).

**Architecture:** La DECISIÓN vive en `core::clipboard` (pura, testeable: `decide_paste` + `encode_image` + `expand_name_template`); la LECTURA/ESCRITURA Win32 vive en `platform::clipboard`; la UI orquesta y reusa el motor de ops-A (Transfer) o un worker corto (CreateText/CreateImage). El crate `image` se mueve de `ui` a `core` (features png+jpeg).

**Tech Stack:** Rust, `naygo-core`/`naygo-platform`/`naygo-ui`, `eframe`/`egui` 0.34.3, `serde`/`serde_json`, `image` 0.25 (png+jpeg), crate `windows` 0.62 (Win32 portapapeles/GDI), `std::thread`/`mpsc`. Sin chrono.

**Estado de partida (rama `feat/paste-inteligente`, desde `main` con ops-A+ops-B):**
- `naygo_core::ops::names::dedup_name(candidate: &Path, exists: &dyn Fn(&Path) -> bool) -> PathBuf` — genera `nombre (2).ext` si choca. EXISTE, se reutiliza.
- `naygo_core::ops`: `OpRequest { kind, sources, dest_dir, conflict }`, `OpKind`, `ConflictPolicy`.
- `naygo_core::config`: `const CONFIG_VERSION: u32 = 1;` (NO cambia). `Settings` (línea 57) usa el patrón aditivo `#[serde(default = "fn_name")]` con una `fn default_x()` por campo. `load_settings`/`save_settings`.
- `naygo-platform`: patrón Win32 = `#[cfg(windows)]` impl real + `#[cfg(not(windows))]` stub que devuelve `NotSupported`/vacío. Cargo: `[target.'cfg(windows)'.dependencies] windows = { workspace = true, features = [...] }`. `windows` workspace = "0.62". `trash.rs` es el molde a seguir.
- `naygo-ui::app`: `struct InternalClipboard { paths: Vec<PathBuf>, cut: bool }` (línea ~57, `#[derive(Default)]`); campo `clipboard: InternalClipboard` en `NaygoApp` (~126, init ~194). `clipboard_set(&mut self, cut: bool)` (~901) puebla `self.clipboard`. `paste(&mut self)` (~944) lee `self.clipboard`, arma `transfer(...)`, llama `launch_transfer`. `launch_transfer(&mut self, req: OpRequest, label)` (~926) resuelve conflicto y `start_op`. `start_op` (~410). `active_dir()`. `ops_actions::transfer(kind_move: bool, sources: Vec<PathBuf>, dest_dir: PathBuf) -> OpRequest` (ui/src/ops_actions.rs:11). `self.i18n.t(key) -> String`. `self.status: String`. `pending_dialog: Option<PendingDialog>` + `process_pending_dialog`. `ops_dialogs.rs` tiene modales con `egui::Modal::new(egui::Id::new(...)).show(ctx, |ui| {...})`, i18n por `&I18n` (use `naygo_core::i18n::I18n`), `i18n.t(k)`.
- `naygo-ui` Cargo: `image = { version = "0.25", default-features = false, features = ["png"] }` (línea 19) — se MUEVE a core.

**Prerequisito:** Rust en PATH. PowerShell: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path";`. NUNCA `2>&1` con cargo. `cargo fmt --all -- --check`. Binario `--bin naygo`. Bash: NO `cd /d`.

**Convenciones (CLAUDE.md):** inglés en código; comentarios/commits español OK. Header de 2 líneas en archivos NUEVOS. `core` NUNCA importa egui/windows (sí `image`, que es puro). UI nunca hace I/O en el hilo de UI (la escritura va a worker). Tolerante (filesystem hostil, Result tipado, sin panics). Build+tests+clippy `--workspace --all-targets -- -D warnings`+fmt antes de cada commit. SIEMPRE `cargo fmt --all` antes de commitear (este repo arrastra fmt drift). Footer:
```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Rama:** ya estás en `feat/paste-inteligente`. NO cambiar de rama.

**SECUENCIA (crítica — no dejar el árbol roto entre tareas):**
Tasks 1-4 (core puro + config) son self-contained y no rompen nada. Task 5 (platform) es self-contained (módulo nuevo). Task 6 hace DOS cosas acopladas EN UN COMMIT: mueve `image` de ui a core (Cargo) Y reescribe el paste de la UI quitando `InternalClipboard`. Se hace junto porque quitar `InternalClipboard` rompe la compilación de ui hasta que el nuevo paste esté en su lugar, y mover `image` toca ambos Cargo. Antes de Task 6 todo compila; después de Task 6 todo compila. Nunca se commitea un estado intermedio roto.

**Alcance:** ENTRA: `core::clipboard` (decide_paste/encode_image/expand_name_template + tipos), Settings de paste, `platform::clipboard` (read/write_files), UI (Ctrl+C/X/V reescritos, modo directo+toast, modo confirmar mini-diálogo, toast de copia clicable), i18n. NO ENTRA: DnD COM/OLE, HTML/RTF, formatos de imagen extra, audio.

---

## Estructura de archivos

```
crates/core/src/clipboard/
├── mod.rs       # NUEVO: ClipboardContent, ClipboardImage, ImageFmt, PastePlan, decide_paste
├── encode.rs    # NUEVO: encode_image (PNG/JPG) + EncodeError
└── naming.rs    # NUEVO: expand_name_template ({fecha}, sin chrono)
crates/core/src/lib.rs        # + pub mod clipboard
crates/core/src/config/mod.rs # + Settings de paste
crates/core/Cargo.toml        # + image (png+jpeg) [movido de ui]
crates/core/src/i18n/{es,en}.json # + claves toast/diálogo de paste

crates/platform/src/clipboard.rs # NUEVO: read() + write_files() Win32 (+ stub no-windows)
crates/platform/src/lib.rs        # + pub mod clipboard
crates/platform/Cargo.toml        # + features windows (DataExchange/Memory/Shell/Gdi)

crates/ui/Cargo.toml          # - image (movido a core)
crates/ui/src/app.rs          # Ctrl+C/X→write_files; Ctrl+V→read+decide_paste+ejecutar; quitar InternalClipboard; worker escritura; toast
crates/ui/src/ops_dialogs.rs  # + PastePreview (mini-diálogo modo B)
```

---

## Task 1: `core::clipboard` — tipos del módulo

**Files:**
- Create: `crates/core/src/clipboard/mod.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear el módulo con los tipos + un test de serde de ImageFmt**

Create `crates/core/src/clipboard/mod.rs`:
```rust
// Naygo — pegado inteligente: decidir qué hacer con el portapapeles del SO (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA del pegado inteligente. `platform` lee el portapapeles del SO y lo
//! normaliza a `ClipboardContent`; aquí `decide_paste` decide la acción (`PastePlan`)
//! y `encode_image` codifica una imagen del portapapeles a PNG/JPG. Sin Windows ni egui.

pub mod encode;
pub mod naming;

use crate::ops::names::dedup_name;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Límite de píxeles de una imagen del portapapeles aceptable (≈512 megapíxeles).
/// Evita asignar memoria absurda ante un DIB corrupto.
pub const MAX_IMAGE_PIXELS: u64 = 512 * 1024 * 1024;

/// Contenido del portapapeles del SO, ya leído por `platform` y normalizado.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipboardContent {
    /// Archivos (CF_HDROP). `cut` = el Preferred DropEffect es MOVE.
    Files { paths: Vec<PathBuf>, cut: bool },
    /// Texto plano (CF_UNICODETEXT).
    Text(String),
    /// Imagen (CF_DIB) ya pasada a RGBA8.
    Image(ClipboardImage),
    /// Nada usable en el portapapeles.
    Empty,
}

/// Imagen del portapapeles: RGBA8 sin comprimir + dimensiones.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>, // longitud esperada = width * height * 4
}

/// Formato de salida al pegar una imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFmt {
    Png,
    Jpg,
}

impl ImageFmt {
    /// Extensión de archivo (sin punto).
    pub fn ext(self) -> &'static str {
        match self {
            ImageFmt::Png => "png",
            ImageFmt::Jpg => "jpg",
        }
    }
}

/// Qué hará el pegado, decidido a partir del contenido + config. Resultado puro.
#[derive(Clone, Debug, PartialEq)]
pub enum PastePlan {
    /// Transferencia de archivos → motor de ops-A.
    Transfer { paths: Vec<PathBuf>, cut: bool },
    /// Crear un archivo de texto con `body` en `path`.
    CreateText { path: PathBuf, body: String },
    /// Crear una imagen en `path` con el formato dado.
    CreateImage {
        path: PathBuf,
        fmt: ImageFmt,
        img: ClipboardImage,
    },
    /// Nada que pegar.
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_fmt_ext() {
        assert_eq!(ImageFmt::Png.ext(), "png");
        assert_eq!(ImageFmt::Jpg.ext(), "jpg");
    }

    #[test]
    fn image_fmt_serde_round_trip() {
        let j = serde_json::to_string(&ImageFmt::Jpg).unwrap();
        let back: ImageFmt = serde_json::from_str(&j).unwrap();
        assert_eq!(back, ImageFmt::Jpg);
    }
}
```
NOTE: `encode`/`naming` submodules are declared now but created in Tasks 2-3; until then they're empty files. Create EMPTY placeholder files so the module compiles: `crates/core/src/clipboard/encode.rs` and `naming.rs` each with just the 2-line header comment. Tasks 2-3 fill them.

Create `crates/core/src/clipboard/encode.rs` (placeholder):
```rust
// Naygo — codificación de imagen del portapapeles a PNG/JPG (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```
Create `crates/core/src/clipboard/naming.rs` (placeholder):
```rust
// Naygo — expansión de plantillas de nombre para el pegado (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
```

- [ ] **Step 2: Declarar el módulo en lib.rs**

Modify `crates/core/src/lib.rs`: add `pub mod clipboard;` next to the other `pub mod` declarations (e.g. near `pub mod ops;`).

- [ ] **Step 3: Verificar**

Run: `cargo test -p naygo-core clipboard` → 2 tests PASS.
Run: `cargo clippy -p naygo-core --lib -- -D warnings` → clean. (If clippy flags the empty submodule files as having no items, that's fine — they're modules with only comments, which is valid.)

- [ ] **Step 4: Commit**
```
git add crates/core/src/clipboard/ crates/core/src/lib.rs
git commit -m "feat(core): módulo clipboard — tipos de pegado inteligente

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep types/fields EXACTLY (Tasks 2-7 depend on ClipboardContent, ClipboardImage{width,height,rgba}, ImageFmt{Png,Jpg}+ext(), PastePlan variants).

---

## Task 2: `expand_name_template` — fecha sin chrono

**Files:**
- Modify: `crates/core/src/clipboard/naming.rs`

- [ ] **Step 1: Tests (TDD)**

Replace `crates/core/src/clipboard/naming.rs` content with header + tests first:
```rust
// Naygo — expansión de plantillas de nombre para el pegado (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Expande plantillas de nombre como "pegado {fecha}" sin depender de `chrono`.
//! `{fecha}` → "YYYY-MM-DD HH-MM" en UTC, derivada de los segundos epoch (determinista
//! y testeable). Otros `{...}` desconocidos se dejan literales.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expande_fecha_epoch_cero() {
        // 1970-01-01 00:00 UTC
        assert_eq!(expand_name_template("pegado {fecha}", 0), "pegado 1970-01-01 00-00");
    }

    #[test]
    fn expande_fecha_conocida() {
        // 2021-01-01 00:00:00 UTC = 1609459200
        assert_eq!(expand_name_template("x {fecha}", 1_609_459_200), "x 2021-01-01 00-00");
    }

    #[test]
    fn expande_fecha_con_hora_minuto() {
        // 2021-01-01 13:45:00 UTC = 1609459200 + 13*3600 + 45*60 = 1609508700
        assert_eq!(expand_name_template("{fecha}", 1_609_508_700), "2021-01-01 13-45");
    }

    #[test]
    fn sin_token_queda_igual() {
        assert_eq!(expand_name_template("captura", 123), "captura");
    }

    #[test]
    fn token_desconocido_literal() {
        assert_eq!(expand_name_template("a {otro} b", 0), "a {otro} b");
    }
}
```

- [ ] **Step 2: Correr y ver fallar**

Run: `cargo test -p naygo-core clipboard::naming` → ERROR: `expand_name_template` no existe.

- [ ] **Step 3: Implementar (en naming.rs, antes del mod tests)**
```rust
/// Expande `{fecha}` en `template` por "YYYY-MM-DD HH-MM" (UTC) derivado de
/// `now_secs` (segundos epoch). Tokens `{...}` desconocidos se dejan literales.
pub fn expand_name_template(template: &str, now_secs: u64) -> String {
    let (y, mo, d, h, mi) = civil_from_epoch(now_secs);
    let fecha = format!("{y:04}-{mo:02}-{d:02} {h:02}-{mi:02}");
    template.replace("{fecha}", &fecha)
}

/// Convierte segundos epoch (UTC) a (año, mes, día, hora, minuto). Algoritmo de
/// días-civiles de Howard Hinnant (sin librerías de fecha). Asume calendario
/// gregoriano proléptico.
fn civil_from_epoch(secs: u64) -> (i64, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = (rem / 3_600) as u32;
    let minute = ((rem % 3_600) / 60) as u32;

    // Hinnant civil_from_days: días desde 1970-01-01.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, hour, minute)
}
```
NOTE: `civil_from_epoch` returns year as `i64` (Hinnant's algorithm is general); for our epoch-based use it's always positive. The `format!("{y:04}")` handles it. If clippy complains about `i64`→`format` width, it's fine.

- [ ] **Step 4: Correr — pasan**

Run: `cargo test -p naygo-core clipboard::naming` → 5 PASS.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 5: Commit**
```
git add crates/core/src/clipboard/naming.rs
git commit -m "feat(core): expand_name_template ({fecha} sin chrono)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `expand_name_template(template: &str, now_secs: u64) -> String` EXACTLY (Task 4 depends).

---

## Task 3: `encode_image` — PNG/JPG (requiere `image` en core)

**Files:**
- Modify: `crates/core/Cargo.toml`
- Modify: `crates/core/src/clipboard/encode.rs`

- [ ] **Step 1: Añadir `image` a core (NO quitarlo de ui todavía)**

Modify `crates/core/Cargo.toml`: in `[dependencies]`, add:
```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
```
(Leave ui's `image` in place for now — both crates can depend on it temporarily; ui's is removed in Task 6. This keeps every intermediate state compiling.)

- [ ] **Step 2: Tests (TDD)**

Replace `crates/core/src/clipboard/encode.rs` with header + tests:
```rust
// Naygo — codificación de imagen del portapapeles a PNG/JPG (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Codifica una `ClipboardImage` (RGBA8) a bytes PNG o JPG en memoria, vía el crate
//! `image`. Sin Windows. PNG es sin pérdida; JPG usa `jpg_quality` (1..=100).

use super::{ClipboardImage, ImageFmt};

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(w: u32, h: u32) -> ClipboardImage {
        // Patrón RGBA simple: rojo opaco.
        let rgba = (0..(w * h)).flat_map(|_| [255u8, 0, 0, 255]).collect();
        ClipboardImage { width: w, height: h, rgba }
    }

    #[test]
    fn png_round_trip_conserva_dimensiones() {
        let img = sample(4, 3);
        let bytes = encode_image(&img, ImageFmt::Png, 90).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (4, 3));
        // PNG es sin pérdida: el primer píxel sigue rojo opaco.
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255]);
    }

    #[test]
    fn jpg_round_trip_conserva_dimensiones() {
        let img = sample(8, 8);
        let bytes = encode_image(&img, ImageFmt::Jpg, 85).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.to_rgba8().dimensions(), (8, 8));
        // JPG es con pérdida: no comparamos píxeles exactos.
    }

    #[test]
    fn error_si_rgba_inconsistente() {
        let bad = ClipboardImage { width: 2, height: 2, rgba: vec![0, 0, 0] }; // != 2*2*4
        assert!(encode_image(&bad, ImageFmt::Png, 90).is_err());
    }
}
```

- [ ] **Step 3: Correr y ver fallar**

Run: `cargo test -p naygo-core clipboard::encode` → ERROR: `encode_image`/`EncodeError` no existen.

- [ ] **Step 4: Implementar (en encode.rs, antes del mod tests)**
```rust
use std::io::Cursor;

/// Error al codificar una imagen.
#[derive(Debug)]
pub enum EncodeError {
    /// La longitud de `rgba` no coincide con `width * height * 4`.
    BadBuffer,
    /// El codificador del crate `image` falló.
    Encode(String),
}

/// Codifica `img` (RGBA8) a bytes PNG o JPG. `jpg_quality` (1..=100) solo aplica a JPG.
pub fn encode_image(
    img: &ClipboardImage,
    fmt: ImageFmt,
    jpg_quality: u8,
) -> Result<Vec<u8>, EncodeError> {
    let expected = img.width as usize * img.height as usize * 4;
    if img.rgba.len() != expected {
        return Err(EncodeError::BadBuffer);
    }
    let buf = image::RgbaImage::from_raw(img.width, img.height, img.rgba.clone())
        .ok_or(EncodeError::BadBuffer)?;
    let mut out = Cursor::new(Vec::new());
    match fmt {
        ImageFmt::Png => {
            image::DynamicImage::ImageRgba8(buf)
                .write_to(&mut out, image::ImageFormat::Png)
                .map_err(|e| EncodeError::Encode(e.to_string()))?;
        }
        ImageFmt::Jpg => {
            // JPEG no soporta alfa: convertir a RGB8. La calidad va por el encoder.
            let rgb = image::DynamicImage::ImageRgba8(buf).to_rgb8();
            let q = jpg_quality.clamp(1, 100);
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q);
            enc.encode_image(&rgb)
                .map_err(|e| EncodeError::Encode(e.to_string()))?;
        }
    }
    Ok(out.into_inner())
}
```
NOTE: verify the `image` 0.25 API names: `image::RgbaImage::from_raw`, `image::DynamicImage::ImageRgba8`, `write_to(&mut Cursor, image::ImageFormat::Png)`, `image::codecs::jpeg::JpegEncoder::new_with_quality(w, q)` + `encode_image(&rgb)`. These match image 0.25. If a name differs, adapt to the actual 0.25 API (check with `cargo doc` or compiler errors).

- [ ] **Step 5: Correr — pasan**

Run: `cargo test -p naygo-core clipboard::encode` → 3 PASS.
Run: `cargo test -p naygo-core` → green.
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 6: Commit**
```
git add crates/core/Cargo.toml crates/core/src/clipboard/encode.rs Cargo.lock
git commit -m "feat(core): encode_image (PNG/JPG) + image en core

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `encode_image(&ClipboardImage, ImageFmt, u8) -> Result<Vec<u8>, EncodeError>` EXACTLY (Task 6 depends).

---

## Task 4: Settings de paste + `decide_paste`

**Files:**
- Modify: `crates/core/src/config/mod.rs`
- Modify: `crates/core/src/clipboard/mod.rs`

- [ ] **Step 1: Añadir Settings de paste (patrón aditivo)**

Modify `crates/core/src/config/mod.rs`. In `struct Settings`, after the last field, add (each with the additive serde-default pattern the file already uses):
```rust
    /// Pegar texto/imagen: pedir confirmación de nombre antes de crear (modo B).
    /// `false` (default) = crear directo con nombre automático (modo A).
    #[serde(default = "default_paste_confirm")]
    pub paste_confirm: bool,
    /// Plantilla de nombre para un archivo de texto pegado. `{fecha}` → fecha/hora.
    #[serde(default = "default_paste_text_name")]
    pub paste_text_name: String,
    /// Extensión (sin punto) para texto pegado.
    #[serde(default = "default_paste_text_ext")]
    pub paste_text_ext: String,
    /// Plantilla de nombre para una imagen pegada. `{fecha}` → fecha/hora.
    #[serde(default = "default_paste_image_name")]
    pub paste_image_name: String,
    /// Formato de salida para imagen pegada.
    #[serde(default = "default_paste_image_fmt")]
    pub paste_image_fmt: crate::clipboard::ImageFmt,
    /// Calidad JPG (1..=100) para imagen pegada como JPG.
    #[serde(default = "default_paste_jpg_quality")]
    pub paste_jpg_quality: u8,
```
And add the default fns near the other `default_*` fns:
```rust
/// Default de `paste_confirm`: false (crear directo).
fn default_paste_confirm() -> bool {
    false
}
/// Default de `paste_text_name`.
fn default_paste_text_name() -> String {
    "pegado {fecha}".to_string()
}
/// Default de `paste_text_ext`.
fn default_paste_text_ext() -> String {
    "txt".to_string()
}
/// Default de `paste_image_name`.
fn default_paste_image_name() -> String {
    "captura {fecha}".to_string()
}
/// Default de `paste_image_fmt`: PNG.
fn default_paste_image_fmt() -> crate::clipboard::ImageFmt {
    crate::clipboard::ImageFmt::Png
}
/// Default de `paste_jpg_quality`: 90.
fn default_paste_jpg_quality() -> u8 {
    90
}
```
IMPORTANT: if `Settings` has a manual `Default` impl (not derived), add these fields there too with the same default values. Check the file. CONFIG_VERSION stays 1 (these are additive serde-default fields, retro-compatible).

- [ ] **Step 2: Test de decide_paste (TDD)**

Add to the `#[cfg(test)] mod tests` of `crates/core/src/clipboard/mod.rs`:
```rust
    use crate::config::Settings;
    use std::collections::HashSet;

    fn settings() -> Settings {
        Settings::default()
    }

    // exists closure: nada existe.
    fn none_exists(_: &Path) -> bool {
        false
    }

    #[test]
    fn files_a_transfer() {
        let c = ClipboardContent::Files {
            paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
            cut: true,
        };
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &none_exists);
        assert_eq!(
            plan,
            PastePlan::Transfer {
                paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
                cut: true
            }
        );
    }

    #[test]
    fn text_a_create_text_con_nombre_plantilla() {
        let c = ClipboardContent::Text("hola".into());
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &none_exists);
        match plan {
            PastePlan::CreateText { path, body } => {
                assert_eq!(body, "hola");
                // "pegado {fecha}" con epoch 0 → "pegado 1970-01-01 00-00.txt"
                assert_eq!(path, Path::new("D:/dst/pegado 1970-01-01 00-00.txt"));
            }
            other => panic!("esperaba CreateText, vino {other:?}"),
        }
    }

    #[test]
    fn text_dedup_si_existe() {
        let taken: HashSet<PathBuf> =
            [PathBuf::from("D:/dst/pegado 1970-01-01 00-00.txt")].into_iter().collect();
        let exists = |p: &Path| taken.contains(p);
        let c = ClipboardContent::Text("x".into());
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &exists);
        match plan {
            PastePlan::CreateText { path, .. } => {
                assert_eq!(path, Path::new("D:/dst/pegado 1970-01-01 00-00 (2).txt"));
            }
            other => panic!("esperaba CreateText dedup, vino {other:?}"),
        }
    }

    #[test]
    fn image_a_create_image() {
        let img = ClipboardImage { width: 2, height: 2, rgba: vec![0u8; 16] };
        let c = ClipboardContent::Image(img.clone());
        let plan = decide_paste(&c, Path::new("D:/dst"), &settings(), 0, &none_exists);
        match plan {
            PastePlan::CreateImage { path, fmt, img: got } => {
                assert_eq!(fmt, ImageFmt::Png);
                assert_eq!(got, img);
                assert_eq!(path, Path::new("D:/dst/captura 1970-01-01 00-00.png"));
            }
            other => panic!("esperaba CreateImage, vino {other:?}"),
        }
    }

    #[test]
    fn image_dims_absurdas_a_nothing() {
        // rgba inconsistente con dims.
        let bad = ClipboardImage { width: 2, height: 2, rgba: vec![0u8; 3] };
        let plan = decide_paste(&ClipboardContent::Image(bad), Path::new("D:/dst"), &settings(), 0, &none_exists);
        assert_eq!(plan, PastePlan::Nothing);
        // dims cero.
        let zero = ClipboardImage { width: 0, height: 5, rgba: vec![] };
        let plan2 = decide_paste(&ClipboardContent::Image(zero), Path::new("D:/dst"), &settings(), 0, &none_exists);
        assert_eq!(plan2, PastePlan::Nothing);
    }

    #[test]
    fn empty_a_nothing() {
        let plan = decide_paste(&ClipboardContent::Empty, Path::new("D:/dst"), &settings(), 0, &none_exists);
        assert_eq!(plan, PastePlan::Nothing);
    }
```

- [ ] **Step 3: Correr y ver fallar**

Run: `cargo test -p naygo-core clipboard` → ERROR: `decide_paste` no existe.

- [ ] **Step 4: Implementar `decide_paste` (en clipboard/mod.rs, antes del mod tests)**
```rust
use crate::clipboard::naming::expand_name_template;
use crate::config::Settings;

/// Decide la acción de pegado a partir del contenido del portapapeles + config.
/// Puro: `exists` consulta si una ruta ya existe (en producción, el FS; en tests, un
/// closure). `now_secs` alimenta la expansión de `{fecha}` en los nombres.
pub fn decide_paste(
    content: &ClipboardContent,
    dest_dir: &Path,
    settings: &Settings,
    now_secs: u64,
    exists: &dyn Fn(&Path) -> bool,
) -> PastePlan {
    match content {
        ClipboardContent::Files { paths, cut } => {
            if paths.is_empty() {
                return PastePlan::Nothing;
            }
            PastePlan::Transfer { paths: paths.clone(), cut: *cut }
        }
        ClipboardContent::Text(body) => {
            let stem = expand_name_template(&settings.paste_text_name, now_secs);
            let name = format!("{stem}.{}", settings.paste_text_ext);
            let path = dedup_name(&dest_dir.join(name), exists);
            PastePlan::CreateText { path, body: body.clone() }
        }
        ClipboardContent::Image(img) => {
            // Validación estricta: dims no-cero, dentro del tope, rgba consistente.
            let pixels = img.width as u64 * img.height as u64;
            let ok = img.width > 0
                && img.height > 0
                && pixels <= MAX_IMAGE_PIXELS
                && img.rgba.len() as u64 == pixels * 4;
            if !ok {
                return PastePlan::Nothing;
            }
            let fmt = settings.paste_image_fmt;
            let stem = expand_name_template(&settings.paste_image_name, now_secs);
            let name = format!("{stem}.{}", fmt.ext());
            let path = dedup_name(&dest_dir.join(name), exists);
            PastePlan::CreateImage { path, fmt, img: img.clone() }
        }
        ClipboardContent::Empty => PastePlan::Nothing,
    }
}
```

- [ ] **Step 5: Correr — pasan**

Run: `cargo test -p naygo-core clipboard` → all PASS (types + naming + encode + decide).
Run: `cargo test -p naygo-core` → green (incl. config serde/parity if any).
Run: `cargo clippy -p naygo-core --all-targets -- -D warnings` → clean.

- [ ] **Step 6: Commit**
```
git add crates/core/src/config/mod.rs crates/core/src/clipboard/mod.rs
git commit -m "feat(core): Settings de paste + decide_paste (decisión pura)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `decide_paste(content, dest_dir, settings, now_secs, exists) -> PastePlan` and the new Settings field names EXACTLY (Tasks 6-7 depend).

---

## Task 5: `platform::clipboard` — read/write Win32

**Files:**
- Create: `crates/platform/src/clipboard.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/Cargo.toml`

- [ ] **Step 1: Añadir features de `windows`**

Modify `crates/platform/Cargo.toml`: add to the `windows` features list:
```toml
    "Win32_System_DataExchange",
    "Win32_System_Memory",
    "Win32_System_Ole",
    "Win32_Graphics_Gdi",
```
(`Win32_UI_Shell` and `Win32_Foundation` are already present — `DragQueryFileW`/`HDROP` live in Shell; `CF_*`/clipboard fns in DataExchange; `GlobalAlloc`/`GlobalLock` in Memory; `BITMAPINFO` in Gdi.)

- [ ] **Step 2: Crear el módulo con la API + stub no-windows**

Create `crates/platform/src/clipboard.rs`:
```rust
// Naygo — portapapeles del SO (Win32: CF_HDROP, CF_DIB, CF_UNICODETEXT), aislado.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lee y escribe el portapapeles del sistema. La lógica de QUÉ hacer con el contenido
//! vive en `core::clipboard`; aquí solo está la frontera Win32. Tolerante: cualquier
//! fallo de lectura → `Empty`; la escritura devuelve `Result`. No tumba el proceso.

use naygo_core::clipboard::ClipboardContent;
use std::path::PathBuf;

/// Error al escribir el portapapeles.
#[derive(Debug)]
pub enum ClipboardError {
    /// No soportado en esta plataforma.
    NotSupported,
    /// La operación Win32 falló (el mensaje describe el error).
    Failed(String),
}

#[cfg(not(windows))]
pub fn read() -> ClipboardContent {
    ClipboardContent::Empty
}

#[cfg(not(windows))]
pub fn write_files(_paths: &[PathBuf], _cut: bool) -> Result<(), ClipboardError> {
    Err(ClipboardError::NotSupported)
}

#[cfg(windows)]
pub fn read() -> ClipboardContent {
    windows_impl::read()
}

#[cfg(windows)]
pub fn write_files(paths: &[PathBuf], cut: bool) -> Result<(), ClipboardError> {
    windows_impl::write_files(paths, cut)
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use naygo_core::clipboard::ClipboardImage;
    // ... imports de windows::Win32::{System::DataExchange, System::Memory,
    //     UI::Shell, Graphics::Gdi, Foundation} según se usen.

    pub fn read() -> ClipboardContent {
        // 1. OpenClipboard(None) con un par de reintentos cortos si falla (otra app
        //    lo tiene abierto). Si no se logra, ClipboardContent::Empty.
        // 2. Prioridad: IsClipboardFormatAvailable(CF_HDROP) → leer archivos +
        //    Preferred DropEffect (RegisterClipboardFormatW("Preferred DropEffect"),
        //    GetClipboardData, GlobalLock, leer DWORD: DROPEFFECT_MOVE=2 → cut=true).
        //    Si no, CF_DIB → reconstruir ClipboardImage (ver dib_to_rgba). Si no,
        //    CF_UNICODETEXT → String. Si nada, Empty.
        // 3. CloseClipboard SIEMPRE (incluso en caminos de error).
        // Implementar con cuidado de GlobalLock/GlobalUnlock balanceados.
        ClipboardContent::Empty // <- reemplazar por la implementación real
    }

    pub fn write_files(paths: &[PathBuf], cut: bool) -> Result<(), ClipboardError> {
        // 1. OpenClipboard(None) + EmptyClipboard.
        // 2. Construir DROPFILES + lista de rutas UTF-16 doble-NUL-terminada en un
        //    HGLOBAL (GMEM_MOVEABLE), SetClipboardData(CF_HDROP, hglobal).
        // 3. Segundo HGLOBAL con un DWORD = DROPEFFECT_COPY(1) o DROPEFFECT_MOVE(2),
        //    SetClipboardData(RegisterClipboardFormatW("Preferred DropEffect"), h).
        // 4. CloseClipboard. Devolver Ok(()) o ClipboardError::Failed(msg).
        let _ = (paths, cut);
        Err(ClipboardError::Failed("no implementado".into())) // <- reemplazar
    }

    /// Reconstruye RGBA8 desde un CF_DIB (BITMAPINFOHEADER + bits). Maneja 24/32 bpp
    /// y orientación bottom-up/top-down. Devuelve None si el formato no se soporta.
    fn dib_to_rgba(_dib: &[u8]) -> Option<ClipboardImage> {
        None // <- reemplazar
    }
}
```
THIS IS A SKELETON. The implementer MUST fill the three Win32 functions for real, following `trash.rs` for the `windows` 0.62 calling conventions (`OpenClipboard`, `GetClipboardData`, `GlobalLock`/`GlobalUnlock`, `DragQueryFileW`, `SetClipboardData`, `GlobalAlloc`, `RegisterClipboardFormatW`, `CloseClipboard`). Key correctness points:
- `OpenClipboard(None)` returns `Result`; retry up to ~5 times with a tiny spin if it fails, else return `Empty`/`Failed`.
- Always `CloseClipboard()` on every path (use a scope guard or careful early-returns).
- For CF_HDROP read: `GetClipboardData(CF_HDROP)` → `HDROP` → `DragQueryFileW(hdrop, 0xFFFFFFFF, None)` for count, then per-index to get each path.
- Preferred DropEffect: `RegisterClipboardFormatW(w!("Preferred DropEffect"))` → if present, `GlobalLock` the handle, read first `u32`, `cut = (effect & DROPEFFECT_MOVE) != 0`.
- For CF_DIB: the handle is a `BITMAPINFO` followed by pixel bits. Parse `biWidth`/`biHeight` (negative height = top-down), `biBitCount` (handle 24 and 32), compute row stride padded to 4 bytes, convert BGR(A)→RGBA. Reject if `biWidth*biHeight` exceeds a sane cap.
- For write: `DROPFILES` struct (pFiles offset = sizeof(DROPFILES)=20, fWide=TRUE) then the UTF-16 paths each NUL-terminated and a final extra NUL. `GlobalAlloc(GMEM_MOVEABLE, ...)`, `GlobalLock`, write, `GlobalUnlock`, `SetClipboardData`. After `SetClipboardData` succeeds, the system owns the HGLOBAL (do NOT free it).

- [ ] **Step 3: Declarar el módulo**

Modify `crates/platform/src/lib.rs`: add `pub mod clipboard;`.

- [ ] **Step 4: Build + verify (compila en ambos targets; la verificación funcional real es manual)**

Run: `cargo build -p naygo-platform` → compiles on Windows (the real impl).
Run: `cargo clippy -p naygo-platform --all-targets -- -D warnings` → clean.
Run: `cargo build -p naygo-platform --target x86_64-unknown-linux-gnu` is NOT available on this machine; instead just confirm the `#[cfg(not(windows))]` stubs are syntactically present (they're compiled out on Windows but must exist for portability — they don't need a non-Windows toolchain to be correct by inspection).
MANUAL smoke test (do it, report result): write a tiny example or a `#[test]` gated `#[cfg(windows)]` that:
  - copies a known file to the clipboard via `write_files(&[path], false)`, then `read()` returns `Files{paths:[that path], cut:false}`.
  - NOTE: clipboard tests are flaky under parallel test runners (global resource). Mark such a test `#[ignore]` with a comment, run it explicitly with `cargo test -p naygo-platform clipboard_roundtrip -- --ignored --test-threads=1`, and report the result. Do not leave a non-ignored clipboard test that could flake CI.

- [ ] **Step 5: fmt + commit**

Run `cargo fmt --all`.
```
git add crates/platform/src/clipboard.rs crates/platform/src/lib.rs crates/platform/Cargo.toml Cargo.lock
git commit -m "feat(platform): portapapeles del SO (read/write_files Win32)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

Keep `read() -> ClipboardContent` and `write_files(&[PathBuf], bool) -> Result<(), ClipboardError>` EXACTLY (Task 6 depends). If the Win32 impl proves too large/risky to complete confidently, report DONE_WITH_CONCERNS detailing exactly which of read/write/dib are stubbed, so the controller can dispatch a focused follow-up — do NOT silently leave a stub that returns Empty.

---

## Task 6: UI — Ctrl+C/X/V reescritos + quitar InternalClipboard + mover image fuera de ui

**Files:**
- Modify: `crates/ui/Cargo.toml`
- Modify: `crates/ui/src/app.rs`

This task makes the cross-crate switch atomically: remove `InternalClipboard`, wire Ctrl+C/X to `platform::clipboard::write_files`, wire Ctrl+V to `read`+`decide_paste`+execute, and drop `image` from ui's Cargo (it's in core now). Everything compiles before and after; the broken intermediate only exists within this single task/commit.

- [ ] **Step 1: Quitar `image` de ui Cargo**

Modify `crates/ui/Cargo.toml`: remove the line `image = { version = "0.25", default-features = false, features = ["png"] }`. (Grep ui's source for direct `image::` uses first — `grep -rn "image::" crates/ui/src`. If ui uses `image` directly anywhere besides the paste path we're adding, it must now go through `naygo_core` re-exports or keep a thin use. The plan assumes ui only needs `image` for the new paste encode, which lives in core. If grep finds other `image::` uses, report NEEDS_CONTEXT before proceeding.)

- [ ] **Step 2: Quitar `InternalClipboard`**

Modify `crates/ui/src/app.rs`:
- Delete the `struct InternalClipboard { paths, cut }` (~line 57) and its `#[derive(Default)]`.
- Remove the `clipboard: InternalClipboard` field from `NaygoApp` (~126) and its initializer in `new` (~194).

- [ ] **Step 3: Rewire `clipboard_set` (Ctrl+C/X) to the OS clipboard**

Replace `clipboard_set` (~901):
```rust
    /// Copia/corta la selección al portapapeles del SISTEMA (CF_HDROP + DropEffect),
    /// para interoperar con el Explorador de Windows y otras apps.
    fn clipboard_set(&mut self, cut: bool) {
        let paths = self.selected_paths();
        if paths.is_empty() {
            return;
        }
        if let Err(e) = naygo_platform::clipboard::write_files(&paths, cut) {
            self.status = format!("{:?}", e); // error discreto; clave i18n abajo si se prefiere
        }
    }
```
(Optional: use an i18n key `clipboard.write_error` instead of `{:?}`. Add it in Task 7's i18n if desired; for now a discreet status is acceptable.)

- [ ] **Step 4: Rewrite `paste` (Ctrl+V) — read OS clipboard + decide + execute**

Replace `paste` (~944) with:
```rust
    /// Pega el portapapeles del SISTEMA en la carpeta activa según su tipo:
    /// archivos → copiar/mover (motor de ops); texto → .txt; imagen → png/jpg.
    fn paste(&mut self) {
        let Some(dest) = self.active_dir() else {
            return;
        };
        let content = naygo_platform::clipboard::read();
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let exists = |p: &std::path::Path| p.exists();
        let plan = naygo_core::clipboard::decide_paste(
            &content, &dest, &self.settings, now_secs, &exists,
        );
        use naygo_core::clipboard::PastePlan;
        match plan {
            PastePlan::Transfer { paths, cut } => {
                let req = crate::ops_actions::transfer(cut, paths, dest.clone());
                let verb = if cut { self.i18n.t("op.cut") } else { self.i18n.t("op.paste") };
                let label = format!("{verb} → {}", dest.display());
                self.launch_transfer(req, label);
            }
            PastePlan::CreateText { path, body } => {
                if self.settings.paste_confirm {
                    self.pending_dialog = Some(crate::ops_dialogs::paste_preview_text(path, body));
                } else {
                    self.write_pasted_file(WriteJob::Text { path, body });
                }
            }
            PastePlan::CreateImage { path, fmt, img } => {
                if self.settings.paste_confirm {
                    self.pending_dialog =
                        Some(crate::ops_dialogs::paste_preview_image(path, fmt, img));
                } else {
                    self.write_pasted_file(WriteJob::Image {
                        path,
                        fmt,
                        img,
                        quality: self.settings.paste_jpg_quality,
                    });
                }
            }
            PastePlan::Nothing => {
                self.status = self.i18n.t("paste.empty");
            }
        }
    }
```
NOTE: `paste_preview_text`/`paste_preview_image` construct a `PendingDialog::PastePreview{...}` (Task 7 adds the variant + the modal). `write_pasted_file` + `WriteJob` are added in the next step. The mini-dialog path is finished in Task 7; for THIS task, to keep it compiling, you may temporarily route `paste_confirm == true` through `write_pasted_file` too (direct), with a `// TODO Task 7: mini-diálogo` — BUT prefer to just add the `PendingDialog::PastePreview` variant and the constructors as part of this task if straightforward. Decide based on what keeps the commit cohesive; if you defer the dialog to Task 7, make the deferral explicit and ensure `paste_confirm` still does something safe (direct write).

- [ ] **Step 5: Add the short worker that writes the pasted file**

Add to `impl NaygoApp` (near `start_op`):
```rust
    /// Escribe un archivo pegado (texto o imagen) en un worker corto: el hilo de UI
    /// nunca escribe a disco. Al terminar, refresca el panel y deja status; en error,
    /// status discreto. (No usa el OpPlan de ops: es un único archivo en memoria.)
    fn write_pasted_file(&mut self, job: WriteJob) {
        let i18n_ok_text = self.i18n.t("paste.done_text");
        let i18n_ok_img = self.i18n.t("paste.done_image");
        let i18n_err = self.i18n.t("paste.error");
        let dir = job.path().parent().map(|p| p.to_path_buf());
        // Worker corto: codifica (imagen) y escribe; reporta por canal.
        let (tx, rx) = std::sync::mpsc::channel::<Result<PasteOk, String>>();
        std::thread::spawn(move || {
            let result = job.run();
            let _ = tx.send(result);
        });
        // Bloqueo mínimo: la escritura de un archivo es rápida. Para no introducir un
        // nuevo canal vivo en el loop, esperamos el resultado aquí (es ms). Si se
        // quisiera no bloquear nunca, se mete `rx` en un Vec drenado en pump; por
        // ahora, dado el tamaño, el join inmediato es aceptable y mantiene la UI simple.
        match rx.recv() {
            Ok(Ok(ok)) => {
                self.status = match ok {
                    PasteOk::Text { bytes, chars, lines } => i18n_ok_text
                        .replace("{bytes}", &human_size(bytes))
                        .replace("{chars}", &chars.to_string())
                        .replace("{lines}", &lines.to_string()),
                    PasteOk::Image { w, h, fmt, bytes } => i18n_ok_img
                        .replace("{w}", &w.to_string())
                        .replace("{h}", &h.to_string())
                        .replace("{fmt}", fmt)
                        .replace("{bytes}", &human_size(bytes)),
                };
                if let (Some(id), Some(d)) = (self.workspace.active_id(), dir) {
                    self.refresh_pane(id, d);
                }
            }
            _ => self.status = i18n_err,
        }
    }
```
And the `WriteJob`/`PasteOk` types (top of app.rs or a small new module `crates/ui/src/paste_job.rs`):
```rust
/// Trabajo de escritura de un archivo pegado (corre en un worker).
enum WriteJob {
    Text { path: std::path::PathBuf, body: String },
    Image { path: std::path::PathBuf, fmt: naygo_core::clipboard::ImageFmt, img: naygo_core::clipboard::ClipboardImage, quality: u8 },
}
/// Resumen del archivo escrito (para el toast/status).
enum PasteOk {
    Text { bytes: u64, chars: usize, lines: usize },
    Image { w: u32, h: u32, fmt: &'static str, bytes: u64 },
}
impl WriteJob {
    fn path(&self) -> &std::path::Path {
        match self { WriteJob::Text { path, .. } | WriteJob::Image { path, .. } => path }
    }
    fn run(self) -> Result<PasteOk, String> {
        match self {
            WriteJob::Text { path, body } => {
                let chars = body.chars().count();
                let lines = body.lines().count().max(if body.is_empty() { 0 } else { 1 });
                std::fs::write(&path, &body).map_err(|e| e.to_string())?;
                Ok(PasteOk::Text { bytes: body.len() as u64, chars, lines })
            }
            WriteJob::Image { path, fmt, img, quality } => {
                let (w, h) = (img.width, img.height);
                let bytes = naygo_core::clipboard::encode::encode_image(&img, fmt, quality)
                    .map_err(|e| format!("{e:?}"))?;
                let len = bytes.len() as u64;
                std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                Ok(PasteOk::Image { w, h, fmt: fmt.ext(), bytes: len })
            }
        }
    }
}
```
NOTE: `human_size` is `pub(crate)` in `file_panel` (made so in ops-A) — import it (`use crate::file_panel::human_size;`) or qualify it. `refresh_pane`/`workspace.active_id` exist (used elsewhere in app.rs). `self.settings` holds the Settings. `encode_image` is at `naygo_core::clipboard::encode::encode_image` (re-export it from `clipboard::mod` if you prefer `naygo_core::clipboard::encode_image` — add `pub use encode::encode_image;` to clipboard/mod.rs and adjust). Keep one canonical path.

- [ ] **Step 6: i18n keys used here exist or are added in Task 7**

This task references i18n keys `paste.empty`, `paste.done_text`, `paste.done_image`, `paste.error` (and reuses existing `op.cut`/`op.paste`). Task 7 adds them. To keep THIS task building/running, add them in this task too (or coordinate): simplest is to add the 4 keys to es.json/en.json now (Step 7 of this task) so the build is green. (Do it here; Task 7 then only adds the dialog keys.)

Add to `crates/core/src/i18n/es.json` and `en.json` (same keys both, values per language):
ES: `"paste.empty": "Portapapeles vacío"`, `"paste.done_text": "Texto pegado · {bytes} · {chars} caracteres · {lines} líneas"`, `"paste.done_image": "Imagen pegada · {w}×{h} · {fmt} · {bytes}"`, `"paste.error": "No se pudo pegar"`.
EN: `"paste.empty": "Clipboard empty"`, `"paste.done_text": "Text pasted · {bytes} · {chars} chars · {lines} lines"`, `"paste.done_image": "Image pasted · {w}×{h} · {fmt} · {bytes}"`, `"paste.error": "Could not paste"`.

- [ ] **Step 7: Build, lint, fmt, verify the whole workspace compiles**

Run: `cargo build --workspace` → compiles (InternalClipboard gone, paste rewritten, image only in core).
Run: `cargo test --workspace` → green.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all` then `cargo fmt --all -- --check` → clean.
MANUAL: build `--release -p naygo-ui`; run; Ctrl+C a file in Naygo → paste in Explorer works; copy text in another app → Ctrl+V in Naygo creates a .txt; copy an image (e.g. screenshot) → Ctrl+V creates a .png.

- [ ] **Step 8: Commit**
```
git add crates/ui/Cargo.toml crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/core/src/clipboard/mod.rs Cargo.lock
git commit -m "feat(ui): Ctrl+C/X/V usan el portapapeles del SO (paste inteligente)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Mini-diálogo de confirmación (modo B) + i18n del diálogo

**Files:**
- Modify: `crates/ui/src/ops_dialogs.rs`
- Modify: `crates/ui/src/app.rs`
- Modify: `crates/core/src/i18n/{es,en}.json`

Only needed if Task 6 deferred the `paste_confirm` mini-dialog. Adds `PendingDialog::PastePreview` + the modal + the two constructor helpers.

- [ ] **Step 1: i18n del diálogo**

Add to es.json/en.json (same keys both):
ES: `"paste.preview_text_title": "Pegar texto como archivo"`, `"paste.preview_image_title": "Pegar imagen como archivo"`, `"paste.name_label": "Nombre del archivo"`, `"paste.create": "Crear"`, `"paste.cancel": "Cancelar"`, `"paste.fmt_png": "PNG"`, `"paste.fmt_jpg": "JPG"`.
EN: `"paste.preview_text_title": "Paste text as file"`, `"paste.preview_image_title": "Paste image as file"`, `"paste.name_label": "File name"`, `"paste.create": "Create"`, `"paste.cancel": "Cancel"`, `"paste.fmt_png": "PNG"`, `"paste.fmt_jpg": "JPG"`.

- [ ] **Step 2: `PendingDialog::PastePreview` variant + constructors**

In `app.rs`, add to the `PendingDialog` enum:
```rust
    /// Confirmar nombre/formato antes de crear un archivo pegado (modo B).
    PastePreview {
        path: std::path::PathBuf,
        /// El cuerpo a escribir, o la imagen a codificar.
        kind: PastePreviewKind,
        /// Nombre editable (sin la extensión, que va aparte para imagen).
        name_buf: String,
    },
```
And:
```rust
enum PastePreviewKind {
    Text { body: String, ext: String },
    Image { fmt: naygo_core::clipboard::ImageFmt, img: naygo_core::clipboard::ClipboardImage, quality: u8 },
}
```
In `ops_dialogs.rs`, add constructors that `app.rs` calls (referenced in Task 6 Step 4):
```rust
pub fn paste_preview_text(path: std::path::PathBuf, body: String) -> crate::app::PendingDialog {
    let (stem, ext) = split_stem_ext(&path);
    crate::app::PendingDialog::PastePreview {
        path,
        kind: crate::app::PastePreviewKind::Text { body, ext },
        name_buf: stem,
    }
}
pub fn paste_preview_image(
    path: std::path::PathBuf,
    fmt: naygo_core::clipboard::ImageFmt,
    img: naygo_core::clipboard::ClipboardImage,
    quality: u8,
) -> crate::app::PendingDialog {
    let (stem, _ext) = split_stem_ext(&path);
    crate::app::PendingDialog::PastePreview {
        path,
        kind: crate::app::PastePreviewKind::Image { fmt, img, quality },
        name_buf: stem,
    }
}
```
NOTE: `PendingDialog`/`PastePreviewKind` must be `pub` (or `pub(crate)`) for ops_dialogs to construct them. Add `split_stem_ext(path) -> (String, String)` helper (file stem and extension as Strings). Adjust Task 6's `paste` to pass `quality` to `paste_preview_image` (it already has `self.settings.paste_jpg_quality`).

- [ ] **Step 3: Render the modal in `process_pending_dialog`**

In `process_pending_dialog` (app.rs), add a match arm for `PendingDialog::PastePreview`. Render via a new `ops_dialogs::paste_preview(...)` that shows: title (text/image), an editable name field, for image a small format selector (PNG/JPG radio) + dimensions label, and Create/Cancel buttons. On Create → rebuild the final path (`dir.join(format!("{name}.{ext}"))`, dedup again via `p.exists()`), then call `self.write_pasted_file(...)` with the chosen job; on Cancel → drop the dialog. Follow the EXACT egui::Modal pattern used by the existing dialogs in ops_dialogs.rs (read them; they bind the modal response and mutate locals).

Provide the signature:
```rust
pub enum PastePreviewOutcome { Create { path: std::path::PathBuf }, Cancel, Keep }
pub fn paste_preview(
    ctx: &egui::Context,
    i18n: &I18n,
    title_key: &str,
    name_buf: &mut String,
    ext: &str,
    image_dims: Option<(u32, u32)>,
    fmt: &mut Option<naygo_core::clipboard::ImageFmt>, // Some for image, None for text
) -> PastePreviewOutcome { /* egui::Modal ... */ }
```
The app arm owns the buffers (they live in the `PendingDialog`), calls `paste_preview`, and on `Create` constructs the `WriteJob` and calls `write_pasted_file`. Keep the modal non-dismissable-without-decision is NOT required here (Cancel is a valid decision; backdrop/Esc = Cancel is fine).

- [ ] **Step 4: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --all` + `--check`.
MANUAL: set `paste_confirm = true` (edit settings.json or via the Settings UI if wired) → Ctrl+V on text/image shows the dialog; editing the name + Create writes the file; Cancel does nothing.

- [ ] **Step 5: Commit**
```
git add crates/ui/src/ops_dialogs.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): mini-diálogo de confirmación al pegar texto/imagen (modo B)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Settings UI (exponer las opciones de paste) + toast de copia clicable

**Files:**
- Modify: `crates/ui/src/settings_window.rs` (o donde viva la ventana de Configuración)
- Modify: `crates/ui/src/ops_panel.rs` / `app.rs` (toast de copia clicable)
- Modify: `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Exponer las opciones de paste en Configuración**

Find the Settings window (grep `settings_window` or where `Settings` fields are edited). Add controls in an appropriate section (e.g. a "Pegar" group): a checkbox for `paste_confirm`, text inputs for `paste_text_name`/`paste_text_ext`/`paste_image_name`, a PNG/JPG selector for `paste_image_fmt`, and a slider/drag for `paste_jpg_quality` (1..=100, enabled only when fmt=Jpg). Mutate `self.settings.*`. i18n keys:
ES: `"settings.paste": "Pegar"`, `"settings.paste_confirm": "Confirmar nombre antes de crear"`, `"settings.paste_text_name": "Nombre para texto"`, `"settings.paste_text_ext": "Extensión de texto"`, `"settings.paste_image_name": "Nombre para imagen"`, `"settings.paste_image_fmt": "Formato de imagen"`, `"settings.paste_jpg_quality": "Calidad JPG"`.
EN: `"settings.paste": "Paste"`, `"settings.paste_confirm": "Confirm name before creating"`, `"settings.paste_text_name": "Text name"`, `"settings.paste_text_ext": "Text extension"`, `"settings.paste_image_name": "Image name"`, `"settings.paste_image_fmt": "Image format"`, `"settings.paste_jpg_quality": "JPG quality"`.
Save settings on change (the Settings window already persists — follow its pattern).

- [ ] **Step 2: Toast de copia clicable → abrir/expandir el panel de operaciones**

The spec asks: when pasting FILES (a Transfer), the "copying N files" indicator should be clickable to open/expand the ops panel. The ops panel already exists (ops-A). Find where the ops panel/its compact line renders (ops_panel.rs). Ensure: when an op is active, clicking its compact line/toast expands the detail (the expand toggle already exists in ops-A). If there's a status-line "copiando…" that isn't already linked, make it `ui.button`/clickable that sets the ops panel to visible/expanded. If ops-A already shows the panel on operate (it does — "panel aparece al operar"), this may be a no-op or a tiny affordance; verify and, if already covered, note it and skip. Do NOT build a second indicator.

- [ ] **Step 3: Build, lint, fmt, manual**

Run: `cargo build --workspace`; `cargo test --workspace` → green; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all` + `--check`.
MANUAL: open Settings → "Pegar" section shows all options and editing them persists; pasting files shows the ops panel and clicking expands it.

- [ ] **Step 4: Commit**
```
git add crates/ui/src/settings_window.rs crates/ui/src/ops_panel.rs crates/ui/src/app.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): opciones de paste en Configuración + toast de copia abre el panel

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Cierre — README, verificación final, push

**Files:**
- Modify: `README.md`
- Verificación final + push

- [ ] **Step 1: README**

READ the current status block in `README.md` and replace it with:
```markdown
> **Estado:** Fase paste-inteligente (pegar el portapapeles del SO por tipo) en
> desarrollo. Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-paste-inteligente-design.md`](docs/superpowers/specs/2026-06-08-naygo-paste-inteligente-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-paste-inteligente.md`](docs/superpowers/plans/2026-06-08-naygo-paste-inteligente.md).
> Operaciones de archivo (ops-A), journal/retomar (ops-B) y bloque visual completos.
```

- [ ] **Step 2: Verificación final**

Run: `cargo build --workspace` → compiles.
Run: `cargo test --workspace` → green.
Run: `cargo clippy --workspace --all-targets -- -D warnings` → clean.
Run: `cargo fmt --all -- --check` → clean.
Run: `cargo build --release -p naygo-ui` → release compiles.
MANUAL end-to-end: Ctrl+C en Naygo → pegar en Explorer; copiar archivos en Explorer → Ctrl+V en Naygo copia; copiar texto → .txt; copiar imagen → .png; modo confirmar on/off.

- [ ] **Step 3: Commit y push**
```
git add README.md
git commit -m "chore: actualizar estado del README (fase paste-inteligente)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
git push -u origin feat/paste-inteligente
```

---

## Self-review (cobertura del spec)

| Requisito del spec | Tarea(s) |
|---|---|
| ClipboardContent/ClipboardImage/ImageFmt/PastePlan | 1 |
| expand_name_template ({fecha} sin chrono, now inyectable) | 2 |
| encode_image PNG/JPG + image a core | 3 |
| Settings de paste (6 campos serde default) | 4 |
| decide_paste (Files/Text/Image/Empty + dedup + validación dims) | 4 |
| platform::clipboard read() (prioridad HDROP>DIB>TEXT, DropEffect) | 5 |
| platform::clipboard write_files (HDROP + Preferred DropEffect) | 5 |
| Ctrl+C/X → write_files; quitar InternalClipboard | 6 |
| Ctrl+V → read + decide_paste + ejecutar (Transfer/CreateText/CreateImage/Nothing) | 6 |
| Escritura en worker corto (UI no bloquea I/O) | 6 |
| Toast enriquecido (texto: bytes/chars/líneas; imagen: dims/fmt/bytes) | 6 |
| Modo B mini-diálogo PastePreview | 7 |
| Opciones de paste en Configuración | 8 |
| Toast de copia clicable → panel de ops | 8 |
| i18n ES/EN | 6, 7, 8 |
| Papelera/DnD/HTML fuera de alcance | (no se tocan) |

**Notas de riesgo:**
- **Secuencia cross-crate** (Task 6): mover `image` y quitar `InternalClipboard` en UN commit; antes y después compila. Si UI usa `image::` fuera del paste → NEEDS_CONTEXT.
- **Win32 clipboard** (Task 5): es el grueso técnico; CF_DIB→RGBA (bottom-up/stride/bpp) y DROPFILES son delicados. Smoke test manual obligatorio; si queda parcial, DONE_WITH_CONCERNS con detalle (no stub silencioso).
- **`write_pasted_file` bloquea brevemente** (Task 6): `rx.recv()` espera el worker. Es un solo archivo (ms). Si se prefiere no bloquear nunca, mover `rx` a un Vec drenado en `pump_ops` — pero YAGNI por ahora; documentado en el código.
- **egui Modal** (Task 7): seguir el patrón EXACTO de ops_dialogs.rs.
- **Verificar API de `image` 0.25** (Task 3): `RgbaImage::from_raw`, `DynamicImage::ImageRgba8`, `write_to(.., ImageFormat::Png)`, `codecs::jpeg::JpegEncoder::new_with_quality`. Ajustar a la API real si difiere.
- **decide_paste recibe `now_secs` y `exists`**: la UI los inyecta (SystemTime + `p.exists()`); core queda puro y testeable.
```
