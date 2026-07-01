# Changelog

Todas las novedades de Naygo se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado Semántico](https://semver.org/lang/es/).

## [Sin publicar]

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
