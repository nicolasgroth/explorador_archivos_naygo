# Paleta de comandos (Ctrl+P) + menú de historial — Diseño

> Naygo (explorador de archivos Rust + Slint, render software, Windows).
> Autor: Nicolás Groth / ISGroth. Fecha: 2026-06-22.
> Dos features que se implementan juntas en este desarrollo (un solo build/dist al final).

---

## Feature A — Paleta de comandos (Ctrl+P)

Overlay tipo VS Code: se abre con un atajo, se escribe en un campo de búsqueda y se filtra/salta
a acciones, carpetas recientes, favoritos, temas y configuración. Objetivo: acelerar todo por
teclado (prioridad de velocidad del proyecto).

### Decisiones tomadas (con el usuario)

- **Atajo**: Ctrl+P por defecto, pero como **acción del keymap** (`Action::CommandPalette`),
  editable en Configuración → Atajos como cualquier otra.
- **Categorías** (las 5): Acciones (curadas del keymap), Archivos de la carpeta actual,
  Carpetas recientes, Favoritos, Temas + "Abrir configuración".
- **Matching**: fuzzy (subsecuencia) con ranking. Resalta las letras coincidentes.
- **Aspecto**: overlay centrado arriba, sobre velo oscuro semitransparente. Cada resultado:
  ícono por categoría + nombre con letras resaltadas + etiqueta de categoría + atajo a la derecha.
  ↑↓ mueven, Enter ejecuta, Esc cierra. Con query vacío → lista por defecto.

### Lógica (core) — módulo nuevo `core::palette`

Puro, sin UI ni Windows. 100% testeable.

```rust
/// Categoría de un comando (define el ícono y la etiqueta).
pub enum CommandCategory { Action, File, Recent, Favorite, Theme, Config }

/// Qué ejecuta un comando al elegirlo.
pub enum CommandPayload {
    Action(crate::keymap::Action),  // se rutea por el dispatcher de teclado existente
    Navigate(std::path::PathBuf),   // navegar el panel activo a esta ruta (reciente/favorito)
    /// Enfocar/seleccionar un archivo o carpeta YA cargado en el panel activo. Lleva el índice
    /// de VISTA del entry (no la ruta): la paleta no toca disco, solo mueve foco/selección.
    /// Si el entry es carpeta y el usuario lo eligió, se puede navegar; si es archivo, se enfoca.
    FocusEntry(usize),
    Theme(crate::theme::ThemeId),   // aplicar este tema
    OpenConfig,                     // abrir la ventana de configuración
}

/// Un comando de la paleta.
pub struct Command {
    pub label: String,             // texto traducido a mostrar
    pub category: CommandCategory,
    pub shortcut: Option<String>,  // chord legible ("Ctrl+C"), solo acciones con atajo
    pub payload: CommandPayload,
}

/// Resultado de filtrar: el comando + su score + las posiciones de las letras coincidentes
/// (para resaltarlas en la UI).
pub struct CommandMatch {
    pub index: usize,              // índice en la lista original de comandos
    pub score: i32,
    pub hit_positions: Vec<usize>, // bytes/char idx del label que matchearon
}

/// Matching fuzzy de subsecuencia con ranking. `None` si no matchea. Score más alto = mejor
/// (prefijo > inicio de palabra > disperso). Case-insensitive.
pub fn fuzzy_match(query: &str, text: &str) -> Option<(i32, Vec<usize>)>;

/// Filtra y ordena los comandos por la query. Query vacía → todos en orden de presentación
/// (la UI puede recortar a "lista por defecto").
pub fn filter_and_rank(commands: &[Command], query: &str) -> Vec<CommandMatch>;
```

**Construcción de la lista** (vive en la UI/controlador, NO en core puro, porque las fuentes —
recientes/favoritos/temas/keymap— son estado de la app): una función `build_palette_commands`
en el controlador que arma `Vec<Command>` desde:
- **Acciones curadas**: lista EXPLÍCITA (constante) de las acciones que tienen sentido en la
  paleta — Copiar, Cortar, Pegar, Renombrar, BatchRename, NewFile, NewDir, ComputeSize, Refresh,
  Find, Undo, GoUp, GoBack, GoForward, GoHome, SwitchPane, CopyToOther, MoveToOther, SelectAll,
  Help, EditPath. Se EXCLUYEN las de bajo nivel (MoveUp/MoveDown/FocusPageDown/ExtendUp/
  FocusUpKeep/ToggleSelect/GoFavorite1..9/etc.). El label sale de `Action::i18n_key()`; el
  shortcut del keymap (`chords_for(action)` → texto del primer chord).
- **Archivos de la carpeta actual**: los entries YA cargados en el panel activo (la vista
  filtrada actual). Label = nombre del archivo/carpeta; payload `FocusEntry(view_index)`.
  Instantáneo (ya están en memoria); NO toca disco. Si la carpeta tiene muchísimos entries, se
  puede tope a un máximo razonable para la lista por defecto (el fuzzy igual recorre todos al
  filtrar — es CPU puro sobre RAM, barato).
- **Recientes**: del historial de carpetas recientes (label = nombre de carpeta, payload Navigate).
- **Favoritos**: de la lista de favoritos persistida (label = nombre, payload Navigate).
- **Temas**: del ThemeCatalog (label = "Tema: <nombre>", payload Theme).
- **Config**: una entrada fija "Abrir configuración" (payload OpenConfig).

### UI (Slint) — `command-palette.slint`

