# Naygo — arquitectura y contratos vigentes

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Referencia: base 0.5.2, incluidos RP-1–RP-5A y correcciones de navegación del 2026-09-10,
actualizada con el lote 0.5.7 (en preparación; [plan](plans/2026-11-21-integridad-build-y-docs.md)).
Las ampliaciones pendientes de preview se distinguen en el
[plan de rutas y preview](plans/2026-09-09-rutas-preview-y-configuracion.md).

## Responsabilidades

| Capa | Responsabilidad y puntos de entrada |
| --- | --- |
| `crates/core` | Modelos, listado incremental, motores de operaciones, búsqueda, configuración/temas, reglas de preview y formatos de trabajo. No depende de Slint ni Windows. |
| `crates/platform` | Shell/COM/OLE, Papelera, portapapeles, unidades, vigilancia nativa, ventanas, instancia única y arranque. |
| `crates/ui-gen` | Sólo los `.slint` de `ui/` y su código Rust generado (`AppWindow`, ventanas, structs y globals). Sin lógica: `build.rs` compila `ui/app-window.slint` y `lib.rs` hace `include_modules!`. |
| `crates/ui-slint` | Presentación Slint y coordinación: `workspace_ctrl/`, `ops_ctrl/`, `config_ctrl.rs`, callbacks, `tick.rs` y `sync.rs`. Winit y renderer por software. |

Los workers publican resultados por canales. La UI drena e invalida respuestas obsoletas,
conserva identidad de panel/archivo y actualiza modelos estables. No debe realizar I/O de disco
o red. RP-1 retira las lecturas síncronas conocidas del editor de rutas; no equivale a una
certificación universal de todos los callbacks del producto.

## Navegación y modelos

Correcciones 0.5.2: `FilePaneState` guarda el contexto por ruta antes de re-listar y aplica
el reveal después de ordenar. Subir al padre registra la carpeta hija; navegación nueva
descarta contextos pendientes anteriores. Las operaciones publican una notificación terminal
consumible una sola vez: el controlador re-lista destinos/orígenes abiertos y revela sólo
creaciones confirmadas cuyo destino aún está visible en el panel. No navega ni cambia el panel activo.

Los eventos que llegan durante un listado marcan una reconciliación pendiente; el snapshot
se repite al terminar para no resucitar eliminaciones ni perder altas. Un lote vacío del
watcher invalida el snapshot (registro completado o error/rescan del backend). La UI no resuelve
metadata faltante en disco. El timer comprueba los trabajos recién iniciados antes de dormir.
Las ramas afectadas del árbol se invalidan sin recorrer unidades completas y conservan
descendientes expandidos. Un watcher no soportado sigue requiriendo refresco explícito para
cambios externos posteriores: no se introduce sondeo periódico de ubicaciones remotas.

`matched-name.slint` limita los tramos a su ancho preferido y coloca el espacio sobrante al
final. `new-folder.slint` restablece explícitamente TextEdit al cambiar su estado abierto;
un binding inicial no basta después de editar. Los pegados usan segundos y reserva atómica
`create_new` en worker, desambiguación de nombres y conservación de todos sus canales de resultado.

- `workspace_ctrl/` conserva estado, historial, foco, selección y layout.
- `listing.rs` y `workspace_ctrl/listing.rs` coordinan resultados incrementales.
- `watch.rs` resuelve metadata fuera de UI, publica el lote final antes del wake y descarta
  generaciones antiguas. No hay vigilancia global ni indexador residente.
- `sync.rs` mantiene modelos y filas; en bandeja no reconstruir `TouchArea` entre down/up.
- `callbacks_pathbar.rs`, `workspace_ctrl/input.rs`, `breadcrumb-bar.slint` y
  `file-panel.slint` implementan las rutas actuales. `toolbar-path.slint` ofrece lectura,
  acciones y edición superior expandida; `edit-toolbar` decide dónde se monta el único
  editor de la sesión. Los callbacks usan `edit-pane` capturado, nunca el panel activo
  al confirmar. El overlay no altera geometría del workspace ni tapa los modales.
  `path-editor.slint` concentra la edición nativa y
  `path_window_events.rs` adapta AltGr únicamente si ese campo tiene el foco.
