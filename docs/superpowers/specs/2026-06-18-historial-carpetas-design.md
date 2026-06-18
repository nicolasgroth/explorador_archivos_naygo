# Ícono de historial de carpetas + límite configurable — Diseño

> Mejora pedida por Nicolás 2026-06-18. Se apoya en `RecentDirs` (ya existente). El
> backlog de argumentos de CLI va en un ciclo aparte después de este.

**Objetivo:** un ícono de historial en la toolbar global que despliega un menú con las
carpetas visitadas recientemente; al elegir una, el panel activo navega a ella. La
cantidad de carpetas recordadas pasa a ser configurable (1–100, por defecto 50), en la
sección Avanzado de la configuración.

**Autoría:** Nicolás Groth / ISGroth, 2026, MIT.

**Decisiones tomadas con el usuario:**
- Se construye sobre `RecentDirs` (una sola fuente de verdad: el menú, el panel «Recientes»
  y el autocompletado muestran lo mismo). No se crea un historial paralelo.
- El ícono vive en la **toolbar global**; al elegir una carpeta navega el **panel activo**.
- Límite configurable **1–100**, **por defecto 50**, en la sección **Avanzado**.

---

## Contexto actual (lo que se reutiliza — no rehacer)

- **`core::recent_dirs::RecentDirs`** (`crates/core/src/recent_dirs.rs`): MRU global, más
  reciente primero, sin duplicados; `push(dir)`, `list() -> &[PathBuf]`, `remove_missing()`
  (filtra inexistentes antes de mostrar), `to_json`/`from_json` (persistencia tolerante).
  **Hoy el tope es `const MAX_RECENTS: usize = 30` fijo** → esto se vuelve configurable.
- **`core::config::Settings`** (`crates/core/src/config/mod.rs:119`): struct de settings con
  serde; ya tiene campos como `ToolbarIconStyle`, `LowPowerMode`, `OpsDisplay`, etc. La
  ventana de config los edita; la sección **Avanzado** ya existe (cat de config).
- **Patrón de menú flotante** (veil + rectángulo anclado): ya usado para el ▾ de expulsión de
  USB y el menú «agregar panel» en `app-window.slint`. Se replica para el menú de historial.
- **Navegación del panel activo a una ruta:** `navigate_active_to` (workspace_ctrl) ya existe.
- El panel «Recientes» ya consume `RecentDirs` vía `recent_rows()` (bridge).

---

## Sección 1 — Límite configurable en `Settings` + `RecentDirs`

1. **`Settings` gana un campo** `recent_limit: usize`, con `#[serde(default = "default_recent_limit")]`
   y `fn default_recent_limit() -> usize { 50 }`, para que configs viejas (sin el campo)
   carguen con 50. El valor efectivo se **clampa a `1..=100`** al usarlo (config editada a
   mano podría traer 0 o 9999).
2. **`RecentDirs` deja de usar la constante fija.** Para no acoplar `core::recent_dirs` a
   `Settings`, `push` recibe el límite como parámetro:
   `pub fn push(&mut self, dir: PathBuf, limit: usize)`. El `limit` se clampa a `>= 1` dentro
   de `push` (defensa). Se elimina (o se deja como default interno) `MAX_RECENTS`.
   - Alternativa considerada y descartada: que `RecentDirs` guarde su propio `limit` como
     campo serializado. Se descarta porque el límite es una preferencia del usuario que vive
     en `Settings`, no un dato del historial; duplicarlo invita a desincronización.
3. **El controlador, al registrar una visita**, pasa `self.config.settings.recent_limit`
   (clampeado) a `push`. Si el usuario **baja** el límite en config, al guardar settings el
   controlador **trunca** la lista actual a ese tamaño (un método `RecentDirs::truncate_to(n)`
   o reutilizando `push` no aplica aquí; se agrega `truncate_to`).

**Tests (core, `recent_dirs.rs`):**
- `push` con `limit = 3` mantiene a lo sumo 3 (MRU, el más viejo se descarta).
- `push` con `limit = 0` se comporta como `limit = 1` (clamp defensivo; nunca lista vacía tras
  un push).