- Componente overlay: `Rectangle` velo a pantalla completa (semitransparente, `TouchArea` que
  cierra al clic fuera) + tarjeta centrada arriba con:
  - Campo de búsqueda (`LineEdit` o TextInput con foco al abrir).
  - Lista de resultados: por fila → ícono por categoría (Path-drawn), label con segmentos
    resaltados (las `hit_positions` → spans en `Theme.accent`), etiqueta de categoría, atajo a
    la derecha. Fila seleccionada con fondo `Theme.selection-bg`.
  - Pie con ayuda de teclas (↑↓ moverse · ↵ ejecutar · Esc cerrar).
- **VM**: `palette-open: bool`, `palette-query: string`, `palette-results: [CommandVm]`
  (label-spans/category/shortcut/icon-kind), `palette-selected: int`.
- **Teclado**: el campo tiene foco; ↑↓ mueven `palette-selected` (con wrap o clamp), Enter
  ejecuta el seleccionado, Esc cierra. Al teclear → callback a Rust recalcula `palette-results`
  (filter_and_rank) y resetea selección a 0.
- **Disparo**: `Action::CommandPalette` (default Ctrl+P) en el dispatcher abre el overlay. OJO:
  el overlay debe interceptar el teclado mientras está abierto (el `on_key` del panel NO debe
  actuar con la paleta abierta — patrón ya usado con `pending_dialog` en op-dialogs).
- **Ejecución** (en el controlador): `execute_palette_command(index)` despacha por payload —
  Action → rutea por el mismo camino que `on_key`; Navigate → navega el panel activo;
  FocusEntry(view_idx) → enfoca/selecciona ese entry en el panel activo y hace scroll a él (si
  es carpeta, opcionalmente entra); Theme → aplica y persiste; OpenConfig → abre la ventana.
  Luego cierra la paleta.

### Regla de oro

El fuzzy corre sobre ~30-50 comandos en memoria: CPU puro, cero I/O. Sin riesgo de rendimiento.

---

## Feature B — Menú ▾ de historial en Atrás/Adelante

Junto a los botones Atrás/Adelante del toolbar, un triángulo ▾ que despliega la lista de carpetas
en esa dirección y permite saltar a una. Mismo patrón visual que el ▾ de Expulsar de la tira USB.

### Estado actual (ya existe)

- Los botones Atrás/Adelante ya se atenúan con `can_go_back()`/`can_go_forward()`.
- `NavHistory::stack() -> (&[PathBuf], Option<usize>)` ya da la pila completa + el cursor
  (comentario en el código: "Para el menú de historial del botón atrás/adelante").

### Lógica (core, con tests) — en `nav_history.rs`

```rust
/// Rutas hacia ATRÁS desde el cursor (de la más cercana a la más lejana). Vacío si no hay.
pub fn back_entries(&self) -> Vec<PathBuf>;
/// Rutas hacia ADELANTE desde el cursor (de la más cercana a la más lejana). Vacío si no hay.
pub fn forward_entries(&self) -> Vec<PathBuf>;
```

### Controlador

`go_to_history(target: PathBuf | pos)`: mueve el cursor del NavHistory del panel activo a esa
entrada y navega (lista) sin re-apilar. El NavHistory ya tiene `jump_to`/cursor; reusar.

### UI (toolbar)

- Junto a Atrás y Adelante, un ▾ pequeño (patrón del ▾ de Expulsar USB de la tira de discos).
- Clic en el ▾ → menú con las rutas de `back_entries()`/`forward_entries()` (nombre de carpeta +
  ruta atenuada); clic en una → `go_to_history`.
- El ▾ se atenúa/oculta cuando no hay historial en esa dirección (mismo `can_go_back/forward`).

### i18n

Claves nuevas (triple es/en, sin voseo, sin reutilizar): tooltip "Historial hacia atrás" /
"Historial hacia adelante".

---

## Testing

- `core::palette`: `fuzzy_match` (prefijo/inicio-de-palabra/disperso/no-match/case), ranking en
  `filter_and_rank`, query vacía. `hit_positions` correctas para resaltado.
- `nav_history`: `back_entries`/`forward_entries` (pila vacía, cursor inicio/medio/final).
- `Action::CommandPalette` en el keymap: binding default Ctrl+P; clave i18n `action.*` en es/en
  (el test `cada_accion_tiene_nombre_en_ambos_idiomas` lo exige).
- UI (overlay, menú ▾): verificación visual en la VM.

## i18n (todas, triple, español neutral sin voseo, sin reutilizar claves)

Paleta: título/placeholder del campo, etiquetas de categoría (Acción/Archivo/Reciente/Favorito/
Tema), ayuda de teclas, "Abrir configuración", "Tema: {nombre}". Historial: tooltips de los ▾.
`action.command_palette` (nombre de la acción para el editor de atajos).

## Orden de implementación sugerido

1. core: `palette` (Command/fuzzy/filter, con tests) + `nav_history::back/forward_entries` (tests)
   + `Action::CommandPalette` con binding default Ctrl+P en el keymap + su clave i18n.
2. Controlador: build_palette_commands + execute_palette_command + go_to_history.
3. UI paleta: command-palette.slint + VM + teclado + disparo + ejecución.
4. UI historial: ▾ en el toolbar + menús.
5. i18n + gate + dist.

## Fuera de alcance (YAGNI)

- Buscar archivos por nombre RECORRIENDO EL DISCO (carpeta + subcarpetas) dentro de la paleta:
  eso ya lo hace Ctrl+F (búsqueda recursiva con worker cancelable) y rompería que la paleta sea
  instantánea. La paleta SÍ incluye los archivos de la carpeta actual YA cargados en el panel
  activo (categoría File, en memoria, sin tocar disco) — esa es la frontera.
- Historial global entre paneles (cada panel tiene su NavHistory; el menú muestra el del activo).
- Acciones de bajo nivel en la paleta (curaduría explícita las excluye).
