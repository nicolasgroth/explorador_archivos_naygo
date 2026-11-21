# Naygo 0.5.7 — integridad de datos, build, rendimiento y documentación

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
SPDX-License-Identifier: MIT

> **Para agentes:** ejecutar con superpowers:subagent-driven-development, tarea por tarea y en
> serie (todas comparten `target/`; nunca dos `cargo` a la vez). Pasos con casillas `- [ ]`.

**Objetivo:** cerrar las rutas de pérdida de datos de copiar/mover, bajar el tiempo y la memoria
de compilación, quitar trabajo redundante del hilo de UI y dejar README y docs al día.

**Origen:** revisión de código del 2026-11-21 (core/platform y ui-slint). Los hallazgos de la
Fase 1 se verificaron a mano en el código antes de escribir este plan.

**Rama:** `fix/integridad-build-0.5.7`, creada desde `82db92d5` (`codex/naygo-0.5.6`).

## Restricciones globales

- Rust 1.96.0 (`rust-toolchain.toml`). Gate: `scripts/test-all.ps1` (tests + clippy `-D warnings` + fmt).
- `core` no conoce Windows ni Slint: todo Win32 va en `platform`, inyectado por trait.
- El hilo de UI nunca hace I/O de disco.
- Ningún texto visible hardcodeado: claves i18n en los 10 idiomas (`check_i18n_lang.py`).
- Cada archivo nuevo lleva el header Naygo + SPDX.
- Dependencias nuevas solo MIT/Apache/Unlicense; actualizar `THIRD-PARTY-NOTICES.md`.
- Un solo trabajo Cargo a la vez. Correr tests por crate durante la tarea y el gate completo al
  cerrar cada fase.
- Commits en español, con build limpio y tests pasando. No se hace push sin el OK de Nicolás.
- Cambios visuales: solo propuesta, hasta que Nicolás los apruebe (Fase 5).

## Foco de revisión (casos que ningún test cubre del todo)

1. Mover entre volúmenes una carpeta que contiene un junction: el destino real del junction
   debe quedar intacto. Test en T1 (con un renombrador falso que fuerza `CrossesDevices`).
