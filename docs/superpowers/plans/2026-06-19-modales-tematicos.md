# Modales temáticos — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reemplazar los diálogos nativos `rfd::MessageDialog` con ventana viva (confirmar expulsar USB, error de import/export) por un modal Slint que usa el tema de Naygo, y aclarar el texto del diálogo de panic.

**Architecture:** Componente Slint `MessageModal` (hermano de `OpDialogs`, mismo look: velo + tarjeta centrada + `DlgBtn` + Esc), gobernado por un `MessageVm` (`kind` 1=confirmación / 2=aviso). Cableado por una propiedad `message` + callbacks `message-confirm`/`message-cancel` en `AppWindow`. En `main.rs`, la expulsión deja de ser inline-bloqueante: el path pendiente se guarda en un `Rc<RefCell<Option<String>>>` y la expulsión real corre en `message-confirm`. El panic sigue en `rfd` pero sin el payload técnico crudo (va solo al log).

**Tech Stack:** Rust workspace (`naygo-ui-slint`), Slint 1.16 (render por software), `rfd` (solo para panic + selectores de archivo). Build con `CARGO_BUILD_JOBS=2`.

> **Nota de contexto verificada:** la clave i18n `slint.drive.eject` ("Expulsar"/"Eject") YA existe en `es.json:495` / `en.json:495`, y `slint.dialog.accept`/`slint.dialog.cancel` también. NO hay que crear claves en los JSON. Solo falta exponer `drive-eject` en `i18n.slint` y setearla en `i18n_keys.rs` (las de diálogo ya están expuestas).

---

### Task 1: `MessageVm` struct + componente `MessageModal`

**Files:**
- Modify: `crates/ui-slint/ui/types.slint` (agregar el struct cerca de `OpDialogVm`, ~línea 139)
- Create: `crates/ui-slint/ui/message-modal.slint`

- [ ] **Step 1: Agregar el struct `MessageVm` a `types.slint`**

Justo después del cierre de `export struct OpDialogVm { ... }` (línea 139), agregar:

```slint
// Estado del modal de mensajes simple (avisos/confirmaciones temáticos, reemplazo de
// los diálogos nativos rfd). `kind`: 0=oculto 1=confirmación(2 botones) 2=aviso(1 botón).
// `level`: 0=info 1=warning 2=error (tiñe la franja del título).
export struct MessageVm {
    kind: int,
    level: int,
    title: string,
    body: string,
    confirm-label: string,
    cancel-label: string,
    danger: bool,
}
```

- [ ] **Step 2: Crear `message-modal.slint`**

```slint
// Naygo — modal de mensaje temático (aviso/confirmación). Reemplaza los diálogos
// nativos rfd con la ventana viva. Mismo look que op-dialogs: velo + tarjeta centrada.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
import { MessageVm } from "types.slint";
import { Tr } from "i18n.slint";
import { Theme } from "theme.slint";

// Botón de diálogo reutilizable (copia del de op-dialogs para no acoplar los archivos).
component DlgBtn inherits Rectangle {
    in property <string> label;
    in property <bool> primary: false;
    in property <bool> danger: false;
    callback clicked();
    width: txt.preferred-width + 24px;
    height: 28px;
    border-radius: 4px;
    background: touch.has-hover
        ? (root.danger ? Theme.error : Theme.selection-bg)
        : (root.danger ? Theme.error : (root.primary ? Theme.selection-bg : Theme.row-bg));
    txt := Text {
        text: root.label;
        color: white;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
    touch := TouchArea {
        clicked => { root.clicked(); }
    }
}

export component MessageModal inherits Rectangle {
    in property <MessageVm> vm;
    callback message-confirm();
    callback message-cancel();

    visible: root.vm.kind > 0;
    background: #000000aa;

    // Velo: clic fuera de la tarjeta = cancelar (kind 1) / cerrar (kind 2).
    veil := TouchArea {
        clicked => {
            if (root.vm.kind == 1) { root.message-cancel(); }
            if (root.vm.kind == 2) { root.message-confirm(); }
        }
    }

    Rectangle {
        x: (parent.width - self.width) / 2;
        y: (parent.height - self.height) / 2;
        width: 380px;
        height: card.preferred-height + 32px;
        background: Theme.row-bg;
        border-color: Theme.selection-bg;
        border-width: 1px;
        border-radius: 8px;
        // Captura los clics dentro de la tarjeta (no propaga al velo).
        TouchArea {}
        card := VerticalLayout {
            padding: 16px;
            spacing: 14px;

            // Título con franja de color según level.
            HorizontalLayout {
                spacing: 10px;
                Rectangle {
                    width: 3px;
                    border-radius: 1.5px;
                    background: root.vm.level == 0 ? Theme.accent : Theme.error;
                }
                Text {
                    text: root.vm.title;
                    color: white;
                    font-size: 15px;
                    font-weight: 600;
                    vertical-alignment: center;
                }
            }

            Text {
                text: root.vm.body;
                color: Theme.text;
                wrap: word-wrap;
            }

            HorizontalLayout {
                alignment: end;
                spacing: 8px;
                if root.vm.kind == 1: DlgBtn {
                    label: root.vm.cancel-label;
                    clicked => { root.message-cancel(); }
                }
                DlgBtn {
                    label: root.vm.confirm-label;
                    primary: root.vm.kind == 1;
                    danger: root.vm.danger;
                    clicked => { root.message-confirm(); }
                }
            }
        }
    }

    // Esc cierra/cancela.
    forward-focus: msg-focus;
    msg-focus := FocusScope {
        key-pressed(event) => {
            if (event.text == Key.Escape) {
                if (root.vm.kind == 1) { root.message-cancel(); }
                if (root.vm.kind == 2) { root.message-confirm(); }
            }
            accept
        }
    }
}
```

