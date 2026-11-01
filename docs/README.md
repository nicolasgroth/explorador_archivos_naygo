# Documentación de Naygo

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

La distribución local **0.5.4 de avance** se regeneró el 2026-11-01 sin instalar, firmar ni publicar.
Incluye controles de presentación 3D, prioridad de geometría, distinción del pie de panel, colores
de materiales básicos 3MF y presentación de errores fatales de operaciones. El arrastre de carpetas
desde 7-Zip pasó una prueba integrada en una rama experimental y aún no está en la distribución;
materiales 3MF avanzados y aceptación manual continúan pendientes. Ver hashes y estado
en [estado actual](ESTADO-ACTUAL.md).

La instalación local **0.5.3** quedó actualizada al release disponible el 20 de octubre
(EXE SHA-256 `868a69dec0f6f79d0e91e54326eb9762cd745921467dd45a72fd5a4000bd2b5a`).
La actualización preservó los cuatro JSON de usuario y el PDB previo; sus hashes se registran en
[`VALIDACION-INSTALACION-W06B-2026-10-20.md`](VALIDACION-INSTALACION-W06B-2026-10-20.md). El setup
y el ZIP de ese momento no tenían firma Authenticode. El 2026-11-01 sí se hizo la prueba
interactiva de 7-Zip descrita arriba, en un binario experimental separado.
La comparación W-07 controlada del 20 de octubre dio medianas 100k de 261,7 ms para el binario
previo y 295,7 ms para el actual (+13,0 %), bajo el umbral orientativo de 15 %. Los dos runs tuvieron
CPU total muestreada de 12–31 % y más de 44 GiB libres; por ser builds distintos y con actividad de
fondo, el delta no se atribuye al código ni justifica optimizar aún. Faltan UI/Winit, presentación y
matriz local/UNC. La evidencia y JSONL están en [`BENCHMARKS.md`](BENCHMARKS.md).
UI Automation pasiva del build actual encontró 356 descendientes, 120/120 controles enfocables con
nombre, 69 acciones `Invoke` y cero botones sin ese patrón. Narrador y recorrido manual de diálogos
siguen pendientes. La evidencia de instalación, preservación y UIA está en
[`VALIDACION-INSTALACION-W06B-2026-10-20.md`](VALIDACION-INSTALACION-W06B-2026-10-20.md) y
[`ESTADO-ACTUAL.md`](ESTADO-ACTUAL.md).

## Orden de lectura para desarrollar

1. [Estado local vigente](ESTADO-ACTUAL.md): base, comprobaciones y límites.
2. [Backlog](BACKLOG.md): pendientes aprobados y cronología de cierres.
3. [Arquitectura](ARQUITECTURA.md): responsabilidades, contratos y puntos de extensión.
4. [Plan de rutas, preview y configuración](plans/2026-09-09-rutas-preview-y-configuracion.md):
   avances por bloque y mejoras pendientes; no sustituye las capacidades existentes.