- `core/path_input.rs` valida sintaxis sin I/O. `path_validation.rs` valida físicamente
  en workers: dos plazas adquiridas ANTES de crear hilos, retenidas hasta su terminación
  real. El timeout de cinco segundos cancela la espera de UI, no la llamada del SO.
  `CancellationToken` se señala al descartar/expirar; el worker comprueba antes de I/O
  y antes de publicar, sin esperar ni hacer `join` desde la interfaz.
  La señal de cancelación se conserva aparte del callback: `Timer::stop()` en Slint
  retiene su cierre y no garantiza ejecutar el `Drop` del job al presionar Esc.
  Callback/revisión/panel/carpeta capturados descartan resultados obsoletos; `sync.rs`
  invalida el borrador al cerrar o navegar el panel. No se registran rutas inválidas en historial.
- `AutocompleteState` usa debounce de 120 ms y una consulta viva por campo (ruta/búsqueda).
  Cancelar conserva su receptor hasta drenarlo. Un timeout deja dormir el tick sin liberar
  capacidad artificialmente; resultados se descartan por revisión, no sólo por texto.
  Las enumeraciones consultan el token entre entradas y antes de empezar a leer.

### Admisión de listados: cupos local y remoto

`core/listing.rs` reparte los listados en dos cupos independientes (`ListingAdmissions`):
`LOCAL_LISTING_SLOTS = 8` y `REMOTE_LISTING_SLOTS = 4`. El permiso vive hasta que el worker sale
de verdad, así que un `read_dir` colgado en un share caído retiene una plaza remota. El listado
no sigue enlaces (`core/listing.rs`, `entry_from_path`): en Windows el tipo sale del propio enlace
(`FileTypeExt::is_symlink_dir` / `is_symlink_file`; un junction lleva el bit de directorio),
un enlace a archivo queda sin tamaño, y atributos y fechas son los del enlace. Así un enlace a un
UNC caído dentro de una carpeta local no retiene la plaza local. El watcher, la vista profunda y
la búsqueda leen igual (`listing::entry_at`, `symlink_metadata`). La clasificación está en
`core/path_locality.rs`: el trait `VolumeLocality` se inyecta
(como `NoReplaceRename`); una ruta UNC es remota por sintaxis, una letra se decide con la tabla
de unidades y lo desconocido cuenta como remoto. `platform/drives.rs` implementa el trait sobre
la tabla que ya trae el worker de unidades (`DriveTableLocality`, `DriveKind::Network`): el hilo
de UI no llama a `GetDriveTypeW`. Hasta que llega la primera tabla, las rutas con letra usan el
cupo remoto. En `workspace_ctrl`, `listing_admission_for` elige el cupo; las colas pendientes ya
no se cortan en el primer `Capacity`, y cada pendiente se reclasifica al reintentar.

### Modelos persistentes y `RowsModel`

`sync_rows` corre en cada tick mientras hay trabajo (30 ms). Un `ModelRc` nuevo por tick hace que
Slint marque sucio lo que lo contiene (los `ModelRc` se comparan por puntero) y, con el renderer por
software, repinte la ventana entera. Por eso:

- `persistent_models.rs`: `PersistentModels` guarda un `Rc<VecModel>` por modelo de ventana (ops,
  portapapeles, ayuda, favoritos, recientes, menú de columna, renombrado por lotes, asistente de
  sincronización, metadata del Inspector, resultados y puntos). `update_rows` usa `set_row_data`
  solo en las filas distintas si el largo no cambia y `set_vec` si cambia; sin cambios no notifica.
  `PreviewVmCache` arma la parte pesada del VM de preview una vez por revisión del preview e idioma.
