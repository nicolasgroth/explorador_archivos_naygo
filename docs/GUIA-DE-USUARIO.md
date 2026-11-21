# Guía de usuario — Naygo

Naygo es un explorador de archivos rápido y liviano para Windows 10/11, estilo
*Commander* (inspirado en Directory Opus / Total Commander / XYplorer). Esta guía
explica cada sección de la app y cómo usarla, sobre todo **por teclado**, que es
la forma más rápida de trabajar.

> **Atajo de ayuda:** dentro de la app, **F1** abre una ayuda rápida con esta
> orientación de uso y la lista de atajos activos; no es una copia de este documento.

Esta guía describe la base 0.5.4 en preparación. Las mejoras futuras se mantienen separadas en el
[plan de rutas, preview y configuración](plans/2026-09-09-rutas-preview-y-configuracion.md).

Al iniciar, Naygo muestra primero sus recursos integrados y aplica después la configuración,
idioma, atajos y workspace guardados en un worker. Esto evita que la lectura inicial de esos datos
detenga la ventana; las opciones que cambies mientras termina la carga se conservan.

Los controles de navegación, carpetas recientes, árbol, acciones del preview y resultados de la
paleta Ctrl+P declaran nombres y acciones para UI Automation. Los menús de contexto y filtros también
exponen sus opciones a lectores de pantalla. La validación completa con Narrador y los diálogos sigue
pendiente de una sesión interactiva.

---

Al usar **Atrás** o **Adelante**, Naygo recupera durante la sesión la selección, el archivo
enfocado, los filtros y la posición que tenía el listado en esa visita. Los archivos que ya no
existen se omiten. Este contexto no se guarda al cerrar Naygo.

Al **subir a la carpeta padre**, mediante teclado o botón, queda seleccionada y visible la
carpeta de donde venías. El árbol señala la carpeta actualmente abierta. **Crear carpetas**
actualiza la rama y selecciona el resultado después de completar la operación; si creas una
ruta anidada, se selecciona su primer nivel en el panel. La siguiente apertura del cuadro
empieza vacía. Se respetan los filtros de visibilidad configurados.
Ctrl/Shift en un doble clic se toma del gesto del mouse; si se pierde el evento de liberación de
una tecla al cerrar un modal, el siguiente doble clic sin modificadores abre la carpeta en el
panel actual.

El resaltado de búsqueda colorea la coincidencia dentro del nombre sin agregar espacios.
El resaltado de los cambios de una visita sigue la ruta del archivo; un archivo con el mismo
nombre en otra carpeta no hereda esa marca.
**Refrescar** conserva la selección por ruta, aunque se inserten o desaparezcan filas.
Los cambios durante un listado se reconcilian al terminar y las operaciones de Naygo
solicitan actualización aunque el sistema no haya notificado el cambio. En ubicaciones
que no admiten vigilancia, usa el refresco del panel o el global; no se sondea todo el disco.

Al pegar texto o una imagen como archivo, `{fecha}` genera `YYYY-MM-DD HH.MM.SS`, con
segundos y hora UTC como en versiones anteriores. Las plantillas personalizadas se conservan.
Si dos pegados coinciden en el mismo segundo/nombre, se añade un sufijo como `(2)` sin
sobrescribir el primero, incluso cuando ambas escrituras están en curso.

Si una carpeta no se puede leer por completo, Naygo muestra los elementos disponibles y un
aviso discreto. Ese resultado parcial no se usa para marcar archivos como eliminados en la
comparación de visitas. Pulsa **F5** para reintentar; si falla el acceso, las filas anteriores de
esa misma carpeta se conservan cuando no llegó ningún resultado nuevo.

La comparación entre visitas se guarda solo durante la sesión y tiene un presupuesto de memoria
limitado. Si la carpeta es demasiado grande o su snapshot fue expulsado por antigüedad, la próxima
visita comienza una comparación nueva; Naygo no marca por ello archivos como eliminados.

Al copiar o mover a otro panel, aparece el **radar de destinos**. Pulsa **1–9** para elegir
rápidamente entre paneles, favoritos, destinos recientes y carpetas frecuentes; **Esc** cancela.
Al mover una carpeta, Naygo procesa primero sus elementos y retira del origen sólo las carpetas
que quedan vacías; si un archivo se omite o falla, su carpeta se conserva junto con el archivo.
En Windows, si otro proceso ocupa el destino durante un movimiento, Naygo conserva ambos archivos
y reporta el conflicto; entre volúmenes usa copia cancelable y retira el origen sólo después de
publicar correctamente el destino.

Al elegir **Reemplazar** una carpeta, Naygo prepara primero el árbol completo en un temporal junto
al destino. El árbol anterior permanece hasta terminar la copia; cancelar o fallar durante esa
preparación conserva el destino y el origen. Para un Move se necesita espacio temporal para otra
copia del árbol, y el origen se retira sólo después de publicar el nuevo árbol y verificar que sus
archivos no hayan cambiado. En Windows la publicación requiere apartar el destino anterior y luego
renombrar el temporal, así que puede haber un instante breve en que la ruta no exista. Si otro
proceso ocupa esa ruta en ese intervalo, Naygo no lo reemplaza y conserva el árbol anterior en un
respaldo `.naygo-backup-*`; un error de limpieza también informa la ubicación del respaldo.
Si Naygo termina entre los renames, al volver a listar esa carpeta o su padre recupera el intento:
restaura el árbol anterior si el nuevo aún estaba en staging, o completa la limpieza si el nuevo ya
se publicó. Si otra aplicación ocupó la ruta, conserva el respaldo y presenta el error. Esta
recuperación cubre la terminación del proceso; no se ha validado pérdida de energía ni fallo del disco.

## 1. La ventana

La ventana se divide en:

- **Barra de herramientas** (arriba), organizada de izquierda a derecha por grupos:
  navegación y ruta actual; búsqueda, visibilidad, recientes y favoritos; controles de
  paneles y disposiciones; acciones; unidades de disco; y Configuración al extremo derecho.
  Los separadores finos marcan cada grupo. En modo de solo iconos, los tooltips conservan
  el nombre y la función de cada botón.
- **Paneles**: el área principal. Empieza con un panel de archivos; puedes dividir
  en varios (dual-pane y más).
- **Barra de estado** (abajo): ruta del panel activo y conteo de elementos.

El **panel activo** lleva un borde de color de acento. Casi todo (teclado,
operaciones) actúa sobre el panel activo.

**Tamaño y posición:** Naygo recuerda el tamaño, la posición y el estado
maximizado de la ventana entre sesiones, así la próxima vez arranca tal como
la dejaste. Si el monitor donde estaba la ventana ya no está conectado (por
ejemplo, cambiaste de notebook a monitor externo), Naygo la abre en una
posición visible en vez de dejarla fuera de pantalla.

**Cerrar a la bandeja:** con la opción activada, el botón **X** oculta Naygo si
hay una vía activa para volver a mostrarlo: el ícono de bandeja o, en Windows,
el atajo global registrado. Para salir desde la bandeja, haz **clic derecho**
en el ícono y elige **Salir**. Si no existe una vía de retorno (por ejemplo,
porque el tray no pudo crearse y no hay atajo global), la X guarda la sesión y
cierra Naygo para no dejar un proceso oculto inaccesible. Puedes cambiar esta
opción en *Configuración → Avanzado → "Cerrar a la bandeja (no salir)"*.

**Atajo global (Ctrl+Alt+Z):** funciona **desde cualquier aplicación**, no
solo con Naygo en primer plano: siempre muestra la ventana y la trae al
frente (para esconderla, cierra con la X con «cerrar a la bandeja» activo).
La combinación se puede cambiar o desactivar en
*Configuración → Avanzado*. La tecla Windows no se puede usar (la reserva el
sistema); si al capturar una combinación nueva Windows la rechaza —por
ejemplo porque otra aplicación ya la tiene tomada—, Naygo te avisa y conserva
la combinación anterior.