5. [Mejoras por bloques para GPT-6 Luna](plans/2026-10-13-mejoras-y-ejecucion-luna.md):
   W-01–W-06E implementados, probados y empaquetados. W-07 tiene baseline sintético, mediciones
   de filesystem local y rasterizado headless debug/release; siguen pendientes interacción real,
   presentación Winit, matriz local/UNC y consumo en reposo. La instrumentación opt-in cubre tick,
   callbacks, fases de sincronización UI, listados, preview, validación de rutas y cola de packs; la
   prueba integrada del colector, suite del workspace, Clippy, formato, catálogos y build supervisado
   pasan. RP-6 y aceptación siguen pendientes. M-01 limpió los avisos Clippy y quedó empaquetado.
   W-08 tiene límites por poll/canal y presupuesto global rotativo probados y empaquetados.
   Falta medir primera fila, p95 de interacción y tiempo total frente a la base anterior.
   W-09 añade admisión compartida y coalescing de solicitudes; pasó pruebas y quedó empaquetado.
   W-10A comparte índices de vista entre cache y consumidores; suite, Clippy y build supervisado
   pasaron, con microbenchmark favorable a 100.000 filas. Falta medir el frame UI completo.
   W-11A reduce a una carga el parse del PDF y limita texto/páginas; suites, Clippy y build
   supervisado pasaron, con hashes de distribución verificados. Aceptación instalada y medición
   con corpus PDF siguen pendientes. W-11B añade límites ZIP/TAR/GZIP y pasa 28 pruebas de
   preview, suite workspace, Clippy y build supervisado; aceptación instalada continúa pendiente.
   W-11C preserva codificación en cortes UTF-8/16 y evita duplicar el buffer SVG al inspeccionar
   su canvas; estrés sintético, suite, Clippy y distribución supervisada pasan. El corpus real y
   comparación de latencia siguen pendientes en W-07. W-12 fija Rust 1.96.0, `--locked`, jobs=1
   y acciones externas a SHAs completos; el gate valida que sus referencias sigan inmutables.
   JSONL de benchmarks y smoke headless quedaron configurados. La consulta remota más reciente solo
   muestra corridas de septiembre y no cubre el workflow local actualizado, pendiente de Windows
   Server 2025.
   W-14 prepara Authenticode para ambos ejecutables; el empaquetador rechaza firmas inválidas, pero
   aprobación/configuración SignPath y la primera firma real siguen pendientes.
   La repetición sintética W-07 del 14 de octubre quedó dentro del umbral comparativo orientativo;
   la repetición optimizada headless del 16 de octubre midió sync, append y raster. El callback de
   teclado tuvo otra medición release el 17 de octubre: tres repeticiones de 256 invocaciones dieron
   p50 11,3/7,8/8,0 ms (mediana 8,0 ms) y máximo observado 64,3 ms. Es el costo dentro del callback
   Slint, no entrada física, Winit ni presentación; faltan UI interactiva y renderer.
   Las capturas pasivas anteriores usaron builds distintos y no caracterizan la entrega actual. El
   portable previo se abrió con métricas opt-in y cerró con `--shutdown`, sin muestras `ui.input.*`
   de entrada real. La prueba con 10.000 archivos sintéticos produjo métricas de carga, pero dos
   capturas de ventana agotaron el tiempo y no se enviaron teclas; W-07 sigue abierto. La versión con
   pestañas accesibles expuso 312 descendientes UIA. Una instalación histórica incluyó recuperación
   de Replace tras terminación abrupta. La actualización vigente del 2026-10-17 incluye la ruta
   abreviada junto a cada favorito; el EXE instalado está abierto y responde como PID 38744, SHA-256
   `357ee82a0a27b2dbae5cecbe35d63a6394e97315fd384cee9db4dd951d2b1561`. Los hashes de portable y
   setup están en [Validación de favoritos](VALIDACION-FAVORITOS-2026-10-17.md); el gate completo
   pasó 1.447 pruebas (8 ignoradas), Clippy `-D warnings`, formato y empaquetado supervisado.
   La captura pasiva
   histórica posterior al smoke, PID 9204, midió durante 3×30 s 79,1 MiB de working set,
   47,5 MiB privados, 387–389 handles, 25–26 hilos y 0,016/0,031/0,016 s CPU; no se controló
   inactividad ni hubo interacción. Una muestra previa del PID 12532 registró más CPU después de
   la inspección UIA, sin causa aislada. La hoja de aceptación W-13 para el release 0.5.3 conserva la matriz manual
   pendiente. La
   prioridad vigente es cerrar Windows antes de retomar hitos Linux.
6. [Validación del visor STL/3MF y distribución Windows, 2026-10-17](VALIDACION-VISOR-3D-2026-10-17.md):
   dirección de giro, tooltip de controles, rasterización y build instalado.
7. [Corrección de la columna Extensión para carpetas y distribución Windows, 2026-10-17](VALIDACION-BUILD-2026-10-17-E1.md).
8. [Preservación JSON de rutas POSIX y distribución Windows, 2026-10-17](VALIDACION-BUILD-2026-10-17-L01A-JSON.md):
   round-trip nativo pendiente de Linux; la distribución 0.5.3 quedó instalada y abierta en Windows.
9. [Plan Linux Ubuntu LTS y derivados](plans/2026-10-13-linux-ubuntu-luna.md):
   x86_64, capacidades e hitos; L-00 tiene decisiones y dependencia runtime documentadas. El binding
   GLib local pasa typecheck aislado; falta entorno Linux nativo, validar degradación segura, notices del paquete real y
   compatibilidad del menú en GNOME/Wayland. La revisión de autoría/licencias y la verificación de
   firmas del runtime Ayatana están en [Validación L-00](VALIDACION-L00-AUDITORIA-2026-10-17.md).
