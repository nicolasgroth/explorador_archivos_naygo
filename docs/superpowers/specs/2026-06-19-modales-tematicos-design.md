# Diálogos temáticos (reemplazar `rfd::MessageDialog` por modales Slint)

> Spec de diseño. Fecha: 2026-06-19. Autor: Nicolás Groth / ISGroth.

## Objetivo

Los mensajes y confirmaciones que hoy salen como diálogos nativos del sistema
(`rfd::MessageDialog`) rompen visualmente con el tema oscuro de Naygo. Este trabajo
reemplaza los diálogos **controlados con la ventana viva** por un modal Slint que usa
el tema activo, reutilizando el patrón ya existente de `OpDialogs`. El mensaje del
diálogo de **panic** se aclara (sigue siendo nativo por seguridad). Los diálogos de
**línea de comandos** y los **selectores de archivo/carpeta** del SO no cambian.

## Inventario y clasificación

Búsqueda de `rfd::MessageDialog` en `crates/ui-slint/src/`:

| Sitio | Qué es | Cuándo ocurre | Acción |
|---|---|---|---|
| `main.rs:2331` | Confirmar expulsar unidad USB | ventana viva | **Reskinear** (modal Slint, confirmación) |
| `main.rs:3211` (`report()`) | Error de import/export de packs | ventana viva | **Reskinear** (modal Slint, aviso) |
| `logging.rs:210` | Panic — "error inesperado" | UI puede estar rota | **Solo mejorar texto** (sigue en `rfd`) |
| `main.rs:179` | `--help` | antes de crear la ventana | sin cambios |
| `main.rs:188` | `--version` | antes de crear la ventana | sin cambios |
| `main.rs:272` | Aviso de argumentos inválidos | antes de crear la ventana | sin cambios |

Los `rfd::FileDialog` (`main.rs:2009/2023/2036/2051/2719`) son selectores del SO y se
quedan nativos por definición (no se reskinnean).

**Razón de no tocar el panic con un modal Slint:** cuando salta un panic, el bug que lo
causó puede haber dejado el árbol de UI inutilizable o el event-loop colgado; abrir un
componente Slint propio podría no pintarse o volver a paniquear. `rfd` es el último
recurso fiable. Lo único que cambia es el **texto**: deja de mostrarse el mensaje técnico
crudo (p. ej. `called Option::unwrap() on a None value`) — ese detalle ya se vuelca al
`naygo.log` vía `log_line(&report)`.

## Componente nuevo: `MessageModal`

Hermano de `OpDialogs`, mismo look-and-feel: velo `#000000aa`, tarjeta centrada
`Theme.row-bg` con borde `Theme.selection-bg`/acento, botones `DlgBtn`
(primary/danger/normal), cierre por velo + Esc + botones, todo el texto por i18n.

Lo gobierna un `MessageVm` simple, separado de `OpDialogVm` (que está atado a semántica
de operaciones de archivo y no debe contaminarse):

```slint
// types.slint
export struct MessageVm {
    kind: int,            // 0=oculto, 1=confirmación (2 botones), 2=aviso (1 botón)
    level: int,           // 0=info, 1=warning, 2=error → tiñe la franja del título
    title: string,
    body: string,
    confirm-label: string,   // p.ej. "Expulsar" / "Aceptar"
    cancel-label: string,    // "Cancelar" (solo kind==1)
    danger: bool,            // botón de confirmar en color error
}
```

Comportamiento:

- **kind 1 (confirmación):** dos botones. Confirmar → callback `message-confirm()`.
  Cancelar / clic en el velo / Esc → `message-cancel()`.
- **kind 2 (aviso):** un botón ("Aceptar"). Botón / velo / Esc → `message-confirm()`
  (cerrar; no hay distinción confirmar/cancelar).
- **level** pinta una franja vertical fina a la izquierda del título:
  `0`→`Theme.accent`, `1`→`Theme.error` (ámbar/rojo según tema; el token de aviso es el
  mismo `error`), `2`→`Theme.error`. Sin glifos Unicode (el render por software no los
  dibuja): solo color + texto, igual que el resto de Naygo.
- `visible: vm.kind > 0`.

## Cableado en `app-window.slint`

Análogo a `OpDialogs`:

- `in property <MessageVm> message;`
- `callback message-confirm();`
- `callback message-cancel();`
- Instancia de `MessageModal { vm: root.message; message-confirm() => { root.message-confirm(); } message-cancel() => { root.message-cancel(); } }`, colocada **después** de `OpDialogs` en el z-order (encima), para que un aviso pueda superponerse si hiciera falta.

## Cableado en `main.rs`

Como la confirmación deja de ser bloqueante (con `rfd` el hilo se congela hasta
responder; con el modal la app sigue viva detrás del velo — comportamiento correcto y
consistente con Naygo), hay que recordar **qué hacer al confirmar**. Solo hay un caso de
confirmación (expulsar) y uno de aviso (error sin acción), así que basta con guardar el
dato pendiente:

```rust
// Estado compartido para el modal de mensajes.
let pending_eject: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
```

