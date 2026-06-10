# Persistencia real del workspace + valores de fábrica — Diseño

> Autor: Nicolás Groth / ISGroth — 2026-06-10. MIT License.

## Problema

Nicolás pidió: (a) que Naygo recuerde la config visual y los paths abiertos por panel
al reabrir, (b) restaurar valores de fábrica y arranque seguro ante config corrupta.

**Diagnóstico**: la persistencia YA está implementada de punta a punta
(`Settings` en settings.json; `WorkspacePersist` con layout + panel activo +
`FilePanePersist{current_dir, sort, view, …}` por panel; carga al arrancar en
`app.rs`). Pero el guardado vive SOLO en `eframe::App::save()`, y eframe 0.34 **no
incluye la feature `persistence` en sus defaults** → sin storage, `save()` jamás se
invoca → **nunca se escribe nada**. Verificado: tras decenas de cierres no existe
ningún `.json` junto al exe.

## Diseño

No dependemos de eframe: guardado explícito, dirigido por nosotros.

### 1. Guardado de settings — inmediato al cambiar
`NaygoApp` guarda un snapshot `last_saved_settings: Settings`. Al final de cada
frame, si `settings != snapshot` → `config::save_settings` + actualizar snapshot.
`Settings` gana `derive(PartialEq)` (y sus tipos anidados si falta). Costo por frame
despreciable (struct chica); escribe solo cuando hubo cambio real.

### 2. Guardado de workspace — al cerrar + autosave
- **Cierre**: en `logic()`, si `ctx.input(|i| i.viewport().close_requested())` →
  `save_workspace()` + `save_settings` (el frame del CloseRequested siempre llega
  antes de cerrar).
- **Autosave**: cada 60 s (`Instant` en `NaygoApp`) → `save_workspace()`. Tolera
  matar el proceso / crash sin perder más de 1 min de estado. Escritura ~KB, async no
  necesario (es 1 vez/min; el write de `save_workspace` ya es tolerante a fallos).
- `App::save()` se conserva (inofensivo sin storage; si algún día se activa la
  feature, suma).

### 3. Arranque seguro ante config corrupta
`read_json` (config/mod.rs) hoy degrada a default y loguea. Se agrega: si el parse
FALLA (archivo existe pero malformado) → renombrar a `<nombre>.bad` (backup, no se
pierde) y devolver `None`. Próximo arranque: limpio con defaults. La UI muestra un
mensaje en la barra de estado al detectar esto al arrancar (clave i18n
`status.config_recovered`).

### 4. Restaurar valores de fábrica
Botón "Restaurar valores de fábrica" en Configuración → sección General, con
confirmación inline (dos clics: el botón cambia a "¿Confirmar restauración?" 4 s).
Acción (vía el patrón de acciones diferidas existente):
- `settings = Settings::default()` (idioma re-detectado del SO como primer arranque)
- workspace → layout por defecto (mismo camino que un primer arranque sin
  workspace.json) con los paneles en el dir home
- guardar ambos de inmediato; status `status.factory_done`.
No toca: templates de layout guardadas por el usuario, keymap, logs.

### i18n
Claves nuevas ES+EN: `settings.factory_reset`, `settings.factory_reset_confirm`,
`status.factory_done`, `status.config_recovered`.

### Tests
- core: read_json corrupto → renombra a .bad y devuelve None (tempfile).
- core: round-trip Settings PartialEq (igualdad tras save/load).
- UI: manual + verificación en vivo (computer-use): cambiar tema → cerrar con X →
  reabrir → tema y paths por panel restaurados; botón fábrica restaura y persiste.

### Fuera de alcance (entrega siguiente)
Embellecer la ventana de Configuración (pedido 3 de Nicolás) — diseño visual, se
hará con su input/companion.
