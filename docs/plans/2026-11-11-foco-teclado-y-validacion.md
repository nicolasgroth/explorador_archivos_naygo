# Naygo — continuidad de foco y validación Windows

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Plan de continuación del [plan de usabilidad y confiabilidad Windows](2026-11-09-usabilidad-confiabilidad-windows.md).
Nicolás autorizó avanzar de forma autónoma. Los puntos siguientes son **estimaciones relativas**
de complejidad e incertidumbre, no presupuesto de tokens ni plazo; se ajustan al reproducir cada
caso. La aceptación nativa no se declara aprobada con pruebas simuladas.

Prioridad agregada por Nicolás: corregir el clic derecho sobre selección múltiple antes de Foco-1.
Se reprodujo la reducción a una fila; corregida y probada en Slint/controlador. La preparación Shell
comprueba selección completa y padres por PIDL, con aviso ante rechazo; una prueba obtiene el
`IContextMenu` Windows de dos archivos sin presentar ni invocar comandos. Nicolás confirmó que
instaló la build, seleccionó varios archivos y el menú mostró varias opciones; no se ha comprobado
un comprimido ni medido el hash instalado. Queda auditar costo en UI y submenús de extensiones.
Gate: 1.613 aprobadas, 9 ignoradas, Clippy/formato correctos y paridad de diez idiomas. Build
supervisado terminado y paquetes verificados en `dist`; ver hashes y límites en estado actual.

## Orden y criterios

Secuencia aprobada de continuación: controles de Operaciones y selector de carpeta de inicio,
árbol y destino OLE, y finalmente filtros y orden de vista profunda. Estimaciones provisionales
de esos cortes: 3, 5 y 5 puntos. Continúa abierta la aceptación nativa de los flujos.

1. **Foco-1: renombrado inline y diálogos superpuestos (5 puntos, provisional).** Evitar que
   una pérdida de foco causada por un diálogo confirme accidentalmente una edición pendiente,
   preservando el comportamiento existente de confirmar por Enter y por clic deliberado fuera.
   Definir el contrato al recibir un aviso, cerrar/cambiar panel, cambiar de ruta o recibir una
   actualización del listado. Conservar borrador, cursor, selección e identidad del archivo cuando
   sea aplicable; Esc debe cancelar y ↑/↓ encadenar sin dobles commits. Probar sesiones tardías,
   Unicode, validación y errores de operación. No proponer cambios al filesystem en la prueba UI.
   Evitar carreras entre el cambio del indicador de diálogo y el evento real de pérdida de foco;
   demostrar orden de eventos y superposición, no inferirlo. El contrato histórico de clic fuera
   sigue vigente salvo evidencia que justifique modificarlo.

   Vista profunda: el modelo de filas es diferente. Se detectaron operaciones contra selección
   normal y destinos de arrastre ambiguos; quedan bloqueados temporalmente con aviso. El doble
   clic usa ahora el índice visible filtrado. Para recuperar selección y F2 se requiere diseño
   propio y pruebas de identidad y reordenamiento. Avance: el cursor profundo usa la ruta y
   admite clic, flechas, Inicio/Fin, avance de página y Enter; test de controlador cubre orden
   y filtrado. Selección y operaciones siguen pendientes; falta aceptación nativa y medir
   rendimiento en listados grandes. Continuación F2: identidad y reconciliación sobre filas
   profundas, relistado en worker tras el resultado real, y protección del encadenamiento a
   hijos de una carpeta en proceso de rename. Las pruebas dirigidas cubren worker y ventana
   headless; falta aceptación nativa. Continuación: selección por ruta y operaciones de copia,
   corte, borrado, duplicado, transferencia, Bandeja y menú Shell; ZIP de una sola carpeta.
   La creación protege Enter durante el relistado y respeta una selección explícita posterior.
   Avance: arrastre interno hacia carpeta profunda, con identidad verificada y confirmación;
   falta aceptación nativa. Batch rename profundo de un mismo padre implementado y empaquetado:
   3 puntos provisionales para este bloque, por revisión del preview, recorrido y resultados;
   los puntos expresan complejidad, no tokens. Falta aceptación nativa. Restan destino
   profundo externo, ZIP de carpetas distintas, exportación, medición de rendimiento
   y aceptación nativa. Transformación de texto profunda implementada y empaquetada: 3 puntos
   provisionales por captura del lote y reconciliación de la cancelación; falta aceptación
   nativa. Exportación profunda incremental/cancelable implementada y empaquetada: 5 puntos
   provisionales por instantánea, worker, cancelación y foco; faltan aceptación nativa y medir
   rendimiento. Exportación normal incremental/cancelable implementada y empaquetada: 3 puntos
   provisionales por el uso de índices compartidos, control del menú y pruebas de coherencia;
   faltan aceptación nativa y medición de rendimiento. Renombrado en lote entre carpetas distintas
   implementado y empaquetado: 5 puntos provisionales por colisiones, planificación, vista previa
   y reconciliación; falta aceptación nativa. Los puntos no son una medida de tokens.
   Compresión profunda entre carpetas implementada y empaquetada: 5 puntos provisionales por
   rutas internas, cancelación, compatibilidad y protección frente a destinos ocupados; falta
   aceptación nativa.
   Destino externo en vista profunda implementado y empaquetado: 5 puntos provisionales por
   captura OLE, identidad de fila, revalidación tras preparación y pruebas; falta aceptación real.
   Rendimiento profundo: medición sintética debug y optimización acotada de flechas empaquetada,
   3 puntos provisionales. Persisten recorridos de filas, firma y pie; medir release/UI antes de
   atribuir una mejora perceptible y diseñar invalidación segura antes de cachear.
   Firma profunda por revisión: implementada y empaquetada, 3 puntos provisionales; falta
   medición nativa. Filas y pie continúan como bloques de rendimiento separados.
   Pie profundo: caché de contadores implementada y empaquetada, 3 puntos provisionales; solo las
   lecturas sin cambios evitan el recorrido. Falta medición nativa; filas siguen pendientes.
   Marcas parciales de vista profunda: implementadas y empaquetadas, 5 puntos provisionales por
   control de contexto, reconciliación de respaldo y pruebas. Falta medición real del frame; el
   recálculo completo sigue para cambios amplios.
   Posición después de renombrar y archivos nuevos al final: 5 puntos provisionales por
   reconciliación del resultado con el watcher, conservación de selección y scroll, y cobertura
   en ambas vistas. La preferencia anterior de cada instalación se conserva. Falta aceptación
   interactiva en Windows.
   Los 5 puntos originales siguen
   provisionales: la ampliación de vista profunda se registrará como bloque independiente al
   dimensionar los pendientes, sin equiparar puntos con consumo de tokens.

