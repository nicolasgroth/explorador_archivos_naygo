# Paleta de comandos (Ctrl+P) + menú de historial — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agregar a Naygo una paleta de comandos estilo VS Code (Ctrl+P) que filtra acciones, archivos de la carpeta actual, recientes, favoritos y temas con fuzzy-match, y un menú ▾ de historial en los botones Atrás/Adelante del toolbar.

**Architecture:** Lógica pura nueva en `naygo-core` (módulo `palette` con fuzzy-match, `nav_history::back/forward_entries`), `Action::CommandPalette` en el keymap, y cableado en `naygo-ui-slint` (overlay de la paleta, ejecución por payload, ▾ en el toolbar). El hilo de UI no hace I/O nuevo: el fuzzy corre sobre ~30-50 comandos en memoria.

**Tech Stack:** Rust workspace (naygo-core / naygo-platform / naygo-ui-slint), Slint 1.16 render software, serde. Build con `CARGO_BUILD_JOBS=2`.

**Gate (correr SIEMPRE uno mismo tras cada subagente):**
```
$env:CARGO_BUILD_JOBS = "2"
cargo fmt
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**i18n:** triple (es + en en `crates/core/src/i18n/{es,en}.json` + props en `crates/ui-slint/ui/i18n.slint` + setters en `crates/ui-slint/src/i18n_keys.rs`), español neutral SIN voseo. NO reutilizar nombres de claves. El test `cada_accion_tiene_nombre_en_ambos_idiomas` exige que CADA `Action` tenga `action.<x>` en es Y en.

---

## Estructura de archivos

**Crear:**
- `crates/core/src/palette.rs` — módulo de la paleta (Command/CommandCategory/CommandPayload/CommandMatch, fuzzy_match, filter_and_rank). Puro.
- `crates/ui-slint/ui/command-palette.slint` — overlay de la paleta.

**Modificar (core):**
- `crates/core/src/lib.rs` — `pub mod palette;`.
- `crates/core/src/keymap.rs` — `Action::CommandPalette` + binding default Ctrl+P + `Action::all()` + `i18n_key()`.
- `crates/core/src/workspace/nav_history.rs` — `back_entries()` / `forward_entries()`.
- `crates/core/src/i18n/{es,en}.json` — claves nuevas.

**Modificar (ui-slint):**
- `crates/ui-slint/src/workspace_ctrl.rs` — `build_palette_commands`, `execute_palette_command`, `go_to_history`, ruteo de `Action::CommandPalette`.
- `crates/ui-slint/src/main.rs` — VM de la paleta, callbacks (abrir/teclear/ejecutar/cerrar), ▾ de historial.
- `crates/ui-slint/ui/types.slint` — structs VM de la paleta + del menú de historial.
- `crates/ui-slint/ui/app-window.slint` — instanciar el overlay + ▾ en el toolbar.
- `crates/ui-slint/ui/i18n.slint` + `crates/ui-slint/src/i18n_keys.rs` — props/setters de las claves nuevas.

---

# FASE 1 — core (lógica pura)

### Task 1: Tipos de la paleta

**Files:**
- Create: `crates/core/src/palette.rs`
- Modify: `crates/core/src/lib.rs`

- [ ] **Step 1: Crear el módulo con los tipos**

`crates/core/src/palette.rs`:

```rust
// Naygo — paleta de comandos: modelo de comandos y fuzzy-match (puro).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Lógica PURA de la paleta de comandos (sin UI ni Windows). La UI arma la lista de
//! `Command` desde sus fuentes (acciones, archivos, recientes, favoritos, temas) y usa
//! `filter_and_rank` para filtrar/ordenar según lo que el usuario escribe. 100% testeable.

use crate::keymap::Action;
use crate::theme::ThemeId;
use std::path::PathBuf;

/// Categoría de un comando (define el ícono y la etiqueta en la UI).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCategory {
    Action,
    File,
    Recent,
    Favorite,
    Theme,
    Config,
}

/// Qué ejecuta un comando al elegirlo.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandPayload {
    /// Una acción del keymap; se rutea por el dispatcher de teclado existente.
    Action(Action),
    /// Navegar el panel activo a esta ruta (reciente/favorito).
    Navigate(PathBuf),
    /// Enfocar/seleccionar un entry YA cargado en el panel activo, por su índice de VISTA.
    FocusEntry(usize),
    /// Aplicar este tema.
    Theme(ThemeId),
    /// Abrir la ventana de configuración.
    OpenConfig,
}

/// Un comando de la paleta.
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// Texto a mostrar (ya traducido por la UI).
    pub label: String,
    pub category: CommandCategory,
    /// Atajo legible ("Ctrl+C"); solo acciones con atajo. Vacío si no tiene.
    pub shortcut: String,
    pub payload: CommandPayload,
}

