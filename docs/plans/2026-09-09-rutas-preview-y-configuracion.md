# Naygo — rutas, configuración y preview responsable

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-09-09. Alcance aprobado, **RP-1/RP-2/RP-3 implementados y validados automáticamente** sobre la base local 0.5.1
`abdef402babd2c136744c67ee9833c2f67bf0f16`. Consolidado del traspaso por petición de Nicolás.
Consultar [estado vigente](../ESTADO-ACTUAL.md) antes de actuar. No aplicar fuentes recuperadas
encima del proyecto ni confundir el intento experimental con una entrega funcional.

## Objetivo y no regresiones

Mejorar edición de rutas, claridad de configuración, preview cancelable y animaciones ligeras
sin sacrificar navegación ni consumo. Conservar tres crates, Slint/software, listado incremental,
modelos estables, teclado, docking, watchers, 7-Zip, Papelera, bandeja y flujos de trabajo existentes.

La excepción a «no reproducir media» se limita a imágenes animadas en el preview. No incluye
audio/vídeo, editor, FFmpeg, VLC, WebView o Chromium. No distribuir fuentes nuevas ni actualizar
dependencias en bloque. Las APIs deben comprobarse contra `Cargo.lock` y el toolchain real.

## Bloques y estado

| ID | Alcance | Estado / cierre exigido |
| --- | --- | --- |
| RP-0 | Inventario local, consolidación y recuperación | Completado. Artefacto recuperado y verificado en carpeta separada; sin aplicar scripts ni fuentes en bloque. |
| RP-1 | Edición de rutas de panel | Implementado; 1.255 pruebas y Clippy aprobados. Build 202609091710 generado/verificado; aceptación nativa/DPI/red pendiente. |
| RP-2 | Ruta superior | Implementado; 1.257 pruebas, Clippy e idiomas aprobados. Build 202609091819 generado/verificado; aceptación instalada pendiente. |
| RP-3 | Temas y configuración | Implementado; 1.263 pruebas, Clippy, idiomas y render de quince temas aprobados. Build 202609091919 generado/verificado; aceptación nativa pendiente. |
| RP-4 | Política y ciclo de vida del preview | RP-4A/RP-4B1 y retención RP-5B entregados. RP-4B2: texto/3D aislados, protocolo acotado y temporales entregados; 1.324 pruebas/Clippy y siete pruebas release aprobadas, build 202609142135 verificado. Aceptación nativa y medición de RSS quedan pendientes. |
| RP-5 | Animaciones | RP-5A/RP-5B entregados: WebP/GIF/APNG, composición/timeline, pausa, cola limitada y calidad económica configurable. 1.317 pruebas y build 202609100757 verificados. Aceptación instalada pendiente. |
| RP-6 | Lectores e instrumentación | Pendiente. PDF/TAR/texto/SVG, métricas opt-in y drenado de listado medido. |
| RP-7 | Entrega y aceptación | RP-1/RP-2/RP-3/RP-4A/RP-4B1/RP-5A/RP-5B/RP-4B2: suite, documentación e instalador/portable verificados. Último build 202609142135, ThinLTO supervisado. Aceptación instalada, medición global y RP-6 pendientes. |

Cerrar cambios pequeños y comprobables, sin otra reimplementación masiva. Las animaciones
dependen de RP-4. RP-1–3 aprovechan componentes existentes. El plan no cancela pendientes de
aceptación del [plan anterior](2026-09-04-usabilidad-y-flujos-de-trabajo.md).

## RP-1 / RP-2 — rutas

- Mantener acciones Editar, Copiar y Favorito accesibles aunque la ruta sea larga; botón
  explícito de edición, ancho reservado y menú predecible para acciones secundarias.
- Reutilizar un editor de una línea con desplazamiento horizontal; separar ruta confirmada,
  borrador y representación recortada. No reemplazar el valor real por lo visible.
