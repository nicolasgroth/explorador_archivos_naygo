# Auditoría de paridad Slint vs. contrato (2026-06-16)

> Tras cerrar la migración egui→Slint (F6) y el round de feedback post-VM, Nicolás reportó
> varios huecos. Esta auditoría compara el contrato `CONTRATO-PARIDAD-FUNCIONAL.md` contra el
> código Slint REAL (`crates/ui-slint/`), con evidencia. Hecha con 3 agentes en paralelo +
> verificación manual. La lógica de core/platform está intacta; lo que falta es UI Slint.

## Veredicto

La UI Slint está ~85% de paridad. La mayoría de B/C/D/E/F/K/L está. Lo que **falta o está roto**
se agrupa en: **bugs** (resize, reveal del árbol), **features no portadas** (menú de columna con
filtros, clic derecho en header, reordenar columnas, plantillas de layout, batch-rename), y
**config incompleta** (faltan secciones Avanzado y Acerca-de; pulido de la ventana).

---

## BUGS (rompen uso básico — prioridad máxima)

### B1. Resize de paneles no funciona
- Reportado: seleccionar el borde/splitter no redimensiona.
- Cadena: `app-window.slint` pinta las barras + TouchArea con `split-drag(index, dx, dy)` →
  `main.rs` handler → `set_fraction`. El core (`workspace/layout.rs`: `split_handles`,
  `set_fraction`) está bien y testeado.
- Sospecha: el cálculo de `dx/dy` en el TouchArea (offset de press) o que tras `set_fraction`
  los rects no se repintan. **Diagnosticar la cadena del drag.**

### B2. El árbol no se revela hasta la carpeta activa
- Reportado: con una carpeta ya seleccionada, el árbol no se expande/scrollea hasta ese nodo.
- Contrato D: "reveal (centrar) el nodo objetivo". El core `tree.rs` tiene `reveal_chain`.
- Falta: cablear que al navegar el Files activo, el árbol expanda los ancestros + scroll al nodo.

---

## FEATURES NO PORTADAS A SLINT (la lógica de core existe)

### P1. Menú de columna con filtros (estilo Excel) — clic derecho en header
- Contrato B/G. `core::filter` tiene los 4 filtros (texto+case, extensiones con conteo, rango
  tamaño, rango fecha) + `extension_counts`; `core::columns` persiste filtros; todo testeado.
- Falta en Slint: clic derecho en `HeaderCell` → menú con ordenar asc/desc + filtros + quitar
  filtro + mostrar/ocultar. Hoy solo hay el botón ≡ (mostrar/ocultar) y clic-izq ordena.
- También: indicadores ▲/▼ de orden en el header; embudo en columnas con filtro; aviso "sin
  coincidencias" cuando el filtro vacía la vista.

### P2. Reordenar columnas arrastrando el header
- Contrato B: "arrastrar encabezado reordena (línea de inserción)". `column_move` ya existe en
  el controlador; falta el gesto de arrastre horizontal en el header + la línea de inserción.

### P3. Menú de Layouts (plantillas de distribución de paneles)
- Contrato A/F/I. `core::workspace::template::TemplateStore` tiene 4 builtins (minimalista,
  dual_pane, clásico, power_user) + favoritos + recientes + persistencia. NO hay UI.
- Falta: botón/menú en la toolbar para aplicar una plantilla + guardar la actual + borrar
  personalizada. Controlador: `apply_layout(name)`.

### P4. Batch-rename (renombrado por lotes)
- Contrato J. `core::batch_rename` está 100% (plantilla, comodines, buscar/reemplazar +regex,
  caso, contador, preview con estados Ok/SinCambios/Inválido/Colisión), con tests. NO hay
  ventana en Slint.
- Falta: modal con los campos + preview antes→después + Aplicar (deshab. si hay problemas).

---

## CONFIGURACIÓN INCOMPLETA

Categorías ACTUALES (6): General, Operaciones, Pegado, Apariencia, Atajos, Importar/Exportar.
Faltan respecto al contrato I:

### C1. Sección "Acerca de" (FALTA completa)
- Contrato I: logo, versión, autor (Nicolás Groth / ngroth@gmail.com), empresa (ISGroth),
  licencia MIT, stack, enlace al repo. **Easter egg** (egui: 5 clics en el logo → lluvia de
  carpetas 8s con mensaje letra-a-letra; ver `git show b4069f5~1:crates/ui/src/settings_window/about.rs`).
- Hoy solo hay "Versión: X.Y" en el pie de la ventana.

### C2. Sección "Avanzado" (FALTA, hoy disperso/parcial)
- Falta agrupar/exponer: info del sistema; display del panel de ops (panel/modal/siempre);
  formato de imagen + calidad JPG del pegado; duración de resaltado + agrupar-al-final + caché
  máx dirs del watcher; tray + cerrar-a-bandeja; rendimiento (modo bajo consumo Auto/Siempre/
  Nunca); factory reset (2 pasos). Varios de estos campos ya están en Settings pero sin control.

### C3. Sección "Previsualización" (FALTA): tabla editable de reglas (extensión, on/off, tratar como).

### C4. Plantilla de tabla por defecto (guardar/limpiar) en la sección Paneles.

### C5. Pregunta de bienvenida del primer arranque (modo de consumo). [baja prioridad]

---

## USABILIDAD / PULIDO

### U1. La ventana de config no se puede arrastrar (tarjeta fija 720×520).
### U2. Espacio vacío abajo en categorías con pocos campos (la tarjeta es de alto fijo).
### U3. Combobox de tamaños dispares/desproporcionados (sin ancho fijo; algunos enormes para poco texto).
### U4. Editor de atajos básico: falta búsqueda por acción + indicador de conflicto.

---

## NAVEGACIÓN POR TECLADO (inspiración TC / Directory Opus / XYplorer)

- El keymap tiene 47 acciones; `keys.rs` mapea Slint→KeyCode (las F2..F6 se arreglaron antes;
  verificar que TODAS las del keymap lleguen).
- A revisar/completar como parte del cierre: foco entre paneles (Tab), ←/→ en el árbol,
  typeahead (salto por tipeo, reset ~500ms), atajos para plantillas de layout.
- A futuro (roadmap, acordado con Nicolás): **paleta de comandos (Ctrl+P)** para ejecutar
  cualquier acción por teclado.

---

## PLAN DE CIERRE (orden propuesto)

1. **Bugs** B1 (resize) + B2 (reveal árbol) — rompen el uso básico.
2. **Menú de columna + filtros + clic derecho** (P1) + reordenar arrastrando (P2) — lo de "Excel".
3. **Config**: pulido U1–U3 (arrastrable, alto adaptable, combos parejos) + Acerca-de (C1) +
   Avanzado (C2) + Previsualización (C3) + plantilla de tabla (C4).
4. **Plantillas de layout** (P3) en la toolbar.
5. **Batch-rename** (P4) — ventana modal.
6. **Navegación por teclado** completa + verificación del keymap.
7. (Roadmap) **Paleta de comandos Ctrl+P.**
