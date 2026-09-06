# Backlog — pendientes y mejoras de Naygo

> Documento de arranque para nuevas sesiones. Todo lo acordado que aún no se implementa,
> con contexto y punteros al código relevante. Actualizarlo al cerrar cada ítem.
> Última actualización: 2026-09-05.

## Estado de partida

- Respaldo solicitado publicado y comprobado en GitHub: `dcca05b1`, rama
  `fix/single-instance-y-bandeja`, 131 archivos. Incluye código, documentación y galería de demo;
  excluye tmp/ personal, configuraciones y artefactos generados. No se modificó main.
- Revisión posterior de 0.5.1 implementada y validada: etiquetas/acciones secundarias de bandeja,
  espacios recientes de sesión y filtro/reintento revisado de fallidos. 1.238 pruebas aprobadas,
  seis smoke ignorados; Clippy/formato/diff/diez idiomas correctos. Grafo: 7.498 nodos / 13.618
  aristas. Registros `post-github-verified-tests.log`, `post-github-clippy-final.log` y
  `post-github-graphify-final.log` en target/agent-out. Distribución regenerada y verificada:
  instalador del 2026-09-05 a las 22:31 y portable a las 22:29 (America/Santiago); hashes en
  [validación 0.5.1](VALIDACION-0.5.1.md). Continuación preparada para publicar en la misma rama.
  Pendiente sólo validación instalada de Nicolás para estos ajustes.

- Entregada **0.5.1** (petición 2026-09-05): rutas con ancho natural y extremo actual
  visible, acciones de bandeja arriba con ajuste de filas según ancho, vaciar separado abajo,
  MIT completa y seleccionable en Acerca de desde LICENSE. Sin nuevas dependencias.
  Validación: 1.230 pruebas aprobadas, 6 smoke interactivos ignorados; Clippy sin advertencias,
  formato/diff y paridad de diez catálogos correctos. Logs 051-validated-tests.log y
  051-final-clippy.log en target/agent-out. Render software inspeccionado a 900/360/200/110 px,
  con comprobación de ancho real, ruta compacta y clics de las ocho acciones al envolver.
  Grafo actualizado: 7.442 nodos / 13.526 aristas (051-graphify.log).
  Instalador regenerado a las 20:36 y portable a las 20:34, hora America/Santiago;
  checksums, integridad ZIP, versión del EXE y LICENSE empaquetada verificados.
  Registro: target/agent-out/051-release.log. Detalle y hashes: [validación 0.5.1](VALIDACION-0.5.1.md).
  Pendiente prueba instalada de Nicolás.
- Distribución anterior: **0.5.0**; instalador y portable regenerados el 2026-09-05. Hay cambios
  previos sin commit en el working tree; inventariar y validar antes de nuevas entregas.
- Suite de recetas verificada 2026-09-05: **1.228 tests verdes** (más 6 smoke tests de Windows
  ignorados por requerir interacción). Registro final: `target/agent-out/recipes-final-tests.log`.
  Clippy aprobado con `--all-targets --locked -- -D warnings` (`recipes-final-clippy.log`).
  Séptima entrega regenerada y verificada: instalador del 2026-09-05 a las 18:32,
  portable de las 18:30, hora America/Santiago. Detalle y hashes en el plan.
- i18n: 10 idiomas con paridad verificada por `scripts/check_i18n_lang.py` (en CI).
- **Fix ya aplicado en el working tree (incluir en el próximo commit):** las 7 claves
  `slint.preview.{loading,cancel,cancel_tip,copy_tip,wrap_on_tip,wrap_off_tip,reset_view_tip}`
  se agregaron a los 8 idiomas que las perdían (pt/de/fr/hi/it/ja/ko/zh). Sin esto, el test
  `i18n::tests::todos_los_idiomas_tienen_las_claves_de_es` falla.
- **Cierre operativo 2026-08-28:** `scripts/build-release.ps1` ya tiene fallback SHA-256 con
  .NET para runtimes PowerShell sin `Get-FileHash`. El release del 2026-08-22 sí produjo ZIP e
  instalador; antes de este fix el script fallaba solo al escribir `SHA256SUMS.txt`, después de
  terminar ambos artefactos.
- **Cierre técnico 2026-08-28 (T-1):** `ops_ctrl` ahora es un módulo con raíz en
  `ops_ctrl/mod.rs`; los workers de archivo comprimido, borrado por lotes/Papelera y recuperación
  de journal viven respectivamente en `archive.rs`, `batch_delete.rs` y `resume.rs`. El wizard de
  sincronización y la bandeja ya estaban correctamente aislados en `workspace_ctrl/`.
