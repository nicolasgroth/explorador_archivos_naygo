# Guía de usuario — Naygo

Naygo es un explorador de archivos rápido y liviano para Windows 10/11, estilo
*Commander* (inspirado en Directory Opus / Total Commander / XYplorer). Esta guía
explica cada sección de la app y cómo usarla, sobre todo **por teclado**, que es
la forma más rápida de trabajar.

> **Atajo de ayuda:** dentro de la app, **F1** abre una ayuda rápida con esta
> misma información y la lista de atajos activos.

---

Al usar **Atrás** o **Adelante**, Naygo recupera durante la sesión la selección, el archivo
enfocado, los filtros y la posición que tenía el listado en esa visita. Los archivos que ya no
existen se omiten. Este contexto no se guarda al cerrar Naygo.

Al copiar o mover a otro panel, aparece el **radar de destinos**. Pulsa **1–9** para elegir
rápidamente entre paneles, favoritos, destinos recientes y carpetas frecuentes; **Esc** cancela.

## 1. La ventana

La ventana se divide en:

- **Barra de herramientas** (arriba): botones de navegación (atrás / adelante /
  inicio), botones de acciones + la tira de unidades de disco (C:, D:, …) + la ruta
  actual.
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

**Cerrar a la bandeja:** por defecto, el botón **X** de la ventana no cierra
Naygo: lo esconde en la **bandeja del sistema** (junto al reloj), de modo que
la próxima vez que lo abras sea instantáneo. Para salir de verdad, haz **clic
derecho** en el ícono de la bandeja y elige **Salir**. Este comportamiento se
puede desactivar en *Configuración → Avanzado → "Cerrar a la bandeja (no
salir)"*, y en ese caso la X vuelve a cerrar la aplicación como cualquier
programa.

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

**Pie de panel (footer):** cada panel de archivos muestra al pie una barra con
información de **ese** panel: archivos seleccionados sobre el total, bytes marcados, y
espacio libre/total del disco de su unidad. Se muestra u oculta y se da formato desde
**Configuración → Avanzado**, sección *Pie de panel* (ver §9). El formato es **global**
(igual en todos los paneles), pero cada panel muestra **sus propios** datos.

**Vista previa de código y texto:**

- **Resaltado automático de código:** los archivos de código de extensiones conocidas
  (`.rs`, `.json`, `.xml`, `.js`, `.html`, `.css`, `.c`, `.cpp`, `.java`, `.py`, `.sh`,
  `.md`, `.yaml`, `.toml`, `.ini`, `.sql`) se muestran con **resaltado de sintaxis por
  colores** de forma automática. Viene activado por defecto y se puede apagar con el
  interruptor **"Resaltar código automáticamente"** en *Configuración →
  Previsualización*. Las **reglas por extensión** (forzar un modo de vista a una
  extensión concreta) tienen prioridad sobre este ajuste global.
- **Seleccionar y copiar:** el **texto plano** de la vista previa siempre se puede
  seleccionar con el mouse y copiar con **Ctrl+C**. Cuando la vista muestra **código
  resaltado por colores**, aparece un botón **✎** en la barra de la vista previa que
  alterna a una vista de **texto seleccionable** (en un solo color) para poder
  seleccionar y copiar; al pulsarlo de nuevo vuelve a la vista con colores.
- **Modelos de impresión 3D:** los archivos **STL** (binario o ASCII) y **3MF** muestran
  una vista estática sin requerir GPU. En 3MF creados por un slicer se prefiere la miniatura
  incrustada (más fiel, con colores y muy liviana); sin ella se rasteriza por CPU. El panel de
  propiedades informa dimensiones X/Y/Z, triángulos y, para 3MF, la unidad declarada.
  La primera versión prioriza rapidez y compatibilidad: no rota el modelo ni interpreta
  materiales complejos o funciones de laminado.

---

## 3. Navegar

- **Doble clic** en una carpeta entra; **Enter** entra a la enfocada.
- **Backspace** o **←** suben un nivel.
- **Alt+←** / **Alt+→**: atrás / adelante en el historial (como un navegador).
- **F5**: refresca (vuelve a leer la carpeta del disco).
- **Tab**: cambia el panel de archivos activo.
- **Tipeo rápido (typeahead):** escribe las primeras letras de un nombre y el foco
  salta a ese ítem. Si haces una pausa (~½ segundo), empieza una búsqueda nueva.
- **Esc**: cancela un listado en curso (útil en discos de red lentos).

**Botones Atrás / Adelante / Inicio (estilo navegador):** la barra de herramientas
tiene tres botones de navegación que actúan sobre el panel activo:

- **Atrás** (**Alt+←**) y **Adelante** (**Alt+→**) recorren el historial de carpetas
  del panel. Se **atenúan** (se deshabilitan) cuando no hay a dónde ir hacia atrás o
  hacia adelante.
- **Inicio** (ícono de casa, **Alt+Inicio**) lleva a la carpeta de inicio. Esa carpeta
  se define en **Configuración → Avanzado → "Carpeta de inicio (Home)"**; si la dejas
  vacía, se usa tu carpeta personal (la de tu perfil de usuario).

**Barra de ruta (breadcrumbs):** muestra la ruta como segmentos clicables. Clic en el
hueco vacío la convierte en un editor de texto con autocompletado (Enter navega, Esc
cancela). A la derecha tienes **★** (anclar a favoritos) y **📋** (copiar la ruta).

**Unidades de disco:** la tira C:/D:/… de la toolbar navega el panel activo a esa
unidad.

**Historial de carpetas:** el ícono de reloj en la barra de herramientas despliega un
menú con las carpetas visitadas recientemente; al elegir una, el panel activo navega a
ella. En **Configuración → Avanzado** puedes ajustar cuántas carpetas recordar (1–100,
por defecto 50).

**Vista profunda (recursiva):** el botón de la barra del panel (tres líneas
escalonadas) activa un modo que muestra, en una sola lista, todo el contenido de la
carpeta actual y de sus subcarpetas, a cualquier profundidad. Cada fila se sangra
según su nivel para ver de dónde viene. Los resultados aparecen mientras se recorre el
árbol (streaming) y puedes cancelar con **Esc** o volviendo a pulsar el botón. El orden
y los filtros de columna funcionan sobre toda la lista. El doble clic en una carpeta
sale de la vista profunda y navega a ella; en un archivo, lo abre.

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
navega el panel activo; los grupos se expanden y contraen). Para **organizarlos** usa el
**panel de Favoritos** (agrégalo desde *Panel ▾*): con **clic derecho** sobre una fila
puedes crear un **Nuevo grupo**, **Renombrar** un grupo, **Eliminar**, o **Mover a…** otro
grupo (o a la raíz). La estrella **★** de la barra de ruta sigue anclando la carpeta actual
a la raíz de favoritos; luego la reorganizas en grupos desde el panel.

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

Toda operación larga es **cancelable** y queda en el **Historial de acciones** con
opción de deshacer. Pegar un **texto** o una **imagen** del portapapeles crea un
archivo (formato y nombre configurables en *Configuración → Pegado / Avanzado*).
El historial de `Ctrl+Shift+V` vive solo durante la sesión y guarda rutas de archivos;
no incorpora textos ni imágenes del portapapeles de Windows.

**Exportar listados:** en el menú contextual elige exportar al portapapeles o a CSV, con
separador punto y coma o coma. Si hay selección exporta solo esa selección; si no, exporta la
vista filtrada completa. Usa las columnas visibles en su orden actual, agrega la ruta completa
y los archivos CSV incluyen BOM UTF-8 para abrir correctamente en Excel.

**Abrir terminal aquí:** clic derecho → *Abrir PowerShell / CMD / Windows Terminal
aquí* (abre en la carpeta seleccionada o, si no hay, la del panel). Desde la
**barra de herramientas**, el botón **Terminal** abre el mismo combo
(PowerShell / CMD y, si están instalados, Windows Terminal y WSL) en la carpeta
del panel con foco.

**Menú de carpeta (zona vacía):** clic derecho en el **espacio vacío** de un panel
(donde no hay archivos) abre un menú de la *carpeta*: *Abrir en Explorador de
Windows*, *Nueva carpeta…*, *Pegar* y las terminales. Un clic en esa zona también
le da el **foco** al panel, así las acciones de la toolbar usan su carpeta.