---

## 2. Paneles

Cada panel puede ser de un **tipo**:

| Tipo | Para qué sirve |
|------|----------------|
| **Archivos** | Lista navegable de una carpeta. Es el panel principal. |
| **Árbol** | Árbol de carpetas; puede seguir al último panel de archivos activo o quedar enlazado a uno específico. |
| **Propiedades** | Datos del ítem enfocado (nombre, tipo, tamaño, fechas). |
| **Historial de acciones** | Operaciones hechas, con botón de deshacer. |
| **Favoritos** | Carpetas ancladas + recientes. |
| **Vista previa** | Vista liviana del archivo enfocado: texto, imagen, …o el contenido de un comprimido (`.zip`, `.tar`, `.tar.gz`) como árbol con totales. |
| **Operaciones** | Copias y movimientos en curso, en cola y recién terminados (ver §5). Aparece solo al iniciar una operación. |

En cada espacio hay una sola Bandeja temporal, Favoritos, Vista previa y Operaciones: el menú
deshabilita agregar otro mientras exista. Si una disposición anterior ya contiene duplicados,
puedes cerrarlos sin que Naygo elimine automáticamente ninguno.

**Dividir y reorganizar:**

- El botón **＋** divide el panel activo (elige dirección: derecha/abajo/izq/arriba).
  Dividir varias veces en la misma dirección crea una fila pareja de paneles (no cajas
  anidadas), así los divisores se comportan de forma predecible.
- El botón **Panel ▾** agrega un panel especial (árbol, propiedades, etc.) o un
  **Explorador enlazado**, que crea Árbol + Archivos como una pareja 28/72. En la cabecera
  del árbol, `○ Árbol común` / `● ↔ carpeta` alterna entre seguir al último panel usado y
  quedar dedicado a ese panel.
- El árbol comienza con accesos a las carpetas conocidas de Windows y a las cuentas de
  **OneDrive/Dropbox** detectadas. Cada acceso navega su carpeta física real —también si fue
  reubicada a otra unidad— y al marcar un panel de archivos el árbol revela esa ubicación.
  Si las filas cambian mientras la ruta activa sigue siendo la misma, el árbol evita volver a
  centrarla automáticamente; al navegar a otra ruta intenta mostrarla.
  Si un proveedor ya no muestra un tramo de la ruta al terminar de listar, el árbol deja de
  esperarlo y el panel conserva la carpeta solicitada y su aviso, desde donde puedes reintentar.
  Cuando vuelva a estar disponible, el panel y el árbol recuperan la carpeta al pulsar **Reintentar**.
- El menú contextual de la zona vacía utiliza la ruta del panel incluso si la carpeta deja de
  estar disponible. Las acciones pueden informar un error si ya no es posible operar en ella.
- **Arrastra la barra de título** de un panel sobre otro para reacomodar (los bordes
  dividen; el centro apila como pestaña).
- **Arrastra las barras divisorias** para redimensionar: mientras arrastras verás una
  barra-fantasma de acento que marca dónde quedará el borde; al soltar se aplica. Mover
  una barra solo redimensiona sus **dos paneles vecinos**: el resto de los paneles no se
  mueve. Al cambiar el tamaño de la ventana, todos escalan proporcionalmente.
- **Doble clic en una barra divisoria** reparte el espacio **50/50** entre sus dos
  paneles vecinos.
- **Swap / Clonar / Tabs** (toolbar): intercambiar carpetas de dos paneles, abrir la
  carpeta actual en otro panel, o apilar el panel como pestaña sobre otro.
- Cuando la barra de pestañas recibe el foco, **←/→** recorre las pestañas sin cambiar
  el panel activo; **Enter** o **Espacio** activa la enfocada.

**Pie de panel (footer):** cada panel de archivos muestra al pie una barra con
información de **ese** panel: archivos seleccionados sobre el total, bytes marcados, y
espacio libre/total del disco de su unidad. Se muestra u oculta y se da formato desde
**Configuración → Avanzado**, sección *Pie de panel* (ver §9). El formato es **global**
(igual en todos los paneles), pero cada panel muestra **sus propios** datos.

**Vista previa de código y texto:**

- Los archivos `.sif` se tratan como texto en modo **Automático**, igual que `.sh`; la actualización
  agrega la regla a configuraciones existentes sin cambiar los modos o habilitaciones que ya
  guardaste para otras extensiones.

- **Seleccionar y copiar:** TXT, XML y los demás contenidos textuales usan un editor
  de sólo lectura seleccionable. Marca un fragmento y pulsa **Ctrl+C**; el botón de
  cabecera copia todo el contenido cargado, no necesariamente el archivo completo.
  No hay que alternar de modo para copiar XML/código. La presentación seleccionable
  actual es de un solo color, aunque el motor conserve reglas y preparación de resaltado.
  Los lectores de pantalla reciben el campo como «Contenido de la vista previa de texto»;
  la etiqueta identifica el control sin usar el nombre ni el contenido del archivo.
- **Scroll y límites:** rueda/barra recorren el fragmento cargado; al cambiar de archivo
  se reinician selección y viewport. **Configuración → Previsualización → Lectura máxima de texto**
  permite elegir 64 KiB, 256 KiB, 512 KiB o 1 MiB por archivo (256 KiB por defecto). El preview
  informa los bytes leídos y los caracteres/líneas mostrados. Se mantienen topes de 250 líneas y
  1.000 caracteres por línea para proteger el renderizador; un borde rasgado indica que se alcanzó
  algún límite. XML/SIF minificados en una sola línea pueden recortarse por el tope de línea aunque
  se aumente el límite de bytes. Si el corte cae dentro del último carácter UTF-8/UTF-16, se omite
  ese carácter parcial sin cambiar la codificación del resto. Esta opción solo afecta la lectura del
  preview y nunca modifica el archivo original.
- **Imágenes:** un conmutador alterna ajuste al panel y `1:1`, conservando el modo al
  cambiar de imagen. El modo natural usa la imagen preparada por el preview y permite
  scroll si excede el panel; imágenes grandes pueden haberse reducido por sus límites.
  WebP, GIF y APNG (`.png` o `.apng`) animados se reproducen automáticamente si su regla
  de imagen está habilitada. **Pausar / Reanudar** conserva el punto actual; **Repetir**
  aparece al terminar un ciclo finito. Las repeticiones infinitas siguen hasta pausar,
  cancelar o cambiar/ocultar el preview; no existe un corte arbitrario de 60 segundos.
  Se prepara una salida de hasta 768 px y 15 presentaciones/s, sin ampliar imágenes pequeñas.
  **Configuración → Previsualización → Modo económico del preview** reduce las animaciones
  a 512 px / 10 presentaciones/s, imágenes estáticas/SVG a 1024 px y el render 3D a
  512 × 384. Está desactivado por defecto; se guarda y actualiza el preview actual.
  No modifica originales, reglas por extensión ni el modo ajuste/`1:1`.
  Existe un presupuesto compartido para contenido retenido; un modelo demasiado grande
  puede omitirse con un aviso y reintento. Puedes abrirlo con su aplicación externa.
  `1:1` se refiere a esa imagen preparada, no al canvas original si tuvo que reducirse.
  Los delays inferiores a 10 ms se normalizan para evitar bucles de consumo excesivo.
  Entrada hasta 20 MiB y canvas animado hasta cuatro millones de píxeles; un archivo
  excesivo o dañado muestra un error/límite con **Reintentar** o apertura externa.
  Pausar no decodifica en segundo plano salvo el único pedido que ya estuviera en curso.
  En una configuración antigua puede faltar la regla `.apng`: agrégala en Previsualización
  como imagen. No se reemplazan tus reglas guardadas; APNG con extensión `.png` usa la regla PNG.
