# Validación L-07 — Papelera FreeDesktop

Fecha: 2026-10-19. Estado: backend inicial implementado; falta ejecución nativa Ubuntu y aceptación.

## Cambio

- Linux crea pares `Trash/files` y `Trash/info` según FreeDesktop, guarda `Path` como bytes con
  percent-encoding y `DeletionDate` local.
- Usa la Papelera del filesystem de origen. En el filesystem de datos del usuario emplea
  `$XDG_DATA_HOME/Trash`; en otros, valida `.Trash` y su sticky bit antes de crear `.Trash/$uid`, o
  usa `.Trash-$uid`. Si el destino no es seguro o no está disponible, reporta error y no borra ni
  copia el original a otro volumen.
- El traslado se publica con `renameat2(RENAME_NOREPLACE)`; la restauración usa el mismo guard y
  respeta destinos ya ocupados. Los recibos conservan el path concreto reciclado en el historial
  persistente de Deshacer. Lectura de historial anterior sin ese campo sigue siendo válida.
- La ruta de entrada se hace absoluta resolviendo el padre con el filesystem y conservando el
  último componente. Esto respeta `symlink/..` y evita seguir el symlink final.
- Lotes verifican cancelación entre elementos, retornan recibos/fallos asociados al índice y
  preservan lo completado. Reciclar una symlink opera sobre el enlace, no sobre su destino.
- Al restaurar se validan las carpetas privadas y el `trashinfo` sin symlink, y se verifica que la
  ruta guardada corresponda al recibo antes de mover el elemento.

## Verificación local

- `scripts/test-all.ps1`: **1.478 tests aprobados, 8 ignorados**, Clippy sin warnings. El primer
  cierre marcó solamente formato; se corrigió y `cargo fmt --all` terminó correctamente.
- `cargo check --workspace --tests --locked --offline --target x86_64-unknown-linux-gnu --jobs 1`:
  correcto en 2m29s. Solo aparece el aviso esperado del icono `.exe` ausente en el host Windows.
- Clippy cruzado con `--all-targets -D warnings` pasó por separado para `naygo-core`,
  `naygo-platform` y `naygo-ui-slint`.
- El primer empaquetado se detuvo preventivamente al registrar 7,87 GiB libres, bajo el umbral de
  8 GiB (`guarded-20261019-125429-629.*.log`). Tras recuperar margen, el segundo
  `scripts/build-release-guarded.ps1` terminó: Release 42m14s, Inno Setup 159,329 s, pico del Job
  10,65 GiB bajo el límite de 12 GiB (`guarded-20261019-133153-611.*.log`).
- Distribución 0.5.3 verificada, sin instalación/publicación: portable SHA-256
  `4223bb3317c8988ca1fa180dd1ba722fee19043b23477e609f95485bd9199e32`; setup
  `b9be1868c7e0e2fd075cbc26deef6ee3ac244fffddb4ae2520a782decdec7ea8`; EXE
  `56876a94cef124c913366e0beb543d9768ad66acfd76b338a29ba2aa307ab8f1`. Manifiesto contrastado;
  el EXE dentro del ZIP coincide con Release. El ZIP tiene cuatro entradas y excluye el PDB. EXE y
  setup siguen sin Authenticode.
- El gate Windows incluye el test de compatibilidad serde: historial antiguo se deserializa con
  `trashed_path = None`; los recibos nuevos conservan su path al serializar/deserializar.
- Los tests Linux añadidos cubren dos nombres homónimos, restauración sin sobrescritura, symlink
  que conserva destino, ruta `symlink/..`, cancelación parcial, fallback ante `.Trash` symlink,
  percent-encoding no UTF-8 y lectura de `.trashinfo`. Se compilaron para
  `x86_64-unknown-linux-gnu`; no se ejecutaron porque el host es Windows y WSL no inicia.

## Pendiente antes del cierre

Ejecutar los tests nativos y los casos de permisos, fallos parciales, cancelación y montajes en
Ubuntu 26.04 con filesystem temporal; comprobar la interacción Undo y que el entorno desktop vea los
elementos en su Papelera. La fuente no se presenta todavía como aceptación Linux completa.

Especificaciones primarias: [FreeDesktop Trash](https://specifications.freedesktop.org/trash/latest/)
y [XDG Base Directory](https://specifications.freedesktop.org/basedir/latest/).
