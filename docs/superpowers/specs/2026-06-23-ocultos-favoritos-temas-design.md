# Bloque: ocultos + favoritos con grupos + editor de temas — Diseño

> Naygo (explorador de archivos Rust + Slint, render software, Windows).
> Autor: Nicolás Groth / ISGroth. Fecha: 2026-06-23.
> Tres features INDEPENDIENTES en un bloque, cada una con su sección. Se implementan juntas pero
> no se acoplan entre sí. Un solo build/dist al final.

---

## Feature 1 — Mostrar/ocultar ocultos, de sistema y dotfiles

Tres interruptores para controlar qué archivos/carpetas se muestran, con acceso rápido desde el
toolbar.

### Decisiones (con el usuario)
- Tres toggles: **mostrar ocultos** (HIDDEN), **mostrar de sistema** (SYSTEM), **ocultar dotfiles**
  (los que empiezan con `.`, estilo Linux).
- **Acceso rápido**: botón "ojo" con ▾ en el toolbar (menú con las 3 casillas). Persistente.
- Aplicación **global** (todos los paneles + el árbol).
- **Defaults (pedido de Nicolás): mostrar TODO por defecto** — `show_hidden=true`,
  `show_system=true`, `hide_dotfiles=false`. Naygo arranca mostrando ocultos, de sistema y dotfiles;
  el usuario los oculta con los toggles si quiere. (Difiere del Explorer de Windows a propósito.)

### Core
- `Entry` ya tiene `hidden: bool` pero está hardcodeado a `false` en `entry_from_path`
  (`listing.rs:145`). Ampliar: poblar `hidden` y un nuevo campo `system: bool` desde los atributos
  REALES de Windows con `std::os::windows::fs::MetadataExt::file_attributes()` (es std; NO requiere
  la crate `windows`). En no-Windows, ambos `false` (tolerante).
  - `FILE_ATTRIBUTE_HIDDEN = 0x2`, `FILE_ATTRIBUTE_SYSTEM = 0x4`.
- Función pura de visibilidad (en la capa de vista/filtro, donde ya se filtra y ordena):
  `fn is_visible(entry, show_hidden, show_system, hide_dotfiles) -> bool` —
  oculta si `entry.hidden && !show_hidden`; oculta si `entry.system && !show_system`; oculta si
  `hide_dotfiles && entry.name.starts_with('.')`. Testeable.

### Settings
`#[serde(default = "...")]` los tres: `show_hidden` (default **true**), `show_system` (default
**true**), `hide_dotfiles` (default **false**). Cada uno con su fn default explícita para los `true`.

### UI
- Botón "ojo" + ▾ en el toolbar → menú flotante (patrón de los menús ▾ existentes) con 3 casillas.
- Al cambiar un toggle: persistir + re-filtrar todos los paneles Files y el árbol al instante.
- El filtro se aplica donde se construye la vista (view_indices) y en el árbol (solo-carpetas).

### i18n
Claves nuevas: tooltip del botón, "Mostrar archivos ocultos", "Mostrar archivos de sistema",
"Ocultar archivos que empiezan con punto".

---

## Feature 2 — Favoritos con grupos anidados + ícono ▾ en el toolbar

Los favoritos pasan de lista plana a un árbol de grupos anidados; se añade un ícono ▾ en el toolbar
para navegarlos rápido.

### Decisiones (con el usuario)
- **Grupos anidados** (carpetas dentro de carpetas, árbol libre).
- **Gestión desde el panel de Favoritos** (árbol con clic derecho: nuevo grupo, renombrar,
  eliminar; arrastrar para mover). El **▾ del toolbar solo NAVEGA** (despliega el árbol para saltar).

### Core (`favorites.rs`)
Modelo nuevo de árbol. Un nodo es favorito o grupo:
```rust
pub enum FavNode {
    Favorite { path: PathBuf, label: String },
    Group { name: String, children: Vec<FavNode> },
}
pub struct Favorites { roots: Vec<FavNode> }
```
Operaciones puras (con tests): `add_favorite(path)` (al raíz, si no existe), `remove(path)`,
`contains(path)`, `new_group(parent_path_o_raíz, name)`, `rename_group`, `move_node`, `list_flat()`
(recorrido en orden para los atajos Ctrl+1..9, que siguen apuntando a los favoritos en orden de
aparición). **Migración**: un `favorites.json` viejo (lista plana `{items:[{path,label}]}`) se carga
como `roots` de favoritos al nivel raíz — `from_json` detecta el formato viejo y lo convierte. El
formato nuevo serializa el árbol.

### UI
- **Ícono ▾ en el toolbar** (patrón del menú de historial / USB): despliega el árbol de favoritos
  jerárquico (grupos expandibles con indentación; favoritos navegables). Clic en un favorito →
  navega el panel activo. Solo navegación.
- **Panel de Favoritos** (`PanePurpose::Favorites`): muestra el árbol EDITABLE. Clic derecho →
  Nuevo grupo / Renombrar / Eliminar. Arrastrar un favorito o grupo a otro grupo lo mueve
  (reusa el patrón de drag dentro de un árbol; si Slint no alcanza, mover vía menú "Mover a…").
