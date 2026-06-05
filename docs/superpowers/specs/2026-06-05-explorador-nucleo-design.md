# Explorador de archivos — Diseño del núcleo (Build 1)

> Spec de diseño. Autoría: Nicolás Groth / ISGroth. Licencia: MIT.
> Fecha: 2026-06-05. Estado: aprobado, listo para escribir plan de implementación.

---

## 1. Visión y alcance

Explorador de archivos para **Windows 10/11**, estilo **Commander** (inspirado en
Directory Opus), construido en **Rust + egui**, **100% open source bajo licencia
MIT**. Autoría de **Nicolás Groth / ISGroth**.

**Prioridad absoluta:** velocidad de navegación y bajo consumo. Lo visual es
importante pero **subordinado a la velocidad**.

**Filosofía de alcance:** hace bien una cosa — navegar, ver y operar archivos
rápido. **No** reproduce media, no es editor, no hace de todo. Abre el archivo con
su programa por defecto cuando el usuario lo pide.

### Estrategia de construcción — faseado

- **Build 1 (NÚCLEO):** lo que detalla este documento. Será el primer
  spec → plan → implementación.
- **Capas posteriores** (cada una con su propio brainstorm → spec → build):
  miniaturas, visor de contenido (imágenes/texto/PDF), comprimir/descomprimir,
  batch-rename avanzado (patrones/regex), caché de carpetas visitadas, biblioteca
  ampliada de temas/idiomas, paleta de comandos (Ctrl+P), personalización fina de
  toolbar, animaciones de íconos.

### Núcleo (Build 1) incluye

- Layout de **paneles dinámicos dockables** (árbol / listas / inspector
  reacomodables, layouts guardables).
- Motor de listado **streaming incremental** (nunca congela).
- Vistas: **detalle (columnas), lista compacta, íconos/mosaico** — con "hueco"
  arquitectónico para miniaturas futuras.
- **Inspector de metadatos/propiedades** del archivo seleccionado.
- Operaciones: copiar, mover, eliminar (a Papelera), renombrar, crear
  carpeta/archivo, **selección múltiple**, **drag interno** entre paneles,
  **drag&drop con el SO** (escritorio y otras apps).
- **Espacio libre de disco** siempre visible; **tamaño de carpeta** bajo demanda
  async.
- Abrir archivo con programa por defecto.
- **Infraestructura de i18n** (ES + EN incluidos) y **temas + color sets**
  intercambiables en caliente (2-3 presets).
- Navegación 100% por teclado, atajos **configurables**, default estilo Windows.
- **Interfaz rica en íconos**: toolbar con accesos rápidos directos, breadcrumbs,
  íconos de tipo en listas, íconos de unidad de disco.
- **Cancelación universal**: toda operación larga es cancelable por el usuario.

---

## 2. Arquitectura técnica

Idea rectora: **separar el "qué hace" (lógica de filesystem, pura y testeable) del
"cómo se ve" (egui), y nunca bloquear el hilo de UI.** Módulos de responsabilidad
única que se comunican por mensajes/canales.

### Capa 1 — `core` (lógica pura, sin UI, sin Win32, testeable al 100%)

- **`fs_model`**: tipos POCO — `Entry` (archivo/carpeta: nombre, tamaño, fechas,
  atributos), `PaneState`, `SortSpec`, `ViewMode`.
- **`listing`**: motor de **streaming incremental**. Corre en worker, lee el
  directorio, emite `Entry`s por canal a la UI. **Cancelable** (cambiar de carpeta
  o `Esc` aborta).
- **`ops`**: copiar / mover / eliminar / renombrar / crear como operaciones async
  con reporte de progreso por canal. Lógica de conflictos pura. **Cancelable**.
- **`sizing`**: cálculo recursivo de tamaño de carpeta bajo demanda, con progreso.
  **Cancelable**.
- **`i18n`**: catálogo de strings por clave, carga de archivos de idioma (formato
  simple JSON/FTL), cambio en caliente.
- **`theme`**: definición de tema + color set (colores/espaciados serializables),
  presets, cambio en caliente.
- **`config`**: persistencia JSON portable (al lado del `.exe`): layouts de
  paneles, idioma, tema, columnas, mapa de teclas.

### Capa 2 — `platform` (todo lo que toca Windows, aislado; único módulo "sucio")

- **`shell`**: extracción de íconos (`SHGetFileInfo`), abrir con programa default
  (`ShellExecute`), papelera (`IFileOperation` / `SHFileOperation`), menú
  contextual nativo, info de discos (espacio libre).
- **`dnd`**: módulo COM/OLE de drag&drop con el SO (`IDataObject`, `IDropSource`,
  `IDropTarget`, `DoDragDrop`, `CF_HDROP`). El más espinoso, **aislado aquí** tras
  una interfaz limpia. Implementado como **uno de los últimos pasos del núcleo**
  por su riesgo.