- **Cierre técnico 2026-08-28 (T-2):** `scripts/smoke-ui.ps1` ejecuta los drivers interactivos de
  editor de ruta y scroll, exige capturas y rechaza panics nuevos en el log. Quedó documentado como
  gate manual post-build porque los runners alojados no ofrecen un escritorio Windows fiable.
- **Cierre técnico 2026-08-28 (T-3):** el ZIP portable ya no incluye `naygo.pdb` (bajó de ~70 MB a
  ~11 MB). El instalador ofrece los símbolos como tarea opcional y desmarcada; el PDB se sigue
  generando junto al binario para diagnósticos.
- **Cierre funcional 2026-08-29 (C-2):** el diálogo «nueva carpeta» mantiene `Enter` como salto
  de línea y confirma con dos `Enter` consecutivos dentro de 500 ms; `Ctrl+Enter` continúa como
  confirmación inmediata.
- **Cierre funcional 2026-08-29 (F-3):** la mini-barra del filtro por tipeo ahora alterna entre
  resaltar coincidencias y mostrar solo estas. El filtro actúa sobre la vista real del panel,
  conserva foco/selección por ruta y no se persiste ni altera los filtros por columna.
- **Cierre funcional 2026-08-29 (F-2):** `Ctrl+D` duplica la selección en su misma carpeta con
  nombres seguros (` - copia`, ` - copia (2)`, …), planificación asíncrona, journal y deshacer.
  Las configuraciones heredadas liberan el antiguo `Ctrl+D` de Favoritos solo cuando conservan
  exactamente ese valor histórico; los atajos personalizados distintos se respetan.
- **Cierre funcional 2026-08-29 (C-1):** `Ctrl+N` crea carpeta y `Ctrl+Shift+N` crea archivo.
  La carga del keymap migra únicamente el par de defaults anterior, sin alterar reasignaciones.
- **Corrección 2026-09-01 (C-1):** `Ctrl+N` vuelve a abrir el editor multilínea de
  «Nueva(s) carpeta(s)», idéntico al botón y al menú contextual. Así el estado vacío explica que
  falta una carpeta en vez de marcar un nombre inexistente como inválido, y se recupera la
  creación simultánea (incluidas rutas anidadas con `\\`).
- **Corrección 2026-09-01 (Papelera + preview):** la capa Shell de Papelera ahora valida el
  resultado de cada `DeleteItem` y que las rutas hayan desaparecido; si `IFileOperation` informa
  éxito sin retirar el archivo, reintenta mediante `SHFileOperationW` y nunca muestra una
  operación falsa como completada. El preview de texto reinicia viewport, selección y caret cada
  vez que llega contenido nuevo, por lo que abre desde el inicio y no mantiene una selección
  invisible del archivo anterior.
- **Ajuste 2026-09-01 (Papelera + preview):** el callback de resultado se corrigió a
  `PostDeleteItem` (no `PostMoveItem`) y un rechazo ahora queda registrado por archivo como
  fallido, en vez de aparecer engañosamente como «hecho: 0». Para lectura, la vista previa pasa
  de 100 a 250 líneas dentro del mismo máximo de 64 KiB y remata el contenido recortado con un
  borde vectorial de papel rasgado; ambos cambios mantienen el costo de memoria acotado.
- **Ajuste 2026-09-01 (Papelera + preview, segunda validación):** las rutas internas de Naygo
  del tipo `D:carpeta\\archivo` son válidas para `std::fs`, pero Shell las rechaza como
  `E_INVALIDARG`; antes de invocar la Papelera se resuelven ahora a rutas físicas con formato
  Shell (sin el prefijo extendido `\\?\\`). El preview usa un único editor multilínea de sólo
  lectura también para XML/código, con viewport explícito: permite rueda, barra de scroll,
  selección parcial y `Ctrl+C` en cualquier texto que se despliegue.
- **Cierre funcional 2026-08-31 (F-4):** `Ctrl+Shift+V` abre el historial interno de los últimos
  10 conjuntos copiados/cortados dentro de Naygo. Es efímero, deduplica entradas, no guarda rutas
  privadas en disco y pega mediante el mismo motor cancelable de operaciones.
- **Cierre funcional 2026-08-31 (F-1):** el menú contextual exporta la selección —o toda la vista
  filtrada si no hay selección— al portapapeles o a CSV. Respeta columnas visibles y su orden,
  agrega ruta completa, permite separador `;` o `,` y escribe BOM UTF-8 en archivos para Excel.

---

