# Backlog — pendientes y mejoras de Naygo

> Documento de arranque para nuevas sesiones. Todo lo acordado que aún no se implementa,
> con contexto y punteros al código relevante. Actualizarlo al cerrar cada ítem.
> Última actualización: 2026-09-02.

## Estado de partida

- Versión en curso: **0.4.0+** (0.4.0 publicada 2026-08-16; hay trabajo sin publicar en el
  working tree: buscador F3 ampliado, carpetas conocidas de Windows en el árbol, y más).
- Suite: **1.100 tests verdes** (más 6 smoke tests de Windows ignorados por requerir interacción).
  Clippy: cero warnings en todo el workspace y todos los targets.
- i18n: 10 idiomas con paridad verificada por `scripts/check_i18n_lang.py` (en CI).
- **Fix ya aplicado en el working tree (incluir en el próximo commit):** las 7 claves
  `slint.preview.{loading,cancel,cancel_tip,copy_tip,wrap_on_tip,wrap_off_tip,reset_view_tip}`
  se agregaron a los 8 idiomas que las perdían (pt/de/fr/hi/it/ja/ko/zh). Sin esto, el test
  `i18n::tests::todos_los_idiomas_tienen_las_claves_de_es` falla.
- **Cierre operativo 2026-08-28:** `scripts/build-release.ps1` ya tiene fallback SHA-256 con
  .NET para runtimes PowerShell sin `Get-FileHash`. El release del 2026-08-22 sí produjo ZIP e
  instalador; antes de este fix el script fallaba solo al escribir `SHA256SUMS.txt`, después de
  terminar ambos artefactos.
- **Cierre técnico 2026-08-28 (T-1):** `ops_ctrl` ahora es un módulo con raíz en
  `ops_ctrl/mod.rs`; los workers de archivo comprimido, borrado por lotes/Papelera y recuperación
  de journal viven respectivamente en `archive.rs`, `batch_delete.rs` y `resume.rs`. El wizard de
  sincronización y la bandeja ya estaban correctamente aislados en `workspace_ctrl/`.
- **Cierre técnico 2026-08-28 (T-2):** `scripts/smoke-ui.ps1` ejecuta los drivers interactivos de
  editor de ruta y scroll, exige capturas y rechaza panics nuevos en el log. Quedó documentado como
  gate manual post-build porque los runners alojados no ofrecen un escritorio Windows fiable.
- **Cierre técnico 2026-08-28 (T-3):** el ZIP portable ya no incluye `naygo.pdb` (bajó de ~70 MB a
  ~11 MB). El instalador ofrece los símbolos como tarea opcional y desmarcada; el PDB se sigue
  generando junto al binario para diagnósticos.
- **Cierre funcional 2026-08-29 (C-2):** el diálogo «nueva carpeta» mantiene `Enter` como salto
  de línea y confirma con dos `Enter` consecutivos dentro de 500 ms; `Ctrl+Enter` continúa como
  confirmación inmediata.
- **Cierre funcional 2026-08-29 (F-3):** la mini-barra del filtro por tipeo ahora alterna entre
  resaltar coincidencias y mostrar solo estas. El filtro actúa sobre la vista real del panel,
  conserva foco/selección por ruta y no se persiste ni altera los filtros por columna.
- **Cierre funcional 2026-08-29 (F-2):** `Ctrl+D` duplica la selección en su misma carpeta con
  nombres seguros (` - copia`, ` - copia (2)`, …), planificación asíncrona, journal y deshacer.
  Las configuraciones heredadas liberan el antiguo `Ctrl+D` de Favoritos solo cuando conservan
  exactamente ese valor histórico; los atajos personalizados distintos se respetan.
- **Cierre funcional 2026-08-29 (C-1):** `Ctrl+N` crea carpeta y `Ctrl+Shift+N` crea archivo.
  La carga del keymap migra únicamente el par de defaults anterior, sin alterar reasignaciones.
- **Corrección 2026-09-01 (C-1):** `Ctrl+N` vuelve a abrir el editor multilínea de
  «Nueva(s) carpeta(s)», idéntico al botón y al menú contextual. Así el estado vacío explica que
  falta una carpeta en vez de marcar un nombre inexistente como inválido, y se recupera la
  creación simultánea (incluidas rutas anidadas con `\\`).
