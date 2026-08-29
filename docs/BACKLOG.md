# Backlog — pendientes y mejoras de Naygo

> Documento de arranque para nuevas sesiones. Todo lo acordado que aún no se implementa,
> con contexto y punteros al código relevante. Actualizarlo al cerrar cada ítem.
> Última actualización: 2026-08-29.

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

### F-1. Exportar listado a CSV/texto
De la selección (o la vista completa) al portapapeles o a un archivo, con columnas
configurables (nombre, extensión, tamaño, fechas, ruta). Punteros: la construcción de celdas
ya existe en `crates/ui-slint/src/bridge.rs` (`cell_value`); añadir acción en menú contextual
(`crates/ui-slint/src/workspace_ctrl/context.rs`) + `Action` de keymap si se quiere atajo.
Pensado para flujo Excel/CSV del usuario. Formato CSV con `;` o `,` configurable, BOM UTF-8
para que Excel lo abra bien.

### F-4. Historial de portapapeles interno (Ctrl+Shift+V)
Los últimos N (p. ej. 10) conjuntos de rutas copiados/cortados **dentro de Naygo**, con
popup para elegir y pegar. Solo rutas de archivos (no texto/imágenes del portapapeles de
Windows). Punteros: el pipeline de copiar/cortar/pegar vive en `workspace_ctrl/ops.rs` y
`ops_ctrl.rs`; guardar el historial en memoria (o en `workspace.json` si se quiere persistente).

### F-5. Historial de navegación con memoria de contexto («Atrás de verdad»)
Al volver Atrás/Adelante, restaurar no solo la ruta sino también el archivo enfocado, selección,
posición de scroll y filtros activos de esa visita. El objetivo es retomar exactamente donde se
estaba sin reconstruir visualmente el contexto. Extender `core::workspace::NavHistory` con una
entrada de navegación compacta; no debe persistir listados ni provocar I/O adicional. Definir qué
parte del contexto sobrevive al reinicio y cómo degradar si los archivos ya no existen.

### F-6. Radar de destinos para copiar/mover
Overlay navegable íntegramente por teclado que reúna paneles abiertos numerados, favoritos,
carpetas recientes/frecuentes y últimos destinos de operaciones. Elegir un destino con una tecla
y ejecutar copiar/mover sin navegar primero hasta él. Reutilizar el selector numérico de paneles,
`RecentDirs`, Favoritos y el historial de operaciones; el ranking debe ser local, determinista y
sin escaneo ni servicio residente.

### F-7. Vista «qué cambió desde mi última visita»
Comparar el último listado completo conservado en memoria con el listado actual al volver a una
carpeta y marcar elementos nuevos, modificados y desaparecidos. Primera versión solo durante la
sesión, usando metadatos ya listados (nombre, tamaño y fecha), sin indexador ni vigilancia global.
Los desaparecidos se presentan como información, no como filas operables. Evaluar persistencia
opt-in únicamente después de medir memoria y utilidad real.

### F-8. Comparación entre paneles accionable
Extender la comparación rápida actual con acciones para ocultar iguales, seleccionar «solo aquí»
o «distintos» y copiar/mover ese subconjunto al otro panel. Agregar navegación relativa enlazada
opcional entre dos raíces: entrar a `sub/a` en un lado intenta abrir `sub/a` en el otro y muestra
discretamente si no existe. La comparación superficial debe seguir usando solo las entradas ya
cargadas; cualquier comparación recursiva pertenece al asistente cancelable de sincronización.

### F-9. Lente de procedencia de archivos de Windows
Mostrar bajo demanda los datos de `Zone.Identifier` (zona, URL de origen y referente cuando
existan) en Propiedades/Inspector, con acciones explícitas para copiar la información y desbloquear
el archivo. Implementar la lectura/escritura de ADS exclusivamente en `naygo-platform`, nunca al
listar carpetas: solo para la selección activa y en worker. Informar las limitaciones en volúmenes
sin ADS y exigir confirmación antes de quitar la marca de procedencia.

### F-10. Bandejas guardables como conjuntos de trabajo (`.naygolist`)
Guardar y abrir una bandeja como lista portable de referencias a archivos dispersos, sin copiar
los datos. Debe admitir rutas absolutas y, cuando haya una raíz común declarada, rutas relativas;
al cargar, conservar entradas ausentes marcadas como tales en vez de descartarlas silenciosamente.
Integrar con búsqueda, filtros, exportación y operaciones por lote. Definir primero un formato
JSON versionado, pequeño, inspeccionable y sin metadatos privados innecesarios.

---

## C. Cambios de atajos y diálogo de creación

### C-1. Intercambiar atajos de creación: Ctrl+N = carpeta
Hoy: `Ctrl+N` = nuevo archivo, `Ctrl+Shift+N` = nueva carpeta. El usuario prefiere:
**`Ctrl+N` = nueva carpeta**, y el archivo pasa a otra combinación (propuesta: `Ctrl+Shift+N`,
o sea, intercambiar los defaults). Cambiar en `crates/core/src/keymap.rs` (entradas `NewFile`/
`NewDir` en `defaults()`), documentarlo en la ayuda (F1) y en `docs/GUIA-DE-USUARIO.md`. Ojo:
el keymap del usuario ya persistido en `keybindings.json` tiene el valor viejo — decidir si se
migra o solo aplica a instalaciones nuevas (el sistema de keymap tiene `reset_action`).

---

## D. Bugs/mejoras de arrastrar y soltar

### D-1. Arrastrar dentro del MISMO panel sobre una carpeta (mejora futura)
Hoy el drop intra-panel no mueve/copia a subcarpetas. Implementar con **las mismas reglas que
el drop entre paneles** (copiar por defecto, mover con Shift/mismo volumen, menú con clic
derecho si aplica). Punteros: el drop entre paneles está resuelto en
`crates/ui-slint/src/workspace_ctrl/ops.rs` (`drop_at` y siguientes); extender el hit-test de
`body-touch` en `file-panel.slint` para detectar «fila carpeta destino» dentro del mismo panel.

---

## Notas de operación para la próxima sesión

- Verificación estándar: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets` (cero warnings), `cargo test --workspace` (todo verde), paridad i18n con
  `scripts/check_i18n_lang.py` para los 10 idiomas al tocar claves.
- Tras cada fix que afecte la app: rebuild de `dist` con `scripts/build-release.ps1` (el
  usuario prueba instalando desde `dist/`; ver el punto «Cómo trabajar» en AGENTS.md).
- Validación manual del D-2 cerrado en código: arrastrar archivo + carpeta desde 7-Zip y WinRAR,
  probar conflicto/cancelación y repetir desde Explorer para confirmar que `CF_HDROP` no regresó.
- La máquina del usuario estuvo bajo presión de RAM (~700 MB libres): correr builds pesados
  (release con LTO) SOLOS, sin otros trabajos en paralelo.
- `scripts/run-logged.sh` envuelve comandos pesados (deja salida + curva de RAM en
  `target/agent-out/` y bitácora en `logs/agent-bitacora.md`); útil ante caídas del entorno.
