# Naygo — plan de usabilidad y flujos de trabajo

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-09-04. Base de producto: 0.5.0.
Estado: séptima entrega implementada, validada automáticamente y empaquetada (recetas).
No representa la aceptación completa del plan: faltan pruebas visuales instaladas, escenarios
reales de red/permisos y revisión editorial de traducciones.

## Distribución actual — séptima entrega

2026-09-05, hora America/Santiago. `scripts/build-release.ps1` ejecutado solo, salida 0;
log `target/agent-out/recipes-release.log`. Release: 25 min 26 s; Inno: 85,390 s.

- `dist/Naygo-0.5.0-setup.exe`: 18:32:16, 51.160.742 bytes.
  SHA-256: `acdff6bbe7c55087911d5f0cc92c021e340c381b9bdd3627cd8ad3c9da123aaf`.
- `dist/Naygo-0.5.0-portable.zip`: 18:30:50, 12.506.624 bytes.
  SHA-256: `7f6b0a2ab2e93dfc4bf324266aa00c43ce42cd283c039f491e2214e1e5ae228b`.
- `naygo.exe`: 35.517.952 bytes, versión 0.5.0 y autoría Nicolás Groth / ISGroth verificadas.
  SHA-256: `c0886bc2956bc12c8b213833bf887172698c0e81a125cb73723a4b83653476f9`.

Checksums contrastados con `dist/SHA256SUMS.txt`; ZIP íntegro y ejecutable interno idéntico al
release. PDB: 332.640.256 bytes, opcional en instalador y excluido del ZIP según T-3. Respecto
de la sexta entrega: portable +153.843 bytes, exe +509.952 bytes, instalador +561.630 bytes.
Sin dependencias nuevas; estos tamaños no son mediciones de memoria/rendimiento en ejecución.

Suite: 1.228 aprobados (859 core + 22 integración + 47 platform + 300 UI), seis smoke tests
interactivos ignorados; Clippy sin advertencias, formato/diff y paridad de diez catálogos aprobados.
Logs `recipes-final-tests.log`, `recipes-final-clippy.log` y `recipes-graphify.log`; grafo AST:
7.426 nodos / 13.508 aristas. Graphify conserva avisos de 23 archivos sin nodos (incluidos JSON)
y etiquetas históricas; no se ejecutó extracción ni etiquetado con LLM.
Origen: working tree sin commit sobre `6ea74b9f39da`, rama `fix/single-instance-y-bandeja`.
No se cerró ni instaló sobre la sesión del usuario. Pendientes: aceptación instalada de las etapas
(teclado/DPI/Narrador, permisos/red y cancelación real), traducción editorial de ocho catálogos con
respaldo EN y firma externa. Recetas accesibles desde Disposiciones/Ctrl+P; guía `docs/RECETAS.md`.

## Distribución anterior — sexta entrega

2026-09-05, hora America/Santiago. `scripts/build-release.ps1` ejecutado solo, salida 0;
log `target/agent-out/points-release.log`. Release: 36 min 27 s; Inno: 85,719 s.

- `dist/Naygo-0.5.0-setup.exe`: 17:24:23, 50.599.112 bytes.
  SHA-256: `635c8d426976edadb0dff7dd9510566004e3f3a7abdd83fb3ec2cb1910c66b41`.
- `dist/Naygo-0.5.0-portable.zip`: 17:22:57, 12.352.781 bytes.
  SHA-256: `43f96d7b07cb99bc530a84aada2590451ac4bc0a44ef7b91c3e2bfa83a2c43c5`.
- `naygo.exe`: 35.008.000 bytes, versión 0.5.0 y autoría Nicolás Groth / ISGroth verificadas.
  SHA-256: `7c58acaa0081d2a91b301636272a705f315cdfb121462649a0c2ce2398c05644`.

Checksums contrastados con `dist/SHA256SUMS.txt`; `7z t` aprobado y ejecutable dentro del ZIP
idéntico al release. PDB de 328.609.792 bytes: opcional en instalador, fuera del ZIP según T-3.
Respecto de la quinta entrega: portable +164.145 bytes, ejecutable +520.704 bytes, instalador
+605.587 bytes. No se añadieron dependencias. Esto no mide consumo ni rendimiento en ejecución.

Suite final: 1.207 aprobados y seis smoke tests interactivos ignorados; Clippy sin advertencias,
formato/diff y paridad de diez catálogos aprobados. Logs `points-final-tests.log`,
`points-final-clippy.log` y `points-graphify-validated.log`. Grafo: 7.309 nodos / 13.256 aristas.
Origen: working tree sin commit sobre `6ea74b9f39da`, rama `fix/single-instance-y-bandeja`.
No se cerró ni instaló sobre la sesión de Naygo del usuario. Pendientes: aceptación instalada
(DPI/Narrador, permisos/red real, cancelación de hash grande y papelera), traducción editorial
de los ocho catálogos con respaldo EN, firma externa y etapa 7 (recetas).

## Distribución anterior — quinta entrega