- **Corrección 2026-09-01 (Papelera + preview):** la capa Shell de Papelera ahora valida el
  resultado de cada `DeleteItem` y que las rutas hayan desaparecido; si `IFileOperation` informa
  éxito sin retirar el archivo, reintenta mediante `SHFileOperationW` y nunca muestra una
  operación falsa como completada. El preview de texto reinicia viewport, selección y caret cada
  vez que llega contenido nuevo, por lo que abre desde el inicio y no mantiene una selección
  invisible del archivo anterior.
- **Ajuste 2026-09-01 (Papelera + preview):** el callback de resultado se corrigió a
  `PostDeleteItem` (no `PostMoveItem`) y un rechazo ahora queda registrado por archivo como
  fallido, en vez de aparecer engañosamente como «hecho: 0». Para lectura, la vista previa pasa
  de 100 a 250 líneas dentro del mismo máximo de 64 KiB y remata el contenido recortado con un
  borde vectorial de papel rasgado; ambos cambios mantienen el costo de memoria acotado.
- **Ajuste 2026-09-01 (Papelera + preview, segunda validación):** las rutas internas de Naygo
  del tipo `D:carpeta\\archivo` son válidas para `std::fs`, pero Shell las rechaza como
  `E_INVALIDARG`; antes de invocar la Papelera se resuelven ahora a rutas físicas con formato
  Shell (sin el prefijo extendido `\\?\\`). El preview usa un único editor multilínea de sólo
  lectura también para XML/código, con viewport explícito: permite rueda, barra de scroll,
  selección parcial y `Ctrl+C` en cualquier texto que se despliegue.
- **Cierre funcional 2026-08-31 (F-4):** `Ctrl+Shift+V` abre el historial interno de los últimos
  10 conjuntos copiados/cortados dentro de Naygo. Es efímero, deduplica entradas, no guarda rutas
  privadas en disco y pega mediante el mismo motor cancelable de operaciones.
- **Cierre funcional 2026-08-31 (F-1):** el menú contextual exporta la selección —o toda la vista
  filtrada si no hay selección— al portapapeles o a CSV. Respeta columnas visibles y su orden,
  agrega ruta completa, permite separador `;` o `,` y escribe BOM UTF-8 en archivos para Excel.

---

## A. Mejoras técnicas pendientes (importante aplicarlas)

### T-4. Firma de código del instalador
**Preparación completada; activación externa pendiente.** `scripts/build-release.ps1` ya acepta
`SignToolPath` + `CertificateThumbprint` (o sus variables de entorno), firma el `setup.exe` con
SHA-256, timestamp y luego ejecuta `signtool verify`. Falta que Nicolás/ISGroth cree y autorice
un certificado (p. ej. SignPath.io) y entregue la identidad de firma; no se puede fabricar ni
usar una sin esa autorización. Mientras tanto, el bypass de SmartScreen está documentado en
`docs/DISTRIBUTION.md`.

---

## B. Funcionalidades aprobadas para implementar

### F-5. Historial de navegación con memoria de contexto («Atrás de verdad»)
Al volver Atrás/Adelante, restaurar no solo la ruta sino también el archivo enfocado, selección,
posición de scroll y filtros activos de esa visita. El objetivo es retomar exactamente donde se
estaba sin reconstruir visualmente el contexto. Extender `core::workspace::NavHistory` con una
entrada de navegación compacta; no debe persistir listados ni provocar I/O adicional. Definir qué
parte del contexto sobrevive al reinicio y cómo degradar si los archivos ya no existen.

- **Cierre funcional 2026-08-31:** cada entrada conserva durante la sesión foco, selección,
  primera fila visible, filtros de columna y filtro visual. Se referencia por ruta (no por índice):
  al volver se reubica tras ordenar y se omiten archivos ausentes. Tras reiniciar solo sobrevive
  el estado de tabla/ruta ya persistido; no se guardan listados ni contexto efímero adicional.

### F-6. Radar de destinos para copiar/mover
Overlay navegable íntegramente por teclado que reúna paneles abiertos numerados, favoritos,
carpetas recientes/frecuentes y últimos destinos de operaciones. Elegir un destino con una tecla
y ejecutar copiar/mover sin navegar primero hasta él. Reutilizar el selector numérico de paneles,
`RecentDirs`, Favoritos y el historial de operaciones; el ranking debe ser local, determinista y
sin escaneo ni servicio residente.