- [ ] **Step 3: Verificar que compila el `.slint` (build de la UI)**

Run (PowerShell):
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-Object -Last 20
```
Expected: compila sin error. (Aún no se usa `MessageModal`; el compilador de Slint valida la sintaxis del archivo al estar en el directorio `ui/`, pero para forzar su parseo se importa en la Task 2. Si esta build pasa sin tocar el archivo, es porque Slint solo compila lo importado — está bien, la Task 2 lo importa y revalida.)

- [ ] **Step 4: Commit**

```
git add crates/ui-slint/ui/types.slint crates/ui-slint/ui/message-modal.slint
git commit -m "feat(ui): MessageVm + componente MessageModal tematico"
```

---

### Task 2: Cablear `MessageModal` en `app-window.slint`

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint` (import línea 4 y 13; propiedad ~540; instancia ~1188)

- [ ] **Step 1: Importar `MessageVm` y `MessageModal`**

En la línea 4, agregar `MessageVm` a la lista de imports de `types.slint`:
```slint
import { RowData, PaneVm, SplitVm, PickVm, TabVm, OpDialogVm, OpRowVm, ResumeRowVm, ContextMenuVm, ColumnMenuVm, NavRow, LayoutRow, MessageVm } from "types.slint";
```

Después de la línea 13 (`import { OpDialogs } from "op-dialogs.slint";`), agregar:
```slint
import { MessageModal } from "message-modal.slint";
```

- [ ] **Step 2: Declarar propiedad + callbacks**

Justo después de `in property <OpDialogVm> op-dialog;` (línea ~540), agregar:
```slint
    in property <MessageVm> message;
    callback message-confirm();
    callback message-cancel();
```

- [ ] **Step 3: Instanciar el modal (encima de `OpDialogs`)**

Localizar la instancia `OpDialogs { ... }` (~línea 1188). Inmediatamente DESPUÉS de su bloque de cierre `}`, agregar:
```slint
    MessageModal {
        vm: root.message;
        message-confirm() => { root.message-confirm(); }
        message-cancel() => { root.message-cancel(); }
    }
```

- [ ] **Step 4: Verificar build**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-Object -Last 20
```
Expected: compila. Ahora `message-modal.slint` está importado y el compilador de Slint lo valida. Si hay error de sintaxis en el `.slint`, aparece aquí.

- [ ] **Step 5: Commit**

```
git add crates/ui-slint/ui/app-window.slint
git commit -m "feat(ui): cablear MessageModal en AppWindow (message + callbacks)"
```

---

### Task 3: i18n — exponer `drive-eject` en Slint

**Files:**
- Modify: `crates/ui-slint/ui/i18n.slint` (junto a `drive-eject-confirm`, ~línea 26)
- Modify: `crates/ui-slint/src/i18n_keys.rs` (junto a `set_drive_eject_confirm`, ~línea 42)

> Las claves JSON ya existen (`slint.drive.eject`); solo falta el puente Slint↔Rust.

- [ ] **Step 1: Agregar la propiedad en `i18n.slint`**

Después de la línea 26 (`in property <string> drive-eject-confirm: "...";`), agregar:
```slint
    in property <string> drive-eject: "Expulsar";
