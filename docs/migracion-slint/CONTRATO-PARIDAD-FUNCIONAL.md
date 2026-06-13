# Contrato de paridad funcional — migración de UI a Slint (2026-06-13)

> Naygo migra su capa de presentación de **egui** a **Slint** (modo retenido +
> renderizador por software) porque egui rasteriza la ventana entera por CPU sin GPU,
> lo que hace inviable el uso en VMs/equipos sin tarjeta gráfica. **Medido en la VM de
> Nicolás: egui ~79% CPU al mover el mouse; el prototipo Slint, 3.7% máximo.** ~20× menos.
>
> **Regla de oro de la migración:** el producto final NO pierde NINGUNA funcionalidad ni
> característica de las que existen hoy. Este documento es la checklist oficial. Una
> sección no se marca completa hasta que su comportamiento de cara al usuario funcione en
> Slint igual o mejor que en egui.

## Qué se conserva sin tocar (NO se reescribe)

Toda la lógica vive fuera de egui y se reusa tal cual:
- **`naygo-core`** (~11.7k líneas, libre de egui): listing, listing_cache, ops (+undo/
  journal), sizing, batch_rename, clipboard, columns, filter, sort, fs_model, keymap,
  i18n, theme, config, favorites, recent_dirs, preview, render_hint, workspace +
  view-cache. **Todos sus tests siguen pasando.**
- **`naygo-platform`** (~2.3k líneas, libre de egui): dnd (OLE), shell (menú nativo),
  papelera, open (ShellExecute), drives, disk, dir_watch, device_watch, autostart,
  locale, icons del shell.

Lo único que se reescribe: **`naygo-ui`** (la capa egui). Se crea una nueva capa Slint
que consume el mismo core/platform por las mismas interfaces (PaneRequest, TableAction,
TreeAction, Action del keymap, OpRequest, etc.).

---

## CHECKLIST DE PARIDAD (contrato)

### A. Toolbar
- [ ] Posición configurable: Top (horizontal) / Side (vertical).
- [ ] Atrás / Adelante (deshabilitados según historial); clic derecho → menú con el
      historial del panel (saltar a cualquier paso; actual marcado).
- [ ] Arriba (carpeta padre). Refrescar (re-listar la carpeta activa).
- [ ] Multi-panel: Intercambiar (⇄, deshab. con <2 paneles Files), Clonar ruta (⎘).
- [ ] Ops: Copiar, Cortar, Pegar, Eliminar, Nuevo archivo, Nueva carpeta.
- [ ] Menú Layouts (plantillas). Agregar panel (+). Menú ▾ otros paneles
      (Historial/Árbol/Inspector/Favoritos/Preview).
- [ ] Strip de unidades (C:/D:/…): clic navega el panel activo a la raíz; tooltip ruta.
- [ ] Nueva ventana (lanza otro proceso de Naygo).
- [ ] Ajustes (⚙). Estilo de íconos Glifos/Pack; color de glifo (tema o custom);
      "solo íconos". Tooltips i18n en todo.

### B. Panel de archivos (tabla)
- [ ] Tabla virtualizada (solo pinta filas visibles), filas ~20px.
- [ ] Columnas: Nombre (obligatoria), Extensión, Tamaño (sufijo "~" si parcial),
      Modificado, Creado. Orden/visibilidad/ancho por panel.
- [ ] Encabezado: indicadores ▲/▼ (orden) y ≡ (filtro); botón ▾ menú de columna;
      clic derecho abre el mismo menú; **arrastrar encabezado reordena columnas** (línea
      de inserción).
- [ ] Fila ".." (subir al padre) si hay padre y la opción está activa.
- [ ] Selección: clic (única), Ctrl+clic (toggle), Shift+clic (rango desde ancla);
      multi-selección pintada; fila con foco con borde punteado.
- [ ] Doble clic: carpeta navega, archivo abre con app por defecto; **Ctrl+doble-clic en
      carpeta abre en OTRO panel** (selector 1..9 si hay 3+ paneles).