## A. Mejoras técnicas pendientes (importante aplicarlas)

### Mejoras de usabilidad posteriores a 0.5.1 — aprobadas el 2026-09-05

Nicolás pidió respaldar primero todos los cambios del proyecto en GitHub y después continuar
con estas recomendaciones. No incluye publicar muestras personales de tmp/ ni binarios en Git.

- Opción «ícono + etiqueta» y menú de acciones secundarias en barras con íconos poco familiares.
- Acceso a espacios recientes sin indexación global ni abrirlos/ejecutarlos automáticamente.
- Filtrar fallos del historial y preparar un reintento sólo de fallidos, con revisión de fuentes,
  destinos y conflictos; no repetir a ciegas una operación completa ni los pasos ya completados.

Implementadas en la revisión posterior: etiquetas de bandeja reutilizan la preferencia persistida
«Solo íconos»; recientes limitados a diez referencias de sesión (con limpiar, sin I/O al renderizar);
reintentos sólo de archivos regulares fallidos de copiar/mover con origen conocido en el plan.
No se reintentan borrados, ZIP, carpetas, saltados, éxitos ni operaciones sin request recuperable.
Se conservan destinos exactos, se revalidan tamaño/mtime/rutas físicas al confirmar y el motor
consulta conflictos. No registra undo; el diálogo lo advierte. Los fallos no compatibles siguen
accesibles en el detalle. Referencia: [reintentos](REINTENTOS.md).

Referencia de interfaz: https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/command-bar

### T-4. Firma de código del instalador
**Preparación completada; activación externa pendiente.** `scripts/build-release.ps1` ya acepta
`SignToolPath` + `CertificateThumbprint` (o sus variables de entorno), firma el `setup.exe` con
SHA-256, timestamp y luego ejecuta `signtool verify`. Falta que Nicolás/ISGroth cree y autorice
un certificado (p. ej. SignPath.io) y entregue la identidad de firma; no se puede fabricar ni
usar una sin esa autorización. Mientras tanto, el bypass de SmartScreen está documentado en
`docs/DISTRIBUTION.md`.

---

## B. Funcionalidades aprobadas para implementar

### Plan 2026-09-04 — usabilidad y flujos de trabajo (pendiente)

Plan detallado: [Usabilidad y flujos de trabajo](plans/2026-09-04-usabilidad-y-flujos-de-trabajo.md).
Solicitado por Nicolás; séptima entrega implementada y empaquetada (recetas declarativas).
El alcance completo de las siete etapas NO está cerrado.

Implementado en el working tree del 2026-09-04:

- Tinte tenue en la columna ordenada solo del panel activo, con prioridad de selección/alertas.
- Ruta acotada con ancestros accesibles y botones laterales reservados; comparación en segunda
  fila cuando falta ancho.
- Bandeja con Inicio/Fin/Página, scroll siguiendo foco y metadata básica asíncrona por ruta.
- Radar desplazable con flechas/Enter y fuentes capturadas al abrir; mover no vacía la bandeja
  antes de conocer el resultado.
- Maximizar/restaurar un panel con botón/paleta/Ctrl+Shift+M sin alterar el layout persistido.

Segunda entrega implementada: selección múltiple por identidad en bandeja (Ctrl/Shift/Ctrl+A,
Ctrl+flechas y arrastre del grupo), marcas persistentes entre paneles y alcance explícito de
acciones. Borrar confirma los originales; quitar/vaciar solo retira referencias. Los controles
están agrupados y tienen foco de teclado. Ctrl+V agrega referencias, no pega en otro panel.

Asistente de entregas implementado: carpeta nueva o ZIP desde marcados/Ctrl+P; grupos por fuente,
plano o raíz relativa explícita, revisión y revalidación, SHA-256 opcional, manifiesto/lista sin
rutas privadas, temporales propios, cancelación y deshacer únicamente del destino. Se reutilizan
los motores existentes; ZIP usa inventario congelado. [Guía y límites](ENTREGAS.md).

Tercera entrega implementada (2026-09-05): nombres de grupo editables, revisión conjunta de
homónimos y propuesta numerada explícita en modo plano, conservando todas las fuentes. Tarjeta
de resultado no modal con abrir carpeta de salida, copiar ubicación y recuperar las fuentes
marcadas en bandeja sin vaciar otras referencias. El historial conserva los errores de una
entrega fallida, en vez de perder el motivo y aparentar «hecho: 0». La detección de solapamientos
entre muchas fuentes pasa de comparar todas las parejas a búsquedas ordenadas O(n log n).