2. **Foco-2: paneles auxiliares (5 puntos, provisional).** Inventariar Bandeja, Operaciones,
   Favoritos, búsqueda y preview según su capacidad real de teclado. Reproducir qué componente
   posee foco antes/durante/después de cada overlay; preservar selección y acciones del panel que
   corresponda sin cambiar el destino al último panel Files. Distinguir controles internos con
   foco propio de paneles que no son navegables por teclado. Cubrir cambio/cierre de panel mientras
   hay modal y superposiciones; corregir solo casos reproducidos.

   Avance Bandeja: reproducida pérdida de teclado tras diálogo. La lista y los botones de la barra
   recuperan foco tras el último overlay si la Bandeja sigue activa; prueba headless cubre teclado,
   overlays superpuestos, cambio de panel y botón. Falta validación nativa y revisar el resto de
   controles/paneles auxiliares. El mini-modal de Favoritos ahora enfoca el campo al abrir;
   una prueba headless cubre crear grupo desde el menú con texto y Enter. Falta validación
   nativa y revisar la recuperación de foco y los demás controles del panel.
   Operaciones: «Cancelar todo» conserva foco tras confirmar/cancelar si sigue activo, con
   prueba headless de overlays y cambio de panel; empaquetado, 2 puntos provisionales. Faltan
   el resto de controles y aceptación nativa.
   Historial de Operaciones: acceso por teclado al detalle y recuperación por identidad
   implementados y empaquetados, 3 puntos provisionales; falta aceptación nativa.

3. **Foco-3: ventanas y diálogos nativos (3 puntos de investigación, provisional).** Revisar
   diálogos Shell, selector de carpetas y ventana Configuración: propietario, reactivación,
   cancelación y foco al regresar. Verificar qué APIs bloquean y en qué hilo; no interrumpir COM
   ni forzar cierre de ventanas ajenas. Separar pruebas headless de aceptación Windows real y
   no mover llamadas entre hilos sin validar COM, vida útil, capacidad y errores.

4. **Foco-4: aceptación de los flujos entregados (3 puntos de preparación; ejecución manual
   aparte).** Preparar datos sintéticos y lista corta reproducible para crear/borrar, submenú
   Abrir, terminal/Explorador, F2 y foco tras modal. Comprobar primero los procesos de Naygo a
   nivel de sistema: el portable usa instancia única y podría enviar su ruta a una instancia ya
   abierta. No cerrar ni modificar la sesión ajena. Registrar build por hash, resultados y límites.