2026-09-05, hora America/Santiago. `scripts/build-release.ps1` ejecutado solo, salida 0;
log `target/agent-out/queries-release.log`. Release: 39 min 31 s; Inno: 135,297 s.

- `dist/Naygo-0.5.0-setup.exe`: 14:44:47, 49.993.525 bytes.
  SHA-256: `d7874bd9532947d2cdaf09eaf27a833118314c7c275cb5f5a7ede24b94017acc`.
- `dist/Naygo-0.5.0-portable.zip`: 14:42:32, 12.188.636 bytes.
  SHA-256: `3a86c4efee1eb430722a74fee678c51f5ad158e3e627d747dcb8098747a144a7`.
- `naygo.exe`: 34.487.296 bytes, versión 0.5.0 y autoría Nicolás Groth / ISGroth verificadas.
  SHA-256: `30fb2305b7ce8b669620245099837b938ffba7c29fbb998e1c24a215675a1353`.

SHA-256 contrastados con `dist/SHA256SUMS.txt`; ZIP aprobado con `7z t` y ejecutable interno
idéntico al release. PDB de 324.087.808 bytes, opcional en instalador y excluido del ZIP como antes.
Portable: 281.686 bytes menos que la cuarta entrega; ejecutable: 427.520 bytes más.
El build consumió más tiempo/memoria que la entrega previa (pico observado del compilador ≈18,3 GiB);
esto no es una medición del consumo en ejecución. Sigue pendiente la comparación de rendimiento instalada.

Suite exacta: 1.181 aprobados, 6 ignorados; Clippy sin advertencias, formato, diff y paridad de
10 idiomas aprobados. Logs `queries-validated-tests.log`, `queries-validated-clippy.log` y
`queries-graphify.log` en target/agent-out. Grafo AST: 7.178 nodos / 12.966 aristas.
Origen: working tree sin commit sobre `6ea74b9f39da`, rama `fix/single-instance-y-bandeja`.
No se cerró ni reemplazó la instancia instalada del usuario. Validación visual/DPI/Narrador/red real
y firma externa siguen pendientes; al cerrar esa entrega las etapas 6–7 no estaban implementadas.

## Distribución anterior — cuarta entrega

2026-09-05, hora America/Santiago. `scripts/build-release.ps1` ejecutado solo, salida 0;
registro `target/agent-out/spaces-release.log`. Compilación release: 25 min 40 s; Inno: 85,5 s.

- `dist/Naygo-0.5.0-setup.exe`, 12:40:27, 49.512.070 bytes.
  SHA-256: `92f86e9936f9d83394fc3d4ec022bfa17cd1c47faa58959bf32e1d08a8f89f4f`.
- `dist/Naygo-0.5.0-portable.zip`, 12:39:01, 12.470.322 bytes.
  SHA-256: `c4628c3dc1657d744199e951a461c4832e6b802ba7ccb2546cdec130d0447918`.
- `naygo.exe`: 34.059.776 bytes, versión 0.5.0 y autoría verificadas.
  SHA-256: `fea676a50dceb6df72a47efaac36c6b3c2668c99101f29210be497f7dff25092`.

Hashes contrastados con `dist/SHA256SUMS.txt`; ZIP aprobado con `7z t` y ejecutable idéntico
al release. PDB de 320.688.128 bytes disponible para diagnóstico, opcional en instalador,
excluido del ZIP como en las entregas previas. Sin nuevas dependencias; portable +145.762 bytes.
Formato, diff y paridad i18n verificados. Suite final: 1.162 aprobados (815 core + 22 integración
+ 40 platform + 285 UI), 6 ignorados; Clippy `--all-targets --locked -- -D warnings` aprobado.
Logs: `spaces-final-tests.log`, `spaces-final-clippy.log`; grafo AST actualizado a 7.046 nodos /
12.702 aristas (`spaces-graphify.log`). Todos dentro de `target/agent-out`.

No se cerró la instancia instalada ni se dio por hecha la validación visual/DPI/Narrador/red caída.
Sin firma externa. Origen: working tree sin commit sobre `6ea74b9f39da`, rama
`fix/single-instance-y-bandeja`. La etapa 4 mantiene aceptación manual pendiente; 6–7 no implementadas.

## Entregas anteriores

Segunda entrega: 1.142 pruebas aprobadas, 6 ignoradas; formato, Clippy con `-D warnings`,
paridad de 10 idiomas y grafo AST verificados. Distribución regenerada con
`scripts/build-release.ps1`, salida 0 (compilación release: 23 min 14 s).
Logs: `target/agent-out/delivery-validated-tests.log`, `delivery-final-clippy.log`,
`delivery-graphify.log`, `delivery-release.log`.

Artefactos de la SEGUNDA entrega del 2026-09-05, hora America/Santiago (anteriores a estos refinamientos):

- `dist/Naygo-0.5.0-setup.exe`: 00:13, 48.621.974 bytes.
  SHA-256: `a215594af311fa4eafe3aa98f65c414c4e981f10f34ad105860d71628da6e858`.
- `dist/Naygo-0.5.0-portable.zip`: 00:11, 12.233.915 bytes.
  SHA-256: `549d7a5e64e87e3b548947f1a57dc4fc8b1d8231bccd84d243ce0af418ef46cf`.
