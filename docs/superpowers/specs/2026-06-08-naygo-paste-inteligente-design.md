# Naygo — Fase paste-inteligente: pegar el portapapeles del SO según su tipo (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-08. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Tercera fase del sprint de funcionalidad, sobre ops-A (operaciones) y ops-B (journal),
ambas mergeadas. Hoy `Ctrl+V` pega un **clipboard interno** de Naygo (`InternalClipboard`,
copiar/cortar entre paneles, ops-A). Esta fase lo reemplaza por leer el **portapapeles
del SISTEMA** y actuar según su contenido:

- **Archivos** (CF_HDROP) → copiar o mover a la carpeta activa (reusa el motor de ops-A,
  con su panel de operaciones y el journal de ops-B).
- **Texto** (CF_UNICODETEXT) → crear un archivo de texto (`.txt` por defecto, nombre y
  extensión configurables).
- **Imagen** (CF_DIB) → crear un archivo de imagen (PNG por defecto; PNG o JPG
  configurable).

Y `Ctrl+C` / `Ctrl+X` ahora **escriben los archivos al portapapeles del SO** (CF_HDROP +
Preferred DropEffect), de modo que Naygo interopera con el Explorador de Windows y otras
apps. El `InternalClipboard` paralelo se retira.

### Decisiones tomadas en el brainstorm

1. **Unificar en el portapapeles del SO.** Un solo `Ctrl+V`: lee el portapapeles del
   sistema y decide. `Ctrl+C`/`Ctrl+X` escriben al portapapeles del SO. Se elimina el
   `InternalClipboard`.
2. **Modo A+B configurable.** Por defecto (modo A) crea el archivo de texto/imagen
   DIRECTO con un nombre automático y muestra un toast enriquecido. Una opción de
   Configuración (modo B) activa un mini-diálogo para confirmar/editar el nombre (y, para
   imagen, vista previa + selector PNG/JPG) antes de crear. Los archivos siempre se copian
   directo (como Explorer), sin diálogo.
3. **Toast enriquecido (modo A).** Texto: nombre + tamaño + nº de caracteres + líneas.
   Imagen: miniatura + dimensiones (px) + formato + tamaño del archivo. Archivos: NO usan
   toast propio — usan el panel de operaciones de ops-A; el toast "copiando N archivos" es
   CLICABLE y abre/expande ese panel.
4. **`image` a `core`.** El crate `image` (hoy en `ui`, solo feature `png`) se mueve a
   `core` con features `png` + `jpeg`, para que la codificación de imagen sea lógica pura
   testeable junto a la decisión de paste.
5. **Formatos de imagen: solo PNG + JPG** (peso mínimo, ~99% de los casos; ampliable
   luego activando un feature, sin romper nada).
6. **Escritura directa en worker corto** para texto/imagen (no se mete por el `OpPlan`
   completo de ops-A; es un solo archivo de bytes en memoria). El hilo de UI nunca escribe.

### Qué entra en paste-inteligente

- `core::clipboard`: `ClipboardContent`, `ClipboardImage`, `PastePlan`, `ImageFmt`,
  `decide_paste()` (puro), `encode_image()` (puro), expansión de plantilla de nombre con
  `{fecha}` (puro, `now` inyectable).
- `platform::clipboard`: `read()` (Win32, prioridad Files>Image>Text) y
  `write_files(paths, cut)` (CF_HDROP + Preferred DropEffect).
- `ui`: `Ctrl+C/X` → `write_files`; `Ctrl+V` → `read` + `decide_paste` + ejecutar
  (Transfer vía motor ops-A; CreateText/CreateImage vía worker corto); modo confirmar
  (mini-diálogo); toast enriquecido; toast de copia clicable → panel de ops; retirar
  `InternalClipboard`.
- `core::config`: Settings de paste (confirmación, plantillas de nombre, extensión,
  formato de imagen, calidad JPG).
- i18n ES/EN.

### Qué NO entra

- Drag&drop COM/OLE con el SO (fase aparte).
- Pegar otros formatos (HTML enriquecido→.html, RTF, audio) — solo files/text/image.
- Formatos de imagen más allá de PNG/JPG.
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