- [ ] Rubber-band (selección por rectángulo) desde zona vacía; Ctrl+arrastre aditivo.
- [ ] Arrastre OLE de archivos hacia Explorer/escritorio/otro panel.
- [ ] Menú contextual: Abrir, Abrir con…, Copiar, Cortar, Pegar, Renombrar, Eliminar,
      "Más opciones de Windows…" (menú nativo del Shell).
- [ ] Rename inline (F2) con ciclo de selección (nombre/ext/todo), viajar con ↑/↓,
      Esc cancela, Enter/blur confirma, validación de nombre.
- [ ] Resaltado de archivos nuevos (watcher); modo "agrupar al final"; duración
      configurable.
- [ ] Aviso "sin coincidencias" cuando un filtro deja la vista vacía.
- [ ] Ancho de columnas: modo Fijo (resizable) / Auto (reparte por contenido).

### C. Path-bar
- [ ] Breadcrumbs clicables (cada segmento navega); separador ›; zona vacía clicable.
- [ ] Íconos a la derecha SIEMPRE visibles (no tapados por rutas largas): 📋 copiar ruta,
      ☆/★ favorito (toggle).
- [ ] Modo edición (Ctrl+L / F4 / clic en zona vacía): TextEdit con ruta preseleccionada;
      autocompletado de subcarpetas (popup, ★ débil si reciente); Enter navega/valida,
      Esc cancela, Tab completa.

### D. Árbol de carpetas
- [ ] Raíces = unidades de disco; nodos = solo carpetas; expand/colapsa (▶/▼) lazy.
- [ ] Nodo activo resaltado (fondo + barra de acento). Estados: cargando/vacío/error.
- [ ] Clic en ícono/nombre navega el panel activo; clic en ▶/▼ expande/colapsa.
- [ ] Barra de uso de disco por unidad (verde/naranja/rojo según %); bloque clicable.
- [ ] Favoritos anclados arriba del árbol. Watcher refleja cambios del FS.
- [ ] Scroll bidireccional; reveal (centrar) el nodo objetivo.

### E. Paneles especiales
- [ ] Historial (undo): lista de ops (más nueva arriba), "Deshacer" / "Deshacer hasta
      aquí", estado inválido con tooltip, marca "deshecho".
- [ ] Favoritos: lista ordenada (1..9 = Ctrl+1..9), clic navega, clic derecho → quitar;
      sección Recientes debajo.
- [ ] Inspector: nombre, tipo, ruta, tamaño del ítem enfocado del panel activo.
- [ ] Preview: texto (monospace, truncado con aviso), imagen (proporcional), mensaje
      "sin vista previa"; async, no bloquea; reglas por extensión + alias.

### F. Docking / layout
- [ ] Múltiples paneles, apilables en tabs, separadores arrastrables (resize).
- [ ] Agregar panel = dividir el leaf enfocado (split 50%), no apilar.
- [ ] Tab activo resaltado; título dinámico (carpeta para Files, i18n para el resto).
- [ ] Cerrar tab; layout persiste entre reinicios (workspace.json).

### G. Menú de columna
- [ ] Ordenar asc/desc. Filtrar (Texto contiene + case; Extensiones con conteo;
      rango de Tamaño; rango de Fecha). Quitar filtro. Mostrar/ocultar columnas.

### H. Atajos de teclado (47 acciones, configurables)
- [ ] Todos los defaults del keymap (navegación, foco, selección por bloques —
      AvPag/RePag/Inicio/Fin/Shift/Ctrl—, ops, favoritos Ctrl+1..9, EditPath).
- [ ] Typeahead (salto por tipeo, reset ~500ms). Botones laterales del mouse (atrás/
      adelante).
- [ ] Editor de atajos en Configuración: búsqueda, capturar/quitar chord, reset por
      acción y global, indicador de conflicto.

### I. Configuración (todas las secciones)
- [ ] Apariencia: set de íconos (+ vista previa), estilo toolbar (glifos/pack), color de
      glifo, tarjetas de tema, packs, "solo íconos".