- **Cancelar:** disponible desde el inicio, también mientras espera capacidad o carga
  geometría detrás de una miniatura 3D. El preview no vuelve a arrancar solo tras cancelar
  o fallar: usa **Reintentar** o selecciona otro archivo.
- **Preview oculto:** cambiar su pestaña, maximizar otro panel o minimizar/ocultar Naygo
  cancela el trabajo y libera la vista/escena. Al volver sólo carga la selección vigente.
  Hay hasta dos workers, contando los cancelados que aún no terminan; mientras ambos
  siguen ocupados, la nueva selección espera y también se puede cancelar.
  En imágenes/SVG/PDF/comprimidos y también texto/código/STL/3MF, Cancelar termina el lector
  auxiliar incluso si una lectura queda bloqueada.
  Cada lector tiene un tope de 192 MiB de memoria comprometida y cinco segundos de ejecución;
  al exceder un límite aparece un aviso, con Reintentar o abrir externamente como alternativas.
  El resaltado, copia parcial y truncado de texto se conservan. Una vez cargada la geometría
  3D, girarla no relee el archivo: su rasterización en memoria sigue siendo cancelable y
  tiene un límite propio. Los modelos que superan los recursos disponibles pueden abrirse
  externamente. Estos límites no representan el consumo total de Naygo.
  El preview PDF muestra texto, nunca renderiza páginas; su extracción se detiene a los
  8.000 caracteres o 500 páginas de contenido y marca el resultado como truncado.
  Los previews ZIP/TAR/TAR.GZ muestran hasta 500 entradas y no extraen archivos. El índice ZIP
  está limitado a 10.000 registros, 6 MiB de directorio central y 8 MiB de lectura; TAR recorre
  hasta 32 MiB, y TAR.GZ hasta 16 MiB comprimidos / 32 MiB expandidos. Se omiten rutas internas
  mayores a 1 KiB o 64 niveles. Si el límite corta el recorrido, se marca parcial o aparece un
  aviso para reintentar/abrir externamente.
  SVG admite imágenes incrustadas, pero no carga imágenes de archivos externos referenciados.
- **Modelos de impresión 3D:** los archivos **STL** (binario o ASCII) y **3MF** muestran
  una vista estática sin requerir GPU. En 3MF se intenta primero rasterizar la geometría por CPU;
  si no se puede interpretar, se utiliza la miniatura incrustada cuando existe. Los modelos con
  referencias a componentes externos requieren esa miniatura hasta que se admita dicho formato.
  El panel de propiedades informa dimensiones X/Y/Z, triángulos y, para 3MF, la unidad declarada.
  Cuando la geometría está disponible, arrastrar hace girar el modelo siguiendo los ejes de la
  cámara, incluso si ya está inclinado; la rueda cambia el zoom y restaurar reinicia la cámara. En
  un 3MF que utiliza la miniatura de respaldo, el primer arrastre intenta cargar la geometría
  interactiva. Los botones del encabezado alternan el tono
  uniforme (azul, blanco, cálido), el color del archivo y la presentación (sólido, bordes o malla);
  el color del archivo admite materiales básicos 3MF por objeto y triángulo, usando azul cuando
  no se encuentra un color compatible. La transparencia se aproxima sobre el fondo del preview y
  no muestra capas transparentes. Texturas, extensiones de materiales y colores propietarios aún
  no se interpretan; la miniatura puede representar información visual adicional. Los botones cargan la
  geometría si se estaba mostrando una miniatura. Al seleccionar otro modelo vuelven al valor
  inicial. La ayuda aparece al pasar el mouse por el
  modelo. No es un editor ni un laminador.

---

## 3. Navegar

- **Doble clic** en una carpeta entra; **Enter** entra a la enfocada.
- **Backspace** o **←** suben un nivel.
- **Alt+←** / **Alt+→**: atrás / adelante en el historial (como un navegador).
- **F5**: refresca el panel activo. El botón local refresca su panel sin cambiar el foco;
  el botón global refresca todos los paneles, ramas abiertas y unidades.
- Tras renombrar uno o varios elementos, cada uno conserva su posición y sus marcas en la
  lista. El teclado sigue en la fila enfocada. **F5** u ordenar por una columna integra los
  nombres nuevos en el orden de la vista.
- Los elementos nuevos se agregan al final en el orden en que Naygo los detecta. Siguen ahí
  cuando desaparece el resaltado, hasta que actualices u ordenes la lista. Puedes desactivar
  **Archivos nuevos al final del listado** en **Configuración → Avanzado**.
- **Tab**: cambia el panel de archivos activo.
- **Tipeo rápido (typeahead):** escribe las primeras letras de un nombre y el foco
  salta a ese ítem. Si haces una pausa (~½ segundo), empieza una búsqueda nueva.
- **Esc**: cancela un listado en curso (útil en discos de red lentos).
- **Abrir un archivo:** Enter o doble clic usa la aplicación predeterminada del escritorio. En Linux,
  Naygo delega la apertura en la integración XDG; no interpreta ni edita el contenido dentro de la app.

En compilaciones Unix, los nombres que contienen bytes no válidos en UTF-8 se muestran con escapes
`\\xNN`; el backslash literal se duplica. La búsqueda por nombre usa esa misma forma visible. Es sólo
una representación: las acciones sobre una fila conservan la ruta nativa original. El texto escapado
no es una sintaxis para pegar en la barra de ruta.

**Botones Atrás / Adelante / Inicio (estilo navegador):** la barra de herramientas
tiene tres botones de navegación que actúan sobre el panel activo:

- **Atrás** (**Alt+←**) y **Adelante** (**Alt+→**) recorren el historial de carpetas
  del panel. Se **atenúan** (se deshabilitan) cuando no hay a dónde ir hacia atrás o
  hacia adelante.
- **Inicio** (ícono de casa, **Alt+Inicio**) lleva a la carpeta de inicio. Esa carpeta
  se define en **Configuración → Avanzado → "Carpeta de inicio (Home)"**; si la dejas
  vacía, se usa tu carpeta personal (la de tu perfil de usuario).

**Barra de ruta (breadcrumbs):** muestra segmentos clicables con ancho natural. Edita
desde el **lápiz**, **… → Editar ruta**, **Ctrl+L** o **F4** (atajos predeterminados).
Puedes editar en medio, seleccionar, pegar y deshacer sin perder la posición del cursor.
**Enter** comprueba la carpeta sin bloquear la interfaz: si falla, conserva el texto y
muestra el motivo bajo el campo. **Esc** (tecla o botón) descarta el borrador; la **×** interna
del campo sólo vacía el texto. Las rutas relativas
parten desde la carpeta de ese panel; se admiten rutas pegadas entre comillas.
El autocompletado aparece al pausar la escritura; ↑/↓ y Enter permiten elegir sugerencias.
Si una carpeta de red no responde, la espera visible vence a los cinco segundos. Una
consulta bloqueada puede impedir nuevas sugerencias hasta que Windows la libere.
A la derecha están **★** (favoritos) y **📋** (copiar la ruta confirmada completa).
**Ruta superior:** haz clic en el texto o su lápiz para editar. Mientras escribes, el
editor ocupa el ancho de la barra superior; debajo aparecen sugerencias o el error de
validación. Enter confirma, las flechas recorren sugerencias y Esc descarta. Copiar y
favorito siguen disponibles a la derecha y usan la carpeta confirmada, no el borrador.
El destino queda fijado al panel donde empezaste: seleccionar otro no cambia el destino.
Si cierras ese panel o navegas su carpeta por otra vía, se cancela la edición pendiente.
Los atajos Ctrl+L/F4 mantienen la edición dentro del panel, sin añadir atajos globales.

