# Backlog — pendientes y mejoras de Naygo

> Documento de arranque para nuevas sesiones. Todo lo acordado que aún no se implementa,
> con contexto y punteros al código relevante. Actualizarlo al cerrar cada ítem.
> Última actualización: 2026-08-26.

## Estado de partida

- Versión en curso: **0.4.0+** (0.4.0 publicada 2026-08-16; hay trabajo sin publicar en el
  working tree: buscador F3 ampliado, carpetas conocidas de Windows en el árbol, y más).
- Suite: ~1.022 tests verdes. Clippy: 3 warnings menores (ver T-5).
- i18n: 10 idiomas con paridad verificada por `scripts/check_i18n_lang.py` (en CI).
- **Fix ya aplicado en el working tree (incluir en el próximo commit):** las 7 claves
  `slint.preview.{loading,cancel,cancel_tip,copy_tip,wrap_on_tip,wrap_off_tip,reset_view_tip}`
  se agregaron a los 8 idiomas que las perdían (pt/de/fr/hi/it/ja/ko/zh). Sin esto, el test
  `i18n::tests::todos_los_idiomas_tienen_las_claves_de_es` falla.

---

## A. Mejoras técnicas pendientes (importante aplicarlas)

### T-1. Split de `ops_ctrl.rs` (4.167 líneas)
El archivo creció el doble con 0.4.0 (wizard de sincronización + bandeja temporal + progreso
de borrado por lotes). Dividir por dominio siguiendo el precedente de `workspace_ctrl/`:
`ops_ctrl/sync_wizard.rs`, `ops_ctrl/basket.rs`, `ops_ctrl/batch_delete.rs`, dejando el core
de cola/ejecución en `ops_ctrl/mod.rs`. Refactor puro, sin cambio de comportamiento.

### T-2. Smoke test de UI en CI
`scripts/repro-rename.ps1` y `scripts/repro-scroll.ps1` (automatización real de la ventana:
SendKeys + screenshots + log) ya encontraron 4 bugs reales (crash F2, scroll muerto, blink
con Shift, auto-scroll sin cablear). Convertirlos en un smoke test que corra post-build
(release.yml o manual documentado en `docs/PRUEBAS.md`), idealmente con asserts sobre el log
y comparación de screenshots. Nota: requieren sesión interactiva de Windows (no corren
headless); quizá como paso manual pre-release más que en GitHub Actions.

### T-3. PDB fuera del camino crítico de descarga
`naygo.pdb` pesa ~263 MB y es ~80% del paquete (zip ~65 MB). Hacerlo **componente opcional**
del instalador Inno ("Símbolos de depuración", desmarcado por defecto) y **quitarlo del ZIP
portable** (baja a ~15 MB). Con el crash de F2 resuelto, el PDB solo se necesita a demanda.
Mantener el perfil release con `debug = true` (el PDB se genera igual; solo no se distribuye
por defecto).

### T-4. Firma de código del instalador
En máquinas externas SmartScreen advierte al instalar. Opciones: **SignPath.io** (certificado
gratis para open source, se integra en CI) o documentar el bypass en `docs/DISTRIBUTION.md`.
El gancho de `signtool` aún no existe en `scripts/build-release.ps1`.

### T-5. Tres warnings de clippy
- `crates/core/src/ops/undo.rs:522` — `clone()` → `std::slice::from_ref(&action)` (en test).
- `crates/ui-slint/src/workspace_ctrl/layout_panes.rs:594` — método muerto `split_for_target`
  (borrar o cablear donde corresponda).
- `crates/ui-slint/src/preview.rs:138` — tipo complejo del `mesh_load_rx`: extraer a un
  `type` alias.

---

## B. Funcionalidades aprobadas para implementar

### F-1. Exportar listado a CSV/texto
De la selección (o la vista completa) al portapapeles o a un archivo, con columnas
configurables (nombre, extensión, tamaño, fechas, ruta). Punteros: la construcción de celdas
ya existe en `crates/ui-slint/src/bridge.rs` (`cell_value`); añadir acción en menú contextual
(`crates/ui-slint/src/workspace_ctrl/context.rs`) + `Action` de keymap si se quiere atajo.
Pensado para flujo Excel/CSV del usuario. Formato CSV con `;` o `,` configurable, BOM UTF-8
para que Excel lo abra bien.

### F-2. Duplicar rápido (Ctrl+D)
Duplicar la selección en la misma carpeta con sufijo « - copia» (y « (2)», « (3)» si ya
existe, reusando la desambiguación existente del motor de ops). Implementar como una op más
del engine (`crates/core/src/ops/`), con undo incluido. Nueva `Action` de keymap con su
entrada i18n en los 10 idiomas.