- **Cierre funcional 2026-08-31:** F5/F6 (o las acciones configuradas de copiar/mover al otro
  panel) muestran un radar con hasta nueve destinos. Prioriza paneles abiertos, últimos destinos
  de operación, favoritos, frecuentes y recientes; deduplica rutas sin consultar disco. Se elige
  con 1–9, clic o Escape y reutiliza el motor cancelable de operaciones.

### F-7. Vista «qué cambió desde mi última visita»
Comparar el último listado completo conservado en memoria con el listado actual al volver a una
carpeta y marcar elementos nuevos, modificados y desaparecidos. Primera versión solo durante la
sesión, usando metadatos ya listados (nombre, tamaño y fecha), sin indexador ni vigilancia global.
Los desaparecidos se presentan como información, no como filas operables. Evaluar persistencia
opt-in únicamente después de medir memoria y utilidad real.

- **Cierre funcional 2026-08-31:** Naygo conserva el último snapshot completo de cada carpeta
  solo en memoria y compara nombre, tamaño, fecha y tipo al volver/listar otra vez. Las filas
  nuevas o modificadas se resaltan; el footer indica `+n ~m -d`, donde los ausentes (`-d`) son
  únicamente información y nunca se convierten en filas operables. No hay indexador, watcher
  global ni persistencia entre reinicios.

### F-8. Comparación entre paneles accionable
Extender la comparación rápida actual con acciones para ocultar iguales, seleccionar «solo aquí»
o «distintos» y copiar/mover ese subconjunto al otro panel. Agregar navegación relativa enlazada
opcional entre dos raíces: entrar a `sub/a` en un lado intenta abrir `sub/a` en el otro y muestra
discretamente si no existe. La comparación superficial debe seguir usando solo las entradas ya
cargadas; cualquier comparación recursiva pertenece al asistente cancelable de sincronización.

**Cierre funcional 2026-09-01:** permite seleccionar distintos/exclusivos, ocultar iguales de
la vista real y copiar/mover la selección desde el panel que originó la acción. El botón ↔ activa
opcionalmente el enlace relativo; sus marcas se invalidan al recargar, pero la relación persiste.

### F-9. Lente de procedencia de archivos de Windows
Mostrar bajo demanda los datos de `Zone.Identifier` (zona, URL de origen y referente cuando
existan) en Propiedades/Inspector, con acciones explícitas para copiar la información y desbloquear
el archivo. Implementar la lectura/escritura de ADS exclusivamente en `naygo-platform`, nunca al
listar carpetas: solo para la selección activa y en worker. Informar las limitaciones en volúmenes
sin ADS y exigir confirmación antes de quitar la marca de procedencia.

**Cierre funcional 2026-08-31 (F-9):** Inspector carga Zone.Identifier solo para el archivo
enfocado y desde worker. Permite copiar los datos resueltos y quitar explícitamente la marca tras
confirmación; la escritura ADS también ocurre en worker y luego refresca la metadata.

**Ajuste 2026-09-01:** un ADS presente sin ZoneId/URLs legibles igual mantiene habilitado el
desbloqueo; la confirmación nativa quedó localizada.

### F-10. Bandejas guardables como conjuntos de trabajo (`.naygolist`)
Guardar y abrir una bandeja como lista portable de referencias a archivos dispersos, sin copiar
los datos. Debe admitir rutas absolutas y, cuando haya una raíz común declarada, rutas relativas;
al cargar, conservar entradas ausentes marcadas como tales en vez de descartarlas silenciosamente.
Integrar con búsqueda, filtros, exportación y operaciones por lote. Definir primero un formato
JSON versionado, pequeño, inspeccionable y sin metadatos privados innecesarios.

**Cierre funcional 2026-08-31 (F-10):** `.naygolist` usa JSON versionado, admite raíz con rutas
relativas y conserva referencias ausentes. Al importar, un worker verifica disponibilidad y la
bandeja marca visualmente las rutas faltantes sin eliminarlas ni bloquear la UI.

**Ajuste 2026-09-01:** al guardar, la raíz se selecciona explícitamente y ya no se infiere del
panel activo.

### Preview de texto seleccionable

**Cierre funcional 2026-09-01:** el texto plano de Vista previa es solo lectura, pero permite
seleccionar fragmentos y copiarlos con `Ctrl+C`; el botón de la cabecera conserva copiar todo.

### Cierre funcional 2026-09-02 (árbol, preview, apariencia e íconos)

- El árbol común ahora sigue al panel Files activo y revela su carpeta cuando estaba fuera del
  viewport, sin modificar la navegación ni expandir ramas ajenas.