2. Origen y destino que son el mismo archivo con otra ortografía (hard link, mayúsculas Unicode,
   `\\?\`): Sobrescribir/Mover nunca debe borrar el único original. Tests en T2.
3. Copiar y luego "Saltar idénticos": la segunda pasada debe saltar todo. Test en T3.
4. Rutas de más de 260 caracteres en el paso de publicación. Test en T4.
5. Carpetas de OneDrive con placeholders: el tamaño y la búsqueda deben entrar en ellas.
   No hay fixture automatizable: validación manual en el OneDrive real de este PC (T5).

---

## Fase 1 — Integridad de datos (prioridad máxima)

### T1. El plan de copiar/mover no atraviesa enlaces

**Decisión:** los enlaces (symlink o junction) anidados nunca se recorren. Si un **origen
seleccionado** es un enlace y la operación es Copiar, se sigue una sola vez, porque el usuario lo
eligió explícitamente; lo anidado dentro de él tampoco se recorre. Al mover, un enlace se
renombra como enlace (mismo volumen). Si eso no es posible (entre volúmenes), el paso falla con
un mensaje claro y el enlace queda en el origen. Así nunca se borra nada a través de un enlace.
Alternativa descartada por ahora: recrear el enlace en el destino (los symlinks exigen
privilegio o Developer Mode; queda en el backlog).

**Archivos:** `crates/core/src/ops/plan.rs` (`plan_transfer_with`, `expand`),
`crates/core/src/ops/engine.rs` (`exec_copy_step`), tests en ambos.

- [ ] Helper de test `fn make_junction(link: &Path, target: &Path) -> bool` (Windows: `cmd /C mklink /J`; Unix: `symlink`). Devuelve `false` si no se pudo, y el test retorna temprano.
- [ ] Test `plan_copy_no_recorre_junction_anidado`: carpeta `A/` con `A/j -> T/` (con `T/f.txt`). `plan(Copy A)` produce un paso para `A/j` con `is_dir == false` y ningún paso cuyo `from` esté bajo `T/` o `A/j/`.
- [ ] Test `plan_copy_sigue_origen_enlace_seleccionado`: `plan(Copy [j])` con `j -> T/` produce pasos para `T/f.txt` (seguido una vez).
- [ ] Test `move_entre_volumenes_no_borra_a_traves_de_junction`: engine con un `NoReplaceRename` falso que devuelve `ErrorKind::CrossesDevices`, Move de `A/` → `dest/`. Asserts: `T/f.txt` sigue existiendo con su contenido; `A/j` sigue existiendo; el resumen tiene 1 `Failed`.
- [ ] Verificar que fallan: `cargo test -p naygo-core --locked ops::` → FAIL en los 3.
- [ ] `expand`: `symlink_metadata`; si `file_type().is_symlink()` y no es la raíz seguible, emitir un paso de archivo con `bytes: 0`. `expand` recibe `follow_root_link: bool` (true solo para Copy en el primer nivel). `DestInsideSource` usa `symlink_metadata` y no `src.is_dir()`.
- [ ] `exec_copy_step`: antes de resolver conflictos, si `symlink_metadata(from)` es enlace: en Move intentar `rename_noreplace` (mueve el enlace); ante cualquier error devolver `OpOutcome::Failed("enlace no movido entre volúmenes: …")`. En Copy devolver `Failed("enlace no copiado: …")`. Nunca `copy_buffered` ni `remove_file` sobre un enlace.
- [ ] Tests en verde: `cargo test -p naygo-core --locked ops::`.
- [ ] Commit `fix(ops): copiar/mover no atraviesa symlinks ni junctions`.

### T2. Identidad real de archivo antes de sobrescribir o mover

**Archivos:** `crates/core/Cargo.toml` (+`same-file = "1"`, ya está en el lock), `plan.rs`,
`engine.rs`.

- [ ] Test `mover_sobre_hard_link_del_mismo_archivo_no_pierde_datos`: `a.txt` y su hard link `b.txt` en otra carpeta. Move `a.txt` → carpeta de `b.txt`, renombrado para que el destino sea `b.txt` con `ConflictPolicy::Overwrite`. Assert: `b.txt` conserva el contenido y el paso es `Skipped`.
- [ ] Test `copiar_sobre_si_mismo_con_otra_ortografia_es_noop`: destino escrito como `\\?\C:\…` (Windows) del mismo archivo → `Skipped`, contenido intacto.
- [ ] Test `paths_eq_ci_ignora_mayusculas_unicode`: `"D:\\Árbol"` == `"D:\\ÁRBOL"`, `"D:\\ß"` != `"D:\\SS"` (NTFS no expande).
- [ ] Verificar que fallan.
- [ ] `fn eq_ignore_case_ntfs(a: &str, b: &str) -> bool` en `plan.rs`: compara char por char con `to_uppercase` solo cuando produce exactamente un char; si produce más, compara el char original. Usarla en `paths_eq_ci` y `path_starts_with_ci`.
- [ ] `exec_copy_step`: si `step.to` existe y `same_file::is_same_file(&from, &step.to)` es `Ok(true)`, devolver `Skipped` antes de `resolve_action`. `plan_transfer_with`: mismo chequeo para `base_to` frente a `src`.
- [ ] Tests en verde. Commit `fix(ops): detectar el mismo archivo por identidad, no por texto`.

### T3. Copia que preserva metadatos y es durable al mover

**Decisión:** el trait inyectado `NoReplaceRename` gana un método con implementación por defecto
(se evita renombrar el trait en este lote):

```rust
fn copy_and_publish(
    &self, from: &Path, to: &Path, overwrite: bool, durable: bool,
    token: &CancellationToken, on_bytes: &mut dyn FnMut(u64),
) -> std::io::Result<bool>; // Ok(false) = cancelado, sin parcial
```

- **Por defecto (core, Linux/tests):** el `copy_buffered` actual más `File::set_times` (modified y accessed; created en Windows vía `FileTimesExt`), `sync_all` si `durable`, y permisos de solo lectura aplicados **después** de publicar (el `persist` de tempfile limpia atributos).
- **Windows (`platform/src/copy_file.rs`, nuevo):** `CopyFile2` hacia un hermano temporal `.naygo-copy-<id>`, con callback de progreso que devuelve `COPY_FILE_CANCEL` si el token se cancela y espera en pausa. Luego `FlushFileBuffers` si `durable`, y `MoveFileExW` (con `MOVEFILE_REPLACE_EXISTING` solo si `overwrite`) usando rutas `\\?\` (T4). Ante error o cancelación se borra el temporal. Preserva timestamps, atributos y ADS (incluida `Zone.Identifier`) y aprovecha el offload SMB.
- `exec_copy_step` llama a `no_replace_rename.copy_and_publish(…, durable = is_move, …)`; el origen solo se borra si devolvió `Ok(true)`.

Pasos:
- [ ] Tests core: `copia_preserva_fecha_de_modificacion`, `copia_preserva_solo_lectura`, `copiar_dos_veces_con_saltar_identicos_salta_todo` (segunda pasada: todos `Skipped`), `move_cross_volume_pide_copia_durable` (copiador falso que registra `durable == true`).
- [ ] Tests platform (`#[cfg(windows)]`): `copyfile2_preserva_oculto_y_ads` (escribe `f.txt:Zone.Identifier` y comprueba que existe en el destino), `copyfile2_cancelado_no_deja_temporal`, `copyfile2_overwrite_reemplaza`, `copyfile2_noclobber_falla_si_aparece_destino`.
- [ ] Verificar que fallan; implementar; verde con `cargo test -p naygo-core -p naygo-platform --locked`.
- [ ] La UI ya inyecta `PlatformNoReplaceRename`; verificar con grep que todos los `spawn*` de producción la usan.
- [ ] Commit `fix(ops): copias con CopyFile2, metadatos preservados y move durable`.

### T4. Rutas largas

- [ ] `platform/src/long_path.rs` (nuevo): `pub fn to_verbatim(path: &Path) -> PathBuf`. `C:\x` → `\\?\C:\x`; `\\srv\sh\x` → `\\?\UNC\srv\sh\x`; si ya es verbatim o relativa, no se toca. Tests unitarios de cada caso.
- [ ] Usarla en `move_rename.rs` y `copy_file.rs`.
- [ ] Test `rename_noreplace_ruta_de_300_caracteres` (crea el árbol con std, que soporta rutas largas).
- [ ] `ui-slint/build.rs`: embeber un manifest con `<longPathAware>true</longPathAware>` vía `winresource::set_manifest`. Solo agrega ese setting; no cambia DPI ni comctl. Nota en ARQUITECTURA: depende de `LongPathsEnabled` del sistema; el `\\?\` cubre las llamadas propias aunque esté apagado.
- [ ] Commit `fix(platform): soporte de rutas largas en rename y copia`.

### T5. Clasificación de enlaces y placeholders de nube

- [ ] `sizing.rs`, `search.rs`, `search_many.rs`: reemplazar "cualquier `FILE_ATTRIBUTE_REPARSE_POINT`" por `file_type().is_symlink()` (std ya cubre symlinks y junctions, que son name-surrogate; los placeholders de OneDrive/Dropbox no lo son). Corregir el comentario errado de `sizing.rs:124`.
- [ ] Tests: `sizing_no_entra_en_junction` (sigue en verde) y `sizing_cuenta_carpeta_normal`.
- [ ] `listing.rs:407-411`: comprobar con un test que un junction a carpeta se lista y es navegable (doble clic o Enter entran). Si sale `EntryKind::Other` y no navega, clasificarlo como carpeta-enlace navegable sin recursión.
- [ ] Validación manual: tamaño de una carpeta de OneDrive en este PC antes y después. Anotar el resultado en ESTADO-ACTUAL.
- [ ] Commit `fix(core): los placeholders de nube no se tratan como enlaces`.

### T6. Carrera al restaurar de la Papelera

- [ ] `trash.rs:790,871`: no confiar en el chequeo previo más `FOF_NOCONFIRMATION`. Restaurar con `FOF_RENAMEONCOLLISION`, o verificar de nuevo justo antes y abortar si apareció un destino. Test con fixture que crea el destino entre el chequeo y la restauración (hook de test).
- [ ] Commit. **Cierre de fase:** gate completo + `graphify update .`.

## Fase 2 — Tiempo y memoria de compilación

Línea base actual: release de ~29 min con pico de 10,9 GiB; `app-window.rs` generado de 33 MB.

### T7. Medir la línea base

- [ ] Con `scripts/build-release-guarded.ps1` (ya mide el pico), registrar en `docs/BENCHMARKS.md`: release limpio, release tras tocar solo un `.rs` de `ui-slint/src`, y `cargo test -p naygo-ui-slint` incremental. Hora, pico y tamaño del exe.

### T8. build.rs deja de invalidar todo

- [ ] `rerun-if-changed` solo para `ui/` y el `.ico` (Cargo ya rastrea src y dependencias).
- [ ] `NAYGO_BUILD_ID`: lo fija el empaquetador (`build-release*.ps1` exporta la variable, con `rerun-if-env-changed`). En builds de desarrollo vale `dev`. La versión mostrada queda `X.Y.Z+build.dev` fuera de dist (decisión: distinguir builds importa en dist, que es lo que prueba Nicolás).
- [ ] Medir de nuevo el caso "tocar un .rs".

### T9. UI generada en su propio crate

- [ ] Crear `crates/ui-gen` (`naygo-ui-gen`, lib): se mueven `ui/` y la parte Slint de `build.rs`; `lib.rs` hace `slint::include_modules!();`. `ui-slint` depende de él y re-exporta con `pub use naygo_ui_gen::*;` en `main.rs`, así las rutas existentes no cambian.
- [ ] Mover también las features de Slint (`backend-winit`, `renderer-software`, `accessibility`, …) al crate nuevo o dejarlas unificadas por workspace.
- [ ] Verificar: tocar un `.rs` de ui-slint ya no recompila el código generado (medir).
- [ ] Actualizar ARQUITECTURA, CLAUDE.md y AGENTS.md (4 crates).

### T10. Perfil release

- [ ] `debug = "line-tables-only"`: medir pico y tiempo; comprobar que el backtrace del panic handler sigue mostrando nombres de función (provocar un panic en build `diag`).
- [ ] Probar `CARGO_BUILD_JOBS=2` con el pico medido. Subir el valor por defecto solo si el pico queda bajo 8 GiB.
- [ ] Solo con datos: si `ui-gen` sigue siendo el cuello, evaluar compilar ConfigWindow/Splash aparte (los globals como `Tr` se duplican por ventana). Decidir con la medición en la mano, no antes.
- [ ] Registrar el antes y después en BENCHMARKS. **Cierre de fase:** gate.

## Fase 3 — Memoria y CPU en ejecución, robustez de UI

### T11. Línea base de memoria en ejecución

- [ ] Con `capture-w07-process.ps1`, medir RSS/Private Bytes: arranque en bandeja, ventana idle, carpeta de 100k entradas, tras 20 previews, panel de operaciones con historial y 10 min idle. Registrar en BENCHMARKS. Esto define qué de T12/T13 se ataca primero.

### T12. Modelos persistentes en `sync_rows`

- [ ] `sync.rs:1111-1126` (ops), `:1154` (historial de portapapeles), `:443` (favoritos y grupos) y `:1403` (ayuda): guardar `Rc<VecModel<_>>` en un struct `PersistentModels` del sync. Actualizar con `set_row_data` cuando el largo no cambia, y con `set_vec` solo cuando cambia la firma del contenido. La ayuda solo al cambiar el idioma.
- [ ] Test headless: dos `sync_rows` sin cambios devuelven el mismo `ModelRc` (`ModelRc` comparado por puntero) y no emiten notificaciones.
- [ ] `windows_terminal_available()`: se calcula una vez en un worker al arrancar y se cachea.

### T13. Listados grandes sin costo O(n) por fila

- [ ] Benchmark existente (`w10c_headless_sync_measurement`): "actualizar una fila" 28,5 ms y "anexar una fila" 22,3 ms con 100k. Meta: < 3 ms.
- [ ] Reemplazar `apply_rows_incremental_by` (remove/insert por elemento) por un `RowsModel` propio (`impl Model`) sobre `Vec<RowData>`, con notificaciones por rango (`row_added(i, n)` / `row_removed(i, n)`) y sin `reset` (el `reset` mata la rueda; ver gotcha set_vec).
- [ ] Construir la fila completa (incluido el `VecModel` anidado de celdas) solo para las filas cambiadas.

### T14. Bloqueos en el hilo de UI

- [ ] `navigation.rs:1088` `is_dir()`: quitarlo; el worker de vista profunda ya reporta una carpeta inexistente.
- [ ] `ShellExecute` (navigation.rs:222, context.rs:433/442, input.rs:680): correr en un hilo con COM STA propio y reportar errores por canal.
- [ ] Test: abrir con un handler que tarda (fake) no bloquea el tick.

### T15. Shares caídos no bloquean lo local

- [ ] `workspace_ctrl/mod.rs:918`: dos pools de permisos, local (8) y remoto (4; UNC o unidad `DRIVE_REMOTE`).
- [ ] Test con un lister falso que bloquea: 8 listados remotos colgados y 1 local que termina.
- [ ] Planes de copia: una subcarpeta ilegible ya no aborta todo; se registra como paso `Failed` (`plan.rs:397`).

### T16. Menú contextual completo

- [ ] `context_menu.rs:225-253`: reenviar `WM_INITMENUPOPUP`, `WM_DRAWITEM`, `WM_MEASUREITEM` y `WM_MENUCHAR` a `IContextMenu3::HandleMenuMsg2` (o `IContextMenu2`) mientras `TrackPopupMenu` está activo (subclass temporal del HWND). Validación manual: "Enviar a", "Abrir con" y "Nuevo" muestran sus entradas e íconos.

### T17. i18n pendiente

- [ ] Claves para `packs.rs:282,469`, `batch_delete.rs:70,193` (sin `{:?}`), `splash.slint:43` y `color-picker.slint:22-23`, en los 10 idiomas.
- [ ] Test que recorre las propiedades del global `Tr` y exige un setter en `i18n_keys.rs`.
- [ ] Borrar el código muerto `save_editing_theme` y `delete_user_theme` (config_ctrl.rs:606-740), o hacer que los tests usen el camino real.
- [ ] **Cierre de fase:** gate más la medición de T11 repetida.

## Fase 4 — README y documentación al día

- [ ] `docs/evidencia/`: mover allí los 59 `VALIDACION-*.md` y actualizar los enlaces (grep de `VALIDACION-` en todo el repo).
- [ ] `BACKLOG.md`: archivar la bitácora actual en `docs/evidencia/backlog-historico-2026.md` y dejar un backlog corto en bullets, priorizado, con lo pendiente de este plan y la Fase 5.
- [ ] `ESTADO-ACTUAL.md`: solo el estado vigente (versión, gate, hashes de la última dist); el historial pasa a `docs/evidencia/`.
- [ ] `README.md`: novedades 0.5.7, comportamiento con enlaces, cifras reales de memoria y build, y la arquitectura de 4 crates. Revisar que cada función listada exista.
- [ ] `ARQUITECTURA.md`, `BUILD.md`, `GUIA-DE-USUARIO.md` (enlaces, rutas largas), `CHANGELOG.md` (0.5.7), `CLAUDE.md` y `AGENTS.md`.
- [ ] `bump.ps1` a 0.5.7, gate completo, regenerar dist (portable + instalador) con hashes.

## Fase 5 — Propuesta visual (requiere visto bueno de Nicolás)

Solo maqueta y propuesta. No se implementa hasta su aprobación:
- toolbar agrupada, con íconos de un solo tono y tooltips con el atajo;
- un grip en lugar del texto "mover" en los headers de panel;
- una sola barra de ruta;
- breadcrumb sin huecos;
- contraste AA en los temas oscuros;
- Propiedades sin truncar por defecto.

Revisar además el tamaño del instalador (69 MB contra 15 MB del portable).

## Deuda estructural (lotes propios, después de 0.5.7)

- Dividir `WorkspaceCtrl` (131 campos) en subcontroladores con un trait `poll() -> Changed`, para que el tick reposo sea `all(idle)`. Uno por lote: ops, portapapeles, preview, árbol.
- Mover a `core` el parseo de ZIP/TAR de `preview.rs` y la máquina de deshacer/journal de `ops_ctrl`.
- Separar preview/metadata (3D, syntect, PDF) en un crate `naygo-preview` usado solo por el proceso de preview.
- Tests: módulo `test_support` compartido y `recv_timeout` en vez de sleeps.
