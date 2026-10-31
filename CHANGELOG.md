# Changelog

Todas las novedades de Naygo se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado Semántico](https://semver.org/lang/es/).

## [Sin publicar]

## [0.5.4] (en preparación)

### Linux (experimental; aceptación nativa pendiente)
- La instancia única usa un lock por usuario y Unix socket dentro de un `XDG_RUNTIME_DIR` validado;
  las rutas llegan como bytes POSIX acotados y el listener trabaja fuera de la UI. La compilación
  cruzada pasa, pero WSL no puede arrancar en este equipo; no se anuncia el backend como validado
  hasta ejecutar la matriz nativa Ubuntu.
- El core incorpora un codec puro para `text/uri-list`, con rutas locales absolutas y bytes POSIX
  no UTF-8 preservados. Aún no conecta Nautilus/Dolphin ni habilita portapapeles de archivos.
- El core codifica y decodifica los metadatos copy/cut de Nautilus (`x-special/gnome-copied-files`)
  y el indicador de corte de Dolphin (`application/x-kde-cutselection`). Es preparación de
  interoperabilidad; el clipboard Linux aún no está integrado. La entrada está acotada a 16 MiB,
  100.000 elementos y 64 KiB por ruta.

### Plataforma Windows
- Papelera: asocia cada callback con la identidad del Shell obtenida antes de eliminar, para
  conservar recibos con rutas cortas y largas sin seguir junctions. Libera nombres COM y conserva
  UTF-16 no válido. Ya no usa el fallback clásico que solo intentaba reciclar si era posible ni
  confirma automáticamente una eliminación permanente; si no puede confirmar la identidad del
  elemento reciclado, lo informa en el historial. Validación remota y build pendientes.
- El visor agrega la opción de usar `displaycolor` de materiales básicos 3MF por objeto y cara;
  conserva los tonos uniformes y usa azul para propiedades no compatibles. La transparencia se
  aproxima sobre el fondo; texturas, extensiones de materiales y colores propietarios siguen pendientes.
- El empaquetador impide mezclar un setup previo con un portable recién generado si falta Inno
  Setup y registra en checksums solo los artefactos generados en la ejecución actual.
- El preview 3MF intenta mostrar primero la geometría compatible y recurre a la miniatura
  incorporada cuando no puede interpretarla; conserva los límites y la cancelación del lector.
  Las referencias a componentes externos aún no se interpretan: se rechazan para evitar confundir
  sus identificadores con objetos locales y mostrar una figura incorrecta.
- El pie de los paneles de archivos usa un matiz derivado del tema para distinguirlo del título,
  sin agregar campos obligatorios a los temas personalizados existentes. Sus leyendas usan el
  texto principal del tema para mejorar la legibilidad sobre ese matiz.
- Mejorado el contraste de texto en seis temas de fábrica, incluidos Dracula y Solarized Dark,
  sobre paneles, filas, selección y barra de herramientas.
- El preview STL/3MF incorpora controles para alternar tonos uniformes azul, blanco o cálido y
  vistas sólida, con bordes o en malla. Reutiliza la geometría retenida; no interpreta materiales
  propios del 3MF. El render por defecto conserva el aspecto anterior.
- La vista previa automática reconoce `.sif` como texto y agrega esa extensión a la configuración
  existente sin sobrescribir preferencias previas. La lectura de `.sh` ya estaba habilitada; un
  error «No se pudo leer» en una unidad mapeada apunta a un fallo de apertura/lectura del origen,
  que no puede confirmarse sin acceso a ese equipo.
- La lectura de texto del preview se puede ajustar entre 64 KiB y 1 MiB desde Configuración; el
  valor inicial es 256 KiB. La vista informa bytes leídos y caracteres/líneas mostrados. El worker
  sigue aislado y mantiene los límites de líneas y caracteres por línea para proteger equipos modestos.
- Al pegar, la consulta de contenido del portapapeles y la decisión de destino ahora se ejecutan
  juntas en el worker. Un proveedor externo lento de clipboard ya no bloquea el hilo de interfaz.
- Las lecturas y escrituras del portapapeles ahora pasan por un worker serial con cola acotada:
  Copiar/Cortar, pegar en la Cesta, copiar rutas/texto, exportar CSV y copiar texto de preview ya no
  acceden al clipboard desde callbacks. Los resultados actualizan el indicador de corte después de
  confirmar la escritura; los errores se registran y muestran sin bloquear la UI.
- La asociación entre rutas de origen, resultados parciales y recibos de Papelera compara UTF-16
  ordinal directamente. Nombres distintos con secuencias sustitutas inválidas ya no se fusionan al
  convertirlos a texto de reemplazo; se conserva la comparación sin distinción de caja y con `/` o `\`.
- La búsqueda de recibos reconstruidos compara también el origen del elemento del Shell con las
  reglas de ruta de Windows; diferencias de caja o separadores ya no impiden encontrar el elemento.
- Guardar temas personalizados se hace de forma atómica en el worker, evitando que un fallo de
  escritura deje un JSON truncado; borrar y actualizar el catálogo también permanecen fuera del hilo UI.
- Linux incorpora un backend inicial de Papelera FreeDesktop con recibos exactos para deshacer,
  nombres no UTF-8, destinos por filesystem y rechazo de rutas de Papelera inseguras. La validación
  de runtime Ubuntu y la matriz completa de permisos, conflictos y fallos parciales siguen abiertas.
- La enumeración de unidades y la consulta USB pasan a workers; actualizar la tira o construir/restaurar
  árboles ya no llama a Win32 desde la UI. El árbol conserva ramas expandidas de unidades que siguen
  conectadas y reconcilia las raíces cuando cambia el hardware.
- La expulsión segura reenumera y valida la unidad en un worker antes de actuar; la confirmación,
  liberación de watchers, actualización de paneles y resultado visible siguen coordinados por la UI.
- Linux enumera desde `/proc/self/mountinfo` los puntos de montaje de sistemas de archivos y red,
  conserva sus rutas POSIX en bytes y omite pseudofs de la tira rápida. Los tipos desconocidos no
  ofrecen expulsión. La integración compila cruzada; la aceptación nativa Ubuntu y detección de
  removibles siguen pendientes.

## [0.5.3] — 2026-10-19

### Usabilidad
- El doble clic nativo de Slint recibe Ctrl/Shift del gesto actual del mouse. Si un modal o un
  arrastre pierde el evento de liberación de Ctrl, el siguiente doble clic normal navega en el
  panel actual en vez de abrir accidentalmente la carpeta en otro panel.

### Plataforma
- El callback y los enums de la bandeja Linux usan ahora los tipos exactos de GLib 2.88 (`GUnixFDSourceFunc`, `GIOCondition` y `GConnectFlags`), contrastados con los headers de Ubuntu 26.04.
- El footer Linux consulta mediante `statvfs` el volumen de la carpeta abierta. La consulta ocurre en un worker; la caché queda acotada a 128 carpetas y ocho consultas simultáneas para evitar crecimiento o ráfagas al recorrer montajes lentos.

### Operaciones de archivos
- Renombrar y renombrar por lotes publican cada destino sin reemplazo atómico. Si otro proceso
  crea el nuevo nombre durante la operación, Naygo conserva el origen y el archivo externo; los
  cambios de mayúsculas/minúsculas siguen funcionando en Windows.
- «Renombrar antiguo» aparta el destino con publicación atómica sin reemplazo; si otro proceso
  ocupa el nombre de respaldo elegido, la operación falla conservando intactos el destino original,
  el archivo entrante y el archivo externo.

### Rendimiento
- El listado reutiliza la misma `PathBuf` para filtrar nombres internos y construir la fila, evitando
  materializar una segunda ruta completa por entrada. La medición de disco local no fue concluyente.

### Usabilidad
- Los botones compactos de la bandeja temporal, las operaciones y las opciones de carpeta perdida
  exponen su acción accesible y activación por teclado mediante los mismos callbacks visuales; los
  controles deshabilitados siguen protegidos. Maximizar/cerrar panel y las categorías de ajustes
  también anuncian y ejecutan sus acciones por UI Automation y teclado.
- Los controles de «Subir carpeta», recientes y menús de contexto/filtro declaran rol, nombre y
  acción accesibles ligados a sus callbacks visuales. Las filas de tipos en el filtro informan su
  estado marcado.
- Las filas de los paneles de archivos se anuncian como elementos seleccionables, con nombre, tipo,
  estado e índice; su acción predeterminada abre el elemento usando el flujo de activación existente.
  «Subir» en la fila `..` se anuncia con su acción, y las acciones del Inspector para calcular tamaño,
  copiar y quitar la marca de procedencia exponen nombres y acciones para lectores de pantalla.
- Al abrir o confirmar el modal de nuevas carpetas, Naygo limpia modificadores cuyo evento de
  liberación pudo quedar dentro del diálogo. Además, los clics sincronizan Ctrl/Shift desde el
  gesto real antes de resolver un doble clic; un doble clic normal ya no hereda Ctrl ni abre una
  carpeta en otro panel por accidente.
- Los botones personalizados de la barra principal y de unidades ahora exponen a UI Automation y
  lectores de pantalla su rol, nombre y acción; el control de expulsión de unidades extraíbles
  queda anunciado por separado.
- El editor seleccionable de texto del preview tiene ahora un nombre accesible localizado para
  lectores de pantalla. La etiqueta identifica el control sin usar el contenido ni la ruta del archivo.
- El campo de nombre compartido por los diálogos de crear, renombrar y comprimir ahora expone como
  nombre accesible el mismo propósito localizado que muestra su título.
- Las acciones del preview, los resultados de Ctrl+P y el árbol de carpetas declaran roles, nombres
  y acciones accesibles. La paleta anuncia la etiqueta visible y el atajo, nunca su payload; las
  filas del árbol activan la navegación existente.
- Los campos editables de configuración, búsqueda, favoritos, recetas, consultas guardadas y diálogos
  ahora declaran nombres accesibles explícitos. Los textos de entrada y las rutas siguen siendo
  privados; las etiquetas identifican el propósito del control.
- El selector de Espacios ofrece acceso a los espacios recientes; Naygo conserva hasta diez
  referencias en `settings.json` y las carga con el worker de inicio, sin explorar carpetas.
  Si el espacio actual está limpio, el seleccionado se aplica al terminar de leerlo; si hay cambios,
  Naygo abre la revisión existente y exige guardar, descartar o cancelar antes del cambio. Limpiar
  recientes solo elimina las referencias, nunca los archivos `.naygospace`.
- La barra de herramientas agrupa navegación/ruta, búsqueda y visibilidad, paneles,
  acciones, unidades y configuración; la ruta queda junto a los controles de navegación,
  Configuración al extremo derecho y separadores con más aire para distinguir grupos.
- Espacios tiene ahora un acceso propio separado de Disposiciones. En ventanas angostas la barra
  conserva navegación, ruta, búsqueda, favoritos, Espacios y Disposiciones; las acciones secundarias,
  unidades y Configuración pasan a «Más» en la misma fila.
- El menú de favoritos de la barra muestra a la derecha la ruta abreviada de cada carpeta,
  con texto atenuado, para identificar la unidad y ubicación antes de navegar.
- Al guardar un espacio por tarea se puede elegir entre Favoritos globales compartidos o una lista
  propia guardada dentro del archivo .naygospace. Los espacios v1 siguen usando Favoritos globales.

### Portabilidad POSIX
- Se prepararon avisos Linux separados para el runtime Ayatana AppIndicator GLib, incluyendo el
  texto GPL-3 completo, atribuciones, hashes y procedencia autenticada. Los avisos Windows ahora se
  identifican como tales; la revisión e inventario final Linux siguen pendientes de validación nativa.
- En Linux, abrir un archivo con Enter o doble clic usa `xdg-open` con la aplicación predeterminada
  del escritorio. Naygo pasa la ruta absoluta como un único argumento sin shell, conserva los bytes
  POSIX y no canonicaliza symlinks; el entorno de escritorio puede delegar la apertura en un portal.
  La operación de «Abrir con» y las terminales configurables siguen pendientes para Linux.
- En Linux no se ofrece la acción Windows «Ejecutar como administrador» para archivos `.exe` o
  `.com`; Naygo no la sustituye por una invocación silenciosa de `sudo`.
- Las rutas no UTF-8 ahora se preservan de forma reversible en favoritos, cestas y referencias de
  espacios de trabajo, búsquedas guardadas y puntos de comparación. Las rutas UTF-8 conservan el
  formato JSON existente; el lector acepta cadenas antiguas y rechaza codificaciones etiquetadas
  desconocidas o mal formadas. La representación visible sigue escapada y nunca reconstruye la ruta.
- El backend de fuentes Linux carga Fontconfig dinámicamente mediante Fontique; el check cruzado de
  la UI ya no requiere `pkg-config` ni headers Linux. Si falta `libfontconfig.so.1`, Fontique usa su
  fallback de fuentes. Runtime y escritorio nativos aún requieren validación en Ubuntu.

### Preview de modelos 3D
- En STL/3MF, los arrastres horizontales y verticales orbitan sobre los ejes de la cámara aunque
  el modelo ya esté inclinado. El rasterizador calcula los pesos de triángulo incrementalmente por
  fila y píxel; una medición sintética release de 96.800 triángulos mostró cerca de 30 % menos
  tiempo de raster en este equipo. La carga y el raster siguen cancelables y fuera del hilo de UI.

### Apariencia en Windows
- Se puede activar opcionalmente el uso de iconos asociados por Windows para archivos. Está
  desactivado por defecto; las consultas se hacen en un worker, por extensión (sin abrir archivos),
  con caché acotada a 512 tipos y fallback a los iconos habituales de Naygo. Las filas se refrescan
  en una tanda al terminar las consultas pendientes para evitar reconstrucciones por resultado.

### Bandeja del sistema y cierre
- «Cerrar a bandeja» ahora requiere una vía de retorno activa (bandeja o atajo global registrado).
  Si ambos fallan o no están disponibles, cerrar la ventana guarda la sesión y termina Naygo en
  vez de dejar un proceso oculto e inaccesible; esto también mantiene segura la ruta Linux, donde
  el atajo global aún no está implementado.
- El worker de bandeja Linux conserva el source GLib hasta salir del loop antes de retirarlo, para
  evitar que un cierre por HUP/EOF/error lo destruya en el callback y vuelva a quitar el mismo ID.

### Windows — operaciones de archivos
- Si falla la enumeración de una carpeta durante la planificación de copiar/mover, Naygo cancela
  antes de ejecutar la transferencia y señala el origen afectado; ya no descarta errores individuales
  de `ReadDir` y crea un plan incompleto.
- Corregido un cierre inesperado al soltar desde 7-Zip: el tick ahora libera el controlador antes
  de sincronizar filas y layout. El archivo sigue su flujo normal de copia en segundo plano.
- La columna **Extensión** y su filtro ahora agrupan todas las carpetas como **Carpeta**, aunque el
  nombre incluya puntos (por ejemplo, `1.The Buddha` o `2.3DBenchy`). Los archivos sin extensión
  siguen en su categoría propia; ordenar ascendente/descendente y exportar el listado respetan el
  tipo de carpeta. Los elementos `Other` conservan la extensión cuando existe.
- Las copias se escriben primero en un temporal junto al destino y sólo se publican al terminar.
  Cancelar o fallar durante la copia conserva tanto el origen como cualquier versión anterior del
  destino; la publicación sin sobrescritura también rechaza un destino creado durante la copia.
  En el fallback de movimiento entre volúmenes, el origen se retira sólo después de publicar.
  Al mover una carpeta se procesan primero los descendientes y luego se retiran únicamente sus
  directorios vacíos; carpetas con archivos omitidos, fallidos o cambios concurrentes se conservan.
  El movimiento rápido de archivos recibe el renombrado no-clobber desde `platform`; en Windows
  usa `MoveFileExW` sin flags de reemplazo ni copia entre volúmenes, así un destino creado entre la
  planificación y el rename se conserva y el motor mantiene su fallback cancelable cross-volume.
  El fallback nativo autónomo del core sigue siendo best-effort en Windows.
  Se conserva el modo POSIX del archivo copiado. Se validó el adaptador con pruebas Win32 y las
  regresiones de flujos UI; el gate completo pasó y la distribución Windows 0.5.2 fue regenerada.
  Al elegir Reemplazar una carpeta, el motor prepara el árbol completo en un temporal junto al
  destino y lo publica sólo después de terminar. Cancelar/fallar durante la preparación conserva
  el árbol anterior; si falla la publicación intenta restaurarlo y mantiene un respaldo recuperable
  cuando otra ruta ocupa el nombre. En Move retira el origen sólo tras publicar y revalidar el
  contenido copiado. La publicación de directorios en Windows usa dos renames y no es un intercambio
  atómico ante crash; requiere espacio para staging. La aceptación manual y limpieza de un respaldo
  retenido siguen pendientes cuando haya un error de permisos o colisión externa. Cada reemplazo
  ahora persiste un registro antes de cambiar rutas y deja una marca en el árbol staged; al reiniciar,
  listar la carpeta padre restaura el árbol anterior si aún no se publicó el nuevo, o termina de
  limpiar el respaldo si ya se publicó. La recuperación valida sentinels y componentes de ruta,
  respeta cancelación y no pisa destinos concurrentes. Se probaron cierres de proceso justo después
  de cada rename en un proceso hijo; no se afirma durabilidad ante pérdida de energía.

### Windows — accesibilidad
- Las listas de archivos exponen filas seleccionables con nombre, tipo, estado e índice accesibles; la fila `..` se anuncia como «Subir». El Inspector anuncia las acciones de calcular tamaño, copiar y quitar la marca de procedencia. Estas acciones reutilizan los callbacks de los controles visuales. La aceptación con Narrador/UI Automation sobre la distribución queda pendiente.
- Se habilitó la exportación del árbol de controles Slint a las APIs de accesibilidad del sistema
  para W-13, y se añadió `scripts/inspect-naygo-uia.ps1` para comprobar de forma repetible que
  la ventana exponga controles UI Automation. Esta comprobación estructural no sustituye una
  sesión real de Narrador ni la aceptación manual de W-13. Se regeneró el inventario Windows de
  avisos de terceros a 404 paquetes; AccessKit y sus dependencias Windows son MIT/Apache-2.0.
- Las pestañas de panel ahora exponen roles `tab-list`/`tab`, estado seleccionado, nombres localizados
  y acción accesible para activar/cerrar. Con foco en la barra, ←/→ recorre sin cambiar de panel;
  Enter/Espacio activa la pestaña y un borde indica el foco. La prueba headless cubre navegación,
  límites y activación; Narrador y orden de foco siguen pendientes de aceptación manual.

### Linux — core en preparación
- Importar una configuración entre plataformas conserva los ajustes compartidos y omite con aviso localizado las rutas de inicio y recientes incompatibles, el autoinicio y el atajo global cuando Linux aún no los implementa. Los keybindings portables se mantienen; el pack actual no contiene preferencias de terminal.
- L-02 parcial: los nuevos sets de íconos y PNG de usuario usan `XDG_DATA_HOME/naygo/icons`;
  los catálogos y exportaciones consultan también la ubicación antigua bajo XDG config y prefieren
  data cuando hay duplicados. Los imports de packs separan PNG (data) de JSON (config), y no migran
  ni borran archivos existentes. En Windows ambos directorios siguen coincidiendo y las rutas
  portables no cambian. Pasan 13 tests core y 8 tests de packs UI; el check/Clippy Linux del core
  pasa. Se regeneró ZIP/setup Windows bajo supervisor (1.408 tests de workspace); quedan el check de
  UI y la ejecución nativa Linux. Esto no anuncia Linux disponible.
- L-02 parcial: el arranque resuelve XDG sin I/O; Linux guarda configuración bajo
  `XDG_CONFIG_HOME` y logs bajo `XDG_STATE_HOME`, crea directorios desde workers y toma Home de
  `$HOME`. El inicio, la recuperación de rutas perdidas y los paneles de respaldo ya resuelven
  Home mediante la preferencia del usuario y el entorno de plataforma, sin caer en `C:/` en Linux.
  Windows conserva la configuración portable junto al EXE. Los tests dirigidos pasan;
  falta compilación y ejecución nativa Linux. El helper privado de ocupación de Papelera se limita
  a Windows/tests para mantener limpio Clippy del target Linux. Esto no anuncia una versión Linux disponible.
- El core valida nombres con reglas POSIX en builds Unix: backslash, espacios y puntos finales se
  conservan; `relative_components` sólo interpreta `/` como separador. `dedup_name` preserva bytes
  no UTF-8 al elegir un nombre de conflicto. El plan de duplicados también crea nombres ` - copia`
  con componentes nativos y conserva bytes inválidos al resolver colisiones. Esto no habilita UI ni
  operaciones Linux: falta compilar y ejecutar la edición en un entorno Linux nativo.
- La compresión ZIP de core conserva el backslash como carácter literal en nombres POSIX. Los
  componentes de ruta que no sean UTF-8 se rechazan antes de crear o truncar el archivo destino, en
  vez de sustituirse silenciosamente. Los dos tests Unix están compilados para Linux; falta correrlos
  en un host Linux nativo. Se regeneró la distribución Windows con gate completo y build supervisado;
  hashes y firma pendiente constan en `docs/VALIDACION-BUILD-2026-10-15.md`. Esto no habilita
  operaciones Linux.
- Las plantillas conservan rutas fijas como `PathBuf` y rechazan explícitamente una ruta POSIX no
  UTF-8 antes de cambiar el catálogo, ya que el JSON vigente sólo representa texto. La UI muestra el
  motivo en diez idiomas; las rutas UTF-8 conservan el JSON existente. Gate completo y distribución
  Windows supervisada pasan. Esto todavía no habilita UI ni operaciones Linux.
- El workspace mantiene su JSON histórico para rutas UTF-8 y codifica rutas POSIX no UTF-8 en una
  forma etiquetada/versionada reversible; el fingerprint de sesión compara bytes nativos y evita
  confundir rutas distintas por texto sustituto. Los tests Windows y el check cruzado compilan; falta
  el round-trip nativo Linux. Esto no anuncia compatibilidad Linux ya disponible.
- El inicio usa `args_os()` para que una carpeta POSIX con bytes no UTF-8 no haga panic antes de
  abrir Naygo. La ruta se conserva en `PathBuf` y se escapa al construir avisos; flags y valores de
  tema/plantilla siguen siendo UTF-8. Los tests POSIX compilan en check/Clippy cruzados; falta su
  ejecución Linux nativa. El gate Windows y el build supervisado pasan; artefactos y hashes en
  [`docs/VALIDACION-BUILD-2026-10-15.md`](docs/VALIDACION-BUILD-2026-10-15.md).
- L-01A: el listado, el árbol y las búsquedas de core muestran bytes POSIX no UTF-8 como `\\xNN`,
  y duplican el backslash literal para que no se confunda con un escape. Buscar por el mismo texto
  visible encuentra el nombre original; la ruta operativa sigue en `PathBuf`. El check y Clippy
  cruzados compilan los tests Unix; falta ejecutarlos en Linux nativo.
- El inventario L-00 confirma en Ubuntu 26.04 el runtime GLib 2.0.1-1build1, sus dependencias directas
  (libc y GLib >= 2.80), soname `.so.2` y licencia GPL-3 del C runtime según `debian/copyright` de la
  fuente hash-verificada. No habilita la bandeja ni cierra L-00; faltan binding/degradación, notices
  del paquete binario y pruebas Linux nativas.

### Windows — rendimiento y CI
- W-10C registra los lotes append-only del streaming y extiende el modelo Slint solo cuando la
  vista demuestra que las filas previas siguen siendo un prefijo idéntico. Así convierte y agrega
  únicamente las filas nuevas; inserciones que cambian el orden, refreshes y mutaciones conservan
  la reconciliación existente. Tres corridas release headless midieron 21,279 ms de mediana al
  añadir una fila a 100k, frente a 561,367 ms en el harness previo. Gate y distribución 0.5.2
  supervisados pasan; ZIP, setup y EXE sin firma Authenticode y sus hashes están en
  `docs/VALIDACION-BUILD-2026-10-16.md`.
- W-07 añade spans opt-in para duración de callbacks de teclado, navegación por filas y ordenamiento.
  No capturan entradas ni rutas; separan el callback del tick/renderer y no sustituyen las mediciones
  de interacción extremo a extremo.
- Restaurar desde Papelera detecta entradas reparse ocupadas cuyo destino desapareció (junctions o
  symlinks colgantes); sólo `NotFound` representa ausencia y errores de metadata como
  `PermissionDenied` detienen la operación sin recibos falsos ni fallback inseguro. La regresión
  tiene pruebas Windows dedicadas; gate completo y distribución 0.5.2 regenerada con hashes
  verificados. Aceptación manual y runner remoto siguen pendientes.
- La captura pasiva W-07 muestrea el proceso raíz y sus descendientes durante cada ventana, registra
  identidades observadas y picos de recursos para detectar lectores breves de preview; el intervalo
  configurable y la sobrecarga/limitaciones quedan documentados.
- CI, benchmark y release fijan las acciones de terceros a SHA completos verificados para
  `actions/checkout`, `actions/upload-artifact`, `Swatinem/rust-cache` y `dtolnay/rust-toolchain`.
  CI comprueba que esos pins sigan siendo inmutables, exige digest SHA-256 en acciones Docker y limita
  explícitamente el token a `contents: read`; la release mantiene `contents: write` para publicar.
  CI prueba también las guardas de identidad y estado Authenticode de W-14 con certificados
  sintéticos. No se publicó ni ejecutó.
- La sincronización de filas Files compara el contenido visible de `RowData` y sus celdas, no la
  identidad de los modelos anidados que Slint crea en cada conversión. Esto evita que una sola alta
  emita 100.000 eliminaciones y 100.001 inserciones. El watcher ahora propaga deltas para
  modificaciones estables y `sync_rows` construye solo las filas afectadas; en tres mediciones
  headless a 100.000 filas, modificar una fila bajó de 353,568 ms a una mediana de 31,874 ms (~91 %).
  El append tuvo mediana de 403,996 ms frente a la muestra previa de 366,762 ms; requiere una
  comparación controlada antes de atribuir esa variación al cambio. Gate Windows completo y build
  supervisado pasan; los nuevos hashes están en la validación 2026-10-15. Falta validar
  interacción/renderer real.
- La Papelera de Windows ya conserva los recibos de los elementos reciclados si falla otro ítem
  del mismo lote; el resumen señala cada origen fallido por separado y mantiene disponible su
  deshacer. Cada fallo se asocia al índice del origen y a su ruta para admitir entradas relativas.
  El fallback del Shell clásico se ejecuta por ruta y no se activa al cancelar.
- El resumen parcial tiene una prueba UI que verifica atribución por índice cuando la entrada es
  relativa y el fallo trae una ruta absoluta.
- El envío a Papelera convierte rutas a absolutas sin `canonicalize`, para que un symlink o
  junction se recicle como enlace y no se resuelva hasta su destino. Un test de junction real
  verifica reciclaje, restauración y conservación del contenido destino. Pasaron 5/5 tests
  dirigidos de Papelera, 1.389 tests del workspace, Clippy `-D warnings`, formato y checks cruzados
  Windows de platform/UI. La distribución 0.5.2 se regeneró bajo supervisor y sus hashes se
  verificaron en `docs/VALIDACION-BUILD-2026-10-15.md`.
- Se añadió `scripts/capture-w07-process.ps1` para recoger en JSON memoria, CPU, handles y threads
  del proceso Windows en ventanas configurables, más los mapeos SMB cuando están disponibles. No
  interactúa con Naygo ni sustituye las métricas internas opt-in o la matriz real de navegación.
- Nueva observación pasiva de 90 s de la instancia instalada tras el reinicio: 41,6 MiB working set,
  40,1 MiB privados, 349 handles, 17 threads y sin mapeos SMB. No hubo interacción ni control de
  inactividad; la muestra no cierra W-07. Registro en `docs/BENCHMARKS.md`.
- La captura opt-in separa el costo de sincronización de filas Slint en firma, construcción,
  conversión y actualización del modelo estable; sigue pendiente recopilar muestras de uso real.
- Se repitió tres veces el benchmark sintético W-07 en Windows 10 sobre el mismo Ryzen 7 1700X.
  Las medianas a 100.000 filas variaron +1,0–9,4 % frente a la base del 13 de octubre, por debajo
  del umbral orientativo de 15 %. Esto no mide la UI completa ni cierra W-07; resultados en
  `docs/BENCHMARKS.md`.
- Se volvió a verificar el gate completo Windows y se regeneraron portable ZIP e instalador 0.5.2
  bajo el supervisor de memoria; ambos siguen sin firma Authenticode y sus hashes están en
  `docs/VALIDACION-BUILD-2026-10-14.md`.
- El restore de Papelera usa primero el nombre de parsing capturado por `PostDeleteItem` y ahora
  difiere por completo la enumeración de la carpeta virtual hasta necesitar el respaldo. Esto cubre
  runners sin enumerador Shell. El caché de hasta 1.024 identidades ahora expulsa FIFO para conservar
  los recibos más recientes y refresca el orden cuando se reemplaza una ruta. El round-trip integrado
  inyecta un enumerador fallido y confirma cero llamadas para recibos directos; otra prueba cubre
  el error explícito del respaldo para recibos reconstruidos. También pasan la prueba FIFO y el gate
  local; el build 0.5.2 final está verificado. El ZIP, el setup y sus hashes coinciden;
  queda correr GitHub Actions en Windows Server 2025.
- Se regeneraron los artefactos 0.5.2 después de la corrección de Papelera. ZIP e instalador siguen
  sin firma; hashes y límite de memoria constan en `docs/VALIDACION-BUILD-2026-10-14.md`.

### Distribución
- El build supervisado aísla los módulos PowerShell 7 heredados al lanzar Windows PowerShell 5.1.
  Esto resuelve el conflicto de `ExtendedTypeData` que impedía cargar la verificación Authenticode;
  la regeneración de W-10B terminó correctamente con ese entorno.
- El README y las próximas notas de release enlazan la política con el rótulo requerido “Code
  signing policy”. La única release pública existente, `v0.3.0`, aún no contiene el enlace; el
  repositorio local no modifica releases ya publicadas.
- Preparado el empaquetado para firmar/verificar `naygo.exe` antes de crear portable/setup y
  firmar/verificar el instalador final. El workflow puede enviar ambos artefactos a SignPath
  Foundation; W-14 sigue abierto hasta que aprueben el proyecto, se configure la cuenta y se
  complete una ejecución real. La verificación remota exige estado `Valid` y publicador
  `SignPath Foundation` en ambos artefactos, y conserva en el log los datos del certificado. El build
  local aborta si detecta una firma Authenticode inválida;
  sin certificado sólo acepta archivos `NotSigned` o con firma `Valid`. La distribución 0.5.2
  existente permanece sin firma.
- Las configuraciones SignPath validan ahora `ProductName=Naygo` y `ProductVersion` contra la
  versión de Cargo que el workflow pasa como parámetro. Se añadió un borrador enlazado desde README
  con responsabilidades, privacidad, alcance y estado pendiente; aún debe revisarse/publicarse y
  la solicitud OSS debe ser aceptada antes de distribuir artefactos firmados. Inno Setup incluye
  padding al final de los campos de texto PE del instalador; falta confirmar que SignPath lo normalice
  al evaluar sus restricciones en la primera solicitud real.

### Linux (sin publicar)
- Nicolás aprobó conservar la bandeja en la primera alfa y aceptar GTK 3/AppIndicator. La revisión
  confirma GTK 3 LGPL-2.1-or-later y wrappers Rust MIT/Apache; quedan pendientes el copyright y los
  notices del paquete Ubuntu concreto. Hallazgo adicional: `libappindicator-sys 0.9.0` hace `panic!`
  al inicializar si no encuentra biblioteca AppIndicator, por lo que Linux debe detectar/desactivar
  esa capacidad o declarar la dependencia en el paquete antes de exponerla.
- Nicolás autorizó una excepción sólo para Linux para GTK LGPL, con enlace dinámico y notices; no
  cambia la política de Windows ni otras dependencias. Queda revisar el paquete Ubuntu exacto.
- El 15 de octubre Nicolás autorizó distribuir la edición Linux de la alfa bajo GPL-3 si la
  migración al backend GLib lo requiere; Windows mantiene MIT. Falta cerrar derechos/contribuciones,
  fuente correspondiente, notices y validación del paquete; el trabajo Linux sigue pospuesto por
  prioridad Windows.
- La consulta del catálogo oficial encontró el runtime GLib de Ayatana en Ubuntu 26.04, mientras
  Noble 24.04 muestra la implementación GTK antigua. Nicolás fijó Ubuntu 26.04 LTS x86_64 como
  mínimo de la alfa; no habrá backport a Noble en esta fase. Validación de entorno/licencias y matriz
  de escritorios siguen pendientes; fuentes y alcance en el
  [plan Linux](docs/plans/2026-10-13-linux-ubuntu-luna.md).
- El parser compartido de ruta ahora conserva en POSIX espacios iniciales/finales, comillas y
  backslash, y no aplica el límite de 32.767 unidades UTF-16 de Windows. Esto es un subpaso de L-01A;
  la UI Linux y sus operaciones aún no están habilitadas.
- La planificación de copy/move y la guarda previa a borrar comparan igualdad y pertenencia de
  rutas con las reglas de cada sistema: Windows conserva su comparación case-insensitive; Unix
  distingue mayúsculas, componentes y bytes nativos. Esto permite planificar operaciones entre
  directorios POSIX `A` y `a` sin confundirlos; no habilita por sí solo las operaciones Linux.
- Las claves de identidad léxica usadas por guards de rename/duplicate/undo, sync, task spaces,
  búsquedas guardadas, devolución de selección, cesta, planificación de entregas y radar de destinos
  preservan case y bytes de ruta en Unix. El plan de entregas detecta solapamientos por componentes.
  Windows conserva el criterio previo. No implica que las operaciones Linux estén habilitadas ni
  resuelve symlinks o transporte de bytes arbitrarios por la UI.
- El preview de renombrado por lotes conserva nombres UTF-8 POSIX válidos y muestra bytes no UTF-8
  escapados, marcando esas filas como no editables para evitar que un nombre se sustituya
  silenciosamente por texto `U+FFFD`. La UI general y los formatos serializados todavía requieren
  una política común de transporte para rutas con bytes arbitrarios.
- L-01A: las propuestas de nombres de grupo para entregas muestran bytes POSIX no UTF-8 como `\xNN` mediante la
  representación compartida de rutas; así no convierten silenciosamente el nombre de origen en
  `U+FFFD`. Un test Unix diferencia bytes inválidos de backslashes literales; compila en el check
  cruzado Linux, pero requiere ejecución nativa.
- `naygo-core` y `naygo-platform` pasan `cargo check --tests` cruzado a Linux y Clippy con
  `-D warnings`; los stubs explicitan que espacio, carpetas conocidas y Papelera no están disponibles.
  Los recursos/helpers propios de Windows ya no generan warnings al compilar la variante Linux.
  La suite Windows, Clippy, formato y diff pasaron, y se regeneró la distribución 0.5.2 bajo supervisor;
  los hashes figuran en `docs/VALIDACION-BUILD-2026-10-14.md`. Build y ejecución nativos Ubuntu siguen
  pendientes.
- La distribución Windows 0.5.2 se volvió a generar después de los cambios de parser e identidad de
  rutas: el build supervisado
  terminó dentro del tope de 12 GiB. EXE y setup siguen sin firma Authenticode; hashes y límites en
  `docs/VALIDACION-BUILD-2026-10-14.md`.

### CI
- El test de ida/vuelta de Papelera vuelve a ejecutarse en CI. El runner Windows no refrescaba de
  forma fiable la enumeración virtual del Recycle Bin y tampoco tenía disponible el verbo de menú
  `undelete`; ahora el callback `PostDeleteItem` conserva el `IShellItem` recién reciclado y la
  restauración prioriza `IFileOperation::MoveItem`, con las rutas anteriores como respaldo. El test
  comprueba que Shell entregó un elemento reciclado y lo restaura a su ruta temporal. Pasó aislado y
  en el gate local completo (1.381 tests aprobados, 6 ignorados; Clippy limpio); falta el run remoto
  para confirmar el comportamiento en Windows Server 2025.

### Rendimiento
- La inspección de canvas SVG ahora examina los bytes de la etiqueta raíz sin crear dos copias
  temporales del documento completo. El máximo de entrada y de bitmap renderizado no cambia.
- El preview ZIP/TAR/TAR.GZ limita registros e índice ZIP, bytes leídos/comprimidos/expandidos,
  rutas internas y suma de tamaños. Mantiene hasta 500 filas, sin extraer contenido; los límites
  conservan una salida parcial o muestran el aviso de recursos existente. Suite workspace, Clippy
  y distribución supervisada pasaron; hashes en `docs/VALIDACION-BUILD-2026-10-13.md`.
- El preview PDF carga el documento una vez, obtiene las páginas y reutiliza el parser para extraer
  texto con el formato previo. Un writer detiene la extracción al llegar a 8.000 caracteres y el
  lector recorre hasta 500 páginas de contenido; conserva el aislamiento, timeout y cancelación
  existentes. No renderiza páginas. Suite completa, Clippy y build supervisado pasaron; hashes de
  distribución registrados en `docs/VALIDACION-BUILD-2026-10-13.md`.
- El acceso cacheado a índices de vista comparte un `Arc<[usize]>` con el render y la comparación
  de filas, sin copiar el vector entero en cada hit. En el microbenchmark a 100.000 filas bajó de
  0,021924 ms con copia a 0,000061 ms compartido; el resultado no mide el frame completo. Suite
  workspace, Clippy global, formato, diff y Graphify pasaron. La distribución 0.5.2 se regeneró
  bajo supervisor y sus hashes coinciden con el manifiesto; evidencia y límites en
  `docs/BENCHMARKS.md` y `docs/VALIDACION-BUILD-2026-10-13.md`.
- Files, árbol y el diagnóstico de carpetas comparten un máximo de ocho workers. Cuando no hay
  plaza, Files conserva sólo la última ruta por panel, el árbol deduplica ramas y el probe conserva
  el último ámbito. Los permisos se liberan al terminar el hilo; los probes de navegación consultan
  sólo el panel pedido y descartan revisiones obsoletas.
- El listado ahora usa un canal acotado de 1.024 entradas. Si la UI deja de drenar, el worker
  espera con reintentos cancelables sin un `send` bloqueante; el resultado terminal usa un canal
  aparte y se entrega después de las filas anteriores. Las suites core/UI, Clippy y build
  supervisado pasaron. El tick comparte 1.024 filas/3 ms entre paneles y árbol, con cuotas y
  rotación entre listados. Suite completa, Clippy y distribución supervisada pasaron; la comparación
  de latencia y tiempo total continúa pendiente.
- Agregadas métricas locales opt-in (`NAYGO_PERF_METRICS=1`) para tick, actualización de UI,
  listado y construcción de previews. La cola y las muestras están limitadas; el event loop usa
  envío no bloqueante. El log al cierre contiene solo tipos estáticos y duraciones, sin rutas ni
  nombres de archivo. La distribución 0.5.2 se regeneró bajo supervisor; hashes y limitaciones
  figuran en `docs/VALIDACION-BUILD-2026-10-13.md`. La matriz de rendimiento W-07 continúa pendiente.
- Los mismos logs opt-in informan inicio, término, rechazos, trabajos activos y pico simultáneo
  de workers de preview y validación de rutas, usando los permisos reales que limitan esos hilos.
  El worker serial de packs también registra envíos, rechazos, espera de cola y duración. Una prueba
  del cierre integrado verifica el drenaje de muestras antes de unir al colector. Las mediciones
  reales de rendimiento siguen pendientes.

### Corregido
- Si el tope de 64 KiB del preview cae dentro de un carácter UTF-8/UTF-16, se omite sólo la unidad
  incompleta del final; el resto del archivo conserva su decodificación correcta.
- La ventana ya no explora temas ni sets de íconos de usuario durante su construcción. Arranca con
  assets embebidos y aplica los catálogos externos al recibir la respuesta del worker. Lecturas de
  PNG externos, packs, copia de íconos, guardado/borrado de temas, logging ordinario, verificación
  de pegado y reintentos de carpetas perdidas corren fuera de los callbacks de UI.
- Settings en el arranque conservan el ID de un set de íconos propio sin consultar el directorio;
  la resolución se completa al aplicar el catálogo cargado por el worker.
- Los listados distinguen éxito completo, cancelación, fallo y lectura parcial. Un resultado
  incompleto ya no actualiza snapshots de visitas ni recientes; el fallo del worker cierra el
  polling y muestra un aviso. Permisos y errores de red no se confunden con carpetas ausentes.
- El resaltado de cambios de visita consulta rutas completas una sola vez por comparación;
  archivos homónimos de otra carpeta ya no reciben una marca falsa.
- Los snapshots de visita conservan solo ruta, tipo, tamaño y modificación, con LRU y límites
  provisionales de 32 MiB estimados / 100.000 entradas. La expulsión inicia una comparación nueva
  y no se presenta como eliminación.
- El nombre inicial del diálogo de renombrado de conflictos se calcula sin consultar el disco en
  el hilo de UI; el motor mantiene la revalidación y desambiguación al ejecutar.
- La preconsulta de colisiones de arrastre ahora corre en un worker acotado. Los errores de acceso
  no se tratan como destino libre, el staging vive hasta el resultado y el motor revalida al
  ejecutar; copiar/mover ya no consulta `exists()` desde el controlador de operaciones.
- Generada nuevamente la distribución 0.5.2 con W-01–W-06C; ver checksums y límites en
  [validación del build del 13 de octubre](docs/VALIDACION-BUILD-2026-10-13.md).
- Los cambios de ajustes, atajos y sesión se coalescen por archivo en un worker JSON único;
  al salir, Naygo espera las escrituras finales fuera del hilo de UI con plazo acotado.
- La carga inicial de settings, idioma, atajos, sesión, favoritos y plantillas corre en un worker;
  la ventana parte con recursos embebidos y conserva las ediciones que ocurran mientras carga.
  El cierre se difiere hasta recibir el snapshot. Si el worker falla, no se guardan defaults encima
  de datos existentes. Las escrituras de favoritos y plantillas también son asíncronas.
- La escritura atómica crea el directorio padre cuando aún no existe, para que una instalación
  nueva pueda guardar favoritos o plantillas desde el primer uso.

## [0.5.2] — 2026-09-10

### Construcción de menor consumo — 2026-09-14

- Distribución con ThinLTO, 16 unidades de código y un solo trabajo Cargo; conserva
  optimización de tamaño, CRT estático y símbolos completos de diagnóstico en PDB.
- Supervisor opcional de compilación con límites de memoria, prioridad baja y parada
  por margen insuficiente. Retiradas dos declaraciones de dependencias sin consumidores;
  sin quitar funciones ni modificar las dependencias efectivas del producto.

### Lecturas de texto/3D y memoria temporal — 2026-09-14

- Texto, resaltado de código y carga STL/3MF pasan al lector auxiliar con cancelación,
  límite de tiempo y memoria. Un bloqueo de lectura ya no queda alojado en el proceso de UI.
- Se conservan las reglas de lenguaje, copia parcial y truncado. El transporte valida que
  los segmentos coloreados reconstruyan exactamente el texto visible.
- La geometría 3D vuelve por un protocolo binario limitado a 48 MiB; las rotaciones
  posteriores siguen trabajando en memoria, sin releer ni iniciar procesos por gesto.
- Admisión de trabajo temporal compartida y rasterizador con scratch máximo de 64 MiB;
  elimina una copia de vértices durante la proyección. Los límites no equivalen al RSS
  total de Naygo. Sin nuevas dependencias de producto ni cambios de configuración.

### Preview económico y retención acotada

- Previsualización incorpora «Modo económico», persistente y desactivado por defecto:
  imágenes/SVG hasta 1024 px, animaciones hasta 512 px / 10 presentaciones por segundo
  y raster 3D de 512 × 384. Conserva proporciones, composición y preferencias existentes.
- La imagen Slint se construye una vez por resultado; todos los paneles comparten su handle
  en vez de copiar RGBA en cada refresco del listado.
- Presupuesto compartido de 64 MiB para contenido retenido del preview, con reservas
  conservadoras de imagen/texto y geometría 3D. Las escenas conservan su reserva mientras
  un renderer retirado siga usándolas. No es un límite del consumo total del proceso.
- Cambiar calidad cancela la generación anterior; los errores de admisión son terminales
  con reintento explícito. Diez idiomas; sin nuevas dependencias ni cambio de esquema.

### Navegación, creación y refresco
- Al subir al padre por teclado o botón, la carpeta de origen queda seleccionada y visible.
  El árbol acompaña la carpeta abierta; al crear carpetas actualiza la rama correspondiente.
- Las carpetas creadas se seleccionan tras terminar la operación y el listado, no antes.
  El cuadro de creación se abre vacío y con foco, sin conservar el texto anterior.
- Refrescar mantiene selección/foco por ruta. Los cambios que llegan durante un listado
  fuerzan reconciliación al terminar; las operaciones terminadas también actualizan sus paneles.
  El registro del watcher y los avisos de resincronización cierran ventanas de cambios perdidos.
- Resaltado de búsqueda continuo dentro del nombre, sin espacios artificiales entre fragmentos.
- Pegados de texto e imagen: `{fecha}` usa `YYYY-MM-DD HH.MM.SS` (UTC, como antes).
  Las escrituras simultáneas reservan nombres únicos y conservan ambos resultados.

### Incluido desde los builds locales de 0.5.1

### Preview: imágenes animadas bajo demanda — 2026-09-09
- WebP, GIF y APNG (`.png`/`.apng`) animados, con Pausar/Reanudar y Repetir al terminar.
  Se conservan ajuste/1:1, transparencias, composición de frames parciales y repeticiones.
- Decodificación secuencial en el mismo EXE aislado, sin cargar todos los frames. Salida
  hasta 768 px y 15 presentaciones/s; canvas animado hasta cuatro millones de píxeles.
  Los delays menores de 10 ms se normalizan; se descartan presentaciones, no composición.
- Reloj propio que actualiza sólo el preview. Pausa sin nuevos pedidos, sin reloj residual
  para imágenes estáticas; ocultar/cambiar archivo cancela y libera el lector.
- En animación el límite de 5 s se aplica por pedido sin respuesta, no a pausas ni al ciclo
  completo. Job de 192 MiB con tasa CPU máxima del 20 %, prioridad baja y sin corte de 60 s.
  No equivale a memoria global ni a consumo habitual. Sin nuevas dependencias de producción.

### Preview: lectores pesados aislados — 2026-09-09
- Imágenes, SVG, PDF y listados de comprimidos usan un modo interno del mismo EXE,
  anterior a UI/configuración/instancia única. Cancelar termina el lector aun bloqueado;
  las plazas se conservan hasta confirmar su salida y liberar los canales.
- Límites Windows por lector: 192 MiB de memoria comprometida, 5 segundos de CPU y
  5 segundos de tiempo total. Fallos/límites requieren Reintentar; sin fallback ilimitado.
  No son un presupuesto global de memoria ni un sandbox de seguridad.
- Protocolo con versión, tipos/dimensiones/longitudes comprobados antes de reservar memoria.
  SVG con canvas enorme queda acotado a 2048 px y no resuelve imágenes de archivos externos.
- En esta primera entrega WebP/GIF eran estáticos. Animación, lecturas bloqueadas de texto/3D y presupuestos
  globales permanecen pendientes; se conserva selección, scroll, escala y cámara.

### Preview: cancelación y concurrencia acotada — 2026-09-09
- Cancelar disponible desde el debounce, durante espera y sobre miniaturas con trabajo 3D.
  Cancelaciones/errores son terminales hasta Reintentar o seleccionar otro archivo.
- Máximo dos workers de preview, incluidos los cancelados que aún no terminan; las nuevas
  selecciones conservan sólo el último objetivo. Sin join ni espera de I/O en la interfaz.
- Ocultar/minimizar, cambiar pestaña visible o maximizar otro panel suspende el preview y
  libera contenido/escena. Al volver se carga el objetivo actual, sin reintentos implícitos.
- Gestos 3D agrupados en la última cámara pendiente, resultados aislados por solicitud y consulta
  de cancelación durante proyección y relleno de triángulos. Sin dependencias nuevas.
- Límite de esta entrega: cancelación cooperativa; aislamiento de lectores no cooperativos,
  límites globales de memoria y animaciones continúan pendientes.

### Temas y organización de Configuración — 2026-09-09
- Seis paletas claras adicionales: Nieve, Perla, Arena, Salvia, Bruma y Grafito claro.
  Se mantienen los nueve temas previos, el tema por defecto y las preferencias elegidas.
- Énfasis de íconos/texto separado del fondo de archivos recién añadidos; nuevo token
  editable y exportable, con respaldo compatible para los temas antiguos.
- Un único catálogo de sets en Íconos. Apariencia conserva temas, fuentes y densidad,
  con acceso directo a los ajustes de íconos que no cambia el set seleccionado.
- Editor de colores agrupado por función, con índices anteriores conservados, y parser
  de colores que rechaza entradas Unicode inválidas sin provocar un cierre.

### Ruta superior — 2026-09-09
- Clic en la ruta superior o en su lápiz abre el editor compartido, expandido al ancho
  de la barra, con sugerencias, validación sin bloquear y cancelación con Esc.
- Destino capturado al abrir: cambiar de panel no redirige la confirmación. Cerrar o
  navegar el destino invalida el borrador; copiar/favorito usan la carpeta confirmada.
- Acciones con íconos vectoriales, tooltips y nombres accesibles; sin dependencias ni
  cambios de preferencias. Ctrl+L/F4 conservan la edición del panel.

### Rutas de panel — 2026-09-09
- Acceso directo con lápiz y editor nativo compartido; conserva cursor, selección y
  deshacer, incluido el separador `\` insertado en medio y caracteres con AltGr.
- Enter comprueba carpetas fuera de la interfaz: errores y timeout conservan el borrador
  y la carpeta anterior; Escape cancela. Rutas relativas se resuelven respecto del panel.
- Validación ligada a panel/carpeta/revisión, con dos workers como máximo, incluidos
  los cancelados que siguen esperando al SO. Timeout lógico de cinco segundos.
- Autocompletado asíncrono también al abrir y escoger sugerencias (incluida búsqueda),
  un worker por campo y descarte por generación; A → B → A no recupera resultados antiguos.
- Documentación consolidada de estado, arquitectura, pruebas y ampliaciones pendientes.

## [0.5.1] - 2026-09-05

### Revisión de usabilidad — 2026-09-07
- Tipografía configurable en Apariencia: interfaz con fuente del sistema por defecto,
  listados que heredan de ella y vista previa monoespaciada. Sugerencias y nombre de fuente
  instalada, muestra en vivo y persistencia; sin distribuir fuentes ni agregar dependencias.
- Refresco por panel y botón global para paneles abiertos, ramas desplegadas y unidades.
  F5 conserva su alcance local. El refresco también rearma la vigilancia de la carpeta.
  El árbol restaura sus ramas de padre a hijo y respeta colapsos manuales durante la carga.
- Corregida la carrera del watcher: se despierta la UI después de resolver metadata y
  publicar el lote final. Generaciones descartan eventos de una navegación anterior.
  Alta y liberación del watcher nativo fuera del hilo UI.
- Selección y arrastre de bandeja conservan sus filas durante las actualizaciones de foco.
- Íconos de pausa, reanudación, salto, cancelación y unidades centrados verticalmente.

### Añadido — revisión posterior al respaldo en GitHub
- Bandeja con etiquetas al desactivar «Solo íconos» y acciones secundarias desplegables.
- Acceso a los diez espacios recientes de la sesión, con revisión previa y opción de limpiar.
- Filtro de fallidos en historial/detalle y reintento revisado de archivos de copiar/mover.
  Excluye éxitos, saltados, carpetas y otras operaciones; revalida antes de ejecutar, consulta
  conflictos y advierte que no registra deshacer. Sin nuevas dependencias.

### Corregido
- Rutas de panel con ancho natural de cada segmento, sin repartir huecos entre carpetas.
  Conservan el extremo actual cuando falta ancho y todos los ancestros en el menú de ruta.
- Acciones de la bandeja arriba, en una sola fila cuando caben y ajuste a varias filas sólo
  al estrechar el panel. Vaciar permanece separado abajo; se conservan tooltips y teclado.
- Texto MIT completo, seleccionable y desplazable en Acerca de, embebido desde LICENSE.
  Copyright actualizado con el correo del autor, sin cambiar los términos de la licencia.

## [0.5.0] - 2026-09-05

### Añadido
- Recetas declarativas (.naygorecipe) desde Disposiciones/Ctrl+P: selección o consulta,
  filtros, nombres fechados y carpeta/ZIP nuevo mediante revisión y ejecución explícitas.
  Parámetros de raíces/destino separados del documento portable, referencias en espacios,
  guardado protegido por revisión y resultado en Operaciones. Sin scripts ni tareas residentes.
- Filtro «Este mes» en consultas y recetas, resuelto con el calendario local al preparar.
- Puntos de comparación locales (.naygopoint) desde Disposiciones/Ctrl+P: captura cancelable,
  alcance y exclusiones explícitos, SHA-256 opcional, comparación conservadora con cobertura
  parcial, selección de archivos presentes para bandeja, exportación y retirada del punto a papelera.
  No son respaldos ni copian/eliminan los archivos enumerados.
- Búsquedas guardadas multiraíz (.naygosearch): criterios por nombre/contenido, tamaño y fecha
  relativa, carga sin ejecución, streaming cancelable y cobertura parcial explícita. Selección
  por teclado con Preview/Propiedades y envío a bandeja; accesos Ctrl+P y referencias en espacios.
- Espacios por tarea desde Disposiciones o Ctrl+P: guardar/actualizar y abrir archivos
  `.naygospace` con rutas, columnas, filtros, disposición, panel activo y referencias de bandeja.
  Revisión antes de cambiar, raíz portable opcional y protección frente a documentos corruptos
  o modificados externamente. La cola de operaciones continúa con sus destinos originales.
- Entregas: nombres de grupo editables, revisión conjunta de homónimos y propuesta explícita de
  sufijos únicos en modo plano. Resultado no modal con abrir carpeta de salida, copiar ubicación
  y recuperar las fuentes marcadas en la bandeja; los fallos conservan su error en el historial.
- Configuración: el buscador encuentra secciones por las etiquetas de sus ajustes, además del
  nombre de categoría. Navegación lateral con foco visible y activación Enter/Espacio.
- Preparar entregas desde la selección o la bandeja (también desde Ctrl+P): revisión previa,
  grupos por origen, estructura plana o raíz relativa explícita, carpeta nueva o ZIP, manifiesto
  JSON y lista legible sin rutas privadas. SHA-256 opcional, cancelación y deshacer del destino.
- Selección múltiple en la bandeja con Ctrl/Shift, Ctrl+A, flechas y arrastre del grupo marcado.
  Las acciones usan solo los marcados; quitar referencias no borra originales.
- Maximización temporal de paneles (Ctrl+Shift+M) sin cambiar la disposición guardada.
- Indicador por sesión de archivos nuevos, modificados y desaparecidos desde la visita anterior.
- Radar de destinos para copiar/mover: permite elegir con 1–9 entre paneles, favoritos, recientes
  y destinos de operaciones sin navegar antes a la carpeta.
- Atrás/Adelante restaura foco, selección, scroll y filtros de cada visita durante la sesión.
- Historial interno de portapapeles con `Ctrl+Shift+V`: conserva hasta 10 conjuntos de rutas
  copiados/cortados durante la sesión y permite elegir cuál pegar.
- Exportación del listado visible o seleccionado al portapapeles/CSV, con columnas actuales,
  ruta completa, separador `;` o `,` y BOM UTF-8 para Excel.
- **Buscador F3 ampliado**: raíz editable desde el panel activo, coincidencia opcional sensible
  a mayúsculas, comodines Windows `*`/`?`, recorrido recursivo configurable y texto dentro de
  archivos de texto. Los resultados llegan en vivo con ruta relativa y acción para abrir su
  carpeta contenedora.
- El árbol muestra accesos directos a las carpetas conocidas de Windows (Escritorio,
  Documentos, Descargas, Imágenes, Música y Videos), además de las ubicaciones físicas de
  OneDrive y Dropbox, incluso cuando fueron reubicadas a otra unidad.

### Corregido
- Cambiar de panel conserva las marcas de la bandeja sin robar el contexto del preview.
  Sus atajos ya no operan por accidente sobre un panel Files anterior; borrar confirma
  los originales concretos y no vacía las referencias si se cancela o falla.
- Rutas largas mantienen disponibles sus acciones laterales; la columna ordenada recibe un
  tinte discreto exclusivamente en el panel activo, sin tapar selección ni alertas.
- Los archivos arrastrados directamente desde 7-Zip, WinRAR y otras fuentes OLE virtuales
  ahora se extraen en segundo plano y entran al flujo normal de copia. Naygo valida sus rutas,
  no bloquea la interfaz y limpia el staging temporal al terminar o cancelar.
- Marcar un panel de archivos ahora revela y selecciona inmediatamente su carpeta en el árbol
  común o dedicado correspondiente.
- Soltar archivos sobre la Bandeja temporal ya no parpadea ni crea movimientos vacíos: agrega
  las referencias al instante y resalta la Bandeja como destino válido durante el arrastre.
- Enviar a la papelera con la confirmación desactivada ya no pierde la lista de rutas; el panel
  informa el número real de elementos y detecta cancelaciones/errores del Shell de Windows.
- Las instalaciones actualizadas reciben las reglas STL/3MF sin sobrescribir preferencias de
  preview existentes. Los 3MF de slicers con geometría externa usan su miniatura incrustada,
  más fiel y liviana que descomprimir cientos de MB para rasterizarlos.

## [0.4.0] - 2026-08-16

### Añadido
- **Asistente de sincronización entre paneles**: compara dos carpetas en segundo plano,
  presenta un plan seleccionable y ejecuta izquierda→derecha, derecha→izquierda o
  bidireccional. El borrado de sobrantes es opcional y parte desactivado.
- **Bandeja temporal de selección**: reúne rutas de carpetas distintas sin copiar datos y
  permite copiarlas, moverlas o enviarlas a la papelera como un lote.
- **Transformación de archivos de texto**: CRLF, LF y CR clásico; UTF-8/UTF-16 con o sin
  BOM, control del salto final y limpieza de espacio final. Trabaja fuera del hilo de UI,
  con cancelación y reemplazo transaccional.
- **Preview 3D STL/3MF por CPU**: vista isométrica estática sin depender de GPU, junto con
  dimensiones, cantidad de triángulos y unidad declarada en 3MF. Respeta el `build`,
  componentes anidados y sus matrices de transformación, con protección ante ciclos.
- **Exploradores enlazados Árbol + Archivos**: desde el menú Panel se crea una pareja
  visual 28/72 cuyo árbol navega exclusivamente su panel de archivos. Cada árbol puede
  alternarse en caliente entre modo **común** (sigue al último panel Files activo) y
  **dedicado**; los enlaces sobreviven al reinicio y al guardar una plantilla.
- El borrado de lotes grandes muestra planificación, cantidad/tamaño, archivos en curso,
  velocidad, progreso y cancelación en el panel de Operaciones. La confirmación puede incluir
  una vista previa paginada y desactivarse desde Configuración.
- Los ejecutables pueden abrirse **como administrador** manteniendo Shift o desde el menú
  contextual, con elevación nativa de Windows.
- Comparación rápida entre paneles y mejoras visuales inspiradas en exploradores tipo
  Commander/Directory Opus, manteniendo el renderer por software y el bajo consumo.
- **Más temas de fábrica** (9 en total): **Commander** (look tipo Total Commander: fondo
  negro, carpetas en cian, fechas en teal), **Solarized Dark**, **Paper** (claro) y
  **Dracula**, todos con contraste cuidado para legibilidad.
- **Set de íconos Fluent** (Microsoft Fluent UI Emoji, MIT): sexto set de fábrica,
  colorido, generado desde los SVG originales con `gen_icons`.
- **Filtro visual por tipeo**: al escribir en un panel de archivos, las filas cuyo nombre
  **contiene** el texto se tiñen con el acento del tema y el propio tramo coincidente se
  resalta dentro del nombre (sin distinguir mayúsculas ni tildes: «cancion» encuentra
  «canción»). El foco salta a la primera aparición (con la vista desplazándose hasta ella),
  **↓/↑ recorren las coincidencias** (F3/Shift+F3 también), y el marcado persiste hasta Esc
  o navegar. Una **mini-barra** sobre el pie del panel muestra `filtro: "texto" · N
  coincidencias` con botón ✕ para limpiarlo.
- **Barra de desplazamiento** en los paneles de archivos: delgada, visible solo cuando el
  contenido excede lo visible, con manija arrastrable que refleja el tamaño de la carpeta.
- **Título de ventana configurable** (Configuración → General): solo «Naygo», «Naygo —
  ruta del panel activo», o solo la ruta.
- Splash de arranque **centrado** en la pantalla; los avisos (toast) ahora también salen
  **centrados** y duran ~6 segundos.
- Los atajos de teclado quedan registrados en el log de diagnóstico («última acción» ante
  una caída), y el backtrace de los panics sale **simbolizado** (el `naygo.pdb` se incluye
  en el instalador y en el portable).

### Corregido
- **Renombrar inline (F2) funcionaba mal de múltiples formas encadenadas**: la app se caía
  («Recursion detected» de Slint al enfocar el editor dentro del layout), el texto se
  reemplazaba solo al tipear ciertos caracteres, el cursor saltaba al inicio o al final, y
  las flechas/Home/End/Backspace no actuaban dentro del editor. Causas raíz corregidas:
  foco diferido con Timer y sesiones de rename, teclas de edición ya no se las traga el
  panel, el texto del editor ya no se reescribe por tecla, y el modelo de columnas deja de
  re-montar las celdas en cada sincronización (el «pestañeo» al presionar Shift).
- **La vista no seguía al foco** al tipear para buscar ni al navegar con el teclado: la
  propiedad `focused-row` nunca llegaba del controlador al panel. Ahora el desplazamiento
  automático revela siempre la fila enfocada.
- El borrado permanente de carpetas grandes ahora es **cancelable** a mitad de camino.
- Las escrituras de configuración y del journal de operaciones son **atómicas** (tmp +
  rename): un corte a mitad ya no los corrompe.
- Pegar imagen/texto desde el portapapeles ya no escribe en el hilo de UI y **avisa si
  falla** (antes el error se tragaba en silencio). Varios puntos de I/O síncrono en el hilo
  de UI movidos a workers (navegación, autocompletado de ruta, metadata del watcher,
  detalle de archivos de una operación).
- El resaltado de arrastrar-y-soltar podía quedar «pegado» tras un drop fallido.
- `CHANGELOG.md` estaba doble-codificado (se veía «AÃ±adido» en Novedades).

### Cambios
- La UI principal quedó dividida en módulos (`main.rs` pasó de ~5.900 a ~850 líneas:
  `callbacks_*`, `tick`, `vm_builders`, `wire`), y el modelo de filas ahora se actualiza
  de forma **incremental** (quita/inserta solo el tramo que cambió al entrar o salir
  archivos, en vez de reconstruir toda la lista).
- `lopdf` unificado a una sola versión; `image`/`zip`/`tempfile`/`serde_json` centralizados
  en el workspace. El prototipo `proto-slint` se retiró del workspace.
- Releases con **SHA256SUMS**; CI verifica paridad de idiomas; `rust-toolchain.toml`
  añadido; documentación actualizada al stack real (Slint).

## [0.3.0] - 2026-07-01

### Añadido
- Comprimir y descomprimir `.zip`: con archivos o carpetas seleccionados, el menú contextual
  ofrece **«Comprimir en .zip»** (pide el nombre del archivo); con un `.zip` seleccionado,
  **«Extraer aquí»**. Ambas operaciones corren en segundo plano con progreso y cancelación en
  el panel de Operaciones (cancelar una compresión borra el archivo parcial), los conflictos
  al extraer usan el diálogo de conflicto lado a lado, y las dos se pueden **deshacer** de
  forma segura: a la papelera va solo lo que la operación creó, nunca algo preexistente.
  Comprimir jamás pisa un `.zip` que ya exista: desambigua el nombre con «(2)».
- Ocho idiomas nuevos, además de español e inglés: **portugués, francés, alemán, italiano,
  chino, japonés, coreano e hindi** (los cuatro últimos marcados como experimentales). El
  selector de *Configuración → General* muestra cada idioma con su nombre nativo.
- **Íconos personalizables**: cinco sets de fábrica (Lucide, Mono, Tabler, Material y Flat
  Color) que se eligen en una galería de tarjetas en *Configuración → Apariencia*, y una
  pestaña **Íconos** con la grilla de objetos para cambiar el ícono de cualquier objeto
  (carpetas, discos, acciones de la barra…) por el de otro set o por un **PNG propio**. Los
  sets monocromos se tiñen con el color del tema, la barra de herramientas se actualiza en
  caliente, y un set personalizado se comparte como pack `.naygoset`.
- **Importar/Exportar por tópicos**: Idioma, Temas, Íconos y Configuración se exportan e
  importan por separado desde *Configuración → Importar/Exportar*, con extensiones propias
  (`.naygolang`, `.naygotheme`, `.naygoset`, `.naygoconf`).
- La **vista previa de comprimidos** muestra el contenido como un **árbol** con carpetas,
  archivos, tamaños y totales; ahora también para `.tar` y `.tar.gz`, además de `.zip`. Si el
  archivo está dañado, muestra lo que se pudo leer y lo indica («… y más entradas»).
- **Expulsar una unidad USB con paneles abiertos**: el aviso de expulsión indica cuántos
  paneles tienen carpetas de ese disco, Naygo suelta sus vigilancias antes de expulsar y los
  paneles afectados quedan con el aviso «disco expulsado». Si Windows no puede expulsar la
  unidad, el error se informa con claridad.
- Configuración: **iconos de ayuda (?)** con explicación en todas las opciones, y un
  **buscador** que filtra las pestañas por nombre.
- **Divisores de paneles inteligentes**: mover una barra divisoria redimensiona solo sus
  **dos paneles vecinos** (el resto no se mueve), y al cambiar el tamaño de la ventana todos
  los paneles escalan proporcionalmente. Dividir un panel en la misma dirección crea una fila
  pareja (ya no anida cajas dentro de cajas). **Doble clic en una barra divisoria** reparte el
  espacio **50/50** entre sus dos vecinos.
- La ventana **recuerda su tamaño, posición y estado maximizado** entre sesiones. Si el
  monitor donde estaba ya no está conectado, vuelve a una posición visible.
- **Cerrar a la bandeja por defecto**: la **X** ahora esconde Naygo en la bandeja del sistema
  en vez de salir (la próxima apertura es instantánea). Para salir de verdad: clic derecho en
  el ícono de la bandeja → **Salir**. Se puede volver al comportamiento clásico desactivando
  *Configuración → Avanzado → «Cerrar a la bandeja (no salir)»*.
- **Iniciar minimizado en la bandeja**: si Naygo inicia con Windows, puede arrancar directo
  en la bandeja sin mostrar la ventana (*Configuración → General*; requiere «Iniciar con
  Windows» y el ícono de bandeja). El instalador usa el nuevo argumento `--tray`.
- **Atajo global Ctrl+Alt+Q** (configurable en *Configuración → Avanzado*, activado de
  fábrica): muestra Naygo y lo trae al frente **desde cualquier aplicación**; si ya está al
  frente, lo esconde (esconder requiere el ícono de bandeja). La tecla Windows no se puede
  usar en la combinación (la reserva el sistema); si Windows rechaza una combinación —por
  ejemplo porque otra aplicación ya la usa—, Naygo avisa y **conserva la anterior**.
- Instalador: el asistente está en **siete idiomas** (inglés, español, alemán, francés,
  italiano, portugués y japonés) y agrega un paso para elegir el **idioma inicial de Naygo**
  entre los diez disponibles, preseleccionado según el idioma de Windows. Nueva casilla
  opcional «**Iniciar Naygo al arrancar Windows**».

### Cambiado
- Configuración reorganizada: pestañas agrupadas por afinidad (General, Apariencia, Íconos,
  Previsualización, Operaciones, Pegado, Atajos, Avanzado, Importar/Exportar, Acerca de) y el
  **idioma** ahora vive en **General**, con sección propia.
- La pestaña **Pegado** concentra todo lo del portapapeles: nombre del archivo de imagen
  pegada, **formato (PNG/JPG)** —que antes estaba en Avanzado— y **calidad JPG** (escala
  1–100 consistente en toda la aplicación).
- El set de íconos por defecto pasa a ser **Lucide**; las configuraciones existentes se
  migran automáticamente al set equivalente.
- Español neutral en toda la interfaz (se eliminaron los voseos).
- Menos consumo con carpetas grandes: las filas del panel no se reconstruyen si nada cambió,
  la selección y los íconos se resuelven sin copias por fila, y el tipeo rápido, la vista
  profunda y los ordenamientos trabajan sin reservar memoria en cada pulsación.

### Corregido
- Al **actualizar desde una versión anterior ya no se pierde la sesión guardada**
  (disposición de paneles y carpetas abiertas): el cambio interno de versión de la
  configuración descartaba la disposición al primer arranque tras actualizar.
- Blindaje contra archivos de configuración corruptos: una geometría de ventana con
  coordenadas inválidas o una disposición de paneles degenerada ya no afectan el arranque
  (se descartan y se usa el valor por defecto).
- Los modificadores **Shift/Ctrl ya no quedan «pegados»** tras un arrastre: cada clic usa los
  modificadores reales del momento (era la raíz de selecciones que se extendían solas).
- El **menú contextual** actúa sobre el panel donde hiciste clic derecho (antes las acciones
  podían caer en otro panel).
- **Arrastrar archivos** toma como origen el panel donde empezó el gesto, no el panel activo.
- **Deshacer un movimiento** devuelve cada archivo a su carpeta de origen correcta.
- Cerrar una pestaña o un panel **cancela sus listados en curso** (antes seguían trabajando
  de fondo), y los nombres terminados en espacio o punto se manejan bien.
- Traducciones: el panel de operaciones, las pestañas de panel (que además se re-rotulan al
  cambiar de idioma), la barra de estado, los encabezados de la vista previa de PDF y los
  textos de exportación ya siguen el idioma elegido (quedaban fijos en español o inglés).
- La vista previa muestra los archivos **desde el inicio** y respeta las **reglas por
  extensión** definidas por el usuario también para extensiones que Naygo no conoce.
- Los rótulos largos de menús y de los botones del diálogo de conflicto se recortan con «…»
  en vez de desbordar, y los botones de los diálogos **crecen con el texto** (las
  traducciones largas ya no se cortan).
- El tooltip de ayuda de Configuración se ajusta a su texto (ocupaba toda la ventana).
- Las guardas que impiden borrar un origen al copiar o mover comparan las rutas sin
  distinguir mayúsculas/minúsculas (más seguro en Windows).

## [0.2.0] — 2026-06-27
### Añadido
- Diálogo de conflicto de archivo estilo Directory Opus: cuando una copia o movimiento va a pisar un
  archivo que ya existe, se muestran lado a lado el archivo **Existente** y el **Nuevo** con su nombre,
  tamaño, fecha y tipo, para compararlos de un vistazo. Botones directos: Saltar, Mantener ambos
  (renombra el nuevo), Sobrescribir; y un desplegable «Más opciones» con Renombrar, **Renombrar el
  existente**, **Saltar idénticos** (salta sin preguntar los archivos iguales en tamaño y fecha) y
  Sobrescribir/Saltar todos.
- El historial de operaciones muestra **qué archivos** se movieron o copiaron: los nombres en la propia
  fila si son uno o dos, y un enlace «Ver N archivos» que abre un popup con la lista completa (con la
  ruta relativa, el tamaño y el estado de cada archivo, más el origen, destino y estadísticas de la
  operación). El popup aprovecha el tamaño de la ventana.
- Botón «Cancelar todo» en el panel de Operaciones para abortar de una vez todas las operaciones en
  curso, con confirmación previa.
- Atajos de teclado configurables para los botones de la barra de herramientas que no los tenían
  (abrir terminal, dividir panel, refrescar discos, mostrar ocultos, favoritos, configuración,
  disposiciones). El tooltip de cada botón muestra el atajo real asignado.
- Al arrastrar archivos entre paneles, Naygo puede pedir confirmación («¿Copiar/Mover X a «carpeta»?»,
  nombrando los archivos). La confirmación es opcional (Configuración → General → «Preguntar al
  arrastrar entre paneles»). Si el archivo ya existe en el destino, se va directo al diálogo de
  conflicto sin preguntar dos veces.
- Editor de temas: nuevo color «fila de panel inactivo» y casilla «paneles inactivos planos» para
  atenuar o aplanar los paneles que no están activos, de modo que el panel activo resalte más.
- Al copiar o mover una carpeta que ya existe en el destino, Naygo pregunta una sola vez qué hacer:
  **Fusionar** (copia dentro, preguntando solo por los archivos que choquen), **Reemplazar** (deja la
  carpeta destino con el contenido del origen), **Saltar** o **Cancelar**. Con varias carpetas en
  conflicto, una casilla aplica la misma decisión a todas.
- El cálculo previo de una copia grande (recorrer la carpeta para saber cuánto pesa) ahora ocurre en
  segundo plano: la aplicación ya no se congela. Mientras tanto, el panel de Operaciones muestra
  «Calculando…» con los archivos y el tamaño contados hasta el momento, y se puede cancelar desde ahí.
- Menú de visibilidad en la barra de herramientas (botón con un ojo): muestra u oculta los
  archivos y carpetas marcados como ocultos, los de sistema, y los que empiezan con punto (estilo
  Linux). Cada interruptor es global, se aplica al instante a todos los paneles y al árbol, y se
  recuerda. Por defecto Naygo los muestra todos.
- Favoritos organizables en grupos (carpetas) anidados. Un botón con una estrella en la barra de
  herramientas despliega el árbol de favoritos para saltar rápido a cualquiera. Desde el panel de
  Favoritos se gestionan con clic derecho: crear un grupo, renombrarlo, eliminarlo, y mover un
  favorito o grupo a otro grupo. La estrella ★ de la barra de ruta sigue agregando a la raíz.
- Editor de temas: crea tu propio tema duplicando uno existente y ajustando cada uno de sus colores
  con un selector tipo paleta (presets, colores estándar y «Más colores…» con valores R/G/B y hex),
  viendo el cambio aplicado a toda la aplicación en vivo. Guardar lo conserva como tema tuyo y
  Cancelar revierte. Los temas de fábrica quedan intactos; los temas propios se pueden editar o
  eliminar.
- Panel de Operaciones: muestra la copia o el movimiento en curso con todos los datos —archivo
  actual, barra de progreso, copiado X de Y, velocidad media y pico, tiempo transcurrido y
  restante— más los botones Pausar/Reanudar y Cancelar, la cola de operaciones pendientes y un
  historial reciente. Aparece solo al iniciar una operación. Se puede agregar también desde el
  menú «Panel ▾».
- Pausar y reanudar una copia o movimiento en curso (sin perder lo ya copiado).
- Arrastrar archivos de un panel a otro: dentro del mismo disco mueve, a otro disco copia;
  Ctrl fuerza copiar y Shift fuerza mover. Los archivos arrastrados desde el Explorador de
  Windows también caen en el panel sobre el que se sueltan.
- Paleta de comandos (Ctrl+P): un buscador rápido que filtra acciones, archivos de la carpeta
  actual, carpetas recientes, favoritos y temas con coincidencia aproximada (fuzzy), resaltando
  las letras que coinciden. Se navega con las flechas, Enter ejecuta y Esc cierra. El atajo es
  configurable en Configuración → Atajos.
- Menú de historial en los botones Atrás y Adelante: un triángulo ▾ junto a cada uno despliega
  las carpetas visitadas en esa dirección para saltar directo a una.
- Pie de panel (footer): cada panel de archivos muestra al pie sus propios datos —archivos
  seleccionados sobre el total, bytes marcados, y espacio libre/total del disco de su unidad—.
  La plantilla es global y se elige en Configuración → Avanzado entre varias predefinidas
  (Compacta, Completa, Solo disco, Solo selección) o una personalizada con tokens
  (`{sel} {total} {marked} {free} {disk_total} {pct} {items} {files} {dirs}`), con vista previa
  en vivo. Se puede ocultar.
- Botones Atrás, Adelante e Inicio en la barra de herramientas, al estilo de un navegador.
  Atrás/Adelante se atenúan cuando no hay a dónde ir. El botón Inicio (atajo Alt+Inicio) navega
  a la carpeta de inicio, configurable en Avanzado (vacío = carpeta personal del usuario).
- Resaltado automático de código en la Vista previa: las extensiones de código conocidas se
  resaltan solas en modo Automático. Se puede desactivar con un interruptor en
  Configuración → Previsualización; las reglas por extensión siguen mandando sobre el ajuste global.
- La Vista previa de texto permite seleccionar y copiar el contenido: el texto plano siempre es
  seleccionable, y el código resaltado tiene un botón que alterna a una vista seleccionable
  (selección con el mouse y Ctrl+C).
- La Vista previa resalta el código por colores (XML, JSON, HTML, CSS, JavaScript, C/C++, Java,
  Python, Rust, SQL, Bash, Markdown, YAML, TOML, INI). En Configuración → Previsualización se
  puede forzar el modo de vista por extensión: Automático, ver como texto, ver como imagen, o
  ver como código eligiendo el lenguaje.
- Botón en la Vista previa para abrir el archivo con el programa predeterminado del sistema.
- Avisos de software de terceros: archivo `THIRD-PARTY-NOTICES.md` con las licencias de todas
  las dependencias (todas permisivas; la interfaz usa Slint bajo su licencia *royalty-free*).
  La sección "Acerca de" lo menciona y el archivo se incluye en el portable y el instalador.
- Vista profunda: un modo del panel que lista, de forma plana y con sangría por
  profundidad, todo el contenido de la carpeta actual y sus subcarpetas (recursivo).
  Se activa con el botón de la barra del panel; aparece por streaming y se puede
  cancelar. El doble clic en una carpeta sale del modo y navega a ella.
- Ícono de historial de carpetas en la barra de herramientas: despliega un menú con
  las carpetas visitadas recientemente y, al elegir una, navega el panel activo.
- La cantidad de carpetas recientes recordadas ahora es configurable (1–100, por
  defecto 50) en la sección Avanzado de la configuración.
- Tooltips explicativos en todos los botones de la barra de herramientas y de los paneles.
- Guía para agregar idiomas (`docs/AGREGAR-IDIOMA.md`): basta soltar un `.json` en `lang/`.
- Registro (log) con más contexto para diagnosticar caídas: marca de tiempo en hora local
  legible, "migas de pan" de las acciones recientes, y al ocurrir un error se vuelca el
  estado (carpetas abiertas, tema, idioma, entorno). Todo local, sin telemetría.
- Seis temas de color nuevos con más personalidad: Windows XP, macOS, Verde sobre azul,
  Solarized Dark, Terminal ámbar y Ciruela.
- En la primera ejecución, Naygo arranca con la disposición clásica (árbol + dos paneles de
  archivos + Propiedades + Vista previa) en vez de un solo panel.
- Argumentos de línea de comandos: `naygo.exe <carpeta>` abre esa carpeta al iniciar, y
  `--theme`/`--layout` aplican un tema o una disposición solo para esa sesión. `--help` y
  `--version` muestran la información correspondiente. Útil para accesos directos y para
  "Abrir en Naygo".
### Cambiado
- Barra de herramientas reordenada por función y con íconos redibujados a mano (dividir panel, apilar,
  clonar, intercambiar, panel auxiliar, refrescar discos), más representativos y consistentes.
- Los diálogos de operación (eliminar, conflicto, pegar) tienen un borde sutil, el botón con el foco se
  resalta con un anillo del color de acento (ya no en rojo), TAB mueve el resaltado entre botones, cada
  botón muestra su atajo de teclado, y los botones se reparten a lo ancho del diálogo por igual.
- Los temas de fábrica se redujeron a cinco (Dark Blue, Windows XP, Verde sobre azul, High Contrast
  y Neón Retro). El resto se puede recrear a gusto con el nuevo editor de temas.
- Los avisos y confirmaciones internos (confirmar expulsar una unidad USB, errores al
  importar/exportar packs) ahora usan un diálogo con el tema de Naygo en vez del cuadro
  nativo del sistema. El mensaje de cierre por error inesperado es más claro y el detalle
  técnico queda en el registro.
- El registro de diagnóstico ahora se guarda por día: `naygo-AAAA-MM-DD.log` (antes un único
  `naygo.log`), para no mezclar las corridas y diagnosticar más fácil.
- El aviso de "carpeta no encontrada" ahora refresca el panel al instante al pulsar cualquier
  opción (antes había que redimensionar); "Subir un nivel" solo aparece si hay una carpeta
  superior a la que ir; y al volver atrás a una unidad ausente se ven de nuevo las opciones.
- Los paneles de Propiedades y Vista previa se marcan como panel activo al hacer clic en
  cualquier parte de su cuerpo, no solo en la barra de título.
- El menú de plantillas de disposición aparece junto a su botón en la barra (antes salía lejos).
### Corregido
- Refrescar un panel con F5 ya no **duplica** las filas: antes, re-listar la misma carpeta acumulaba
  los archivos sobre los que ya estaban; ahora reemplaza la lista de forma limpia.
- El popup de confirmación al arrastrar entre paneles ya responde **al primer clic** (antes requería
  dos). El modal no tomaba el foco al aparecer, así que el primer clic se gastaba en enfocarlo.
- Los tooltips de los íconos de la barra de ruta del panel (favorito ★, copiar ruta 📋, vista profunda)
  ahora aparecen **junto al ícono**, no arriba en la barra de herramientas.
- Los menús desplegables de la barra de herramientas aparecen alineados **bajo el botón** que los abre
  (algunos salían desplazados, sobre todo los abiertos por atajo de teclado).
- El botón «Deshacer» del historial de acciones distingue ahora cuando una acción **ya fue deshecha**
  (muestra «Deshecho») en vez de quedar igual; y se diferencia de las acciones que no son deshacibles.
- «Saltar idénticos» en un conflicto nunca sobrescribe sin confirmar: si los archivos difieren, vuelve
  a preguntar en vez de pisar el del destino.
- El nombre del archivo en el diálogo de conflicto es legible también en temas claros.
- El diálogo «Ya existe…» que aparece al copiar sobre un archivo existente ahora permite **cancelar
  toda la operación** (botón «Cancelar todo», además de Escape). Antes el aviso atrapaba: solo dejaba
  Saltar/Renombrar/Sobrescribir y su fondo tapaba el botón Cancelar del panel. Cerrar el aviso por
  fuera (Escape o clic afuera) salta solo ese archivo y continúa.
- Arrastrar las filas de archivos entre paneles vuelve a funcionar (la lista capturaba mal el gesto
  del mouse). También se recuperó la selección por rectángulo arrastrando sobre la lista.
- Copiar o mover archivos grandes ya muestra el avance real (antes la copia en curso parecía
  detenerse): el progreso se actualiza por bytes con velocidad y tiempo restante. Resuelve el caso
  en que pegar y sobrescribir un archivo grande parecía copiar solo unos megas.
- El doble clic en una carpeta vuelve a navegar dentro del panel aunque antes se haya usado un
  atajo con Ctrl (los modificadores ya no quedan "pegados").
- Temas claros (Light, macOS, Windows XP, Citrus Glow): mejor legibilidad. El texto atenuado de
  las columnas (extensión, tamaño, fecha) tiene más contraste, y los fondos blanco-puro pasaron a
  grises/cremas suaves para que no "laven" la vista (se veían demasiado brillantes).
- Aviso "carpeta no encontrada": "Subir un nivel" / "Elegir otra" / "Reintentar" ahora refrescan
  el panel al instante. Antes el panel sí navegaba pero el aviso seguía tapando el contenido.
- Vista previa de código: las líneas largas ya no se cortan; el código se desplaza en horizontal
  como en un editor (conservando los colores y la indentación).
- Al confirmar el borrado de un archivo, pulsar Enter ahora activa "Eliminar" en vez de abrir
  el archivo de fondo. (El teclado del panel se suspende mientras hay un diálogo abierto.)
- Hacer clic en el panel de Vista previa (o Propiedades) ya no pierde la selección del archivo:
  la previsualización sigue al último panel de archivos activo.
- Tras pegar un texto o imagen y crear el archivo, ya se puede seleccionar otro de inmediato
  (antes había que clicar primero el archivo recién creado).
- Caída (o cuelgue con la ventana en blanco) al previsualizar archivos de texto con líneas muy
  largas, frecuente en logs: el renderizador por software no podía posicionar glifos tan a la
  derecha y la app se cerraba con un error. Ahora la Vista previa ajusta las líneas a la columna
  y recorta las larguísimas (igual para el texto extraído de PDF).
- Mayor robustez en equipos y máquinas virtuales sin GPU: Naygo fuerza el renderizador por
  software desde el arranque, sin depender de una GPU acelerada.

## [0.1.0] — 2026-06-18
### Añadido
- Navegación de archivos tipo Commander: paneles dinámicos acoplables, dual-pane,
  ir atrás/adelante (incluidos los botones laterales del mouse).
- Árbol de carpetas con expansión incremental, revelado hasta la carpeta activa y
  navegación por teclado.
- Listado por streaming incremental y cancelable; el filesystem hostil (red caída,
  permisos, rutas que desaparecen) no tumba la app.
- Columnas dinámicas estilo planilla: ordenar, filtrar por tipo de columna y
  reordenar arrastrando.
- Operaciones de archivo entre paneles (copiar, mover, eliminar) con cola opcional,
  progreso y cancelación.
- Renombrado en línea y en cadena, y ventana de renombrado por lotes.
- Búsqueda recursiva por nombre en la carpeta y sus subcarpetas.
- Previsualización liviana: imágenes, SVG (rasterizado), PDF (texto y metadatos),
  texto/código y listado de contenido de archivos ZIP.
- Cálculo de tamaño de carpetas bajo demanda.
- Barra de unidades de disco con espacio libre/total y porcentaje usado; ícono
  propio para unidades USB y expulsión segura desde un menú.
- Detección de discos duros externos USB como extraíbles (por tipo de bus).
- Integración con Windows: menú contextual del shell, "Abrir con", watcher de
  carpeta, detección de dispositivos, arrastrar y soltar, ícono de bandeja y
  arranque opcional con el sistema.
- Internacionalización (español e inglés) y temas intercambiables en caliente, con
  galería de selección y packs de usuario.
- Configuración como ventana nativa: apariencia, atajos, previsualización, plantilla
  de tabla, opciones avanzadas y sección "Acerca de".
- Distribución como ejecutable portable e instalador (Inno Setup) con CRT estático.
