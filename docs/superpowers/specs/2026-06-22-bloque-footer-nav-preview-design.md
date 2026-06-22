# Bloque: footer + navegación + auto-resaltado + copia-preview — Diseño

> Naygo (Rust + Slint, render software). Autor: Nicolás Groth / ISGroth. Fecha: 2026-06-22.
> Cuatro mejoras que se implementan juntas en un solo build/dist.

Este documento reúne las 4 mejoras del bloque. El footer tiene su spec detallada aparte
(`2026-06-22-footer-por-panel-design.md`); aquí se resume y se especifican las otras tres.

---

## 1. Footer por panel

Ver spec dedicada: `docs/superpowers/specs/2026-06-22-footer-por-panel-design.md`.
Resumen: barra al pie de cada panel con datos propios (selección, disco); plantilla **global**
configurable (5 campos por defecto, presets + personalizada con tokens) en Avanzado.

---

## 2. Botones Atrás / Adelante / Home en el toolbar

**Ya existe (reuso):** `WorkspaceCtrl::on_go_back()`/`on_go_forward()` (sobre
`core::workspace::nav_history::NavHistory`, el mismo historial de los botones del mouse) y el
componente `ToolBtn` (íconos por Path). NO se toca la lógica de navegación.

**Se agrega:**
- Tres `ToolBtn` al inicio del toolbar, orden navegador: `◀ Atrás` · `▶ Adelante` · `🏠 Home`,
  antes del botón "Subir nivel" ya existente. Íconos dibujados con Path (chevrones + casita).
- Atrás/Adelante: invocan `on_go_back()`/`on_go_forward()` del panel activo. **Deshabilitados**
  (atenuados, sin hover/clic) cuando no hay a dónde ir. Para eso se exponen
  `can_go_back()`/`can_go_forward()` desde `NavHistory` (el cursor ya sabe si está al borde).
- Home: navega el panel activo a la carpeta Home (ver Settings). Atajo de teclado **Alt+Home**
  (acción nueva `Action::GoHome` en el keymap). Si la carpeta Home no existe → cae en el aviso
  "carpeta no encontrada" ya existente (no inventa comportamiento).

**Settings nuevo:**
- `home_dir: String` (`#[serde(default)]`, default `""`). **Vacío = carpeta personal del usuario**
  (`%USERPROFILE%` / `dirs::home_dir`). Si tiene una ruta, usa esa.
- En **Configuración → Avanzado**: campo "Carpeta de inicio (Home)" + botón "Examinar…"
  (FileDialog nativo) + nota "Vacío = carpeta personal del usuario".

**Resolución de Home** (`core` o helper de main): `if home_dir.is_empty() { dirs::home_dir() }
else { PathBuf::from(home_dir) }`. Pura y testeable.

---

## 3. Toggle global de auto-resaltado de código

**Decidido con el usuario.** Switch en **Configuración → Previsualización**, ARRIBA de la tabla de
reglas. ON por defecto.

- Settings nuevo `auto_highlight_code: bool` (`#[serde(default = "..._true")]`, default `true`).
- Cuando **ON**: en `ViewMode::Auto`, si la extensión es una de las 16 conocidas de `CodeLang`
  (.sql/.rs/.json/.xml/.js/.html/.css/.c/.cpp/.java/.py/.sh/.md/.yaml/.toml/.ini), se resalta sola
  (el worker calcula el `CodeLang` por extensión y aplica syntect).
- Cuando **OFF**: `ViewMode::Auto` es texto plano (comportamiento previo).
- El combobox por-extensión (Code(lang)/Text/Image) **sigue mandando** sobre el global: si una
  regla fuerza un modo explícito, ese gana.

**Implementación:** `core::preview::code_lang_for(path, rules, auto_highlight)` gana un parámetro;
cuando la regla es `Auto` y `auto_highlight==true`, intenta `CodeLang::from_ext`. El worker de
preview pasa el flag desde Settings.

---

## 4. Selección + copia en el preview de texto

**Decidido con el usuario.** El preview de texto tendrá:
- Modo **coloreado** (actual, syntect, Text por segmento — NO seleccionable).
- Un **botón en la barra del preview** que alterna a vista **SELECCIONABLE**: un `TextEdit` de
  solo lectura (texto plano, una sola tinta) con selección y copia nativas (mouse + Ctrl+C).
- El botón de alternar **solo aparece cuando hay código resaltado** (modo coloreado activo). El
  texto plano (no resaltado) **siempre** es seleccionable: se cambia el `Text` actual del modo
  plano por un `TextEdit` read-only.

**Implementación UI (`preview-panel.slint`):**
- Estado local `selectable: bool` (toggle del botón).
- Si `highlighted && !selectable`: render coloreado actual (Text por segmento).
- Si `highlighted && selectable`: `TextEdit { read-only: true; text: view.text }` (texto plano).
- Si `!highlighted`: siempre `TextEdit` read-only (reemplaza el `Text` plano actual).
- Botón "seleccionar texto" en la barra, visible solo si `view.highlighted`. Ícono por Path.

`TextEdit` de Slint da selección y Ctrl+C nativos en render software (una sola tinta, sin color
por token — es el trade-off aceptado para poder copiar).

---

## i18n (todas, triple, español neutral sin voseo)

Claves nuevas para: botones Atrás/Adelante/Home + tooltips, "Carpeta de inicio (Home)" + nota,
"Resaltar código automáticamente" (toggle), "Seleccionar texto" (botón preview), y las del footer
(ver su spec). **No reutilizar nombres de claves existentes.**

## Testing

- `core::footer::render` (ver spec footer).
- Resolución de Home (vacío → perfil; ruta → esa).
- `code_lang_for` con `auto_highlight` on/off y regla Auto vs forzada.
- Round-trip serde de los Settings nuevos + migración (defaults).
- UI (botones, toggles, selección de preview): verificación visual en la VM.

## Orden de implementación sugerido

1. core: `footer` + resolución Home + `code_lang_for(auto)` + Settings nuevos (con tests).
2. UI config (Avanzado): footer + Home + toggle auto-resaltado.
3. UI toolbar: botones nav + Home + keymap Alt+Home.
4. UI footer en file-panel + flujo de datos en sync_rows.
5. UI preview: toggle seleccionable + TextEdit.
6. Gate completo + dist.

## Fuera de alcance (YAGNI)

- Plantilla de footer por panel (global decidido).
- Multiplataforma (anotado aparte como inquietud futura).
- Resaltado por token en el modo seleccionable (TextEdit es monocolor; trade-off aceptado).