- Ejecutable: 33.288.192 bytes; ZIP y `target/release/naygo.exe` idénticos.
  SHA-256: `bc2c06a4cbae81aa46b050e1006ea263b3cb90bbb3d644df71cc224b13375608`.

Checksums contrastados con `dist/SHA256SUMS.txt`, ZIP legible sin PDB, versión 0.5.0 y autoría
del exe verificadas. PDB disponible en `target/release` y como componente opcional del instalador.
Origen: working tree sin commit sobre `6ea74b9f39da`, rama `fix/single-instance-y-bandeja`.
Instalador sin firma (certificado externo T-4 pendiente). No se cerró la instancia instalada del
usuario; la matriz visual/DPI/Narrador sigue pendiente y no se infiere de las pruebas automáticas.

## Avance de implementación — 2026-09-04

- Columna ordenada: tinte de acento al 8 % en sus celdas, exclusivamente en el panel activo.
  Selección, coincidencias, comparación, cambios recientes y hover conservan prioridad.
- Ruta: área flexible acotada, últimos segmentos elididos, menú de todos los ancestros y
  edición de ruta completa. Favoritos/copiar/vista profunda conservan espacio propio. En paneles
  estrechos los controles de comparación pasan a una segunda fila.
- Bandeja: flechas, Inicio/Fin y página con seguimiento del scroll; primera flecha sin foco
  empieza en el extremo correspondiente. El foco ya no dirige esos gestos al último Files.
  Metadata básica de tamaño/fechas/tipo resuelta en worker, vinculada a la ruta vigente.
  Botones con nombre accesible separado del tooltip y activación Enter/Espacio.
- Radar: lista desplazable, flechas/Enter y acceso numérico inicial; las fuentes se congelan
  antes de elegir destino. Mover no vacía prematuramente las referencias de la bandeja.
- Maximización: botón de título/pestañas, paleta y atajo configurable Ctrl+Shift+M.
  No modifica el layout serializado ni elimina modelos de paneles ocultos. Cambiar de panel,
  cerrar o aplicar una plantilla abandona la maximización. El modo de imagen sobrevive al remontaje.

### Segunda entrega — implementación

- Selección múltiple por ruta en bandeja: Ctrl/Shift, Ctrl+A, Ctrl+flechas, rango por teclado,
  foco diferenciado, arrastre del grupo y conservación de marcas al cambiar de panel.
- Copiar/mover/borrar/entregar solo usan los marcados. Vaciar es explícitamente toda la bandeja;
  quitar referencias no elimina originales. Borrar confirma fuentes concretas y conserva las
  referencias al cancelar/fallar. Ctrl+V agrega referencias en vez de pegar en un Files anterior.
- Botones de bandeja agrupados en dos filas; controles con foco/teclado y tooltips. Sus nuevos
  íconos usan assets compilados, sin lectura de disco en refresco (fallback Lucide para externos).
- Asistente de entrega desde bandeja o Ctrl+P: revisión congelada, grupos numerados por origen,
  plano o raíz relativa explícita; carpeta nueva o ZIP; cancelación, detección de conflictos,
  verificación opcional SHA-256, manifiesto y lista sin rutas privadas. Reutiliza motores de
  copia/ZIP y el panel de operaciones. Undo retira solo el destino publicado.
- El ZIP consume el inventario revisado; no reescanea silenciosamente. Límite de 50.000 entradas,
  revisión visual hasta 500, temporales exclusivos junto al destino y protección ante cambios
  de fuentes o destino ya existente. Detalles y límites: [Entregas](../ENTREGAS.md).

### Tercera entrega — implementación

- Editor de nombres por origen (una línea por fuente); valida cantidad, componentes y homónimos
  antes de publicar. Las extensiones se conservan por defecto y la guía explica mantenerlas.
- Revisión conjunta de homónimos, incluidas diferencias de mayúsculas y nombres reservados del
  manifiesto. En modo plano, propuesta numerada opt-in; exige revisar de nuevo y no descarta fuentes.
  Las filas de conflicto están limitadas a 500, con indicador de contenido adicional.
- Resultado no modal: abrir carpeta en un panel nuevo, copiar ubicación completa y recuperar las
  fuentes en bandeja (con sus guards de staging mientras se necesitan) sin quitar otras referencias.
  La tarjeta muestra el último resultado; errores/cancelaciones no habilitan acciones de salida.
- Fallos de ejecución de entrega conservados como resultado fallido por destino en Operaciones;
  deshacer solo se registra cuando existe un destino publicado. Nunca un falso «hecho: 0» por perder
  el mensaje de error del flujo de entrega.
- Detección de raíces solapadas O(n log n), evitando comparar cada pareja de referencias.
- Configuración: búsqueda de secciones por etiquetas de ajustes traducidas, estado sin coincidencias
  y navegación lateral con foco visible y activación Enter/Espacio. Sin I/O ni índice residente.

