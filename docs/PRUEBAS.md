# Pruebas automáticas de Naygo

### Lectura aislada de texto/3D y temporales (RP-4B2)

- Protocolo de escenas: roundtrip, todos los cortes de tubería, tamaños desbordados,
  coordenadas no finitas e índices fuera de rango. Reservas antes de leer el cuerpo.
- Texto coloreado: Unicode, líneas vacías, tuberías truncadas, conteos excesivos y
  rechazo de segmentos que no coinciden con lo que se copia.
- EXE real: texto truncado, XML con regla de lenguaje explícita, STL/3MF, geometría y
  rotación sin archivo original. Cancelación y timeout de una lectura de texto bloqueada
  sobre una tubería efímera de prueba, creada por un helper oculto de Windows PowerShell.
- Las reservas temporales siguen ocupadas mientras termina el worker retirado. Contrato
  entre límite del hijo, mayor respuesta IPC, scratch de raster y margen por plaza.
- Evidencia de ejecución, pendientes y paquetes: [registro RP-4B2](VALIDACION-RP4B2-2026-09-14.md).

### Preview económico y retención (RP-5B)

- Compatibilidad del booleano aditivo en settings y roundtrip sin cambiar esquema/reglas.
- Calidad normal/económica, proporciones y composición GIF con descarte de presentaciones.
- Request real al EXE para PNG/SVG económicos y GIF animado (512 px).
- Cambio de modo cancela generación; no invalida al repetir el mismo valor.
- Admisión con overflow/dimensiones inválidas, reservas compartidas, liberación al ocultar
  y fallo terminal con reintento. VM comparte el handle Slint de imagen.
- La suite no sustituye prueba instalada de toggle, DPI/VM/red ni mide RSS global.

> Naygo — explorador de archivos para Windows.
> Copyright (c) 2026 Nicolás Groth / ISGroth. Licencia MIT.

Este documento explica cómo correr la suite de pruebas completa de Naygo y qué cubre.
El objetivo es no perder funcionalidad ya construida entre entregas: cada flujo crítico
del sistema tiene pruebas automáticas que se ejecutan con un solo comando.

Estado y última ejecución: [ESTADO-ACTUAL.md](ESTADO-ACTUAL.md). La existencia de tests
no significa cobertura exhaustiva: red real, DPI, accesibilidad y sesión instalada requieren
comprobación aparte. La matriz del [plan de rutas/preview](plans/2026-09-09-rutas-preview-y-configuracion.md)
conserva aceptación manual pendiente; no queda aprobada por pasar los tests de la base.
La hoja de ejecución para el release actual está en
[VALIDACION-WINDOWS-0.5.2-2026-10-15.md](VALIDACION-WINDOWS-0.5.2-2026-10-15.md). La actualización
local y la inspección UI Automation tienen resultados parciales documentados en
[VALIDACION-BUILD-2026-10-16.md](VALIDACION-BUILD-2026-10-16.md); la matriz funcional completa aún
requiere prueba en un entorno Windows aislado.

Para preparar las carpetas sintéticas de los casos manuales W-07/W-13, usar
`scripts/prepare-w13-filesystem-fixtures.ps1`. Por defecto crea listados de 10.000 y 100.000
archivos vacíos, además de fixtures pequeñas para copiar, mover, conflicto y nombre Unicode, bajo
`target/agent-out/`. La salida nunca sobrescribe una carpeta existente ni elimina datos. Para una
prueba pequeña del generador se puede indicar, por ejemplo, `-FileCounts 50`; la matriz manual
debe usar los tamaños completos y correrse contra la entrega verificada en una sesión aislada.

Para comprobar que la ventana instalada expone controles a UI Automation, ejecutar
`powershell -ExecutionPolicy Bypass -File scripts/inspect-naygo-uia.ps1`. Es una inspección
de solo lectura: informa conteos y tipos de controles, pero no registra etiquetas ni rutas.
Un árbol con descendientes es condición técnica necesaria; la aceptación de Narrador, foco,
lectura de controles y navegación por teclado sigue siendo manual dentro de W-13.

La barra de pestañas usa roles accesibles de lista/pestaña y admite recorrido por flechas más
activación con Enter/Espacio. La regresión headless `tab_bar_accessibility` verifica que las flechas
muevan el foco sin cambiar el panel, que Enter/Espacio activen la pestaña enfocada y que el límite
del grupo no permita desbordarse. No reemplaza la prueba manual con Narrador.