/// Resultado de filtrar: índice del comando + score + posiciones (char-index) que matchearon.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandMatch {
    pub index: usize,
    pub score: i32,
    pub hit_positions: Vec<usize>,
}
```

- [ ] **Step 2: Registrar el módulo**

En `crates/core/src/lib.rs`, junto a los otros `pub mod` (orden alfabético; va entre `ops` y `preview` o donde corresponda):

```rust
pub mod palette;
```

- [ ] **Step 3: Compilar**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-core`
Expected: compila (puede haber warnings de dead_code en los tipos; se usan en Task 2).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/palette.rs crates/core/src/lib.rs
git commit -m "feat(core): tipos de la paleta de comandos"
```
Termina el mensaje con esta línea literal:
Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>

---

### Task 2: fuzzy_match + filter_and_rank

**Files:**
- Modify: `crates/core/src/palette.rs`

- [ ] **Step 1: Escribir los tests (al final del archivo, fallan)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(label: &str) -> Command {
        Command {
            label: label.to_string(),
            category: CommandCategory::Action,
            shortcut: String::new(),
            payload: CommandPayload::OpenConfig,
        }
    }

    #[test]
    fn match_de_prefijo() {
        let (_score, hits) = fuzzy_match("cop", "Copiar").expect("debe matchear");
        assert_eq!(hits, vec![0, 1, 2]);
    }

    #[test]
    fn match_disperso_subsecuencia() {
        // 'cps' matchea C-o-P-iar-S... no; usar un caso real: "cpo" en "Copiar al otro"
        let m = fuzzy_match("cpo", "Copiar al otro panel");
        assert!(m.is_some(), "subsecuencia debe matchear");
    }

    #[test]
    fn no_match_si_falta_una_letra() {
        assert!(fuzzy_match("xyz", "Copiar").is_none());
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_match("COP", "copiar").is_some());
        assert!(fuzzy_match("cop", "COPIAR").is_some());
    }

    #[test]
    fn prefijo_puntua_mas_que_disperso() {
        let (prefix_score, _) = fuzzy_match("co", "Copiar").unwrap();
        let (sparse_score, _) = fuzzy_match("co", "Calcular tamañO").unwrap();
        assert!(prefix_score > sparse_score, "prefijo {prefix_score} > disperso {sparse_score}");
    }

    #[test]
    fn filter_rank_ordena_por_score_y_query_vacia_da_todos() {
        let cmds = vec![cmd("Calcular tamaño"), cmd("Copiar"), cmd("Cortar")];
        // Query vacía: todos, en el orden original.
        let all = filter_and_rank(&cmds, "");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].index, 0);
        // Query "co": Copiar y Cortar matchean (prefijo), Calcular no por prefijo pero sí disperso.
        let res = filter_and_rank(&cmds, "co");
        assert!(res.iter().any(|m| m.index == 1), "Copiar debe estar");
        // El de mejor score va primero.
        assert!(res[0].score >= res[res.len() - 1].score);
    }
}
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core palette::`
Expected: FAIL — `fuzzy_match`/`filter_and_rank` no existen.

- [ ] **Step 3: Implementar fuzzy_match y filter_and_rank**

Antes del bloque `#[cfg(test)]`:

```rust
/// Match fuzzy de subsecuencia con ranking. Devuelve `(score, hit_positions)` o `None` si la
/// query no es subsecuencia de `text`. Case-insensitive. Score más alto = mejor coincidencia:
/// se premia el match contiguo, el inicio de palabra y el prefijo; se penaliza la dispersión.
pub fn fuzzy_match(query: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let q: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();
    let t: Vec<char> = text.chars().collect();
    let t_lower: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut hits = Vec::with_capacity(q.len());
    let mut qi = 0usize;
    let mut score = 0i32;
    let mut prev_match: Option<usize> = None;

    for (ti, &tc) in t_lower.iter().enumerate() {
        if qi >= q.len() {
            break;
        }
        if tc == q[qi] {
            hits.push(ti);
            // Bonus: match al inicio del texto (prefijo).
            if ti == 0 {
                score += 15;
            }
            // Bonus: inicio de palabra (tras separador) — usa el char ORIGINAL.
            else if matches!(t.get(ti - 1), Some(' ') | Some('_') | Some('-') | Some('\\') | Some('/') | Some('.')) {
                score += 10;
            }
            // Bonus: contiguo al match anterior.
            if let Some(p) = prev_match {
                if ti == p + 1 {
                    score += 8;
                } else {
                    // Penaliza el hueco entre matches (dispersión).
                    score -= (ti - p - 1).min(10) as i32;
                }
            }
            score += 1; // base por cada letra que matchea
            prev_match = Some(ti);
            qi += 1;
        }
    }

    if qi == q.len() {
        Some((score, hits))
    } else {
        None
    }
}

/// Filtra los comandos por la query y los ordena por score (desc). Query vacía → todos, en el
/// orden original (la UI puede recortar a una "lista por defecto").
pub fn filter_and_rank(commands: &[Command], query: &str) -> Vec<CommandMatch> {
    let mut matches: Vec<CommandMatch> = commands
        .iter()
        .enumerate()
        .filter_map(|(index, c)| {
            fuzzy_match(query, &c.label).map(|(score, hit_positions)| CommandMatch {
                index,
                score,
                hit_positions,
            })
        })
        .collect();
    // Orden estable: score desc; a igualdad, conserva el orden original (por índice asc).
    matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
    matches
}
```

