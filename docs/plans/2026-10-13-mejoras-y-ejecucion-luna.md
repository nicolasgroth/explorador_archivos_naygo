# Naygo — plan de mejoras para ejecución con GPT-6 Luna

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-20. Estado: **W-00–W-06E, M-01, W-08–W-09, W-10A–C y W-11A–C implementados, con gate completo y distribución Windows 0.5.3 regenerada bajo supervisor. W-12 workflows y restore de Papelera probados localmente; acción-pins, pruebas de firma sintética y `actionlint` v1.7.12 revalidados; ejecución remota pendiente. W-13 amplió nombres y acciones accesibles a controles Slint; Narrador/diálogos siguen pendientes. La build W-06B más reciente quedó instalada y abierta. W-07 ya tiene una comparación local adicional de dos binarios bajo carga muestreada: medianas 261,693 ms (anterior) y 295,669 ms (actual) a 100k, diferencia +13,0 %, bajo el umbral orientativo de 15 %; no se atribuye a un cambio de código. Aún faltan interacción real, presentación y matriz local/UNC.**
Nicolás autorizó avanzar con todo lo que pueda resolverse sin su validación y pidió avisar si
aparecen hallazgos nuevos para decidir su incorporación. Autorizó actualizar la instalación local
y dejar el build final corriendo; publicación y firma siguen pendientes de sus requisitos externos.

## Objetivo y base

Cerrar los hallazgos de la revisión de octubre y completar RP-6/aceptación pendiente:
errores de listado, I/O en UI, resaltado costoso, retención de snapshots, concurrencia,
presupuesto por tick, lectores y mediciones. Preparar después el port Linux descrito en
[su plan](2026-10-13-linux-ubuntu-luna.md), conservando las tres capas y la UI Slint.

La última distribución documentada es 0.5.2+build.202609142135. Las 1.324 pruebas y
Clippy aprobados son evidencia de septiembre; **no se ejecutaron nuevamente en esta revisión**.
Antes de escribir estos planes había 74 archivos modificados y 49 entradas sin seguimiento
en `git status`; HEAD era `abdef402`. Esa situación exige inventario y respaldo verificable,
no reset, limpieza de temporales ni una reimplementación de lo ya entregado.

Lectura: `AGENTS.md` → `docs/BACKLOG.md` → `docs/ESTADO-ACTUAL.md` →
`docs/README.md` → bloque concreto. El plan de septiembre continúa siendo el contrato
de rutas/preview; este documento divide su continuación y los hallazgos nuevos en entregas.
No incorpora reproductor de audio/vídeo, editor general ni servicios de indexación.

## Método de trabajo para Luna