Idea rectora: la **decisión** vive en `core` (pura, testeable); la **lectura Win32** vive
en `platform`; la **codificación de imagen** vive en `core` (junto a la decisión); la UI
orquesta y reusa el motor de ops-A.

### Capa `core::clipboard` (módulo nuevo, puro)

```rust
/// Contenido del portapapeles del SO, ya leído por platform y normalizado.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipboardContent {
    Files { paths: Vec<PathBuf>, cut: bool }, // CF_HDROP + Preferred DropEffect
    Text(String),                             // CF_UNICODETEXT
    Image(ClipboardImage),                    // CF_DIB → rgba8 + dims
    Empty,
}

/// Imagen del portapapeles: RGBA8 sin comprimir + dimensiones.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipboardImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>, // width*height*4
}

/// Formato de salida al pegar una imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFmt { Png, Jpg }

/// Qué hará el paste, decidido a partir del contenido + config. Puro.
#[derive(Clone, Debug, PartialEq)]
pub enum PastePlan {
    /// Transferencia de archivos → motor de ops-A.
    Transfer { paths: Vec<PathBuf>, cut: bool },
    /// Crear un .txt con `body` en `dest_dir/name`.
    CreateText { path: PathBuf, body: String },
    /// Crear una imagen en `dest_dir/name.ext` con el formato dado.
    CreateImage { path: PathBuf, fmt: ImageFmt, img: ClipboardImage },
    /// Nada que pegar (portapapeles vacío o sin representación usable).
    Nothing,
}
```

- **`decide_paste(content, dest_dir, settings, now, exists) -> PastePlan`** (puro):
  - `Files { paths, cut }` → `Transfer { paths, cut }`.
  - `Text(s)` → `CreateText { path, body: s }`, donde `path = dest_dir/<nombre dedup>`,
    nombre = plantilla `settings.paste_text_name` con `{fecha}` expandido por `now`, más
    la extensión `settings.paste_text_ext`. Dedup vía `dedup_name(candidate, exists)`.
  - `Image(img)` → valida dimensiones (rechaza si `width==0 || height==0` o
    `width*height > MAX_PIXELS` ~512 MP, o si `rgba.len() != width*height*4`) → si
    inválida, `Nothing`; si válida, `CreateImage { path, fmt: settings.paste_image_fmt,
    img }`, nombre análogo con `settings.paste_image_name` + extensión del formato.
  - `Empty` → `Nothing`.
  - `exists: &dyn Fn(&Path) -> bool` se inyecta (en producción consulta el FS; en tests es
    un closure) — igual patrón que `dedup_name` de ops-A.
- **`encode_image(img: &ClipboardImage, fmt: ImageFmt, jpg_quality: u8) -> Result<Vec<u8>,
  EncodeError>`** (puro): usa `image` para codificar el RGBA8 a PNG o JPG en memoria.
  PNG ignora `jpg_quality`. Devuelve los bytes listos para escribir.
- **`expand_name_template(template: &str, now_secs: u64) -> String`** (puro): expande
  `{fecha}` → `YYYY-MM-DD HH-MM` derivado de `now_secs` (epoch). SIN chrono: cálculo
  civil-from-days propio (algoritmo de Howard Hinnant, días→Y/M/D) + hora/min desde el
  resto. `now_secs` es UTC; se documenta como hora UTC (simple y determinista para tests).
  Otros tokens desconocidos se dejan literales.
- **`PASTE_PLAN`/errores**: `EncodeError { Unsupported, Encode(String) }`.

### Capa `platform::clipboard` (módulo Win32 nuevo)

- **`read() -> ClipboardContent`**: `OpenClipboard` (con reintento breve si está
  bloqueado), consulta formatos en **orden de prioridad Files > Image > Text**:
  - `IsClipboardFormatAvailable(CF_HDROP)` → `GetClipboardData` → `DragQueryFileW` para
    cada ruta; lee el Preferred DropEffect (`RegisterClipboardFormatW("Preferred
    DropEffect")`) para `cut = (effect == DROPEFFECT_MOVE)`. → `Files`.
  - `CF_DIB` → reconstruye `ClipboardImage` (BITMAPINFOHEADER → rgba8, manejando
    top-down/bottom-up y stride). → `Image`.
  - `CF_UNICODETEXT` → `String` UTF-16→UTF-8. → `Text`.
  - ninguno → `Empty`. Siempre `CloseClipboard`. Tolerante: cualquier fallo → `Empty`.