La actualización automática depende de notificaciones de Windows/proveedor. Si una ruta
de red no se actualiza o se reconecta, usa refresco manual; no hay sondeo global.

**Unidades de disco:** la tira C:/D:/… de la toolbar navega el panel activo a esa unidad.
En una ventana angosta, abre **Más** para ver las unidades, refrescarlas o expulsar con seguridad
las extraíbles.

**Barra adaptable:** al reducir el ancho, la navegación, la ruta, búsqueda, favoritos, **Espacios**
y **Disposiciones** permanecen visibles en una sola fila. **Espacios** abre el selector de perfiles
de trabajo y mantiene sus recientes; **Disposiciones** contiene las plantillas de paneles. Las
acciones secundarias, unidades y Configuración quedan dentro de **Más** hasta que la ventana tenga
espacio para mostrarlas directamente.

**Historial de carpetas:** el ícono de reloj en la barra de herramientas despliega un
menú con las carpetas visitadas recientemente; al elegir una, el panel activo navega a
ella. En **Configuración → Avanzado** puedes ajustar cuántas carpetas recordar (1–100,
por defecto 50).

**Vista profunda (recursiva):** el botón de la barra del panel (tres líneas
escalonadas) activa un modo que muestra, en una sola lista, todo el contenido de la
carpeta actual y de sus subcarpetas, a cualquier profundidad. Cada fila se sangra
según su nivel para ver de dónde viene. Los resultados aparecen mientras se recorre el
árbol (streaming) y puedes cancelar volviendo a pulsar el botón. La visibilidad de ocultos y
archivos de sistema se aplica a la lista. El doble clic en una carpeta
sale de la vista profunda y navega a ella; en un archivo, lo abre.

El encabezado ordena las entradas dentro de cada carpeta y mantiene las ramas juntas.
El orden de la vista profunda se elige por separado del listado normal. Los filtros de
columna se comparten entre ambos modos y también se aplican al exportar.

Puedes seleccionar con clic, Ctrl+clic, Shift+clic, rectángulo o teclado, y usar Ctrl+A para
marcar las filas visibles. Se puede copiar, cortar, borrar, duplicar, transferir a otra carpeta
o panel y agregar a la Bandeja temporal. El clic derecho conserva el lote si pulsas sobre una
fila marcada; «Más opciones de Windows…» exige que todos los elementos compartan padre Shell.
Si una marca queda oculta o el lote contiene una carpeta junto a un descendiente, la acción se
rechaza completa: vuelve a seleccionar los elementos que necesitas. Durante un relistado se
espera a que las rutas seleccionadas reaparezcan; un recorrido incompleto no habilita un subconjunto.
Al crear una carpeta, Enter espera su creación y el nuevo listado; si eliges otra fila durante
ese proceso, se respeta tu elección.

La compresión propia crea el ZIP en la carpeta raíz del panel. Si eliges elementos de carpetas
distintas, conserva dentro del ZIP sus rutas relativas a esa raíz; si comparten carpeta, conserva
el formato habitual con sus nombres. En la selección entre carpetas se rechazan enlaces simbólicos
y errores al recorrer una carpeta, para no crear un ZIP incompleto. «Extraer aquí» usa
la carpeta que contiene el ZIP. El arrastre puede comenzar desde filas profundas. Un arrastre
interno de Naygo puede soltarse sobre una carpeta profunda, con confirmación; si la fila cambia
antes de soltar, se rechaza para evitar elegir otra carpeta. También puedes soltar un arrastre
externo sobre una carpeta profunda: si no se puede identificar con seguridad la fila bajo el
puntero, se rechaza el destino. Si cambias de carpeta durante la preparación, Naygo pide elegir
destino nuevamente aunque hayas vuelto a la carpeta inicial. El renombrado en lote admite elementos de distintas
carpetas cuando el recorrido profundo terminó sin errores; comprueba nombres ocupados en cada
directorio. La transformación de texto admite selecciones de distintas carpetas. La exportación
de listado a CSV o al portapapeles usa la selección o, si está vacía, todas las filas visibles;
en vista profunda requiere que el recorrido termine sin errores. En ambas vistas la captura y
generación son cancelables; si el listado cambia antes de terminar la captura, vuelve a intentarlo. Al
exportar a archivo, cancelar antes de terminar conserva el destino anterior.

Puedes enfocar con clic, flechas, Inicio, Fin o avance de página, abrir con Enter o doble clic,
y renombrar con F2. Las flechas conservan el foco y la selección por ruta aunque el recorrido
cambie el orden de las filas. Las actualizaciones conservan la identidad de los elementos mientras
el recorrido crece y los contadores del pie siguen la selección visible. Tras renombrar, la vista
conserva las filas y verifica el resultado en segundo plano. Los cambios pequeños de selección conservan las filas
existentes cuando el contenido de la vista no ha cambiado.
Si al encadenar con flechas el siguiente elemento está dentro de la carpeta recién renombrada,
el editor se cierra para evitar editar una ruta anterior; espera a que reaparezca y usa F2.

**Menú de historial en Atrás / Adelante:** junto a los botones **Atrás** y **Adelante**
de la barra de herramientas hay un pequeño triángulo **▾**. Al pulsarlo se despliega la
lista de carpetas visitadas en esa dirección (las de "atrás" o las de "adelante" según
el botón), de modo que puedes saltar directo a cualquiera sin pulsar el botón varias
veces. El **▾** se atenúa cuando no hay historial en esa dirección.

**Paleta de comandos (Ctrl+P):** un buscador rápido que aparece centrado en la parte
superior, por encima del resto de la interfaz. Lo abres con **Ctrl+P** (atajo
configurable en *Configuración → Atajos*). Escribes y la lista se filtra con
**coincidencia aproximada** (*fuzzy*): las letras que tecleas aparecen en ese orden
aunque no estén seguidas, y se resaltan las que coinciden. La paleta busca en:

- **Acciones** (Copiar, Renombrar, Calcular tamaño, Refrescar, etc.), mostrando su
  atajo asociado.
- **Archivos de la carpeta actual** del panel activo: escribe el nombre y saltas a él.
- **Carpetas recientes** y **favoritos**.
- **Temas**.

Navegas con las flechas **↑ / ↓**, **Enter** ejecuta el elemento seleccionado y **Esc**
cierra. La paleta **no** busca archivos recorriendo subcarpetas del disco; para eso
está **F3** (búsqueda recursiva).

**Buscar archivos (F3):** abre el panel de búsqueda tomando la carpeta del último panel de
archivos activo como raíz. La ruta es editable. Puedes buscar una parte del nombre o activar
comodines Windows (`*` y `?`), distinguir mayúsculas, limitarte a la carpeta raíz o incluir
subcarpetas. El campo opcional **Texto dentro del archivo** combina ambas condiciones, por
ejemplo `*cosa*.txt` y `AQUÍ`. Los resultados llegan en vivo, muestran la ruta relativa y
permiten abrir el elemento o navegar a su carpeta contenedora. Para mantener la aplicación
ágil, el contenido se revisa solo en archivos UTF-8 de texto de hasta 16 MiB; binarios y
archivos mayores se omiten de esa condición.

**Mostrar u ocultar archivos (menú del ojo):** el botón con un **ojo** en la barra de
herramientas despliega tres interruptores:

- **Mostrar archivos ocultos** (los marcados con el atributo *oculto* de Windows).
- **Mostrar archivos de sistema** (atributo *de sistema*).
- **Ocultar los que empiezan con punto** (`.algo`, al estilo de Linux).

Cada interruptor es **global** (vale para todos los paneles y el árbol), se aplica **al
instante** y se recuerda entre sesiones. Por defecto Naygo **muestra todo**; usa el menú
para esconder lo que no quieras ver.

