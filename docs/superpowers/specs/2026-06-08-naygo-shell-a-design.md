# Naygo — Fase shell-A: abrir con app default + espacio de discos (diseño)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-08. Estado: aprobado, listo para escribir plan de implementación.
> Producto: **Naygo** (explorador de archivos estilo Commander, Rust + egui).

---

## 1. Contexto y alcance

Cuarta fase del sprint de funcionalidad, sobre ops-A/ops-B/paste-inteligente (mergeadas).
La integración con el Shell de Windows se **sub-fasea** por peso técnico:

- **shell-A (ESTA fase):** abrir archivos con su app por defecto, "Abrir con…", y espacio
  libre de discos + acceso rápido a unidades. Bajo riesgo, alto valor.
- **shell-B (fase futura, su propio brainstorm):** menú contextual NATIVO de Windows vía
  IContextMenu/IShellFolder COM. Es el grueso técnico (HWND, PIDLs, despacho por id,
  popup owner-draw) y se construye SOLO bajo demanda.

Hoy, activar (doble-clic/Enter) una carpeta navega; activar un archivo deja un placeholder
`status.open_pending` — este spec lo reemplaza por `ShellExecute`. Ya existe un menú
contextual PROPIO de Naygo (Copiar/Cortar/Pegar/Renombrar/Eliminar) en el file panel.

### Decisiones tomadas en el brainstorm

1. **Sub-fase shell-A / shell-B.** shell-A entrega abrir + discos; el menú COM nativo es
   shell-B.
2. **Abrir:** doble-clic / Enter → abrir con la app default (`ShellExecuteW "open"`). Una
   entrada **"Abrir con…"** en el menú contextual propio → diálogo nativo
   (`ShellExecuteW "openas"`).
3. **Menú contextual reordenado y PEREZOSO con el shell:** el menú propio muestra primero
   las acciones de Naygo (Abrir, Abrir con…, Copiar, Cortar, Pegar, Renombrar, Eliminar) y
   deja como ÚLTIMA entrada un **placeholder "Más opciones de Windows…"** — que en shell-A
   está deshabilitado (o con tooltip "próximamente") y en shell-B construirá/lanzará el
   menú COM nativo. Clave de rendimiento: NO se enumera el menú del shell (lento: carga
   handlers de todas las apps) salvo que el usuario lo pida explícitamente.
4. **Espacio de discos:** barra de uso bajo cada disco en el árbol, color según uso (azul
   normal / ámbar >75% usado / rojo >90% usado), texto "X libres de Y · N% **usado**".
   Lectura **async** (un disco de red lento no congela el árbol).
5. **Acceso rápido a unidades:** un panel/toolbar de discos clicables (navegar a la raíz),
   actualizado por **re-escaneo periódico** (~2-3 s y/o al refrescar) — un pendrive aparece
   con un pequeño retardo. Sin enganchar el loop de ventana.
6. **Detección en vivo de dispositivos (WM_DEVICECHANGE)** → NO en shell-A; se diseña con
   el **watcher (5ª fase)**, donde encaja lo de "escuchar al SO".
7. **`human_size` se mueve a `core`** (formateo puro reutilizable), con re-export para que
   `ui` lo siga usando.

### Qué entra en shell-A

- `platform::open`: `open_default` + `open_with_dialog` (ShellExecuteW).
- `platform::drive_space`: lectura async de `GetDiskFreeSpaceExW` por unidad.
- `core::disk`: `DiskUsage` puro (used/percent_used/is_high/is_critical).
- `core::format`: `human_size` movido desde `ui`, testeado.
- `ui`: activar archivo → abrir; "Abrir con…" en el menú; menú reordenado + placeholder
  "Más opciones de Windows…"; barra+%+color de espacio en el árbol; panel/toolbar de
  discos; worker de espacio + re-escaneo periódico.
- i18n ES/EN.

### Qué NO entra

- Menú contextual nativo COM (shell-B).
- Detección en vivo de dispositivos (watcher, 5ª fase).
- "Abrir carpeta contenedora en Explorer" (no pedido; fácil de añadir luego).
- Nunca: reproducción de media, edición de archivos.

---

## 2. Arquitectura

### Capa `platform`

**`platform::open` (módulo nuevo):**
- `open_default(path: &Path) -> Result<(), ShellError>` — `ShellExecuteW(HWND(0), "open",
  path_wide, NULL, NULL, SW_SHOWNORMAL)`. Abre con la app asociada. El valor de retorno de
  ShellExecuteW ≤ 32 indica error (mapear SE_ERR_NOASSOC, ERROR_FILE_NOT_FOUND, etc. a
  `ShellError`). No requiere CoInitialize.