**Nueva(s) carpeta(s):** el botón **Carpeta** de la toolbar (o *Nueva carpeta…* del
menú de carpeta) abre un cuadro donde puedes crear **varias a la vez**: una carpeta
por línea, y usando `\` se anidan (p. ej. `proyecto\src\bin` crea las tres). Las
líneas inválidas se avisan y se omiten; las válidas se crean.

**Panel de Operaciones:** muestra las copias y movimientos en curso. Aparece solo al
iniciar una operación (también se puede agregar a mano desde el menú **Panel ▾**).
Tiene tres zonas:

- **Calculando…:** antes de copiar una carpeta, Naygo la recorre para saber cuánto pesa.
  Eso ahora ocurre **en segundo plano** (la aplicación no se congela) y se muestra como
  *"Calculando… N archivos, M GB"* con su propio botón **Cancelar**.
- **En curso:** la operación que se está ejecutando. Muestra el archivo actual, una
  **barra de progreso**, *"Copiado X de Y"*, la **velocidad** (media y pico), el
  **tiempo transcurrido** y el **restante** estimado. Trae botones **Pausar/Reanudar**,
  **Saltar** y **Cancelar**.
- **En cola:** las operaciones que esperan su turno.
- **Historial reciente:** las últimas operaciones terminadas con su resultado (cuántos
  archivos se copiaron, se saltaron o fallaron).

Cómo se muestra el progreso (panel, ventana modal o ambos) también depende de
*Configuración → Avanzado → progreso de operaciones* (ver §9).

**Pausar y reanudar:** una copia o movimiento en curso se puede **pausar** con el botón
del panel de Operaciones (se detiene sin perder lo ya copiado) y **reanudar** desde
donde quedó.

**Arrastrar archivos entre paneles:** puedes arrastrar los archivos seleccionados de un
panel a otro. Dentro del **mismo disco** la operación **mueve**; hacia **otro disco**
**copia**. Mantener **Ctrl** mientras arrastras **fuerza copiar**; mantener **Shift**
**fuerza mover**. (Arrastrar archivos **fuera** de Naygo, al Explorador de Windows,
sigue funcionando como antes.)

También puedes arrastrar archivos directamente desde una ventana de **7-Zip**, **WinRAR** u
otra aplicación que entregue contenido virtual. Naygo lo prepara en segundo plano y lo copia
al panel de destino; el contenido temporal se elimina automáticamente al terminar o cancelar.

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
línea. El reemplazo es transaccional y cancelable; los binarios, textos inválidos y archivos
mayores a 128 MiB se rechazan para evitar corrupción o picos de memoria.

**Cuando algo ya existe en el destino:**

- **Un archivo** que choca abre el aviso *"Ya existe «…»"* con **Saltar**, **Renombrar**,
  **Sobrescribir** y **Cancelar todo** (este último aborta la operación entera). La casilla
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
- **Extensión**: marca los tipos a mostrar (con su conteo).
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

- **General**: fila "..", botones solo-ícono, posición de la barra, tamaño sin
  subcarpetas, iniciar con Windows, iniciar minimizado en la bandeja (requiere "iniciar
  con Windows" y el ícono de bandeja activos), e **Idioma** en su propia sección: **diez
  idiomas** de fábrica, cada uno mostrado con su nombre nativo en el selector.
- **Operaciones**: cola vs. paralelo, confirmar papelera, resumen al terminar.
- **Pegado**: todo lo del portapapeles en un solo lugar — confirmar nombre al pegar,
  plantilla/extensión del texto pegado, nombre del archivo de imagen pegada, **formato
  de imagen (PNG/JPG)** y **calidad JPG** (escala 1–100).
- **Previsualización**: interruptor **"Resaltar código automáticamente"** (resaltado de
  sintaxis por colores para extensiones de código conocidas; ver §2) y **reglas por
  extensión** para forzar un modo de vista a una extensión concreta (estas reglas
  tienen prioridad sobre el resaltado automático).
- **Apariencia**: el **tema** se elige en una galería de tarjetas (cada una muestra
  sus colores; la activa lleva borde de acento y ★). Vienen **cinco temas de fábrica**
  (Dark Blue, Windows XP, Verde sobre azul, High Contrast y Neón Retro) y puedes crear
  los tuyos con el **editor de temas** (ver abajo). El **set de íconos** también se
  elige en una galería de tarjetas, con un botón **Personalizar** que salta a la
  pestaña Íconos. También: formato de fecha y tamaño, densidad de fila. Todo se aplica
  en caliente.
- **Íconos**: grilla con **todos los objetos** con ícono (carpetas, discos, acciones de
  la barra…); cada uno se puede cambiar por el de otro set instalado o por un **PNG
  propio**. Un set personalizado se comparte como pack `.naygoset`.
- **Atajos**: editor de atajos por acción (cambiar / restablecer / restablecer todo),
  con detección de conflictos.
- **Importar/Exportar**: idioma, temas, íconos y configuración se exportan e importan
  **por separado**, cada uno con su propia extensión (`.naygolang`, `.naygotheme`,
  `.naygoset`, `.naygoconf`).
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
  editable (los cinco originales quedan intactos). Tus temas propios muestran además
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