- [ ] **Step 4: Correr y ver que pasan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core palette::`
Expected: PASS (6 tests). Si algún assert de score falla por la calibración de bonos, AJUSTA los
valores de los tests a lo que produce la función (lo importante es el ORDEN relativo: prefijo >
disperso), no números mágicos exactos.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/palette.rs
git commit -m "feat(core): fuzzy_match + filter_and_rank de la paleta"
```
Termina el mensaje con la línea Co-Authored-By literal (igual que Task 1).

---

### Task 3: nav_history back_entries / forward_entries

**Files:**
- Modify: `crates/core/src/workspace/nav_history.rs`

> VERIFICADO: `NavHistory { stack: Vec<PathBuf>, cursor: Option<usize> }`. `jump_to(index) ->
> Option<&Path>` ya existe. `can_back/can_forward` ya existen.

- [ ] **Step 1: Escribir los tests (fallan)**

En el `#[cfg(test)]` de nav_history.rs:

```rust
#[test]
fn back_y_forward_entries_parten_la_pila_por_el_cursor() {
    let mut h = NavHistory::new();
    h.push(std::path::PathBuf::from("A"));
    h.push(std::path::PathBuf::from("B"));
    h.push(std::path::PathBuf::from("C")); // cursor en C (índice 2)
    // back: las anteriores al cursor, de la más cercana a la más lejana → B, A.
    assert_eq!(h.back_entries(), vec![std::path::PathBuf::from("B"), std::path::PathBuf::from("A")]);
    // forward: vacío (estamos al final).
    assert!(h.forward_entries().is_empty());
    // Retrocedemos a A.
    h.back(); h.back(); // cursor en A (índice 0)
    assert!(h.back_entries().is_empty());
    // forward: las posteriores, de la más cercana a la más lejana → B, C.
    assert_eq!(h.forward_entries(), vec![std::path::PathBuf::from("B"), std::path::PathBuf::from("C")]);
}
```

> Ajusta `h.back()` al nombre real del método que mueve el cursor atrás (existe `back`/`forward`;
> si devuelven `Option<&Path>` no pasa nada, el test no usa el retorno).

- [ ] **Step 2: Correr y ver que fallan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core back_y_forward_entries`
Expected: FAIL — métodos no existen.

- [ ] **Step 3: Implementar back_entries / forward_entries**

En `impl NavHistory` (junto a `can_back`/`can_forward`):

```rust
    /// Rutas hacia ATRÁS desde el cursor (de la más cercana a la más lejana). Vacío si no hay.
    pub fn back_entries(&self) -> Vec<PathBuf> {
        match self.cursor {
            Some(i) if i > 0 => self.stack[..i].iter().rev().cloned().collect(),
            _ => Vec::new(),
        }
    }

    /// Rutas hacia ADELANTE desde el cursor (de la más cercana a la más lejana). Vacío si no hay.
    pub fn forward_entries(&self) -> Vec<PathBuf> {
        match self.cursor {
            Some(i) if i + 1 < self.stack.len() => self.stack[i + 1..].iter().cloned().collect(),
            _ => Vec::new(),
        }
    }
```

- [ ] **Step 4: Correr y ver que pasan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core back_y_forward_entries`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/workspace/nav_history.rs
git commit -m "feat(core): nav_history back_entries/forward_entries para el menú de historial"
```
(con la línea Co-Authored-By literal).

---

### Task 4: Action::CommandPalette + Ctrl+P + i18n

**Files:**
- Modify: `crates/core/src/keymap.rs`, `crates/core/src/i18n/{es,en}.json`

- [ ] **Step 1: Test del binding (falla)**

En el `#[cfg(test)]` de keymap.rs:

```rust
#[test]
fn ctrl_p_dispara_la_paleta() {
    let km = KeyMap::defaults();
    let chord = Chord::ctrl(KeyCode::Char('p'));
    assert_eq!(km.action_for(&chord), Some(Action::CommandPalette));
}
```

> VERIFICADO: el constructor de defaults es `KeyMap::defaults()` (NO `default()`). `Chord::ctrl`
> y `KeyCode::Char` existen. Si hay un test de conteo `all_tiene_N_acciones`, habrá que subir N
> en 1 (igual que pasó con GoHome) — ajústalo.

- [ ] **Step 2: Correr y ver que falla**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core ctrl_p`
Expected: FAIL — `Action::CommandPalette` no existe.

- [ ] **Step 3: Agregar la variante, all(), i18n_key(), el binding**

1) En `enum Action`, al final (tras `GoFavorite9`):
```rust
    /// Abrir la paleta de comandos (Ctrl+P).
    CommandPalette,