- `open_with_dialog(path: &Path) -> Result<(), ShellError>` — `ShellExecuteW(..., "openas",
  ...)` lanza el diálogo "Abrir con…" nativo.
- `ShellError { NotSupported, NoAssociation, Failed(String) }`.
- Patrón `#[cfg(windows)]` real + `#[cfg(not(windows))]` stub `NotSupported` (molde:
  trash/clipboard). Cargo: features `Win32_UI_Shell` (ShellExecuteW ya disponible) +
  `Win32_UI_WindowsAndMessaging` (SW_SHOWNORMAL) si hace falta.

**`platform::drive_space` (módulo nuevo):**
- `read_space(root: &Path) -> Option<(u64, u64)>` — `GetDiskFreeSpaceExW(root)` → `(total,
  free)` en bytes. `None` si falla (disco caído, óptico vacío). Síncrono pero se llama
  DESDE un worker (puede tardar en red). Cargo: `Win32_Storage_FileSystem` (ya está).
- NOTA: NO se mete el espacio en `DriveInfo` (que debe seguir devolviéndose rápido por
  `drives()`); el espacio es un dato aparte, leído async y asociado por root en la UI.

**`platform::drives` (existe, SIN cambios):** sigue devolviendo `Vec<DriveInfo> { path,
label, kind }` rápido. El re-escaneo de la UI lo vuelve a llamar periódicamente.

### Capa `core` (pura, testeable)

