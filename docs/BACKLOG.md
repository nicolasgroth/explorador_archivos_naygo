# Backlog — pendientes de Naygo

Lista corta y priorizada de lo que falta. La bitácora completa hasta el 2026-11-24 está en el
[backlog histórico](evidencia/backlog-historico-2026.md). Al cerrar un ítem, se borra de aquí y
el detalle va al CHANGELOG o a `docs/evidencia/`.

## 1. Validaciones manuales pendientes (Nicolás)

- **Menú contextual nativo ("Más opciones de Windows…").** Con una build de desarrollo
  (`cargo build -p naygo-ui-slint --bin naygo`) lanzada con `$env:NAYGO_DEBUG_MULTI_INSTANCE="1"`
  para no chocar con la instancia instalada:
  1. Clic derecho sobre un archivo (por ejemplo un `.txt`) → "Más opciones de Windows…".
  2. "Enviar a": debe listar destinos (Escritorio, Documentos, carpeta comprimida, unidades) con
     íconos. Antes salía vacío o con un solo ítem.
  3. "Abrir con": debe listar aplicaciones con sus íconos y "Elegir otra aplicación".
  4. Si están instalados 7-Zip, Git o TortoiseGit: sus submenús muestran entradas e íconos.
  5. Teclado: la letra subrayada de un ítem de extensión funciona dentro del menú.
  6. "Enviar a → Escritorio (crear acceso directo)" se ejecuta de verdad.
- **Copia hacia un USB FAT32/exFAT** de un archivo descargado de internet (tiene el flujo
  alternativo `Zone.Identifier`): confirmar que la copia termina bien y no falla por el flujo.
- **Política EFS → FAT.** Hoy un archivo cifrado con EFS copiado a un volumen sin EFS sale
  descifrado sin aviso (igual que antes del lote). Decidir si eso se mantiene o se avisa.
- **Tamaño de carpeta en OneDrive.** Calcular el tamaño de una carpeta de OneDrive (con archivos
  solo en línea) y confirmar que entra en ella y que no descarga los archivos.
- **Resaltado de cambios en vivo.** Ver en pantalla que el resaltado de un archivo nuevo aparece y
  se apaga a los 8 s, y que un renombrado en curso sobrevive a un archivo que aparece en la misma
  carpeta. Hoy solo lo cubren tests headless.
- **Memoria en ejecución, escenarios 5 y 6** de la línea base de
  [BENCHMARKS.md](BENCHMARKS.md): tras 20 previews (medición solo indicativa) y con el panel de
  operaciones con historial (no medido).

## 2. Pendientes del lote 0.5.7

Primero lo que nota el usuario:

- **"Nuevo" en el menú contextual.** Solo existe en el menú de *fondo* de carpeta
  (`IShellFolder::CreateViewObject`). Naygo arma el menú sobre ítems, incluso al hacer clic en
  zona vacía. Requiere un modo "fondo" en `crates/platform/src/context_menu.rs`: tarea aparte.
- **Enlaces al copiar/mover.**
  - Copiar un árbol con junctions o symlinks a carpeta informa un fallo por cada enlace. Falta la
    opción de recrear el enlace en el destino (los symlinks exigen privilegio o Modo de
    desarrollador).
  - Mover un enlace cuyo destino ya existe informa "enlace no movido entre volúmenes" en vez de un
    conflicto, y no aplica la política de conflictos.
- **Mismo archivo por otra ruta.** El aviso de conflicto de carpetas (`ops/mod.rs`,
  `folder_conflicts`) sigue comparando por texto: puede pedir conflicto para el mismo archivo y,
  con Reemplazar, abortar el plan. Con la política de conflicto "Renombrar", copiar sobre el
  mismo archivo ahora se omite en vez de crear "x (1)": confirmar que es lo deseado.
- **Acciones del Shell.** Hay un único hilo para abrir archivos, terminal y Explorador: un
  manejador colgado retrasa las acciones siguientes (la UI no se congela). Posible timeout o pool.