- La estrella ☆/★ de la path-bar sigue agregando/quitando al nivel raíz (sin pedir grupo — se
  reorganiza después en el panel, decisión del usuario).

### i18n
Claves nuevas: tooltip del ▾ de favoritos, "Nuevo grupo", "Renombrar grupo", "Eliminar",
"Mover a…", "(favoritos vacíos)".

### Riesgo
La feature de mayor esfuerzo del bloque: modelo de árbol + panel editable con drag/menú + menú
jerárquico. El drag dentro del árbol es lo más incierto (Slint software); fallback: mover por menú
contextual "Mover a <grupo>".

---

## Feature 3 — Reducir temas a 5 + editor de colores en vivo

Recortar el catálogo de fábrica y permitir al usuario crear sus propios temas con un editor visual.

### Decisiones (con el usuario)
- **5 temas de fábrica**: Dark Blue, Windows XP, Verde sobre azul (green-on-blue), High Contrast,
  Neón Retro. Quitar los otros 10 builtin.
- **Editor de colores**: "Personalizar" sobre un tema → copia editable → editar los 11 tokens con
  **selector estilo Office** (grilla de presets + colores estándar + "Más colores…" con sliders
  R/G/B + hex) → **vista previa aplicada a toda la app en vivo** (Cancelar revierte, Guardar
  persiste como tema de usuario). Los 5 de fábrica quedan INTACTOS (solo se duplican).
- **Cuentagotas**: FUERA de este bloque (anotado para futuro — requiere Win32 que lee el
  framebuffer del escritorio).

### Core (`theme/mod.rs`)
- En `ThemeCatalog::load`, dejar en el array solo los 5: `dark-blue`, `winxp`, `green-on-blue`,
  `high-contrast`, `neon-retro`. Quitar los `include_str!` y los `.json` de los otros 10
  (`dark-teal`, `light`, `citrus-glow`, `ocean-midnight`, `ember-forge`, `polar-graphite`, `macos`,
  `solarized-dark`, `amber-terminal`, `plum-dusk`).
- Si el tema activo guardado ya no existe → cae a `default_id()` (dark-blue). El `get()` ya tolera
  id desconocido cayendo al default.
- Guardar un tema de usuario: `Theme::to_json()` (ya existe el serialize) en
  `<config>/themes/<id>.json` (el `load` ya levanta los sueltos). Helper para nombre → id (slug).
- Marcar cuáles son de fábrica (los 5 embebidos) vs de usuario (los de `<config>/themes/`), para
  permitir editar/borrar solo los de usuario.

### UI (config-window, sección Apariencia)
- Cada tema en la galería: los 5 de fábrica con botón "Personalizar" (duplica); los de usuario con
  "Editar" y "Eliminar".
- **Editor**: lista de los 11 tokens (muestra + nombre + hex). Al tocar un token, abre el selector
  de color estilo Office:
  - Grilla de presets (fila base + 4 filas de variaciones claro→oscuro) + fila "Colores estándar".
  - "Más colores…" → sliders R/G/B (0-255) + campo hex + muestra grande. Bidireccional
    (slider↔hex↔muestra).
  - Todo dibujado con Path/Rectangle (render software; sin color picker nativo).
- **Preview en vivo**: cada cambio aplica el tema a toda la app (mismo mecanismo de aplicar tema que
  ya existe). Guardar persiste; Cancelar restaura el tema que estaba antes de entrar al editor.
- Campo de nombre del tema + base (Oscuro/Claro, que afecta los defaults de tokens faltantes).

### i18n
Claves nuevas: "Personalizar", "Editar", "Eliminar tema", "Guardar tema", "Restaurar de fábrica",
"Más colores…", "Colores estándar", "Nombre del tema", "Base", nombres de los 11 tokens.

### Riesgo
Segunda pieza más grande: la grilla de presets + sliders + preview en vivo es UI Slint nueva. El
backend de temas de usuario (JSON) ya existe.

---

## Testing
- F1: `is_visible` (los 8 combos de flags); lectura de atributos (mock de hidden/system).
- F2: modelo de árbol (add/remove/group/rename/move/list_flat), migración del JSON viejo→árbol.
- F3: catálogo reducido (5 ids), tema activo inexistente→default, round-trip de tema de usuario,
  slug de nombre.
- UI (menús, panel editable, editor de color): verificación visual en la VM.

## i18n general
Triple (es + en en `crates/core/src/i18n/{es,en}.json` + props i18n.slint + setters i18n_keys.rs),
español neutral SIN voseo, sin reutilizar claves.

## Orden de implementación sugerido
1. **F1 (ocultos)**: la más acotada — core (atributos + is_visible) → Settings → menú toolbar.
2. **F3 (temas)**: reducir catálogo (rápido) → editor de color → preview en vivo.
3. **F2 (favoritos)**: el árbol — modelo + migración → panel editable → menú ▾. La más grande, al final.
4. Cierre: gate + CHANGELOG + guía + dist.

## Fuera de alcance (YAGNI)
- Cuentagotas de pantalla (Win32 framebuffer) — anotado para futuro.
- Toggles de ocultos por panel (se eligió global).
- Editar los temas de fábrica (solo se duplican).