Configuración: búsqueda por etiquetas de ajustes, no solo por nombre de categoría; navegación
lateral con foco visible, Enter/Espacio y estado sin coincidencias. No consulta disco.
Quedan controles/configuración restantes (incluida restauración por sección), validación
visual/DPI/accesibilidad y mediciones. Etapa 4 implementada con validación instalada pendiente;
etapas 5–7 implementadas con validación instalada pendiente. El alcance completo NO está cerrado.

Cuarta entrega (2026-09-05): espacios por tarea `.naygospace` desde Disposiciones y Ctrl+P,
guardado/actualización explícitos, revisión antes de cambiar y raíz portable opcional. Conserva
layout, panel activo, columnas/filtros, tipeo y bandeja sin serializar listados ni preview.
La cola de operaciones conserva sus destinos capturados al cambiar. Archivos corruptos,
incompatibles o editados externamente no se sobrescriben; staging efímero se rechaza al guardar.
Conversión de layouts existentes aplicándolos y usando Guardar como. [Guía y límites](ESPACIOS.md).
El indicador de cambios se recalcula en el gestor, no en cada frame; abrir tarea usa el selector
nativo, sin catálogo residente ni exploración al arranque. ES/EN; nuevas claves usan inglés
provisional en los otros ocho idiomas (paridad no significa traducción editorial terminada).

Validación de cuarta entrega: 815 core + 22 integración + 40 platform + 285 UI = 1.162 pruebas,
6 ignoradas. Grafo AST: 7.046 nodos / 12.702 aristas; registro `spaces-graphify.log`.
Formato/diff y paridad de los diez idiomas verificados. Prueba de firma de filas desacoplada del
portapapeles global para que un bloqueo externo del SO no produzca fallos ajenos a su contrato.

Distribución de cuarta entrega terminada con `scripts/build-release.ps1`, ejecutado solo,
salida 0 (`target/agent-out/spaces-release.log`): release 25 min 40 s, Inno 85,5 s.
Instalador `dist/Naygo-0.5.0-setup.exe` de las 12:40:27 (49.512.070 bytes), portable de
las 12:39:01 (12.470.322 bytes). Hashes verificados contra `dist/SHA256SUMS.txt`; `7z t`
aprobado y ejecutable del ZIP idéntico al release (34.059.776 bytes, versión/autoría verificadas).
PDB de diagnóstico: 320.688.128 bytes en `target/release`, opcional en instalador y fuera del ZIP.
Sin dependencias nuevas; portable +145.762 bytes frente a la tercera entrega. Sin firma externa
y sin cerrar la instancia instalada de Nicolás: validación visual/DPI/Narrador pendiente.
Origen: working tree sin commit sobre `6ea74b9f39da`, rama `fix/single-instance-y-bandeja`.

Validación de la segunda entrega: `cargo test --workspace --locked` (1.142 aprobados, 6 ignorados),
`cargo fmt --all -- --check` y paridad de los 10 idiomas aprobadas.
`cargo clippy --workspace --all-targets --locked -- -D warnings` aprobado; grafo actualizado por AST
(6.924 nodos, 12.393 aristas). Logs: `delivery-validated-tests.log` y `delivery-final-clippy.log`
dentro de `target/agent-out`. Distribución de esta segunda entrega regenerada con
`scripts/build-release.ps1` (salida 0): instalador del 2026-09-05 a las 00:13
(48.621.974 bytes), portable de las 00:11 (12.233.915 bytes), hora America/Santiago. Ambos SHA-256
verificados contra `dist/SHA256SUMS.txt`; `naygo.exe` del ZIP coincide con `target/release`.
Versión y autoría del exe verificadas. Origen: working tree sin commit sobre `6ea74b9f39da`,
rama `fix/single-instance-y-bandeja`. Hashes completos en el plan enlazado arriba.
Registro: `target/agent-out/delivery-release.log`. Validación visual pendiente: no se cerró
la instancia instalada del usuario para forzar el smoke. Firma externa T-4 sigue pendiente.

Tercera entrega: suite completa con 1.147 aprobados (808 core + 22 integración + 37 platform +
280 UI), 6 ignorados; formato, diff y paridad i18n verificados. Grafo AST actualizado:
6.943 nodos / 12.467 aristas. Logs `target/agent-out/refinements-tests.log` y
`refinements-graphify.log`. Clippy final con `--all-targets --locked -- -D warnings` aprobado,
registro `refinements-final-clippy.log`. Distribución regenerada con `scripts/build-release.ps1`
(salida 0): instalador del 2026-09-05 a las 07:32 (48.987.054 bytes), portable de las 07:30
(12.324.560 bytes), hora America/Santiago. Ambos hashes contrastados con `SHA256SUMS.txt`,
ZIP legible y ejecutable idéntico al release (33.639.936 bytes); versión/autoría verificadas.
Log: `target/agent-out/refinements-release.log` (compilación: 23 min 48 s).
Hashes completos en el plan. Sigue sin firma externa y sin validación visual/DPI instalada;
no se cerró la sesión del usuario. Cambios aún sin commit sobre `6ea74b9f39da`.

