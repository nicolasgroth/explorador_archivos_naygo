# Plan — usabilidad y confiabilidad de navegación Windows

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-11-09. **Diagnóstico estático y propuesta para revisión; no autoriza implementación.**

## Base y límites

Rama `codex/avance-0.5.4-20261031`, documentación `b6120ae1`, producto `af2c64f3`; 0.5.5 instalada según [estado actual](../ESTADO-ACTUAL.md). CI Windows registró 1.575 pruebas aprobadas, 9 ignoradas, Clippy y formato; no se ejecutaron pruebas ni builds para este diagnóstico. La distribución y preferencias no se tocaron. [0.5.2](../VALIDACION-0.5.2.md) ya documentó selección tras crear, preservación al navegar y reconciliación de watchers; su aceptación instalada completa quedó pendiente. Los síntomas nuevos no demuestran por sí solos una regresión.

Este plan complementa [rutas/preview](2026-09-09-rutas-preview-y-configuracion.md) y [cierre Windows/OLE](2026-11-02-cierre-windows-arrastre-y-validacion.md), sin reabrir cierres ni sustituir sus pendientes. Windows primero; UI sin I/O, streaming, cancelación, bajo consumo y filesystem hostil siguen siendo invariantes.

## Diagnóstico: evidencia y preguntas

