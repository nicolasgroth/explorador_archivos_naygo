# Guía de usuario — Naygo

Naygo es un explorador de archivos rápido y liviano para Windows 10/11, estilo
*Commander* (inspirado en Directory Opus / Total Commander / XYplorer). Esta guía
explica cada sección de la app y cómo usarla, sobre todo **por teclado**, que es
la forma más rápida de trabajar.

> **Atajo de ayuda:** dentro de la app, **F1** abre una ayuda rápida con esta
> misma información y la lista de atajos activos.

---

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

---

## 2. Paneles

Cada panel puede ser de un **tipo**:

| Tipo | Para qué sirve |
|------|----------------|
| **Archivos** | Lista navegable de una carpeta. Es el panel principal. |
| **Árbol** | Árbol de carpetas; clic navega el panel de archivos activo. |
| **Propiedades** | Datos del ítem enfocado (nombre, tipo, tamaño, fechas). |
| **Historial de acciones** | Operaciones hechas, con botón de deshacer. |
| **Favoritos** | Carpetas ancladas + recientes. |
| **Vista previa** | Vista liviana del archivo enfocado: texto, imagen, o la lista de contenido de un `.zip`. |

**Dividir y reorganizar:**

- El botón **＋** divide el panel activo (elige dirección: derecha/abajo/izq/arriba).
- El botón **Panel ▾** agrega un panel especial (árbol, propiedades, etc.).
- **Arrastra la barra de título** de un panel sobre otro para reacomodar (los bordes
  dividen; el centro apila como pestaña).
- **Arrastra las barras divisorias** para redimensionar: mientras arrastras verás una
  barra-fantasma de acento que marca dónde quedará el borde; al soltar se aplica.
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
| **Supr** | Enviar a la papelera |
| **Shift+Supr** | Eliminar permanente |
| **F2** | Renombrar (en línea) |
| **Shift+F2** | Renombrar por lotes (ventana) |
| **F6** | Mover la selección al otro panel |
| **Ctrl+N / Ctrl+Shift+N** | Nuevo archivo / nueva carpeta |
| **Ctrl+Z** | Deshacer la última operación |

Toda operación larga es **cancelable** y queda en el **Historial de acciones** con
opción de deshacer. Pegar un **texto** o una **imagen** del portapapeles crea un
archivo (formato y nombre configurables en *Configuración → Pegado / Avanzado*).

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
  subcarpetas, iniciar con Windows.
- **Operaciones**: cola vs. paralelo, confirmar papelera, resumen al terminar.
- **Pegado**: confirmar nombre al pegar, plantilla/extensión del texto pegado.
- **Previsualización**: interruptor **"Resaltar código automáticamente"** (resaltado de
  sintaxis por colores para extensiones de código conocidas; ver §2) y **reglas por
  extensión** para forzar un modo de vista a una extensión concreta (estas reglas
  tienen prioridad sobre el resaltado automático).
- **Apariencia**: el **tema** se elige en una galería de tarjetas (cada una muestra
  sus colores; la activa lleva borde de acento y ★). Hay temas claros y oscuros
  (Citrus Glow, Neon Retro, Ocean Midnight, Ember Forge, Polar Graphite, y los
  clásicos). También: idioma, set de íconos, formato de fecha y tamaño, densidad de
  fila. Todo se aplica en caliente.
- **Atajos**: editor de atajos por acción (cambiar / restablecer / restablecer todo),
  con detección de conflictos.
- **Importar/Exportar**: packs `.zip` de idioma, tema o configuración.
- **Avanzado**: **Carpeta de inicio (Home)** (destino del botón Inicio / Alt+Inicio;
  vacío = tu carpeta personal), **Pie de panel** (ver abajo), progreso de operaciones
  (panel/modal/siempre), formato de imagen pegada, modo de bajo consumo, archivos
  nuevos al final, historial de carpetas a recordar (1–100), bandeja del sistema,
  cerrar-a-bandeja, y **Restablecer todo** (en dos pasos).
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
| F1 | Ayuda |
| F2 / Shift+F2 | Renombrar / renombrar por lotes |
| F3 | Calcular el tamaño de la carpeta |
| F5 | Refrescar |
| F6 | Mover al otro panel |
| Esc | Cancelar listado |
| Ctrl+C / X / V | Copiar / cortar / pegar |
| Supr / Shift+Supr | Papelera / eliminar permanente |
| Ctrl+N / Ctrl+Shift+N | Nuevo archivo / carpeta |
| Ctrl+A | Seleccionar todo |
| Espacio / Ctrl+Espacio | Marcar / marcar dejando el resto |
| Ctrl+L, F4 | Editar la ruta |
| Ctrl+Z | Deshacer |
| Ctrl+1..9 | Ir al favorito N |

---

## 11. Personalización avanzada (packs)

Naygo guarda su configuración junto al ejecutable (modo portable). Puedes agregar:

- **Idiomas**: suelta un `.json` de traducción en la carpeta `lang/`; aparece en el
  selector. ES y EN vienen de base. Guía paso a paso: [`AGREGAR-IDIOMA.md`](AGREGAR-IDIOMA.md).

**Lanzar desde un acceso directo o terminal:** `naygo.exe "D:\carpeta"` abre Naygo en esa
carpeta; además acepta `--theme <id>` y `--layout <nombre>` (solo para esa sesión) y
`--help`/`--version`. Detalle en el README.
- **Temas**: *color sets* intercambiables en caliente.
- **Sets de íconos**: empaquetados o propios.

Se importan/exportan como packs `.zip` desde *Configuración → Importar/Exportar*.

---

© 2026 **Nicolás Groth / ISGroth** — Licencia MIT.