- [ ] Etapa 0: base 0.5.0 reproducible e inventario de validación.
- [ ] Etapa 1: ruta adaptable, foco/teclado, propiedades completas de bandeja, accesibilidad,
  controles/diálogos adaptables y configuración fácil de encontrar.
- [ ] Etapa 2: maximización temporal implementada y probada en controlador; validar
  interactivamente restauración visual, pestañas y DPI con el nuevo instalador.
- [ ] Etapa 3: carpeta/ZIP, manifiesto, SHA-256 y refinamientos de grupos/homónimos/posentrega
  implementados. Falta validación manual de fallos/cancelación de archivos grandes. Los demás
  errores de I/O/ruta todavía se informan al encontrarlos, sin resolución masiva ni omisiones.
- [ ] Etapa 4: guardado/actualización/importación de espacios implementados y probados;
  falta validar instalado (diálogo/teclado/DPI/Narrador y red caída real). Referencias a búsquedas
  implementadas con etapa 5; recetas añadidas en etapa 7. Sin catálogo ni autoguardado de espacios.
- [ ] Etapa 5: consultas .naygosearch multiraíz implementadas, con tamaños/fechas relativas,
  ejecución explícita, cobertura parcial, selección/preview/propiedades/bandeja y referencias
  Ctrl+P/espacios. Validación automática aprobada: 1.181 tests, 6 ignorados, Clippy limpio.
  Logs queries-validated-tests.log y queries-validated-clippy.log en target/agent-out.
  Instalador/portable regenerados y verificados (2026-09-05, 14:44/14:42); build salida 0,
  ZIP íntegro, ejecutable idéntico al release y SHA-256 contrastados. Falta validación instalada/DPI/red real.
  Guía: docs/BUSQUEDAS-GUARDADAS.md. No hay índice residente ni ejecución al cargar o arrancar.
- [ ] Etapa 6: puntos .naygopoint implementados; captura/lectura/comparación/exportación/papelera
  en workers, SHA-256 opcional, exclusiones y cobertura desconocida explícitas, límites 50.000
  entradas / 32 MiB, 5.000 filas de cambios y envío sólo de archivos presentes a bandeja.
  Validación automática aprobada: 1.207 tests, 6 ignorados; Clippy limpio. Instalador y portable
  regenerados/verificados: 2026-09-05 a las 17:24/17:22 (build salida 0, ZIP íntegro, exe idéntico
  al release, versión 0.5.0 y SHA-256 contrastados). Log: target/agent-out/points-release.log.
  Pendientes aceptación instalada/DPI/Narrador,
  permisos/red reales y traducción editorial de los ocho catálogos con respaldo EN.
  Guía: docs/PUNTOS-DE-COMPARACION.md. No es respaldo ni incluye catálogo/indexador residente.
- [ ] Etapa 7: recetas .naygorecipe implementadas en working tree; editor de selección/consulta,
  filtros y fecha «este mes», parámetros de raíces/destino separados, revisión congelada,
  entrega carpeta/ZIP con nombres fechados, guardado protegido por revisión y referencias en
  espacios/Ctrl+P. Sin scripts, borrado de originales ni servicios residentes. Guía: docs/RECETAS.md.
  Validación final: 1.228 pruebas aprobadas (859 core + 22 integración + 47 platform + 300 UI),
  6 smoke tests interactivos ignorados; Clippy sin advertencias, formato/diff y paridad de diez
  catálogos aprobados. Logs recipes-final-tests.log y recipes-final-clippy.log en target/agent-out.
  Distribución regenerada/verificada: 2026-09-05 a las 18:32/18:30 (build salida 0, ZIP íntegro,
  exe idéntico al release, versión 0.5.0 y SHA-256 contrastados). Log recipes-release.log.
  Grafo AST actualizado: 7.426 nodos / 13.508 aristas (recipes-graphify.log).
  Pendiente aceptación instalada: teclado/DPI/Narrador, red/permisos y cancelación real.
  Ocho catálogos usan respaldo EN para las claves nuevas; traducción editorial pendiente.