Validación de tercera entrega: 1.147 pruebas aprobadas (808 core + 22 integración + 37 platform +
280 UI), 6 ignoradas; formato, diff, paridad i18n de diez idiomas y grafo AST verificados
(6.943 nodos, 12.467 aristas). Logs `target/agent-out/refinements-tests.log` y
`refinements-graphify.log`. Clippy final con `--all-targets --locked -- -D warnings` aprobado
(`refinements-final-clippy.log`). Distribución regenerada con `scripts/build-release.ps1`, salida 0;
log `target/agent-out/refinements-release.log` (compilación optimizada: 23 min 48 s).

Artefactos de la TERCERA entrega, 2026-09-05, hora America/Santiago (históricos):

- `dist/Naygo-0.5.0-setup.exe`: 07:32, 48.987.054 bytes.
  SHA-256: `d2be90bcb3f81fb7cc343b4e8c753f6d7fa1a9ab9bf51b3961b68157650545e0`.
- `dist/Naygo-0.5.0-portable.zip`: 07:30, 12.324.560 bytes.
  SHA-256: `d7d313700ca45c41539631ff584e32aa3a1eeb6cc2f7ecaf8bd7059f7e99df94`.
- Ejecutable: 33.639.936 bytes; versión y autoría verificadas, ZIP idéntico al release.
  SHA-256: `078bd9e637fc0a0e1da8da62ab9e9c83a5d5b2d989ec68fe5ab717c5a21e299d`.

Checksums contrastados con `dist/SHA256SUMS.txt`, ZIP legible sin PDB, staging de empaquetado
limpiado. PDB disponible para diagnóstico y opcional en instalador. Sin nuevas dependencias;
portable +90.645 bytes respecto a la segunda entrega. Origen: working tree sin commit sobre
`6ea74b9f39da`. Sigue pendiente la firma externa y la validación visual/DPI/Narrador instalada;
no se cerró la sesión del usuario. No confundir las pruebas automáticas con esa validación manual.

Pendiente de etapa 3: validación manual de disco lleno, cancelación de entregas grandes y limpieza
fallida. Otros errores de lectura/ruta se informan al encontrarlos; no hay resolución masiva de
esos errores, sobreescritura ni omisiones. No marcar la etapa completa todavía.

Pendiente dentro de etapas 0–2: medidas comparativas de rendimiento, matriz visual/DPI y
Narrador, uniformar todos los
controles y la restauración por sección de configuración. La validación visual requiere cerrar la instancia
instalada (instancia única); no se cierra automáticamente una sesión del usuario.

## Objetivo y alcance

Hacer que navegar, reunir archivos y preparar una entrega resulte rápido, predecible y fácil de
descubrir. Implementar las siete mejoras conversadas: rutas adaptables, maximización temporal,
entregas desde bandeja, espacios por tarea, búsquedas guardadas, puntos de comparación y recetas.
La usabilidad se valida en cada entrega, no se reserva para una fase cosmética final.

Se conserva Rust/Slint con renderer software, procesamiento pesado cancelable en workers,
streaming incremental, configuración local y formatos pequeños/versionados. No se añade un
indexador residente ni se convierte Naygo en editor o reproductor. Los nuevos flujos usan el motor
existente de operaciones y su tratamiento de conflictos, errores y recuperación.

## Evidencia y punto de partida

Revisión de código y captura aportada por Nicolás; no equivale a una prueba interactiva de esta
versión instalada. Revalidar los puntos indicados en el primer build.

| Área | Estado observado | Consecuencia para el plan |
|---|---|---|
| Ruta del panel | `file-panel.slint` asigna a cada segmento su ancho preferido completo en la misma fila que las acciones | Reservar acciones y limitar el ancho del breadcrumb |
| Bandeja: propiedades | `basket_inspector_info()` fija `Archivo` y deja tamaño/fechas vacíos | Completar metadata básica en worker y distinguir carpetas/ausentes |
| Bandeja: teclado | Existen ↑/↓ en controlador; sus botones personalizados usan `TouchArea` sin foco explícito | Verificar el recorrido real, scroll y accesibilidad; tooltip no sustituye nombre accesible |
| Radar | Hasta nueve opciones numéricas y tarjeta con altura acotada, sin lista desplazable explícita | Verificar ventanas bajas y más de nueve paneles/destinos |
| Espacios | Ya hay plantillas de layout y sesión persistida | Extender el concepto sin duplicar un segundo gestor de disposiciones |
| Herramientas | Ctrl+P, F1, sincronización, comparación, historial, `.naygolist` y cambios de visita ya existen | Integrarlas; no presentarlas como novedades pendientes |
| Entrega previa | Hay cambios de 0.5.0 sin commit en el árbol de trabajo | Inventariar y validar la base antes de mezclar nuevas etapas |

No deducir contraste, tamaño real de fuentes ni accesibilidad desde una captura redimensionada.
Esas medidas necesitan el binario instalado, el tema activo y DPI conocido.

## Orden y dependencias