```

- [ ] **Step 2: Setear la clave en `i18n_keys.rs`**

Después de la línea 42 (`tr.set_drive_eject_confirm(c.t("slint.drive.eject_confirm").into());`), agregar:
```rust
    tr.set_drive_eject(c.t("slint.drive.eject").into());
```

- [ ] **Step 3: Verificar build**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-Object -Last 20
```
Expected: compila. `tr.set_drive_eject` existe porque la propiedad `drive-eject` está declarada en el `.slint`.

- [ ] **Step 4: Commit**

```
git add crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs
git commit -m "feat(i18n): exponer drive-eject (Expulsar/Eject) en Slint"
```

---

### Task 4: Reemplazar el `rfd` de expulsar USB por el modal de confirmación

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (handler `on_eject_drive` ~2318-2354; agregar `pending_eject` + `on_message_confirm` + `on_message_cancel`)

**Contexto:** el handler actual (líneas 2318-2354) muestra `rfd`, y si el usuario acepta, expulsa inline. Hay que: (a) crear el estado `pending_eject`, (b) en `on_eject_drive` poblar el `MessageVm` y guardar el path, (c) mover la lógica de expulsión a `on_message_confirm`. La importación de tipos generados de Slint ya trae `MessageVm` (se genera del struct del `.slint`).

- [ ] **Step 1: Crear el estado compartido `pending_eject`**

Buscar la zona donde se crean los `Rc<RefCell<...>>` compartidos antes de los handlers (cerca de donde se clona `ctrl` para los handlers; el handler de eject está ~2318). Justo ANTES del bloque `{ ... ui.on_eject_drive(...) ... }` (línea 2318), agregar:
```rust
    // Path pendiente de expulsar mientras el modal de confirmación está abierto.
    let pending_eject: std::rc::Rc<std::cell::RefCell<Option<String>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
```

- [ ] **Step 2: Reescribir `on_eject_drive` para abrir el modal**

Reemplazar el cuerpo del bloque `on_eject_drive` (líneas 2318-2354). El bloque completo nuevo:
```rust
    {
        let ui_weak = ui.as_weak();
        let pending_eject = pending_eject.clone();
        ui.on_eject_drive(move |path| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let tr = ui.global::<Tr>();
            let body = tr.get_drive_eject_confirm().replace("{drive}", path.as_str());
            *pending_eject.borrow_mut() = Some(path.to_string());
            ui.set_message(MessageVm {
                kind: 1,
                level: 1, // warning
                title: tr.get_drive_eject_confirm_title(),
                body: body.into(),
                confirm_label: tr.get_drive_eject(),
                cancel_label: tr.get_dlg_cancel(),
                danger: false,
            });
        });
    }
```

- [ ] **Step 3: Agregar el handler `on_message_confirm` (ejecuta la expulsión)**

Inmediatamente después del bloque de `on_eject_drive`, agregar el handler de confirmación. Captura `ctrl`, `refresh_drives` y `pending_eject` (igual que el viejo handler de eject):
```rust
    {
        let ui_weak = ui.as_weak();
        let ctrl = ctrl.clone();
        let refresh_drives = refresh_drives.clone();
        let pending_eject = pending_eject.clone();
        ui.on_message_confirm(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            let kind = ui.get_message().kind;
            // Cerrar el modal.
            ui.set_message(MessageVm::default());
            // kind 1 = confirmación de expulsar.
            if kind == 1 {
                if let Some(path) = pending_eject.borrow_mut().take() {
                    let outcome = ctrl
                        .borrow()
                        .eject_drive(std::path::PathBuf::from(path.as_str()));
                    let tr = ui.global::<Tr>();
                    let msg = match outcome {
                        workspace_ctrl::EjectOutcome::Ok => {
                            refresh_drives();
                            tr.get_drive_eject_ok()
                        }
                        workspace_ctrl::EjectOutcome::InUse => tr.get_drive_eject_in_use(),
                        workspace_ctrl::EjectOutcome::Failed(_) => tr.get_drive_eject_failed(),
                    };
                    ui.invoke_show_toast(msg);
                }
            }
        });
    }
```