```
2) En `Action::all()`, agregar `CommandPalette` al final del array.
3) En `Action::i18n_key()` (match exhaustivo), agregar:
```rust
            Action::CommandPalette => "action.command_palette",
```
4) En `KeyMap::defaults()`, junto a los demás bindings:
```rust
        map.bind(Chord::ctrl(KeyCode::Char('p')), Action::CommandPalette);
```
> Ajusta `map.bind(...)` al mecanismo REAL que usen los defaults (mira cómo se bindea
> `Action::Find` a Ctrl+F para copiar el patrón EXACTO). Si hay un test de conteo de acciones,
> súbelo en 1.

- [ ] **Step 4: i18n — clave en ambos idiomas**

En `crates/core/src/i18n/es.json`, junto a las otras `action.*`:
```json
"action.command_palette": "Paleta de comandos",
```
En `crates/core/src/i18n/en.json`:
```json
"action.command_palette": "Command palette",
```

- [ ] **Step 5: Correr y ver que pasan**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test -p naygo-core` (incluye `ctrl_p` y el test de i18n
`cada_accion_tiene_nombre_en_ambos_idiomas`).
Expected: PASS. Si `cada_accion...` falla por falta de la clave, revisa que esté en AMBOS json.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/keymap.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(core): Action::CommandPalette con binding Ctrl+P + i18n"
```
(con la línea Co-Authored-By literal).

---

# FASE 2 — Controlador (construir + ejecutar comandos, historial)

### Task 5: build_palette_commands + execute_palette_command + go_to_history

**Files:**
- Modify: `crates/ui-slint/src/workspace_ctrl.rs`

> VERIFICADO: `chord_text_for(action) -> String` está en config_ctrl (para el atajo). El acceso a
> settings es `self.config.settings`. `navigate_active_to` navega el panel activo a un PathBuf.
> Las acciones se rutean en `on_key` por un `match action`. Recientes/favoritos/temas: busca cómo
> los listan otros menús (recientes en el historial; favoritos persistidos; `ThemeCatalog`).

- [ ] **Step 1: build_palette_commands**

Agregar al controlador un método que arme `Vec<naygo_core::palette::Command>`:

```rust
    /// Construye la lista de comandos de la paleta desde las fuentes vivas: acciones curadas,
    /// archivos del panel activo, recientes, favoritos, temas y "abrir configuración".
    pub fn build_palette_commands(&self) -> Vec<naygo_core::palette::Command> {
        use naygo_core::palette::{Command, CommandCategory, CommandPayload};
        use naygo_core::keymap::Action;
        let mut out: Vec<Command> = Vec::new();

        // 1) Acciones CURADAS (las que tienen sentido en una paleta).
        const CURATED: &[Action] = &[
            Action::Copy, Action::Cut, Action::Paste, Action::Rename, Action::BatchRename,
            Action::NewFile, Action::NewDir, Action::ComputeSize, Action::Refresh, Action::Find,
            Action::Undo, Action::GoUp, Action::GoBack, Action::GoForward, Action::GoHome,
            Action::SwitchPane, Action::CopyToOther, Action::MoveToOther, Action::SelectAll,
            Action::Help, Action::EditPath,
        ];
        for &a in CURATED {
            out.push(Command {
                label: self.config.t(a.i18n_key()), // traduce la clave de la acción
                category: CommandCategory::Action,
                shortcut: self.config.chord_text_for(a), // "Ctrl+C" o "" si no tiene
                payload: CommandPayload::Action(a),
            });
        }

        // 2) Archivos del panel activo (entries de la VISTA actual).
        if let Some(f) = self.ws.active_files() {
            for (view_idx, real_idx) in f.view_indices().iter().enumerate() {
                if let Some(e) = f.entries.get(*real_idx) {
                    out.push(Command {
                        label: e.name.clone(), // nombre del archivo/carpeta
                        category: CommandCategory::File,
                        shortcut: String::new(),
                        payload: CommandPayload::FocusEntry(view_idx),
                    });
                }
            }
        }

        // 3) Recientes, 4) Favoritos, 5) Temas, 6) Config.
        // Reusa las fuentes existentes (ajusta a los métodos reales):
        //   - recientes: self.recent_dirs() o el historial de recientes → Navigate(path)
        //   - favoritos: self.favorites() → Navigate(path)
        //   - temas: self.config.theme_catalog().ids()+nombres → Theme(id), label "Tema: <n>"
        //   - config: una entrada fija label t("palette.open_config") → OpenConfig
        // ... (poblar out con esas 4 fuentes)

        out
    }
