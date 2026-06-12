# Path-bar interactiva + Favoritos — Diseño

> Fase 3 de la serie navegación (tras caché+recientes). Pedidos de Nicolás:
> path editable con autocompletado, ícono copiar path, ícono guardar a favoritos,
> saltar a carpetas del path con el mouse (como Explorer), y panel de favoritos.

## Path-bar (reemplaza el `ui.monospace(dir)` del tope de cada panel Files)

Dos modos, mutuamente excluyentes:

**Modo breadcrumbs (default).** El path se pinta como segmentos clicables
(`D:\ › Empresas › ISGroth › naygo_test_rename`): clic en un segmento navega a
esa carpeta. Clic en la zona vacía a la derecha (o `Ctrl+L` / `F4` en el panel
activo) pasa a modo edición. A la derecha, dos íconos chicos:
- 📋 **copiar**: pone el path absoluto en el portapapeles (status: «copiado»).
- ☆/★ **favorito**: agrega/quita la carpeta actual de Favoritos (estrella
  llena si ya es favorita).

**Modo edición.** TextEdit con el path completo seleccionado, con
**autocompletado**: al escribir, un popup muestra hasta ~12 subcarpetas que
calzan con el último segmento tecleado (más las recientes que calcen, marcadas).
`Tab`/clic completa, `Enter` navega (si la ruta existe; si no, status de error y
sigue editando), `Esc` vuelve a breadcrumbs sin navegar. El listado de candidatos
corre en un WORKER con debounce (~120 ms) — regla de oro: la UI no hace I/O.
Perder el foco cancela la edición (vuelve a breadcrumbs).

- v2 (fuera de alcance): clic en el separador `›` despliega las carpetas
  hermanas de ese nivel.

## Favoritos

- **`core/src/favorites.rs`** (puro): `Favorites { items: Vec<Favorite> }`,
  `Favorite { path: PathBuf, label: String }` (label = nombre de la carpeta,
  editable a futuro). API: `toggle(path)`, `contains(path)`, `list()`,
  `remove(path)`, (de)serialización JSON tolerante. Persistencia en
  `<config>/favorites.json` (se escribe al cambiar). Tests.
- **Panel Favoritos** (`PanePurpose::Favorites`, entra al menú ▾): lista de
  favoritos (clic navega el panel activo) + sección «Recientes» debajo (consume
  `RecentDirs`, con `remove_missing` al pintar). Clic derecho sobre un favorito:
  quitar.
- **Sección en el árbol**: los favoritos como raíces ancladas ARRIBA de las
  unidades en el panel Carpetas (clic navega, como cualquier nodo).
- **Atajos `Ctrl+1..9`**: saltar al favorito N (orden del panel). Entradas
  nuevas del keymap (acciones `GoFavorite1..9`, configurables como el resto).

## i18n / Config

Claves ES/EN para tooltips de los íconos, título del panel, «Recientes»,
«copiado», error de ruta. Sin settings nuevos (el autocompletado siempre activo).

## Tests

Favorites (toggle/contains/persistencia/carga corrupta), generación de
candidatos del autocompletado (función pura sobre una lista de nombres +
prefijo), breadcrumb split (función pura path→segmentos, incluida raíz de
unidad). UI manual en vivo.