**`core::disk` (módulo nuevo):**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskUsage {
    pub total: u64,
    pub free: u64,
}
impl DiskUsage {
    pub fn used(self) -> u64 { self.total.saturating_sub(self.free) }
    /// Porcentaje USADO 0..=100 (satura; 0 si total==0).
    pub fn percent_used(self) -> u8 { /* (used*100/total) clamp 0..=100 */ }
    pub fn is_high(self) -> bool { self.percent_used() > 75 }
    pub fn is_critical(self) -> bool { self.percent_used() > 90 }
}
```

**`core::format` (módulo nuevo):** `pub fn human_size(bytes: u64) -> String` MOVIDO desde
`ui::panes::file_panel`. Misma lógica (KB/MB/GB/TB, redondeo). `core::lib` lo re-exporta o
los callers de ui pasan a `naygo_core::format::human_size`. Los 3 call sites de ui
(`app.rs`, `ops_panel.rs`, `panes/file_panel.rs`) se actualizan al nuevo path.

### Capa `ui`

- **Abrir:** `activate_focused` (app.rs:1443) — para archivos, en vez del placeholder
  `status.open_pending`, llama `platform::open::open_default(&entry.path)`; éxito →
  status "Abriendo {name}"; error → status traducible. ShellExecute retorna al lanzar (no
  espera la app), así que va en el hilo de UI sin bloquear de forma apreciable.
- **"Abrir con…":** entrada nueva en el menú contextual del file panel (file_panel.rs:287)
  → acción diferida (patrón `ops_actions`/`Action`) → `open_with_dialog`.
- **Menú contextual reordenado:** Abrir / Abrir con… / (sep) / Copiar / Cortar / Pegar /
  (sep) / Renombrar / Eliminar / (sep) / **"Más opciones de Windows…"** (placeholder
  deshabilitado en shell-A; tooltip "próximamente"). Se añaden `Action::Open` y
  `Action::OpenWith`.
- **Espacio en el árbol:** las raíces (discos) muestran, cuando el dato llegó, una barra de
  uso + "X libres de Y · N% usado", color por umbral. El estado vive en un mapa
  `root → DiskUsage` en `NaygoApp`, rellenado por el worker.
- **Panel/toolbar de discos:** acceso rápido — un strip de unidades clicables (navegar a la
  raíz en el panel activo). Ubicación: una fila en el toolbar (toolbar.rs) o un panelcito;
  se decide en el plan según encaje visual (mínimo: botones por unidad con su letra/ícono).
- **Worker de espacio + re-escaneo:** al arrancar y cada ~2-3 s (constante conservadora),
  la UI lanza un worker que: `drives()` → por cada root `read_space` → emite `(root,
  Option<DiskUsage>)` por canal. La UI drena por frame (patrón `pump_*`) y actualiza el
  mapa + la lista de discos (detecta pendrives nuevos/quitados). No se solapan escaneos
  (flag "en curso"). Bajo consumo: el intervalo es amplio y el escaneo es barato salvo I/O
  de red, que va en el worker.

### Lo que NO cambia

El motor de ops, el clipboard, el listing, el árbol (solo se le añade el render de
espacio), el menú propio (se reordena y se le añaden 2 entradas + 1 placeholder).

---

## 3. Flujo de datos

**Abrir:** doble-clic/Enter → `activate_focused` → carpeta: navega (como hoy); archivo:
`open_default` → status. "Abrir con…": menú → `Action::OpenWith` → `open_with_dialog`.

**Espacio (async):** arranque + cada ~2-3 s → worker: `drives()` → `read_space(root)` por
unidad → canal `(root, Option<DiskUsage>)` → UI drena por frame → mapa `root→DiskUsage` +
lista de discos actualizada → árbol/panel pintan barra+%+color y los discos clicables.

## 4. Manejo de errores / casos límite

- **Archivo sin asociación** → `open_default` falla (SE_ERR_NOASSOC) → status "No hay app
  asociada a {name}". No crashea.
- **Ruta desaparecida al abrir** → ShellExecute falla → status de error.
- **Disco de red caído / óptico vacío** → `read_space` → `None` → el disco se muestra sin
  barra; el árbol no se congela (lectura en worker).
- **Pendrive quitado entre escaneo y clic** → navegar falla → lo maneja el flujo de listing
  hostil existente.
- **Re-escaneo solapado** → flag "escaneo en curso"; no se lanza otro hasta terminar.
- **ShellExecute lento** (raro) → retorna al lanzar; si alguna vez bloqueara, se podría
  mover a worker — por ahora directo (open es inmediato).

## 5. Testing

- **`core::disk::DiskUsage`** (puro): `used` (free>total → satura a 0), `percent_used`
  (0/50/75/90/100, total=0→0), `is_high`/`is_critical` en umbrales (75/90 exactos y
  alrededor).
- **`core::format::human_size`** (movido): 0 B, bytes, KB/MB/GB/TB, bordes (1023, 1024),
  redondeo. (Reusar/expandir los tests que existan.)
- **`platform::open`**: smoke manual (abrir .txt → app default; "Abrir con…" → diálogo).
- **`platform::drive_space::read_space`**: smoke manual (C: da total/free plausibles;
  la conversión a `DiskUsage` es pura y testeada).
- **UI:** validación manual (barra+color+% usado, panel de discos clicable, re-escaneo
  detecta un pendrive con retardo, "Abrir con…").

Meta: build + tests + clippy + fmt verde antes de cada commit.

---

## 6. Estructura de archivos (incremental)

```
crates/core/src/
├── disk.rs          # NUEVO: DiskUsage (used/percent_used/is_high/is_critical) + tests
├── format.rs        # NUEVO: human_size (movido de ui) + tests
└── lib.rs           # + pub mod disk; pub mod format;

crates/platform/src/
├── open.rs          # NUEVO: open_default / open_with_dialog (ShellExecuteW) + stub
├── drive_space.rs   # NUEVO: read_space (GetDiskFreeSpaceExW) + stub
├── lib.rs           # + pub mod open; pub mod drive_space;
└── Cargo.toml       # + features windows si faltan (UI_WindowsAndMessaging)

crates/ui/src/
├── app.rs           # activate_focused→open_default; worker de espacio + re-escaneo + mapa; Action::Open/OpenWith
├── panes/file_panel.rs  # menú reordenado + Abrir/Abrir con…/placeholder; quitar human_size (usar core)
├── ops_panel.rs     # human_size → naygo_core::format
├── tree (árbol)     # render de barra+%+color de espacio por disco
├── toolbar.rs / panel de discos  # strip de unidades clicables
└── (i18n se edita en core)

crates/core/src/i18n/{es,en}.json  # + claves de abrir/abrir-con/espacio/más-opciones
```

---

## 7. Dependencias

Ninguna nueva de terceros. Crate `windows` 0.62 (features ya presentes + quizá
`Win32_UI_WindowsAndMessaging` para SW_SHOWNORMAL). Sin chrono.

---

## Fuera de alcance (recordatorio)

Menú contextual COM nativo (shell-B), detección en vivo de dispositivos (watcher), abrir
carpeta en Explorer. Nunca: reproducción de media, edición de archivos.