```

> AJUSTA `f.entries`, `e.name`, `view_indices`, `self.config.t`, `chord_text_for`, y las fuentes
> de recientes/favoritos/temas a los nombres REALES. Si `t()` no está en config, usa el i18n que
> uses para traducir claves. Lo esencial: una entrada por acción curada, una por archivo visible,
> una por reciente/favorito/tema, y una para config.

- [ ] **Step 2: execute_palette_command**

```rust
    /// Ejecuta el comando de la paleta en `index` de la lista que devolvió build_palette_commands.
    /// Devuelve true si algo cambió (para refrescar la UI). Cierra la paleta en el llamador.
    pub fn execute_palette_command(&mut self, commands: &[naygo_core::palette::Command], index: usize) -> bool {
        use naygo_core::palette::CommandPayload;
        let Some(cmd) = commands.get(index) else { return false; };
        match &cmd.payload {
            CommandPayload::Action(a) => self.run_action(*a), // rutea por el dispatcher existente
            CommandPayload::Navigate(p) => self.navigate_active_to(p.clone()),
            CommandPayload::FocusEntry(view_idx) => self.focus_entry_in_active(*view_idx),
            CommandPayload::Theme(id) => { self.apply_theme(*id); true }
            CommandPayload::OpenConfig => { self.request_open_config(); true }
        }
    }
```

> AJUSTA `run_action`/`navigate_active_to`/`focus_entry_in_active`/`apply_theme`/
> `request_open_config` a los métodos REALES. Para Action: el dispatcher de `on_key` ya hace
> `match action => self.on_xxx()`; extrae ese match a un `run_action(&mut self, a: Action) -> bool`
> reutilizable (o llama directo a los `on_*`). Para FocusEntry: setear el foco/selección del
> panel activo a ese índice de vista (reusar la lógica de selección que ya existe, p.ej.
> select_single) y pedir scroll. Para Theme/Config: reusar lo que hacen el selector de temas y el
> botón de abrir config.

- [ ] **Step 3: go_to_history**

```rust
    /// Salta el panel activo a una entrada del historial (por índice en la pila completa).
    pub fn go_to_history(&mut self, stack_index: usize) -> bool {
        // Mueve el cursor del NavHistory del panel activo y navega a esa ruta.
        // jump_to(index) ya existe en NavHistory y devuelve la ruta destino.
        let Some(f) = self.ws.active_files_mut() else { return false; };
        let Some(dir) = f.history.jump_to(stack_index).map(|p| p.to_path_buf()) else { return false; };
        self.navigate_to_without_pushing_history(f_id, dir) // ver nota
    }
```

> OJO: `jump_to` ya mueve el cursor y NO debe re-apilar (sería navegación normal). Busca cómo
> `on_go_back` navega tras `f.go_back()` (que también mueve el cursor sin apilar) y replica ESE
> camino: mover cursor con jump_to + listar la carpeta SIN push al historial. Ajusta los nombres
> (`active_files_mut`, `history`, el método de listar-sin-apilar) a lo real. Si el menú de la UI
> te pasa la RUTA en vez del índice, agrega una variante que busque el índice por ruta o navegue
> directo moviendo el cursor.

- [ ] **Step 4: Rutear Action::CommandPalette**

En el `match action` de `on_key`, agregar un brazo que ABRE la paleta (no la ejecuta):

```rust
            Action::CommandPalette => { self.open_palette = true; return true; }
```

> Agrega un campo `open_palette: bool` (o el mecanismo que usa el proyecto para pedir abrir un
> overlay; mira cómo se abre la ayuda F1 / la ventana de config y replícalo). La UI lee ese flag.

- [ ] **Step 5: Gate**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings
```
Expected: PASS, clippy limpio. (La UI aún no usa estos métodos; si clippy se queja de dead_code,
pon `#[allow(dead_code)]` con comentario "lo consume la UI de la paleta (Task 7)" — se quita en Task 7.)

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/src/workspace_ctrl.rs
git commit -m "feat(ui): build/execute de comandos de la paleta + go_to_history"
```
(con la línea Co-Authored-By literal).

---

# FASE 3 — UI de la paleta

### Task 6: VM + estructuras Slint de la paleta

**Files:**
- Modify: `crates/ui-slint/ui/types.slint`

- [ ] **Step 1: Structs VM**

En `types.slint`, agregar:

```slint
// Un segmento del label de un resultado de la paleta: texto + si está resaltado (match).
struct PaletteSpanVm {
    text: string,
    hit: bool,
}

// Un resultado de la paleta.
struct PaletteItemVm {
    spans: [PaletteSpanVm],   // el label partido en segmentos resaltados/normales
    category: int,            // 0=Acción 1=Archivo 2=Reciente 3=Favorito 4=Tema 5=Config
    shortcut: string,         // "Ctrl+C" o ""
}

// Una entrada del menú de historial (▾) de Atrás/Adelante.
struct HistoryItemVm {
    name: string,             // nombre de la carpeta
    path: string,             // ruta completa (atenuada)
}
```

- [ ] **Step 2: Campos en el AppWindow VM**

Donde el AppWindow declara su estado, agregar:
```slint
    in-out property <bool> palette-open: false;
    in-out property <string> palette-query: "";
    in property <[PaletteItemVm]> palette-results;
    in-out property <int> palette-selected: 0;