Cada entrega que modifique la app requiere pruebas e instalador/portable regenerados mediante
`scripts/build-release.ps1`, ejecutado solo, y validación de Nicolás antes de la siguiente feature.

### F-5. Historial de navegación con memoria de contexto («Atrás de verdad»)
Al volver Atrás/Adelante, restaurar no solo la ruta sino también el archivo enfocado, selección,
posición de scroll y filtros activos de esa visita. El objetivo es retomar exactamente donde se
estaba sin reconstruir visualmente el contexto. Extender `core::workspace::NavHistory` con una
entrada de navegación compacta; no debe persistir listados ni provocar I/O adicional. Definir qué
parte del contexto sobrevive al reinicio y cómo degradar si los archivos ya no existen.

- **Cierre funcional 2026-08-31:** cada entrada conserva durante la sesión foco, selección,
  primera fila visible, filtros de columna y filtro visual. Se referencia por ruta (no por índice):
  al volver se reubica tras ordenar y se omiten archivos ausentes. Tras reiniciar solo sobrevive
  el estado de tabla/ruta ya persistido; no se guardan listados ni contexto efímero adicional.

### F-6. Radar de destinos para copiar/mover
Overlay navegable íntegramente por teclado que reúna paneles abiertos numerados, favoritos,
carpetas recientes/frecuentes y últimos destinos de operaciones. Elegir un destino con una tecla
y ejecutar copiar/mover sin navegar primero hasta él. Reutilizar el selector numérico de paneles,
`RecentDirs`, Favoritos y el historial de operaciones; el ranking debe ser local, determinista y
sin escaneo ni servicio residente.

- **Cierre funcional 2026-08-31:** F5/F6 (o las acciones configuradas de copiar/mover al otro
  panel) muestran un radar con hasta nueve destinos. Prioriza paneles abiertos, últimos destinos
  de operación, favoritos, frecuentes y recientes; deduplica rutas sin consultar disco. Se elige
  con 1–9, clic o Escape y reutiliza el motor cancelable de operaciones.

### F-7. Vista «qué cambió desde mi última visita»
Comparar el último listado completo conservado en memoria con el listado actual al volver a una
carpeta y marcar elementos nuevos, modificados y desaparecidos. Primera versión solo durante la
sesión, usando metadatos ya listados (nombre, tamaño y fecha), sin indexador ni vigilancia global.
Los desaparecidos se presentan como información, no como filas operables. Evaluar persistencia
opt-in únicamente después de medir memoria y utilidad real.

- **Cierre funcional 2026-08-31:** Naygo conserva el último snapshot completo de cada carpeta
  solo en memoria y compara nombre, tamaño, fecha y tipo al volver/listar otra vez. Las filas
  nuevas o modificadas se resaltan; el footer indica `+n ~m -d`, donde los ausentes (`-d`) son
  únicamente información y nunca se convierten en filas operables. No hay indexador, watcher
  global ni persistencia entre reinicios.

### F-8. Comparación entre paneles accionable
Extender la comparación rápida actual con acciones para ocultar iguales, seleccionar «solo aquí»
o «distintos» y copiar/mover ese subconjunto al otro panel. Agregar navegación relativa enlazada
opcional entre dos raíces: entrar a `sub/a` en un lado intenta abrir `sub/a` en el otro y muestra
discretamente si no existe. La comparación superficial debe seguir usando solo las entradas ya
cargadas; cualquier comparación recursiva pertenece al asistente cancelable de sincronización.

**Cierre funcional 2026-09-01:** permite seleccionar distintos/exclusivos, ocultar iguales de
la vista real y copiar/mover la selección desde el panel que originó la acción. El botón ↔ activa
opcionalmente el enlace relativo; sus marcas se invalidan al recargar, pero la relación persiste.

### F-9. Lente de procedencia de archivos de Windows
Mostrar bajo demanda los datos de `Zone.Identifier` (zona, URL de origen y referente cuando
existan) en Propiedades/Inspector, con acciones explícitas para copiar la información y desbloquear
el archivo. Implementar la lectura/escritura de ADS exclusivamente en `naygo-platform`, nunca al
listar carpetas: solo para la selección activa y en worker. Informar las limitaciones en volúmenes
sin ADS y exigir confirmación antes de quitar la marca de procedencia.

**Cierre funcional 2026-08-31 (F-9):** Inspector carga Zone.Identifier solo para el archivo
enfocado y desde worker. Permite copiar los datos resueltos y quitar explícitamente la marca tras
confirmación; la escritura ADS también ocurre en worker y luego refresca la metadata.