- La rueda permite recorrer la Vista previa de texto seleccionable (incluye TXT, XML y otros
  formatos de texto); se conserva el límite liviano de lectura y la señal visual de contenido
  recortado.
- Los sets Fluent y de color conservan su identificador al reiniciar. El editor de temas incorpora
  un token independiente para el fondo de la barra de herramientas, compatible con temas previos.
- Se incorporaron los sets compactos y tintables `Kenney Game` y `Kenney Board`: 39 íconos por set,
  con licencia CC0 documentada en `assets/icons/KENNEY-SOURCES.md`; la colección fuente de `tmp/`
  no se empaqueta.

### Cierre funcional 2026-09-02 (SVG, imágenes y ZIP)

- Los SVG con `viewBox` o dimensiones declaradas respetan su canvas original. Si un export carece
  de ambos —como algunos assets de Flash/Animate con coordenadas negativas— el preview encuadra
  sus límites reales y evita el recorte. Al ser vectoriales se rasterizan hasta 2048 px para que
  no se pixelicen al ajustar el panel; ese mismo tope limita la textura a ~16 MiB.
- Las imágenes estáticas disponen de dos controles locales: ajustar al panel (inicial) y `1:1`.
  El modo natural no amplía fotos pequeñas, las centra y habilita scroll solamente cuando exceden
  el área visible. No hay I/O ni decodificación en el hilo de UI.
- El árbol de ZIP/TAR alinea los tamaños en una columna monoespaciada a la derecha, calculada a
  partir de todas las rutas mostradas. Se mantuvo el árbol ASCII y se descartaron puntos de relleno
  para conservar una lectura limpia.

---

---

## D. Bugs/mejoras de arrastrar y soltar

### D-1. Arrastrar dentro del MISMO panel sobre una carpeta (mejora futura)
Hoy el drop intra-panel no mueve/copia a subcarpetas. Implementar con **las mismas reglas que
el drop entre paneles** (copiar por defecto, mover con Shift/mismo volumen, menú con clic
derecho si aplica). Punteros: el drop entre paneles está resuelto en
`crates/ui-slint/src/workspace_ctrl/ops.rs` (`drop_at` y siguientes); extender el hit-test de
`body-touch` en `file-panel.slint` para detectar «fila carpeta destino» dentro del mismo panel.

**Cierre funcional 2026-08-31 (D-1):** el hover OLE ahora cruza su coordenada real con el
`ListView` del `FilePanel`, incluyendo scroll, y reporta la fila de vista bajo el cursor. Si es
una carpeta, `drop_at` la resuelve como destino antes de evaluar las reglas existentes; por tanto
Ctrl/Shift, mover por mismo volumen, confirmación y conflictos se comportan igual que entre
paneles. Se rechaza una carpeta sobre sí misma o dentro de su propio árbol.

---

## Notas de operación para la próxima sesión

- Verificación estándar: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets` (cero warnings), `cargo test --workspace` (todo verde), paridad i18n con
  `scripts/check_i18n_lang.py` para los 10 idiomas al tocar claves.
- Tras cada fix que afecte la app: rebuild de `dist` con `scripts/build-release.ps1` (el
  usuario prueba instalando desde `dist/`; ver el punto «Cómo trabajar» en AGENTS.md).
- **Corrección 2026-09-01 (D-2):** 7-Zip en esta máquina entrega `CF_HDROP` con archivos
  temporales `Temp\\7zE…`, no contenido virtual; borra esa ruta al salir de `Drop`. Naygo ahora
  abre esos handles antes de retornar al archivador y un worker los copia a staging propio, por
  lo que la planificación asíncrona no depende de una ruta efímera. Si una fuente ofrece contenido
  virtual también se prioriza y materializa. Se corrigió además un préstamo `RefCell` en el toast
  de error que podía cerrar Naygo al informar `SourceUnreadable`. Revalidar manualmente archivo +
  carpeta desde 7-Zip y WinRAR, conflicto/cancelación y Explorer (`CF_HDROP`) con el build de
  distribución resultante.
- La máquina del usuario estuvo bajo presión de RAM (~700 MB libres): correr builds pesados
  (release con LTO) SOLOS, sin otros trabajos en paralelo.
- `scripts/run-logged.sh` envuelve comandos pesados (deja salida + curva de RAM en
  `target/agent-out/` y bitácora en `logs/agent-bitacora.md`); útil ante caídas del entorno.