**Helper para abrir un aviso (kind 2):**

```rust
fn show_message(ui: &AppWindow, level: i32, title: &str, body: String) {
    ui.set_message(MessageVm {
        kind: 2,
        level,
        title: title.into(),
        body: body.into(),
        confirm_label: ui.global::<Tr>().get_dlg_accept(),
        cancel_label: SharedString::new(),
        danger: false,
    });
}
```

**Expulsar USB (`on_eject_drive`, ~2322):** en vez de mostrar `rfd` y expulsar en línea,
poblar el VM de confirmación y guardar el path; la expulsión real se mueve al callback
`message-confirm`.

```rust
ui.on_eject_drive(move |path| {
    let Some(ui) = ui_weak.upgrade() else { return; };
    let tr = ui.global::<Tr>();
    let body = tr.get_drive_eject_confirm().replace("{drive}", path.as_str());
    *pending_eject.borrow_mut() = Some(path.to_string());
    ui.set_message(MessageVm {
        kind: 1,
        level: 1, // warning
        title: tr.get_drive_eject_confirm_title(),
        body: body.into(),
        confirm_label: tr.get_drive_eject(),   // clave nueva: "Expulsar"
        cancel_label: tr.get_dlg_cancel(),
        danger: false,                          // expulsar no es destructivo
    });
});
```

**`message-confirm` (despacha la acción pendiente):**

```rust
ui.on_message_confirm(move || {
    let Some(ui) = ui_weak.upgrade() else { return; };
    let kind = ui.get_message().kind;
    ui.set_message(MessageVm::default()); // cerrar (kind 0)
    if kind == 1 {
        if let Some(path) = pending_eject.borrow_mut().take() {
            // ... lógica de eject_drive + toast + refresh_drives (la que hoy está inline) ...
        }
    }
});
```

**`message-cancel`:** cierra el modal (`set_message(default)`) y limpia `pending_eject`.

**`report()` (~3209):** pasa de `rfd` a `show_message(level=error)`. Como `report()` es
una función libre sin acceso a `ui`, se la convierte en un closure/método que captura el
`Weak<AppWindow>` (o se le pasa el `ui` por parámetro). Mantiene la firma semántica
"silencioso si Ok, muestra error si Err".

## Mejora de texto del panic (`logging.rs`)

El `format!` del `set_description` cambia para **no** incluir `{payload}`:

```rust
.set_description(format!(
    "Naygo se cerró por un error inesperado.\n\nGuardamos un registro técnico en:\n{}\n\n\
     Si el problema se repite, ese archivo ayuda a diagnosticarlo.",
    log.display()
))
```

El `{payload}` y el backtrace siguen yendo al log (vía `log_line(&report)`, intacto).

## Claves i18n nuevas

En `i18n.slint`, `es.json`, `en.json` y `i18n_keys.rs`:

- `drive-eject` / `slint.drive.eject` → ES "Expulsar", EN "Eject".

Se **reutilizan** `drive-eject-confirm`, `drive-eject-confirm-title`, `dlg-accept`,
`dlg-cancel` (ya existen). NO se crean claves con nombres ya usados (evitar la colisión
que ya ocurrió con `history-empty`).

## Arquitectura / capas

- `core`: sin cambios (los modales son UI pura; no hay lógica de negocio nueva).
- `platform`: sin cambios.
- `ui`: `MessageModal` nuevo + `MessageVm` + cableado en `main.rs` + claves i18n.

## Manejo de errores y casos borde

- **Doble disparo:** si ya hay un modal abierto y llega otro `eject`, el nuevo VM
  sobrescribe al anterior (igual que `OpDialogs`); aceptable, el usuario ve el último.
- **`pending_eject` colgado:** `message-cancel` siempre limpia el path pendiente; el
  `take()` en confirm también lo vacía. No queda estado fantasma.
- **Tema claro:** la franja de color y los textos usan tokens del tema → legibles en los
  15 temas (el botón danger usa `Theme.error`, ya validado en `OpDialogs`).
- **El panic con UI viva:** no aplica — el panic nunca pasa por el modal Slint.

## Pruebas

- `core`: sin tests nuevos (no toca core).
- `ui-slint`: el código Slint no es unit-testeable directamente; la verificación es de
  compilación (`cargo build` compila el `.slint`) + revisión de contrato. Se agrega, si
  cabe, un test de que `show_message` arma el `MessageVm` esperado (helper puro sobre el
  struct generado). Verificación visual final en la VM por Nicolás.
- Gate: `cargo fmt`, `cargo test` (3 crates), `cargo clippy --workspace --all-targets -- -D warnings`, barrido de voseo en archivos nuevos, `graphify update .`.

## Fuera de alcance

- Reskinear los `rfd::FileDialog` (selectores del SO).
- Reskinear los diálogos de CLI (`--help/--version/avisos`): ocurren antes de que exista
  la ventana. Podría hacerse con una mini-ventana Slint temática en el futuro.
- Un modal Slint para el panic (riesgo de UI rota).