- Respetar caret, selección, pegar, deshacer y edición en medio. Eliminar concatenación al
  final del caso especial `\`. Probar teclado chileno, AltGr y, si procede, IME.
- Enter valida sin bloquear la UI. Error conserva carpeta anterior, borrador y mensaje inline;
  Escape descarta. No añadir al historial/favoritos rutas sin confirmar.
- Capturar el ID del panel al comenzar edición superior. Cambio de foco no redirige la acción;
  cierre del panel cancela o informa. Invalidar validaciones y sugerencias obsoletas por revisión.
- Resolver rutas relativas respecto del panel, comillas externas, espacios, Unicode, UNC y
  prefijos extendidos sin reemplazos ad hoc ni interpretación como comando de shell.
- Autocompletado con debounce, límites y consultas fuera de UI en TODAS las vías, incluido
  abrir el editor y elegir sugerencia. Acotar workers bloqueados, no sólo respuestas pendientes.
- Ctrl+L/F4 usan el keymap existente. Alt+D sólo si no entra en conflicto. No tomar Ctrl+D.
  Con foco de edición, las acciones globales no deben consumir teclas del editor. Tooltips
  muestran atajo efectivo. Ctrl+C copia la selección; los botones usan la carpeta confirmada.

Base: `callbacks_pathbar.rs`, `workspace_ctrl/input.rs`, `file-panel.slint`,
`breadcrumb-bar.slint` y `app-window.slint`, bajo `crates/ui-slint/`.
`path-editor.slint`, `path_input.rs` y `path_window_events.rs` citados por el intento
no deben asumirse presentes ni funcionales: verificar al recuperar.

## RP-3 — apariencia y configuración

Paletas aprobadas e integradas selectivamente en RP-3: Nieve (`isgroth-nieve`), Perla
(`isgroth-perla`), Arena (`isgroth-arena`), Salvia (`isgroth-salvia`), Bruma
(`isgroth-bruma`) y Grafito claro (`isgroth-grafito-claro`). JSON originales recuperados,
sin recolorear la paleta; sólo se añadió `icon_accent` desde su `accent` original.
[Procedencia y validación RP-3](../VALIDACION-RP3-2026-09-09.md).

Conservar los nueve temas existentes y el tema elegido. Validar textos principales/secundarios,
foco, selección activa/inactiva, filas recientes, íconos tintables y multicolor, campos,
tooltips, toolbar, scrollbars y estados deshabilitados. Una galería HTML no valida Slint.

Separar responsabilidades: Apariencia (temas, fuentes, densidad), Íconos (catálogo, objetos,
sets/tamaños) y Previsualización (reglas, calidad, límites y animación). Retirar la galería
duplicada de Apariencia, dejando enlace a Íconos; conservar IDs y callbacks persistidos.
Separar color de íconos del fondo de filas recientes: revisar usos de `Theme.highlight`,
con defaults compatibles para temas antiguos, sin forzar preferencias nuevas.

## RP-4 / RP-5 — ciclo de vida y animación

Continuación del 14 de septiembre: RP-4B2 extiende el supervisor a texto/resaltado y 3D,
conserva la escena para giro sin I/O, limita transporte y scratch y reserva un margen
temporal compartido. Ver [alcance/validación RP-4B2](../VALIDACION-RP4B2-2026-09-14.md).
No presentar estos presupuestos como techo de memoria de toda la aplicación. Los
párrafos siguientes documentan los cortes anteriores, no revierten lo ya implementado.

Entrega incremental RP-4A: cierra cancelación inmediata, estados terminales/reintento,
retención de plazas hasta terminar el hilo, suspensión por visibilidad y cámara pendiente
única. No añade procesos ni presenta la cancelación cooperativa como terminación real.
RP-4B conserva ese trabajo y los presupuestos completos; las animaciones siguen en RP-5.
Este corte permite validar la navegación y el 3D antes de introducir aislamiento y medir
su costo. [Registro RP-4A](../VALIDACION-RP4A-2026-09-09.md).

RP-4B1 continúa con lectores pesados aislados, sin imponer proceso a cada gesto 3D ni
presentar 192 MiB por hijo como consumo total. [Alcance/evidencia](../VALIDACION-RP4B1-2026-09-09.md).
La muestra WebP aportada tiene 50 frames y 5 segundos por ciclo: se utiliza sólo en lectura,
no se distribuye; comprobar su poster no certifica composición ni reproducción animada.

### Formatos y fidelidad

RP-5B implementa calidad económica configurable y retención compartida; validación en
[registro RP-5B](../VALIDACION-RP5B-2026-09-10.md). No confundir presupuesto de contenido
retenido con límites completos del proceso/temporales de RP-4 ni con mediciones de RP-6.

RP-5A implementa reproducción secuencial WebP/GIF/APNG, controles y reloj dedicado;
validación/distribución registrada en [RP-5A](../VALIDACION-RP5A-2026-09-09.md).
No cierra presupuestos globales RP-4, calidad económica configurable ni aceptación
instalada de toda la matriz. No se activó `.wepb` por defecto.

WebP animado, GIF y APNG (detectado en `.png`/`.apng`). Los estáticos mantienen una ruta
sin reloj periódico residual. `.wepb` es alias opcional, sin renombrar archivos; comprobar
contenido y respetar reglas deshabilitadas, mayúsculas y modos forzados. PNM/PBM/PGM/PPM/PAM
y TGA son candidatos a evaluar, no prestaciones comprometidas sin medir features/costo.
No ampliar por defecto a HEIC, AVIF animado ni audio/vídeo.

Decodificar secuencialmente con cola de uno o dos frames, nunca colección ilimitada. Componer
alfa, frames parciales, disposal y restauración de fondo. Reescalar fuera de UI, no ampliar
imágenes pequeñas. Identificar cada solicitud por generación, incluso en A → B → A.
Presentar sólo imagen/estado; no reconstruir workspace ni listas en cada frame.

Usar reloj monotónico, delays y repeticiones reales. Limitar presentaciones no debe convertir
los delays a un intervalo fijo; descartar presentaciones atrasadas sin saltarse la composición
de frames dependientes. Proteger tiempos nulos y ciclos patológicos. El corte de 60 segundos
del intento es provisional, no una decisión final: si se conserva debe informarse y permitir
repetir explícitamente, sin confundirlo con el final natural.

### Estados, visibilidad y cancelación

Estados mínimos: cargando, listo estático, reproduciendo, pausado, detenido/cancelado,
omitido por límite y error. Cancelar disponible desde el inicio y también si existe miniatura
pero sigue trabajo 3D. Pausar evita seguir decodificando/acumulando frames. Reanudar, Repetir
y Reintentar son explícitos; no reiniciar fallos en cada tick.

Ocultar el preview, cambiar pestaña/archivo, minimizar/bandeja y cerrar deben detener o
suspender realmente el trabajo. Al volver sólo cargar el objetivo vigente. No bloquear callbacks
con `join`, espera de proceso, I/O o decoder. Mantener cámara 3D existente sin lanzar un proceso
por cada movimiento; el rasterizador también debe respetar cancelación/resultados obsoletos.

### Presupuestos provisionales, no promesas

| Parámetro | Referencia inicial a medir |
| --- | --- |
| Debounce | Conservar aproximadamente 150 ms |
| Presentación animada | Hasta 768 px / 15 actualizaciones por segundo; económico 512 px / 10 |
| Cola | 1–2 frames; conservar sólo la última solicitud pendiente |
| Entrada de imagen | 20 MiB; no aplicar indiscriminadamente a todos los lectores |
| Decodificación/composición | Referencia 64 MiB; contar todos los buffers, no sólo salida |
| Falta de progreso | Referencia 5 s; distinguir pausa y delay válido |
| Diagnósticos | Locales, desactivados por defecto, cola acotada |

Separar bytes comprimidos, canvas original, RGBA, caché, colas y memoria total. Usar aritmética
comprobada antes de reservar. El intento propone `3 × decoded_mib + 128 MiB`: con 64 MiB son
**320 MiB por proceso**, hasta dos procesos. No adoptar ni anunciar «64 MiB de consumo»:
comparar una alternativa más económica y medir arranque, pico, reposo y navegación.

Si un lector no admite cancelación cooperativa, evaluar aislamiento en modo interno del mismo
EXE. Dispatcher antes de UI/instancia única/bandeja; supervisor específico en `platform`.
Acotar activos Y cancelados en retirada; sin capacidad, conservar el último pedido, no crear
trabajadores ilimitados. Validar tipo de mensaje, tamaño, dimensiones y longitudes IPC antes
de reservar. Cancelación no debe quedar detrás de una cola saturada. Comprobar pipes truncados,
muerte/timeout, limpieza y aplicación de límites antes de trabajo costoso. Un Job Object no es
un sandbox completo; no concatenar rutas en comandos de shell. No imponer procesos auxiliares
al código ya cooperativo sin demostrar la necesidad y el costo.

## RP-6 — lectores, listado y métricas

- PDF: evitar doble carga si la dependencia fijada lo permite. Limitar páginas/texto visible
  no limita parseo/descompresión. No usar `extract_text_with_limit` sin verificar su existencia
  ni prometer 4 MiB por página: esa afirmación del intento no está sustentada.
- ZIP/TAR/TAR.GZ: preview sin extraer al filesystem; límites de entradas, bytes y expansión
  donde corresponda, cancelación, suma comprobada de tamaños y señal clara de salida parcial.
  Mostrar 500 filas no implica haber procesado sólo 500 entradas ni pocos bytes.
- Texto: conservar codificaciones, selección y scroll; límites de lectura/líneas/longitud y
  costo del resaltado antes de procesar una línea gigante.
- SVG/imágenes enormes: proteger dimensiones/complejidad; conservar canvas y calidad actuales,
  sin scripts ni resolución arbitraria de recursos externos.
- Listado: evaluar 1.024 entradas o 3 ms por recogida; NO es presupuesto del sort/render completo.
  No perder filas, orden, selección ni mensaje final. Medir antes de modificar la política.
- Métricas: primer contenido/fila, decodificación, frames descartados/presentados, cancelación,
  timeouts, profundidad de colas y drenado. Sin telemetría ni rutas/nombres/contenido innecesarios.
  Cola de log acotada: descartar métricas antes que bloquear navegación.
- Caché de posters/resultados ligeros sólo si mediciones justifican el costo. Invalidar por
  identidad/tamaño/mtime; no precargar carpetas ni conservar animaciones completas.

## Configuración y protección de preferencias

Defaults seguros y normalización de datos parciales/corruptos/extremos. Cero y overflow no
significan recursos ilimitados. Verificar si realmente hace falta pasar de esquema 4 a 5:
0.5.1 descarta versiones futuras a defaults. Probar migración y downgrade con copias aisladas,
nunca sobre preferencias activas. Preservar tema/fuentes/íconos, keymap, favoritos, layout,
reglas de preview y aliases; no reactivar un WebP deshabilitado mediante `.wepb`.

Textos por i18n, claves y placeholders en diez idiomas; revisión editorial separada de paridad.
No suprimir pruebas fallidas ni rebajar Clippy para obtener verde. En cada módulo documentar
responsabilidad, ownership, límites medidos vs provisionales, errores, cancelación y migración.

## Matriz de aceptación

RP-1 cuenta con pruebas nuevas de sintaxis, capacidad/timeout, generaciones y teclado del
editor Slint. Los resultados y límites de cobertura se registran en
[validación del 9 de septiembre](../VALIDACION-2026-09-09.md); no sustituyen R04–R06 nativos
con teclado chileno/IME, servidores caídos ni D01–D02 en otras máquinas.
RP-2 añade cableado real con destino capturado/cancelación/cierre y acciones/teclado/render
del componente superior; ver [validación RP-2](../VALIDACION-RP2-2026-09-09.md).
RP-3 añade compatibilidad/contraste de paletas, persistencia de tokens, catálogo único y
render software de quince temas: [evidencia RP-3](../VALIDACION-RP3-2026-09-09.md).
RP-4A añade cancelación/capacidad/visibilidad y controles Slint; ver
[registro RP-4A](../VALIDACION-RP4A-2026-09-09.md). RP-4B–RP-6 siguen pendientes;
no confundir pruebas sintéticas con aceptación nativa completa.

| Casos | Comprobación exigida |
| --- | --- |
| P01–P04 | WebP/GIF/APNG animados y estáticos, alfa/frames parciales/disposal, delays y loops finitos/infinitos |
| P05–P06 | A → B → A, archivo enorme/corrupto, canvas extremo, ausencia de reservas ilimitadas |
| P07–P09 | Cancelar antes/después de miniatura, pausa con cola llena, cancelar pausado, ocultar/pestaña/minimizar/bandeja |
| P10–P11 | Muerte/timeout/IPC truncado, selección repetida; procesos/hilos/handles/memoria acotados y limpieza |
| P12 | PDF comprimido, TAR.GZ costoso y línea gigante; salida parcial y protecciones reales |
| R01–R03 | Rutas largas con acciones visibles; caret/selección/pegar/deshacer; Enter válido/inválido/Escape |
| R04–R06 | Keymap/Ctrl+L/F4, AltGr/teclado chileno, UNC lenta/caída/Unicode/prefijos; sin validación I/O en UI |
| R07–R09 | Cambio/cierre del panel capturado, copiar/favorito sólo confirmados, autocompletado fuera de orden |
| T01–T02 | Seis paletas + temas previos, contraste/íconos/foco/deshabilitados, ausencia de duplicados |
| C01–C02 / I01 | Config antigua/extrema/corrupta, aliases deshabilitados, defaults, paridad y placeholders |
| M01–M02 | Navegación con/sin animación comparable, listado grande sin pérdida, log saturado sin bloqueo |
| D01–D02 | Windows 10/11, VM, DPI 100/125/150/200 %, teclado/Narrador, portable e instalación aislada |

Registrar evidencia y pendientes por caso; inspección no equivale a prueba. Usar las puertas
de [PRUEBAS.md](../PRUEBAS.md), la metodología de [BENCHMARKS.md](../BENCHMARKS.md) y el
empaquetado de [BUILD.md](../BUILD.md). Ejecutar release SOLO. Un run exitoso sin instalador
no es distribución. Actualizar guías, backlog, CHANGELOG al entregar funcionalidad, y grafo.

## Procedencia recuperable y límites de evidencia

Tras la aprobación se consultó GitHub y se recuperó el artefacto de fuentes del run
`34364248486`. La rama remota y `validation/commit.txt` del paquete coinciden con el SHA
comunicado. Esto certifica procedencia del paquete, no que compile ni que sus cambios sean válidos.

- Copia aislada: `target/agent-out/referencia-rp-20260909/source/` (956 archivos).
- ZIP: `target/agent-out/referencia-rp-20260909/naygo-source.zip`, 11.600.433 bytes,
  SHA-256 `a2b88ea4898716650038936af28a03afd9be026da2209344b62ec150a5535ef6`.
- Integridad: `7z t` aprobada; extracción con comprobación de límites de rutas.
- Reutilización selectiva: editor/AltGr y normalizador como referencia; coordinación de
  validación reescrita para reservar plazas antes de lanzar hilos y descartar revisiones.
  No se integraron preview, temas ni scripts experimentales. Las seis paletas sí están
  recuperadas en `crates/core/src/theme/builtin/isgroth-*.json` dentro de la copia aislada.
- Hashes de las referencias reutilizadas: [registro de validación](../VALIDACION-2026-09-09.md).

| Elemento | Identificador comunicado |
| --- | --- |
| Repositorio | `nicolasgroth/explorador_archivos_naygo` |
| Rama experimental | `feat/responsive-previews-and-paths` |
| SHA comunicado | `3489130d8ee7b7f34a11ebeb09b063b4377fdca0` |
| Último run fallido / job | `34364248486` / `102508758224` |
| Artefacto de fuentes | `10109221926`, `naygo-review-source` |
| Run previo con compilación parcial | `34362289913`, checkout `1c9ca80af3e67f5d8b34b0ad8544fb326fffbaaf` |
| Baseline de prueba de Papelera referido | `34355482791` |

Paquete esperado: `referencia_no_validada/` (69 archivos), `cambios_recuperados.patch`,
`manifest_referencia.json` (SHA-256 base/recuperado), `evidencia_compilacion_anterior/`
(`tests.log`, `clippy.log`, `commit.txt`) y `temas/` con seis JSON. Ese paquete de traspaso
no estaba en el proyecto al iniciar. Se recuperó en su lugar el árbol de fuentes de Actions
descrito arriba. Los módulos experimentales de preview permanecen en la referencia, no en
el producto; no confundir recuperación con integración.

Referencias originales: `Naygo_revision_y_parche_inicial.zip`, `naygo-review-source.zip`,
`naygo-validation-1.zip` a `naygo-validation-3.zip`, `Naygo_6_temas_claros_importar.zip`
y `Naygo_galeria_temas.html`. Son nombres de evidencia, no archivos disponibles certificados.

El intento reconstruía cambios `.b64` en Actions mediante `preview-fixes*.py` y sólo guardaba
fuentes en la rama al pasar las pruebas. Un checkout podía diferir del working tree probado.
El último run habría fallado en un `assert` de reemplazo documental a las 14:34:13 UTC, antes
de tests/Clippy. La ejecución anterior informó 874 core + 22 integración + 47 platform, pero
UI no compiló; siete pruebas platform ignoradas, incluida una de Papelera que fallaba en el
runner. No trasladar esa exclusión a la base local como si fuera una corrección.

Otros fallos comunicados: `accessible-description` sin rol en Slint; `Settings` sin
`preview_policy`; comparación incompatible de arrays/slices; API PDF inexistente; trait
`slint::Model` no importado para `.iter()`; advertencias de `chunks_exact*`. Revisar APIs
y tipos directamente, sin sustituciones textuales globales ni correctores silenciosos.

Recuperar sólo código legible mediante comparación de hashes/diff. La CI de continuación debe
validar el commit preparado, sin reconstrucción oculta ni commits automáticos. No borrar ramas
o evidencia histórica. Si se necesita acceso externo para recuperar y aún no está autorizado
en la tarea activa, informar el faltante antes de ampliar el alcance.