5. **Foco-5: OLE, árbol/OneDrive y rendimiento (separar en bloques estimados tras auditoría).**
   Continuar U2–U4 del plan vigente: carreras de destino OLE, matriz Explorer/7-Zip/Opus y
   persistencia, salto real de árbol/OneDrive y W-07. Evitar duplicar evidencia ya registrada;
   usar fixtures y manifiestos sin datos personales. No afirmar atomicidad ni cancelación de una
   llamada COM bloqueada. La prueba previa 7-Zip y su alcance constan en `BACKLOG.md`.

## Evidencia inicial de Foco-1

El diseño histórico de F2 establece **Enter o pérdida de foco = confirmar**
([spec](../superpowers/specs/2026-06-10-f2-rename-serie-design.md)). El editor ejecuta
`rename-commit` al perder foco ([file-panel.slint](../../crates/ui-slint/ui/file-panel.slint));
el controlador valida panel y sesión y, si el nombre cambió y es válido, inicia la operación
([ops.rs](../../crates/ui-slint/src/workspace_ctrl/ops.rs)). La prueba existente de sesión evita
duplicados y callbacks tardíos, pero no demuestra el caso de aviso superpuesto.

El 2026-11-11 se ejecutó un experimento headless con `FilePanel` y `MessageModal` reales, fila y
datos sintéticos: al editar `file.txt` a `x.txt` y mostrar el aviso se observó el callback de
commit de la sesión 42 con `x.txt` (1 prueba aprobada, sin llamada al filesystem). El experimento
quedó como artefacto temporal en `target/agent-out/rename_focus_audit_20261111.rs` y fue retirado
de la suite: describe el comportamiento observado, no una expectativa deseada permanente. Esto
prueba la secuencia de componentes; **no** prueba qué avisos son alcanzables durante F2 en la app
real ni que se haya renombrado un archivo. Antes de corregir, construir una prueba que reproduzca
un flujo alcanzable y el contrato esperado, incluidos clic fuera y confirmación intencional.

Continuación: un segundo experimento usa `OpDialogs` con conflicto (`kind=2`) y confirma la misma
emisión al aparecer; pasó una prueba con fila sintética, sin ejecutar rename. El controlador
procesa operaciones en el tick y `sync_rows` refleja el diálogo de conflicto, por lo que un
conflicto tardío de una operación previa es un camino plausible durante F2. Aún falta validar la
intercalación completa controlador/UI y los préstamos de `RefCell`, así como cancelación y clic
fuera. Artefacto temporal:
`target/agent-out/rename_conflict_audit_20261111.rs`.

Se corrigió el caso de overlay en `FilePanel`: mientras el diálogo posee foco no se confirma
el borrador, que recupera foco al cerrarlo. La prueba headless con `OpDialogs` cubre cierre,
reconstrucción de filas, paleta, Enter, clic fuera, Escape y encadenamiento, sin tocar archivos.
El mismo editor conserva cursor; al reconstruirlo se usa la posición almacenada, que hoy no
refleja necesariamente el cursor real. Falta validar la intercalación completa controlador/UI,
la prueba nativa y el cambio de panel con una sesión activa; Foco-1 sigue abierto.

El 2026-11-12 se agregó `sync::tests::conflict_during_inline_rename_survives_real_sync_and_controller`:
con `AppWindow`, `WorkspaceCtrl`, `sync_rows` y callbacks F2 de producción en renderer headless,
la prueba edita un nombre, inyecta un conflicto en el controlador, sincroniza, cierra el conflicto,
continúa editando y cancela con Escape. Pasa sin renombrar archivos ni provocar panic de `RefCell`.
El conflicto se inyecta directamente: no prueba el worker, Winit ni el sistema operativo.
La guarda de teclado de `on_key` es global a cualquier sesión F2. Un cambio de panel mientras la
sesión queda activa puede bloquear teclado en el nuevo panel; permitirlo sin más deja otro F2
capaz de reemplazar la sesión y el borrador. Falta reproducir y definir ese flujo extremo a extremo,
incluidas navegación y conservación del borrador.

Continuación 2026-11-12: se reprodujo en controlador que la flecha quedaba bloqueada en el segundo
panel. La guarda se limita al panel del editor; si se solicita F2 desde otro panel se reactiva el
editor pendiente sin crear sesión ni reemplazar el borrador. La prueba integrada de `AppWindow`
inyecta un diálogo, cambia el panel activo, cierra el diálogo, navega con flecha, retorna con F2,
continúa editando y cancela con Escape. El conflicto es sintético, sin worker ni interacción Winit.
Quedan pendientes las pestañas agrupadas, navegación del panel original por otras vías y aceptación
Windows; Foco-1 sigue abierto.