10. [Captura W-07 y UI Automation de la distribución instalada, 2026-10-17](VALIDACION-FAVORITOS-2026-10-17.md).
11. [Typecheck cruzado del binding Linux GLib y build Windows, 2026-10-17](VALIDACION-L03-CROSS-2026-10-17.md):
    la UI Linux completa también pasa check/Clippy cruzados desde Windows con Fontconfig dinámico;
    no valida ABI/runtime ni integración con el escritorio.
12. [Favoritos globales o propios por espacio, gate y build Windows, 2026-10-18](VALIDACION-E4-FAVORITOS-2026-10-18.md):
    migración de espacios v1 validada.
13. [Organización y distribución Windows de la toolbar, 2026-10-18](VALIDACION-TOOLBAR-2026-10-18.md):
    reordenación por grupos; falta aceptación visual en pantalla.
14. [Selector rápido de espacios, gate y build Windows, 2026-10-18](VALIDACION-E4-SELECTOR-2026-10-18.md):
    primera versión con recientes de sesión; el catálogo persistente y su gate están en el informe siguiente.
15. [Catálogo persistente de recientes, gate y build Windows, 2026-10-18](VALIDACION-E4-CATALOGO-2026-10-18.md):
    settings antiguos compatibles, distribución instalada y UIA inspeccionada; aceptación visual pendiente.
16. [Corrección de errores ReadDir y build Windows, 2026-10-18](VALIDACION-READDIR-2026-10-18.md):
    gate completo, checks cruzados `core`/`platform` para Linux y paquetes locales verificados; no instalados.
17. [Corrección del ciclo de vida del tray Linux y build Windows, 2026-10-18](VALIDACION-L03-CROSS-2026-10-17.md):
    typecheck/Clippy del harness y gate Windows aprobados; distribución regenerada y verificada sin instalar.
18. [L-05: capacidad de elevación sólo en Windows y build Windows, 2026-10-18](VALIDACION-L05-ADMIN-2026-10-18.md):
    1.453 pruebas (8 ignoradas), Clippy, formato y paquete supervisado con hashes verificados.
19. [Revisión UI Automation de la instancia abierta, 2026-10-18](VALIDACION-UIA-2026-10-18.md):
    un `Edit` enfocable sin nombre en un binario anterior; el editor de texto del preview ya tiene
    etiqueta localizada; 1.455 pruebas y distribución pasaron, falta repetir UIA/Narrador con sesión
    desbloqueada.
20. [Accesibilidad de toolbar y unidades, 2026-10-18](VALIDACION-W13-TOOLBAR-2026-10-18.md):
    roles, nombres y acciones accesibles en botones personalizados; gate completo aprobado, queda
    comprobación real con Narrador y UIA.
21. [L-05: abrir con el escritorio Linux por defecto, 2026-10-18](VALIDACION-L05-OPEN-2026-10-18.md):
    argv sin shell y rutas POSIX conservadas; falta ejecutar el flujo en Ubuntu 26.04.
22. [L-01B: renombrado seguro del destino en conflicto, 2026-10-18](VALIDACION-L01B-RENAME-2026-10-18.md):
    gate y distribución Windows verificados; la prueba Unix se compiló cruzada, queda ejecutarla en Linux nativo.
23. [W-17: renombrar sin reemplazar y build Windows, 2026-10-18](VALIDACION-W17-RENAME-2026-10-18.md):
    carrera de destino cubierta, cambio de mayúsculas/minúsculas Windows conservado; gate, check Linux cruzado
    y distribución supervisada aprobados. Build no instalada.
24. [E-6: toolbar adaptable y Espacios separado de Disposiciones, 2026-10-18](VALIDACION-E6-TOOLBAR-RESPONSIVE-2026-10-18.md):
    gate completo y check/Clippy Linux cruzados aprobados; distribución Windows regenerada y verificada.
    La aceptación visual y UIA/Narrador siguen pendientes.
25. [L-08 y build/instalación Windows 0.5.3, 2026-10-19](VALIDACION-L08-INSTANCE-2026-10-19.md):
    gate Windows y compilación cruzada Linux aprobados; runtime Linux nativo pendiente por WSL no disponible.
26. [Comparación UTF-16 de rutas de Papelera Windows, 2026-10-19](VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md):
    regresión reproducida y corregida; gate y build supervisado aprobados. Build aún no instalada.