- `truncate_to(n)` recorta a n y conserva los n más recientes.
- `Settings` por defecto trae `recent_limit == 50` (test en `config`), y la deserialización de
  un JSON sin el campo da 50.

## Sección 2 — Ícono + menú flotante de historial (toolbar global)

- Un **ícono de historial** (un reloj con una manecilla/flecha, dibujado con `Path`, NUNCA
  glifo de fuente — render por software) en la toolbar global, junto a los grupos existentes.
- Al pulsarlo, abre un **menú flotante** anclado bajo el ícono (mismo patrón veil + rectángulo
  que el ▾ de USB): lista las carpetas de `RecentDirs` (tras `remove_missing()`), más reciente
  arriba, mostrando el nombre de la carpeta y, tenue, su ruta (o ruta recortada). Limitado a lo
  que haya (hasta `recent_limit`).
- **Elegir una** carpeta: cierra el menú y `navigate_active_to(esa_ruta)` (el panel activo va a
  ella). Si la carpeta resultara inexistente al navegar, cae en el aviso «carpeta no
  encontrada» in-place que ya existe.
- **Historial vacío:** el menú muestra una línea discreta «Sin carpetas recientes» (clave i18n
  `history-empty`).
- **i18n triple:** `history` («Historial» / «History») para el tooltip del ícono, y
  `history-empty`.
- El menú se cierra al hacer clic fuera (veil) o al elegir, como los otros menús flotantes.

## Sección 3 — Fila en la config (Avanzado)

- En la sección **Avanzado** de la ventana de config, una fila «Carpetas recientes a recordar»
  con un control numérico (campo + flechas o un editor de texto validado) acotado a **1–100**.
- Cambiarlo actualiza `Settings.recent_limit`, persiste, y trunca la lista actual si el nuevo
  valor es menor.
- i18n triple para la etiqueta (`cfg-recent-limit`) y, si hace falta, una ayuda breve.

---

## Archivos tocados

| Archivo | Acción |
|---|---|
| `crates/core/src/recent_dirs.rs` | `push(dir, limit)`, `truncate_to(n)`, quitar `MAX_RECENTS` fijo; tests. |
| `crates/core/src/config/mod.rs` | Campo `recent_limit: usize` + default 50 (serde). |
| `crates/ui-slint/src/workspace_ctrl.rs` | Pasar el límite a `push`; truncar al cambiar settings; abrir/cerrar el menú de historial; navegar al elegir. |
| `crates/ui-slint/ui/app-window.slint` | Ícono de historial en la toolbar + menú flotante. |
| `crates/ui-slint/ui/config-window.slint` | Fila «Carpetas recientes a recordar» en Avanzado. |
| `crates/ui-slint/ui/i18n.slint` | `history`, `history-empty`, `cfg-recent-limit`. |
| `crates/core/src/i18n/es.json`, `en.json` | Traducciones. |
| `crates/ui-slint/src/i18n_keys.rs` | Setters. |
| `crates/ui-slint/src/main.rs` | Cablear el ícono (abrir menú), el modelo del menú y el control numérico. |

## Testing

- **core** (lo importante): `RecentDirs::push(limit)` respeta y clampa el límite; `truncate_to`;
  `Settings.recent_limit` default 50 y deserialización tolerante.
- **Visual (Nicolás):** que el ícono aparezca en la toolbar, el menú liste las recientes y
  navegue al elegir, el control de la config funcione (1–100) y trunque al bajar.

## Fuera de alcance (YAGNI)

- Historial cronológico con repeticiones (estilo navegador) — se reusa el MRU sin duplicados.
- Atajo de teclado dedicado para abrir el menú — se puede agregar luego.
- Limpiar/editar el historial desde el menú — no pedido; el panel Recientes ya existe.

## Riesgos y mitigaciones

- **Acoplar `recent_dirs` a `Settings`:** evitado pasando el límite por parámetro a `push`.
- **Config con `recent_limit` inválido:** clamp a `1..=100` al usar.
- **Menú con muchas rutas largas:** mostrar nombre + ruta tenue recortada; el tope de 100 evita
  una lista interminable.
- **Carpeta muerta elegida:** `remove_missing` antes de mostrar; y el aviso in-place cubre el
  caso límite.