**Favoritos con grupos:** el botón con una **estrella** en la barra de herramientas
despliega tus favoritos como un árbol para saltar rápido a cualquiera (clic en un favorito
navega el panel activo; a su derecha aparece la ruta abreviada en texto atenuado y los grupos
se expanden y contraen). Para **organizarlos** usa el
**panel de Favoritos** (agrégalo desde *Panel ▾*): con **clic derecho** sobre una fila
puedes crear un **Nuevo grupo**, **Renombrar** un grupo, **Eliminar**, o **Mover a…** otro
grupo (o a la raíz). La estrella **★** de la barra de ruta sigue anclando la carpeta actual
a la raíz de favoritos; luego la reorganizas en grupos desde el panel. Al crear o renombrar
un grupo, o cambiar el alias de un favorito, puedes escribir inmediatamente y confirmar con Enter.

---

## 4. Seleccionar

- **Clic** selecciona uno; **clic+arrastre** sobre filas no seleccionadas hace
  selección por rectángulo.
- **Ctrl+A**: seleccionar todo.
- **Espacio**: marcar/desmarcar el ítem enfocado.
- **Shift+↑/↓**, **Shift+Inicio/Fin**, **Shift+RePág/AvPág**: extender la selección.
- **Ctrl+↑/↓**: mover el foco sin tocar la selección; **Ctrl+Espacio**: marcar el
  enfocado dejando el resto.

---

## 5. Operaciones de archivo

El botón «Cancelar todo» del panel Operaciones vuelve a recibir el teclado al cerrar su
confirmación si el panel sigue activo. Si cambiaste de panel, se respeta el nuevo foco.
En el historial de Operaciones, Tab permite llegar a «Ver archivos» y Enter o espacio abre el
detalle; al cerrarlo, el foco vuelve al enlace si sigue siendo la misma operación.

| Atajo | Acción |
|-------|--------|
| **Ctrl+C / Ctrl+X / Ctrl+V** | Copiar / cortar / pegar |
| **Ctrl+Shift+V** | Elegir entre los últimos 10 conjuntos copiados/cortados en Naygo |
| **Ctrl+D** | Duplicar en la misma carpeta (` - copia`, ` - copia (2)`, …) |
| **Supr** | Enviar a la papelera |
| **Shift+Supr** | Eliminar permanente |
| **F2** | Renombrar (en línea) |
| **Shift+F2** | Renombrar por lotes (ventana) |
| **F6** | Mover la selección al otro panel |
| **Ctrl+N / Ctrl+Shift+N** | Nueva carpeta / nuevo archivo |
| **Ctrl+Z** | Deshacer la última operación |

Si un diálogo interrumpe la Bandeja temporal, al cerrarlo se recupera el foco de su lista o del
botón de acciones que lo tenía, siempre que la Bandeja siga siendo el panel activo.

Toda operación larga es **cancelable** y queda en el **Historial de acciones** con
opción de deshacer. Pegar un **texto** o una **imagen** del portapapeles crea un
archivo (formato y nombre configurables en *Configuración → Pegado / Avanzado*).
El panel de Operaciones abierto automáticamente recupera su ubicación anterior cuando la
disposición no cambió y se oculta al terminar sin errores ni elementos omitidos. Puedes desactivar
este cierre en **Configuración → Operaciones**; los paneles abiertos manualmente y los resultados
que necesitan revisión permanecen visibles.
Renombrar nunca reemplaza otro archivo: si un proceso externo crea el nuevo nombre mientras
Naygo ejecuta la operación, el renombre falla y conserva ambos archivos.
Si aparece un diálogo mientras editas un nombre con F2, el borrador permanece pendiente y el
editor activo recupera el teclado al cerrar el diálogo. Enter o un clic fuera confirman; Escape cancela.
Si el panel donde está el editor se cierra con una edición aún pendiente, esa edición se cancela.
Si un diálogo deja activo otro panel, puedes navegar en él; al pedir F2 vuelves al renombrado
pendiente para confirmarlo o cancelarlo antes de empezar otro.
F2 dentro del editor alterna la selección entre nombre, extensión y nombre completo sin borrar lo
que escribiste. Si el panel navega a otra carpeta, la edición pendiente se cancela; refrescar la
misma carpeta no la cancela.
Si cambia el orden de la lista durante F2, el editor sigue al archivo original. Mientras un
refresco parcial aún no muestra ese archivo, el panel conserva temporalmente las filas anteriores;
al terminar la edición se actualizan. Si encadenas con flechas justo cuando falta el archivo en el
listado, se intenta confirmar la edición y no se inicia otra hasta que elijas una fila.
Si un borrado por lote no puede enviar todos los orígenes a la Papelera, el historial conserva el
deshacer de los que sí se reciclaron e identifica por separado los que fallaron.
Si una operación falla antes de producir su resumen, el historial muestra el error en sus detalles;
no se ofrece un reintento automático cuando no se conoce el estado parcial.
Al arrastrar desde otra aplicación, Naygo confirma con ella la recepción sin pedirle que borre el
origen; si eliges mover, Naygo realiza el movimiento tras tu confirmación. Una fuente que solo
permita mover no puede entregar sus elementos a Naygo; una que solo permita copiar se copia.
En un arrastre externo se confirma la ruta completa del panel donde se soltó; si el panel
cambió durante la preparación se pide elegir explícitamente uno abierto;
para apuntar a una subcarpeta, navega primero a ella. Puede solicitarse cancelar la preparación,
pero una llamada COM en curso termina antes de que se aplique la cancelación.
Durante el arrastre, un borde de acento señala el panel de archivos o la bandeja bajo el cursor;
este resaltado es visual y no cambia el foco del teclado.
El enlace de detalle distingue archivos y carpetas cuando el plan conoce su tipo; en otros casos
indica «elementos» sin asumir que todos sean archivos.
Los resúmenes grandes no construyen listas completas de nombres que no se muestran.
Naygo solo registra como recuperable un elemento cuya identidad en la Papelera pudo confirmar.
En la vista **Papelera de reciclaje**, Windows muestra el nombre del elemento. Si abres la
carpeta interna `$Recycle.Bin`, puedes ver nombres de almacenamiento como `$R…`. Al restaurar
desde el historial de Naygo, el destino usa el nombre y la ruta originales.
Si el origen desapareció pero esa identidad no se pudo confirmar, el historial lo indica: revisa
la Papelera antes de actuar. Windows puede pedir confirmación si solo ofrece borrado permanente.
Antes de reciclar, Naygo consulta el nombre largo para poder restaurarlo; si Windows no permite
obtenerlo, la operación se detiene y muestra el error.
Al deshacer, la ruta original debe estar libre; cualquier entrada existente, incluso un junction
colgante, se considera ocupada para evitar reemplazarla.
El historial de `Ctrl+Shift+V` vive solo durante la sesión y guarda rutas de archivos;
no incorpora textos ni imágenes del portapapeles de Windows.

**Exportar listados:** en el menú contextual elige exportar al portapapeles o a CSV, con
separador punto y coma o coma. Si hay selección exporta solo esa selección; si no, exporta la
vista filtrada completa. Usa las columnas visibles en su orden actual, agrega la ruta completa
y los archivos CSV incluyen BOM UTF-8 para abrir correctamente en Excel.

**Clic derecho con selección múltiple:** al hacer clic derecho sobre un elemento seleccionado,
el menú conserva todo el lote. Si haces clic derecho sobre un elemento que no estaba seleccionado,
el menú usa solo ese elemento. **Más opciones de Windows…** entrega el lote al Shell cuando todos
los elementos siguen disponibles y pertenecen a una misma carpeta del Shell; si no, muestra un
aviso en vez de operar sobre una parte. Las opciones visibles dependen de Windows y de las
extensiones instaladas.

