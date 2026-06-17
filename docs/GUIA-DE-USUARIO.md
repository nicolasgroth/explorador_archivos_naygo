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

- **Barra de herramientas** (arriba): botones de acciones + la tira de unidades de
  disco (C:, D:, …) + la ruta actual.
- **Paneles**: el área principal. Empieza con un panel de archivos; podés dividir
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

- El botón **＋** divide el panel activo (elegís dirección: derecha/abajo/izq/arriba).
- El botón **Panel ▾** agrega un panel especial (árbol, propiedades, etc.).
- **Arrastrá la barra de título** de un panel sobre otro para reacomodar (los bordes
  dividen; el centro apila como pestaña).
- **Arrastrá las barras divisorias** para redimensionar: mientras arrastrás ves una
  barra-fantasma de acento que marca dónde quedará el borde; al soltar se aplica.
- **Swap / Clonar / Tabs** (toolbar): intercambiar carpetas de dos paneles, abrir la
  carpeta actual en otro panel, o apilar el panel como pestaña sobre otro.

---

## 3. Navegar

- **Doble clic** en una carpeta entra; **Enter** entra a la enfocada.
- **Backspace** o **←** suben un nivel.
- **Alt+←** / **Alt+→**: atrás / adelante en el historial (como un navegador).
- **F5**: refresca (vuelve a leer la carpeta del disco).
- **Tab**: cambia el panel de archivos activo.
- **Tipeo rápido (typeahead):** escribí las primeras letras de un nombre y el foco
  salta a ese ítem. Si hacés una pausa (~½ segundo), empieza una búsqueda nueva.
- **Esc**: cancela un listado en curso (útil en discos de red lentos).

**Barra de ruta (breadcrumbs):** mostrá la ruta como segmentos clicables. Clic en el
hueco vacío la convierte en un editor de texto con autocompletado (Enter navega, Esc
cancela). A la derecha tenés **★** (anclar a favoritos) y **📋** (copiar la ruta).

**Unidades de disco:** la tira C:/D:/… de la toolbar navega el panel activo a esa
unidad.

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
menú de carpeta) abre un cuadro donde podés crear **varias a la vez**: una carpeta
por línea, y usando `\` se anidan (p. ej. `proyecto\src\bin` crea las tres). Las
líneas inválidas se avisan y se omiten; las válidas se crean.

---

## 6. Columnas (estilo Excel)

El encabezado de la tabla:

- **Clic** en una columna ordena por ella; otro clic invierte (▲/▼).
- **Arrastrá el borde derecho** de una columna para cambiar su ancho.
- **Botón ≡** (al final del encabezado): mostrar/ocultar columnas.
- **Clic derecho** en una columna abre su menú: **ordenar** asc/desc, **filtrar…**,
  **quitar filtro**, **mover ←/→**, **ocultar**.

**Filtros por columna:**

- **Nombre**: contiene un texto (con opción de distinguir mayúsculas).
- **Extensión**: marcás los tipos a mostrar (con su conteo).
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
- **Apariencia**: el **tema** se elige en una galería de tarjetas (cada una muestra
  sus colores; la activa lleva borde de acento y ★). Hay temas claros y oscuros
  (Citrus Glow, Neon Retro, Ocean Midnight, Ember Forge, Polar Graphite, y los
  clásicos). También: idioma, set de íconos, formato de fecha y tamaño, densidad de
  fila. Todo se aplica en caliente.
- **Atajos**: editor de atajos por acción (cambiar / restablecer / restablecer todo),
  con detección de conflictos.
- **Importar/Exportar**: packs `.zip` de idioma, tema o configuración.
- **Avanzado**: progreso de operaciones (panel/modal/siempre), formato de imagen
  pegada, modo de bajo consumo, archivos nuevos al final, bandeja del sistema,
  cerrar-a-bandeja, y **Restablecer todo** (en dos pasos).
- **Acerca de**: autoría, licencia, stack, enlace al repo (y un pequeño easter egg).

---

## 10. Atajos de teclado (resumen)

Todos son configurables en *Configuración → Atajos*. Por defecto:

| Tecla | Acción |
|-------|--------|
| ↑ / ↓ | Mover foco |
| Enter | Abrir / entrar |
| Backspace, ← | Subir un nivel |
| Alt+← / Alt+→ | Atrás / adelante |
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

Naygo guarda su configuración junto al ejecutable (modo portable). Podés agregar:

- **Idiomas**: soltá un `.json` de traducción; aparece en el selector. ES y EN vienen
  de base.
- **Temas**: *color sets* intercambiables en caliente.
- **Sets de íconos**: empaquetados o propios.

Se importan/exportan como packs `.zip` desde *Configuración → Importar/Exportar*.

---

© 2026 **Nicolás Groth / ISGroth** — Licencia MIT.