| Etapa | Contenido | Depende de | Tamaño relativo |
|---|---|---|---|
| 0 | Base reproducible, inventario y escenarios de medición | — | Pequeño |
| 1 | Rutas, foco, bandeja, accesibilidad y controles adaptables | 0 | Mediano |
| 2 | Maximizar/restaurar panel temporalmente | 1 | Pequeño/mediano |
| 3 | Preparar entregas desde la bandeja | 1 | Grande |
| 4 | Espacios de trabajo por tarea | 2, 3 | Grande |
| 5 | Búsquedas guardadas | 4 | Mediano |
| 6 | Comparación contra punto guardado | 3 | Grande |
| 7 | Recetas reutilizables | 3, 4, 5 | Grande |

Secuencia de trabajo propuesta: 0 → 1 → 2 → 3 → 4 → 5 → 6 → 7. Los tamaños reflejan complejidad,
no estimaciones de días. Al comenzar cada etapa grande, concretar su esquema y casos límite antes
de editar código. Entregar feature por feature y recoger la validación de Nicolás antes de seguir.
No fijar de antemano nuevas versiones: cada entrega indica versión y revisión exactas.

## Etapa 0 — base reproducible

- Separar en el inventario cambios previos de nuevas tareas; conservar `tmp/` y archivos del usuario.
- Verificar versión de workspace, lockfile, About, recursos del exe e instalador.
- Registrar resultados reales de pruebas; corregir cifras históricas del backlog cuando se midan.
- Preparar fixtures propios: rutas profundas/UNC, Unicode, archivos grandes, referencias ausentes,
  carpetas vacías, homónimos y permisos denegados. Nunca probar borrados con documentos del usuario.
- Medir arranque, memoria en reposo, navegación/listado y respuesta a entrada sobre la misma máquina
  y los mismos datos. Guardar máquina, DPI, configuración y revisión junto a las mediciones.

Aceptación: base identificable y resultados disponibles; no considerar un proceso terminado como
prueba aprobada sin capturar su código de salida y resultado.

## Etapa 1 — usabilidad esencial

### U-1. Barra de rutas adaptable (prioridad máxima)

- Tres zonas: navegación, ruta flexible y acciones reservadas a la derecha.
- Mantener copiar ruta, favorito y la acción actual de vista siempre accesibles. Verificar el
  significado del tercer botón con su callback/tooltip: actualmente `toggle-deep` controla la
  vista recursiva, no la expansión textual de la ruta. No cambiarle la función por interpretación.
- Mostrar raíz, `…` y los últimos segmentos que quepan. Menú de ancestros omitidos con mouse y
  teclado; ruta completa al editar/copiar y disponible como tooltip.
- Si un único nombre excede el ancho, elidirlo sin cambiar la ruta operable. UNC conserva la
  identidad de servidor/recurso cuando quepa; nunca confundir ruta visual con ruta del filesystem.
- En anchos extremos, agrupar navegación secundaria en un menú y establecer un mínimo de panel
  realista. Ninguna acción debe desaparecer fuera del recorte.

Aceptación: paneles de 240/320/480/800 píxeles lógicos, rutas profundas, segmentos largos, Unicode,
UNC y modo edición; sin solapamiento ni pérdida de botones. Copiar devuelve la ruta íntegra y
elegir un ancestro navega al correcto. Probar también con acciones de comparación visibles.

### U-2. Contexto de selección y bandeja

- Definir una fuente común de selección para Preview y Propiedades (Files, Bandeja y resultados
  cuando corresponda), referenciada por ruta/identidad estable y no por índice visual.
- Flechas, Inicio/Fin, RePág/AvPág y scroll para mantener visible el elemento enfocado. Sin selección,
  ↓ entra por el primero y ↑ por el último; listas vacías y extremos son estables.
- Selección múltiple Ctrl/Shift, Ctrl+A y arrastre del conjunto elegido. Mostrar recuento y alcance:
  operar la selección si existe; toda la bandeja solo con alcance explícito, visible en la acción.
- Distinguir «Quitar de la bandeja», «Vaciar bandeja» y «Enviar a la papelera». Eliminar referencias
  no elimina originales. Confirmaciones de borrado muestran nombres, cantidad y modo real.
- Completar tipo, tamaño y fechas en worker cancelable; descartar respuestas de una selección
  anterior. Carpetas, rutas ausentes y metadata inaccesible tienen estados propios.
- Verificar estabilidad al pasar de Bandeja a Preview/Inspector y de vuelta a Files; copiar texto
  del preview no debe disparar copiar archivos del último panel.

Aceptación: recorrer 100 referencias con mouse y teclado sin perder foco ni contexto; carpeta
seleccionada identificada correctamente; resultados tardíos no contaminan la nueva selección;
quitar/vaciar deja intactos los originales.

### U-3. Controles, menús, configuración y feedback

- Componente de acción compartido: foco visible, nombre accesible, activación con Enter/Espacio,
  tooltip con atajo y estado habilitado coherente. Usar assets de acciones del set activo en lugar
  de depender de glifos emoji ambiguos. Verificar soporte real del backend con Narrador.
- Densidades compacta y cómoda coherentes con los ajustes existentes. Ampliar área clicable sin
  agrandar innecesariamente el dibujo; ensayar 28–32 px lógicos y ajustar con mediciones.