0.5.2 añade casos `nav052_*` del controlador para selección tras creación sin watcher,
retorno al padre, estabilidad tras insertar filas, evento durante listado, rescan y finalización
después de navegar. El pegado simultáneo comprueba ambos contenidos y notificaciones terminales.
`tests/navigation_controls.rs` renderiza el componente real de resaltado y prueba el TextEdit
al reabrir el modal; captura sintética en `target/agent-out/nav052-highlight.png`.
Los tests sólo crean/eliminan datos dentro de directorios temporales propios.
RP-1 añade pruebas del editor Slint real, validación y capacidad de workers: ver
[registro del 9 de septiembre](VALIDACION-2026-09-09.md).
RP-2 añade `callbacks_pathbar_tests.rs` (cableado real, destino capturado, cancelación,
error y cierre de panel, con configuración temporal aislada) y `tests/toolbar_path.rs`
(clics, teclado, acciones y render software a 260 px). No usan el portapapeles real ni
archivos personales. La imagen sintética se guarda en `target/agent-out/rp2-toolbar-260.png`.

RP-3 añade `core/src/theme/rp3_tests.rs` (seis paletas, contraste numérico de texto/énfasis,
compatibilidad y colores Unicode inválidos), una prueba de guardado/restauración independiente
en `config_ctrl.rs` y `tests/theme_render.rs`. Esta última renderiza quince temas con paneles,
ruta y botones Slint reales; además comprueba el catálogo único y los índices estables.
Capturas: `target/agent-out/rp3-themes/`. No certifican accesibilidad ni aceptación nativa
de toda Configuración. [Evidencia RP-3](VALIDACION-RP3-2026-09-09.md).

## Gate reproducible de una entrega

RP-5A añade `preview_animation_wire.rs`, `preview_animation_reader.rs` y
`preview_animation.rs`: formato acotado, composición de GIF/APNG/WebP sintéticos,
repeticiones/delays, cola/pausa, estáticos sin reloj, cancelación y generaciones.
`tests/preview_process.rs` también prueba animación en EXE real, pausas de 5,2 s y salida
sin lectores residuales; con `NAYGO_PREVIEW_SAMPLE` recorre 55 frames de la muestra privada.
`tests/preview_controls.rs` renderiza/pulsa Pausar/Reanudar/Repetir (capturas rp5-preview-*).
Los encoders `png`/`gif` se declaran sólo como dev-dependencies, en versiones ya fijadas.
La aceptación instalada y las métricas globales no quedan certificadas por esos casos.

RP-4A añade `preview_workers.rs`, `preview_lifecycle_tests.rs`, el caso de visibilidad
del controlador y `tests/preview_controls.rs`: permisos retenidos con receptor descartado,
A → B → A, desconexión terminal, cancelar durante debounce/capacidad/miniatura, ocultar y
restaurar, pestañas, panel maximizado, cierre, cámara coalescida y callbacks Slint reales.
El core añade cancelación de un triángulo sin modificar los buffers. Usan datos temporales
o canales controlados, no red ni documentos personales. No prueban terminación forzosa
de un decoder ni presupuesto global de memoria. [Registro RP-4A](VALIDACION-RP4A-2026-09-09.md).

Conservar `Cargo.lock` y registrar `rustc --version`. Ejecutar secuencialmente:

RP-4B1 añade protocolo truncado/inválido, procesos Windows reales para Cancelar/timeout con
stdin lleno, error/salida incompleta, límites CPU/memoria y terminación al cerrar el Job.
`tests/preview_process.rs` usa el EXE real sin UI, PNG/SVG/ZIP/corrupto temporales y comprueba
que un SVG no acceda a imágenes externas. `NAYGO_PREVIEW_SAMPLE` permite pasar una muestra
privada WebP 800 × 450 en lectura: no se copia ni se guarda su ruta en el resultado.
La prueba original sólo certificaba imagen estática. Medidas del proceso hijo
son memoria comprometida y tiempo de lectura, no consumo completo ni fluidez del explorador.
[Registro RP-4B1](VALIDACION-RP4B1-2026-09-09.md).

