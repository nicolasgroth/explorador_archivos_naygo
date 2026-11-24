# Decisiones tomadas durante el lote 0.5.7

Registro de las decisiones que se tomaron sin consultar al autor mientras se ejecutaba el plan
[`2026-11-21-integridad-build-y-docs.md`](../plans/2026-11-21-integridad-build-y-docs.md), entre el
21 y el 24 de noviembre de 2026. Cada una indica qué se decidió, por qué y qué cuesta si resulta
equivocada, para que el autor pueda revisarlas y deshacer las que no comparta. El estado vigente
está en [`ESTADO-ACTUAL.md`](../ESTADO-ACTUAL.md) y los pendientes en [`BACKLOG.md`](../BACKLOG.md).

## Comportamiento con archivos (las que cambian lo que ve el usuario)

| Decisión | Por qué | Costo si es incorrecta |
|---|---|---|
| Al copiar, un enlace a **archivo** se copia por contenido en vez de fallar. | No hay riesgo de borrado ni de bucle, y fallar rompería copiar un enlace a archivo seleccionado. | Un enlace a archivo anidado queda en el destino como archivo normal. |
| La guardia "destino dentro del origen" sigue el enlace raíz en Copiar y Duplicar, y no lo sigue en Mover. | El plan no previó que Copiar sigue el enlace seleccionado. | Ninguno: restaura la guardia anterior. |
| La búsqueda por contenido, las recetas y los puntos de comparación **no leen** archivos de nube solo-en-línea; los cuentan como omitidos o como cobertura incompleta. Las entregas sí los descargan, porque son una copia explícita. | Evita descargar todo OneDrive por una búsqueda. | La búsqueda por contenido no encuentra texto dentro de archivos solo-en-línea. |
| El mismo criterio para reconocer enlaces se aplicó también a recetas, puntos de comparación y entregas. | Deja un solo criterio en todo el código; en la práctica no cambia el comportamiento. | Un marcador de nube seleccionado explícitamente se descarga, igual que en el Explorador. |
| Mover una carpeta completa con un solo renombrado se lleva también una rama ilegible, intacta. | Es un renombrado atómico que no borra nada. | La rama ilegible queda en el destino y no en el origen; son los mismos datos. |
| Si el destino existe y no se puede comprobar si es el mismo archivo que el origen, el paso **falla** con cualquier política de conflicto. | En una guardia de integridad, "no se puede saber" debe detener el paso. | Con el destino abierto en exclusiva por otro programa, antes «Saltar» lo omitía y «Conservar ambos» copiaba con otro nombre; ahora se ve un fallo. Afinarlo está en el backlog. |
| Los avisos de enlace no copiado, enlace no movido y copia que no se pudo guardar se tradujeron a los 10 idiomas. El resto de los fallos del motor sigue en español. | Son resultados rutinarios y el mecanismo de códigos ya existía. | Unas pocas claves más por idioma. |
| Se **revirtió** el rechazo anticipado de «Reemplazar» cuando la carpeta contiene un enlace o una subcarpeta ilegible. | El primer intento reabría un «Reintentar» sin salida, abortaba toda la operación sin informar los demás elementos y hacía una revisión previa no cancelable. Era una mejora opcional. | «Reemplazar» en ese caso sigue copiando todo a una carpeta temporal para descartarlo: lento, sin pérdida de datos. Se perdió el test de Mover + Reemplazar con un enlace anidado. |
| El listado ya no sigue enlaces: un enlace a archivo aparece **sin tamaño**. | Seguir un enlace a un recurso de red caído bloqueaba los listados locales. | Columna Tamaño vacía y orden como 0 para esos enlaces. |
| El menú contextual nativo carga las funciones de Windows por número de ordinal, sin declarar los controles comunes versión 6 en el manifiesto. | Declararlos cambia el aspecto de los controles nativos, y los cambios visuales requieren aprobación. | Código de interoperabilidad menos estándar. |

## Rendimiento y memoria