- `rows_model.rs`: `RowsModel` (`impl slint::Model` sobre `Vec<RowData>`) avisa altas y bajas por
  tramo (`row_added(i, n)` / `row_removed(i, n)`), nunca con `reset` (que mata la rueda). Guarda en
  paralelo una `RowKey { id, content }` por fila (hash de 64 bits de la ruta y de todo lo que se
  pinta). `sync_rows` compara claves y construye solo las filas que entran o cambian; los deltas de
  core (alta al final, filas modificadas) se aplican directo. La fila en renombrado en línea nunca
  se re-arma mientras dura la edición.
- El resaltado de cambios no reconstruye filas por tick: la firma de filas incluye el conjunto de
  rutas frescas (`Watchers::fresh_signature`) y, si solo queda apagar resaltados, el timer duerme
  hasta el próximo vencimiento (`next_fresh_expiry`).

## Operaciones y datos del usuario

`core/ops/` planifica/ejecuta; `ui-slint/src/ops_ctrl/` coordina archivos, ZIP, lotes y journal.
Planificar no equivale a ejecutar: preservar destinos capturados, confirmaciones, conflictos,
cancelación y contratos de deshacer. Papelera se implementa en `platform/trash.rs`; no aceptar
un resultado aparente del Shell sin comprobarlo. DnD desde archivadores necesita conservar o
materializar fuentes efímeras antes de devolver el control al emisor.

Contratos del motor de copiar/mover (`core/ops/plan.rs`, `core/ops/engine.rs`):

- **Enlaces.** `expand` usa `symlink_metadata` y nunca recorre un enlace anidado
  (`is_link_like` = `file_type().is_symlink()`, que en std incluye junctions; no se usa
  `FILE_ATTRIBUTE_REPARSE_POINT`, que también llevan los placeholders de nube). Solo el origen
  seleccionado en Copiar/Duplicar se sigue una vez (`follow_root_link`). En el motor, un enlace en
  Mover se renombra como enlace (`rename_noreplace`) y, si no se puede (entre volúmenes), el paso
  es `Failed` (`naygo:link-not-moved`); en Copiar, un enlace a carpeta es `Failed`
  (`naygo:link-not-copied`) y uno a archivo se copia por contenido. "Reintentar" excluye los
  fallos de enlace (`ops::is_link_failure`, que también reconoce los textos previos sin código).
  Un Replace cuyo árbol tiene un enlace que no se copiaría o un `planned_failure` se rechaza en
  `prepare_tree_replacements` antes de crear staging (`naygo:replace-incomplete`).
  Nunca se borra nada a través de un enlace ni se recorre uno anidado; la única lectura a través
  de un enlace es la copia por contenido de un enlace a archivo.
- **Identidad.** Antes de resolver conflictos, `ops::same_identity(from, to)` (sobre
  `same_file::is_same_file`) convierte el paso en `Skipped`; `plan_transfer_with` hace lo mismo con
  la raíz. Es fail-closed: si el destino existe y la identidad no se puede determinar, el paso (o
  la raíz, como `planned_failure`) falla con `naygo:identity-unknown`. `paths_eq_ci` y
  `path_starts_with_ci` comparan con `eq_ignore_case_ntfs` (mayúsculas Unicode de un solo char;
  aproxima la tabla `$UpCase` de NTFS).