```powershell
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --no-fail-fast --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Comprobar también cada catálogo con `python scripts/check_i18n_lang.py <idioma>`.
No ejecutar pruebas destructivas sobre datos personales ni ocultar fallos del baseline
mediante nuevos `ignore`. No mezclar tests/Clippy con release LTO. Registrar logs, revisión,
códigos de salida, casos omitidos y condiciones de la máquina. Un timeout lógico no prueba
la terminación del worker: comprobar saturación, cancelación y limpieza explícitamente.

## Correr todo (un solo comando)

```powershell
scripts\test-all.ps1
```

El script ejecuta, en orden, las tres puertas de calidad que deben estar verdes antes de
cada commit, e imprime un resumen legible al final:

1. **Tests** — `cargo test --workspace --locked` (todos los tests de todos los crates).
2. **Clippy** — `cargo clippy --workspace --all-targets --locked -- -D warnings` (warnings = error).
3. **Formato** — `cargo fmt --all --check`.

Opciones:

| Opción       | Efecto                                                        |
|--------------|--------------------------------------------------------------|
| `-NoLint`    | Solo tests (salta clippy y fmt). Iteración rápida.           |
| `-FailFast`  | Se detiene en la primera puerta que falle.                  |

El script usa `CARGO_BUILD_JOBS=1` (la prioridad del proyecto es correr bien en equipos
modestos) y está escrito para PowerShell 5.1 (sin `&&` ni `||`). Devuelve código de salida
`0` si todo pasó y `1` si algo falló, así que sirve también para CI o para un hook de git.

### Solo los tests, a mano

```powershell
$env:CARGO_BUILD_JOBS = "1"; cargo test --workspace --locked
```

Para un crate puntual:

```powershell
cargo test -p naygo-core             # lógica pura
cargo test -p naygo-ui-slint         # controladores de UI (sin ventana)
cargo test -p naygo-core --test flujos_ops   # solo los flujos end-to-end de operaciones
```

## Smoke visual post-build (Windows interactivo)

La suite Rust prueba los controladores sin abrir una ventana. Antes de distribuir una versión,
en un escritorio Windows real y con **todas las instancias de Naygo cerradas**, correr:

```powershell
# Compilación normal de prueba:
cargo build -p naygo-ui-slint
scripts\smoke-ui.ps1

# O bien, contra el binario release recién generado:
scripts\smoke-ui.ps1 -Exe target\release\naygo.exe
```

El smoke ejecuta los flujos de editor de ruta y selección/scroll mediante clics y teclas reales.
Exige que la instancia abra una ventana, produce al menos seis capturas en
`target\ui-smoke\<fecha>/`, y falla si el log nuevo del ejecutable contiene un panic. Las capturas
son parte de la revisión humana: confirmar que el editor conserva el cursor tras `Shift+X` y que
la selección se mantiene visible tras el movimiento/filtrado.

No se agrega a GitHub Actions: los runners Windows alojados no tienen una sesión de escritorio
interactiva fiable para `SendKeys`, foco de ventana y captura de pantalla. En una sesión no
interactiva el script informa que fue omitido y termina correctamente; para un release este paso
se realiza manualmente en la máquina de empaquetado.

### Aceptación manual del drop desde 7-Zip (E-2)

Preparar una fixture inocua con `pwsh -File scripts/prepare-e2-7zip-fixture.ps1`. El script crea una
carpeta nueva bajo `target/agent-out/e2-7zip-fixtures/`, un ZIP con un OBJ pequeño y un TXT, un
destino vacío y un LEEME con la ruta y el SHA-256 esperado. Abrir el ZIP con 7-Zip y arrastrar
primero la carpeta `E2 fixture` al destino vacío de Naygo; luego repetir con `sample.obj` solo.
Confirmar que Naygo sigue respondiendo, que el historial registra la copia sin fallos y que el
archivo resultante coincide con el SHA-256 indicado. Repetir desde una build portable aislada si se
quiere probar otra versión. La fixture no contiene datos personales, nunca borra carpetas previas y
no sustituye la prueba real de archivo temporal `Temp\\7zE…` que el archivador entrega en el drop.

## Qué cubre la suite

Las pruebas se reparten en tres capas, igual que el código:

- **`naygo-core`** — lógica pura, 100 % testeable sin Windows ni UI. Es la mayor parte de
  la cobertura.
- **`naygo-platform`** — integración con Windows. El test de ida/vuelta a Papelera usa un archivo
  temporal único, confirma que `PostDeleteItem` reportó el elemento reciclado y lo restaura usando
  primero su nombre de parsing. La enumeración de la carpeta virtual queda como respaldo y no se
  inicializa para ese recibo; así puede ejecutarse en runners sin Explorer ni proveedor de
  enumeración funcional. El test integrado inyecta un enumerador fallido y exige cero llamadas al
  restaurar el recibo directo; además, un test auxiliar confirma que el caché de 1.024 recibos expulsa
  los más antiguos en orden FIFO. La cobertura también verifica que un lote parcial conserve recibos
  por origen, que un junction pueda reciclarse/restaurarse sin modificar su destino y que el restore
  rechace una entrada reparse colgante como destino ocupado sin enumerar la Papelera. Sólo
  `NotFound` se interpreta como ruta ausente; errores de metadata como `PermissionDenied` fallan de
  forma segura y no disparan el fallback. Otros tests que necesitan recursos reales del sistema
  operativo (discos, Shell interactivo) se marcan `#[ignore]` y se corren a mano cuando corresponde.