**Ajuste 2026-09-01:** un ADS presente sin ZoneId/URLs legibles igual mantiene habilitado el
desbloqueo; la confirmación nativa quedó localizada.

### F-10. Bandejas guardables como conjuntos de trabajo (`.naygolist`)
Guardar y abrir una bandeja como lista portable de referencias a archivos dispersos, sin copiar
los datos. Debe admitir rutas absolutas y, cuando haya una raíz común declarada, rutas relativas;
al cargar, conservar entradas ausentes marcadas como tales en vez de descartarlas silenciosamente.
Integrar con búsqueda, filtros, exportación y operaciones por lote. Definir primero un formato
JSON versionado, pequeño, inspeccionable y sin metadatos privados innecesarios.

**Cierre funcional 2026-08-31 (F-10):** `.naygolist` usa JSON versionado, admite raíz con rutas
relativas y conserva referencias ausentes. Al importar, un worker verifica disponibilidad y la
bandeja marca visualmente las rutas faltantes sin eliminarlas ni bloquear la UI.

**Ajuste 2026-09-01:** al guardar, la raíz se selecciona explícitamente y ya no se infiere del
panel activo.

### Preview de texto seleccionable

**Cierre funcional 2026-09-01:** el texto plano de Vista previa es solo lectura, pero permite
seleccionar fragmentos y copiarlos con `Ctrl+C`; el botón de la cabecera conserva copiar todo.

### Cierre funcional 2026-09-02 (árbol, preview, apariencia e íconos)

- El árbol común ahora sigue al panel Files activo y revela su carpeta cuando estaba fuera del
  viewport, sin modificar la navegación ni expandir ramas ajenas.
- La rueda permite recorrer la Vista previa de texto seleccionable (incluye TXT, XML y otros
  formatos de texto); se conserva el límite liviano de lectura y la señal visual de contenido
  recortado.
- Los sets Fluent y de color conservan su identificador al reiniciar. El editor de temas incorpora
  un token independiente para el fondo de la barra de herramientas, compatible con temas previos.
- Se incorporaron los sets compactos tintables Outline y Tiles (IDs estables `kenney-game` y
  `kenney-board`): 39 íconos por set, con licencia CC0 documentada en
  `assets/icons/KENNEY-SOURCES.md`; la colección fuente de `tmp/` no se empaqueta.

### Cierre funcional 2026-09-02 (SVG, imágenes y ZIP)

- Los SVG con `viewBox` o dimensiones declaradas respetan su canvas original. Si un export carece
  de ambos —como algunos assets de Flash/Animate con coordenadas negativas— el preview encuadra
  sus límites reales y evita el recorte. Al ser vectoriales se rasterizan hasta 2048 px para que
  no se pixelicen al ajustar el panel; ese mismo tope limita la textura a ~16 MiB.
- Las imágenes estáticas disponen de dos controles locales: ajustar al panel (inicial) y `1:1`.
  El modo natural no amplía fotos pequeñas, las centra y habilita scroll solamente cuando exceden
  el área visible. No hay I/O ni decodificación en el hilo de UI.
- El árbol de ZIP/TAR alinea los tamaños en una columna monoespaciada a la derecha, calculada a
  partir de todas las rutas mostradas. Se mantuvo el árbol ASCII y se descartaron puntos de relleno
  para conservar una lectura limpia.

### Cierre funcional 2026-09-02 (ajustes de controles y configuración)

- El control de escala del preview de imagen es ahora un conmutador único: muestra ↔ cuando la
  imagen está ajustada y `1:1` cuando se ve a tamaño natural; el tooltip anuncia el modo que se
  activará al pulsarlo.
- El editor de temas agrupa los tokens por superficies y paneles, filas y tablas, texto,
  interacción/foco y alertas. La representación no altera los índices internos ni los temas ya
  guardados.
- Los sets antes rotulados por su fuente se presentan como Outline y Tiles, manteniendo sus IDs
  persistentes. Se añadieron Vivid y Pastel: dos sets CC0 de 39 íconos multicolor, con paleta
  semántica por tipo de archivo o acción y sin tintado por tema.

### Cierre funcional 2026-09-02 (productividad y bandeja temporal)

- Se verificaron y conservaron los tres flujos de productividad ya presentes: paleta de comandos
  con `Ctrl+P`, ayuda contextual de atajos con `F1` y disposiciones de workspace guardables desde
  el menú Layouts. No se duplicaron con una segunda implementación.
- Copiar/Mover desde Bandeja abre el radar de destinos y ofrece primero los paneles Files visibles;
  mantiene «Elegir otra carpeta…» como alternativa explícita. Arrastrar una fila de la bandeja a
  un panel abierto usa el mismo OLE drag seguro que el resto de la aplicación.