- **`watcher`** (futuro): invalidación de caché. Hueco previsto.

### Capa 3 — `ui` (egui, sin lógica de negocio)

- **`app`**: estado raíz, loop de egui, despacho de mensajes de los workers.
- **`docking`**: integración de `egui_dock` para paneles dinámicos.
- Paneles: `tree_panel`, `file_panel` (las 3 vistas), `inspector_panel`,
  `disk_panel`.
- **`theme_apply`**: traduce `theme`/color set del core a estilos de egui.
- **`input`**: mapa de atajos configurable.
- **`icons`**: carga set monocromo temable (Lucide/Tabler) + set multicolor
  (Fluent Emoji / Flat Color Icons), expuestos por nombre.
- **`progress`**: cola de operaciones en progreso, cada tarea con botón Cancelar.

### Flujo de datos clave (cómo se siente instantáneo)

UI manda "lista esta carpeta" → `listing` corre en worker → emite entries por
canal → UI los va pintando frame a frame sin bloquear. Idéntico patrón para `ops`
y `sizing`. **El hilo de UI nunca hace I/O de disco.**

### Principio de aislamiento

- `core` no depende de egui ni de Windows → testeable con cualquier carpeta de
  prueba.
- `platform` esconde Win32 tras interfaces simples → portar a otro SO = cambiar
  solo `platform`.
- `ui` no sabe leer un disco, solo pinta lo que el core le manda.

---

## 3. Modelo de interacción y atajos de teclado

Filosofía: **todo lo frecuente tiene un atajo, y los atajos son configurables**
(el mapa de teclas vive en `config`). **Default estilo Windows**; esquema Commander
clásico (F5=copiar, F6=mover) disponible como **preset alternativo**.

### Navegación
- `↑ ↓` mover selección; `Enter` entrar / abrir con default; `Backspace` o `←`
  subir un nivel.
- `Tab` cambia el panel de archivos activo.
- `Ctrl+L` / `Alt+D` editar la ruta (barra de direcciones tipeable).
- Type-ahead: tipear salta al archivo que empieza así.
- `Esc` cancela el listado en curso.

### Selección
- `Ctrl+clic` toggle; `Shift+clic` / `Shift+↑↓` rango; recuadro de arrastre;
  `Ctrl+A` todo; `Space` marcar (estilo Commander).

### Operaciones (default Windows)
- `F2` renombrar; `Del` a Papelera; `Shift+Del` permanente (con confirmación).
- `Ctrl+C / Ctrl+X / Ctrl+V` portapapeles del Shell (interop con Explorer).
- Nueva carpeta / nuevo archivo desde toolbar y menú contextual.

### Vista y app
- `Ctrl+1/2/3` cambiar vista (detalle / lista / íconos).
- `F3` calcular tamaño de carpeta.
- `Ctrl+,` settings; atajos para cambiar tema/color set e idioma en caliente.

### Hueco previsto (fase posterior)
- Paleta de comandos `Ctrl+P`. La arquitectura de `input` lo deja fácil.

---

## 4. Interfaz rica en íconos

- **Toolbar superior con íconos**: atrás/adelante/arriba, refrescar, nueva
  carpeta, copiar/mover/eliminar, cambiar vista, calcular tamaño, etc. Cada acción
  frecuente = botón-ícono con tooltip (respeta i18n).
- **Toolbar arquitectónicamente configurable** (personalización fina como pulido
  posterior; el núcleo trae un set sensato).
- **Íconos con sentido en todos lados**: breadcrumbs clicables en barra de
  direcciones, panel de discos con íconos de unidad + barra de espacio, íconos de
  tipo de archivo en listas (vía Shell), íconos en menú contextual.

### Estrategia de íconos — híbrido por zona

- **Monocromo temable** (Lucide o Tabler, ISC/MIT): toolbar, botones de acción,
  breadcrumbs, controles. **Se recolorean con el color set activo.**
- **Multicolor fijo** (Fluent Emoji de Microsoft o Flat Color Icons, libres):
  unidades de disco, tipos de archivo "hero", favoritos/accesos rápidos. Set
  elegido para verse bien en tema claro **y** oscuro.
- **Ganchos de animación** previstos (hover, estados); animaciones reales como
  pulido posterior. No detallar micro-animaciones antes de que navegar funcione.
- Todos los sets son de **libre uso real** (MIT/ISC/Apache/CC0), compatibles con
  la licencia MIT del proyecto.

---

## 5. Cancelación universal (principio transversal)

**Toda operación que pueda tomar tiempo es cancelable por el usuario, siempre.**
Encaja naturalmente con los workers async.