- Diferenciar selección, foco, hover y panel activo mediante borde/forma además del color.
  Validar contraste de texto normal de al menos 4.5:1 y controles/foco de 3:1 como objetivos de
  diseño; conservar personalización y no afirmar conformidad completa sin auditoría.
- Radar desplazable, ↑/↓/Enter/Escape, búsqueda si hay muchos destinos y retorno de foco al origen.
  Los primeros nueve mantienen atajos numéricos, pero los demás siguen accesibles.
- Menús y diálogos ajustados a ventana/DPI; botones principales visibles aunque el cuerpo requiera
  scroll. Reducir cabeceras repetidas y ordenar acciones por frecuencia/función.
- Configuración: buscador de ajustes, grupos consistentes, indicación de cambios y restauración
  por sección. Mantener nombres uniformes («Disposiciones», «Espacios», «Bandeja») en español.
- Estados vacíos con acción útil; errores por archivo con reintento; historial legible con detalle
  expandible. No indicar éxito si la operación no completó lo solicitado. Revisar precedencia de
  Escape: cerrar menú/modal antes de abandonar una vista o cancelar otro trabajo.

Aceptación: completar navegación, bandeja y copia usando únicamente teclado; acciones de icono
identificables por Narrador donde el backend lo soporte; menús accesibles con ventanas bajas y
escalado 100/125/150/200 %. Cualquier limitación de accesibilidad se registra explícitamente.

## Etapa 2 — maximización temporal

- Acción «Maximizar/restaurar panel» en cabecera, menú y Ctrl+P; atajo configurable elegido tras
  revisar colisiones. Doble clic en cabecera como alternativa, sin interferir con mover paneles.
- Mantener el árbol de layout original y aplicar una presentación temporal. Ocultar otros paneles
  no destruye sus modelos, selección ni trabajos. Desactivar splitters/hit-test de paneles ocultos.
- Restaurar proporciones y pestañas al salir. Definir salida al cerrar el panel maximizado,
  aplicar disposición o cambiar espacio; no persistir accidentalmente el layout de un solo panel.

Aceptación: maximizar Files, Preview y Bandeja y restaurar exactamente la disposición; operaciones
en curso sobreviven y los drops se resuelven con la geometría que realmente está visible.

## Etapa 3 — preparar una entrega

- Flujo corto: elegir referencias → revisar contenido/destino → generar carpeta o ZIP.
- Elegir explícitamente estructura plana o relativa a una raíz. Para varias raíces, proponer
  grupos y permitir revisar sus nombres; no adivinar una raíz común peligrosa.
- Vista del resultado con nombre original/final, origen, ruta de salida, tamaño y conflictos:
  homónimos, diferencias solo de mayúsculas, ausentes, destinos inválidos y solapamientos.
- Originales conservados por defecto. Planificación y ejecución cancelables; revalidar entradas
  antes de ejecutar porque pueden cambiar desde la vista previa.
- Generar manifiesto JSON y listado legible con rutas relativas, tamaños y resultados reales.
  Hash SHA-256 opcional bajo demanda; explica que añade lectura. No exportar rutas privadas
  absolutas por defecto ni recopilar metadata no necesaria para la entrega.
- Directorio/ZIP provisional propio, publicación al finalizar sin sobreescritura silenciosa;
  cancelar limpia solo parciales del job. Conservar staging de archivadores mientras se usa.
- Resultado con abrir destino, copiar ubicación y volver a la selección. Errores muestran qué
  ocurrió; no llamar «completa» a una entrega parcial.

Aceptación: entregar referencias de varias carpetas, gestionar homónimos y verificar manifiesto
contra salida; simular modificación de origen, falta de espacio y cancelación. Originales intactos.

## Etapa 4 — espacios de trabajo por tarea

Implementada en la cuarta entrega: gestor desde Disposiciones/Ctrl+P, archivos `.naygospace`,
guardar como/exportar, actualizar con revisión conocida y abrir/importar con confirmación.
Raíz explícita, conservación de filtros de tabla y tipeo; cola de operaciones global intacta.
El indicador de cambios vive en el gestor; elección rápida mediante el selector nativo de archivos,
sin catálogo automático ni búsquedas al arranque. Migración de layout: aplicar el existente y
guardar el estado como espacio, manteniendo la plantilla original. [Guía completa](../ESPACIOS.md).

Validado por pruebas: serialización y límites, relaciones entre paneles, staging léxico, raíces,
colisiones, cancelación, corrupción/cambio externo, reapertura en otro controlador y copia que
continúa al alternar espacio. Pendiente la aceptación instalada con teclado/DPI/Narrador y una
unidad de red realmente desconectada. No marcar completa hasta esa validación.

- Evolucionar layouts: un espacio con nombre guarda disposición, rutas, columnas/filtros, panel
  activo y bandeja; incluir referencias a búsquedas/recetas después de sus respectivas etapas.
- Guardar/actualizar explícitamente y cambiar rápido desde menú y Ctrl+P. Mostrar cuál está activo
  y si hay cambios sin guardar; elegir guardar/descartar al cambiar cuando corresponda.