- **Copia y publicación.** El trait inyectado `NoReplaceRename` tiene `copy_and_publish(from, to,
  overwrite, durable, token, on_bytes) -> io::Result<bool>` (`Ok(false)` = cancelado, sin parcial).
  El default de core copia por buffers a un temporal `.naygo-copy-…`, aplica fechas, `sync_all`
  si `durable` y permisos. En Windows, `platform/copy_file.rs` usa `CopyFile2` (fechas, atributos,
  ADS) y publica con `MoveFileExW` (`WRITE_THROUGH` si `durable`). `durable` vale `true` en Mover:
  el origen solo se borra con `Ok(true)`. Con `overwrite`, si publicar da acceso denegado sobre un
  destino de solo lectura, ambos caminos quitan el atributo y reintentan una vez
  (`ops::retry_over_readonly_destination`, que lo repone si vuelve a fallar). Un fallo de
  publicación sale como `naygo:publish-failed`. Las entregas (`core/delivery.rs::execute`) reciben
  el copiador inyectado; la UI les pasa `PlatformNoReplaceRename`.
- **Rutas largas.** El manifest del `.exe` (`ui-slint/build.rs`) declara `longPathAware`, que solo
  surte efecto si el sistema tiene `LongPathsEnabled`. Las llamadas Win32 propias de copia y rename
  usan rutas `\\?\` (`platform/long_path::to_verbatim`), así que funcionan aunque esa
  opción esté apagada.
- **Fallos planificados.** `OpStep.planned_failure: Option<String>` (serde `default`, se omite si
  es `None`: los journals viejos siguen leyéndose). Si falla el `read_dir` de una subcarpeta, el plan
  deja un único paso fallido para ella y no planifica su contenido; si falla el stat de un hijo, un
  paso fallido de archivo (un hijo que desaparece, `NotFound`, se omite). La raíz ilegible sigue
  siendo `PlanError::SourceUnreadable`. El motor reporta el paso como `Failed` sin ejecutar nada.
  En Mover, `remove_empty_move_source` usa `remove_dir` no recursivo, así que la carpeta que
  contiene la rama ilegible sobrevive; un Replace con fallos no publica. El lector se inyecta con
  `PlanDirReader` para probarlo sin ACL. Para carpetas, el motivo lleva el prefijo
  `ops::UNREADABLE_FOLDER_FAILURE`. Todos los códigos (`naygo:<código>` + U+001F + campos) se leen
  con `ops::parse_failure` → `OpFailure`; la UI los traduce (`OpDetailTexts::localize`, claves
  `ops.failed.*`) y core los quita en los textos sin i18n, como los de entregas
  (`ops::plain_failure_text`). Un texto sin código (journal viejo) se muestra tal cual.
- **Papelera.** Restaurar usa `FOF_RENAMEONCOLLISION` y un `IFileOperationProgressSink` que
  registra la ruta real de aterrizaje; si difiere del original, se informa el otro nombre en vez de
  pisar el destino.

### Acciones del Shell fuera del hilo de UI

`ui-slint/src/shell_exec.rs`: `ShellExecutor` con un único hilo `naygo-shell` que inicializa COM
STA una vez (`platform::open::shell_job_runner`) y ejecuta `ShellJob` (Open, OpenWith,
RunAsAdministrator, Folder). Ese hilo no bombea mensajes, así que `platform/open.rs` llama a
`ShellExecuteExW` con `SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI`: las activaciones DDE o delegadas
terminan antes de volver. `submit` nunca bloquea; los resultados vuelven por canal y despiertan
la UI con un waker compartido. Cada orden corre en `catch_unwind` y una orden idéntica ya encolada
se descarta. `WorkspaceCtrl::submit_shell` muestra el error por el toast existente. Un manejador
colgado retrasa las órdenes siguientes, pero la UI no se congela.

Entregas, espacios, consultas, puntos y recetas ya tienen guías específicas. No sustituirlos
por scripts automáticos ni modificar originales al importar un archivo de configuración.

## Preview actual y ampliación

Actualización RP-5B (0.5.2): `core::preview_policy::Quality` concentra resolución/cadencia.
`Settings.preview_economical` es un booleano aditivo en esquema 4, ausente = false.
El request interno propaga calidad a imágenes/SVG y animación; 3D la aplica al raster.
Cambiarla cancela la generación y rearma el debounce conservando identidad/reglas.

`RetainedBudget` comparte 64 MiB entre caché de imágenes/texto y escenas del único
controlador Preview (todos los paneles). Usa reservas RAII no bloqueantes con suma
comprobada. Imagen reserva 2× RGBA antes de construir el buffer Slint; texto reserva
4× bytes/estructuras copiados; escenas contabilizan capacidades y conservan el permiso
dentro del Arc usado por renderers retirados. Se admiten escenas de hasta 48 MiB para
dejar margen de presentación. La imagen Slint se construye una vez y los VM clonan
su handle, no los píxeles en cada actualización. Límite agotado: estado terminal/reintento.

Este presupuesto limita retención, **no RSS global**, memoria del parser, raster temporal,
gramáticas compartidas ni copias internas del renderer/VM durante reemplazo. Los lectores
aislados mantienen su límite independiente de 192 MiB y dos plazas incluyendo retirada.
RP-4B2 extiende el aislamiento a texto/resaltado y STL/3MF. La medición global de RSS
completa sigue pendiente; no confundirla con las cotas de buffers administrados.

`ui-slint/src/preview.rs` coordina debounce de 150 ms, workers, lectura y rasterización.
`core/preview.rs` contiene clasificación, reglas y límites de texto. `preview-panel.slint`
presenta texto seleccionable/scroll, imágenes estáticas, escala y controles de cámara 3D.
SVG se rasteriza por CPU; PDF muestra texto, no páginas renderizadas.

RP-4A: `preview_workers.rs` usa permisos RAII retenidos por el hilo (dos plazas compartidas
por lectura, geometría y raster). Soltar un receptor no libera una plaza prematuramente;
los receptores exclusivos aíslan generaciones incluso en A → B → A. Canal de resultado
acotado a uno y sólo último objetivo/cámara pendiente. Sin capacidad, no clona reglas
ni lanza un hilo nuevo. Fallo/desconexión/cancelación terminan hasta Reintentar.
`drive_preview_visible` combina pestaña/panel maximizado con visibilidad de ventana;
`platform/window::content_visible` distingue WS_VISIBLE de ventana minimizada (IsIconic).
No se exige foco de foreground. Al ocultar, la vista/escena se libera y el trabajo recibe
cancelación cooperativa; las plazas de quienes siguen bloqueados permanecen ocupadas.
El hook winit existente en `path_window_events.rs` conserva AltGr y difiere un wake puntual
ante foco/tamaño/ocultación, para restaurar desde la barra de tareas aun con el tick dormido.
Gestos 3D presentan frames de la escena vigente mientras preparan la última cámara;
el relleno de triángulos consulta cancelación por bloques de scanlines.

RP-4B1: `core/preview_wire.rs` valida el protocolo NGP1 (petición 256 KiB, texto 1 MiB,
RGBA hasta 2048 × 2048, aritmética comprobada antes de reservar). `platform/preview_process.rs`
supervisa el modo interno del mismo EXE para imágenes, SVG, PDF y comprimidos. El dispatcher
`ui-slint/preview_isolated.rs` corre antes de logging, UI, preferencias e instancia única.
El padre asigna un Job Object antes de enviar la petición y el hijo comprueba sus límites
antes de leer archivos. El Job no se hereda y admite un solo proceso: 192 MiB comprometidos,
5 s de CPU; el supervisor impone 5 s de tiempo de pared. Cancelación fuera de la cola IPC,
kill/reap y join de pipe sólo en el supervisor worker. Al morir el padre se cierra el Job.
Dos plazas incluyen supervisores retirándose, como RP-4A; cada proceso añade un hilo de pipe
en el padre. El hijo tiene prioridad inferior a normal y Rayon limitado a un hilo,
sin modificar esas preferencias en el proceso principal.
Errores de IPC/salida/límite son terminales, traducidos y sin fallback ilimitado.

La memoria del Job excluye buffers/cachés de UI, texturas, escenas 3D y otros workers: no es
un límite global ni un sandbox. El decoder de imagen añade límites de dimensiones y reserva
best-effort de 64 MiB; la entrada comprimida se limita durante lectura a 20 MiB. SVG limita
canvas de salida sin un mínimo de escala que permita desbordarlo y desactiva referencias
a imágenes externas (datos incrustados conservados). El PDF aún carga/extrae antes de recortar;
el Job contiene su expansión pero no sustituye la mejora del lector prevista en RP-6.
Texto y 3D también usan el supervisor: no existe fallback de producto a lectura en proceso.
Las reglas y el lenguaje se deciden sin I/O antes de despachar. El resaltado corre en el
hijo; `preview_text_wire` limita texto, líneas y segmentos antes de cada reserva, y valida
que los segmentos reconstruyan el contenido original (sin texto oculto añadido).

`mesh_scene_wire` recibe geometría directamente de la tubería, sin un JSON ni un segundo
buffer completo. Valida longitud total (48 MiB), conteos, índices, unidades y coordenadas
finitas antes de retenerla. La escena validada entra al presupuesto de retención existente;
su Arc sigue al último renderer retirado. Girar la cámara no hace I/O ni crea otro proceso.
`mesh_preview::rasterize` admite como máximo 64 MiB de scratch (proyección + RGBA + profundidad)
y transforma la proyección en el mismo vector, consultando cancelación también en esa fase.

`Workers` reserva 256 MiB de margen contable por trabajo antes de despachar, compartiendo
512 MiB para las dos plazas, incluidas las retiradas. No preasigna esa RAM: cubre el hijo
limitado a 192 MiB más la mayor respuesta IPC (48 MiB), o el raster local de 64 MiB.
El permiso se libera al terminar el hilo/supervisor, no al descartar el receptor. Las
respuestas ya terminadas tienen canales de capacidad uno y tamaños máximos de protocolo;
la retención de 64 MiB es separada. Allocator, stacks, Slint/VM y resto de Naygo quedan fuera:
esto no impone ni mide un techo de RSS total. RP-6 conserva la medición y optimización fina.

RP-5A: `preview_animation_reader.rs` usa los decoders fijados de image, composición original,
lectura hasta 20 MiB, canvas hasta 4 Mpx, salida hasta 768 px y presentación hasta 15/s.
No se colectan frames; reiniciar un ciclo reutiliza los bytes y libera el decoder anterior
antes de crear otro. GIF interpreta la extensión de repeticiones por bloques, distinguiendo
ausencia (una pasada), cero (infinito) y n repeticiones adicionales. WebP/APNG usan sus APIs
de loop count. Delays mínimos de 10 ms; microsegundos acumulados con aritmética comprobada.

`preview_animation_wire.rs` (NGA1) encapsula imagen estática, frame con tiempo o final.
Las dimensiones animadas se validan antes de reservar el cuerpo. Un crédito stdin por
respuesta, canales de capacidad uno y cancelación independiente. El supervisor persistente
en `platform/preview_animation_process.rs` conserva las dos plazas compartidas de RP-4A;
no lanza un proceso por frame. El Job mantiene 192 MiB, un proceso y kill-on-close; reemplaza
CPU acumulada por tasa hard-cap del 20 % del sistema (no es una meta de consumo). Timeout
de 5 s sólo con pedido pendiente; pausa/delay no consumen ese presupuesto.

`preview_animation.rs` posee reloj monotónico, pendiente única y token por generación.
Pausar no genera créditos y Reanudar desplaza el origen temporal. El lector puede terminar
su único pedido en curso, pero no acumula la animación. Cancelar/ocultar/cambiar destruye
la sesión sin join en UI. Un final natural permite Repetir; error no se reintenta solo.
`preview_animation_tick.rs` actualiza únicamente los PaneVm de preview (cada presentación),
sin bombeo de listados/árbol/workspace. Se apaga en pausa/final/estático/error.
El reloj descarta presentaciones atrasadas, pero permite avance tras 250 ms sin
mostrar ninguna si el decoder no puede alcanzar el ritmo. Cambiar el tipo/deshabilitación
de la regla activa retira el lector; editar otra regla no reinicia la reproducción. No hay precarga
de carpetas ni persistencia de fotogramas. RP-5B incorpora los ajustes económicos descritos
arriba. La medición global de memoria/consumo continúa pendiente; las animaciones de íconos
son otra preferencia.

## Configuración, temas e idiomas

- `core/config/mod.rs`: versión actual de configuración **6** (`CONFIG_VERSION`), defaults y
  migraciones por etapas al cargar. Un `settings.json` de versión mayor (downgrade de la app) se
  descarta a defaults; un `workspace.json` de versión mayor se ignora (`load_workspace_flagged`,
  `config/mod.rs:927-944`). Revisar compatibilidad y downgrade antes de subir la versión.
- `core/config/fonts.rs`: interfaz de sistema, listados que heredan y Consolas para preview.
- `core/config/settings_writer.rs`: orden por revisión para impedir que un guardado antiguo
  de fuentes sobrescriba preferencias nuevas. No implica que todo guardado histórico sea async.
- `core/theme/mod.rs` y `theme/builtin/`: quince temas de fábrica, incluidos Nieve, Perla,
  Arena, Salvia, Bruma y Grafito claro. `icon_accent` separa énfasis de primer plano de
  `highlight` (filas recientes); al faltar hereda el `highlight` antiguo. No fuerza tema
  ni modifica el esquema 4 de configuración. El editor conserva índices 0–14 y añade 15.
- `config-window.slint`: catálogo de íconos único en categoría 9; Apariencia (3) sólo
  enlaza a él, sin aplicar otro set. Previsualización conserva su categoría 7.
- `core/icon_set.rs`: diez sets, IDs persistentes; Outline/Tiles conservan `kenney-game`/
  `kenney-board`. No renombrar IDs al reorganizar etiquetas o pantallas.
- `core/i18n/`: diez catálogos. Conservar placeholders; paridad no equivale a traducción completa.

## Construcción, diagnóstico y comentarios

Slint compila en un hilo de build con stack de 8 MiB; no es memoria reservada al ejecutar Naygo.
El código generado (~32 MiB de Rust) vive en `naygo-ui-gen` para que tocar un `.rs` de
`ui-slint`, `core` o `platform` no lo recompile: sólo se reconstruye al cambiar `ui/`.
`ui-slint` lo re-exporta (`pub use naygo_ui_gen::*;` en `main.rs`), así `crate::AppWindow` y
demás rutas no cambian. Las features de `slint` se definen una sola vez en
`[workspace.dependencies]`. El `build.rs` de `ui-slint` sólo embebe ícono/manifest/metadatos y
estampa el build ID; se re-ejecuta por el `.ico`, `Cargo.toml` o `NAYGO_BUILD_ID`.
La versión sale de `Cargo.toml`; el build ID lo fija el empaquetador (`NAYGO_BUILD_ID`,
`YYYYMMDDHHMM`) y en desarrollo vale `dev` (`X.Y.Z+build.dev`). `Cargo.lock` fija
dependencias y `rust-toolchain.toml` fija Rust 1.96.0 con rustfmt y clippy. Perfiles y cifras de
compilación: [BUILD.md](BUILD.md) y [BENCHMARKS.md](BENCHMARKS.md).

Comentar el porqué de identidad, límites, ownership, cancelación y compatibilidad junto al código.
No documentar como garantía una aspiración ni copiar una API sin comprobar la dependencia fijada.
Mantener headers Nicolás Groth/ISGroth y SPDX. Los comentarios actuales sobre límites pendientes
se deben corregir junto con la implementación correspondiente, no para aparentar que ya se resolvió.