- **`naygo-ui-slint`** — los **controladores** de la UI (`workspace_ctrl`, `ops_ctrl`,
  `config_ctrl`, `bridge`, `preview`), ejercitados **sin abrir una ventana real**. Lo
  visual tiene además render por software y gestos de puntero headless en
  `crates/ui-slint/tests/controls_render.rs` y `layout_render.rs`. Esto no sustituye
  probar ventanas nativas, teclado/AltGr, DPI, Narrador y drag & drop externo.

### Subsistemas con pruebas

| Subsistema | Qué se prueba |
|---|---|
| **Operaciones de archivo** | Copiar/mover/eliminar (permanente)/renombrar/crear, de punta a punta, verificando el estado **real en disco**. Plan + ejecución por el motor síncrono (`run_plan`) y por el motor en hilo (`spawn`). |
| **Conflictos** | Sobrescribir, saltar, renombrar (sufijo `(N)`), renombrar con nombre elegido (`RenameTo`), aplicar-a-todos. Conflicto de **carpeta** (fusionar/reemplazar/saltar/cancelar). |
| **Seguridad del motor** | Nunca borra un origen al "Reemplazar" (guardas cinturón-y-tirantes en `folder_conflicts` y en `run_plan`). El `pre_delete` jamás toca un origen ni un ancestro de origen. |
| **Cancelación** | Cancelar antes de empezar, durante el escaneo del plan y durante la espera. Una copia cancelada no deja basura. |
| **Batch-rename** | Plan ordenado por dependencia (corrimientos en cadena), rechazo de ciclos puros `a↔b`, ejecución real en disco. |
| **Deshacer** | Round-trip ejecutado: mover/renombrar → construir el inverso → re-emitir → verificar que el disco vuelve a su estado previo. Borrar no es deshacible (contrato v1). |
| **Drag & drop (lógica)** | `drop_at` enruta al panel bajo el cursor (no al activo); `move_hint` del OLE fuerza mover; mismo disco sin modificadores mueve por defecto; rubber-band; `same_drive` con rutas mixtas/UNC. En platform, los descriptores OLE virtuales validan límites/rutas y conservan solo sus raíces superiores. |
| **Listado y vista** | Streaming, atributos ocultos/sistema, `is_visible` (filtro ocultos/sistema/dotfiles), `compute_view_indices` (filtro + orden + selección alineada), vista profunda recursiva, tamaño de carpeta, búsqueda recursiva. |
| **Navegación** | Historial atrás/adelante, favoritos (árbol de grupos anidados + migración del formato plano viejo), recientes, caché de carpetas. |
| **Config / Settings** | Round-trip, migraciones, guardados ordenados, fuentes por función, packs, nueve temas de fábrica, tema de usuario, `theme_slug`, `is_builtin_id`, paridad de diez idiomas. |
| **Selección** | `select_single`, `select_range`, `select_rect_range`, modificadores; archivos nuevos seleccionados al llegar. |
| **Estado de UI (controlador)** | `any_modal_open`, `pending_dialog`, cancelar conflicto, máquina de estados de la operación (Planning → conflicto → copia → fin), id estable de op tras reordenar la cola. |

### Dónde viven los tests

- La mayoría son módulos `#[cfg(test)]` **dentro** del archivo de producción que prueban
  (patrón estándar del repo). Por ejemplo, los tests del motor están en
  `crates/core/src/ops/engine.rs`, los del controlador de operaciones en
  `crates/ui-slint/src/ops_ctrl/`.
- Los **flujos compuestos end-to-end** de operaciones de archivo viven aparte, en
  `crates/core/tests/flujos_ops.rs` (integration test): arman el `OpRequest`, lo ejecutan
  por el motor y verifican el estado final en disco con carpetas temporales (`tempfile`).
  Es el lugar al que mirar para entender, de un vistazo, qué flujos de archivo están
  garantizados.

## Antes de cada commit

La regla del proyecto es: build limpio + tests pasando + clippy sin warnings + formato OK.
`scripts\test-all.ps1` verifica las tres cosas de una sola vez.