El destinatario es **GPT-6 Luna** (`gpt-6-luna`). OpenAI lo describe para tareas enfocadas;
dividir el trabajo en bloques verificables es una decisión de este plan, no una garantía
de resultados del modelo. [Referencia oficial consultada el 13 de octubre](https://developers.openai.com/api/docs/models/gpt-6-luna).

1. Ejecutar un ID por sesión de trabajo. Leer sus archivos y sus contratos antes de editar.
2. Usar `graphify query` primero; si el grafo contradice el código, manda el código.
   El grafo actual advierte IDs antiguos con posibles colisiones: una consulta no certifica exhaustividad.
3. Reproducir el problema con datos sintéticos o dobles de I/O antes del fix. No usar originales
   personales ni la configuración instalada. Los fallos se prueban con barreras/canales, no sleeps frágiles.
4. Separar lógica pura de coordinación UI. Ningún `Rc`, `RefCell`, handle Slint ni préstamo
   del controlador cruza al worker; capturar datos propios y aplicar resultados en el event loop.
5. Todo resultado lleva revisión/identidad del pedido. Descartar respuestas obsoletas también
   en A → B → A; comparar sólo la ruta no basta. No hacer `join`, I/O o espera de mutex de escritura en UI.
6. No aumentar memoria, actualizar dependencias o cambiar esquema/configuración para hacer pasar un test.
   Si resulta necesario, presentar evidencia y una decisión acotada a Nicolás.
7. No crear una abstracción universal de jobs para resolver el primer fix. Extraer módulos
   por responsabilidad al crecer; preservar los permisos RAII existentes del preview.
8. Mantener una bitácora por ID con archivos, pruebas ejecutadas/salidas, límites, build/hashes
   y aceptación pendiente. No contar una prueba ignorada o una medición histórica como nueva.
9. Trabajar secuencialmente; este plan no requiere subagentes. No lanzar compilaciones simultáneas.
10. Ante un fallo de contrato, API inexistente, dato faltante o decisión de alcance, detener sólo
    el bloque dependiente, registrar el problema y consultar. No cerrar el ítem con un fallback ficticio.

Sugerencia operativa: razonamiento `high` para implementación ordinaria y `xhigh` para
concurrencia/identidad/operaciones. Es una preferencia propuesta; no cambia el modelo de este chat.
No se necesitan claves API ni crear nuevos chats para ejecutar el plan en Codex.

## Dependencias y entregas

| Estado | ID | Entrega | Depende de | Tamaño orientativo |
| --- | --- | --- | --- | --- |
| [x] | W-00 | Inventario, respaldo y documentación vigente | — | Pequeño |
| [x] | W-01 | Resultado tipado de listado y snapshot válido | W-00 | Medio |
| [x] | W-02 | Resaltado por identidad con búsquedas directas | W-01 | Pequeño |
| [x] | W-03 | Presupuesto de snapshots compactos | W-01, W-02 | Medio |
| [x] | W-04 | Preconsulta de conflictos fuera de UI | W-01 | Medio |
| [x] | W-05 | Guardado de ajustes con worker único | W-00 | Medio |
| [x] | W-06A | Temas e íconos externos fuera del render | W-05 | Medio |
| [x] | W-06B | Packs y resto de persistencia fuera de callbacks | W-05, W-06A | Medio |
| [x] | W-06C | Carga inicial asíncrona de configuración y workspace | W-06A/B | Medio |
| [x] | W-06D | Enumeración de unidades/USB y expulsión segura fuera de UI | Gate completo, release 0.5.3 regenerado e instalado; smoke responde 2026-10-19 |
| [x] | W-06E | Guardado/borrado de temas personalizados fuera de callbacks, con escritura atómica | Gate y distribución 0.5.3 regenerada; aceptación visual manual pendiente |
| [ ] | W-07 | Métricas acotadas y línea base | W-00; repetir tras W-01–06 | Medio |
| [x] | M-01 | Limpiar avisos Clippy existentes | Hallazgos del gate W-06C | Pequeño |
| [x] | W-08 | Recogida por lotes y canal acotado | W-01, W-07 | Medio |
| [x] | W-09 | Admisión de workers y probes de carpetas | W-08 | Medio |
| [x] | W-10A | Índices de vista compartidos | W-02, W-07 | Medio |
| [x] | W-10B | Actualización proporcional de modelos | W-08, W-10A | Comparación semántica y actualización selectiva por delta `Modified`; interacción real queda en W-07/W-13 |
| [x] | W-10C | Append incremental en el modelo Slint | W-10B | Gate, benchmark release y distribución 0.5.2 verificados; interacción queda en W-07/W-13 |
| [x] | W-11A | Lector PDF con trabajo reducido | W-07 | Medio |
| [x] | W-11B | Límites de recorrido ZIP/TAR/TAR.GZ | W-07 | Medio |
| [x] | W-11C | Límites de texto/SVG/imágenes auditados y casos adversos | W-07, W-11A/B | Medio |
| [x] | W-12 | CI reproducible y comparación de rendimiento | W-07, W-08–11 | Medio |
| [ ] | W-13 | Aceptación instalada y cierre de Windows | W-01–12 | Manual + entrega |
| [ ] | W-14 | Firma de código | SignPath Foundation, aprobación/configuración externa | Externo |

Nota de ejecución W-13 (2026-10-16): la UIA del build W-10C instalado sólo exponía la ventana raíz.
Se habilitó la feature `accessibility` de Slint (AccessKit) y se añadió
`scripts/inspect-naygo-uia.ps1`, que verifica el árbol Control View sin registrar nombres ni rutas.
El build final con AccessKit se regeneró, instaló y quedó ejecutándose; el inspector encontró 318
descendientes (224 con nombre y 46 enfocables). La captura pasiva de la instalación quedó registrada
en `docs/BENCHMARKS.md`. La revisión encontró que la barra custom de pestañas no declaraba roles ni
soportaba recorrido de foco: se separó a `ui/tab-bar.slint`, se agregaron roles/estados/acciones
AccessKit y navegación ←/→ + Enter/Espacio. La prueba headless `tab_bar_accessibility` y el gate
completo pasan; la distribución se regeneró y se instaló. Tras la regeneración L-01A, el EXE instalado
coincide con el hash del release (`bfb03176daa0dedf9312d42178ccfa57f1bf96d4437889aad5e4a3ead5bd48ef`),
la app responde y los cuatro JSON de usuario y el PDB antiguo conservan sus hashes. En la sesión
actual UIA no encontró roles de pestaña porque el workspace no tiene grupos apilados; validar Narrador,
foco y teclado manualmente. El inspector y los tests no cierran W-13.

Seguimiento W-13 (2026-10-16): el probe UIA temporal con el componente real expuso tres pestañas
enfocables y selección correcta. Un intento adicional de clic/teclado no fue concluyente: el clic
seleccionó Panel dos, pero la consulta tras Enter no encontró selección. Se repitió la prueba dirigida
headless (1/1 pasó); la interacción de Windows y Narrador sigue pendiente.
Una repetición posterior no inyectó teclas porque Windows no permitió activar la ventana temporal;
la guarda evitó enviar entrada a otra aplicación. Esto no demuestra un defecto de la barra.
El 16-10 se volvió a intentar observar el probe aislado de tres pestañas con Computer Use; la captura
del frame agotó el tiempo dos veces, no se enviaron teclas y se cerró sólo el proceso temporal.

Repetición W-07 (2026-10-16): la instalación actual se observó en 3×30 s pasivos, con 16 muestras
cada 2 s. Un proceso, sin SMB; working set 60,9 MiB, privado 29,2 MiB, 385 handles, 24 hilos y
CPU 0,047–0,141 s por ventana. No confirma reposo/latencia y no cierra W-07. JSON registrado en
`docs/BENCHMARKS.md`.
Después de instalar el EXE L-01A (PID 44816), otra captura 3×30 s registró 62,8 MiB de working set,
31,1 MiB privados, 394 handles, 27 hilos y CPU 0,359/0,110/0,047 s; cero SMB. La primera ventana
empezó 36 s después del arranque, por lo que no es reposo controlado ni cierra W-07. JSON en
`target/agent-out/w07-installed-latest-20261016.json`.
Luego el portable actual pasó smoke aislado de 10 s con métricas opt-in: 16 resúmenes internos de
listados/modelo/tick y contadores de workers; no hubo `ui.input.*`. Cerró con `--shutdown`, se
restauró la app instalada y sus cuatro JSON/PDB quedaron intactos. Las cifras de arranque no son una
medición de input real, renderer Winit ni preview; W-07 sigue pendiente. Detalle en
`docs/BENCHMARKS.md`.

Orden recomendado: W-00 → W-01 → W-02 → W-03 → W-04 → W-05 → W-06A/B →
W-06C → W-06D → W-06E → W-07 → W-08 → W-09 → W-10A/B/C → W-11A/B/C → W-12 → W-13.
M-01 se puede ejecutar después de W-07 y antes de W-08 sin cambiar producto.
W-07 puede recoger una línea base antes de W-01 si su preparación se autoriza como bloque propio.
El port Linux empieza por L-00/L-01 con una base Windows trazable; no desarrollar dos
implementaciones divergentes de los listados o lectores.

Los tamaños indican complejidad relativa, no días ni estimaciones de tokens.

## W-00 — preparar una base recuperable

**Estado: completado el 2026-10-13.** Rama `codex/rutas-preview-051`, HEAD
`abdef402babd2c136744c67ee9833c2f67bf0f16`; 74 modificados y 49 nuevos en el inventario
de partida. Respaldo recuperable de 983 archivos en
`target/agent-out/baseline-w00-20261013.zip`, SHA-256
`8aeed6dfe843a97345885d50b939e2be3d645c15a1f0d4cc92c43ca2c649e2a6`; manifiesto con
toolchain, versiones y hashes de `dist/` en `target/agent-out/w00-baseline-20261013.txt`.
Los artefactos de septiembre coinciden con ambos hashes de `dist/SHA256SUMS.txt`.

Se actualizó la situación vigente del backlog y el estado; RP-4B2/RP-5B aparecen entregados,
la animación RP-5A figura disponible y RP-6/RSS global/aceptación instalada continúan pendientes.
Sólo documentación cambió en este bloque. El snapshot conserva los archivos versionables y
deja fuera configuración, muestras personales y salidas de build/distribución.

**Archivos:** `docs/BACKLOG.md`, `ESTADO-ACTUAL.md`, `README.md`, `ARQUITECTURA.md`,
planes/validaciones vigentes, `Cargo.toml`, `Cargo.lock`, `.gitignore`.

- Inventariar diff y archivos nuevos; separar producto, docs y artefactos privados/ignorados.
  Comprobar exclusiones de `tmp/`, configuraciones y corpus personales antes de preparar respaldo.
- Registrar rama, HEAD, toolchain, dependencias fijadas y hashes de la distribución disponible.
  Preparar respaldo local recuperable; un commit sólo después de gates limpios. No publicar a GitHub
  sin autorización expresa ni agregar indiscriminadamente todo el directorio.
- Consolidar documentación: la cabecera actual manda sobre antecedentes. Corregir checklist
  RP-4/RP-5 que aún agrupa texto/3D/modo económico como pendientes y el párrafo de imagen estática
  que describe una base anterior. Conservar las validaciones históricas y sus hashes.
- No retirar `target/` ni cambiar la toolchain como parte de esta consolidación.

**Cierre:** restaurabilidad comprobada, inventario explícito, enlaces coherentes y pendientes
reales identificados. No requiere EXE si sólo cambia documentación.

## W-01 — errores, cancelación y snapshots

**Estado de ejecución (2026-10-13):** código y pruebas dirigidas aprobados; tres intentos se
detuvieron cuando el margen cayó bajo 8 GiB y un cuarto terminó correctamente después de liberar
memoria. Los hashes nuevos están en
[`VALIDACION-BUILD-2026-10-13.md`](../VALIDACION-BUILD-2026-10-13.md). Se conserva el último snapshot completo; el listado parcial se
muestra con aviso y no alimenta comparaciones ni recientes. Error de lectura, cancelación y
desconexión terminan como estados distintos. El probe considera ausente sólo `NotFound` y
`NotADirectory`; ante permiso denegado u otro fallo conserva su diagnóstico previo.

**Archivos:** `core/src/listing.rs`, `ui-slint/src/listing.rs`,
`workspace_ctrl/listing.rs`, `workspace_ctrl/tests*.rs`, `tick.rs`, catálogos/guía si cambia el aviso.

- Sustituir la tupla de `poll()` por un resultado con entries y terminal explícito:
  éxito, cancelación, error tipado y desconexión inesperada. La propuesta exacta se decide al leer
  los consumidores de Files y árbol; no introducir Slint en core.
- Preservar causa de I/O y cobertura parcial. Un error no equivale a carpeta vacía ni inexistente.
  Mantener generación/panel/carpeta del resultado y ofrecer reintento explícito.
- Actualizar `listing_snapshots`, recientes y comparación sólo tras éxito completo. Una
  cancelación o error conserva el snapshot anterior. Definir presentación parcial sin anunciar
  archivos desaparecidos ni conservar filas viejas como si pertenecieran a otra carpeta.
- Atender `TryRecvError::Disconnected`: terminar el estado para que el timer no quede vivo
  eternamente si el worker sale sin mensaje final. Aviso discreto, sin reintentos infinitos.
- Revisar `refresh_missing_cache`: permiso denegado/red inaccesible no deben diagnosticarse
  automáticamente como carpeta inexistente.

**Pruebas:** vacío exitoso; refresco fallido después de éxito; error tras algunas entradas;
cancelación a mitad; desconexión sin terminal; A → B → A; panel cerrado; evento watcher
durante error/éxito; reciente sólo en éxito. Fixtures deterministas para permisos/red.
**Cierre:** el snapshot válido permanece intacto y no hay falsos `-n` tras fallar/cancelar.

## W-02 — resaltado lineal

**Estado de ejecución (2026-10-13):** índice por ruta completa implementado en el controlador;
la invalidación al navegar evita que un cambio de una visita anterior se aplique a un archivo
homónimo. Pruebas del core/controlador, benchmark y nueva distribución aprobados.

**Archivos:** `core/src/listing_changes.rs`, `workspace_ctrl/listing.rs` y estado asociado.

- Construir un conjunto de rutas nuevas/modificadas cuando cambia la comparación; excluir Missing.
- Aplicar resaltado por `Entry.path`, no `file_name()` ni búsqueda de todos los cambios por fila.
- Invalidar por revisión del snapshot y al navegar. No crear el conjunto en cada tick.

**Pruebas:** cambios vacíos, todos modificados, ausentes, nombres repetidos en vistas multirraíz
si ese camino comparte presentación, caracteres Unicode y filtros/orden. Benchmark sintético
10k/100k con muchos cambios; documentar tamaños y comparar misma máquina.
**Cierre:** costo O(entries + cambios) de esta fase y mismos estados visibles.

## W-03 — retención de snapshots

**Estado de ejecución (2026-10-13):** almacén LRU compacto implementado en core. El valor
provisional es 32 MiB estimados / 100.000 entradas; cuatro pruebas dirigidas y la distribución
pasaron. La medición W-07 debe confirmar o ajustar esos límites antes de tratarlos como definitivos.

**Archivos:** módulo nuevo de core para snapshots/presupuesto, `listing_changes.rs`,
`workspace_ctrl/mod.rs`, `workspace_ctrl/listing.rs`, `docs/BENCHMARKS.md` y guía.

- Conservar únicamente ruta/identidad, tipo, tamaño y mtime necesarios para comparar.
  No guardar `Vec<Entry>` completo con campos redundantes, íconos o modelos Slint.
- Presupuesto compartido entre paneles por bytes estimados comprobados, más límite de entradas.
  LRU determinista; actualizar sólo al éxito. Expulsar antes de admitir; un snapshot demasiado
  grande no se retiene. Ninguna expulsión genera archivos Missing.
- Conservar comparación en sesión cuando haya base disponible; si fue expulsada, tratar la
  siguiente visita como primera comparación. Documentar este trade-off.
- Elegir el presupuesto con W-07/datasets; hasta medir, registrar el valor como provisional.
  No persistir snapshots ni aumentar esquema para introducir controles innecesarios.

**Pruebas:** límite exacto, overflow, reemplazo de misma carpeta, expulsión LRU, dos paneles
en misma carpeta, snapshot enorme, fallo sin sobrescritura. Medir muchas visitas y pico de
memoria; el contador de retención no es RSS global.

## W-04 — conflictos sin consultas en UI

**Archivos:** `ops_ctrl/mod.rs` (`first_collision`, `begin_conflict_rename`),
`workspace_ctrl/ops.rs`, `callbacks_ops.rs`, contratos de conflicto en core.

- Mantener la decisión aprobada de un único modal coherente al soltar: preconsulta async
  antes de elegir confirmación genérica/conflicto. Capturar fuentes/destino/modificadores
  y conservar staging hasta terminal; no arrancar la operación mientras se consulta.
- Sugerencia de nombre en worker o candidato sintáctico revalidado por motor. No usar
  `exists()`/`dedup_name` con I/O desde callback.
- Token, revisión y plazas acotadas; cancelar/otro drop/cierre descartan respuesta vieja.
  La preconsulta no garantiza que el destino no cambie: el motor revalida al ejecutar.
- Distinguir ocupado, libre y error de consulta; un fallo de acceso no significa destino libre.

**Estado:** implementado. La sugerencia inicial del modal `ConflictRename` usa un candidato
sintáctico `(2)` y no toca el filesystem en UI; el motor la deduplica/revalida en su worker. El
drop comprueba colisiones en un worker con una plaza acotada antes de elegir el diálogo genérico o
el específico; errores de acceso abortan el drop en vez de asumir que está libre. El staging se
conserva hasta el resultado y una carrera se vuelve a verificar en el motor. Copy/Move ya no usa
`exists()` desde el controlador. La suite completa UI (354 tests) y la distribución final pasaron;
la aceptación instalada queda pendiente en W-13.

**Pruebas:** destino lento, cancelar esperando, dos drops, staging virtual, conflicto nuevo
después del precheck, nombre ocupado entre sugerencia/confirmación y tecla Ctrl/Shift capturada.

## W-05 — persistencia ordenada y acotada

**Archivos:** `core/config/settings_writer.rs`, `core/config/mod.rs`, `config_ctrl.rs`,
`callbacks_config.rs`, `workspace_ctrl/session.rs` y guardados de preferencias relacionados.

- Worker único por ámbito de persistencia con última revisión pendiente coalescida. Reservar
  orden en memoria antes de encolar. No sustituir cada `save()` por un `thread::spawn` ilimitado.
- Mantener escritura atómica y compatibilidad de esquema 4; devolver estado/error de guardado
  para un aviso discreto. Aplicar el ajuste visual inmediatamente en memoria.
- Política de cierre explícita: finalizar el guardado final desde coordinación fuera del callback
  y con plazo, sin `join` en UI; revisar el watchdog de salida para no perder el último ajuste.
- Auditar keymap, layout, favoritos y recientes: listar qué rutas siguen síncronas y dividirlas
  en W-06B si son otro formato. No permitir un guardado viejo después del más nuevo.

**Pruebas:** writer bloqueado + ráfaga de cambios, última revisión persistida, disco lleno,
permiso denegado, cierre con pendiente, atomicidad y downgrade. Ningún test modifica ajustes reales.

**Estado de ejecución (2026-10-13):** ajustes, keymap y workspace encolan JSON en un worker
compartido; queda una revisión pendiente por archivo y el orden se reserva antes de encolar. Las
escrituras siguen siendo atómicas. ConfigCtrl espera tickets fuera de UI al cerrar; el plazo total
es 3 s y continúa el cierre ante fallo/timeout, que queda en el log. `maybe_persist_session` ya no
escribe desde tick. Tres pruebas core del writer y los 354 tests UI pasaron. La distribución se
regeneró y verificó; la aceptación instalada queda en W-13.

## W-06A/B — resto del I/O de interfaz

**W-06A, archivos:** `icons.rs`, `core/icons/`, `core/theme/mod.rs`, `core/icon_set.rs`,
`config_ctrl.rs`, `workspace_ctrl/mod.rs`, `main.rs`, `packs.rs`, `callbacks_config.rs`,
`vm_builders.rs`, `theme_apply.rs`. Cargar PNG externos/temas en workers acotados; durante carga
usar asset embebido. Cachear por revisión de set/override/tinte; rechazar respuesta vieja tras
cambiar tema. Validar tamaño/dimensiones antes de decodificar; sin precarga global. Guardar/eliminar
temas también en worker, con catálogo actualizado sólo al resultado correcto. `ThemeCatalog::embedded`
y `IconSetCatalog::embedded` permiten que el constructor de producción no explore los catálogos
del usuario; `PackIo` los carga en el fondo y el hilo UI aplica la respuesta. La primera pintura
puede mostrar brevemente el tema y los íconos de fábrica hasta que termine esa carga.
**Pruebas:** PNG corrupto/enorme, fuente lenta, cambio de set durante lectura, tinte nuevo,
archivo desaparecido, cancelación, persistencia fallida y fallback visible.

**Cierre W-06A:** PNG externo asíncrono con límite de 8 MiB codificados, 1024×1024 y 4 MiB
decodificados, fallback embebido y protección por generación. Catálogos externos de tema/set,
guardado y borrado de temas se procesan mediante worker serial acotado. `load_settings_flagged_deferred_icon_set`
preserva un set propio sin escanear `icons/` en UI; el handler resuelve el ID al recibir el catálogo.
La prueba nueva verifica que se descubren ambos catálogos desde el worker y se despierta la UI.

**W-06B, archivos:** `packs.rs`, `callbacks_config.rs`, `icon_pack`, callbacks de exportación,
`logging.rs`, `workspace_ctrl/ops.rs` (`op_paste`) y `workspace_ctrl/listing.rs`
(`missing_folder_retry` / `missing_folder_go_ancestor`) y stores/persistencias inventariados.
`op_paste`, los dos controles de carpeta perdida y la revisión de persistencia incremental de
workspace se agregaron tras revisión y aprobación de Nicolás. El guardado desde
`maybe_persist_session()` ya se corrigió en W-05; W-06B debe comprobar si quedan otras escrituras
de formato fuera del worker. Mantener selector nativo en UI; mover lectura/compresión/escritura/recarga
a workers. Snapshot de datos antes del selector; no mantener
préstamos `RefCell` durante bucles modales. Cancelación en recorridos y commits de importación
explícitos; no dejar configuración parcialmente aplicada sin informarlo/recuperarlo.
**Pruebas:** además de persistencia/importación, simular consulta de colisión lenta o fallida al
pegar, carpeta perdida que reaparece o desaparece durante el reintento, ancestro inaccesible,
cancelación y navegación/cambio de generación mientras cada consulta está pendiente; escrituras de
workspace coalescidas, lentas o fallidas, cierre con ticket pendiente y confirmación de que el tick
no espera I/O. Confirmar que la UI no bloquea y que un resultado viejo no reemplaza el panel actual.

Auditar logging llamado en navegación: `breadcrumb` ya es circular en memoria, conservarlo;
no atribuirle I/O. Enviar las escrituras ordinarias de log a cola acotada si aparecen en UI;
no perder el diagnóstico de panic ni bloquear la navegación por métricas saturadas.
**Cierre W-06B:** importación/exportación de packs, copias de PNG del usuario, actualización de
temas y logging ordinario en worker acotado. La comprobación de colisiones en `op_paste` y las
consultas de reintento/ancestro de carpeta perdida también salen del callback. El log de panic
conserva una ruta síncrona de diagnóstico. Suites ejecutadas: 359 UI, 891 core y 22 flujos de
operaciones; el build 0.5.2 supervisado y verificado está en `VALIDACION-BUILD-2026-10-13.md`.
No afirmar cobertura universal sólo por un `rg` limpio. Refinamiento 2026-10-20: la lectura del
portapapeles externo también se movió dentro del worker de `op_paste`, junto con la decisión y la
consulta de colisiones; un test comprueba que la lectura se ejecuta en un hilo distinto al llamador.
El gate actual pasó 1.498 pruebas, 8 ignoradas, Clippy y formato; ver
`../VALIDACION-W06B-PASTE-CLIPBOARD-2026-10-20.md`.
Refinamiento adicional 2026-10-20: se encontró que Copiar/Cortar, pegar en la Cesta y las acciones
de copiar ruta/texto/CSV/procedencia aún llamaban al clipboard del SO desde callbacks. Todas las
lecturas y escrituras ahora se serializan en un worker único con cola acotada; el pegado de archivos
usa el mismo servicio, por lo que un pegar inmediato no adelanta una copia pendiente. Los indicadores
locales de corte e historial se actualizan sólo tras éxito; los errores tienen toast. La prueba
focalizada (2/2), gate (1.500 aprobadas, 8 ignoradas), Clippy, formato y check UI cruzado Linux pasan.
La nueva distribución Windows 0.5.3 se regeneró bajo supervisor (Release 46m08s, Inno 176,562 s),
verificada y no instalada; runtime/Ubuntu nativo sigue pendiente. Hashes y límites en
`../VALIDACION-W06B-PASTE-CLIPBOARD-2026-10-20.md`.
Auditoría autorizada del alcance, 2026-10-20: la ruta de resultado de Entregas comparte la misma
abstracción; un barrido de `ui-slint` no encontró llamadas directas al backend fuera de
`ops_ctrl/clipboard_io.rs`. La prueba adicional `async_read_is_processed_off_caller_thread`
confirma que la lectura no corre en el hilo solicitante. Suite focalizada actual: 3/3.

## W-06C — datos iniciales de configuración y workspace

**Implementado, probado y empaquetado el 2026-10-13.** Mover a workers la lectura al arranque de `settings.json`,
catálogos `lang/`, `keybindings.json`, `workspace.json`, `favorites.json` y `templates.json`, junto
con recuperación/copia de archivos corruptos, migraciones y persistencia del idioma detectado en
primer uso. La resolución diferida de `icon_set` se incorporó al cierre de W-06A para que la lectura
de settings no vuelva a escanear `icons/` desde UI.

Arrancar con estado embebido/de fábrica y aplicar el snapshot cuando llegue el resultado. Diseñar
el merge para que una carga lenta no sobrescriba cambios de ajustes, atajos, favoritos, plantillas,
navegación o sesión efectuados mientras espera. Mantener los fallbacks actuales (defaults, idioma
embebido, primera disposición clásica) ante archivo ausente/corrupto, y respetar versiones futuras.
No hacer writes de migración ni backups desde UI. Colas y resultados acotados, waker al completar y
errores discretos al log.

**Implementación:** `ConfigCtrl::new_for_ui` inicia con recursos embebidos; un worker lee los archivos
de usuario y el event loop aplica el snapshot. Merge por campo conserva ajustes y keymap editados;
el baseline protege navegación y sesión, y favoritos/plantillas combinan las ediciones locales con
el snapshot. El idioma de primer uso se detecta y persiste desde el worker. Cierre durante la carga
queda diferido. Si no llega el snapshot, settings y workspace no escriben defaults sobre datos que
no se pudieron leer. Favoritos y plantillas usan el writer JSON compartido; `write_atomic` crea el
directorio padre desde ese worker para una configuración nueva.

**Pruebas ejecutadas:** directorio sin archivos y conjunto persistido completo; edits de settings y
keymap durante carga; navegación mientras espera el snapshot de sesión; fallo inicial sin
sobrescribir settings; escritura atómica con padre ausente y error real de escritura; workspace
completo: 893 core, 22 flujos de operaciones, 54 platform (6 ignorados), 365 UI y pruebas de
integración. Pasaron también `cargo check -p naygo-ui-slint`, formato y `git diff --check`.
El build release supervisado generó ZIP e instalador; hashes, tiempos y cobertura directa se
registran en `VALIDACION-BUILD-2026-10-13.md`. No se ejecutó instalación manual ni se afirma que
cada caso de corrupción/locale de la matriz se haya probado individualmente.

**Criterio de cierre:** cumplido para código, pruebas y distribución. W-07 es el siguiente bloque.
La aceptación instalada queda en W-13.

## W-07 — evidencia de rendimiento

**Progreso 2026-10-14:** capturada línea base sintética en `docs/BENCHMARKS.md` con tres
ejecuciones del harness existente y medianas por operación. Agregada instrumentación local opt-in
con cola acotada y no bloqueante en el event loop, retención limitada, p50/p95 y resumen en log al
cierre. Cubre tick, sincronización Slint, sondeo/primera respuesta/finalización/cancelación de
listados y trabajo de preview/escena 3D. Además registra inicio, término, rechazo, cantidad activa
y pico simultáneo en los permisos reales de workers de preview y validación de rutas. La suite UI
(370 unitarias + 16 de integración/render), Clippy global con `-D warnings`, formato, diff y graphify
pasaron; la distribución supervisada 0.5.2 se regeneró y verificó (registro de hashes en
`VALIDACION-BUILD-2026-10-13.md`).
El 2026-10-14 se agregó al worker serial de packs la medición opt-in de espera en cola y duración,
además de contadores submitted/started/finished/active/pico/rejected. Una prueba integrada
comprueba que el cierre drena las muestras antes de unir al colector. El delta pasó 898 tests core,
22 flujos de operaciones, 54 tests platform (6 ignorados), 388 unitarios UI y 16 de integración/
render, Clippy global, formato, diff, diez catálogos y Graphify. El build supervisado terminó con
10,41 GiB pico, y sus hashes están en `VALIDACION-BUILD-2026-10-14.md`.

Esto no cierra W-07. Siguen pendientes
latencia de interacción, CPU en reposo, memoria principal + auxiliares, handles y matriz local/UNC
con escenarios reales. Se regeneró y verificó distribución 0.5.2 con hashes; la evidencia está en
`VALIDACION-BUILD-2026-10-13.md`. No se fijan objetivos de rendimiento antes de contar con esas
mediciones.

**Repetición Windows, 2026-10-14:** tres corridas externas del harness, mismo Ryzen 7 1700X,
Windows 10 Home y Rust 1.96.0. Las medianas de 100.000 filas quedaron en 124,9877 ms para vista
inicial, 2,4532 ms para sort por tamaño, 128,6979 ms para filtro+sort y 20,3729 ms para el índice
de cambios por ruta. Respecto de la base del día 13, la variación observada va de +1,0 % a +9,4 %,
sin superar el umbral orientativo del 15 %. El JSONL y la tabla están en `docs/BENCHMARKS.md` y
`target/agent-out/w07-windows-repeat-20261014-193105.jsonl`. No mide UI completa y no cierra W-07.

**Repetición Windows, 2026-10-15:** otras tres corridas secuenciales del mismo harness con
fingerprint completo (Windows 10 Home, AMD64 Family 23 Model 1 Stepping 1, 16 hilos, Rust 1.96.0).
Las medianas externas de 100.000 filas fueron 21,3892 ms para vista inicial, 2,4636 ms para sort
por tamaño, 24,1554 ms para filtro+sort y 19,7888 ms para el índice por ruta. Son marcadamente
menores que las del día anterior; no se atribuye causa porque no se aisló el estado del CPU ni se
reconstruyó el checkout/artefacto del 14 de octubre. Los JSONL y la tabla están en
`docs/BENCHMARKS.md` y `target/agent-out/w07-repeat-20261015-035239-run{1,2,3}.jsonl`. No cierra W-07.

**Filesystem local real, 2026-10-15:** el benchmark `large_directory` acepta `--filesystem` para
crear temporalmente 10k/100k archivos vacíos y listarlos con `core::spawn_listing`; mide primera
entrada, lote inicial de 1.024 y finalización/join en tres pasadas por tamaño. En Windows 10 Home
19045, el host reportó AMD64 Family 23 Model 1 Stepping 1, 16 hilos y Rust 1.96.0; las medianas fueron
1,124/15,963/15,969 ms para 10k y 1,170/161,651/161,659 ms para 100k. El target se ejecutó con
`cargo bench ... -- --json --filesystem`, pasó y limpió los temporales. Es worker/canal con consumo
inmediato, no mide Slint, interacción ni UNC. La misma ejecución canceló seis listados tras 1.024
entradas; todos reportaron `Cancelled` y el worker terminó/join en mediana 0,0712 ms (10k) / 0,0876
ms (100k). Fingerprint completo: Ryzen 7 1700X, 63,94 GiB RAM, Rust 1.96.0, Windows 10. Es evidencia
local parcial, no interacción real ni cierre W-07. JSONL y modo de repetición en `docs/BENCHMARKS.md`.

**Separación de sincronización Slint, 2026-10-14:** el opt-in conserva ahora, además del span
global `ui.sync_rows`, las fases `ui.rows_signature`, `ui.rows_build`, `ui.rows_convert` y
`ui.rows_model_apply`. Permite distinguir firma, conversión desde workspace a `PlainRow`, mapeo a
`RowData` y mutaciones del modelo estable. Los spans no leen reloj cuando la captura está apagada;
los resultados aún no se han medido en una sesión interactiva.

**Duración de callbacks W-07:** se agregaron spans opt-in para despacho de teclado, clic/doble
clic/clic medio de fila y sort de columnas. Los de fila y sort incluyen `sync_rows`/`sync_layout`
cuando el callback lo ejecuta sincrónicamente; teclado incluye su sincronización directa, pero no el
tick posterior ni el frame presentado. No se registran teclas, rutas ni nombres. El test focalizado
de las seis pruebas de `metrics` pasó. El release vigente se abrió con `NAYGO_PERF_METRICS=1` y se
cerró por `--shutdown`; el log confirmó opt-in y métricas de listado/modelo/tick. La automatización
local no produjo muestras `ui.input.*_callback`: `SendKeys` falló, `PostMessage` de teclado y ratón
no llegó a los callbacks y Windows negó el foco mientras otra ventana estaba activa. No se enviaron teclas a esa
ventana. Sigue pendiente capturar callbacks con Naygo en primer plano y completar la matriz real, renderer, CPU/RAM de
auxiliares y local/UNC. Evidencia de build y hashes en
[`VALIDACION-BUILD-2026-10-15.md`](../VALIDACION-BUILD-2026-10-15.md).

**Rasterizado headless W-07, 2026-10-16:** la prueba manual de sincronización ahora también fuerza
cinco redraws de `AppWindow` en `MinimalSoftwareWindow` a 1280×800, tras cargar 10k/100k filas. Tres
pasadas en Windows 10 Home, Ryzen 7 1700X, Rust 1.96.0 y perfil `test` (sin optimización) dieron
medianas de render de 72,409 ms y 78,535 ms, respectivamente. En las mismas pasadas, sync inicial
medió 233,274/1.823,465 ms y append 263,609/2.398,735 ms. La virtualización limita el rasterizado
al viewport visible; los números son headless/debug, no latencia de release ni presentación Winit.
La repetición individual en perfil release supervisado se registra en `docs/BENCHMARKS.md`; tampoco
mide presentación/interacción ni cierra W-07. Logs debug en
`target/agent-out/w07-software-render-debug-run{1,2,3}.log`; la ventana interactiva queda pendiente.
El gate workspace posterior pasó 1.399 tests,
7 ignorados y Clippy global `-D warnings`; el log supervisado es
`target/agent-out/guarded-20261016-022620-776.out.log`. No se regeneró distribución porque el cambio
es sólo del harness de medición.

**Observación pasiva, 2026-10-15:** además de una muestra inicial de 20 s, se recogieron tres ventanas
consecutivas de 30 s del `naygo.exe` instalado, sin interacción deliberada. Memoria, handles y threads
se mantuvieron estables (72,8 MiB working set, 54,9 MiB privados, 432 handles, 35 threads); los
deltas CPU fueron 0,14/0,03/0,16 s. No se confirmó inactividad completa, no se capturaron auxiliares
y no había unidades SMB mapeadas. Los JSON están en `target/agent-out/`; falta interacción real y la
matriz local/UNC, por lo que W-07 sigue abierto.

Se añadió `scripts/capture-w07-process.ps1` para repetir estas lecturas en ventanas configurables y
guardar JSON sin interactuar con la aplicación. Su prueba local con una ventana de 2 s produjo un
JSON válido. El helper no habilita `NAYGO_PERF_METRICS` ni reemplaza la matriz interactiva.
El 15 de octubre se actualizó a schema v3 para muestrear descendientes durante cada ventana (2 s por
defecto), retener las identidades observadas y reportar picos de recursos. Una prueba sintética
confirma que registra un proceso hijo que termina antes del cierre de la ventana. El intervalo
configurable permite observar lectores breves a cambio de consultas y sobrecarga adicionales; aún
puede perder procesos más cortos que el intervalo. No reemplaza la medición interactiva.

**Repetición pasiva, 2026-10-15 08:00:** sobre la instancia instalada abierta, tres ventanas de
30 s registraron 41,6 MiB working set, 40,1 MiB privados, 349 handles, 17 threads y deltas CPU de
0,031/0,047/0,016 s; no había SMB. No se confirmó que la sesión estuviera inactiva, no se capturaron
procesos auxiliares y no se midió latencia. Las diferencias de memoria respecto de muestras previas
no se atribuyen sin controlar build/sesión/actividad. Registro:
`target/agent-out/w07-process-20261015-080019.json`.

**Nueva repetición pasiva, 2026-10-15 12:53:** la instancia instalada (PID 23856) permaneció
abierta durante tres ventanas de 30 s: 70,2 MiB working set, 46,9 MiB privados, 439 handles,
19 threads y deltas CPU de 0/0,047/0 s; sin mapeos SMB. No se interactuó con la ventana ni se
confirmó inactividad; no incluye procesos auxiliares ni mide latencia. No se atribuye la diferencia
con la muestra de las 08:00. Registro: `target/agent-out/w07-process-20261015-125432.json`.
Nueva repetición pasiva del mismo PID, 19:04–19:06: 46,6 MiB working set, 47,5 MiB privados,
437 handles, 17 threads y CPU sin incremento durante tres ventanas; sin SMB. No hubo interacción
ni confirmación de inactividad, y la muestra excluye auxiliares/latencia. La variación respecto a
las capturas anteriores queda sin atribuir. Registro en
`target/agent-out/w07-process-20261015-190632.json` y `docs/BENCHMARKS.md`.
Al comparar en disco, la instancia de PID 23856 usa el EXE instalado el 14 de septiembre
(42.060.800 bytes; SHA-256 `c78828504e8f5908a0fb47c31f542d7c52d65a84f98469c3352079809d876f8e`),
no el release recién generado (42.762.240 bytes; SHA-256
`729e71fc00546136d2063b82a41fe1f73772ef584c1478302f381c37e7d5bc8c`). Las muestras pasivas no
describen el binario actual; la aceptación W-13 y medición vigente requieren probar la distribución
del 15 de octubre en un entorno autorizado.
La instancia llevaba activa desde las 07:08. El log no contiene la marca de activación opt-in.
`naygo-platform` usa el mutex fijo `NaygoSingleInstance` para toda la sesión, de modo que lanzar una
copia en otra carpeta con métricas activadas enviaría la solicitud a la ventana existente y saldría;
no sirve como aislamiento de la corrida. Para obtener spans internos hay que cerrar limpiamente y
relanzar la instancia con `NAYGO_PERF_METRICS=1`, o esperar una sesión del usuario con esa variable.
**Smoke del portable vigente, 2026-10-16:** se extrajo el ZIP de distribución a una carpeta aislada,
se inició con `NAYGO_PERF_METRICS=1` y se cerró con `--shutdown`. El log muestra el backend de
software, event loop, listado, sync de filas/modelo y cierre ordenado con métricas; no contiene
muestras `ui.input.*`. Es verificación de arranque/cierre, no benchmark de interacción. El ZIP, EXE y
setup quedaron registrados en `docs/VALIDACION-BUILD-2026-10-16.md`; W-07 permanece abierto.

**Repetición 2026-10-16 con portable aislado:** 100 archivos sintéticos, 23 filas visibles en UIA;
`listing.first_batch`/`complete` p50 45,955/45,964 µs y máximos 1.108.163/1.108.170 µs. El opt-in
registró `sync_rows` p50 2.781 µs y `tick` 3.279 µs. La captura gráfica falló dos veces antes de
enviar entradas; no hubo spans `ui.input.*`. La configuración instalada se restauró y el release
quedó abierto. No cierra latencia Winit, interacción real, reposo controlado ni la matriz local/UNC.

**Archivos:** `docs/BENCHMARKS.md`, benchmark `large_directory`, módulo pequeño de métricas,
`tick.rs`, `listing.rs`, `sync.rs`, preview/supervisor y scripts de medición.

- [x] Opt-in, sin telemetría; tiempos monotónicos; cola de métricas acotada; descarte antes que
  bloquear el event loop; sin nombres/rutas/contenido.
- [x] Benchmark sintético de 10k/100k, tres ejecuciones y medianas; sólo representa el modelo.
- [x] Contadores de cola/job reales: workers de preview, validación de rutas y packs informan
  submitted/started/finished/active/pico/rejected; packs informa además espera y duración. La prueba
  de cierre integrado verifica el drenaje antes del join del colector.
- [x] Helper Windows para muestreo pasivo del proceso principal, salida JSON y consulta opcional de
  mapeos SMB; probado con una ventana breve. No sustituye métricas internas opt-in ni interacción.
- [x] Enumeración real local y cancelación del worker core medidas en fixtures temporales de 10k/100k;
  se registra primera entrada, lote, terminal y join. No incluye la UI Slint ni red.
- [x] Sincronización real de `sync_rows` headless a 10k/100k, incluida actualización de fila y append;
  la tabla separa este costo de render e interacción.
- [x] Rasterizado software headless a 10k/100k con `MinimalSoftwareWindow`; perfil test sin optimizar,
  no mide composición/presentación del backend Winit ni sirve de baseline release.
- [x] Spans opt-in para callbacks de teclado, filas y sort; aún faltan muestras de callbacks reales y
  latencia de presentación.
- [ ] Medir primera fila/contenido, recogida, sort, conversión, actualización del modelo, interacción,
  cancelación, hilos/handles/procesos, CPU en reposo y memoria principal + auxiliares.
- [ ] No sumar RSS compartido como si fuera memoria única ni confundir compromiso/Job/RSS;
  indicar método, solapamientos y presupuesto de buffers por separado.
- [ ] Matriz restante: 10k/100k sintéticos, UNC disponible/caída, preview apagado,
  estático/animado, A → B → A, minimizar/bandeja, PDF comprimido, TAR.GZ, texto línea gigante.
- [ ] Repetir escenarios interactivos tres veces, mediana y p95 de latencias con suficientes muestras,
  mismo equipo/dataset/
  toolchain/build. Preservar logs de referencia. Definir objetivos antes de optimizar, sin inventar
  números de rendimiento de esta sesión.

**Pruebas:** [x] apagada sin actividad, [x] cola saturada no bloquea, [x] identidad anónima,
[x] contadores de cola/jobs y prueba de cierre integrado. **Cierre:** informe con resultados reales
y limitaciones de hardware/red.

**Captura pasiva adicional, 2026-10-17:** tras regenerar e instalar la build local de la corrección
del drop, se observaron tres ventanas de 30 s del PID 23400: 83,7–83,8 MiB working set, 52,7 MiB
privados, 370 handles, 24 hilos y 0,063/0,015/0,047 s CPU; un proceso, sin SMB. La sesión no se
controló como reposo ni hubo interacción. No mide latencia, presentación Winit o red y no cierra W-07.
JSON y método en `docs/BENCHMARKS.md`; EXE SHA-256 `309d751b…`.

**Revalidación W-07/W-13 sobre 0.5.3, 2026-10-19:** la instancia instalada PID 48444 responde.
UIA de solo lectura encontró 768 descendientes, 600 con nombre y 140/140 controles enfocables
nombrados, árbol sin truncar. Captura pasiva de recursos en tres ventanas de 30 s: 76,9 MiB
working set, 45,3 MiB privados, 385 handles, 26 hilos, CPU delta 0/0/0,063 s, un proceso y sin
SMB. No se controló inactividad ni hubo entrada; esto no mide latencia, presentación o actividad del
filesystem y no cierra W-07/W-13. Evidencia en `docs/VALIDACION-0.5.3-2026-10-19.md` y
`docs/BENCHMARKS.md`.

**Captura pasiva W-07, 2026-10-19 23:51:** la instalación abierta PID 50260 se observó en tres
ventanas de 30 s: working set 62,0 MiB, privado 43,2 MiB, 368 handles, 22 hilos, CPU delta
0,031/0,032/0,015 s; un proceso y sin SMB. Es el EXE previo al paquete actual; no se controló reposo
ni se midió input, renderer, latencia o archivos. No cierra W-07. JSON y método en `docs/BENCHMARKS.md`.

**W-13, corrección y paquete local 2026-10-19:** el callback nativo de doble clic Slint recibe
Ctrl/Shift del mouse-up de la fila; la ruta de respaldo en Rust ya usaba ese gesto. La prueba para
estado Ctrl/Shift pegado tras modal y el gate completo pasan (1.465 tests, 8 ignorados, Clippy y
formato). Se regeneró ZIP/setup 0.5.3 con pico de 10,64 GiB, sin instalar: PID 48444 conserva el
binario previo. Aceptación con Narrador, foco/DPI y recorridos manuales sigue pendiente.

### M-01 — deuda de Clippy observada

Nicolás aprobó registrar como bloque separado los avisos del gate global, fuera del alcance
funcional de W-06C. El log de la corrida del 13 de octubre presenta 16 ocurrencias agrupadas en
siete clases: `type_complexity`, `too_many_arguments`, `iter_cloned_collect`,
`field_reassign_with_default`, `items_after_test_module`, `large_enum_variant` e
`if_same_then_else`.

**Progreso 2026-10-13:** corregidas las 16 ocurrencias sin supresiones: inicializadores de tests,
alias de callbacks, agrupación de dependencias de `wire_config`, enum de resultados liviano y
reordenamiento de tests. Se preserva el flujo de carga de sesión y aplicación de packs.
`cargo clippy --workspace --all-targets --locked --offline -- -D warnings` pasa; la suite completa
del workspace pasa (893 core, 22 flujos, 54 platform/6 ignoradas, 369 UI y 16 integraciones).
Resta verificar el build de distribución para marcar el bloque cerrado.

**Alcance:** corregir cada ocurrencia con cambios legibles y locales, sin supresiones globales,
alterar contratos por silenciar Clippy ni mezclar cambios funcionales. Revisar tamaño de enum y
coste de clonar antes de escoger un arreglo; mover módulos de tests conservando visibilidad.
**Pruebas:** gate Clippy global, tests del workspace, formato/diff, graphify y build supervisado.
**Cierre 2026-10-14:** el gate Clippy global queda limpio con `-D warnings`; pasan las suites,
formato, diff, diez idiomas y build supervisado. Hashes del ZIP y setup en
`VALIDACION-BUILD-2026-10-13.md`. Sin cambio de comportamiento previsto.

## W-08 — recogida y canal acotados

**Archivos:** `core/listing.rs`, `ui-slint/listing.rs`, `workspace_ctrl/listing.rs`, árbol,
tests de streaming y métricas W-07.

**Progreso 2026-10-14:** cada poll limita la recogida a 1.024 entradas o 3 ms. El tick comparte
ese tope entre Files y árbol, reserva 512 filas por grupo cuando ambos están activos y rota el
primer listado dentro de cada grupo. El canal productor→UI tiene capacidad 1.024, usa reintentos
cancelables y abandona al desconectarse el receptor; el terminal viaja por separado y sólo se
entrega tras drenar filas anteriores. Un corte conserva filas y terminal para ticks siguientes;
`fresh` sólo se reclama con avance real. Pasaron la suite completa (894 core, 22 flujos, 60 platform
con 7 ignoradas, 372 UI + 16 integración/render), Clippy global con `-D warnings`, formato y diff.
El build supervisado terminó y ambos artefactos se verificaron el 2026-10-14.

- [x] Presupuesto cooperativo por poll y conservación del terminal tras el límite.
- [x] Canal acotado con backpressure cancelable, salida al desconectar receptor y terminal separado.
- [x] Presupuesto global compartido y rotación entre paneles/árbol.

- Probar inicialmente máximo 1.024 entradas o 3 ms por recogida, según plan RP-6;
  parametrizar para medición, no convertirlos en garantía de frame completo.
- Canal de lotes acotado; backpressure que consulte token y desconexión sin quedarse bloqueado
  en un `send` cuando UI deja de drenar. Entregar terminal exactamente una vez, en orden.
- Round-robin entre paneles/árbol con presupuesto global por tick, no un presupuesto completo
  multiplicado por número de paneles. Conservar orden, identidad, contexto y notificaciones.
- No consumir `fresh` en un poll vacío; no perder `Done` tras un corte de presupuesto.

**Pruebas:** cola llena + cancelar, receptor cerrado, vacío, varias carpetas, terminal detrás
de lotes, watcher en tránsito y conservación de todas las entradas pasaron. Queda comparar primera
fila, p95 de interacción y tiempo total con la misma carga de la base anterior. Más ticks pueden
mejorar respuesta y aumentar el tiempo total; no se cierra el resultado de rendimiento hasta medirlo.

## W-09 — admisión y probes

**Archivos:** `Listing::start`, `core/listing.rs`, `workspace_ctrl/listing.rs`
(`refresh_missing_cache`), controlador de árbol y workers relacionados.

**Progreso 2026-10-14:** Files, árbol y probes usan una admisión común de ocho plazas adquiridas
antes de crear el hilo. El permiso queda dentro del worker y se libera sólo al salir; cancelar o
soltar el receptor no libera capacidad mientras un `read_dir` del SO siga bloqueado. Files guarda
una única ruta pendiente por panel; las ramas pendientes del árbol se deduplican por panel/ruta y
se reintentan desde el tick. Los probes de navegación consultan sólo ese panel; solicitudes nuevas
cancelan cooperativamente el recorrido anterior, coalescen el ámbito más reciente y usan una
revisión para descartar respuestas A→B→A. Los probes globales (cambio de dispositivos/restauración)
conservan su recorrido completo. El estado de espera está representado por las colas pendientes y
mantiene el tick activo hasta conseguir plaza o terminar; falta decidir si se requiere una señal
visible de capacidad agotada y validar comportamiento con una syscall de red realmente lenta.
La prueba del controlador retuvo la única plaza y emitió 100 navegaciones; sólo quedó la última ruta,
y al soltar la plaza el worker pendiente arrancó. La prueba core mantuvo ocupado el permiso durante
la fase final del hilo y comprobó su liberación al salir. Suite workspace (895 core, 22 flujos,
54 platform/6 ignoradas, 373 UI + 16 integración/render), Clippy global con `-D warnings`, formato,
diff y Graphify pasaron; build y hashes supervisados se verificaron el 2026-10-14.

- [x] Plaza RAII antes del spawn, retenida hasta la salida real del worker.
- [x] Última navegación por panel y ramas de árbol deduplicadas/reintentadas.
- [x] Probe único con cancelación cooperativa, coalescing de ámbito y revisión de resultado.
- [ ] Aceptación de interfaz y prueba de bloqueo de filesystem real; no se promete interrumpir una
  syscall que el sistema operativo mantenga bloqueada.

- Permisos adquiridos ANTES de crear hilos, retenidos hasta salida real, incluidos retirados.
  No liberar una plaza por soltar receptor ni por timeout lógico.
- Última petición pendiente por panel, coalescer probes por ubicación/revisión; cancelar listado
  y probe antes de abrir red cuando ya se descartó. No escanear todos los paneles por cada navegación.
- Timeout lógico conserva respuesta UI; un `read_dir` bloqueado puede retener plaza. Documentar
  ese límite. Si las mediciones demuestran agotamiento persistente, proponer proceso auxiliar
  para enumeración como bloque nuevo; no prometer matar hilos de Rust ni reusar el lector preview.
- Definir estado de capacidad agotada y reintento al salir worker; sin sondeo residente remoto.

**Pruebas:** worker retenido por barrera + 100 navegaciones, cota de plazas/hilos, cancelación
antes de I/O, cierre de panel, liberación RAII y recuperación al descongelar. No usar UNC real
como único mecanismo de prueba de concurrencia.

## W-10A/B — costo de vista y modelos

**W-10A:** `core/workspace/file_pane.rs`, consumidores de `view_indices`, benchmark.
El caché devuelve clones de `Vec<usize>` aun en hit. Introducir acceso compartido/inmutable
con API segura, sin préstamo `RefCell` vivo durante mutación. Mantener único orden de vista
para foco, selección y render; eliminar doble sort final sólo si se confirma redundancia.
**Pruebas:** invalidación por filtros/orden/entries, contexto de navegación, sort estable,
selección después de watcher y varias llamadas cacheadas. Medir allocs/copia en hit.

**Cierre 2026-10-14:** `ViewCache` conserva `Arc<[usize]>`; `view_indices_shared()` comparte
el buffer entre el bridge y los consumidores de comparación/resaltado. La API previa que devuelve
`Vec` se conserva por compatibilidad. Una prueba verifica identidad de allocation en cache hit e
invalidación al cambiar sort. En benchmark release, a 100.000 filas el hit con copia promedió
0,021924 ms y el acceso compartido 0,000061 ms (100 llamadas por escenario); a 10.000 filas,
0,002137 ms y 0,000057 ms. La ruta compartida evita copiar hasta 800.000 bytes de índices en cada
hit de 100.000 filas. Es un microbenchmark del modelo, no una medición de frame/UI completa.
Suite workspace (896 core, 22 flujos, 54 platform/6 ignoradas, 373 UI + 16 integración/render),
Clippy global con `-D warnings`, formato, diff y Graphify pasaron. Build supervisado terminado:
Cargo 26m 01s, pico Job 10,41 GiB/12, mínimo de memoria libre muestreada 32,02 GiB, Inno Setup
106,407 s y hashes verificados; evidencia en `docs/VALIDACION-BUILD-2026-10-13.md`.

**W-10B:** `sync.rs`, `workspace_ctrl/listing.rs`, `bridge.rs`, modelos estables.
Elegir una mejora por evidencia: revisiones/deltas para foco y selección; construir únicamente
filas cambiadas; o modelo con datos de fila bajo demanda. No rediseñar toda la tabla en un bloque.
No remount del delegate en edición inline, ni destruir hover durante down/up/scroll.
**Pruebas:** rename editando mientras watcher actualiza, scroll y filtros, selección masiva,
cambio de largo, resaltado que expira y dos paneles. Comparar CPU/RAM de frame completo.
Si no hay mejora medible o hay complejidad desproporcionada, registrar el resultado y conservar
el diseño actual; esa evaluación no equivale a afirmar una optimización implementada.

**Implementación W-10B, 2026-10-15:** una prueba manual headless ejecuta el `sync_rows` real de Slint
con 10k/100k filas. La conversión crea un `ModelRc` nuevo para las celdas anidadas de cada `RowData`,
por lo que la igualdad por identidad nunca encontraba filas comunes y un append de una fila emitía
100k remove + 100001 insert; a 100k medía 44,032 s. `row_data_eq` compara los campos visibles y el
contenido de las celdas, de modo que el append real se aplica como un único cambio estructural y
midió 366,762 ms (~120x menos en la misma muestra). Las pruebas cubren igualdad de contenido con
modelos anidados distintos y una celda diferente. Se completó el delta `Modified`: solo permite
refresco puntual cuando ruta, nombre, tipo, visibilidad, sort y filtros garantizan posiciones estables;
la vista cacheada se conserva. En tres corridas `diag`, a 100k el cambio de una fila medió 31,874 ms
frente a la muestra anterior de 353,568 ms (~91 % menos). Carga y append variaron frente a muestras
históricas únicas, así que requieren comparación controlada antes de atribuir causa. Es una muestra
headless, sin rasterizado ni interacción: W-07 conserva la validación de frames/latencia/CPU/RAM y
W-13 la aceptación instalada. El gate completo de este delta pasó (1.393 tests, 7 ignorados, Clippy
sin warnings y formato). El 15 de octubre se regeneraron ZIP e instalador 0.5.2 bajo supervisor, con
pico de Job 10,42 GiB. El ejecutable del ZIP coincide con `target/release/naygo.exe`, el ZIP no incluye
PDB, EXE y setup reportan `NotSigned` y los hashes están verificados en
`docs/VALIDACION-BUILD-2026-10-15.md`.

## W-10C — append incremental durante streaming

La UI no debe reconstruir ni convertir las filas previas en cada revisión append-only. El core
conserva el rango de lotes consecutivos; el sync solo agrega la cola cuando coinciden la revisión,
el contexto, la cantidad fuente y el prefijo de índices visibles. Un refresh, una mutación no
append, un cambio de contexto o un elemento que se ordenaría entre filas existentes usa la
reconciliación actual. La prueba de sync headless cubre append ordenado y fallback ante inserción
intermedia; la prueba core cubre coalescencia de lotes e invalidación por mutación.

**Implementación local 2026-10-16:** el productor de streaming marca cada lote append-only y el
modelo estable convierte/agrega solo las nuevas filas cuando la vista confirma el prefijo. Los
índices visibles anteriores se comparten con `Arc`, y la comprobación del prefijo solo compara esos
índices; `rows_at` calcula las filas agregadas bajo el mismo formato/iconos/selección ya validado por
la firma de contexto. Tras un refresh, el primer lote reemplaza el listado anterior y lotes
posteriores pueden volver a usar append. La inserción intermedia, los cambios `Modified` y las
mutaciones no clasificadas conservan sus rutas previas. Tests focalizados core/UI y gate completo
pasan. Tres corridas release a 100k dieron append 21,140/21,741/21,279 ms, mediana 21,279 ms frente
a 561,367 ms de la muestra anterior (~26,4×, 96,2 % menos; evidencia headless). Distribución 0.5.2
regenerada bajo supervisor, gate y hashes en `docs/VALIDACION-BUILD-2026-10-16.md`; W-07/W-13
conservan la prueba interactiva y aceptación instalada.

## W-11A/B/C — completar lectores RP-6

**W-11A:** `preview.rs` (PDF), metadata PDF y dispatcher aislado. Auditar API de `lopdf`
0.41 y `pdf-extract` 0.10 fijadas; evitar doble carga sólo con una API comprobada y salida
equivalente. Límites de parseo/expansión, páginas recorridas y texto se distinguen. Mantener
Job/timeout/cancelación y salida parcial explícita. No agregar render de páginas.
**Pruebas:** PDF vacío/corrupto/comprimido, extracción enorme, cifrado, límite/timeout y texto
visible equivalente en corpus. Medir tiempo/memoria de principal e hijo.

**Implementación W-11A, 2026-10-14:** el crate `pdf-extract` 0.10 reexporta el mismo tipo
`Document` de lopdf 0.38 que usa internamente y ofrece `output_doc` con el mismo
`PlainTextOutput` de `extract_text`. El preview carga ese documento una vez para contar páginas y
extraer el texto; `naygo-core` mantiene lopdf 0.41 para metadata general. Un writer detiene la
acumulación al exceder los 8.000 caracteres y un adaptador de `OutputDev` limita el contenido
recorrido a 500 páginas, dejando explícito `truncated`. El proceso auxiliar mantiene sus límites
de 192 MiB y cinco segundos; no se añadió renderer PDF.

Las pruebas dirigidas cubren el máximo Unicode exacto, salida PDF larga, corte al máximo de páginas
y el panic conocido de streams hostiles. Suite completa (896 core, 22 flujos, 54 platform/6
ignoradas, 376 UI + 16 integración/render), Clippy global, formato, diff y Graphify pasaron. Build
supervisado y hashes de distribución verificados el 2026-10-14; el log está en
`target/agent-out/guarded-20261014-064512-790.out.log`. Cargo tardó 29 min 54 s, el pico del Job fue
10,41 GiB de 12 GiB y el margen libre mínimo observado fue 16,19 GiB; Inno Setup compiló en
134,969 s. El ZIP contiene `naygo.exe`, `LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB.
No se midieron tiempo ni memoria con corpus PDF; aceptación instalada continúa pendiente.

**W-11B:** preview de ZIP/TAR/TAR.GZ. Acotar entradas inspeccionadas, bytes comprimidos y
descomprimidos procesados, lectura de headers y suma de tamaños con aritmética comprobada.
El tope de filas visibles no limita el recorrido. Sin extracción a disco; mantener separación
de preview respecto de los motores completos de comprimir/descomprimir.
**Pruebas:** muchas entradas, tamaños declarados hostiles, TAR truncado, expansión gzip,
cancelación y salida recortada correcta. Verificar que el hijo se reap al abortar.

**Implementación W-11B, 2026-10-14:** la auditoría confirmó que ZIP cargaba su
directorio central completo antes de recortar las filas y TAR.GZ inflaba contenido al saltar
entradas. Se añadió preflight ZIP32/ZIP64 con máximo de 10.000 registros y 6 MiB de índice, además
de un límite acumulado de 8 MiB de lectura para índice y headers locales. TAR limita bytes leídos
a 32 MiB; TAR.GZ limita a 16 MiB comprimidos y 32 MiB expandidos. Ambos conservan hasta 500 filas.
Las rutas internas superan el preview si exceden 1 KiB o 64 componentes; overflow de total se
detecta con `checked_add` y deja el árbol parcial. Un límite conserva las filas ya leídas cuando
existen; sin filas muestra el aviso de límite ya traducido. No escribe ni extrae contenidos. Las
pruebas del módulo preview pasaron (28 tests, incluidos preflight ZIP32/ZIP64, índice sobredimensionado,
501 entradas, ruta larga/profunda, overflow y expansión TAR.GZ). Workspace completo (896 core,
22 operaciones, 54 platform/6 ignoradas, 384 UI + 16 integración/render), Clippy global con
`-D warnings`, formato, diff, diez catálogos y Graphify pasaron. Build supervisado
`target/agent-out/guarded-20261014-080649-329.out.log`: Cargo 37 min 22 s, pico Job 10,41 GiB de
12 GiB y margen libre mínimo observado 16,23 GiB; Inno Setup `Successful compile` en 182,125 s.
El ZIP contiene `naygo.exe`, `LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. Hashes y
aceptación externa quedan registrados en `docs/VALIDACION-BUILD-2026-10-13.md`.

**W-11C:** texto/SVG/imágenes. Conservar codificaciones, resaltado, selección, scroll y dimensiones
actuales; auditar trabajo antes del recorte de línea y complejidad SVG. Recursos externos continúan
deshabilitados. Proponer cambios sólo donde el corpus/mediciones encuentren gasto relevante.
**Pruebas:** Unicode multibyte, UTF-16, línea gigante, SVG complejo, imagen con canvas extremo,
animación pausada y cambio rápido. No recolectar frames ni introducir caché adicional sin evidencia.

**Implementación W-11C, 2026-10-14:** los cortes del preview de texto descartan únicamente la
unidad UTF-8/UTF-16 incompleta del final; mantienen Windows-1252 para bytes inválidos reales. El
reconocimiento del canvas SVG inspecciona directamente la etiqueta raíz en bytes, sin crear una
cadena lossily-decoded y otra copia en minúsculas del archivo entero (máximo 20 MiB). No se añadió
caché ni se recolectan cuadros. Casos sintéticos cubren UTF-8 cortado, UTF-16 LE/BE incompleto,
línea larga, 4.000 elementos SVG, canvas explícito, dimensiones de imagen mayores al decoder,
animación pausada, cambio rápido/cancelación y lector aislado. Las pruebas UI y workspace, Clippy,
formato, diff y diez catálogos pasan. El empaquetado supervisado y hashes se registran en la bitácora
de validación. La medición con corpus real/uso cotidiano sigue abierta dentro de W-07; los casos
sintéticos validan límites, no sustituyen esa línea base.

## W-12 — CI y reproducibilidad

**Archivos:** `.github/workflows/ci.yml`, `release.yml`, `rust-toolchain.toml`, scripts y BUILD.

- Añadir `--locked` a test/Clippy en CI; conservar paridad i18n y formato. Definir versión numérica
  de toolchain validada con un proceso de actualización explícito; no elegir una sin compilar.
- Benchmark con corpus/fingerprint del entorno, salida estructurada y referencia comparable.
  En runner compartido guardar informe primero; no poner gate ruidoso sin controlar variabilidad.
  El criterio existente de >15 % en dos ejecuciones aplica a mismo entorno/métrica y se revisa.
- Alinear empaquetado de CI con ThinLTO/16 unidades/jobs=1 y supervisor adecuado al runner.
  El script de release ya usa jobs=1; no afirmar que la variable global jobs=2 lo revierte.
- Ejecutar smoke del EXE empaquetado/protocolos headless en CI donde sea viable. Mantener
  separado el gate interactivo Windows; headless no certifica DPI, Narrador ni red real.

**Avance local W-12, 2026-10-14:** toolchain fijado a Rust 1.96.0 (compiló, pasó suite/Clippy y
release local); CI y release usan `--locked`, un job Cargo y ese pin. El benchmark de grandes
directorios puede emitir JSONL con huella del runner, muestras, mediana y p95 para ≥20 rondas; CI
lo guarda semanalmente/manual como artifact informativo, sin gate de porcentaje ruidoso. El release
extrae el ZIP en su runner efímero y ejercita la salida headless `--shutdown`. YAML, parser PowerShell
y benchmark JSONL pasaron validación local; el smoke local se omitió por una instancia Naygo activa.
En la continuación del 15 de octubre se volvieron a parsear ambos workflows YAML y los nueve
catálogos distintos de español pasaron `scripts/check_i18n_lang.py`.
No se ejecutó el workflow actualizado ni se publicó un tag. Consulta de solo lectura del repo el
2026-10-14: el último run CI visible corresponde a `main`/`abdef402` del 7 de septiembre y falla en
`naygo-platform::trash::tests::enviar_un_archivo_a_papelera` al restaurar desde la Papelera del
runner Windows Server 2025; el resto de ese job pasó. Se corrigió la ruta: `PostDeleteItem` captura
el nombre de parsing del ítem recién reciclado, y restore intenta `IFileOperation::MoveItem` sobre
ese objeto sin depender de la enumeración de Explorer ni del verbo contextual `undelete`; las rutas
anteriores quedan como fallback. El test ya no usa `#[ignore]`, exige el ítem real reportado por Shell
y pasó en el gate local completo (1.381 tests, 6 ignorados, Clippy limpio) y en la distribución
supervisada. La causa está corregida localmente; W-12 aún requiere un run real sobre el workflow
actualizado para confirmar Windows Server 2025.
El 2026-10-16 se leyó el log de ese run: falló `Tests` con 46 tests platform aprobados y 6 ignorados;
`enviar_un_archivo_a_papelera` recibió `el elemento ya no está en Papelera` al restaurar. Esto
confirma el síntoma del recibo/identidad del elemento en el runner, no la corrección actual, que
todavía requiere el run actualizado.

**Refuerzo de Papelera aprobado, 2026-10-15:** `move_to_trash_with_progress` ya no canonicaliza
el origen; obtiene la ruta absoluta conservando el junction/symlink final. `PostDeleteItem` asocia
el HRESULT y el nombre del elemento reciclado a su ruta origen, sin presuponer orden de callbacks.
Si quedan entradas, el fallback Shell opera una por una y se omite tras cancelar. La API conserva
los `TrashReceipt` de los éxitos en un lote parcial y la UI atribuye cada fallo por índice de origen
y ruta, aunque la ruta de entrada sea relativa, manteniendo deshacer. Pasaron 5 pruebas Windows
dirigidas, incluido crear/reciclar/restaurar un junction con su destino y archivo intactos, más
checks de platform/UI cruzados para Windows. El gate completo pasó con 1.389 tests, seis ignorados,
Clippy `-D warnings` y formato. La distribución se regeneró y verificó (Cargo 25m20s, Inno Setup
109,093 s, pico 10,42 GiB); el run remoto Windows Server 2025 sigue pendiente.

**Refuerzo local W-12, 2026-10-14:** el primer fix todavía inicializaba `FOLDERID_RecycleBinFolder`
y `BHID_EnumItems` antes de intentar el nombre de parsing cacheado. Si el runner no expone ese
proveedor (y no solo si devuelve cero ítems), restore fallaría antes de usar el recibo. La búsqueda
directa ahora ocurre primero; la enumeración queda diferida como fallback para recibos reconstruidos.
El gate supervisado volvió a pasar (suite completa, incluido el round-trip de Papelera, Clippy
`-D warnings`, pico 7,82 GiB). La distribución 0.5.2 también se regeneró bajo supervisor (32m03s
Cargo, 138,453 s Inno Setup, pico 10,42 GiB); ZIP, setup, hashes y ejecutable incluido se verificaron.
Sigue faltando un run real del workflow actualizado en Windows Server.

**Runner Windows explícito, 2026-10-15:** CI, benchmark y release ahora seleccionan el label
`windows-2025` en vez de `windows-latest`, para que la validación solicitada use Windows Server
2025 aunque GitHub cambie la imagen detrás del alias `latest`. El label está publicado en la tabla
oficial de [GitHub Actions runner images](https://github.com/actions/runner-images). La modificación
solo fija el entorno declarado; requiere un run remoto satisfactorio para cerrar W-12.
CI, benchmark y release también comprueban que `Win32_OperatingSystem.Caption` diga Windows Server
2025; el JSONL de benchmark conserva caption, versión/build, modelo del CPU y RAM física total.

**Acciones fijadas a SHA, 2026-10-15:** por aprobación de Nicolás, ambos workflows ahora usan commits
inmutables para `actions/checkout` (`11d5960a326750d5838078e36cf38b85af677262`),
`actions/upload-artifact` (`ea165f8d65b6e75b540449e92b4886f43607fa02`),
`Swatinem/rust-cache` (`6323deb102c322ba6fcbdcafc7e3dddab59af2b6`) y
`dtolnay/rust-toolchain` (`3fc1e40d4c4ce3007de028907c886bc3996ce2ea`). Cada SHA se resolvió desde su
tag/referencia con GitHub API el 15 de octubre; `SignPath` ya estaba fijada a
`f6d04783b4569d051e0c80105fe66e82819d0092`. Las cinco referencias exactas se consultaron también
por SHA y GitHub devolvió el mismo commit. Los workflows conservan permisos y triggers actuales.
YAML y los bloques PowerShell `pwsh` embebidos se parsearon localmente; el verificador de pins pasó.
W-12 sigue pendiente de una ejecución real en Windows Server 2025.
El nuevo `scripts/check_action_pins.py` se ejecuta en el gate y rechaza revisiones externas sin
SHA completo; las acciones Docker, si se agregan, deben usar digest SHA-256.
CI declara explícitamente `contents: read`, que cubre checkout, tests y artifacts; release conserva
`contents: write` para publicar. La corrida remota Windows Server 2025 sigue pendiente.
Revalidación de solo lectura, 2026-10-16: PyYAML parseó ambos workflows; `check_action_pins.py`
y `test-signing.ps1` pasaron localmente. El último CI remoto de `main` continúa fallido el 7 de
septiembre en `abdef402`, igual que el SHA de `HEAD` remoto/local; las modificaciones W-12 de esta
sandbox siguen sin commit/publicación. No se disparó ese workflow antiguo porque no probaría el
contenido local actualizado.

**Nota W-12 aprobada, 2026-10-15:** la consulta remota también encontró cinco ejecuciones de
`Preview implementation review` del 9 de septiembre en `feat/responsive-previews-and-paths`, sin PR
abierto. La última (run `34364248486`) se detuvo en `Apply reviewed changes with source hashes`:
la base de `docs/PREVIEW_AND_PATHS.md` no coincidía con el parche esperado, antes de compilar o probar.
El script remoto `scripts/preview-fixes-3.py` no existe en este checkout. Se registra como incidencia
del flujo experimental de parches si se retoma esa automatización; no es un fallo del workflow W-12
actual ni sustituye la validación pendiente en Windows Server 2025.

**Acotación FIFO del caché de recibos, 2026-10-14:** el límite de 1.024 identidades evitaba
retención ilimitada, pero una clave arbitraria de `HashMap` podía expulsar un recibo reciente. El
caché ahora conserva orden de inserción, refresca rutas reemplazadas y expulsa primero la más antigua.
Un test dirigido verifica capacidad y prioridad reciente. Suite Windows y Clippy global pasaron bajo
supervisor (pico 5,93 GiB); la distribución se regeneró bajo supervisor (31m11s Cargo, 114,485 s
Inno Setup, pico 10,41 GiB) y sus hashes se verificaron. Queda el run remoto de Windows Server.

El round-trip `enviar_un_archivo_a_papelera` ahora inyecta un enumerador que fallaría si se llama y
exige que la restauración del `TrashReceipt` directo lo deje sin invocar. La prueba dirigida pasó en
Windows, así como suite/Clippy bajo supervisor (pico 5,89 GiB). Esta evidencia local cubre la
independencia del enumerador. La distribución final 0.5.2 se regeneró y verificó: Cargo release
26m28s, Inno Setup 111,860 s, pico 10,41 GiB; hashes en `VALIDACION-BUILD-2026-10-14.md`. Solo el
workflow remoto puede confirmar Windows Server 2025. El 15 de octubre se añadió una prueba para el
recibo reconstruido: fuerza error del enumerador, verifica que el respaldo se intenta una sola vez y
que el mensaje preserva el fallo. El test dirigido y el gate completo con Clippy pasaron (56 tests
platform, seis ignorados; pico 5,89 GiB); log `target/agent-out/guarded-20261015-001236-148.out.log`.

**Revisión adicional de identidad Windows, 2026-10-19:** una prueba nueva reprodujo que
`windows_paths_equal` fusionaba dos rutas con unidades UTF-16 inválidas por convertirlas a texto
con U+FFFD. La comparación ahora usa `CompareStringOrdinal` directamente sobre UTF-16, tras
normalizar separadores. La regresión falló antes y pasó después; tests de caja y separadores añadidos.
En el mismo ámbito se corrigió el respaldo de Deshacer: compara la ruta `DeletedFrom` y el nombre
con reglas ordinales de Windows para tolerar caja/separadores equivalentes y evitar que una ruta
distinta seleccione un recibo incorrecto. La regresión cubre equivalencia y rechazos por carpeta o
nombre. Las 11 pruebas de Papelera y el gate (**1.483 aprobadas, 8 ignoradas**, Clippy y formato),
además de los nueve idiomas, pasan. Build supervisado concluido: Release 51m07s, Inno Setup
199,000 s y pico de 10,66 GiB. ZIP/setup verificados; no instalados ni firmados. El run remoto
`windows-2025` sigue pendiente; evidencia en
[`../VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md`](../VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md).

**Cierre:** workflow validado por ejecución real o registrado como pendiente hasta poder
ejecutarse; no publicar un tag/release para probarlo sin autorización.

## W-13/W-14 — aceptación y distribución

**W-13:** instalar desde `dist/` en entorno aislado autorizado; Windows 10/11,
DPI 100/150/200 %, teclado/AltGr/IME, Narrador, paneles y rutas angostas, conflictos,
scroll/rename, minimizar/bandeja, UNC caída, permisos/USB, cancelación de archivos grandes
y ciclos de preview. Usar matriz P1–P12 del plan de septiembre y pendientes de usabilidad.
Registrar probado, fallido o pendiente por entorno. Cada fallo nuevo vuelve a un bloque concreto.
La hoja con hashes de la entrega final, resultados parciales y casos manuales pendientes está en
[`docs/VALIDACION-WINDOWS-0.5.2-2026-10-15.md`](../VALIDACION-WINDOWS-0.5.2-2026-10-15.md). La
instalación actual coincide con el EXE final; la matriz interactiva aún no se completó.

**Avance local W-13, 2026-10-18:** el preview textual de solo lectura y el `TextInput` genérico de
crear/renombrar/comprimir ya declaran nombres accesibles localizados. Check focal de UI y gate
completo aprobados; quedan por confirmar en UI Automation y Narrador con la distribución actual
en una sesión desbloqueada. Las etiquetas defensivas no identifican el `Edit` anónimo del binario
anterior.

**Revalidación W-13, 2026-10-18:** la auditoría Slint añadió etiquetas accesibles a campos de
configuración, búsqueda, favoritos, recetas, consultas guardadas y otros diálogos, reutilizando
etiquetas localizadas sin incluir rutas ni valores. El gate pasó con 1.455 pruebas y 8 ignoradas,
Clippy y formato; el build supervisado produjo los ZIP/setup 0.5.2 verificados (pico 10,53 GiB,
release 28m32s, Inno 128,218 s). Con autorización previa, se actualizó in-place: PID 23868 responde,
su EXE coincide con Release y los cuatro JSON más el PDB anterior conservaron hashes. UIA pasiva
expuso 330 descendientes, 239 nombrados y 46/46 enfocables nombrados, pero los diálogos estaban
cerrados y Windows seguía bloqueado. Requiere Narrador/UIA de campos individuales y revisión visual
en sesión desbloqueada; evidencia y hashes en `docs/VALIDACION-UIA-2026-10-18.md`.

**Hallazgo adicional W-13, 2026-10-18:** una revisión estática encontró botones de toolbar y de
unidades dibujados con `Rectangle`/`TouchArea` que no exponían rol, nombre ni acción al árbol de
accesibilidad. Ahora publican semántica de botón y una acción accesible; la expulsión de USB expone
su acción separada. Falta confirmar el árbol real UIA y activación con Narrador en la sesión manual.
El gate posterior pasó con 1.458 pruebas y 8 ignoradas, Clippy y formato correctos. El build
supervisado regeneró portable y setup 0.5.2 con hashes verificados (pico 10,54 GiB); no se instaló.
No cierra la aceptación manual de W-13; ver `docs/VALIDACION-W13-TOOLBAR-2026-10-18.md`.

**W-14:** Nicolás eligió evaluar SignPath Foundation para OSS. El empaquetador ya puede firmar y
verificar el EXE antes del portable/setup con un certificado del store local; la acción de release
puede solicitar firma remota para el EXE y el setup, verificar el resultado y recalcular hashes.
El workflow usa `scripts/NaygoSigning.psm1` para exigir estado Authenticode `Valid` y nombre simple
del firmante exactamente igual (case-sensitive) a `SignPath Foundation`, en cada artefacto firmado;
registra el sujeto, emisor y thumbprint para auditar el certificado del primer run real.
`scripts/test-signing.ps1` crea certificados sintéticos para probar el publicador previsto, rechazar
otro publicador y rechazar una diferencia de mayúsculas; CI ejecuta esta prueba. La primera firma real
sigue pendiente de aprobación/configuración externa.
Se añadieron dos configuraciones XML revisables bajo `signpath/`; ambas restringen ProductName a
`Naygo` y ProductVersion a la versión actual de Cargo.toml, que el workflow pasa como parámetro.
`docs/POLITICA-DE-FIRMA.md` prepara el alcance, privacidad y responsabilidades requeridas; README y
las notas de futuras releases ya usan el rótulo visible “Code signing policy”. La release pública
`v0.3.0` aún no tiene ese enlace y la política local aún no se publica en el repositorio remoto.
**Revalidación local 2026-10-16:** `scripts/test-signing.ps1` pasó identidad SignPath Foundation,
rechazo de publicadores distintos/mayúsculas y rechazo de un archivo sin firma; `python
scripts/check_action_pins.py` confirmó los pins actuales. `gh run list` aún no muestra una release
que pruebe la integración SignPath; la última ejecución visible sigue en septiembre. No se publicó
ni se solicitaron credenciales externas.
La etiqueta oficial `v3` de `signpath/github-action-submit-signing-request` resuelve directamente al
commit [`f6d04783b4569d051e0c80105fe66e82819d0092`](https://github.com/SignPath/github-action-submit-signing-request/commit/f6d04783b4569d051e0c80105fe66e82819d0092),
verificado por GitHub API el 15 de octubre. El workflow fija ambas solicitudes al SHA inmutable
de ese commit. También se leyó el `action.yml` de ese commit por GitHub API:
los nombres de inputs de ambas solicitudes del workflow coinciden con el contrato (`github-artifact-id`,
`artifact-configuration-slug`, `parameters`, `wait-for-completion` y `output-artifact-directory`).
Antes de solicitar SignPath se debe publicar y confirmar que la página de release aplicable muestra
el enlace, revisar los roles con los mantenedores reales y activar MFA.
Inno Setup 6.7.3 deja espacios ASCII finales en los campos textuales PE del setup. Una compilación
aislada con `VersionInfoProductName` y las dos versiones fijadas explícitamente conservó padding
(`ProductName` longitud 60, `ProductVersion` 50); por eso cambiar esas directivas no resuelve el caso.
El helper ahora compara tras quitar únicamente espacios ASCII finales, rechaza cualquier otro
desajuste y el build/workflow aplica el chequeo antes y después de las solicitudes. La primera
respuesta SignPath aún debe confirmar normalización equivalente antes de habilitar firmas. Revisión
de documentación oficial, 2026-10-16: la referencia
de artifact configuration enumera `product-name` y `product-version` como restricciones PE y los
ejemplos usan valores concretos, pero no documenta si el comparador recorta espacios finales. Sigue
sin estar probado que `product-name="Naygo"` / `product-version="${version}"` acepte el setup
generado por Inno Setup. Resolverlo con una muestra/configuración en el proyecto SignPath antes de
habilitar solicitudes reales; no relajar las restricciones ni reescribir recursos PE a mano. Tests
locales pasan para el padding observado y rechazan nombre/versión erróneos; esto no simula el
comparador remoto. Fuentes:
[referencia oficial](https://docs.signpath.io/artifact-configuration/reference) y
[ejemplos oficiales](https://docs.signpath.io/artifact-configuration/examples).
El empaquetador local sólo tolera
`NotSigned` sin certificado o una firma ya `Valid`; HashMismatch/NotTrusted/UnknownError abortan el
paquete. W-14 sigue abierto hasta que el
proyecto sea aceptado/configurado en SignPath, se publique la política de firma requerida y una
ejecución real con aprobación manual produzca y verifique ambos artefactos. Sin habilitación de
SignPath, las distribuciones siguen declarándose sin firma; no se inventa identidad ni se manipulan
claves privadas.

**Preparación externa 2026-10-16:** `docs/POLITICA-DE-FIRMA.md` ahora enumera el proyecto,
Trusted Build System `GitHub.com`, origin verification, los dos XML de artefacto y las variables/
secreto exactos que consume el workflow. Hallazgo por confirmar con SignPath antes de habilitarlo:
el release workflow parte de tags `v*.*.*`, mientras origin verification documenta `Allowed branch
names`; verificar qué referencia permite la política. Se registró como riesgo de configuración, sin
cambiar el trigger ni habilitar firmas.

**Revisión de elegibilidad, 2026-10-15:** los [términos oficiales de SignPath Foundation](https://signpath.org/terms.html)
requieren que el proyecto ya esté publicado en la forma que se firmará. La release pública
[`v0.3.0`](https://github.com/nicolasgroth/explorador_archivos_naygo/releases/tag/v0.3.0) ya documenta
portable ZIP e instalador, por lo que el requisito de formato parece satisfecho; eso no garantiza la
aceptación de Naygo. La política local aún debe estar enlazada desde la release aplicable y la
postulación confirmar el resto de los criterios.

## Gate común por entrega de producto

1. Tests de regresión del ID y suite `cargo test --workspace --no-fail-fast --locked --jobs 1`.
2. `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked --jobs 1 -- -D warnings`;
   paridad de los diez idiomas con `scripts/check_i18n_lang.py`; `git diff --check`.
3. Si hubo código: `graphify update .`, revisar salida; no reconstruir grafo por la fuerza
   como supuesto requisito para corregir un bug pequeño. Anotar su advertencia si persiste.
4. Código con headers/responsabilidad/límites/cancelación; guía + backlog + CHANGELOG + estado
   cuando corresponda. No anunciar pendientes como funciones disponibles.
5. `scripts/build-release-guarded.ps1` invocando `scripts/build-release.ps1`, SOLO,
   ThinLTO/16 unidades/jobs=1, supervisor vigente. No subir límites automáticamente si falla.
6. Comprobar portable e instalador, hashes, versión/autoría, contenido ZIP sin PDB y PDB opcional
   en setup. Preservar entrega anterior hasta confirmar integridad de la nueva.
7. Registrar aceptación manual pendiente, presentar resultado a Nicolás y esperar siguiente ID.

Una entrega sólo documental verifica Markdown/enlaces/diff y no recompila el producto.

## Prompt de ejecución por bloque

```text
Ejecuta únicamente el bloque <ID> de docs/plans/2026-10-13-mejoras-y-ejecucion-luna.md.
Lee AGENTS.md, BACKLOG, ESTADO-ACTUAL y el bloque. Usa GPT-6 Luna.
Preserva cambios locales y preferencias. Confirma la evidencia del problema y las
dependencias del ID. Implementa el mínimo cambio completo, con pruebas específicas
de fallo/cancelación/identidad y documentación. Cumple el gate común y deja actualizado
dist/ si cambia la app. No publiques, instales sobre mi sesión ni empieces otro bloque.
Si una decisión no está cerrada, explícala y consulta antes de su implementación.
Al terminar informa qué cambió, evidencia, límites y el siguiente ID recomendado.
```

## Seguimiento

Registrar por ID: estado, diff identificado, pruebas/logs, build/hashes y aceptación. W-00 a
W-06E están implementados, probados y empaquetados. W-07 tiene el benchmark sintético y
la instrumentación opt-in empaquetados, además de un smoke portable actualizado, pero las mediciones
reales siguen pendientes. M-01 está
implementado, probado y empaquetado.
La aceptación instalada (W-13)
y la firma con certificado (W-14) requieren intervención externa/autorización específica.

**Seguimiento W-13, 2026-10-16:** se añadieron roles UIA/AccessKit a la barra custom de pestañas,
selección accesible y navegación ←/→ + Enter/Espacio. El test headless, suite, Clippy y formato
pasaron; la distribución 0.5.2 se regeneró con el gate supervisado. Hashes y límites están en
[`docs/VALIDACION-BUILD-2026-10-16.md`](../VALIDACION-BUILD-2026-10-16.md). La build nueva quedó
empaquetada pero no instalada; Narrador, aceptación manual de foco y matriz W-13 siguen pendientes.

**Revalidación W-07/W-13, 2026-10-18:** la distribución E-5 se instaló con autorización previa y
quedó abierta como PID 51408 (EXE `681b99ae…`). UIA de solo lectura expuso 721 descendientes, 536
con nombre y 55 enfocables, todos estos con nombre accesible. Tres ventanas pasivas de 30 s dieron
76,7–77,9 MiB working set, 45,5–46,7 MiB privados, 385–387 handles, 26–27 hilos y CPU delta
0,032/0,359/0 s. La muestra comenzó 79 s después del arranque; no se controló inactividad ni se
midió input, foco, Narrador, renderer Winit o respuesta. No cierra W-07/W-13. JSON y límites en
[`docs/BENCHMARKS.md`](../BENCHMARKS.md).

**Revalidación W-07/W-13, 2026-10-18 04:37–04:39:** la instalación E-4 actual es PID 25124 y su
SHA-256 coincide con `target/release/naygo.exe`. UIA volvió a encontrar 726 descendientes, 539 con
nombre y 55 enfocables (todos nombrados), sin truncamiento. Con la pantalla de bloqueo visible
inmediatamente antes y después, tres ventanas pasivas de 30 s registraron 78,5 MiB working set,
47,0 MiB privados, 387 handles, 27 hilos y CPU 0,015/0/0,047 s; sólo se observó Naygo y no había
SMB. Es evidencia de recursos en escritorio bloqueado, no de entrada, latencia, presentación o
Narrador. W-07 conserva interacción física, Winit y matriz local/UNC; W-13 conserva Narrador y
aceptación manual. Registro y límites en [`docs/BENCHMARKS.md`](../BENCHMARKS.md).

**Reinspección UIA W-13, 2026-10-18:** el inspector ahora cuenta tipos enfocables sin nombre,
sin registrar el nombre, la ruta ni el valor del control. En PID 25124 halló un `ControlType.Edit`
enfocable sin nombre (732 descendientes; 56 enfocables, 55 nombrados). No se atribuye a un control
Slint concreto: el proceso ejecuta un EXE anterior al release actual y `LogonUI` seguía activo.
Repetir con el release actual cuando la sesión esté desbloqueada; verificar con Narrador antes de
decidir si se requiere un cambio de etiqueta. Evidencia: [`docs/VALIDACION-UIA-2026-10-18.md`](../VALIDACION-UIA-2026-10-18.md).