### F-3. Modo filtro «ocultar no-matches»
Toggle en la mini-barra del filtro visual (la que muestra `filtro: "texto" · N ✕`): al
activarlo, la vista solo muestra las coincidencias (filtro real, no solo tinte). El buffer y
el ciclo ↓/↑ se mantienen. Punteros: el matching vive en `core::text_match` y la marca
`filter_match` en `bridge::rows_from_view`; la variante «ocultar» debe actuar sobre la VISTA
del `FilePaneState` (cuidar alineación de posiciones de vista con selección/foco, igual que
los filtros de columna existentes en `core::filter`).

### F-4. Historial de portapapeles interno (Ctrl+Shift+V)
Los últimos N (p. ej. 10) conjuntos de rutas copiados/cortados **dentro de Naygo**, con
popup para elegir y pegar. Solo rutas de archivos (no texto/imágenes del portapapeles de
Windows). Punteros: el pipeline de copiar/cortar/pegar vive en `workspace_ctrl/ops.rs` y
`ops_ctrl.rs`; guardar el historial en memoria (o en `workspace.json` si se quiere persistente).

---

## C. Cambios de atajos y diálogo de creación

### C-1. Intercambiar atajos de creación: Ctrl+N = carpeta
Hoy: `Ctrl+N` = nuevo archivo, `Ctrl+Shift+N` = nueva carpeta. El usuario prefiere:
**`Ctrl+N` = nueva carpeta**, y el archivo pasa a otra combinación (propuesta: `Ctrl+Shift+N`,
o sea, intercambiar los defaults). Cambiar en `crates/core/src/keymap.rs` (entradas `NewFile`/
`NewDir` en `defaults()`), documentarlo en la ayuda (F1) y en `docs/GUIA-DE-USUARIO.md`. Ojo:
el keymap del usuario ya persistido en `keybindings.json` tiene el valor viejo — decidir si se
migra o solo aplica a instalaciones nuevas (el sistema de keymap tiene `reset_action`).

### C-2. Doble Enter seguido = Ctrl+Enter en el diálogo «nueva carpeta»
El diálogo de nueva carpeta tiene un textarea multi-línea (permite crear varias carpetas de
una): **Enter+Enter seguidos debe equivaler a Ctrl+Enter** (confirmar y crear lo escrito).
Implementar en el handler `key-pressed` del textarea en `crates/ui-slint/ui/new-folder.slint`:
detectar dos Enter consecutivos (timestamp del último Enter, p. ej. < 500 ms) → disparar el
mismo callback que Ctrl+Enter.

---

## D. Bugs/mejoras de arrastrar y soltar

### D-1. Arrastrar dentro del MISMO panel sobre una carpeta (mejora futura)
Hoy el drop intra-panel no mueve/copia a subcarpetas. Implementar con **las mismas reglas que
el drop entre paneles** (copiar por defecto, mover con Shift/mismo volumen, menú con clic
derecho si aplica). Punteros: el drop entre paneles está resuelto en
`crates/ui-slint/src/workspace_ctrl/ops.rs` (`drop_at` y siguientes); extender el hit-test de
`body-touch` en `file-panel.slint` para detectar «fila carpeta destino» dentro del mismo panel.

### D-2. BUG: no se puede arrastrar desde 7-Zip (ni apps similares) a un panel
Arrastrar archivos desde un zip abierto en 7-Zip hacia un panel no copia nada. Causa probable:
7-Zip no entrega `CF_HDROP` sino `CFSTR_FILEDESCRIPTOR` + `CFSTR_FILECONTENTS` (streams
virtuales OLE), formato que el `IDropTarget` propio (`crates/platform/src/drop_target.rs`) no
implementa. Hay que soportar esos dos formatos: leer el descriptor (nombres) y extraer los
contenidos por `IStream` a la carpeta destino (o pedir a `core` un destino temporal).
Reproducir también con WinRAR y con el propio Explorer de Windows (que usa CF_HDROP y sí
funciona hoy) para no romper el camino actual.

---

## Notas de operación para la próxima sesión

- Verificación estándar: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets` (cero warnings), `cargo test --workspace` (todo verde), paridad i18n con
  `scripts/check_i18n_lang.py` para los 10 idiomas al tocar claves.
- Tras cada fix que afecte la app: rebuild de `dist` con `scripts/build-release.ps1` (el
  usuario prueba instalando desde `dist/`; ver el punto «Cómo trabajar» en AGENTS.md).
- La máquina del usuario estuvo bajo presión de RAM (~700 MB libres): correr builds pesados
  (release con LTO) SOLOS, sin otros trabajos en paralelo.
- `scripts/run-logged.sh` envuelve comandos pesados (deja salida + curva de RAM en
  `target/agent-out/` y bitácora en `logs/agent-bitacora.md`); útil ante caídas del entorno.