| Decisión | Por qué | Costo si es incorrecta |
|---|---|---|
| Se investigaron y corrigieron las ráfagas de CPU en reposo dentro de la tarea de modelos persistentes. | Era el mayor hallazgo de consumo de la línea base. | La tarea se alargó. |
| El costo de un cambio en una carpeta de 100.000 entradas se atacó en la tarea de listados grandes. | Es exactamente el problema de costo por fila. | La tarea creció. |
| Se aceptaron 3,80 ms y 4,14 ms por evento con 100.000 filas, sin seguir optimizando. | Ya es una mejora de siete veces; lo que queda cambia memoria o interfaces y lo decide el autor. | Cerca de 1 ms extra por evento. |
| Las filas cambiadas se detectan con un hash de 64 bits de identidad y contenido. | La probabilidad de una colisión doble es despreciable. | Una fila desactualizada visible hasta el siguiente cambio. |

## Compilación

| Decisión | Por qué | Costo si es incorrecta |
|---|---|---|
| Las tareas de `build.rs` y del crate de interfaz generada se hicieron como una sola. | Tocan el mismo archivo; separarlas duplicaba trabajo y revisión. | Un diff más grande de revisar. |
| La línea base de release es la documentada (29 min 18 s, pico de 10,92 GiB), sin repetir ese build. | Ahorra un build de 30 minutos. | La comparación usa una cifra de otro commit. |
| No se verificó un backtrace real con `debug = "line-tables-only"`. | Habría requerido otro build largo; la documentación de rustc dice que conserva funciones y líneas. | Backtraces con menos detalle, que se notarían en el próximo crash. |
| El crate de interfaz generada se compila sin información de depuración en desarrollo. | Es código generado; baja el pico unos 0,3 GiB. | Un panic en código generado pierde el número de línea. Se revierte con una línea. |
| Los 10 minutos extra del build release quedaron documentados como no explicados. | El objetivo era bajar el pico de memoria y se cumplió. | Un release algo más lento. |

## Proceso y alcance

| Decisión | Por qué | Costo si es incorrecta |
|---|---|---|
| Se agregó `NAYGO_DEBUG_MULTI_INSTANCE=1` para lanzar instancias de prueba. Después se limitó: en ese modo la instancia no recupera reemplazos interrumpidos ni retoma operaciones pendientes. | Sin la variable no se podía medir en este PC sin tocar el Naygo instalado; sin el límite, una instancia de prueba podía dañar un «Reemplazar» en curso de la instancia normal. | Un reemplazo huérfano de una instancia de prueba queda bloqueando ese destino hasta que lo recupere una instancia normal. |
| No se ejecutó `scripts/bump.ps1`. | Su cabecera lo reserva al autor y crea el commit y el tag de la versión. | Un paso manual de un comando. |
| El menú contextual nativo quedó descrito como "implementado; pendiente de validación manual". | Nadie lo vio funcionando en vivo. | Una frase de cautela de más en las novedades. |
| Se agregó el arreglo de «Novedades», que mostraba solo la primera línea de cada viñeta. | Era un error visible y de un solo archivo. | Una ronda extra de implementación y revisión. |
| La ola de arreglos del review final incluyó, además de los cinco hallazgos importantes, cinco menores: instancias de prueba, mismo archivo, entregas, vigilante de carpetas y carga de `comctl32`. | Eran cambios chicos sobre gestos normales o sobre seguridad. | Una ola más larga. |
| Quedaron en el backlog: medir un Mover durable a USB, la barra de progreso que no llega al total con un fallo planificado, el límite de edad de la cola de acciones del Shell y el I/O del journal en el hilo de interfaz. | No afectan la integridad de los datos. | Pendientes conocidos sin resolver. |
| El review final corrió en paralelo al build, y dos builds en curso se detuvieron cuando el código cambió. | Ahorró cerca de una hora de reloj. | Unos 30 minutos de máquina perdidos. |
| La medición final de memoria se hizo sobre el mismo build de la distribución. | El CHANGELOG va embebido en el binario. | La medición llegó al final y no al cierre de la fase. |
| En la revisión de la documentación, los retoques menores se corrigieron junto con los importantes. | Eran ediciones de una línea en los mismos archivos. | Una ronda algo más larga. |