- [ ] Paneles: mostrar "..", posición de barra, ancho de columnas, guardar/limpiar
      plantilla de tabla por defecto.
- [ ] Previsualización: tabla editable de reglas (extensión, on/off, tratar como).
- [ ] Atajos (editor, ver H).
- [ ] Idioma: lista de idiomas disponibles, hot-swap.
- [ ] Avanzado: info del sistema; ops (modo cola/paralelo, display panel/modal/siempre,
      confirmar papelera, mostrar resumen); pegado (confirmar, plantillas de nombre/ext,
      formato imagen, calidad JPG); watcher (duración resaltado, agrupar al final, tamaño
      sin subdirs, caché máx dirs); sistema (tray, cerrar a bandeja, inicio con Windows);
      rendimiento (modo bajo consumo Auto/Siempre/Nunca); factory reset (2 pasos).
- [ ] Acerca de: logo, versión, autor, licencia, stack, enlace al repo. (Easter egg:
      opcional, baja prioridad.)
- [ ] Pregunta de bienvenida del primer arranque (modo de consumo).

### J. Diálogos y progreso
- [ ] Confirmar borrado (papelera/permanente, N ítems). Conflicto (Sobrescribir/Saltar/
      Renombrar/Cancelar). Entrada de nombre (crear/renombrar, validación). Preview de
      pegado (modo B: nombre + formato imagen). Retomar ops interrumpidas.
- [ ] Panel de progreso de operaciones: por op (etiqueta, barra %, cancelar, en cola);
      detalle expandible (archivos, bytes, archivo actual); resumen (hechos/saltados/
      errores) + exportar. Modo Panel/Modal/Siempre visible.
- [ ] Batch-rename: plantilla + comodines (ayuda plegable), buscar/reemplazar (+regex),
      caso (None/Lower/Upper/Title), contador (inicio/paso), preview antes→después con
      estados (Ok/Sin cambios/Inválido/Colisión), Aplicar (deshab. si hay problemas).

### K. Plataforma e integraciones del SO
- [ ] Drag&drop OLE (sacar y recibir archivos). Menú contextual nativo del Shell.
      Papelera (recuperable). Abrir con app por defecto / "Abrir con…". Detección de
      unidades + espacio. Watcher de carpeta + de dispositivos (USB). Autostart. Locale.

### L. Producto / distribución
- [ ] Splash de arranque (release). Bandeja del sistema (ícono + menú Abrir/Salir).
- [ ] Persistencia: settings/workspace/templates/keybindings/recents/favorites + journal.
- [ ] i18n ES+EN en paridad; temas embebidos + cargables.
- [ ] Build release + portable + instalador (Inno Setup), CRT estático, marca de autoría.

---

## Método de migración (propuesto, a planificar)

La migración es grande (~11k líneas de UI). Se hará **por fases verticales**, cada una
dejando la app funcional y medible, NO un big-bang. Orden propuesto (a refinar en el
spec/plan de cada fase):

1. **Esqueleto Slint**: ventana + un panel de archivos navegable (tabla, columnas,
   selección, navegación) consumiendo core. Reemplaza el `main` para arrancar Slint.
2. **Paneles + docking**: multi-panel, tabs, splits, los paneles especiales.
3. **Operaciones + diálogos**: copiar/mover/borrar/rename/pegar + sus modales + progreso.
4. **Configuración completa** (todas las secciones) + atajos.
5. **Integraciones**: OLE, menú nativo, papelera, watcher, tray, splash.
6. **Pulido + distribución**: temas, i18n, release/instalador, verificación de paridad
   completa contra esta checklist en la VM.

Cada fase: spec corto → plan → ejecución con puertas (tests core intactos, build, clippy,
fmt) → verificación en vivo de Nicolás (incluida la medición de CPU en la VM).

**Decisión pendiente con Nicolás:** ¿hacemos la migración fase por fase reemplazando
`naygo-ui`, o construimos la nueva UI en paralelo (`naygo-ui-slint`) y cambiamos el
binario al final? (afecta cómo conviven ambas durante la transición).
