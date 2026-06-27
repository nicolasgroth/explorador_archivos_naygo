# Changelog

Todas las novedades de Naygo se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado Semántico](https://semver.org/lang/es/).

## [Sin publicar]

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