```

- [ ] **Step 3: Build**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint`
Expected: compila (los structs nuevos sin usar son ok).

- [ ] **Step 4: Commit**

```bash
git add crates/ui-slint/ui/types.slint
git commit -m "feat(ui): VM de la paleta y del menú de historial"
```
(con la línea Co-Authored-By literal).

---

### Task 7: Overlay command-palette.slint + wiring

**Files:**
- Create: `crates/ui-slint/ui/command-palette.slint`
- Modify: `crates/ui-slint/ui/app-window.slint`, `crates/ui-slint/src/main.rs`

- [ ] **Step 1: Componente del overlay**

`crates/ui-slint/ui/command-palette.slint`:

```slint
// Naygo — overlay de la paleta de comandos (Ctrl+P).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
import { LineEdit, ScrollView } from "std-widgets.slint";
import { PaletteItemVm, PaletteSpanVm } from "types.slint";
import { Tr } from "i18n.slint";
import { Theme } from "theme.slint";

export component CommandPalette inherits Rectangle {
    in property <[PaletteItemVm]> results;
    in-out property <string> query;
    in-out property <int> selected;
    callback query-changed(string);
    callback run(int);          // ejecutar el resultado en el índice dado
    callback dismiss();

    // Velo a pantalla completa.
    background: #00000070;
    TouchArea { clicked => { root.dismiss(); } }   // clic fuera cierra

    Rectangle {
        x: (parent.width - self.width) / 2;
        y: 24px;
        width: min(560px, parent.width - 48px);
        height: min(420px, parent.height - 48px);
        background: Theme.panel-bg;
        border-width: 1px;
        border-color: Theme.border;
        border-radius: 8px;

        VerticalLayout {
            padding: 0px;
            // Campo de búsqueda.
            search := LineEdit {
                placeholder-text: Tr.palette-placeholder;
                text <=> root.query;
                edited => { root.query-changed(self.text); }
            }
            // Lista de resultados.
            ScrollView {
                vertical-stretch: 1;
                VerticalLayout {
                    for item[i] in root.results: Rectangle {
                        height: 30px;
                        background: i == root.selected ? Theme.selection-bg : transparent;
                        HorizontalLayout {
                            padding-left: 10px; padding-right: 10px; spacing: 8px;
                            // (ícono por categoría: un Text simple o Path; mínimo viable un Text)
                            // label con segmentos resaltados:
                            HorizontalLayout {
                                horizontal-stretch: 1;
                                for sp in item.spans: Text {
                                    text: sp.text;
                                    color: sp.hit ? Theme.accent : Theme.text;
                                    vertical-alignment: center;
                                }
                            }
                            Text {
                                text: item.shortcut;
                                color: Theme.text-dim;
                                font-family: "Consolas";
                                vertical-alignment: center;
                            }
                        }
                        TouchArea { clicked => { root.run(i); } }
                    }
                }
            }
            // Pie con ayuda de teclas.
            Text {
                text: Tr.palette-help;
                color: Theme.text-dim;
                font-size: 11px;
                horizontal-alignment: center;
            }
        }

        // Teclado: ↑↓ mover, Enter ejecutar, Esc cerrar.
        fs := FocusScope {
            key-pressed(e) => {
                if e.text == Key.UpArrow { root.selected = max(0, root.selected - 1); return accept; }
                if e.text == Key.DownArrow { root.selected = min(root.results.length - 1, root.selected + 1); return accept; }
                if e.text == Key.Return { root.run(root.selected); return accept; }
                if e.text == Key.Escape { root.dismiss(); return accept; }
                return reject;
            }
        }
    }
}
```

> AJUSTA a Slint 1.16 real: `LineEdit` para el campo (foco al abrir → `search.focus()` cuando se
> abre la paleta; quizá vía un `init` o una propiedad). El `FocusScope` para las flechas/Enter/Esc
> puede tener que envolver el conjunto. Verifica nombres de teclas (`Key.UpArrow` etc.). Si el
> campo de texto se come las flechas, maneja ↑↓ en el `key-pressed` del FocusScope que envuelve
> todo y deja que el LineEdit reciba las letras. Itera hasta que compile y el teclado funcione.

- [ ] **Step 2: Instanciar en app-window.slint**

Importar y poner el overlay ENCIMA de todo, condicional a `palette-open`:

```slint
import { CommandPalette } from "command-palette.slint";
// ... al final del contenido del AppWindow, como último hijo (z-order arriba):
    if root.palette-open: CommandPalette {
        width: 100%; height: 100%;
        results: root.palette-results;
        query <=> root.palette-query;
        selected <=> root.palette-selected;
        query-changed(q) => { root.palette-query-changed(q); }
        run(i) => { root.palette-run(i); }
        dismiss => { root.palette-dismiss(); }
    }
```
Declarar los callbacks `palette-query-changed(string)`, `palette-run(int)`, `palette-dismiss()` en
el AppWindow.