- **Shares caídos.** Seguir un enlace que apunta a un UNC caído bloquea un worker de listado
  hasta el timeout. Hasta que llega la primera tabla de unidades, los listados con letra usan el
  cupo remoto.
- **Papelera.** Si el Shell no informa la ruta de aterrizaje, una restauración con colisión se
  informa como éxito (sin pérdida). Comparar por identidad y no por texto evitaría falsos "otro
  nombre" con rutas 8.3, `subst` o `\\?\`.
- **Metadatos de copia.** La copia por defecto aplica todos los bits de atributo del origen
  (incluidos OFFLINE/PINNED de OneDrive); conviene limitarla a solo lectura, oculto, sistema y
  archivo. La creación no se conserva igual en todos los caminos.
- **Textos sin clave i18n:** unos 20 errores de importar/exportar packs, los detalles técnicos de
  `TrashError::Failed`, "worker de listado desconectado" (`listing.rs`), el aviso "La plantilla …
  no existe" y el diálogo de `--theme`. Los demás `Failed` del motor siguen en español (deuda
  previa).
- **Propuestas de rendimiento (decide Nicolás).**
  - Filas perezosas: guardar solo las claves y armar la fila al pedirla; ~100 MiB menos por cada
    100.000 filas.
  - Campos fijos de fila en vez de `cells: [string]` (toca `.slint`: es visual).
  - Índice por nombre en core para quitar el recorrido O(n) por evento.
  - Bajar de ~3,8 ms por evento con 100.000 filas: índice ruta→posición (~1,6 MiB por 100.000),
    índice de la entrada modificada desde core, o holgura de capacidad en `RowsModel`.
- **Instalador.** 69 MB contra 15 MB del portable: revisar qué lo infla.

Técnico (sin efecto visible):

- Tests inestables bajo carga: `platform::preview_process` (timeout de procesos hijo).
- Build: medir A/B el tiempo del release (`line-tables-only` contra `debug = true` en el mismo
  commit, ~10 min más sin causa confirmada); verificar que el backtrace de la build `diag`
  conserva nombres de función; el clippy Linux de CI con `-p naygo-ui-slint` no cubre
  `naygo-ui-gen`; el `pub use naygo_ui_gen::*` puede sombrear nombres en silencio.
- Scripts de medición: `capture-w07-process.ps1` suma PIDs reutilizados en los campos `tree_*`;
  `measure-build.ps1` mide con ±0,5 s y no fija `CARGO_BUILD_JOBS`.
- Listados: un cambio de revisión de íconos cambia todas las claves de fila (n avisos);
  `update_rows` duplica bucles de puntos y resultados de búsqueda; `search_many::contains` hace un
  `symlink_metadata` extra por archivo.
- Operaciones: un `planned_failure` temprano levanta la barrera del journal (más
  `skipped_changed` al retomar); `create_dir_all` del padre descarta su error antes del rename de
  un enlace; un `stat` extra por archivo en red; `TempGuard` sin log si falla
  `symlink_metadata`; `Advise` sin `Unadvise` en errores tempranos de la restauración.
- Tests: identidad en `plan_transfer_with` sin test propio; el test de hard link no ejercita
  rename-falla→copia→borrado; los tests de textos usan claves, no las cadenas de los 10 idiomas;
  falta un test de extremo a extremo de una entrega con subcarpeta ilegible real.
- Menores: `eq_ignore_case_ntfs` aproxima `$UpCase`; `sync_file` pierde el error; `to_verbatim`
  usa `to_string_lossy` en UNC; el comentario "rename durable" exagera `WRITE_THROUGH`; doc
  duplicado de `fs_lister` (`sizing.rs:88-91`); `PostMoveItem` no mira `hrMove` y la caché de
  nombres queda vieja; `measure-build.ps1` no lista `rust-lld` y el quoting de `cmd /c`.
