# Expulsar USB con paneles abiertos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Al expulsar un USB, avisar si hay paneles con carpetas abiertas en ese disco, soltar esos paneles (cerrar sus watchers, dejarlos en el aviso in-place "elegir carpeta") antes de expulsar para evitar el "en uso", y avisar claro si la expulsión falla igual.

**Architecture:** Lógica pura testeable en `workspace_ctrl.rs` (`path_is_on_drive` + `panes_on_drive`). El aviso in-place "elegir carpeta" sale GRATIS del mecanismo existente (`pane_dir_missing` ya detecta `read_dir` fallido tras expulsar). El trabajo real: detectar paneles en el disco, cerrar sus watchers antes de `eject_drive`, y enriquecer el modal de confirmación. Un refinamiento opcional diferencia el texto del aviso ("disco expulsado" vs "carpeta no encontrada").

**Tech Stack:** Rust, Slint, i18n JSON. Plataforma: `naygo_platform::eject`.

---

## Contexto y convenciones (leer antes de empezar)

- Rama: `feat/iconos-personalizables`.
- ⚠️ **REGLA GIT (un subagente corrompió el árbol antes):** SOLO `git add <rutas explícitas>` + `git commit`. PROHIBIDO `git reset/restore/checkout/stash/clean`, `git commit -a`/`-am`, `git add -A`/`add .`. Hay 2 archivos ajenos modificados (`CLAUDE.md`, `crates/core/src/favorites.rs`) que NO se tocan ni se stagean. Si el árbol parece mal, PARAR y reportar.
- Header en archivos: `// Naygo — <desc>.` / `// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.` / `// SPDX-License-Identifier: MIT`.
- Comentarios/commits español NEUTRAL, NUNCA voseo.
- Hay test de parity i18n `es_en_tienen_las_mismas_claves`: toda clave va en es.json Y en.json.
- Build limpio + clippy + tests antes de commit. Tests: `cargo test -p naygo-ui-slint` (workspace_ctrl vive en ui-slint). Build: `cargo build -p naygo-ui-slint --bins`.

## Hechos verificados del código

- `EjectOutcome { Ok, InUse, Failed(String) }` (workspace_ctrl.rs:42). `eject_drive(root: PathBuf) -> EjectOutcome` (workspace_ctrl.rs:2676).
- Iteración de paneles Files: `self.ws.panes().iter().filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))` da `(PaneId, PathBuf)` (patrón en workspace_ctrl.rs:698).
- `PaneId` es un newtype con `.0: u64`.
- `pane_dir_missing(id) -> bool` (workspace_ctrl.rs:825) chequea `read_dir(dir).is_err()` → tras expulsar, el aviso in-place aparece SOLO.
- El watcher: `self.watchers.unwatch(pane: u64)` (watch.rs:62), `watched_panes() -> Vec<u64>`.
- El modal de eject: `on_eject_drive` (main.rs:3937) guarda `pending_eject: Rc<RefCell<Option<String>>>` y arma `MessageVm { kind:1, ... }`. La confirmación (kind 1) está en `on_message_confirm` (main.rs ~3966+), toma `pending_eject` y llama `eject_drive`.
- `PaneVm` tiene `missing: bool`, `missing-path: string`, `missing-has-ancestor: bool` (types.slint:528-532).

## Mapa de archivos

**Modificar:**
- `crates/ui-slint/src/workspace_ctrl.rs` — `path_is_on_drive` (helper puro) + `panes_on_drive` + `release_pane_watcher`.
- `crates/ui-slint/src/main.rs` — enriquecer `on_eject_drive` (lista de paneles en el body) + en la confirmación soltar watchers antes de `eject_drive` + aviso de fallo.
- `crates/core/src/i18n/es.json`, `en.json` — claves del aviso enriquecido + fallo.
- `crates/ui-slint/src/i18n_keys.rs` — cablear las claves nuevas usadas en markup (si aplica).

---

## Fase 1 — Detección de paneles en el disco (lógica pura)

### Task 1: `path_is_on_drive` (helper puro)

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (función libre + tests)

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `crates/ui-slint/src/workspace_ctrl.rs`:

```rust
#[test]
fn path_is_on_drive_casos() {
    use std::path::Path;
    let on = |p: &str, d: &str| path_is_on_drive(Path::new(p), Path::new(d));
    // dentro del disco
    assert!(on(r"E:\foto", r"E:\"));
    assert!(on(r"E:\a\b\c", r"E:\"));
    // la raíz misma
    assert!(on(r"E:\", r"E:\"));
    // case-insensitive (Windows)
    assert!(on(r"e:\x", r"E:\"));
    assert!(on(r"E:\x", r"e:\"));
    // otro disco
    assert!(!on(r"C:\", r"E:\"));
    assert!(!on(r"C:\foto", r"E:\"));
    // prefijo falso (no por substring)
    assert!(!on(r"EE:\x", r"E:\"));
    // UNC / share de red: no está en un USB local
    assert!(!on(r"\\srv\share\x", r"E:\"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-ui-slint path_is_on_drive_casos`
Expected: FAIL — `path_is_on_drive` no existe.

- [ ] **Step 3: Write minimal implementation**

Agregar a `crates/ui-slint/src/workspace_ctrl.rs` (función libre, no método):

```rust
/// ¿La ruta `path` está dentro del disco cuya raíz es `drive_root`? Compara por la LETRA
/// de unidad (primer componente con `:`), case-insensitive (Windows). Una ruta UNC
/// (`\\srv\share`) o sin letra de unidad nunca está "en" un disco con letra. Pura y testeable.
fn path_is_on_drive(path: &std::path::Path, drive_root: &std::path::Path) -> bool {
    fn drive_letter(p: &std::path::Path) -> Option<char> {
        // El primer componente de una ruta absoluta de Windows con letra es "E:" o "E:\".
        let s = p.to_string_lossy();
        let bytes = s.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            Some((bytes[0] as char).to_ascii_uppercase())
        } else {
            None
        }
    }
    match (drive_letter(path), drive_letter(drive_root)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-ui-slint path_is_on_drive_casos`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat(ui): path_is_on_drive — ¿una ruta está en un disco? (case-insensitive, por letra)"