- [ ] **Step 4: Agregar el handler `on_message_cancel`**

Después del bloque anterior:
```rust
    {
        let ui_weak = ui.as_weak();
        let pending_eject = pending_eject.clone();
        ui.on_message_cancel(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            ui.set_message(MessageVm::default());
            pending_eject.borrow_mut().take();
        });
    }
```

- [ ] **Step 5: Asegurar el `use` de `MessageVm`**

`MessageVm` se genera dentro del módulo de Slint (mismo lugar que `OpDialogVm`). Verificar que esté en scope: buscar dónde se importan los tipos Slint (un `use` con `slint::include_modules!` o un `use crate::...`). Como `OpDialogVm` ya se usa en `main.rs` (línea 467: `to_op_dialog_vm`), `MessageVm` está disponible por el mismo camino. Si el compilador se queja de `MessageVm` no encontrado, agregar el `use` análogo al de `OpDialogVm` en la cabecera de `main.rs`.

- [ ] **Step 6: Verificar build**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-Object -Last 30
```
Expected: compila. Si `MessageVm::default()` falla (no implementa Default), usar la forma explícita:
```rust
MessageVm { kind: 0, level: 0, title: Default::default(), body: Default::default(), confirm_label: Default::default(), cancel_label: Default::default(), danger: false }
```
(Los structs Slint generados sí implementan `Default`, así que `MessageVm::default()` debería andar; el fallback queda documentado por si acaso.)

- [ ] **Step 7: Commit**

```
git add crates/ui-slint/src/main.rs
git commit -m "feat(ui): expulsar USB usa el modal tematico (no rfd)"
```

---

### Task 5: `report()` usa el modal de aviso

**Files:**
- Modify: `crates/ui-slint/src/main.rs` (`fn report` ~3209-3217 + sus 4 llamadas ~2014/2028/2041/2092)

**Contexto:** `report()` es una función libre sin `ui`. Para mostrar el modal necesita el `Weak<AppWindow>`. Se cambia su firma a `report(ui: &slint::Weak<AppWindow>, r: Result<T, String>)` y se actualizan las 4 llamadas. Las llamadas viven dentro de closures que ya tienen un `ui_weak`/`as_weak` disponible o pueden capturar uno.

- [ ] **Step 1: Cambiar la firma de `report`**

Reemplazar la función (líneas 3208-3217):
```rust
/// Muestra un modal de error (temático) si el resultado de un import/export falló;
/// silencioso si OK.
fn report<T>(ui: &slint::Weak<AppWindow>, r: Result<T, String>) {
    if let Err(e) = r {
        if let Some(ui) = ui.upgrade() {
            let tr = ui.global::<Tr>();
            ui.set_message(MessageVm {
                kind: 2,
                level: 2, // error
                title: "Naygo".into(),
                body: e.into(),
                confirm_label: tr.get_dlg_accept(),
                cancel_label: Default::default(),
                danger: false,
            });
        }
    }
}
```

- [ ] **Step 2: Actualizar las 4 llamadas a `report`**

Las llamadas están en los closures de import/export de packs (~2014, 2028, 2041, 2092). Cada closure necesita un `Weak<AppWindow>` capturado. Revisar cada bloque: si ya hay un `ui` o `cfg_win`/`cfg_weak` en scope, derivar el weak. Patrón concreto — en el bloque de export (donde está `cfg_win.on_export_lang(...)` etc.), antes del closure agregar:
```rust
        let ui_weak = ui.as_weak();
```
y cambiar la llamada de:
```rust
                report(packs::export_lang(&c.config.config_dir, &code, &path));
```
a:
```rust
                report(&ui_weak, packs::export_lang(&c.config.config_dir, &code, &path));
```
Hacer lo mismo para `export_theme` (~2028), `export_config` (~2041) y el `report(Err::<(), String>(e))` (~2092 → `report(&ui_weak, Err::<(), String>(e))`). Si un closure ya mueve `ui_weak` para otra cosa, clonarlo (`let ui_weak2 = ui.as_weak();`) y usar ese.

- [ ] **Step 3: Verificar build**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-Object -Last 30
```
Expected: compila. Errores típicos: un closure no captura `ui_weak` → agregar el `let ui_weak = ui.as_weak();` antes del closure y moverlo (`move`).

- [ ] **Step 4: Commit**