- Las operaciones en curso son globales y continúan al cambiar de espacio; sus destinos capturados
  no deben recalcularse desde el nuevo panel activo.
- JSON versionado, escritura atómica en worker, migración de layouts existentes y recuperación de
  archivo corrupto sin sobrescribirlo. No guardar listados ni bytes de preview.
- Raíz portable y reasignación guiada de rutas al abrir en otra máquina. Referencias de staging
  efímero deben materializarse por acción explícita o marcarse no persistibles, nunca guardarse
  como si fueran archivos duraderos. Importar un espacio no ejecuta sus recetas.

Aceptación: cerrar/reabrir y alternar dos tareas conservando estado; abrir con unidad desconectada
sin congelamiento; datos corruptos/versión desconocida generan un error recuperable.

## Etapa 5 — búsquedas guardadas

Implementado el 2026-09-05: formato .naygosearch acotado/versionado, editor accesible desde F3 y
Ctrl+P, carga sin ejecutar, escritura atómica/revisión, hasta 64 raíces, filtros por tamaño y
calendario local al ejecutar, streaming con límites globales y cobertura explícita, selección
con preview/propiedades y envío a bandeja. Los espacios conservan referencias, nunca resultados
ni ejecución. Guía: [BUSQUEDAS-GUARDADAS.md](../BUSQUEDAS-GUARDADAS.md).
Chrono ya existía transitivamente; platform lo declara directamente para resolver los cambios
DST en el worker sin añadir otro indexador ni servicio.
Validación final: 1.181 tests aprobados (825 core + 22 integración + 44 platform + 290 UI),
6 smoke tests ignorados; Clippy --all-targets --locked -- -D warnings, formato y diff aprobados.
Logs: target/agent-out/queries-validated-tests.log y queries-validated-clippy.log.
Paridad de 10 idiomas aprobada; grafo AST actualizado a 7.178 nodos / 12.966 aristas
(queries-graphify.log). Distribución regenerada y verificada; detalle arriba.
Aceptación instalada/DPI/Narrador y red caída real permanece pendiente.

- Guardar criterios y una o varias raíces, con nombre, desde el buscador existente. Integrar en
  espacios y paleta; ejecutar/actualizar explícitamente usando streaming y cancelación existentes.
- Agregar fechas relativas y tamaño al modelo de criterios compartido si aún no están soportados;
  «esta semana» se evalúa al ejecutar según fecha/zona local, no al guardar.
- Distinguir conjunto estático `.naygolist` de consulta guardada. Mostrar cuándo se ejecutó y si
  resultados son parciales por permisos, límites o raíces ausentes; deduplicar raíces solapadas.
- Resultados seleccionables, preview/propiedades y envío a Bandeja/entrega. No escanear al arranque
  ni mantener vigilancia global por haber guardado una consulta.

Aceptación: consulta de PDF por fecha sobre dos raíces, persistencia tras reiniciar, cancelación,
renovación de fechas relativas y señal inequívoca de resultados parciales.

## Etapa 6 — puntos de comparación

Implementada en working tree. Captura versionada `.naygopoint`, worker cancelable, SHA-256 opcional,
metadatos y cobertura con límites; lectura pasiva, exportación inmutable y papelera con confirmación
y revisión del documento original. La selección de cambios presentes utiliza la Bandeja existente.
Guía: `docs/PUNTOS-DE-COMPARACION.md`. 1.207 pruebas aprobadas (845 core + 22 integración +
45 platform + 295 UI), seis smoke tests interactivos ignorados. Clippy final sin advertencias,
formato/diff limpios y paridad de diez catálogos. Logs `points-final-tests.log` y
`points-final-clippy.log`; grafo AST actualizado: 7.309 nodos / 13.256 aristas
(`points-graphify-validated.log`). Instalador/portable regenerados y verificados (arriba);
aceptación instalada aún pendiente.

- Acción «Guardar punto de comparación»: raíz, alcance superficial/recursivo y exclusiones claros.
  Captura cancelable con fecha, ruta relativa, tipo, tamaño y modificación; hash opcional.
- Comparar más tarde mostrando agregados, modificados y ausentes; los ausentes son informativos,
  no entradas seleccionables para borrar/copiar. Permitir enviar cambios presentes a una entrega.
- Distinguir igualdad por metadata de igualdad verificada por contenido; no inferir renombrados
  con certeza usando solo nombre/tamaño. No es respaldo ni permite restaurar bytes eliminados.
- Comparar solo alcances compatibles. Recorridos incompletos o directorios inaccesibles producen
  «desconocido/no examinado», nunca una lista falsa de archivos eliminados.
- Persistencia explícita y local, gestión de borrar/exportar puntos y límites de tamaño visibles.

Aceptación: captura, reinicio, modificaciones conocidas y comparación exacta del alcance; probar
permisos denegados y detectar incompletitud; hashing cancelable y memoria acotada.

## Etapa 7 — recetas de operaciones