```

---

### Task 2: `panes_on_drive` (paneles afectados)

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (método + test)

- [ ] **Step 1: Write the failing test**

Agregar a los tests de `workspace_ctrl.rs`. Usa el patrón de construcción de `WorkspaceCtrl` que ya usan los tests existentes del archivo (busca `fn ctrl()` o cómo arman un controller de prueba con paneles; reusa ese helper). El test arma 2 paneles en discos distintos:

```rust
#[test]
fn panes_on_drive_filtra_por_disco() {
    // Reusar el helper de construcción de WorkspaceCtrl de los tests del archivo.
    // Navegar un panel a E:\ y otro a C:\ (o las rutas que el helper permita).
    // Aquí en pseudo-código del patrón; adaptar al helper real:
    let mut c = ctrl_con_dos_paneles(); // helper existente o equivalente
    // panel A -> E:\algo ; panel B -> C:\algo  (usar set/navigate del controller)
    // ...
    let afectados = c.panes_on_drive(std::path::Path::new(r"E:\"));
    // solo el panel en E: aparece
    assert_eq!(afectados.len(), 1);
    assert!(afectados[0].1.to_string_lossy().to_uppercase().starts_with("E:"));
}
```
> NOTA: si montar 2 paneles en discos concretos es difícil en el harness de test (no hay E:\ real), reduce el test a verificar `panes_on_drive` sobre el disco del único panel existente (que devuelve ese panel) y sobre un disco inexistente (que devuelve vacío). Lo esencial es ejercitar el filtrado. Si ni eso es viable con el harness, marca el test como cubriendo solo el caso vacío y apóyate en que `path_is_on_drive` ya está testeado a fondo (Task 1). Usa tu criterio según el helper de test disponible; NO inventes APIs del controller que no existen.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-ui-slint panes_on_drive`
Expected: FAIL — `panes_on_drive` no existe.

- [ ] **Step 3: Write minimal implementation**

Agregar como método de `WorkspaceCtrl` en `workspace_ctrl.rs`:

```rust
/// Los paneles Files cuya carpeta actual está en el disco `drive_root` (el que se va a
/// expulsar). Devuelve `(PaneId, carpeta_actual)`. Pura sobre el estado del workspace.
pub fn panes_on_drive(&self, drive_root: &std::path::Path) -> Vec<(PaneId, std::path::PathBuf)> {
    self.ws
        .panes()
        .iter()
        .filter_map(|p| p.files.as_ref().map(|f| (p.id, f.current_dir.clone())))
        .filter(|(_, dir)| path_is_on_drive(dir, drive_root))
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-ui-slint panes_on_drive`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat(ui): panes_on_drive — paneles con carpeta abierta en el disco a expulsar"
```

---

### Task 3: `release_pane_watcher` (soltar el handle antes de expulsar)

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (método + test)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn release_pane_watcher_quita_el_watcher() {
    // Reusar el helper de construcción. Tras navegar un panel, su watcher está activo
    // (aparece en watched_panes). release_pane_watcher(id) debe quitarlo.
    let mut c = ctrl_con_un_panel(); // helper existente
    let id = c.active_pane_id();      // o como se obtenga el id del panel activo en los tests
    // asegurar que el watcher está (puede requerir un sync/relaunch que los tests ya hacen)
    c.release_pane_watcher(id);
    assert!(!c.watchers.watched_panes().contains(&id.0));
}
```
> Adapta `ctrl_con_un_panel`, `active_pane_id` y cómo se asegura que el watcher esté activo al patrón REAL de los tests del archivo (léelos primero). Si el watcher no se arranca en el harness de test (sin loop async), simplifica: el test verifica que `release_pane_watcher` sobre un id sin watcher es no-op (no panic) y que tras `watch` manual + release el panel sale de `watched_panes`. Usa tu criterio; NO inventes APIs.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p naygo-ui-slint release_pane_watcher`
Expected: FAIL — `release_pane_watcher` no existe.

- [ ] **Step 3: Write minimal implementation**

```rust
/// Suelta el handle que la app mantiene sobre la carpeta del panel `id`: cierra su watcher.
/// Se llama justo ANTES de expulsar el disco, para que el "en uso" no lo cause la propia app.
/// El aviso in-place "elegir carpeta" aparece solo tras expulsar (pane_dir_missing detecta el
/// read_dir fallido). No-op si el panel no tiene watcher.
pub fn release_pane_watcher(&mut self, id: PaneId) {
    self.watchers.unwatch(id.0);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p naygo-ui-slint release_pane_watcher`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat(ui): release_pane_watcher — cerrar el watcher de un panel antes de expulsar"
```

---

## Fase 2 — i18n del aviso enriquecido + fallo

### Task 4: Claves i18n

**Files:**
- Modify: `crates/core/src/i18n/es.json`, `crates/core/src/i18n/en.json`
- Modify: `crates/ui-slint/src/i18n_keys.rs` (cablear las usadas en markup vía Tr)

- [ ] **Step 1: Agregar claves a AMBOS JSON**

es.json (junto a las demás `drive.*`):
```json
"drive.eject_with_panes": "El disco {drive} tiene {n} panel(es) con carpetas abiertas. Al expulsar, esos paneles te pedirán elegir otra carpeta.",
"drive.eject_anyway": "Expulsar de todos modos",
"drive.eject_in_use_external": "No se pudo expulsar: el disco sigue en uso por otro programa. Intenta de nuevo o usa «Quitar hardware con seguridad» de Windows.",
```
en.json:
```json
"drive.eject_with_panes": "Drive {drive} has {n} panel(s) with open folders. After ejecting, those panels will ask you to choose another folder.",
"drive.eject_anyway": "Eject anyway",
"drive.eject_in_use_external": "Could not eject: the drive is still in use by another program. Try again or use Windows' \"Safely remove hardware\".",
```

- [ ] **Step 2: Validar JSON + parity**

Run: `/c/Python313/python -c "import json; json.load(open('crates/core/src/i18n/es.json',encoding='utf-8')); json.load(open('crates/core/src/i18n/en.json',encoding='utf-8')); print('ok')"`
Run: `cargo test -p naygo-core i18n`
Expected: JSON válido + parity verde.

- [ ] **Step 3: Cablear en Tr (i18n_keys.rs / i18n.slint) si se usan como `Tr.*` en markup**

Estas claves se usan desde Rust (componiendo el `body`/`MessageVm` y el toast) vía
`c.t("drive.eject_with_panes")` (el accesor que ya usan otros mensajes en main.rs), NO
necesariamente como `Tr.*` en .slint. Verifica cómo se obtienen hoy los textos del modal de
eject (main.rs usa `tr.get_drive_eject_confirm()` — un campo Tr). Para mantener consistencia:
si el modal usa campos Tr, agrega los Tr fields correspondientes en `ui/i18n.slint`
(`in property <string> drive-eject-anyway;` etc.) + setters en `i18n_keys.rs`
(`tr.set_drive_eject_anyway(c.t("drive.eject_anyway").into());`). Si en cambio el código
compone con `c.t(...)` directo (como otros toasts), no hace falta Tr. Lee main.rs y elige el
patrón que ya use el modal de eject. Documenta cuál usaste.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/i18n/es.json crates/core/src/i18n/en.json crates/ui-slint/src/i18n_keys.rs crates/ui-slint/ui/i18n.slint
git commit -m "feat(i18n): claves del aviso de expulsar con paneles abiertos + fallo (ES/EN)"
```
(Incluye solo los archivos que realmente tocaste; si no agregaste Tr fields, omite i18n.slint/i18n_keys.rs del add.)

---

## Fase 3 — Cableado: modal enriquecido + soltar + expulsar

### Task 5: Enriquecer el modal de confirmación con la lista de paneles

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (`on_eject_drive` + estado pending)

- [ ] **Step 1: Implementar (sin test unitario; verificación en vivo)**

READ `on_eject_drive` (main.rs ~3937) y el estado `pending_eject` (~3931). El cambio:
1. Junto a `pending_eject: Rc<RefCell<Option<String>>>`, agregar un estado para los paneles
   afectados: `pending_eject_panes: Rc<RefCell<Vec<u64>>>` (los ids de panel a soltar).
   Declararlo donde se declara `pending_eject` y clonarlo en los closures que lo necesiten
   (on_eject_drive y on_message_confirm), igual que se hace con `pending_eject`.
2. En `on_eject_drive`, antes de armar el `MessageVm`:
```rust
let afectados = {
    let c = ctrl.borrow();
    c.panes_on_drive(std::path::Path::new(path.as_str()))
};
*pending_eject_panes.borrow_mut() = afectados.iter().map(|(id, _)| id.0).collect();
let tr = ui.global::<Tr>();
let (body, confirm_label) = if afectados.is_empty() {
    (tr.get_drive_eject_confirm().replace("{drive}", path.as_str()),
     tr.get_drive_eject())
} else {
    let cuerpo = c_t(&ui, "drive.eject_with_panes")   // ver nota
        .replace("{drive}", path.as_str())
        .replace("{n}", &afectados.len().to_string());
    (cuerpo, c_t(&ui, "drive.eject_anyway"))
};
```
> `c_t(&ui, key)` = cómo se obtiene un string traducido desde Rust en main.rs. Si el modal usa
> campos Tr (`tr.get_*`), agrega los getters correspondientes (de Task 4) y úsalos en vez de
> `c_t`. Usa el MISMO patrón que el resto del modal de eject. Adapta los nombres reales.
3. Setear el `MessageVm` con `body` y `confirm_label` calculados (el resto igual que hoy:
   kind 1, level 1 warning, title `drive_eject_confirm_title`, cancel `dlg_cancel`).

- [ ] **Step 2: Compilar**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila.

- [ ] **Step 3: Commit**

```bash
git add crates/ui-slint/src/main.rs
git commit -m "feat(ui): el modal de expulsar avisa cuántos paneles tienen el disco abierto"
```

---

### Task 6: Soltar los paneles + expulsar + resultado

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (la rama kind==1 de `on_message_confirm`)

- [ ] **Step 1: Implementar (sin test unitario; verificación en vivo)**

READ la rama de confirmación de eject en `on_message_confirm` (main.rs, donde hoy hace
`if let Some(path) = pending_eject.borrow_mut().take() { ... eject_drive ... match outcome }`).
Modificarla:
1. ANTES de `eject_drive`, soltar los watchers de los paneles afectados:
```rust
let panes = std::mem::take(&mut *pending_eject_panes.borrow_mut());
{
    let mut c = ctrl.borrow_mut();
    for pid in &panes {
        c.release_pane_watcher(naygo_core_pane_id(*pid)); // construir PaneId desde u64
    }
}
```
> Construir `PaneId` desde `u64`: ver cómo se hace en el archivo (probablemente `PaneId(pid)`
> o un constructor). Usa el patrón real. Si `release_pane_watcher` toma `PaneId`, envuelve el
> u64; si conviene, dale una sobrecarga por u64. Mantén el borrow_mut acotado (suéltalo antes de
> los refresh, como en otros handlers — patrón borrow-then-drop).
2. Llamar `eject_drive(PathBuf::from(path))` como hoy.
3. En el `match outcome`:
   - `Ok` → toast/aviso de éxito como hoy (`drive_eject_ok`). Tras expulsar, los paneles
     afectados mostrarán el aviso in-place "elegir carpeta" automáticamente (pane_dir_missing
     detecta el read_dir fallido) — NO hay que forzar nada. Llamar al `sync`/refresh del layout
     que ya se use para que el PaneVm se reconstruya (mirror de cómo otros handlers refrescan
     tras cambiar estado de paneles) + `refresh_drives()`.
   - `InUse` / `Failed(_)` → mostrar el aviso `drive.eject_in_use_external` (en vez del genérico
     `drive_eject_in_use`), vía `MessageVm` kind 2 (aviso 1 botón) o toast, según el patrón del
     archivo. Los paneles ya quedaron con el watcher cerrado; está bien (el usuario reintenta).
     `refresh_drives()`.

- [ ] **Step 2: Compilar**

Run: `cargo build -p naygo-ui-slint --bins`
Expected: compila.

- [ ] **Step 3: Verificación en vivo (Nicolás, requiere USB real)**

Con un USB conectado y un panel abierto en él: pulsar expulsar → el modal avisa "1 panel
tiene carpetas abiertas" → confirmar → el disco se expulsa (o avisa el fallo claro) y el panel
muestra el aviso "elegir otra carpeta".

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/src/main.rs
git commit -m "feat(ui): soltar watchers de los paneles antes de expulsar + aviso de fallo claro"
```

---

## Fase 4 — Refinamiento opcional (texto del aviso diferenciado)

### Task 7: (Opcional) Texto "disco expulsado" en el aviso in-place

> Este task es un REFINAMIENTO. El comportamiento esencial ya funciona tras Task 6 (el aviso
> "carpeta no encontrada" aparece y deja elegir carpeta). Este task SOLO cambia el texto a
> "El disco fue expulsado. Elige otra carpeta." para los paneles soltados por expulsión.
> Si añade complejidad desproporcionada, SE PUEDE OMITIR (reportarlo). Evaluar antes de hacerlo.

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs` (bandera `HashSet<u64>` de paneles soltados por eject)
- Modify: `crates/ui-slint/ui/types.slint` (campo `missing-ejected: bool` en PaneVm) + el builder en main.rs
- Modify: i18n (clave `pane.ejected_choose`)

- [ ] **Step 1: Decidir si vale la pena**

Lee cómo el builder del PaneVm setea `missing` / `missing-path`. Si agregar un flag `ejected`
que cambie solo el texto es ~15 líneas limpias, hazlo (Steps 2-5). Si requiere tocar muchos
sitios o enturbia el builder, OMITE este task y repórtalo como DONE_WITH_CONCERNS explicando
que el aviso genérico "carpeta no encontrada" cumple la función.

- [ ] **Step 2-5 (si se hace):** agregar `ejected: HashSet<u64>` al controller, marcar los ids
  en la confirmación de eject (Task 6), exponer `pane_ejected(id) -> bool`, agregar
  `missing-ejected: bool` al PaneVm, que el builder use el texto `pane.ejected_choose` cuando
  esté activo, limpiar el flag cuando el panel navega a una carpeta válida. Clave i18n
  `pane.ejected_choose` en ambos JSON (parity). Commit:
  `git add <archivos>; git commit -m "feat(ui): texto 'disco expulsado' en el aviso del panel soltado"`.

---

## Fase 5 — Cierre

### Task 8: Suite + clippy + verificación

- [ ] **Step 1:** `cargo test -p naygo-ui-slint` y `cargo test -p naygo-core` → verdes.
- [ ] **Step 2:** `cargo clippy -p naygo-ui-slint --bins` y `cargo clippy -p naygo-core` → sin warnings.
- [ ] **Step 3:** `cargo build -p naygo-ui-slint --bins` → compila.
- [ ] **Step 4:** Verificación en VM por Nicolás (USB real): modal avisa, soltar funciona, fallo claro.

---

## Resumen de fases
1. **Detección** pura (`path_is_on_drive`, `panes_on_drive`, `release_pane_watcher`) — TDD.
2. **i18n** del aviso enriquecido + fallo.
3. **Cableado**: modal enriquecido + soltar watchers + expulsar + resultado.
4. **Refinamiento opcional**: texto "disco expulsado" diferenciado.
5. **Cierre**: suite + clippy + VM.
