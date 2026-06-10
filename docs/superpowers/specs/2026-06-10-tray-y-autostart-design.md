# Ícono en bandeja del sistema + inicio con Windows — Diseño (Entrega 3)

> Autor: Nicolás Groth / ISGroth — 2026-06-10. MIT License.

## Qué es

Pedido de Nicolás: "un ícono en la barra de tareas permanente (al lado del reloj) que
permita abrir rápidamente el aplicativo y que permita cargarlo al iniciar Windows".

## Diseño

### Ícono en la bandeja (system tray)
- Crate **`tray-icon`** (Tauri, MIT — dependencia libre) en `naygo-ui`: maneja
  `Shell_NotifyIcon` + menú nativo + eventos por canal. Se crea en el hilo del event
  loop (en `NaygoApp::new`) y vive en el struct (drop = desaparece).
- Ícono: el `naygo_icon.ico` embebido (feature `ico` del crate `image`), reescalado
  a 32×32 RGBA.
- **Clic izquierdo** en el ícono → mostrar + enfocar la ventana principal
  (`ViewportCommand::Visible(true)` + `Focus`). **Menú** (clic derecho): "Abrir
  Naygo" / separador / "Salir".
- **Despertar con la app idle/oculta**: los handlers de eventos del tray
  (`set_event_handler`) reciben un clon del `egui::Context` y llaman
  `request_repaint()` (thread-safe) → el próximo frame drena los canales de eventos
  en `logic()`. Sin polling: bajo consumo intacto.

### Comportamiento de cierre
- Setting `close_to_tray` (default **false** — no sorprender): si está activo y hay
  tray, `close_requested` → `CancelClose` + ocultar ventana. La app queda residente
  (ventana oculta = 0 frames = ~0 CPU; el costo es solo memoria).
- "Salir" del menú del tray (o `close_to_tray` off) → cierre real. El guardado de
  workspace/settings del cierre ya existe (persistencia real); al ocultar a bandeja
  también se guarda (mismo hook de `close_requested`).

### Inicio con Windows (autostart)
- `naygo-platform/src/autostart.rs`: clave de registro
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, valor `Naygo` = ruta del exe
  entre comillas. `set_enabled(bool) -> Result` + `is_enabled() -> bool` con el crate
  `windows` (Registry API). Sin permisos de admin (HKCU).
- El checkbox refleja el **registro real** (no se duplica en settings.json): estado
  leído al abrir la sección, escrito al toggle.

### Settings + UI
- `Settings`: `tray_enabled: bool` (default **true**), `close_to_tray: bool`
  (default false). `#[serde(default)]` retro-compat.
- Configuración → Avanzado, heading "Integración con Windows": los 2 checkboxes +
  autostart. `close_to_tray` deshabilitado si `tray_enabled` off.
- Toggle de `tray_enabled` en caliente: crea/destruye el TrayIcon.
- i18n ES/EN: `settings.system.*`, `tray.open`, `tray.exit`.

### Tensión con bajo consumo (explícita)
Residente en bandeja = la app sigue en RAM. Por eso `close_to_tray` es **opt-in**
(default cerrar de verdad), y oculta la app no repinta (0 GPU/CPU). El tray en sí es
costo cero (ícono del shell + eventos por canal).

### Tests
- platform::autostart: test `#[ignore]`d (escribe registro real) + verificación
  manual/en vivo.
- Resto UI: verificación en vivo (computer-use): ícono aparece junto al reloj, menú
  Abrir/Salir, ocultar a bandeja y restaurar, checkbox autostart escribe/borra la
  clave (verificable con `reg query`).