Implementada en working tree; validación automática aprobada. Editor y documentos `.naygorecipe`, parámetros de
ejecución separados, carga pasiva, protección de revisión, referencias en espacios y Ctrl+P.
Reutiliza búsqueda/entregas: consulta incompleta bloquea, plan congelado y salida nueva sin
sobrescribir. Fecha local «este mes» y `{date}`; no añade dependencias. Guía: `docs/RECETAS.md`.
No incluye scripts, automatización residente ni copiar sobre carpetas existentes. Distribución
regenerada y verificada (arriba). Pruebas finales: 1.228 aprobadas (859 core + 22 integración + 47 platform +
300 UI), 6 smoke tests interactivos ignorados; Clippy sin advertencias, formato/diff y paridad de
diez catálogos aprobados. Logs `recipes-final-tests.log` y `recipes-final-clippy.log`.

- Secuencia declarativa visible: reunir por consulta/selección → filtrar → proponer nombres →
  preparar carpeta/ZIP o copiar a destino. Reutilizar planes y criterios de etapas anteriores.
- Editor simple con parámetros de fecha/raíz/destino; sin lenguaje de scripts ni ejecución de shell.
- Cada ejecución evalúa el contexto, muestra fuentes y resultados propuestos y requiere ejecutar
  explícitamente. Congelar el plan elegido, revalidarlo al comenzar y no incorporar nuevos archivos
  silenciosamente por cambios posteriores en la consulta.
- No borrar ni mover originales por defecto; cualquier extensión a pasos destructivos necesita
  diseño específico de confirmación y reversibilidad. Reportar pasos completados/fallidos y límites
  reales de deshacer. No prometer transacción atómica entre varios volúmenes.
- Guardar JSON versionado y compartir con parámetros de raíz; importar nunca ejecuta. Registrar
  resultado por ejecución sin servicios residentes ni programación automática en esta etapa.

Aceptación: «reunir PDF del mes y preparar entrega fechada» reproducible; cancelación y fallo de
un paso dejan claro qué existe en destino; reejecución no duplica ni sobrescribe sin resolución.

## Guía de implementación

Puntos existentes: `ui/file-panel.slint`, `basket-panel.slint`, `destination-radar.slint`,
`app-window.slint`, `preview-panel.slint`, `config-window.slint`; controladores
`workspace_ctrl/{input,basket,meta,templates,session,layout_panes,search}.rs`; `core/{workspace,
naygolist,search,listing_changes,ops,archive_ops}`. Verificar nombres/contratos al iniciar cada etapa.

Módulos nuevos propuestos, no existentes aún: modelos puros de entrega, espacio por tarea, consulta
guardada, punto de comparación y receta en core; controladores pequeños respectivos en ui-slint.
El renderer recibe view models y callbacks; platform concentra cualquier integración Windows.
No añadir lógica nueva a `main.rs` ni agrandar indefinidamente los controladores actuales.

## Validación y cierre por entrega

- Pruebas proporcionales: algoritmos de abreviación/selección, round-trip/migración de formatos,
  conflictos/cancelación, restauración del layout y operaciones sobre fixtures temporales.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`;
  paridad de claves/placeholders en los diez idiomas cuando cambien textos.
- Ejecución interactiva del instalador en Windows: ventana 1366×768 y 2560×1440, DPI
  100/125/150/200 %, temas claro/oscuro/alto contraste, teclado, mouse y varias pestañas/paneles.
  Matriz de escenarios críticos, no obligación de todas las combinaciones cartesianas.
- Medir sobre baseline comparable: objetivo de feedback de entrada p95 ≤100 ms en equipo de
  referencia; regresiones repetibles >10 % de memoria/arranque/listado requieren investigación.
  Son presupuestos propuestos a calibrar con etapa 0, no resultados medidos ni garantías de red.
- Ningún worker nuevo hace trabajo en reposo sin petición. Operaciones pesadas muestran progreso
  y aceptan cancelación; distinguir petición de cancelar de finalización de I/O no interrumpible.
- Después de código: `graphify update .` y actualizar BACKLOG con evidencia de cierre.
- SIEMPRE `scripts/build-release.ps1` tras cada cambio de app, SOLO, sin tests/builds pesados en
  paralelo. Esperar salida y código final, verificar versión, archivos, hashes y ZIP legible.
- Entregar enlaces al instalador y portable, revisión de origen y checklist breve de prueba. Build
  generado no significa usabilidad comprobada: registrar aparte la validación interactiva.
- Solo documentación/planificación no obliga a regenerar un binario idéntico. La primera entrega
  ejecutable será la etapa 1; cerrar su validación antes de pasar a la siguiente feature.

## Referencias de diseño

Las guías se aplican como principios a Slint; no implican migración a WinUI/XAML:

- [Microsoft: interacción por teclado](https://learn.microsoft.com/en-us/windows/apps/design/input/keyboard-interactions).
- [Microsoft: tooltips](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/tooltips).
- [Microsoft: accesibilidad](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-overview).
- [Microsoft: layouts adaptables](https://learn.microsoft.com/en-gb/windows/apps/develop/ui/layouts-with-xaml).

La diferenciación propuesta es el flujo integrado y ligero tarea → selección → entrega verificable.
No se afirma que cada función aislada sea exclusiva frente a otros exploradores.