- **`write_files(paths: &[PathBuf], cut: bool) -> Result<(), ClipboardError>`**:
  `OpenClipboard` + `EmptyClipboard`, construye un `DROPFILES` + lista de rutas
  doble-NUL-terminada en `HGLOBAL`, `SetClipboardData(CF_HDROP, ...)`, y un segundo
  `HGLOBAL` con el DWORD DropEffect (`DROPEFFECT_COPY`/`MOVE`) en el formato "Preferred
  DropEffect". `CloseClipboard`. Best-effort tolerante.

Sigue el patrón de `platform::trash` (COM/Win32 con manejo de errores tipado, sin
propagar panics).

### Capa `core::config`

`Settings` gana (serde `#[serde(default = "...")]`, CONFIG_VERSION sigue 1):
`paste_confirm: bool` (false), `paste_text_name: String` ("pegado {fecha}"),
`paste_text_ext: String` ("txt"), `paste_image_name: String` ("captura {fecha}"),
`paste_image_fmt: ImageFmt` (Png), `paste_jpg_quality: u8` (90).

### Capa `ui`

- **`Ctrl+C`/`Ctrl+X`**: `clipboard_set` ahora llama `platform::clipboard::write_files(
  paths, cut)` en vez de poblar `InternalClipboard`. Se elimina el struct
  `InternalClipboard` y el campo `clipboard` de `NaygoApp`.
- **`Ctrl+V` (`paste`)**: `platform::clipboard::read()` → `core::clipboard::decide_paste(
  content, dest, &settings, now, &exists)` → ejecutar el `PastePlan`:
  - `Transfer` → arma `OpRequest` (copy/move) y `start_op` (motor ops-A + journal ops-B).
    Si la copia toma tiempo, el toast/entrada "copiando N archivos" es clicable → abre el
    panel de operaciones.
  - `CreateText`/`CreateImage`: si `paste_confirm` (modo B) → mini-diálogo (`PendingDialog`
    nuevo: `PastePreview`) con nombre editable + metadata (+ selector PNG/JPG para imagen);
    al aceptar, escribir. Si no (modo A) → escribir directo + toast enriquecido.
  - La escritura va a un **worker corto** (`std::thread`): para texto, `fs::write(path,
    body)`; para imagen, `encode_image` (en el worker, fuera del hilo de UI) + `fs::write`.
    Reporta éxito/error por canal; al éxito, refresca el panel activo y resalta el nuevo
    archivo; al error, status discreto.
- **Toast enriquecido**: componente nuevo o reuso del status; muestra la metadata por tipo.
- **Mini-diálogo `PastePreview`**: en `ops_dialogs.rs` (egui::Modal, patrón existente).

### Lo que NO cambia

El motor de ops-A, el panel de operaciones, el journal de ops-B, los diálogos de
conflicto/confirmación. Paste-inteligente los reusa.

---

## 3. Flujo de datos

**Copiar/Cortar:** selección → `Ctrl+C`/`Ctrl+X` → `platform::clipboard::write_files(
paths, cut)` al portapapeles del SO. (Interopera con Explorer.)

**Pegar:** `Ctrl+V` → `platform::clipboard::read()` → `ClipboardContent` (prioridad
Files>Image>Text) → `core::clipboard::decide_paste(...)` → `PastePlan`:
- `Transfer` → `OpRequest` → motor ops-A (panel + journal + cancelable); toast de copia
  clicable abre el panel.
- `CreateText`/`CreateImage` → modo directo: worker escribe + toast enriquecido; modo
  confirmar: mini-diálogo → al aceptar, worker escribe.
- `Nothing` → status discreto.
Tras escribir/copiar → refrescar panel activo + resaltar nuevo archivo.

---

## 4. Manejo de errores / casos límite

