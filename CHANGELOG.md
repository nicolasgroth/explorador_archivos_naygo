# Changelog

Todas las novedades de Naygo se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado SemÃ¡ntico](https://semver.org/lang/es/).

## [Sin publicar]

## [0.3.0] - 2026-07-01

### AÃ±adido
- Comprimir y descomprimir `.zip`: con archivos o carpetas seleccionados, el menÃº contextual
  ofrece **Â«Comprimir en .zipÂ»** (pide el nombre del archivo); con un `.zip` seleccionado,
  **Â«Extraer aquÃ­Â»**. Ambas operaciones corren en segundo plano con progreso y cancelaciÃ³n en
  el panel de Operaciones (cancelar una compresiÃ³n borra el archivo parcial), los conflictos
  al extraer usan el diÃ¡logo de conflicto lado a lado, y las dos se pueden **deshacer** de
  forma segura: a la papelera va solo lo que la operaciÃ³n creÃ³, nunca algo preexistente.
  Comprimir jamÃ¡s pisa un `.zip` que ya exista: desambigua el nombre con Â«(2)Â».
- Ocho idiomas nuevos, ademÃ¡s de espaÃ±ol e inglÃ©s: **portuguÃ©s, francÃ©s, alemÃ¡n, italiano,
  chino, japonÃ©s, coreano e hindi** (los cuatro Ãºltimos marcados como experimentales). El
  selector de *ConfiguraciÃ³n â†’ General* muestra cada idioma con su nombre nativo.
- **Ãconos personalizables**: cinco sets de fÃ¡brica (Lucide, Mono, Tabler, Material y Flat
  Color) que se eligen en una galerÃ­a de tarjetas en *ConfiguraciÃ³n â†’ Apariencia*, y una
  pestaÃ±a **Ãconos** con la grilla de objetos para cambiar el Ã­cono de cualquier objeto
  (carpetas, discos, acciones de la barraâ€¦) por el de otro set o por un **PNG propio**. Los
  sets monocromos se tiÃ±en con el color del tema, la barra de herramientas se actualiza en
  caliente, y un set personalizado se comparte como pack `.naygoset`.
- **Importar/Exportar por tÃ³picos**: Idioma, Temas, Ãconos y ConfiguraciÃ³n se exportan e
  importan por separado desde *ConfiguraciÃ³n â†’ Importar/Exportar*, con extensiones propias
  (`.naygolang`, `.naygotheme`, `.naygoset`, `.naygoconf`).
- La **vista previa de comprimidos** muestra el contenido como un **Ã¡rbol** con carpetas,
  archivos, tamaÃ±os y totales; ahora tambiÃ©n para `.tar` y `.tar.gz`, ademÃ¡s de `.zip`. Si el
  archivo estÃ¡ daÃ±ado, muestra lo que se pudo leer y lo indica (Â«â€¦ y mÃ¡s entradasÂ»).
- **Expulsar una unidad USB con paneles abiertos**: el aviso de expulsiÃ³n indica cuÃ¡ntos
  paneles tienen carpetas de ese disco, Naygo suelta sus vigilancias antes de expulsar y los
  paneles afectados quedan con el aviso Â«disco expulsadoÂ». Si Windows no puede expulsar la
  unidad, el error se informa con claridad.
- ConfiguraciÃ³n: **iconos de ayuda (?)** con explicaciÃ³n en todas las opciones, y un
  **buscador** que filtra las pestaÃ±as por nombre.
- **Divisores de paneles inteligentes**: mover una barra divisoria redimensiona solo sus
  **dos paneles vecinos** (el resto no se mueve), y al cambiar el tamaÃ±o de la ventana todos
  los paneles escalan proporcionalmente. Dividir un panel en la misma direcciÃ³n crea una fila
  pareja (ya no anida cajas dentro de cajas). **Doble clic en una barra divisoria** reparte el
  espacio **50/50** entre sus dos vecinos.
- La ventana **recuerda su tamaÃ±o, posiciÃ³n y estado maximizado** entre sesiones. Si el
  monitor donde estaba ya no estÃ¡ conectado, vuelve a una posiciÃ³n visible.
- **Cerrar a la bandeja por defecto**: la **X** ahora esconde Naygo en la bandeja del sistema
  en vez de salir (la prÃ³xima apertura es instantÃ¡nea). Para salir de verdad: clic derecho en
  el Ã­cono de la bandeja â†’ **Salir**. Se puede volver al comportamiento clÃ¡sico desactivando
  *ConfiguraciÃ³n â†’ Avanzado â†’ Â«Cerrar a la bandeja (no salir)Â»*.
- **Iniciar minimizado en la bandeja**: si Naygo inicia con Windows, puede arrancar directo
  en la bandeja sin mostrar la ventana (*ConfiguraciÃ³n â†’ General*; requiere Â«Iniciar con
  WindowsÂ» y el Ã­cono de bandeja). El instalador usa el nuevo argumento `--tray`.
- **Atajo global Ctrl+Alt+Q** (configurable en *ConfiguraciÃ³n â†’ Avanzado*, activado de
  fÃ¡brica): muestra Naygo y lo trae al frente **desde cualquier aplicaciÃ³n**; si ya estÃ¡ al
  frente, lo esconde (esconder requiere el Ã­cono de bandeja). La tecla Windows no se puede
  usar en la combinaciÃ³n (la reserva el sistema); si Windows rechaza una combinaciÃ³n â€”por
  ejemplo porque otra aplicaciÃ³n ya la usaâ€”, Naygo avisa y **conserva la anterior**.
- Instalador: el asistente estÃ¡ en **siete idiomas** (inglÃ©s, espaÃ±ol, alemÃ¡n, francÃ©s,
  italiano, portuguÃ©s y japonÃ©s) y agrega un paso para elegir el **idioma inicial de Naygo**
  entre los diez disponibles, preseleccionado segÃºn el idioma de Windows. Nueva casilla
  opcional Â«**Iniciar Naygo al arrancar Windows**Â».

### Cambiado
- ConfiguraciÃ³n reorganizada: pestaÃ±as agrupadas por afinidad (General, Apariencia, Ãconos,
  PrevisualizaciÃ³n, Operaciones, Pegado, Atajos, Avanzado, Importar/Exportar, Acerca de) y el
  **idioma** ahora vive en **General**, con secciÃ³n propia.
- La pestaÃ±a **Pegado** concentra todo lo del portapapeles: nombre del archivo de imagen
  pegada, **formato (PNG/JPG)** â€”que antes estaba en Avanzadoâ€” y **calidad JPG** (escala
  1â€“100 consistente en toda la aplicaciÃ³n).
- El set de Ã­conos por defecto pasa a ser **Lucide**; las configuraciones existentes se
  migran automÃ¡ticamente al set equivalente.
- EspaÃ±ol neutral en toda la interfaz (se eliminaron los voseos).
- Menos consumo con carpetas grandes: las filas del panel no se reconstruyen si nada cambiÃ³,
  la selecciÃ³n y los Ã­conos se resuelven sin copias por fila, y el tipeo rÃ¡pido, la vista
  profunda y los ordenamientos trabajan sin reservar memoria en cada pulsaciÃ³n.

### Corregido
- Al **actualizar desde una versiÃ³n anterior ya no se pierde la sesiÃ³n guardada**
  (disposiciÃ³n de paneles y carpetas abiertas): el cambio interno de versiÃ³n de la
  configuraciÃ³n descartaba la disposiciÃ³n al primer arranque tras actualizar.
- Blindaje contra archivos de configuraciÃ³n corruptos: una geometrÃ­a de ventana con
  coordenadas invÃ¡lidas o una disposiciÃ³n de paneles degenerada ya no afectan el arranque
  (se descartan y se usa el valor por defecto).
- Los modificadores **Shift/Ctrl ya no quedan Â«pegadosÂ»** tras un arrastre: cada clic usa los
  modificadores reales del momento (era la raÃ­z de selecciones que se extendÃ­an solas).
- El **menÃº contextual** actÃºa sobre el panel donde hiciste clic derecho (antes las acciones
  podÃ­an caer en otro panel).
- **Arrastrar archivos** toma como origen el panel donde empezÃ³ el gesto, no el panel activo.
- **Deshacer un movimiento** devuelve cada archivo a su carpeta de origen correcta.
- Cerrar una pestaÃ±a o un panel **cancela sus listados en curso** (antes seguÃ­an trabajando
  de fondo), y los nombres terminados en espacio o punto se manejan bien.
- Traducciones: el panel de operaciones, las pestaÃ±as de panel (que ademÃ¡s se re-rotulan al
  cambiar de idioma), la barra de estado, los encabezados de la vista previa de PDF y los
  textos de exportaciÃ³n ya siguen el idioma elegido (quedaban fijos en espaÃ±ol o inglÃ©s).
- La vista previa muestra los archivos **desde el inicio** y respeta las **reglas por
  extensiÃ³n** definidas por el usuario tambiÃ©n para extensiones que Naygo no conoce.
- Los rÃ³tulos largos de menÃºs y de los botones del diÃ¡logo de conflicto se recortan con Â«â€¦Â»
  en vez de desbordar, y los botones de los diÃ¡logos **crecen con el texto** (las
  traducciones largas ya no se cortan).
- El tooltip de ayuda de ConfiguraciÃ³n se ajusta a su texto (ocupaba toda la ventana).
- Las guardas que impiden borrar un origen al copiar o mover comparan las rutas sin
  distinguir mayÃºsculas/minÃºsculas (mÃ¡s seguro en Windows).

## [0.2.0] â€” 2026-06-27
### AÃ±adido
- DiÃ¡logo de conflicto de archivo estilo Directory Opus: cuando una copia o movimiento va a pisar un
  archivo que ya existe, se muestran lado a lado el archivo **Existente** y el **Nuevo** con su nombre,
  tamaÃ±o, fecha y tipo, para compararlos de un vistazo. Botones directos: Saltar, Mantener ambos
  (renombra el nuevo), Sobrescribir; y un desplegable Â«MÃ¡s opcionesÂ» con Renombrar, **Renombrar el
  existente**, **Saltar idÃ©nticos** (salta sin preguntar los archivos iguales en tamaÃ±o y fecha) y
  Sobrescribir/Saltar todos.
- El historial de operaciones muestra **quÃ© archivos** se movieron o copiaron: los nombres en la propia
  fila si son uno o dos, y un enlace Â«Ver N archivosÂ» que abre un popup con la lista completa (con la
  ruta relativa, el tamaÃ±o y el estado de cada archivo, mÃ¡s el origen, destino y estadÃ­sticas de la
  operaciÃ³n). El popup aprovecha el tamaÃ±o de la ventana.
- BotÃ³n Â«Cancelar todoÂ» en el panel de Operaciones para abortar de una vez todas las operaciones en
  curso, con confirmaciÃ³n previa.
- Atajos de teclado configurables para los botones de la barra de herramientas que no los tenÃ­an
  (abrir terminal, dividir panel, refrescar discos, mostrar ocultos, favoritos, configuraciÃ³n,
  disposiciones). El tooltip de cada botÃ³n muestra el atajo real asignado.
- Al arrastrar archivos entre paneles, Naygo puede pedir confirmaciÃ³n (Â«Â¿Copiar/Mover X a Â«carpetaÂ»?Â»,
  nombrando los archivos). La confirmaciÃ³n es opcional (ConfiguraciÃ³n â†’ General â†’ Â«Preguntar al
  arrastrar entre panelesÂ»). Si el archivo ya existe en el destino, se va directo al diÃ¡logo de
  conflicto sin preguntar dos veces.
- Editor de temas: nuevo color Â«fila de panel inactivoÂ» y casilla Â«paneles inactivos planosÂ» para
  atenuar o aplanar los paneles que no estÃ¡n activos, de modo que el panel activo resalte mÃ¡s.
- Al copiar o mover una carpeta que ya existe en el destino, Naygo pregunta una sola vez quÃ© hacer:
  **Fusionar** (copia dentro, preguntando solo por los archivos que choquen), **Reemplazar** (deja la
  carpeta destino con el contenido del origen), **Saltar** o **Cancelar**. Con varias carpetas en
  conflicto, una casilla aplica la misma decisiÃ³n a todas.
- El cÃ¡lculo previo de una copia grande (recorrer la carpeta para saber cuÃ¡nto pesa) ahora ocurre en
  segundo plano: la aplicaciÃ³n ya no se congela. Mientras tanto, el panel de Operaciones muestra
  Â«Calculandoâ€¦Â» con los archivos y el tamaÃ±o contados hasta el momento, y se puede cancelar desde ahÃ­.
- MenÃº de visibilidad en la barra de herramientas (botÃ³n con un ojo): muestra u oculta los
  archivos y carpetas marcados como ocultos, los de sistema, y los que empiezan con punto (estilo
  Linux). Cada interruptor es global, se aplica al instante a todos los paneles y al Ã¡rbol, y se
  recuerda. Por defecto Naygo los muestra todos.
- Favoritos organizables en grupos (carpetas) anidados. Un botÃ³n con una estrella en la barra de
  herramientas despliega el Ã¡rbol de favoritos para saltar rÃ¡pido a cualquiera. Desde el panel de
  Favoritos se gestionan con clic derecho: crear un grupo, renombrarlo, eliminarlo, y mover un
  favorito o grupo a otro grupo. La estrella â˜… de la barra de ruta sigue agregando a la raÃ­z.
- Editor de temas: crea tu propio tema duplicando uno existente y ajustando cada uno de sus colores
  con un selector tipo paleta (presets, colores estÃ¡ndar y Â«MÃ¡s coloresâ€¦Â» con valores R/G/B y hex),
  viendo el cambio aplicado a toda la aplicaciÃ³n en vivo. Guardar lo conserva como tema tuyo y
  Cancelar revierte. Los temas de fÃ¡brica quedan intactos; los temas propios se pueden editar o
  eliminar.
- Panel de Operaciones: muestra la copia o el movimiento en curso con todos los datos â€”archivo
  actual, barra de progreso, copiado X de Y, velocidad media y pico, tiempo transcurrido y
  restanteâ€” mÃ¡s los botones Pausar/Reanudar y Cancelar, la cola de operaciones pendientes y un
  historial reciente. Aparece solo al iniciar una operaciÃ³n. Se puede agregar tambiÃ©n desde el
  menÃº Â«Panel â–¾Â».
- Pausar y reanudar una copia o movimiento en curso (sin perder lo ya copiado).
- Arrastrar archivos de un panel a otro: dentro del mismo disco mueve, a otro disco copia;
  Ctrl fuerza copiar y Shift fuerza mover. Los archivos arrastrados desde el Explorador de
  Windows tambiÃ©n caen en el panel sobre el que se sueltan.
- Paleta de comandos (Ctrl+P): un buscador rÃ¡pido que filtra acciones, archivos de la carpeta
  actual, carpetas recientes, favoritos y temas con coincidencia aproximada (fuzzy), resaltando
  las letras que coinciden. Se navega con las flechas, Enter ejecuta y Esc cierra. El atajo es
  configurable en ConfiguraciÃ³n â†’ Atajos.
- MenÃº de historial en los botones AtrÃ¡s y Adelante: un triÃ¡ngulo â–¾ junto a cada uno despliega
  las carpetas visitadas en esa direcciÃ³n para saltar directo a una.
- Pie de panel (footer): cada panel de archivos muestra al pie sus propios datos â€”archivos
  seleccionados sobre el total, bytes marcados, y espacio libre/total del disco de su unidadâ€”.
  La plantilla es global y se elige en ConfiguraciÃ³n â†’ Avanzado entre varias predefinidas
  (Compacta, Completa, Solo disco, Solo selecciÃ³n) o una personalizada con tokens
  (`{sel} {total} {marked} {free} {disk_total} {pct} {items} {files} {dirs}`), con vista previa
  en vivo. Se puede ocultar.
- Botones AtrÃ¡s, Adelante e Inicio en la barra de herramientas, al estilo de un navegador.
  AtrÃ¡s/Adelante se atenÃºan cuando no hay a dÃ³nde ir. El botÃ³n Inicio (atajo Alt+Inicio) navega
  a la carpeta de inicio, configurable en Avanzado (vacÃ­o = carpeta personal del usuario).
- Resaltado automÃ¡tico de cÃ³digo en la Vista previa: las extensiones de cÃ³digo conocidas se
  resaltan solas en modo AutomÃ¡tico. Se puede desactivar con un interruptor en
  ConfiguraciÃ³n â†’ PrevisualizaciÃ³n; las reglas por extensiÃ³n siguen mandando sobre el ajuste global.
- La Vista previa de texto permite seleccionar y copiar el contenido: el texto plano siempre es
  seleccionable, y el cÃ³digo resaltado tiene un botÃ³n que alterna a una vista seleccionable
  (selecciÃ³n con el mouse y Ctrl+C).
- La Vista previa resalta el cÃ³digo por colores (XML, JSON, HTML, CSS, JavaScript, C/C++, Java,
  Python, Rust, SQL, Bash, Markdown, YAML, TOML, INI). En ConfiguraciÃ³n â†’ PrevisualizaciÃ³n se
  puede forzar el modo de vista por extensiÃ³n: AutomÃ¡tico, ver como texto, ver como imagen, o
  ver como cÃ³digo eligiendo el lenguaje.
- BotÃ³n en la Vista previa para abrir el archivo con el programa predeterminado del sistema.
- Avisos de software de terceros: archivo `THIRD-PARTY-NOTICES.md` con las licencias de todas
  las dependencias (todas permisivas; la interfaz usa Slint bajo su licencia *royalty-free*).
  La secciÃ³n "Acerca de" lo menciona y el archivo se incluye en el portable y el instalador.
- Vista profunda: un modo del panel que lista, de forma plana y con sangrÃ­a por
  profundidad, todo el contenido de la carpeta actual y sus subcarpetas (recursivo).
  Se activa con el botÃ³n de la barra del panel; aparece por streaming y se puede
  cancelar. El doble clic en una carpeta sale del modo y navega a ella.
- Ãcono de historial de carpetas en la barra de herramientas: despliega un menÃº con
  las carpetas visitadas recientemente y, al elegir una, navega el panel activo.
- La cantidad de carpetas recientes recordadas ahora es configurable (1â€“100, por
  defecto 50) en la secciÃ³n Avanzado de la configuraciÃ³n.
- Tooltips explicativos en todos los botones de la barra de herramientas y de los paneles.
- GuÃ­a para agregar idiomas (`docs/AGREGAR-IDIOMA.md`): basta soltar un `.json` en `lang/`.
- Registro (log) con mÃ¡s contexto para diagnosticar caÃ­das: marca de tiempo en hora local
  legible, "migas de pan" de las acciones recientes, y al ocurrir un error se vuelca el
  estado (carpetas abiertas, tema, idioma, entorno). Todo local, sin telemetrÃ­a.
- Seis temas de color nuevos con mÃ¡s personalidad: Windows XP, macOS, Verde sobre azul,
  Solarized Dark, Terminal Ã¡mbar y Ciruela.
- En la primera ejecuciÃ³n, Naygo arranca con la disposiciÃ³n clÃ¡sica (Ã¡rbol + dos paneles de
  archivos + Propiedades + Vista previa) en vez de un solo panel.
- Argumentos de lÃ­nea de comandos: `naygo.exe <carpeta>` abre esa carpeta al iniciar, y
  `--theme`/`--layout` aplican un tema o una disposiciÃ³n solo para esa sesiÃ³n. `--help` y
  `--version` muestran la informaciÃ³n correspondiente. Ãštil para accesos directos y para
  "Abrir en Naygo".
### Cambiado
- Barra de herramientas reordenada por funciÃ³n y con Ã­conos redibujados a mano (dividir panel, apilar,
  clonar, intercambiar, panel auxiliar, refrescar discos), mÃ¡s representativos y consistentes.
- Los diÃ¡logos de operaciÃ³n (eliminar, conflicto, pegar) tienen un borde sutil, el botÃ³n con el foco se
  resalta con un anillo del color de acento (ya no en rojo), TAB mueve el resaltado entre botones, cada
  botÃ³n muestra su atajo de teclado, y los botones se reparten a lo ancho del diÃ¡logo por igual.
- Los temas de fÃ¡brica se redujeron a cinco (Dark Blue, Windows XP, Verde sobre azul, High Contrast
  y NeÃ³n Retro). El resto se puede recrear a gusto con el nuevo editor de temas.
- Los avisos y confirmaciones internos (confirmar expulsar una unidad USB, errores al
  importar/exportar packs) ahora usan un diÃ¡logo con el tema de Naygo en vez del cuadro
  nativo del sistema. El mensaje de cierre por error inesperado es mÃ¡s claro y el detalle
  tÃ©cnico queda en el registro.
- El registro de diagnÃ³stico ahora se guarda por dÃ­a: `naygo-AAAA-MM-DD.log` (antes un Ãºnico
  `naygo.log`), para no mezclar las corridas y diagnosticar mÃ¡s fÃ¡cil.
- El aviso de "carpeta no encontrada" ahora refresca el panel al instante al pulsar cualquier
  opciÃ³n (antes habÃ­a que redimensionar); "Subir un nivel" solo aparece si hay una carpeta
  superior a la que ir; y al volver atrÃ¡s a una unidad ausente se ven de nuevo las opciones.
- Los paneles de Propiedades y Vista previa se marcan como panel activo al hacer clic en
  cualquier parte de su cuerpo, no solo en la barra de tÃ­tulo.
- El menÃº de plantillas de disposiciÃ³n aparece junto a su botÃ³n en la barra (antes salÃ­a lejos).
### Corregido
- Refrescar un panel con F5 ya no **duplica** las filas: antes, re-listar la misma carpeta acumulaba
  los archivos sobre los que ya estaban; ahora reemplaza la lista de forma limpia.
- El popup de confirmaciÃ³n al arrastrar entre paneles ya responde **al primer clic** (antes requerÃ­a
  dos). El modal no tomaba el foco al aparecer, asÃ­ que el primer clic se gastaba en enfocarlo.
- Los tooltips de los Ã­conos de la barra de ruta del panel (favorito â˜…, copiar ruta ðŸ“‹, vista profunda)
  ahora aparecen **junto al Ã­cono**, no arriba en la barra de herramientas.
- Los menÃºs desplegables de la barra de herramientas aparecen alineados **bajo el botÃ³n** que los abre
  (algunos salÃ­an desplazados, sobre todo los abiertos por atajo de teclado).
- El botÃ³n Â«DeshacerÂ» del historial de acciones distingue ahora cuando una acciÃ³n **ya fue deshecha**
  (muestra Â«DeshechoÂ») en vez de quedar igual; y se diferencia de las acciones que no son deshacibles.
- Â«Saltar idÃ©nticosÂ» en un conflicto nunca sobrescribe sin confirmar: si los archivos difieren, vuelve
  a preguntar en vez de pisar el del destino.
- El nombre del archivo en el diÃ¡logo de conflicto es legible tambiÃ©n en temas claros.
- El diÃ¡logo Â«Ya existeâ€¦Â» que aparece al copiar sobre un archivo existente ahora permite **cancelar
  toda la operaciÃ³n** (botÃ³n Â«Cancelar todoÂ», ademÃ¡s de Escape). Antes el aviso atrapaba: solo dejaba
  Saltar/Renombrar/Sobrescribir y su fondo tapaba el botÃ³n Cancelar del panel. Cerrar el aviso por
  fuera (Escape o clic afuera) salta solo ese archivo y continÃºa.
- Arrastrar las filas de archivos entre paneles vuelve a funcionar (la lista capturaba mal el gesto
  del mouse). TambiÃ©n se recuperÃ³ la selecciÃ³n por rectÃ¡ngulo arrastrando sobre la lista.
- Copiar o mover archivos grandes ya muestra el avance real (antes la copia en curso parecÃ­a
  detenerse): el progreso se actualiza por bytes con velocidad y tiempo restante. Resuelve el caso
  en que pegar y sobrescribir un archivo grande parecÃ­a copiar solo unos megas.
- El doble clic en una carpeta vuelve a navegar dentro del panel aunque antes se haya usado un
  atajo con Ctrl (los modificadores ya no quedan "pegados").
- Temas claros (Light, macOS, Windows XP, Citrus Glow): mejor legibilidad. El texto atenuado de
  las columnas (extensiÃ³n, tamaÃ±o, fecha) tiene mÃ¡s contraste, y los fondos blanco-puro pasaron a
  grises/cremas suaves para que no "laven" la vista (se veÃ­an demasiado brillantes).
- Aviso "carpeta no encontrada": "Subir un nivel" / "Elegir otra" / "Reintentar" ahora refrescan
  el panel al instante. Antes el panel sÃ­ navegaba pero el aviso seguÃ­a tapando el contenido.
- Vista previa de cÃ³digo: las lÃ­neas largas ya no se cortan; el cÃ³digo se desplaza en horizontal
  como en un editor (conservando los colores y la indentaciÃ³n).
- Al confirmar el borrado de un archivo, pulsar Enter ahora activa "Eliminar" en vez de abrir
  el archivo de fondo. (El teclado del panel se suspende mientras hay un diÃ¡logo abierto.)
- Hacer clic en el panel de Vista previa (o Propiedades) ya no pierde la selecciÃ³n del archivo:
  la previsualizaciÃ³n sigue al Ãºltimo panel de archivos activo.
- Tras pegar un texto o imagen y crear el archivo, ya se puede seleccionar otro de inmediato
  (antes habÃ­a que clicar primero el archivo reciÃ©n creado).
- CaÃ­da (o cuelgue con la ventana en blanco) al previsualizar archivos de texto con lÃ­neas muy
  largas, frecuente en logs: el renderizador por software no podÃ­a posicionar glifos tan a la
  derecha y la app se cerraba con un error. Ahora la Vista previa ajusta las lÃ­neas a la columna
  y recorta las larguÃ­simas (igual para el texto extraÃ­do de PDF).
- Mayor robustez en equipos y mÃ¡quinas virtuales sin GPU: Naygo fuerza el renderizador por
  software desde el arranque, sin depender de una GPU acelerada.

## [0.1.0] â€” 2026-06-18
### AÃ±adido
- NavegaciÃ³n de archivos tipo Commander: paneles dinÃ¡micos acoplables, dual-pane,
  ir atrÃ¡s/adelante (incluidos los botones laterales del mouse).
- Ãrbol de carpetas con expansiÃ³n incremental, revelado hasta la carpeta activa y
  navegaciÃ³n por teclado.
- Listado por streaming incremental y cancelable; el filesystem hostil (red caÃ­da,
  permisos, rutas que desaparecen) no tumba la app.
- Columnas dinÃ¡micas estilo planilla: ordenar, filtrar por tipo de columna y
  reordenar arrastrando.
- Operaciones de archivo entre paneles (copiar, mover, eliminar) con cola opcional,
  progreso y cancelaciÃ³n.
- Renombrado en lÃ­nea y en cadena, y ventana de renombrado por lotes.
- BÃºsqueda recursiva por nombre en la carpeta y sus subcarpetas.
- PrevisualizaciÃ³n liviana: imÃ¡genes, SVG (rasterizado), PDF (texto y metadatos),
  texto/cÃ³digo y listado de contenido de archivos ZIP.
- CÃ¡lculo de tamaÃ±o de carpetas bajo demanda.
- Barra de unidades de disco con espacio libre/total y porcentaje usado; Ã­cono
  propio para unidades USB y expulsiÃ³n segura desde un menÃº.
- DetecciÃ³n de discos duros externos USB como extraÃ­bles (por tipo de bus).
- IntegraciÃ³n con Windows: menÃº contextual del shell, "Abrir con", watcher de
  carpeta, detecciÃ³n de dispositivos, arrastrar y soltar, Ã­cono de bandeja y
  arranque opcional con el sistema.
- InternacionalizaciÃ³n (espaÃ±ol e inglÃ©s) y temas intercambiables en caliente, con
  galerÃ­a de selecciÃ³n y packs de usuario.
- ConfiguraciÃ³n como ventana nativa: apariencia, atajos, previsualizaciÃ³n, plantilla
  de tabla, opciones avanzadas y secciÃ³n "Acerca de".
- DistribuciÃ³n como ejecutable portable e instalador (Inno Setup) con CRT estÃ¡tico.