Continuación de entrega agrupada: F2 sobre otro panel activa la pestaña del editor pendiente;
el indicador de selección inicial pertenece a la sesión y sobrevive al desmontaje de la pestaña.
Cambiar la carpeta del panel cancela la sesión antes de mostrar el nuevo listado; refrescar la
misma carpeta la conserva. F2 repetido mantiene sesión y borrador, y calcula la selección sobre el
texto editado, incluyendo Unicode y carpetas. Prueba de ventana headless cubre pestaña agrupada,
diálogo inyectado, selección de extensión y navegación externa. Pendiente aceptar en Winit/Windows,
ejercitar un conflicto producido por worker y resolver la posición visual ante reordenamiento:
`rename_commit` conserva la ruta fuente, pero el índice usado para pintar puede quedar desfasado.
No se declara Foco-1 cerrado.

Continuación: el panel de lista normal reconcilia la posición F2 por ruta cuando cambia la vista;
una identidad de sesión nueva invalida callbacks del editor anterior. Si la fuente aún falta del
listado, se conserva temporalmente el modelo visible y el borrador, sin adivinar el siguiente
archivo al encadenar. Al salir se reconcilian filas y metadatos diferidos. Una prueba headless con
ventana/controlador cubre inserción, reordenamiento con igual longitud, ausencia y retorno,
escritura, cancelación y callbacks tardíos. La misma prueba usa una copia real en temporales para
producir un conflicto del worker y omitirlo; no equivale a aceptación Winit/Windows. La vista
profunda usa otra fuente y no participa en esta reconciliación: queda como seguimiento específico.

Al simular en el controlador un cambio y cierre del panel de F2 se reprodujo que la sesión quedaba
asociada al panel inexistente y bloqueaba `on_key`. El cierre ahora cancela esa sesión antes de
desmontar el panel, ignora confirmaciones tardías y limpia las propiedades del editor en `sync_rows`.
El test de controlador comprueba que el archivo original queda intacto y el otro panel recibe teclas.
Este caso no prueba que la secuencia sea alcanzable con todos los diálogos de la UI ni que el cambio
de panel sin cierre esté resuelto. Queda pendiente aceptación nativa.

## Cierre de implementación 0.5.6 — 2026-11-20

Los cambios de foco de Operaciones y del selector de carpeta de inicio, el modelo estable del
árbol y los filtros y orden por hermanos de la vista profunda quedaron integrados. La prueba
de OneDrive usa un perfil temporal y directorios existentes de sólo lectura para el recorrido.
Comprueba la ruta, el cursor y la aparición de la fila en el árbol al ir de una carpeta interior
a la raíz, al refrescar y al volver, tanto en árbol común como dedicado.

La prueba `onedrive_tree_navega_refresca_y_regresa_en_windows` se ejecuta con `--ignored` y
recibe la raíz de OneDrive en `NAYGO_TEST_ONEDRIVE` y una subcarpeta existente en
`NAYGO_TEST_ONEDRIVE_START`. Los datos del perfil son temporales.

Se añadió una regresión para terminar la revelación cuando el listado ya no puede entregar
un ancestro. Este cierre de árbol y validación se acotó a 3 puntos provisionales. Falta el
recorrido visual Winit con mouse y teclado, OneDrive desconectado y la medición de Release.
La ejecución sintética de orden y filtro figura en [benchmarks](../BENCHMARKS.md).

Seguimiento del 2026-11-21. Una prueba con carpetas temporales reprodujo que **Reintentar**
recuperaba los archivos, pero el árbol seguía sin mostrar la carpeta. Se actualiza la rama
cacheada de los árboles que siguen ese panel. La regresión verifica la fila y que la visita del
historial se conserve. Este cambio y su prueba se estimaron en 2 puntos. OneDrive activo volvió
a pasar. En la 0.5.6 instalada antes del arreglo, UI Automation encontró 237 controles enfocables
con nombre. La medición Release headless de listado, teclado y vista profunda queda en
[benchmarks](../BENCHMARKS.md). Faltan OneDrive realmente desconectado y aceptación visual/Winit.

## Puertas de entrega

Trabajar por bloques; agrupar cambios relacionados antes del build. Ejecutar pruebas dirigidas y
gate supervisado (workspace, Clippy, formato y paridad de idiomas si cambian textos), actualizar
guía, backlog, changelog y grafo cuando corresponda. Tras un cambio de producto regenerar `dist`
con el supervisor y sin tareas pesadas en paralelo; validar ZIP, hashes y coincidencia del EXE.
El primer intento del build anterior se detuvo con 7,66 GiB libres; Nicolás liberó memoria y el
reintento completó, por lo que se mantiene el mínimo de 8 GiB y no se cierran programas ajenos.
Una actualización documental o experimento sin cambios de producto no exige reconstruir `dist`.