- **Portapapeles vacío / sin formato usable** → `Nothing`, status discreto. No crashea.
- **Colisión de nombre** → `dedup_name` → `pegado ... (2).txt`. Siempre.
- **CF_HDROP con rutas inexistentes** → el motor de ops-A las registra como `Failed`.
- **DIB corrupto / dimensiones absurdas** → `decide_paste` valida (0, > MAX_PIXELS, o
  `rgba.len()` inconsistente) → `Nothing`; nunca asigna gigabytes a ciegas.
- **Portapapeles bloqueado por otra app** (`OpenClipboard` falla) → reintento breve →
  `Empty`/error tolerante; nunca cuelga la UI.
- **Disco lleno / permiso denegado al escribir** → error por canal → status discreto.
- **`cut` del SO** (DropEffect=move): pegar archivos cortados es una movida; tras éxito,
  el SO espera que se vacíe el portapapeles → se vacía.
- **Texto muy grande / imagen grande** → se escribe en worker; UI no bloquea.

---

## 5. Testing

- **`core::clipboard::decide_paste`** (el grueso, puro): Files→Transfer (flag cut),
  Text→CreateText (nombre desde plantilla+fecha+dedup contra `exists`), Image→CreateImage
  (formato según settings; validación de dims absurdas→Nothing), prioridad implícita (el
  contenido ya viene resuelto por platform, pero se testea que cada variante mapea bien),
  Empty→Nothing.
- **`core::clipboard::encode_image`**: rgba8→PNG (round-trip: decodificar el PNG y
  verificar dims + algunos píxeles), →JPG (decodificar y verificar dims; lossy, no comparo
  píxeles exactos), error en dims inconsistentes.
- **`expand_name_template`**: `{fecha}` con `now_secs` inyectado → string esperado
  (determinista); token desconocido se deja literal; sin `{fecha}` queda igual.
- **`platform::clipboard`**: smoke test manual (texto/archivo/imagen reales al portapapeles
  y leerlos; escribir archivos y pegar en Explorer). La lógica pura está en core, testeada.
- **UI**: validación manual (Ctrl+C en Naygo→pegar en Explorer; copiar imagen en
  navegador→Ctrl+V en Naygo; modo directo vs confirmar; toast de copia abre panel).

Meta: build + tests + clippy + fmt verde antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── clipboard/
│   ├── mod.rs       # ClipboardContent, ClipboardImage, ImageFmt, PastePlan, decide_paste
│   ├── encode.rs    # encode_image (PNG/JPG vía `image`) + EncodeError
│   └── naming.rs    # expand_name_template ({fecha}) puro
├── config/mod.rs    # + Settings de paste (serde default)
└── i18n/{es,en}.json

crates/core/Cargo.toml    # + image = { version="0.25", default-features=false, features=["png","jpeg"] }
crates/ui/Cargo.toml      # - image (se mueve a core; ui lo usa vía core si hiciera falta)

crates/platform/src/
├── clipboard.rs     # read() + write_files() Win32 (CF_HDROP, CF_DIB, CF_UNICODETEXT, Preferred DropEffect)
└── lib.rs           # + pub mod clipboard
crates/platform/Cargo.toml # + features windows necesarias (Win32_System_DataExchange, Win32_System_Memory, Win32_UI_Shell, Win32_Graphics_Gdi)

crates/ui/src/
├── app.rs           # Ctrl+C/X→write_files; Ctrl+V→read+decide_paste+ejecutar; quitar InternalClipboard; worker de escritura; toast
├── ops_dialogs.rs   # + PastePreview (mini-diálogo modo B)
└── ...
```

---

## 7. Dependencias

- `image` 0.25 se MUEVE de `ui` a `core`, con features `png` + `jpeg` (se añade `jpeg`).
- `platform`: features adicionales del crate `windows` para portapapeles/GDI.
- Sin chrono (fecha por cálculo propio). Sin otras dependencias nuevas.

---

## Fuera de alcance (recordatorio)

Drag&drop COM/OLE, HTML/RTF enriquecido, formatos de imagen extra, audio. Nunca:
reproducción de media, edición de archivos.