**Abrir terminal aquí:** clic derecho → *Abrir PowerShell / CMD / Windows Terminal
aquí* (abre en la carpeta seleccionada o, si no hay, la del panel). Desde la
**barra de herramientas**, el botón **Terminal** abre el mismo combo
(PowerShell / CMD y, si están instalados, Windows Terminal y WSL) en la carpeta
del panel con foco.
La solicitud conserva la carpeta elegida aunque cambies de panel, y se procesa en segundo plano.
Si la carpeta no está disponible se informa el error. Solo se admite una solicitud de terminal o
Explorador a la vez: si una está esperando al sistema, vuelve a intentar cuando termine.

**Menú de carpeta (zona vacía):** clic derecho en el **espacio vacío** de un panel
(donde no hay archivos) abre un menú de la *carpeta*: *Abrir en Explorador de
Windows*, *Nueva carpeta…*, *Pegar* y las terminales. Un clic en esa zona también
le da el **foco** al panel, así las acciones de la toolbar usan su carpeta.
El submenú **Abrir** aparece al pasar el puntero por esa opción; se abre hacia la izquierda si
está cerca del borde derecho. Al cerrar una confirmación de operación o un aviso, el panel activo
recupera el teclado cuando no hay otro de esos diálogos abierto.
La paleta y los diálogos de destinos, consultas, espacios, recetas, comparaciones, entregas y
reintentos también devuelven el teclado al panel activo al cerrarse. El detalle de una operación
se puede cerrar con Escape y recupera el foco tras cerrar un reintento superpuesto.
Si estabas editando una ruta en el panel o en la barra superior, al cerrar el diálogo vuelve el
foco al editor y se conserva lo escrito y la posición del cursor.