- [ ] **Step 3: Cablear en main.rs**

- Al abrir (el flag `open_palette` del controlador o cuando se dispara Ctrl+P): setear
  `ui.set_palette_open(true)`, construir comandos (`build_palette_commands`), guardarlos en un
  `RefCell`/campo accesible, computar `filter_and_rank("")` → `palette-results`, selected=0.
- `on_palette_query_changed(q)`: recomputar `filter_and_rank(&commands, &q)` → results (mapear cada
  CommandMatch a PaletteItemVm: partir el label por `hit_positions` en spans, category int,
  shortcut), selected=0.
- `on_palette_run(i)`: traducir el índice de RESULTADO al índice del COMANDO (los results llevan
  `index`), llamar `execute_palette_command(&commands, cmd_index)`, cerrar (`set_palette_open(false)`),
  refrescar la UI.
- `on_palette_dismiss()`: `set_palette_open(false)`.
- **Importante**: mientras `palette-open`, el `on_key` del panel NO debe actuar. Mira cómo
  `pending_dialog` bloquea `on_key` (workspace_ctrl) y haz lo mismo: si la paleta está abierta,
  `on_key` retorna temprano (salvo que el propio overlay maneje su teclado).

> El mapeo CommandMatch→PaletteItemVm (partir el label en spans resaltados) es la pieza nueva:
> dado `label` y `hit_positions` (char indices), genera una secuencia de PaletteSpanVm alternando
> normal/resaltado. Hazlo en un helper en main.rs.

- [ ] **Step 4: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo build -p naygo-ui-slint; cargo clippy --workspace --all-targets -- -D warnings`
Expected: compila, clippy limpio. Quita los `#[allow(dead_code)]` de Task 5 si quedaron.

- [ ] **Step 5: i18n**