### Mecanismo (uniforme)
- Cada operación larga recibe un **token de cancelación** (`AtomicBool` compartido
  o canal). El worker lo chequea **entre cada paso** (cada archivo copiado, cada
  entrada leída) y **aborta limpio** si está activado.
- "Limpio" significa:
  - **Copia** cancelada → borra el archivo parcial que estaba escribiendo (no deja
    corrupto a medias).
  - **Listado** cancelado → para de emitir entries, descarta el worker.
  - **Cálculo de tamaño** cancelado → para de recorrer.

### Experiencia de usuario
- **Listado lento** (red / 100k archivos): streaming muestra en vivo; navegar a
  otra carpeta o `Esc` cancela al instante.
- **Operaciones de archivo**: panel/diálogo de progreso no intrusivo con barra,
  archivo actual, botón **Cancelar** + tecla `Esc`. Varias operaciones a la vez =
  cola de progreso, cada una cancelable por separado.
- Cancelar se siente instantáneo (corta en el siguiente chequeo).

### En el spec
- `core`: cada función de `listing`, `ops`, `sizing` recibe `CancellationToken` y
  lo respeta. **Testeable**: lanzar operación, cancelar, verificar que paró y no
  dejó basura.
- `ui`: módulo `progress` con cola de tareas activas, cada una con botón Cancelar.

---

## 6. Manejo de errores y testing

### Errores — principio: el filesystem es hostil, la app nunca cae por eso

Discos de red que se caen, permisos denegados, archivos bloqueados, rutas que
desaparecen a mitad de operación son **normales**, no excepcionales.

- **Recuperables → no crashean, se comunican.** `Result` tipado en cada operación
  de `core`/`platform`. La UI muestra el error de forma discreta (barra/toast no
  intrusivo) y sigue viva.
- **Conflictos en operaciones largas** (archivo ya existe, permiso denegado a
  mitad): diálogo sobreescribir / saltar / renombrar / cancelar, con "aplicar a
  todos".
- **Timeouts en I/O de red** (referencia: los 500 ms de `SHGetFileInfo` en
  WinShelf): una unidad caída no congela el listado ni el cálculo de tamaño; se
  cancela limpio.
- **Logging básico a archivo** desde el día uno (al lado del `.exe` o
  `%LOCALAPPDATA%`). **Sin telemetría.**
- **Panic handler**: captura, loguea y muestra mensaje en vez de morir en
  silencio.

### Testing — la razón de que `core` sea puro

- **`core` se testea al 100% sin UI ni Windows**: motor de listado, sort,
  conflictos de operaciones, cálculo de tamaño, cancelación, i18n, parsing de
  config y temas. Tests unitarios sobre carpetas/datos de prueba. **Mayor ganancia
  de la arquitectura de 3 capas.**
- **`platform`** (Win32): tests donde se pueda, mocks en interfaces donde no. El
  COM de drag&drop se valida más manualmente.
- **`ui`**: lógica mínima; lo que tenga lógica (mapeo de teclas, estado de
  selección) se extrae a funciones puras testeables.
- **Meta**: build limpio + tests pasando antes de cada commit.

---

## 7. Dependencias previstas (todas libres, MIT/Apache/ISC/CC0)

- `eframe` / `egui` — UI inmediata GPU.
- `egui_dock` — paneles dinámicos dockables.
- `windows` (crate oficial de Microsoft) — acceso a Shell32 / COM / OLE.
- `serde` + `serde_json` — config y temas.
- Crate de logging (p.ej. `tracing` + sink a archivo).
- Set de íconos: Lucide/Tabler (monocromo) + Fluent Emoji / Flat Color Icons
  (multicolor), embebidos como assets.

> A confirmar versiones y crates exactos en el plan de implementación.

---

## 8. Estructura de proyecto propuesta

```
explorador_de_archivos/
├── Cargo.toml                 # workspace o crate único (decidir en el plan)
├── src/
│   ├── main.rs
│   ├── core/                  # lógica pura, testeable
│   ├── platform/              # Win32/COM aislado
│   └── ui/                    # egui
├── assets/
│   ├── icons/                 # sets de íconos libres
│   ├── lang/                  # es.json, en.json, ...
│   └── themes/                # presets de color set
├── docs/
│   └── superpowers/specs/
├── tests/
├── LICENSE                    # MIT, Nicolás Groth / ISGroth
├── CLAUDE.md
└── README.md
```

---

## Fuera de alcance (recordatorio explícito — NO en Build 1)

Miniaturas reales, visor de contenido (imágenes/texto/PDF), comprimir/descomprimir,
batch-rename avanzado, caché de carpetas visitadas, paleta de comandos, animaciones
de íconos, personalización fina de toolbar, reproducción de media (nunca),
edición de archivos (nunca).