- Código: `return` redundante de `--shutdown` con `NAYGO_DEBUG_MULTI_INSTANCE`; el log de
  `batch_delete.rs` escribe el detalle crudo; helper de test duplicado en `config_ctrl.rs`.

## 3. Fase 5 — propuesta visual (requiere visto bueno de Nicolás)

Solo maqueta y propuesta; no se implementa hasta su aprobación:

- toolbar agrupada, con íconos de un solo tono y tooltips con el atajo;
- un grip en lugar del texto "mover" en los headers de panel;
- una sola barra de ruta;
- breadcrumb sin huecos;
- contraste AA en los temas oscuros;
- Propiedades sin truncar por defecto.

## 4. Deuda estructural (lotes propios, después de 0.5.7)

- Dividir `WorkspaceCtrl` (131 campos) en subcontroladores con un trait `poll() -> Changed`, para
  que el tick en reposo sea `all(idle)`. Uno por lote: ops, portapapeles, preview, árbol.
- Mover a `core` el parseo de ZIP/TAR de `preview.rs` y la máquina de deshacer/journal de
  `ops_ctrl`.
- Separar preview/metadata (3D, syntect, PDF) en un crate `naygo-preview` usado solo por el
  proceso de preview.
- Tests: módulo `test_support` compartido y `recv_timeout` en vez de sleeps.

## 5. Abierto en el backlog histórico

Las etiquetas (T-4, RP-…, E-…, secciones A, B, D y E) son las del
[backlog histórico](evidencia/backlog-historico-2026.md).

- **Firma de código del instalador (T-4).** El script ya firma si recibe `SignToolPath` y
  `CertificateThumbprint`; falta que ISGroth obtenga y autorice un certificado.
- **Aceptación nativa de rutas editables (RP-1/RP-2):** AltGr/IME, DPI/Narrador y UNC lenta o
  caída.
- **Aceptación de preview responsable (RP-4/RP-5):** instalación, RSS global y escenarios de
  escritorio y red. **RP-6/RP-7** (lectores e instrumentación) pendientes.
- **Visor STL/3MF:** aceptación manual del gesto de órbita.
- **Arrastre real desde la ventana de 7-Zip (E-2)** y WinRAR: archivo y carpeta, contenido y hash,
  conflicto y cancelación. La fixture existe (`scripts/prepare-e2-7zip-fixture.ps1`).
- **Aceptación visual pendiente:** selector de Espacios con rutas ausentes o remotas (E-4), orden de la toolbar (E-5) y toolbar adaptable en ancho
  compacto/amplio, con UIA/Narrador (E-6).
- **Ícono propio para Espacios:** hoy usa el de paneles clonados del set activo (E-6).

### Por confirmar con Nicolás

- **Menú de favoritos con ruta (E-3):** el histórico se contradice (una entrada dice que ya tuvo
  aceptación instalada y otra que sigue pendiente).
- **Plan de usabilidad 2026-09-04, etapas 0–7:** el histórico las deja sin marcar, casi todas con
  "validación instalada pendiente" (DPI, Narrador, red caída, cancelación de archivos grandes). No
  hay evidencia de su cierre ni de que sigan vigentes.
- **Traducción editorial** de las claves que se agregaron con respaldo en inglés en ocho idiomas
  (entregas de septiembre). El lote 0.5.7 tradujo sus claves nuevas en los 10 idiomas, pero no
  revisó las anteriores.
- Fuera de las secciones A, B, D y E, el histórico deja abiertos W-07, W-12 remoto, W-13–W-14
  ([plan](plans/2026-10-13-mejoras-y-ejecucion-luna.md)) y el port a Linux L-00–L-14
  ([plan](plans/2026-10-13-linux-ubuntu-luna.md)).

---

Histórico: [backlog hasta el 2026-11-24](evidencia/backlog-historico-2026.md).