**Nueva(s) carpeta(s):** el botón **Carpeta** de la toolbar (o *Nueva carpeta…* del
menú de carpeta) abre un cuadro donde puedes crear **varias a la vez**: una carpeta
por línea, y usando `\` se anidan (p. ej. `proyecto\src\bin` crea las tres). Las
líneas inválidas se avisan y se omiten; las válidas se crean.
`Enter` agrega otra línea y dos `Enter` seguidos confirman la creación; `Ctrl+Enter` confirma de
inmediato. Al volver al panel, un clic o doble clic usa los modificadores de ese gesto, para que
Ctrl no quede activo por haber cerrado el cuadro.

En el árbol de carpetas, el atajo configurado para **Nueva carpeta** (por defecto `Ctrl+N`)
usa la carpeta señalada por el cursor del árbol. También puedes abrir el menú contextual de una
carpeta del árbol y elegir *Nueva carpeta…*. Al cerrar el cuadro, el foco vuelve al panel activo;
tras completarse la creación, la carpeta creada se selecciona si el panel aún muestra su destino.
Mientras la creación y su refresco están pendientes, Enter no abre automáticamente la fila que
estaba enfocada antes y no queda en cola: presiónalo de nuevo al terminar. Puedes seleccionar
deliberadamente otra fila con mouse, flechas o tipeo. Esa elección se conserva cuando llega el
resultado de la creación y ante sus avisos de actualización tardíos. Si Enter se bloquea durante
la espera, Naygo muestra un aviso breve.
Al abrir el cuadro desde el panel de archivos, Naygo no espera una comprobación de disco; si el
destino dejó de existir o no es una carpeta, el resultado de la operación informa el error.
Al crear desde el menú del árbol o breadcrumb se conserva la carpeta elegida, incluso si deja de
estar disponible; Naygo no reemplaza ese destino por la carpeta de otro panel.

**Panel de Operaciones:** muestra las copias y movimientos en curso. Aparece solo al
iniciar una operación (también se puede agregar a mano desde el menú **Panel ▾**).
Tiene tres zonas:

- **Calculando…:** antes de copiar una carpeta, Naygo la recorre para saber cuánto pesa.
  Eso ahora ocurre **en segundo plano** (la aplicación no se congela) y se muestra como
  *"Calculando… N archivos, M GB"* con su propio botón **Cancelar**.
  Si el sistema impide enumerar una entrada, la operación se detiene antes de copiar o mover
  elementos y señala la carpeta afectada; así no se omiten archivos de forma silenciosa.
- **En curso:** la operación que se está ejecutando. Muestra el archivo actual, una
  **barra de progreso**, *"Copiado X de Y"*, la **velocidad** (media y pico), el
  **tiempo transcurrido** y el **restante** estimado. Trae botones **Pausar/Reanudar**,
  **Saltar** y **Cancelar**.
- **En cola:** las operaciones que esperan su turno.
- **Historial reciente:** las últimas operaciones terminadas con su resultado (cuántos
  archivos se copiaron, se saltaron o fallaron).

Cómo se muestra el progreso (panel, ventana modal o ambos) también depende de
*Configuración → Avanzado → progreso de operaciones* (ver §9).
Si aparece un diálogo mientras usas un botón de Operaciones, el botón recupera el teclado al
cerrarlo cuando la misma operación continúa visible y el panel sigue activo.

**Pausar y reanudar:** una copia o movimiento en curso se puede **pausar** con el botón
del panel de Operaciones (se detiene sin perder lo ya copiado) y **reanudar** desde
donde quedó.

**Arrastrar archivos entre paneles:** puedes arrastrar los archivos seleccionados de un
panel a otro. Dentro del **mismo disco** la operación **mueve**; hacia **otro disco**
**copia**. Mantener **Ctrl** mientras arrastras **fuerza copiar**; mantener **Shift**
**fuerza mover**. (Arrastrar archivos **fuera** de Naygo, al Explorador de Windows,
sigue funcionando como antes.) Naygo comprueba los conflictos en segundo plano; si no puede
consultar el destino, cancela la acción y muestra un aviso para que puedas volver a intentarlo.

También puedes soltar sobre una fila de **subcarpeta del mismo panel**, incluso si
arrastraste otra carpeta. Naygo muestra la ruta destino antes de mover o copiar.
Para devolver al lugar original archivos y carpetas movidos a un destino libre del mismo
volumen, usa **Deshacer** en Historial o presiona **Ctrl+Z**.

También puedes arrastrar archivos directamente desde una ventana de **7-Zip**, **WinRAR** u
otra aplicación que entregue contenido virtual. Naygo lo prepara en segundo plano y lo copia
al panel de destino; el contenido temporal se elimina automáticamente al terminar o cancelar.
La copia aparece en **Operaciones**, donde puedes seguir su progreso o cancelarla.

**Comprimir y extraer (.zip):** con uno o más archivos o carpetas seleccionados, el menú
contextual ofrece **«Comprimir en .zip»** (pide el nombre del archivo; si ya existe un
`.zip` con ese nombre, Naygo nunca lo pisa: desambigua agregando «(2)»). Con un `.zip`
seleccionado, el menú ofrece **«Extraer aquí»**. Ambas operaciones corren en **segundo
plano** con progreso y **cancelación** desde el panel de Operaciones (cancelar una
compresión en curso borra el archivo `.zip` parcial). Si al extraer algún archivo choca
con uno existente, se abre el mismo **diálogo de conflicto lado a lado** que usan copiar
y mover. Las dos operaciones se pueden **deshacer** de forma segura: a la papelera va
solo lo que la operación creó, nunca algo que ya existía antes.

**Sincronizar carpetas:** abre el menú contextual sobre el fondo de un panel y elige
**Sincronizar con otro panel**. Naygo usa el panel activo y otro panel de archivos abierto,
compara ambos árboles en segundo plano y muestra el plan antes de tocar datos. Puedes elegir
izquierda → derecha, derecha → izquierda o bidireccional y desmarcar acciones individuales.
**Eliminar sobrantes** parte apagado; al activarlo, esas entradas se envían a la papelera.
Los conflictos donde ambos lados cambiaron se muestran, pero no se eligen automáticamente.

**Bandeja temporal:** agrega el panel **Bandeja** desde **Panel ▾**. En cualquier panel de
archivos, selecciona elementos y usa **Agregar a la bandeja** en el menú contextual. La
bandeja acepta rutas de carpetas distintas, elimina duplicados y permite copiarlas o moverlas
a una carpeta elegida, enviarlas a la papelera, quitarlas o limpiar la lista. Solo guarda las
rutas durante la sesión: no duplica el contenido ni se persiste al cerrar Naygo.

**Transformar texto:** selecciona uno o varios archivos y elige **Transformar texto…** en el
menú contextual. El diagnóstico distingue Windows CRLF, Unix/macOS LF, Mac clásico CR y
mezclas. Como destino puedes conservar o convertir la codificación entre UTF-8, UTF-8 con
BOM y UTF-16 LE/BE, asegurar o quitar el salto final y limpiar espacios al final de cada
línea. El reemplazo es transaccional y cancelable; cancelar no revierte los archivos ya
transformados y esta operación no se puede deshacer con Ctrl+Z. Los binarios, textos inválidos
y archivos mayores a 128 MiB se rechazan para evitar corrupción o picos de memoria.

**Cuando algo ya existe en el destino:**

- **Un archivo** que choca abre el aviso *"Ya existe «…»"* con **Saltar**, **Renombrar**,
  **Renombrar antiguo**, **Sobrescribir** y **Cancelar todo** (este último aborta la operación
  entera). **Renombrar antiguo** conserva el que ya estaba bajo un nombre con sufijo y deja el
  entrante en la ruta original; si otro proceso ocupa ese nombre de respaldo, Naygo conserva los
  tres archivos y reporta el fallo. La casilla
  *"Aplicar a todos los conflictos"* repite tu elección para los demás archivos. Cerrar el
  aviso con **Escape** o un clic afuera **salta** ese archivo y sigue.
- **Una carpeta** que ya existe pregunta una sola vez qué hacer:
  - **Fusionar:** copia dentro de la carpeta existente; si algún archivo choca, pregunta por él.
  - **Reemplazar:** deja la carpeta destino con **solo** el contenido del origen (borra lo que
    había). Es una acción destructiva, por eso el botón está marcado en rojo.
  - **Saltar:** no copia esa carpeta y continúa con el resto.
  - **Cancelar:** aborta toda la operación.

  Con varias carpetas en conflicto, *"Aplicar a todas las carpetas"* repite la decisión.

---

## 6. Columnas (estilo Excel)

El encabezado de la tabla:

- **Clic** en una columna ordena por ella; otro clic invierte (▲/▼).
- **Arrastra el borde derecho** de una columna para cambiar su ancho.
- **Botón ≡** (al final del encabezado): mostrar/ocultar columnas.
- **Clic derecho** en una columna abre su menú: **ordenar** asc/desc, **filtrar…**,
  **quitar filtro**, **mover ←/→**, **ocultar**.

**Filtros por columna:**

- **Nombre**: contiene un texto (con opción de distinguir mayúsculas).
- **Extensión**: marca los tipos a mostrar (con su conteo). Las carpetas se agrupan bajo
  **Carpeta** aunque sus nombres contengan uno o varios puntos; los archivos conservan su extensión
  y los que no tienen muestran «(sin extensión)». Los enlaces y otros elementos no clasificados como
  archivo/carpeta conservan su extensión si la tienen.
- **Tamaño**: rango mínimo/máximo (acepta `2 KB`, `1.5 M`, `3 GB`, o bytes).
- **Fecha** (modificado/creado): rango `AAAA-MM-DD`.

Las columnas con filtro muestran un embudo. Si un filtro deja la vista vacía, aparece
el aviso **"Sin coincidencias"**.

---

## 7. Renombrar por lotes (Shift+F2)

Renombra varios archivos a la vez con una vista previa en vivo.

- **Plantilla** con comodines: `{nombre}`, `{ext}`, `{n}` (contador), `{n:3}`
  (contador con ceros), y de la fecha de modificación: `{año}/{anio}/{year}`,
  `{mes}/{month}`, `{dia}/{day}`, `{hora}/{hour}`, `{min}`, `{seg}/{sec}`.
- **Buscar/Reemplazar** (con regex opcional; grupos `$1`, `$2`…).
- **Mayúsculas**: sin cambio / minúsculas / MAYÚSCULAS / Título.
- **Contador**: valor inicial y paso.
- **Incluir extensión**: si está activo, la plantilla produce el nombre completo.

La tabla **Antes → Después** colorea cada fila: verde = se renombra, atenuado = sin
cambio, rojo = inválido o colisión. **Aplicar** se habilita solo si no hay problemas;
es **una sola operación deshacible**.

En vista profunda, si el lote abarca varias carpetas, la columna «Antes» muestra la carpeta
relativa de cada elemento. El contador sigue el orden del lote y las colisiones se comprueban
en la carpeta de cada destino. Se requiere un recorrido profundo completo y sin errores.

---

## 8. Plantillas de disposición (Layouts)

El botón **Layouts** de la toolbar guarda y aplica disposiciones de paneles:

- **Integradas**: Minimalista, Clásico, Dual-pane, Power-user.
- **Guardar disposición actual…**: guarda tu layout con un nombre.
- Tus plantillas se pueden **borrar** (✕). Todo persiste entre sesiones.

---

## 9. Configuración

Se abre desde la toolbar. Es **arrastrable** por su encabezado, **redimensionable**
por la esquina inferior derecha (el contenido se adapta al tamaño) y se cierra con la
**✕**. Secciones:

- **General**: fila "..", botones solo-ícono (desactívalo para ver icono y etiqueta), posición de la barra, tamaño sin
  subcarpetas, iniciar con Windows, iniciar minimizado en la bandeja (requiere "iniciar
  con Windows" y el ícono de bandeja activos), e **Idioma** en su propia sección: **diez
  idiomas** de fábrica, cada uno mostrado con su nombre nativo en el selector.
- **Operaciones**: cola vs. paralelo, confirmar papelera, resumen al terminar.
- **Pegado**: todo lo del portapapeles en un solo lugar — confirmar nombre al pegar,
  plantilla/extensión del texto pegado, nombre del archivo de imagen pegada, **formato
  de imagen (PNG/JPG)** y **calidad JPG** (escala 1–100).
- **Previsualización**: interruptor **"Resaltar código automáticamente"** (la presentación
  seleccionable actual prioriza copia parcial; ver §2) y **reglas por
  extensión** para forzar un modo de vista a una extensión concreta (estas reglas
  tienen prioridad sobre el resaltado automático).
- **Apariencia**: el **tema** se elige en una galería de tarjetas (cada una muestra
  sus colores; la activa lleva borde de acento y ★). Vienen **quince temas de fábrica**
  (`dark-blue`, `commander`, `solarized-dark`, `paper`, `dracula`, `winxp`,
  `green-on-blue`, `high-contrast`, `neon-retro`, Nieve, Perla, Arena, Salvia, Bruma
  y Grafito claro) y puedes crear los tuyos con el **editor de temas** (ver abajo). Si tienes
  temas o sets propios, pueden aparecer unos instantes después de que abra la ventana mientras
  Naygo termina de descubrirlos; los assets de fábrica se muestran durante esa carga.
  El pie de los paneles de archivos usa un matiz del tema para distinguirlo del título,
  también al utilizar temas personalizados anteriores; sus leyendas usan el texto principal del tema.
  Los temas de fábrica se ajustaron para mejorar la legibilidad del texto secundario sobre filas,
  selecciones y superficies comunes.
  **Abrir ajustes de íconos** lleva a la única galería, en Íconos, sin cambiar el set
  elegido. También: formato de fecha y tamaño, densidad de fila. Todo se aplica
  en caliente. Fuentes por función: interfaz (sistema), listados/árbol/bandeja (heredan
  interfaz) y preview (Consolas). Usa familias instaladas, sin descargarlas; una familia
  inexistente utiliza el reemplazo disponible del toolkit.
  En Windows puedes activar **Usar íconos del sistema para archivos** para mostrar el icono
  asociado a cada extensión. Viene desactivado; las asociaciones se consultan en segundo plano
  y se limita el cache a 512 tipos. Si Windows no devuelve un icono, se conserva el icono de Naygo.
- **Íconos**: grilla con **todos los objetos** con ícono (carpetas, discos, acciones de
  la barra…); cada uno se puede cambiar por el de otro set instalado o por un **PNG
  propio**. Diez sets: Lucide, Fluent, Tabler, Material, Flat Color, Mono, Outline,
  Tiles, Vivid y Pastel. Un set personalizado se comparte como pack `.naygoset`.
  La galería de sets está sólo aquí, junto a la personalización de objetos.
  El editor de temas distingue **Énfasis de íconos y texto** del **Fondo de archivos
  recién añadidos**; cambiar uno no altera el otro. Los temas antiguos mantienen su
  aspecto: si no incluyen el nuevo color, se usa su color de resaltado anterior.
- **Atajos**: editor de atajos por acción (cambiar / restablecer / restablecer todo),
  con detección de conflictos.
- **Importar/Exportar**: idioma, temas, íconos y configuración se exportan e importan
  **por separado**, cada uno con su propia extensión (`.naygolang`, `.naygotheme`,
  `.naygoset`, `.naygoconf`). Al importar ajustes creados en otro sistema, Naygo conserva
  los keybindings y las rutas compatibles; en Linux omite las rutas de inicio/recientes que
  no sean absolutas POSIX, el autoinicio y el atajo global si venían activados, y muestra
  cuáles omitió. El pack no guarda preferencias de terminal.
- **Avanzado**: **Carpeta de inicio (Home)** (destino del botón Inicio / Alt+Inicio;
  vacío = tu carpeta personal), **Pie de panel** (ver abajo), progreso de operaciones
  (panel/modal/siempre), modo de bajo consumo, archivos nuevos al final, historial de
  carpetas a recordar (1–100), **bandeja del sistema**, **cerrar a la bandeja (no
  salir)**, **atajo global** para mostrar/ocultar Naygo (ver §1), y **Restablecer
  todo** (en dos pasos).
- **Acerca de**: autoría, licencia, stack, enlace al repo (y un pequeño easter egg).

**Pie de panel (en Avanzado):** controla la barra de información al pie de cada panel
(ver §2):

- Una **casilla** para mostrarlo u ocultarlo.
- Un **combo de plantilla** con formatos predefinidos: *Compacta*, *Completa*,
  *Solo disco*, *Solo selección* y **"Personalizada…"**.
- Si eliges **"Personalizada…"**, aparece un campo para escribir tu plantilla con
  *tokens*, la lista de tokens disponibles y una **vista previa en vivo**. Tokens:
  `{sel}` (seleccionados), `{total}` (total visible), `{marked}` (bytes marcados),
  `{free}` (espacio libre), `{disk_total}` (capacidad del disco), `{pct}` (% usado),
  `{items}` (elementos), `{files}` (archivos), `{dirs}` (carpetas).
- La plantilla es **global**: vale para todos los paneles, pero cada panel rellena los
  tokens con **sus propios** datos.

**Editor de temas (en Apariencia):** crea un tema a tu medida sin tocar los de fábrica.

- En cada tarjeta de tema de fábrica hay un botón **"Personalizar"** que crea una copia
  editable (los nueve originales quedan intactos). Tus temas propios muestran además
  **"Editar"** y **"Eliminar"**.
- El editor lista los **once colores** del tema (acento, fondo del panel, fila, texto,
  selección, borde, etc.). Al tocar uno se abre un **selector tipo paleta**: una grilla
  de presets, una fila de colores estándar, y **"Más colores…"** para ajustar el valor
  exacto con tres barras **R/G/B** (0–255) y ver el **hex**.
- El cambio se **aplica a toda la aplicación en vivo** mientras editas, así ves el
  resultado real. **Guardar** conserva el tema (queda activo y disponible en la galería),
  **Cancelar** revierte al tema que tenías, y **Restaurar de fábrica** vuelve los colores
  a los del tema original del que partiste.

---

## 10. Atajos de teclado (resumen)

Todos son configurables en *Configuración → Atajos*. Por defecto:

Los controles de pestañas, los botones de la barra principal y de unidades, las filas de archivos y
las acciones del Inspector exponen nombres y acciones para lectores de pantalla. Las filas anuncian
nombre, tipo y selección; su acción predeterminada abre el archivo o entra a la carpeta. Cada unidad
anuncia su ruta de destino; los campos anuncian su función sin leer automáticamente el texto que
contienen. La comprobación con Narrador de la distribución actual sigue pendiente.

| Tecla | Acción |
|-------|--------|
| ↑ / ↓ | Mover foco |
| Enter | Abrir / entrar |
| Backspace, ← | Subir un nivel |
| Alt+← / Alt+→ | Atrás / adelante |
| Alt+Inicio | Ir a la carpeta de inicio |
| Tab | Cambiar panel activo |
| Ctrl+P | Abrir la paleta de comandos |
| F1 | Ayuda |
| F2 / Shift+F2 | Renombrar / renombrar por lotes |
| F3 / Ctrl+F | Buscar archivos |
| F5 | Refrescar |
| F6 | Mover al otro panel |
| Esc | Cancelar listado |
| Ctrl+C / X / V | Copiar / cortar / pegar |
| Ctrl+Shift+V | Historial interno del portapapeles |
| Ctrl+D | Duplicar la selección en su misma carpeta |
| Supr / Shift+Supr | Papelera / eliminar permanente |
| Ctrl+N / Ctrl+Shift+N | Nueva carpeta / archivo |
| Ctrl+A | Seleccionar todo |
| Espacio / Ctrl+Espacio | Marcar / marcar dejando el resto |
| Ctrl+L, F4 | Editar la ruta |
| Ctrl+Z | Deshacer |
| Ctrl+1..9 | Ir al favorito N |
| Ctrl+Alt+Z (global) | Mostrar Naygo y traerlo al frente desde cualquier aplicación (se configura en Avanzado, no en Atajos) |

---

## 11. Personalización avanzada (packs)

Naygo guarda su configuración junto al ejecutable (modo portable). Puedes agregar:

- **Idiomas**: suelta un `.json` de traducción en la carpeta `lang/`; aparece en el
  selector. **Diez idiomas** vienen de fábrica (es, en, pt, fr, de, it, zh, ja, ko, hi).
  Guía paso a paso: [`AGREGAR-IDIOMA.md`](AGREGAR-IDIOMA.md).

**Lanzar desde un acceso directo o terminal:** `naygo.exe "D:\carpeta"` abre Naygo en esa
carpeta; además acepta `--theme <id>` y `--layout <nombre>` (solo para esa sesión) y
`--help`/`--version`. Detalle en el README.
- **Temas**: *color sets* intercambiables en caliente.
- **Sets de íconos**: empaquetados o propios.

Cada tópico se importa y exporta **por separado** desde *Configuración →
Importar/Exportar*, con su propia extensión: idioma (`.naygolang`), temas
(`.naygotheme`), íconos (`.naygoset`) y configuración (`.naygoconf`).

---

© 2026 **Nicolás Groth / ISGroth** — Licencia MIT.