Claves nuevas (es/en + i18n.slint + i18n_keys.rs): `palette.placeholder` ("Escribe un comando o
carpeta…" / "Type a command or folder…"), `palette.help` ("↑↓ moverse · Enter ejecutar · Esc
cerrar" / "↑↓ move · Enter run · Esc close"), `palette.open_config` ("Abrir configuración" / "Open
settings"), y el prefijo `palette.theme_prefix` ("Tema: " / "Theme: ") si lo usas. NO reutilizar.

- [ ] **Step 6: Commit**

```bash
git add crates/ui-slint/ui/command-palette.slint crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): overlay de la paleta de comandos (Ctrl+P) cableado"
```
(con la línea Co-Authored-By literal).

---

# FASE 4 — Menú ▾ de historial en el toolbar

### Task 8: ▾ de historial en Atrás/Adelante

**Files:**
- Modify: `crates/ui-slint/ui/app-window.slint`, `crates/ui-slint/src/main.rs`, `crates/ui-slint/ui/types.slint` (ya tiene HistoryItemVm de Task 6), i18n.

- [ ] **Step 1: Callbacks + props en AppWindow**

Junto a los callbacks de navegación (go-back/go-forward de la entrega anterior), agregar:
```slint
    callback open-back-history(length);     // x para anclar el menú
    callback open-forward-history(length);
    callback go-history(int);               // saltar a la entrada N del menú abierto
    in property <[HistoryItemVm]> history-items;   // se llena al abrir el menú
```

- [ ] **Step 2: Triángulo ▾ junto a cada botón**

Junto al ToolBtn de Atrás y al de Adelante, agregar un ▾ pequeño (mismo patrón que el ▾ de
Expulsar de la tira de discos USB — búscalo en app-window.slint y replica su estructura:
Rectangle estrecho + TouchArea + un Path/Text con el triángulo). El ▾ de Atrás:
- visible/enabled solo si `root.can-go-back`;
- al clic → `root.open-back-history(self.absolute-position.x)`.
El ▾ de Adelante igual con `can-go-forward` y `open-forward-history`.

- [ ] **Step 3: El menú desplegable**

Reusa el mecanismo de menú que ya use el ▾ de USB (un popup/overlay anclado). Lista
`root.history-items` (nombre + ruta atenuada); clic en el ítem i → `root.go-history(i)`.

> Si el ▾ de USB usa un `PopupWindow` de Slint o un overlay propio, usa el MISMO. No inventes un
> mecanismo nuevo de menú.

- [ ] **Step 4: Cablear en main.rs**

- `on_open_back_history(x)`: `ctrl.back_entries()` del panel activo → llenar `history-items`
  (nombre = file_name de cada ruta, path = display), abrir el menú anclado en x.
- `on_open_forward_history(x)`: ídem con `forward_entries()`.
- `on_go_history(i)`: traducir el índice del MENÚ al índice/ruta en la pila y llamar
  `ctrl.go_to_history(...)`; refrescar. (El menú de Atrás lista back_entries en orden cercano→
  lejano; mapea ese i de vuelta al índice real de la pila — cuidado con el orden.)

> El mapeo índice-de-menú → índice-de-pila es la parte delicada: back_entries está en orden
> cercano→lejano (cursor-1, cursor-2, …); el ítem i del menú de Atrás corresponde a la posición
> `cursor - 1 - i` en la pila. forward: `cursor + 1 + i`. Implementa el cálculo en el controlador
> (un método `go_back_history(menu_index)` / `go_forward_history(menu_index)`) para no exponer
> aritmética de índices a la UI. Ajusta go_to_history a recibir el índice de menú + dirección, o
> agrega esos dos métodos.

- [ ] **Step 5: i18n**

Tooltips de los ▾: `toolbar.back_history` ("Historial hacia atrás" / "Back history"),
`toolbar.forward_history` ("Historial hacia adelante" / "Forward history"). NO reutilizar.

- [ ] **Step 6: Gate**

Run: `$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, clippy limpio.

- [ ] **Step 7: Commit**

```bash
git add crates/ui-slint/ui/app-window.slint crates/ui-slint/src/main.rs crates/ui-slint/ui/i18n.slint crates/ui-slint/src/i18n_keys.rs crates/core/src/i18n/es.json crates/core/src/i18n/en.json
git commit -m "feat(ui): menú de historial en los botones Atrás/Adelante del toolbar"
```
(con la línea Co-Authored-By literal).

---

# FASE 5 — Cierre

### Task 9: CHANGELOG + docs + gate final + dist

**Files:**
- Modify: `CHANGELOG.md`, `docs/GUIA-DE-USUARIO.md`

- [ ] **Step 1: CHANGELOG**

En `CHANGELOG.md`, bajo "### Añadido" de la sección en curso:
```
- Paleta de comandos (Ctrl+P): un buscador rápido que filtra acciones, archivos de la carpeta
  actual, carpetas recientes, favoritos y temas con coincidencia aproximada (fuzzy). Se navega
  con las flechas, Enter ejecuta y Esc cierra. El atajo es configurable en Atajos.
- Menú de historial en los botones Atrás y Adelante: un triángulo ▾ junto a cada uno despliega
  las carpetas visitadas en esa dirección para saltar directo a una.
```

- [ ] **Step 2: Guía de usuario**

Agregar a `docs/GUIA-DE-USUARIO.md` (en la sección de navegación / atajos) la paleta (Ctrl+P, qué
busca, cómo se opera) y el menú ▾ de historial. Español neutral sin voseo, respetando el formato.

- [ ] **Step 3: Gate final completo**

Run:
```
$env:CARGO_BUILD_JOBS = "2"; cargo fmt; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings
```
Expected: PASS (todos los tests), clippy 100% limpio.

- [ ] **Step 4: Actualizar el grafo**

Run: `graphify update .`

- [ ] **Step 5: Commit docs**

```bash
git add CHANGELOG.md docs/GUIA-DE-USUARIO.md
git commit -m "docs: paleta de comandos (Ctrl+P) + menú de historial"
```
(con la línea Co-Authored-By literal).

- [ ] **Step 6: Generar el dist**

Run: `$env:CARGO_BUILD_JOBS = "2"; powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1`
Expected: `dist/Naygo-0.1.0-portable.zip` + `dist/Naygo-0.1.0-setup.exe` regenerados.

(El push lo hace Nicolás. Verificación visual en la VM: Ctrl+P abre la paleta, fuzzy filtra y
resalta, ↑↓/Enter/Esc, ejecuta acciones/navega a archivos/recientes/favoritos/temas; ▾ de
Atrás/Adelante lista el historial y salta.)

---

## Notas para el implementador

- **SIEMPRE correr el gate uno mismo tras cada subagente** (no confiar en su reporte): un subagente
  anterior no corrió clippy y dejó errores.
- **Ajustar firmas a lo REAL** (marcadas con "AJUSTA"): `f.entries`/`e.name`/`view_indices`,
  fuentes de recientes/favoritos/temas, `run_action`/`navigate_active_to`/`focus_entry_in_active`,
  el mecanismo de menú del ▾ de USB, el patrón de bloqueo de `on_key` con `pending_dialog`.
- **graphify antes de grep** (hook obligatorio); inclúyelo en prompts de subagentes.
- **Slint 1.16**: `TextEdit`/`LineEdit` de std-widgets; `FocusScope` para teclado; `HorizontalLayout`
  NO soporta `wrap`; ToolBtn la prop de tooltip es `tip`; íconos por Path, no glifos.
- **i18n triple, sin voseo, sin reutilizar claves**; el test `cada_accion_tiene_nombre_en_ambos_idiomas`
  exige `action.command_palette` en es Y en.
- **format_size(bytes, fmt)** para bytes con formato (no `human_size`).
- Un solo dist al final (Task 9). Nicolás hace el push.