- La fila elegida se destaca y alimenta Vista previa, Propiedades y metadata asíncrona. Las acciones
  de la bandeja son ahora compactas, basadas en íconos y tienen tooltip, incluido quitar una fila.

### Cierre funcional 2026-09-02 (0.5.0: continuidad de preview y bandeja)

- El selector de escala de imágenes mantiene el último modo elegido —ajustar o 1:1— al navegar
  entre archivos, sin afectar el comportamiento de scroll del tamaño natural.
- Con la Bandeja temporal activa, las flechas ↑/↓ recorren sus ítems y conservan su contexto como
  fuente de Preview, Propiedades y metadata; no recuperan la selección del último panel Files.
- La versión de distribución pasa a **0.5.0**. Todo cambio de la aplicación debe cerrar con un
  nuevo `setup.exe` y ZIP portable para validación manual.

---

---

## D. Bugs/mejoras de arrastrar y soltar

### D-1. Arrastrar dentro del MISMO panel sobre una carpeta (mejora futura)
Hoy el drop intra-panel no mueve/copia a subcarpetas. Implementar con **las mismas reglas que
el drop entre paneles** (copiar por defecto, mover con Shift/mismo volumen, menú con clic
derecho si aplica). Punteros: el drop entre paneles está resuelto en
`crates/ui-slint/src/workspace_ctrl/ops.rs` (`drop_at` y siguientes); extender el hit-test de
`body-touch` en `file-panel.slint` para detectar «fila carpeta destino» dentro del mismo panel.

**Cierre funcional 2026-08-31 (D-1):** el hover OLE ahora cruza su coordenada real con el
`ListView` del `FilePanel`, incluyendo scroll, y reporta la fila de vista bajo el cursor. Si es
una carpeta, `drop_at` la resuelve como destino antes de evaluar las reglas existentes; por tanto
Ctrl/Shift, mover por mismo volumen, confirmación y conflictos se comportan igual que entre
paneles. Se rechaza una carpeta sobre sí misma o dentro de su propio árbol.

---

## Notas de operación para la próxima sesión

- Verificación estándar: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets` (cero warnings), `cargo test --workspace` (todo verde), paridad i18n con
  `scripts/check_i18n_lang.py` para los 10 idiomas al tocar claves.
- Tras cada fix que afecte la app: rebuild de `dist` con `scripts/build-release.ps1` (el
  usuario prueba instalando desde `dist/`; ver el punto «Cómo trabajar» en AGENTS.md).
- **Corrección 2026-09-01 (D-2):** 7-Zip en esta máquina entrega `CF_HDROP` con archivos
  temporales `Temp\\7zE…`, no contenido virtual; borra esa ruta al salir de `Drop`. Naygo ahora
  abre esos handles antes de retornar al archivador y un worker los copia a staging propio, por
  lo que la planificación asíncrona no depende de una ruta efímera. Si una fuente ofrece contenido
  virtual también se prioriza y materializa. Se corrigió además un préstamo `RefCell` en el toast
  de error que podía cerrar Naygo al informar `SourceUnreadable`. Revalidar manualmente archivo +
  carpeta desde 7-Zip y WinRAR, conflicto/cancelación y Explorer (`CF_HDROP`) con el build de
  distribución resultante.
- La máquina del usuario estuvo bajo presión de RAM (~700 MB libres): correr builds pesados
  (release con LTO) SOLOS, sin otros trabajos en paralelo.
- **Material web 2026-09-05:** generadas y revisadas 20 capturas reales de Naygo 0.5.0
  en seis temas, con datos ficticios y TXT de explicaciones/alt. Entrega local:
  `C:\Users\ngrot\Pictures\Screenshots\Naygo-web-20-capturas.zip`.
  Corpus y copia portable separados en `D:\Naygo-Demo`; sesión instalada restaurada
  en segundo plano. No se modificó la aplicación ni se requirió rebuild de distribución.
  Ampliación: material ubicado por el usuario en `assets/screenshots`; agregadas
  variantes multipanel 21–22 (oscura y clara), anonimizadas mediante edición generativa
  de su referencia. Guía y notas distinguen estas ilustraciones de las 20 capturas
  reales. Revisadas visualmente; referencia privada excluida de la entrega.
- `scripts/run-logged.sh` envuelve comandos pesados (deja salida + curva de RAM en
  `target/agent-out/` y bitácora en `logs/agent-bitacora.md`); útil ante caídas del entorno.