```
git add crates/ui-slint/src/main.rs
git commit -m "feat(ui): errores de import/export usan el modal tematico"
```

---

### Task 6: Aclarar el texto del diálogo de panic

**Files:**
- Modify: `crates/ui-slint/src/logging.rs` (líneas 210-219)

**Contexto:** el panic SIGUE usando `rfd` (la UI puede estar rota; es el último recurso fiable). Solo cambia el texto: se quita el `{payload}` técnico crudo (ya se vuelca al log vía `log_line(&report)` en la línea 205).

- [ ] **Step 1: Reemplazar el `set_description` del diálogo de panic**

Reemplazar el bloque (líneas 210-219):
```rust
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Naygo — error inesperado")
            .set_description(format!(
                "Naygo se cerró por un error inesperado.\n\nGuardamos un registro \
                 técnico en:\n{}\n\nSi el problema se repite, ese archivo ayuda a \
                 diagnosticarlo.",
                log.display()
            ))
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
```

- [ ] **Step 2: Verificar que `payload` no quede como variable sin usar**

`payload` se sigue usando arriba (línea 196: `writeln!(report, "mensaje: {payload}")`), así que no hay warning de variable sin usar. Confirmar leyendo el contexto.

- [ ] **Step 3: Verificar build**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint 2>&1 | Select-Object -Last 20
```
Expected: compila sin warnings nuevos.

- [ ] **Step 4: Commit**

```
git add crates/ui-slint/src/logging.rs
git commit -m "feat(logging): texto claro en el dialogo de panic (detalle solo al log)"
```

---

### Task 7: Gate integral + documentación + graphify

**Files:**
- Modify: `CHANGELOG.md` (sección "Sin publicar" → "Corregido" o "Cambiado")

- [ ] **Step 1: `cargo fmt`**

Run:
```
cargo fmt
```
Expected: sin cambios pendientes o reformatea; `git diff --stat` muestra solo formato.

- [ ] **Step 2: Tests de los 3 crates**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace 2>&1 | Select-Object -Last 30
```
Expected: `naygo-core`, `naygo-platform`, `naygo-ui-slint` todos PASS (no se agregaron tests nuevos; verificar que no se rompió nada).

- [ ] **Step 3: Clippy estricto**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo clippy --workspace --all-targets -- -D warnings 2>&1 | Select-Object -Last 30
```
Expected: sin warnings.

- [ ] **Step 4: Barrido de voseo en archivos tocados**

Run (busca formas de voseo en los archivos nuevos/modificados):
```
git diff --name-only HEAD~6 | ForEach-Object { Select-String -Path $_ -Pattern "avisás|decís|querés|asegurate|hacé|tené|elegí|fijate|mirá" -ErrorAction SilentlyContinue }
```
Expected: sin coincidencias. (Si aparece alguna, corregir a español neutral: "asegúrate", "elige", etc.)

- [ ] **Step 5: Entrada de CHANGELOG**

En `CHANGELOG.md`, dentro de `## [Sin publicar]`, en la subsección `### Corregido` (o crear `### Cambiado` si encaja mejor), agregar:
```markdown
- Los avisos y confirmaciones internos (confirmar expulsar una unidad USB, errores al
  importar/exportar packs) ahora usan un diálogo con el tema de Naygo en vez del cuadro
  nativo del sistema. El mensaje de cierre por error inesperado es más claro y el detalle
  técnico queda en el registro (`naygo.log`).
```

- [ ] **Step 6: Commit de docs**

```
git add CHANGELOG.md
git commit -m "docs: CHANGELOG — modales tematicos"
```

- [ ] **Step 7: Actualizar el grafo**

Run:
```
graphify update .
```
Expected: actualiza `graphify-out/` sin error.

---

## Verificación visual (Nicolás, en la VM)

No automatizable. Tras `cargo build --release` + regenerar dist:
- Expulsar una unidad USB → aparece el modal oscuro de Naygo (no el cuadro de Windows), con botones "Expulsar"/"Cancelar"; Esc y clic-fuera cancelan; confirmar expulsa y muestra el toast.
- Provocar un error de import de pack (importar un `.zip` inválido) → modal de error temático con "Aceptar".
- (Opcional) provocar un panic → el cuadro nativo dice el texto nuevo, sin el mensaje técnico.
- Probar en tema claro y en uno oscuro: textos y botones legibles.