## Uso del producto

- [Guía de usuario](GUIA-DE-USUARIO.md)
- [Entregas](ENTREGAS.md), [espacios](ESPACIOS.md), [consultas](BUSQUEDAS-GUARDADAS.md)
- [Puntos de comparación](PUNTOS-DE-COMPARACION.md), [recetas](RECETAS.md), [reintentos](REINTENTOS.md)
- [Idiomas](AGREGAR-IDIOMA.md)

## Construcción y calidad

- [Compilar y empaquetar](BUILD.md)
- [Auditoría de espacio y compilación de menor consumo](VALIDACION-BUILD-2026-09-14.md)
- [Build supervisado de distribución, 2026-10-13](VALIDACION-BUILD-2026-10-13.md)
- [Build supervisado y métricas W-07, 2026-10-14](VALIDACION-BUILD-2026-10-14.md)
- [Validación del build 0.5.2, 2026-10-15](VALIDACION-BUILD-2026-10-15.md)
- [Validación del build 0.5.2 y smoke portable, 2026-10-16](VALIDACION-BUILD-2026-10-16.md)
- [Hoja de aceptación Windows 0.5.2, actualizada 2026-10-16](VALIDACION-WINDOWS-0.5.2-2026-10-15.md)
- [W-06B: lectura de portapapeles al pegar fuera del hilo UI, 2026-10-20](VALIDACION-W06B-PASTE-CLIPBOARD-2026-10-20.md)
- [Distribución, símbolos y pruebas aisladas](DISTRIBUTION.md)
- [Code signing policy (política de firma de código) y estado SignPath](POLITICA-DE-FIRMA.md)
- [Pruebas automáticas y manuales](PRUEBAS.md)
- [Mediciones de rendimiento](BENCHMARKS.md)
- [Validación de 0.5.2: selección, creación y refresco](VALIDACION-0.5.2.md)
- [Validación de 0.5.3, gate e instalación Windows, 2026-10-19](VALIDACION-0.5.3-2026-10-19.md)
- [Validación RP-5B: modo económico y retención del preview](VALIDACION-RP5B-2026-09-10.md)
- [Validación RP-4B2: texto/3D aislados y memoria temporal](VALIDACION-RP4B2-2026-09-14.md)
- [Validación de rutas RP-1 del 9 de septiembre](VALIDACION-2026-09-09.md)
- [Validación de ruta superior RP-2 del 9 de septiembre](VALIDACION-RP2-2026-09-09.md)
- [Validación de temas y configuración RP-3 del 9 de septiembre](VALIDACION-RP3-2026-09-09.md)
- [Validación de cancelación y concurrencia RP-4A del 9 de septiembre](VALIDACION-RP4A-2026-09-09.md)
- [Validación de lectores aislados RP-4B1 del 9 de septiembre](VALIDACION-RP4B1-2026-09-09.md)
- [Validación de animación de imágenes RP-5A del 9 de septiembre](VALIDACION-RP5A-2026-09-09.md)
- [Validación de la entrega del 7 de septiembre](VALIDACION-2026-09-07.md)
- [Validaciones previas de 0.5.1](VALIDACION-0.5.1.md)
- [Protección de main](PROTEGER-RAMA-MAIN.md)

## Cómo interpretar y mantener estos documentos

Las guías describen prestaciones disponibles; los planes describen trabajo futuro y sus
criterios de aceptación. Los informes de validación son registros fechados e inmutables en
su significado: no sustituir sus hashes o resultados por los de otra entrega.

`superpowers/`, `migracion-slint/` y `auditoria/` conservan diseños/revisiones históricos.
No ejecutar sus pasos como instrucciones actuales sin contrastarlos con el estado vigente.
El [plan de septiembre anterior](plans/2026-09-04-usabilidad-y-flujos-de-trabajo.md) mantiene
sus pendientes de aceptación; no quedó anulado por el nuevo plan.

Después de una funcionalidad: comentar decisiones y límites en el módulo afectado, actualizar
su guía, marcar el bloque en el backlog y registrar la validación. Actualizar `CHANGELOG.md`
cuando cambie el producto (se incorpora en Acerca de); no anunciar futuras prestaciones como
entregadas. Una actualización sólo documental no exige alterar la versión o reconstruir el EXE.