- **Crear y activar:** el diálogo captura destino, procesa líneas y cierra antes de recibir resultados ([context.rs:122](../../crates/ui-slint/src/workspace_ctrl/context.rs#L122), [context.rs:203](../../crates/ui-slint/src/workspace_ctrl/context.rs#L203)). Al terminar se seleccionan éxitos en paneles que aún muestran la ruta y se relista; en anidadas se revela el primer nivel ([listing.rs:1450](../../crates/ui-slint/src/workspace_ctrl/listing.rs#L1450)). Hay pruebas simuladas de selección y de no robar foco tras navegar ([tests_simular.rs:863](../../crates/ui-slint/src/workspace_ctrl/tests_simular.rs#L863), [tests_simular.rs:986](../../crates/ui-slint/src/workspace_ctrl/tests_simular.rs#L986)), no evidencia de Enter/foco real en Slint. El modal usa Enter para salto de línea y Ctrl+Enter o doble Enter rápido para crear ([new-folder.slint:79](../../crates/ui-slint/ui/new-folder.slint#L79)): posible desajuste de expectativa, no causa demostrada. Cada línea inicia una operación separada y `reveal_after_listing` reemplaza el contexto pendiente y enfoca la primera ruta ([file_pane.rs:638](../../crates/core/src/workspace/file_pane.rs#L638)); que resultados en tandas distintas sobrescriban una selección posterior es una hipótesis a probar con barreras. Abrir el diálogo consulta `is_dir()` en el controlador ([context.rs:125](../../crates/ui-slint/src/workspace_ctrl/context.rs#L125)): riesgo de bloqueo en rutas lentas que debe medirse y retirarse de UI con cuidado.
- **Enter y selección:** Activate abre la fila enfocada ([input.rs:664](../../crates/ui-slint/src/workspace_ctrl/input.rs#L664)); Ctrl+clic puede dejar foco en una fila deseleccionada ([file_pane.rs:744](../../crates/core/src/workspace/file_pane.rs#L744)). En cambio, operaciones prefieren seleccionadas y usan la enfocada si no hay selección ([ops.rs:111](../../crates/ui-slint/src/workspace_ctrl/ops.rs#L111)). La divergencia es observable en código; falta reproducirla en la app y acordar contrato.
- **Árbol y refrescos:** el árbol revela centrando al cambiar índice solicitado, ruta o altura ([tree-panel.slint:122](../../crates/ui-slint/ui/tree-panel.slint#L122)); el índice deriva de rutas aplanadas ([tree.rs:20](../../crates/ui-slint/src/workspace_ctrl/tree.rs#L20)). Una expansión o reconstrucción anterior puede cambiarlo sin cambiar la carpeta activa: hipótesis verificable de salto de viewport, no explicación probada de OneDrive en otra unidad. Files reinicia scroll al cambiar `nav-key` ([file-panel.slint:352](../../crates/ui-slint/ui/file-panel.slint#L352)); distinguir ambos scrolls. Árbol dedicado navega su panel enlazado y el común usa el último Files activo ([tree.rs:54](../../crates/ui-slint/src/workspace_ctrl/tree.rs#L54)); expansión y navegación tienen callbacks separados ([callbacks_history.rs:63](../../crates/ui-slint/src/callbacks_history.rs#L63)). Watchers pueden provocar relist y reconstrucción del árbol ([session.rs:335](../../crates/ui-slint/src/workspace_ctrl/session.rs#L335), [session.rs:447](../../crates/ui-slint/src/workspace_ctrl/session.rs#L447), [tree.rs:396](../../crates/ui-slint/src/workspace_ctrl/tree.rs#L396)); existen generaciones para descartar lotes antiguos ([watch.rs:125](../../crates/ui-slint/src/watch.rs#L125), [watch.rs:186](../../crates/ui-slint/src/watch.rs#L186)). Esto no demuestra carrera. Comparación por padre actual sin canonicalización ([session.rs:368](../../crates/ui-slint/src/workspace_ctrl/session.rs#L368)); normalización, junctions, redirecciones y respuestas antiguas son hipótesis a comprobar, no causas establecidas.
- **OLE:** consulta UI antes de `GetData`, con timeout y sin fallback al panel activo ([drop_target.rs:985](../../crates/platform/src/drop_target.rs#L985), [tick.rs:340](../../crates/ui-slint/src/tick.rs#L340)). La ventana entre eventos de ratón encolados y consulta no está sincronizada atómicamente: limitación real documentada, no caso reproducido ni causa demostrada del problema de navegación. Se contrasta panel/ruta al entregar y se solicita destino si cambió ([tick.rs:403](../../crates/ui-slint/src/tick.rs#L403)); el selector revalida ([layout_panes.rs:1024](../../crates/ui-slint/src/layout_panes.rs#L1024)). Una solicitud nueva con probe ocupado se descarta ([ops.rs:601](../../crates/ui-slint/src/workspace_ctrl/ops.rs#L601)); comprobar feedback y no suponer cola. Cancelar no interrumpe una llamada COM bloqueada ([drop_target.rs:1328](../../crates/platform/src/drop_target.rs#L1328)). Staging y publicación protegen archivos ([engine.rs:96](../../crates/core/src/ops/engine.rs#L96), [engine.rs:265](../../crates/core/src/ops/engine.rs#L265)); no implican transacción total de una operación multítem.

## Decisiones de UX para revisar con Nicolás

1. Enter con selección múltiple: propuesta conservadora, abrir solo la fila enfocada **si pertenece a la selección**; si no, pedir elección o definir una regla explícita. Alternativas: abrir todas, preguntar siempre o distribuir a paneles. Abrir muchas puede lanzar programas y distribuir altera layout; preguntar siempre interrumpe teclado. No implementar sin elegir, incluyendo cero/una selección y foco deseleccionado.
2. Crear una o varias carpetas: acordar selección, foco y aviso de éxito/error parcial; comportamiento ante filtros que ocultan resultados, creación anidada y cambio de ruta/panel antes de completar. No robar foco a quien ya navegó. Acordar si Enter del modal debe crear o mantener multilinea y cómo hacer visible el atajo.
3. Árbol: propuesta de preservar ancla de viewport por identidad durante refresco/expansión y revelar la ruta solo ante una intención explícita de navegación. Trade-off: la carpeta activa podría quedar fuera de vista; acordar cuándo revelarla y distinguir expansión de apertura.
4. Drop: mantener destino visible e inequívoco, sin redirigir silenciosamente. Decidir tratamiento de drop concurrente/rechazado y feedback; confirmar que subcarpetas requieren navegación previa o diseñar aparte otra interacción.

## Bloques priorizados — uno por aprobación

**U0 — Reproducción e instrumentación mínima.** Con perfil y fixtures aislados, anotar versión/hash, panel activo/enlazado, rutas representativas, foco, selección, viewport, secuencia exacta y resultado esperado/observado. Reproducir crear→Enter, selección múltiple/Ctrl+clic, árbol abrir frente a expandir, carpeta profunda → OneDrive en otra unidad y vuelta. Distinguir salto de árbol, Files y ruta efectiva; repetir con watcher activo/inactivo cuando sea posible. Si no reproduce, registrar condiciones y no inventar causa. Añadir solo diagnóstico local opt-in y acotado si es necesario, sin nombres/rutas privadas ni telemetría; no bloquear UI. Registrar tiempos de respuesta y base de comparación antes de optimizar.

Avance 2026-11-10: Nicolás aprobó iniciar U0 y pidió puntos de control para administrar el
consumo de contexto. La revisión nativa no pudo comenzar: Computer Use identificó una ventana
Naygo instalada, pero la consulta de accesibilidad no entregó árbol ni foco y dos capturas,
con reidentificación entre ellas, expiraron (`FrameArrived timed out` y `window capture timed out`).
No hubo interacción ni se reprodujo ningún caso. El ejecutable instalado coincide con el SHA-256
de 0.5.5 consignado en `ESTADO-ACTUAL.md`; el commit consultado es `b6120ae1`, con cambios
documentales locales preexistentes. La instancia única usa un mutex por sesión: abrir otra copia
sin aislar la sesión podría enviar una solicitud a la instancia existente. No se inició otra copia,
ni se alteró la sesión, ni se midió latencia. U0 sigue pendiente; se necesita una sesión visual
funcional y un perfil/fixture aislados antes de registrar resultados de interacción.

Observación de Nicolás, 2026-11-10: en el diálogo, Enter agrega un salto de línea; un segundo
Enter confirma la creación, y Ctrl+Enter crea directamente. Tras crear, intenta entrar con Enter
en la carpeta recién creada, pero debe seleccionarla con el mouse para poder continuar. Es un
reporte de interacción real, no una reproducción automatizada ni una identificación de causa;
no se registraron panel, ruta, tiempo de espera ni si la fila aparecía seleccionada. La mejora
solicitada es conservar el contexto de teclado para poder entrar a la carpeta creada sin mouse.
El test `nav052_create_selects_after_completion_and_refreshes_tree_without_watcher` comprueba
foco y selección en el modelo al terminar, pero su gesto posterior es doble clic, no Enter real.
Investigar foco del widget y resultado/listado asíncrono sin asumir que sean la causa. También
evitar que un Enter temprano active una fila anterior mientras la creación sigue pendiente.

Evidencia adicional de Nicolás: una captura posterior muestra la nueva carpeta resaltada; indicó
que ni Enter ni las flechas responden hasta volver a hacer clic. Esto apunta a la entrega de foco
de teclado tras el modal, sin demostrar aún el mecanismo exacto. Solicitó también crear carpetas
desde el árbol. El menú contextual de una carpeta del árbol ya conecta con «Nueva carpeta» y toma
la ruta de esa fila; el árbol no reenvía el atajo general de creación al keymap. Validar ambos
flujos con destino explícito y sin redirigir al último panel Files por accidente.

Avance de implementación 2026-11-10: se devuelve el foco al panel Files o árbol activo al cerrar
el modal, y el atajo configurable desde árbol usa la carpeta de su cursor. Se limpia la selección
anterior al solicitar la creación en el Files que muestra el destino. La prueba Slint reprodujo
primero la pérdida de flechas y pasó tras el cambio; cubre también foco del árbol y envío del
atajo. Dos pruebas del controlador cubren destino del árbol y Enter inmediato antes/después del
resultado; cinco pruebas `nav052_` pasaron. Gate completo: 1.578 aprobadas, 9 ignoradas, Clippy,
formato y diez catálogos correctos; pico supervisado 10,80 GiB. Esto no valida intercalados de
watcher ni aceptación nativa instalada. Tras el gate se ajustó el árbol para no consumir teclas
no reconocidas: se interrumpió el primer build y se repitió el gate final con 1.578 aprobadas,
9 ignoradas, Clippy y formato correctos y pico de 10,88 GiB. Build final completado y paquetes
verificados; setup y portable aún no instalados, con hashes en `ESTADO-ACTUAL.md`. No se modificaron
los gestos Enter/Ctrl+Enter del modal. Aceptación interactiva y carreras de watcher siguen pendientes.

Continuación: una prueba determinista demostró que un listado iniciado antes de crear puede
restaurar el foco anterior y Enter abrir esa fila antes de reconciliar. La protección nueva es
efímera y por panel: mantiene Enter sin activar hasta procesar resultados y terminar el listado;
una elección explícita con clic, flechas o tipeo la descarta. Se limpia al navegar, cerrar el panel
o reemplazar el workspace. Enter no se encola y aún falta feedback explícito cuando queda bloqueado.
Nueve pruebas dirigidas pasaron (incluidas cola, cancelación y cambio de layout); gate del workspace:
1.587 aprobadas, 9 ignoradas, Clippy y formato correctos, pico supervisado 8,51 GiB. Build final en
`dist` verificado; la aceptación en una ventana instalada y otros intercalados de watcher siguen pendientes.

Nuevo avance: se reprodujo que un resultado tardío sobrescribía una selección hecha durante la
creación. La reconciliación conserva esa elección y el watcher sigue resaltando los destinos de la
última creación sin seleccionarlos; entradas ajenas conservan su política anterior. Se actualiza el
contexto pendiente si la elección ocurre durante un listado. El seguimiento retiene hasta 4.096
destinos por panel; sobre el límite preserva la elección ante todas las llegadas de esa carpeta
hasta navegar o iniciar otra creación. La pulsación de Enter bloqueada muestra aviso localizado y
no queda en cola. Diecisiete pruebas dirigidas y el gate completo pasaron: 1.595 aprobadas,
9 ignoradas, Clippy/formato y paridad de diez idiomas; pico supervisado 10,70 GiB. Build Release
supervisado y paquetes verificados el 2026-11-11, con hashes en `ESTADO-ACTUAL.md`; no instalados.
Falta aceptación nativa, revisión editorial de traducciones y probar creaciones separadas superpuestas
y otros intercalados de watcher.

Continuación 2026-11-11: abrir el cuadro desde Files ya no llama `is_dir` en UI. El worker de
creación comprueba la carpeta base y observa cancelación al volver de la consulta; una consulta
bloqueada no se interrumpe y existe una carrera entre comprobar y crear. Desde el menú contextual
de árbol/breadcrumb se conserva el destino capturado aun si desaparece, sin fallback a otro panel.
Pasaron 11 pruebas dirigidas de núcleo, 19 de UI y el gate de 1.602 pruebas, 9 ignoradas, Clippy y
formato (pico 7,43 GiB); build supervisado completado y paquetes verificados, no instalados, con
hashes en `ESTADO-ACTUAL.md`. La consulta síncrona al abrir el menú de zona vacía y las usadas
por terminales son flujos independientes que continúan pendientes.

Continuación: abrir el menú de zona vacía toma la ruta del estado del panel sin consultar disco.
Una prueba reproduce la carpeta base ausente y comprueba que abrir el menú funciona y que el
worker rechaza crear dentro sin recrear la base. Terminal y Explorador aún requieren tratamiento
independiente de validación, ejecución Shell y errores.
Nuevo avance: terminal y Explorador capturan destino del menú o panel y lo validan en un worker
antes de lanzar Shell; en Windows inicializa COM STA. Una solicitud en vuelo, rechazo informado
cuando está ocupada y wake de UI al completar. Las pruebas dirigidas cubren ruta ausente, cambio
de panel, límite y wake. Las llamadas bloqueadas del SO no se interrumpen; el chequeo previo y
el lanzamiento no son atómicos. Validación nativa y rutas remotas siguen pendientes.
Gate supervisado: 1.608 aprobadas, 9 ignoradas, Clippy/formato correctos. Build y paquetes
comprobados en `dist`; hashes en `ESTADO-ACTUAL.md`.
Gate: 1.604 pruebas aprobadas, 9 ignoradas, Clippy y formato correctos; build supervisado y
paquetes comprobados, no instalados. Hashes y límites en `ESTADO-ACTUAL.md`.

**U1 — Creación, foco y Enter.** Tras acordar UX, corregir un flujo pequeño a la vez: resultado/selección por identidad y feedback, luego contrato Enter y teclado del modal. Retirar I/O de la UI con validación asíncrona, capacidad acotada y descarte de respuestas obsoletas; error y cierre/cambio de panel explícitos. Pruebas deterministas: éxito/fallo parcial, anidadas, filtro, cambio/cierre de panel, resultado tardío, intercalado listado/watcher y foco movido; cero/una/múltiples y foco fuera de selección. Manual Windows: teclado real, Ctrl+Enter, Enter y doble Enter, foco y feedback; ruta lenta/caída sin congelar. Aceptación: acción sobre objetivo acordado, sin navegación ni foco robados, feedback verificable y UI receptiva.

Nicolás confirmó el flujo de crear carpeta, Enter y flechas. Reportó pérdida de foco al confirmar
borrado y cierre anticipado del submenú Abrir. Pruebas Slint reprodujeron ambos: se restaura foco
del panel activo tras diálogos de operaciones/avisos y se ubica el submenú en una zona que recibe
puntero, con apertura al otro lado junto al borde. Cubiertos confirmación, cancelación, árbol,
superposición y clic de puntero a izquierda/derecha; pendiente aceptación nativa y auditoría de
otros diálogos con gestores de foco propios.
Continuación: reproducidos la pérdida de foco tras reintento y Escape inoperante al abrir el
detalle de operación. Se coordinan otros overlays para devolver teclado al panel activo y no
robarlo mientras siga abierto un diálogo; el detalle toma foco al abrir y tras cerrar reintento.
Prueba Slint cubre paleta, reintento, destinos, espacios, consultas, recetas, comparaciones,
entregas y detalle, además de cambio de panel. Quedan pendientes aceptación nativa, auxiliares,
diálogos nativos y editores de ruta/rename superpuestos.
Continuación: reproducido que los editores de ruta del panel y barra superior pierden el foco
tras la paleta. El foco se restituye diferido sin reseleccionar; las pruebas comprueban texto y
cursor en medio antes/después, junto con las suites existentes. Renombrado, paneles auxiliares,
diálogos nativos y aceptación visual siguen pendientes.
Gate supervisado: 1.609 pruebas aprobadas, 9 ignoradas, Clippy/formato correctos (11,12 GiB de
pico). Primer build Release detenido preventivamente al caer memoria libre a 7,66 GiB; tras
liberar memoria se completó el reintento supervisado y se verificaron los paquetes en `dist`.
Gate final: 1.609 aprobadas, 9 ignoradas, Clippy/formato correctos, pico 11,04 GiB. Build
supervisado y paquetes comprobados en `dist`, no instalados; hashes en `ESTADO-ACTUAL.md`.
Gate final: 1.609 pruebas aprobadas, 9 ignoradas, Clippy/formato correctos; build supervisado
y paquetes verificados en `dist`, sin instalación ni aceptación nativa. Hashes en el estado actual.

**U2 — Navegación, árbol y refresco.** Aislar primero el salto reproducido; probar expansión anterior a fila activa, cambio de altura y reconstrucción con viewport desplazado. Simular A→B→C y A→B→A, respuestas de listado/árbol fuera de orden, cambio/cierre de panel y lotes de watcher tardíos; preservar ruta, selección y ancla conforme al contrato, sin perder cambios externos. Comprobar ruta lógica frente a identidad física, mayúsculas, UNC, junction/redirección y OneDrive disponible/offline sin canonicalizar en UI ni asumir equivalencia universal. Manual Windows: árbol común/dedicado, expandir vs abrir, carpeta profunda → OneDrive en otra unidad, refresco externo y retorno; registrar qué viewport salta. Aceptación: nunca volver a ruta previa por respuesta obsoleta, preservar contexto según UX y mantener streaming/cancelación; errores discretos y ninguna espera de disco en UI.

Avance 2026-11-11: un test Slint con scroll real reprodujo el recentrado al cambiar el índice
de la ruta activa y verificó la corrección. Cubre inserción/eliminación, reemplazo completo del
modelo, cambio de altura, nueva ruta incluso con igual índice y aparición tardía. No demuestra
conservación de identidad visual en reconstrucciones arbitrarias ni identifica la causa del caso
OneDrive en otra unidad, cuya reproducción y aceptación en Windows siguen pendientes.
Gate: 1.603 aprobadas, 9 ignoradas, Clippy/formato correctos, pico 10,99 GiB; build supervisado
y paquetes comprobados en `dist`. Ver hashes y límites en `ESTADO-ACTUAL.md`.

Continuación 0.5.6: el controlador pasó con OneDrive real en otra unidad desde un perfil
temporal. Se comprobó navegar desde una carpeta profunda a la raíz, refrescar y volver con
árbol común y dedicado. Una regresión verifica que se termina la revelación de un ancestro
ausente cuando el listado concluye. Quedan la aceptación visual y OneDrive desconectado.

**U3 — Destino OLE y operaciones.** Primero definir contrato de captura y efecto, luego reproducir con barreras controladas eventos de ratón pendientes, cambio de ruta/panel, cierre y reordenamiento antes/después de `GetData`, drops simultáneos, timeout/proveedor bloqueado y cancelación en cada frontera. Cubrir conflicto creado tras probe, permisos/destino desaparecido, ausencia de parciales propios y resultados multítem honestos; no afirmar interrupción de COM ni atomicidad global. Corregir solo carreras demostradas o contratos insuficientes y medir costo de sincronización; no bloquear UI ni crear workers ilimitados. Aceptación: destino capturado o elección explícita/rechazo informado, efecto coherente, origen preservado cuando corresponda y staging limpiado tras retorno/cancelación.

**U4 — Aceptación Windows y cierre.** Continuar la matriz existente, no repetir como nueva evidencia lo ya probado: 7-Zip 24.07 copió E2 en 0.5.5 (35 archivos, 4 directorios, cero diferencias SHA-256); las casillas se vieron, pero su selección/persistencia no se probó. Directory Opus tiene evidencia anterior de mover/cancelar; Explorer y adversos siguen pendientes. En Windows 10/11 según disponibilidad, probar Explorer/7-Zip/Opus, archivos y carpetas vacías/anidadas/mezcla/Unicode, origen y destino locales y distintos volúmenes/red disponibles, copia/movimiento/modificadores/cancelación/conflictos y opciones de confirmación por sesión/permanente con reinicio. Registrar versiones, fixtures, manifiestos/hashes de origen y destino, residuos, resultado y pendientes; nunca datos personales. Validar accesibilidad y DPI de foco/selección, además de respuesta y consumo con metodología [W-07](../BENCHMARKS.md). Compatibilidad general solo se declara con alcance y evidencia reales.

## Puertas y economía de trabajo

Nicolás autorizó el 2026-11-11 avanzar autónomamente en los bloques posibles. Cada entrega requiere pruebas dirigidas significativas, gate apropiado de workspace (incluye formato/Clippy e i18n cuando corresponda), comprobación nativa cuando sea viable y documentación de resultados y límites. Ante cambio de producto actualizar guía, backlog y CHANGELOG; actualizar grafo tras código. Regenerar `dist` con `scripts/build-release.ps1` **solo**, supervisor de memoria y configuración vigente; verificar artefactos. Si falta sesión manual, dejar aceptación pendiente sin declararla aprobada.

Política práctica de tokens: consultar graphify primero y acotar por símbolo; una pasada por agente con evidencia archivo/línea y síntesis compacta; lecturas focalizadas, deduplicar hallazgos y reutilizar evidencias ya registradas. No repetir búsquedas, suites ni builds sin cambio, fallo o incertidumbre que lo justifique. Las optimizaciones requieren mediciones comparables: primera fila, interacción y respuesta, scroll, CPU/memoria/colas; preservar límites y cancelación antes de añadir caché o sondeo.
