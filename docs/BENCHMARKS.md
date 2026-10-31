# Benchmarks de rendimiento

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
SPDX-License-Identifier: MIT

### W-07 — colores básicos 3MF, 2026-10-31

Con `scripts/build-release-guarded.ps1 -W07MeshPreviewMeasurement` se generó un 3MF sintético de
96.800 triángulos con dos materiales base y se rasterizó a 1024×768. Tras calentar ambos modos,
siete rondas alternaron el orden: mediana azul uniforme 17,518 ms y color del archivo 18,249 ms
(+4,2 %). Una sola ejecución local; no incluye carga del ZIP, parseo ni transporte aislado, y no
representa necesariamente otros modelos o equipos. Los tiempos ordenados constan en
`target/agent-out/guarded-20261031-072135-037.out.log`; compilación y ejecución supervisadas,
pico 0,87 GiB. No sustituye la revisión visual ni cierra los materiales avanzados de 3MF.

### W-07 — repetición del listado local bajo carga variable, 2026-10-20

Se repitió `cargo bench -p naygo-core --bench large_directory --locked -- --json --filesystem`
dos veces en serie sobre el código 0.5.3. Cada ejecución hizo tres pasadas por fixture de 10.000
y 100.000 archivos vacíos, luego tres cancelaciones tras 1.024 entradas. El harness retiró sus
directorios temporales; al finalizar no quedó ningún `naygo-w07-listing-*` en `%TEMP%`.

| Ejecución | 10k: primera entrada / total (mediana, ms) | 100k: primera entrada / total (mediana, ms) | Cancelación 100k (mediana, ms) |
| --- | ---: | ---: | ---: |
| 1 | 1,551 / 29,584 | 1,997 / 351,502 | 0,176 |
| 2 | 1,426 / 28,863 | 1,819 / 308,775 | 0,203 |

La primera ejecución dio, para 10k, 3,990 / 29,584; 1,551 / 28,194; y 1,325 / 33,486 ms; para
100k, 2,028 / 292,105; 1,347 / 351,502; y 1,997 / 373,314 ms (primera entrada / total).
La segunda dio, para 10k, 2,872 / 28,863; 1,426 / 25,189; y 1,319 / 29,106 ms; para 100k,
1,765 / 308,775; 1,819 / 316,838; y 1,823 / 294,648 ms. La cancelación de 10k tuvo medianas
de 0,155 y 0,140 ms en las ejecuciones 1 y 2. Las medianas externas de listado completo a 100k fueron
351,502 y 308,775 ms; su promedio (dos ejecuciones) es 330,138 ms, 41,7 % sobre la mediana externa
232,889 ms de la repetición de ruta reutilizada del 19 de octubre. Esto supera el umbral orientativo
de 15 %, pero no demuestra una regresión de código: no se controló ni se midió carga concurrente
durante cada pasada. `Win32_Processor.LoadPercentage` marcó 94 % en una consulta inmediatamente
posterior a la primera corrida y 50 % tras la segunda; son muestras puntuales, no una serie alineada
con el benchmark. La variación del sistema impide atribuir la diferencia al listado y exige repetir
con carga estable antes de optimizar.

Host: Windows 10 Home 10.0.19045, Ryzen 7 1700X, 68.652.761.088 bytes RAM, Rust 1.96.0, x86_64,
16 hilos visibles, Naygo 0.5.3. JSONL seleccionados: `target/agent-out/w07-current-repeat-20261020-run1.jsonl`
y `target/agent-out/w07-current-repeat-20261020-run2.jsonl`; la salida bruta de la segunda corrida
está en `target/agent-out/w07-current-repeat-20261020-run2.raw.txt`. La consulta puntual de carga se
conservó en `target/agent-out/w07-current-repeat-20261020-postload.json`. Las cifras miden el worker
de `spawn_listing` con consumidor que drena entradas; no incluyen modelo Slint, renderer, input ni
UNC. W-07 permanece abierto.

### W-07 — repetición controlada y comparación local de builds, 2026-10-20

Se corrió una vez cada ejecutable release del benchmark, primero el binario previo compilado el
15-10 (`large_directory-dda6aca8729e07ba.exe`) y luego el actual del 20-10
(`large_directory-13823f1841aabf21.exe`), sin Cargo ni build durante las mediciones. Ambos ejecutaron
el mismo harness `--json --filesystem`, fixtures locales nuevos y tres pasadas por tamaño. Una
muestra del sistema cada 10 s registró CPU total 16–31 % (mediana 19 %) y 44,02–44,35 GiB libres
para el binario previo; 12–29 % (mediana 19 %) y 44,36–44,38 GiB para el actual. No hubo presión de
memoria; la CPU siguió teniendo actividad ajena al benchmark.

| Build del benchmark | 100k: tres tiempos (ms) | Mediana (ms) | Cancelación tras 1.024 filas (ms) |
| --- | ---: | ---: | ---: |
| Anterior, compilado 15-10 | 255,085 / 290,891 / 261,693 | 261,693 | 0,146 / 0,175 / 0,170 |
| Actual, compilado 20-10 | 295,669 / 313,974 / 248,259 | 295,669 | 0,125 / 0,113 / 0,105 |

La mediana del binario actual queda **13,0 %** sobre la del anterior, dentro del umbral orientativo
de 15 % descrito abajo. La corrida actual también queda bajo el promedio de 330,138 ms observado
en las dos corridas variables anteriores. Como los ejecutables provienen de builds distintos y la
carga del sistema no está completamente aislada, esto no demuestra igualdad ni atribuye el delta a
un cambio de código; por ahora no se justifica optimizar solo por esta variación. Los dos fixtures
se limpiaron. Se midió el worker de listado, no Slint/Winit, renderer, entrada, UNC o disco de red.
JSONL y muestras de carga: `target/agent-out/w07-prior-controlled-20261020.jsonl`,
`target/agent-out/w07-prior-controlled-20261020-system.jsonl`,
`target/agent-out/w07-current-controlled-20261020-run3.jsonl` y
`target/agent-out/w07-current-controlled-20261020-run3-system.jsonl`. W-07 sigue abierto hasta
completar la matriz de UI/presentación y filesystem local/UNC.

### W-07 — build W-06B instalada, 2026-10-20

Tras la actualización autorizada, el proceso instalado PID 43624 (mismo SHA-256 que el Release
0.5.3) se observó durante tres ventanas de 30 s, con 16 muestras por ventana. El working set máximo
fue 57,0 MiB, la memoria privada 26,3–26,4 MiB, 370–375 handles y 25–26 hilos. Los deltas de CPU
fueron 0 / 0,015 / 0 s; se observó un solo proceso y ningún mapeo SMB. La sesión seguía bloqueada;
no hubo input, ni se controló reposo ni se midió latencia o presentación. Esta es evidencia pasiva
del build actual, no cierra W-07. JSON: `target/agent-out/w07-installed-053-w06b-20261020-passive.json`.

### W-07 — proceso instalado 0.5.3, 2026-10-20

Muestra pasiva de PID 31304, cuyo ejecutable coincide con Naygo 0.5.3 instalado. Tres ventanas
de 30 s con 16 observaciones cada una registraron pico estable de 57,7 MiB working set, 26,9 MiB
privados, 367 handles y 25 hilos; CPU delta 0,015/0,047/0,032 s. Se observó un proceso y ningún
mapeo SMB. La sesión de Windows estaba bloqueada. No se controló inactividad ni se midió entrada,
latencia de UI, presentación o sistema de archivos; es evidencia pasiva del proceso y no cierra
W-07. JSON completo: `target/agent-out/w07-installed-053-20261020-passive.json`.

La línea base reproducible del modelo se ejecuta con:

```powershell
cargo bench -p naygo-core --bench large_directory
```

Genera en memoria directorios de 10.000 y 100.000 entradas y mide construcción de vista,
aciertos del caché, orden por tamaño/fecha y filtro por nombre. No depende del disco, por lo que
sirve para detectar regresiones entre commits y comparar CPU/VM.

Para cada medición de interfaz anotar además: commit, Windows, CPU/vCPU, RAM, tipo de disco,
resolución/DPI y si es VM. Usar una carpeta local y una ruta UNC con 10.000 entradas; registrar
tiempo hasta la primera fila, tiempo hasta completar, pico de memoria y CPU en reposo durante
30 segundos. La ruta UNC debe probarse también desconectada: la ventana debe seguir respondiendo
y permitir cancelar. Ejecutar tres veces y conservar la mediana.

El benchmark no establece umbrales universales entre equipos. En CI debe compararse contra la
mediana de la misma máquina y marcar regresión si aumenta más de 15 % en dos ejecuciones seguidas.

Ese umbral es un criterio de revisión, no una garantía de que exista ya un gate automático
de benchmarks en CI. Registrar resultados reales y no inferir fluidez por pasar tests unitarios.

### Resaltado de cambios por visita — 2026-10-13

Se amplió `large_directory` para comparar la búsqueda anterior por nombre con el índice por
ruta completa. La prueba mide también el costo de construir el conjunto y verifica que ambos
métodos devuelvan el mismo número de filas. Resultado en Windows 10 Home, build 19045, Ryzen 7
1700X y 64 GiB RAM visibles; Rust 1.96.0; perfil `bench`, mismo equipo y sesión:

| Filas | Cambios | Búsqueda por nombre (promedio) | Índice por ruta (promedio) |
| ---: | ---: | ---: | ---: |
| 10.000 | 1.000 | 597,114 ms | 2,078 ms |
| 100.000 | 10.000 | no ejecutada (costo cuadrático esperado) | 24,507 ms |

La primera medición corresponde a tres repeticiones por método, con una pasada de calentamiento;
la segunda, a diez. Es un microbenchmark sintético: demuestra el costo relativo del cálculo y no
mide I/O de disco, render, ruido del equipo ni latencia de interacción. No extrapolar los tiempos
a otros equipos; el resultado observado para 10.000 filas fue aproximadamente 287 veces menor.

### Línea base W-07 — tres ejecuciones, 2026-10-13

Se ejecutó `cargo bench -p naygo-core --bench large_directory --locked` tres veces, sin tareas de
compilación paralelas. Mismo checkout (`abdef402` + cambios locales W-01–W-06C), perfil release
`bench`, calentamiento incluido por el harness antes de cada medición. Equipo: Windows 10 Home
10.0.19045, Ryzen 7 1700X (8C/16T), 63,94 GiB RAM visible; Rust 1.96.0, host
`x86_64-pc-windows-msvc`. Directorios en memoria sintéticos; rutas `C:\benchmark` ficticias.

| Escenario | 10.000 filas, mediana por operación | 100.000 filas, mediana por operación |
|---|---:|---:|
| Vista inicial + orden por nombre | 11,601 ms | 117,738 ms |
| Vista cacheada | 0,002 ms | 0,022 ms |
| Orden por tamaño | 0,206 ms | 2,243 ms |
| Filtro + orden | 12,143 ms | 120,315 ms |
| Índice de cambios por ruta completa | 1,673 ms (1.000 cambios) | 20,168 ms (10.000 cambios) |

La búsqueda de referencia por nombre para 10.000 filas y 1.000 cambios tuvo mediana de
551,720 ms. No se ejecutó a 100.000 filas porque su algoritmo cuadrático haría la prueba
innecesariamente larga; el punto W-02 ya documenta su costo relativo histórico. El harness hace
tres rondas para vista/sort/filtro y diez para el índice; la fila cacheada repite cien veces por
corrida. La tabla resume medianas de las tres ejecuciones externas, no p95 ni una garantía de frame.

Esta primera captura W-07 solo mide el modelo aislado: no incluye I/O real, recolección de lotes,
conversión de modelos Slint, renderer, CPU en reposo, memoria del proceso, procesos auxiliares ni
interacción. Faltan métricas opt-in y la matriz de UI/disco/UNC definida abajo; no atribuir estos
tiempos al explorador completo.

### Informe estructurado W-12 — 2026-10-14

`cargo bench -p naygo-core --bench large_directory --locked -- --json` emite JSON Lines: una fila
de huella (Rust, Windows, CPU, arquitectura, paralelismo y versión Naygo) y una fila por medición,
con muestras individuales, mediana y p95 sólo cuando hay al menos 20 rondas. En CI se ejecuta a
solicitud o semanalmente como informe informativo y se conserva como artifact por 30 días; no falla
el gate por fluctuaciones de runner compartido. El informe local está en
`target/agent-out/w12-benchmark-20261014.jsonl` (no versionado).
Los workflows de octubre fijan `windows-2025` y comprueban el caption del SO; `os_version` registra
caption, versión y número de build.

En este host, Rust 1.96.0, Windows 10 Home, Ryzen 7 1700X y 16 hilos visibles, una corrida del
modelo de 100.000 filas registró mediana 25,04 ms para vista inicial+sort, 2,75 ms para sort por
tamaño y 31,78 ms para filtro+sort; índice por ruta con 10.000 cambios: 24,70 ms. El dataset es
sintético en memoria.
Esta corrida usa muestras individuales y mediana exacta; sus tiempos no deben compararse directamente
con las tablas W-07/W-10A anteriores, que usaban otro agrupamiento de rondas.

### Repetición Windows W-07 — 2026-10-14

Tres ejecuciones externas nuevas, guardadas en `target/agent-out/w07-windows-repeat-20261014-193105.jsonl`,
con el mismo Ryzen 7 1700X, Windows 10 Home 19045, Rust 1.96.0 y perfil `bench`. Cada escenario de
vista/sort/filtro conserva tres muestras internas por ejecución; el índice de cambios usa diez. La
tabla muestra la mediana de las tres medianas externas, en ms:

| Escenario | 10.000 filas | 100.000 filas |
|---|---:|---:|
| Vista inicial + sort por nombre | 12,4849 | 124,9877 |
| Vista cacheada | 0,0022 | 0,0216 |
| Vista cacheada compartida | 0,0001 | 0,0001 |
| Sort por tamaño | 0,2109 | 2,4532 |
| Filtro + sort | 12,8814 | 128,6979 |
| Índice de cambios por ruta completa | 1,6403 (1.000 cambios) | 20,3729 (10.000 cambios) |

Frente a la línea base de tres ejecuciones del 13 de octubre, los escenarios de 100.000 filas varían
entre +1,0 % (índice de cambios) y +9,4 % (sort por tamaño), por debajo del umbral orientativo de
15 %; esto no constituye un gate ni una conclusión sobre fluidez. La corrida sigue siendo sintética,
sin I/O, renderer, modelos Slint, interacción, consumo de CPU/RAM, handles o red. No cierra W-07.

### Repetición Windows W-07 — 2026-10-15

Tres ejecuciones nuevas en serie del harness JSONL, mismo host Windows 10 Home, AMD64 Family 23
Model 1 Stepping 1 (AuthenticAMD), 16 hilos lógicos, Rust 1.96.0 (`ac68faa20`, 2026-05-25), Naygo
0.5.2. Cada escenario de vista/sort/filtro incluye tres muestras por corrida, la vista cacheada
100 y los índices de cambios diez. La tabla presenta la mediana de las tres medianas externas,
en ms; el detalle con samples individuales y huella está en
`target/agent-out/w07-repeat-20261015-035239-run{1,2,3}.jsonl` (no versionados).

| Escenario | 10.000 filas | 100.000 filas |
|---|---:|---:|
| Vista inicial + sort por nombre | 1,8594 | 21,3892 |
| Vista cacheada | 0,0022 | 0,0217 |
| Vista cacheada compartida | 0,0001 | 0,0001 |
| Sort por tamaño | 0,2033 | 2,4636 |
| Filtro + sort | 2,2544 | 24,1554 |
| Índice por ruta completa | 1,6109 (1.000 cambios) | 19,7888 (10.000 cambios) |

Los escenarios de 100.000 filas quedaron muy por debajo de las medianas publicadas el 14 de octubre.
No se atribuye esa diferencia a un cambio de producto: las mediciones no aislaron cambios de checkout,
perfil/artefacto ni estado de CPU entre fechas. Esta captura se conserva como una nueva observación del
harness, no como una comparación causal ni cierre de rendimiento; falta la matriz de UI, disco y UNC.

### Enumeración local real W-07 — 2026-10-15

Se agregó un modo opt-in a `large_directory` que crea archivos vacíos en un directorio temporal local
y los enumera mediante `naygo_core::spawn_listing`, el mismo worker/canal acotado que alimenta la UI.
La creación del fixture queda fuera del cronómetro; el directorio temporal se elimina al cerrar la
corrida. Por tamaño se hicieron tres pasadas sobre el mismo fixture. El consumidor captura latencia
hasta la primera entrada, hasta 1.024 entradas y hasta resultado terminal más `join` del worker.

Host: Windows 10 Home 10.0.19045 (build 19045), AMD Ryzen 7 1700X, 63,94 GiB de RAM física,
16 hilos, Rust 1.96.0 (`ac68faa20`), Naygo 0.5.2. Medianas de las tres pasadas:

| Entradas | Primera entrada | Primer lote (1.024) | Enumeración completa |
|---:|---:|---:|---:|
| 10.000 | 1,124 ms | 15,963 ms | 15,969 ms |
| 100.000 | 1,170 ms | 161,651 ms | 161,659 ms |

También se midió cancelación tras consumir 1.024 filas: las seis pasadas terminaron con resultado
terminal `Cancelled` y los workers se unieron sin panic. La latencia mediana desde cancelar el token
hasta terminar/join fue 0,0712 ms para 10k entradas y 0,0876 ms para 100k.

Los seis resultados individuales de listado y cancelación, más el fingerprint, están en
`target/agent-out/w07-filesystem-fingerprint-20261015-v2.jsonl` (no versionado). La fixture se creó y limpió
sin errores. Esto mide filesystem local + worker/canal con consumidor que drena tan pronto recibe
datos; no mide el timer/renderer/modelo Slint, interacción, memoria ni almacenamiento de red. Es
evidencia parcial W-07, no una latencia garantizada de la UI.

Para repetirlo en Windows, define el fingerprint y agrega `--filesystem`; sin ese flag el benchmark
solo ejecuta escenarios sintéticos y no crea archivos:

```powershell
$env:NAYGO_BENCH_RUSTC_VERSION = (rustc --version)
$os = Get-CimInstance Win32_OperatingSystem
$env:NAYGO_BENCH_OS_VERSION = "$($os.Caption) $($os.Version) (build $($os.BuildNumber))"
$env:NAYGO_BENCH_CPU = (Get-CimInstance Win32_Processor | ForEach-Object { $_.Name.Trim() } | Sort-Object -Unique) -join '; '
$env:NAYGO_BENCH_RAM_BYTES = [string](Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory
$env:CARGO_BUILD_JOBS = "1"
cargo bench -p naygo-core --bench large_directory --locked -- --json --filesystem
```

### Muestra pasiva del proceso instalado — 2026-10-15

El proceso instalado `naygo.exe` (PID 24072) se observó sin interacción deliberada: una ventana de
20 s registró 72,4 MiB de working set, 54,8 MiB privados, 430 handles y 33 threads, sin delta de
CPU; luego se midieron tres ventanas consecutivas de 30 s con 72,8 MiB, 54,9 MiB, 432 handles y
35 threads estables. Los deltas de CPU fueron 0,14 s, 0,03 s y 0,16 s. No había unidades SMB
mapeadas. Es una observación pasiva del proceso principal: no confirma inactividad completa, no
incluye auxiliares ni mide latencia, navegación o uso real. No cierra W-07; faltan interacción y
la matriz de disco local/UNC. Los registros estructurados están en
`target/agent-out/w07-process-snapshot-20261015.json` y
`target/agent-out/w07-process-windows-20261015.json` (no versionados). Una repetición a las
03:44 del 15 de octubre mantuvo 72,8 MiB de working set, 54,8 MiB privados, 430 handles y 33
threads durante las tres ventanas; los deltas CPU fueron 0,234/0,156/0,156 s. No había mapeos SMB.
El registro está en `target/agent-out/w07-process-20261015-034602.json` (no versionado).

Repetición después del reinicio, a las 08:00 del mismo día: PID 23856, tres ventanas de 30 s con
working set/privada estables en 41,6/40,1 MiB, 349 handles y 17 threads. Deltas CPU:
0,031/0,047/0,016 s; sin mapeos SMB. La variación frente a las observaciones anteriores no se
atribuye a una causa porque no se controlaron sesión, build ni actividad de fondo. No se confirmó
inactividad completa ni incluye procesos auxiliares. Registro:
`target/agent-out/w07-process-20261015-080019.json` (no versionado).

Nueva captura pasiva de la instancia instalada el 15 de octubre, PID 23856, tres ventanas de 30 s:
working set/privada estables en 70,2/46,9 MiB, 439 handles y 19 threads. CPU: 0/0,047/0 s;
sin mapeos SMB. El proceso seguía abierto desde las 07:08; no se interactuó con la ventana, no se
confirmó el estado de inactividad y la medición no incluye procesos auxiliares ni latencia de UI.
La diferencia con la muestra posterior al reinicio no se atribuye a una causa. Registro:
`target/agent-out/w07-process-20261015-125432.json` (no versionado).

Repetición pasiva a las 14:22 del mismo proceso: working set/privada estables en 72,9/46,9 MiB,
441 handles y 19 threads. CPU por ventanas: 0,094/0,047/0 s; sin mapeos SMB. El proceso seguía
abierto desde las 07:08 y no se interactuó con la ventana. No confirma inactividad ni mide UI,
procesos auxiliares o filesystem; la diferencia de memoria frente a las muestras anteriores no se
atribuye a una causa. Registro: `target/agent-out/w07-process-20261015-142335.json` (no versionado).

Nueva repetición pasiva a las 19:04 del mismo proceso: working set/privada estables en 46,6/47,5
MiB, 437 handles y 17 threads; CPU 0/0/0 s en ventanas de 30 s; sin mapeos SMB. La sesión seguía
abierta desde las 07:08. No se interactuó ni se confirmó inactividad; tampoco incluye auxiliares,
latencia UI o filesystem. No se atribuye la reducción de memoria frente a las muestras previas.
Registro: `target/agent-out/w07-process-20261015-190632.json` (no versionado).
La comprobación local mostró que la instancia instalada usa el EXE del 14 de septiembre
(42.060.800 bytes; SHA-256 `c78828504e8f5908a0fb47c31f542d7c52d65a84f98469c3352079809d876f8e`),
mientras que el EXE del release regenerado el 15 de octubre mide 42.767.872 bytes (SHA-256
`a28106734d5b42ef85ba7d330e7f31be2ee9e787778cb0ed9c0d27337e98cd5b`). Las capturas previas de
la instancia instalada son históricas y no caracterizan este build. El portable actual se probó
por separado con las métricas opt-in activadas y el colector confirmó fases de listado, filas,
sincronización y tick. No generó muestras de los nuevos callbacks con la automatización local.

Una captura pasiva del portable actual tomó tres ventanas de 5 s, con intervalo de 1 s, al inicio
de la sesión: 55,7 MiB de working set, 26,7 MiB privados, 343 handles y 20 threads estables; el
delta CPU observado fue 0,031 s en la primera ventana y 0 s en las dos siguientes. Sólo se observó
el proceso raíz y no había mapeos SMB. Es un smoke corto durante arranque, sin interacción ni
confirmación de inactividad; no es una línea base ni cierra W-07. JSON:
`target/agent-out/w07-portable-20261015-230955.json` (no versionado).

### Release portable 0.5.2 — reposo tras arranque, 2026-10-16

Se repitió la captura sobre el portable actual extraído en una carpeta aislada, habilitando
`NAYGO_PERF_METRICS=1` antes de iniciarlo. Windows 10 Home 10.0.19045; tres ventanas de 30 s,
intervalo de 2 s (16 muestras por ventana). La primera ventana incluye el arranque y marcó 1,953 s
de CPU; las dos ventanas siguientes marcaron 0 s cada una. Pico del proceso raíz: 55,8 MiB de
working set, 26,7 MiB privados, 344 handles y 20 threads. Sólo se observó Naygo, sin descendientes;
no había mapeos SMB. El log interno informó backend `winit-software` y métricas de listado, modelo,
sync y tick; no registró callbacks `ui.input.*`. JSON:
`target/agent-out/w07-release-20261016-idle.json`; log:
`target/agent-out/w07-portable-verify-20261016-homefix/naygo-2026-10-16.log`.

Esta muestra aporta recursos del release tras arrancar, pero no representa una sesión interactiva ni
confirma que se mantuviera inactivo durante todo el periodo. No incluye carga de navegación/preview,
auxiliares, UNC ni latencias de entrada; no cierra W-07.

### Instalación W-13 actualizada — observación pasiva, 2026-10-16

Después de reinstalar la distribución final con AccessKit, el proceso instalado PID 32448 se
observó en tres ventanas de 30 s, con muestreo cada 2 s. Sólo se observó el proceso principal;
working set máximo de 60,8 MiB en cada ventana, memoria privada de 29,4/29,3/29,3 MiB,
387/385/385 handles y 27/26/26 threads. Los deltas de CPU fueron 0/0,047/0,015 s. La muestra
no confirma inactividad, no incluye interacción, latencia UI, filesystem ni UNC, y no es comparable
causalmente con versiones/sesiones anteriores. No cierra W-07. JSON:
`target/agent-out/w07-installed-accessibility-20261016.json` (no versionado).

### Instalación W-10C actualizada — observación pasiva, 2026-10-16

Tras actualizar la instalación de usuario al release W-10C, el proceso instalado (Windows 10 Home
10.0.19045) se observó en tres ventanas de 30 s con intervalo de 2 s (16 muestras por ventana).
Solo apareció el proceso raíz, sin descendientes ni mapeos SMB. Los picos observados fueron 72,1 MiB
de working set, 41,3 MiB privados, 357 handles y 25 threads. El delta CPU del proceso/árbol fue
0,062/0,047/0,047 s por ventana. La ventana se verificó abierta y respondiendo después de la captura;
no hubo interacción deliberada ni confirmación de inactividad, por lo que esto no mide latencia ni
cierra W-07. JSON: `target/agent-out/w07-installed-w10c-20261016-0924.json` (no versionado).

**Intento de interacción automatizada, 2026-10-16:** se abrió el portable aislado sobre el fixture
de 10k con métricas activadas. `SendKeys` falló al enviar Ctrl+L; `PostMessage` informó mensajes de
teclado y clic como encolados, pero el proceso no registró callbacks `ui.input.*`. Windows mantuvo
en primer plano otra ventana y rechazó activar Naygo desde el proceso de prueba;
por eso no se enviaron teclas reales a esa ventana ajena. Las instancias aisladas cerraron con
`--shutdown` y código 0. Logs: `target/agent-out/w07-interaction-20261016-020715/`,
`w07-input2-20261016-020759/`, `w07-postinput-20261016-020837/` y
`w07-input3-20261016-020932/` y `w07-mousepost-20261016-021118/`. Esto identifica un límite del harness de automatización, no un fallo
del callback de Naygo; falta repetir con la ventana en primer plano en una sesión interactiva.

### Release portable 0.5.2 — carga de carpeta inicial 10k/100k, 2026-10-16

Se abrió por argumento CLI el corpus local de 10.000 y 100.000 archivos vacíos preparado para
W-13. En cada escenario se conservaron tres arranques estables (10 s por ejecución), se habilitó
`NAYGO_PERF_METRICS=1` antes del proceso y se cerró con `--shutdown`. Se excluyó del resumen una
ejecución de transición cuya sesión restauraba primero la carpeta del escenario anterior. Tras cada
ejecución, `workspace.json` confirmó la carpeta solicitada. El equipo fue Windows 10 Home 10.0.19045;
no se automatizó teclado ni scroll y el resumen no incluye tiempos del renderer.

La tabla presenta la mediana de los p50 de las tres ejecuciones; cada p50 interno cubre 25–67 syncs
en unos 10 s. Son métricas agregadas de inicio (la sesión tiene varios paneles), no una muestra
aislada por carpeta. El valor de `ui.tick` mide trabajo del callback del timer, no presentación de
frame ni latencia de una acción de usuario.

| Escenario | `ui.rows_build` | `ui.rows_convert` | `ui.rows_model_apply` | `ui.sync_rows` | `ui.tick` |
|---|---:|---:|---:|---:|---:|
| 10.000 archivos | 5,284 ms | 3,658 ms | 2,352 ms | 15,087 ms | 26,091 ms |
| 100.000 archivos | 47,240 ms | 35,078 ms | 29,193 ms | 118,962 ms | 139,940 ms |

En el caso de 100k, `ui.sync_rows` y `ui.tick` superan claramente el intervalo del timer de 30 ms;
`rows_build`, conversión y aplicación del modelo aumentan con el tamaño del listado. La muestra
confirma un costo alto durante la carga, aunque aún no separa el panel grande de los otros paneles ni
incluye interacción. Resúmenes por ejecución: `target/agent-out/w07-listing-release-20261016-100k/`
(`run-100k-2.log`, `run-100k-3.log`, `run-100k-5.log`, `run-10k-2.log`, `run-10k-3.log` y
`run-10k-4.log`); los registros `*-warm.log` son transiciones y quedan fuera de las medianas.

Para observar recursos, el proceso permaneció abierto sobre 100k mientras el capturador tomó tres
ventanas de 30 s (2 s de intervalo): pico de 265,7 MiB working set, 257,9 MiB privados, 347 handles
y 21 threads. Los deltas CPU fueron 48,156/29,876/29,891 s; sólo se observó el proceso raíz. Al
terminar las ventanas, `--shutdown` no cerró el proceso en 20 s y se tuvo que terminar esa instancia
aislada. Esto coincide con la alta carga de sync, pero no determina por sí solo la causa; queda como
hallazgo que requiere confirmar la respuesta de UI y el cierre bajo carga. JSON:
`target/agent-out/w07-100k-process-20261016.json`.

El código de `sync_rows` vuelve a construir y convertir todas las filas cada vez que crece la revisión
del listado; la reconciliación incremental actual reduce cambios del modelo, pero no el costo de
producir el vector completo. Es una hipótesis concreta para revisar en un posible W-10C, pendiente de
decisión de alcance. W-07 sigue abierto hasta medir primera fila por panel, interacción/renderer,
cancelación, CPU/RAM controlados y UNC.

Para repetir la captura en Windows sin interactuar con Naygo:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/capture-w07-process.ps1 `
  -WindowCount 3 -WindowSeconds 30
```

El schema v3 del JSON en `target/agent-out/` conserva las métricas del proceso raíz y muestrea el
árbol de descendientes con intervalo configurable (`-SampleIntervalSeconds`, por defecto 2 s).
Reporta procesos observados al menos una vez y picos de working set, memoria privada, handles y
threads; así detecta lectores de preview que aparecen y terminan entre los extremos de la ventana.
Procesos que viven menos que el intervalo aún pueden quedar fuera. El delta CPU suma deltas
positivos de identidades presentes en muestras consecutivas, así que omite consumo anterior a la
primera observación y posterior a la última. Las consultas más frecuentes agregan sobrecarga. El
working set agregado puede contar páginas compartidas más de una vez. Si hay varias instancias raíz,
especifica una con `-ProcessId 12345`.

El script no lanza Naygo ni habilita sus métricas internas; `NAYGO_PERF_METRICS=1` debe definirse
antes de iniciar la aplicación para capturar las fases instrumentadas. Esta recolección no sustituye
la observación de interacción ni la matriz de filesystem.

### W-10A — vista cacheada compartida, 2026-10-14

Misma máquina, Rust y perfil `bench` descritos arriba. El harness ejecutó 100 cache hits por
escenario; `vista cacheada` conserva el resultado previo que clona `Vec<usize>`, mientras que
`vista compartida cacheada` clona únicamente el `Arc<[usize]>`.

| Filas | Hit con copia (promedio) | Hit compartido (promedio) | Relación observada |
|---:|---:|---:|---:|
| 10.000 | 0,002137 ms | 0,000057 ms | ~37,5x |
| 100.000 | 0,021924 ms | 0,000061 ms | ~359x |

La vista compartida no crea un nuevo buffer ni copia los índices en cada hit; con 100.000 filas
evita copiar hasta 800.000 bytes por llamada. La llamada todavía incrementa/decrementa el contador
del `Arc`. La muestra es muy corta (100 operaciones) y el costo compartido se acerca a la
resolución del reloj; interpretar la razón como indicativa, no como un umbral estable. El tiempo
de construcción inicial del índice y el render/UI completo no cambiaron ni se miden en esta tabla.

### W-10B — sincronización Slint headless, 2026-10-15

Se ejecutó el flujo real `sync_rows` sobre `AppWindow` y `MinimalSoftwareWindow`, sin presentar ni
rasterizar la ventana. Equipo Windows 10 Home 22H2, build 19045, Ryzen 7 1700X, 16 hilos lógicos,
64 GiB RAM visibles, Rust 1.96.0, perfil optimizado `diag` sin LTO. Las filas son sintéticas; la
prueba crea 10.000 y 100.000 entradas y mide sincronización inicial, cinco syncs sin cambios,
modificación de una fila y append de una fila.

| Filas | Sync inicial | Sync sin cambios (mediana) | Modificar una fila | Append de una fila |
|---:|---:|---:|---:|---:|
| 10.000 | 32,924 ms | 0,206 ms | 3,207 ms | 38,908 ms |
| 100.000 | 333,959 ms | 1,825 ms | 31,874 ms | 403,996 ms |

La tabla muestra la mediana de tres ejecuciones externas tras incorporar deltas del watcher. La
modificación de una fila conservó su posición y actualizó solo su `RowData`: a 100.000 filas bajó de
353,568 ms en la muestra anterior a una mediana de 31,874 ms (~91 % menos). Se habilita solo cuando
el evento es `Modified` y conserva ruta, nombre, tipo y visibilidad, sin filtros de columna ni
typeahead y con sort por nombre o extensión. Los demás cambios usan la reconciliación normal. El
delta mantiene los índices de vista existentes.

La carga inicial y el append dieron medianas de 333,959 ms y 403,996 ms, frente a muestras previas
de 282,260 ms y 366,762 ms. La carga nueva varió entre 301,574 y 409,990 ms; el baseline publicado
es una sola muestra, por lo que la diferencia no demuestra por sí misma una regresión. En append,
la variación observada es ~10 %. El sync sin cambios varió ~8 %. Repetir contra una línea base
controlada antes de sacar conclusiones sobre esos tres escenarios.

La medición inicial mostró 44.031,679 ms para append con el comparador anterior. `RowData.cells`
es un `ModelRc` anidado y cada conversión crea un modelo nuevo; la igualdad por identidad marcaba
como distintas todas las filas. La comparación semántica de la fila y sus celdas redujo ese append
a 366,762 ms (~120x en esta misma muestra), emitiendo el cambio estructural real sin 100.000
pares de remove/insert. El test dirigido verifica que dos filas con modelos de celdas distintos
pero igual contenido sean iguales y que una celda diferente no lo sea.

Esta es una medición del flujo de sincronización y del modelo headless, no de frames, rasterizado,
entrada de teclado/rueda ni una sesión interactiva. No extrapolarla a latencia percibida ni dar W-07
por cerrado; falta capturar métricas con interacción real y revisar CPU/RAM y la matriz local/UNC.

### W-07 — rasterizado software headless, 2026-10-16

Se amplió la prueba manual `w10b_headless_sync_measurement` para forzar cinco redraws de la ventana
`AppWindow` en `MinimalSoftwareWindow` después de cargar cada listado. Cada frame rasteriza a un
buffer RGB de 1280×800; las filas fuera del viewport quedan virtualizadas. Las tres pasadas
secuenciales usaron Windows 10 Home 22H2/build 19045, Ryzen 7 1700X, 16 hilos, Rust 1.96.0 y el
perfil `test` por defecto (sin optimización release). La tabla muestra la mediana de tres ejecuciones;
cada `render` ya es la mediana de cinco redraws forzados en esa ejecución, en milisegundos:

| Filas | `sync_rows` inicial | Sin cambios | Modificar una fila | Append una fila | Renderer software |
|---:|---:|---:|---:|---:|---:|
| 10.000 | 233,274 | 1,610 | 37,498 | 263,609 | 72,409 |
| 100.000 | 1.823,465 | 10,943 | 300,216 | 2.398,735 | 78,535 |

La separación muestra que la carga/sync/append aumentan con el total de filas, mientras que el costo
del redraw headless cambia menos porque el `ListView` rasteriza el viewport visible. El perfil de
test no optimizado, una sola ventana, cinco redraws y ausencia de entrada física impiden usar estos
valores como latencia de release o de pantalla. No mide composición/presentación del backend Winit,
interacción ni memoria. Los logs por ejecución están en
`target/agent-out/w07-software-render-debug-run{1,2,3}.log` (no versionados). Repetir con perfil
optimizado supervisado y ventana interactiva antes de cerrar W-07 o definir objetivos.
El gate workspace posterior pasó 1.399 tests y dejó 7 ignorados; Clippy global `-D warnings`,
formato y `git diff --check` pasaron. Log supervisado:
`target/agent-out/guarded-20261016-022620-776.out.log`.

### W-07/W-10B — sync y raster software con test release, 2026-10-16

Se repitió el mismo harness sintético en perfil `release`, supervisado por
`scripts/build-release-guarded.ps1 -W07ReleaseMeasurement` (Job de 12 GiB, Cargo jobs=1, offline y
locked). Se midieron tres ejecuciones independientes en Windows 10 Home 22H2/build 19045, Ryzen 7
1700X, 16 hilos, Rust 1.96.0. Cada ejecución mide sync inicial/no-op, una fila modificada, append de
una fila y cinco redraws RGB de 1280×800 en `MinimalSoftwareWindow`. Los valores están en ms:

| Filas | Pasada | Inicial | No-op | Una fila | Append | Render p50 |
|---:|---:|---:|---:|---:|---:|---:|
| 10.000 | 1 | 46,741 | 0,262 | 4,762 | 62,536 | 3,475 |
| 10.000 | 2 | 46,398 | 0,285 | 4,595 | 53,897 | 2,767 |
| 10.000 | 3 | 50,009 | 0,252 | 4,930 | 54,386 | 2,840 |
| **10.000** | **Mediana** | **46,741** | **0,262** | **4,762** | **54,386** | **2,840** |
| 100.000 | 1 | 374,984 | 1,704 | 48,312 | 561,367 | 3,086 |
| 100.000 | 2 | 502,378 | 2,671 | 47,235 | 629,392 | 3,091 |
| 100.000 | 3 | 453,691 | 2,188 | 45,596 | 504,058 | 1,616 |
| **100.000** | **Mediana** | **453,691** | **2,188** | **47,235** | **561,367** | **3,086** |

Frente a las tres pasadas del perfil `test` no optimizado arriba, las medianas release bajan en todas
las operaciones medidas. El append anterior costaba 561 ms a 100k y no se extrapola a la latencia
percibida. Esto motivó W-10C. El renderer es headless, no incluye el backend Winit en pantalla,
eventos de entrada, I/O de disco ni corpus real; no cierra W-07. La primera ejecución compiló el
target de tests release en 36m45s y alcanzó 8,60 GiB de pico; las dos siguientes usaron el build
incremental (3,80/3,60 s, 0,27 GiB). Logs supervisados:
`target/agent-out/guarded-20261016-040256-888.*.log`,
`guarded-20261016-043956-597.*.log` y `guarded-20261016-044012-762.*.log`.
Tras agregar el modo, el `-Probe -MemoryGiB 0.25` y el `-Verify` completo del supervisor también
pasaron; el gate completo (suite y Clippy) alcanzó un pico incremental de 0,72 GiB. Log:
`target/agent-out/guarded-20261016-044459-297.*.log`.

### W-07 — rasterizado software headless release, repetición individual, 2026-10-16

Una repetición adicional del mismo test se ejecutó con el supervisor de memoria en perfil release
(ThinLTO, Cargo jobs=1, offline y locked). En Windows 10 Home 22H2/build 19045, Ryzen 7 1700X,
16 hilos lógicos y Rust 1.96.0, el test pasó en 1,20 s después de una compilación de 31 min 38 s;
el pico contable del Job fue 8,67 GiB. Es una sola pasada, no se mezcla con las medianas de las
tres ejecuciones anteriores. Los tiempos están en ms:

| Filas | Inicial | No-op | Una fila | Append | Render p50 |
|---:|---:|---:|---:|---:|---:|
| 10.000 | 28,227 | 0,157 | 2,844 | 2,216 | 1,810 |
| 100.000 | 269,938 | 1,479 | 27,949 | 21,101 | 2,301 |

`render p50` resume cinco redraws software de 1280×800 en `MinimalSoftwareWindow`. El harness usa
filas sintéticas residentes en memoria; no incluye I/O, backend Winit, presentación física ni
entrada del usuario. Aporta otra observación de rasterizado release, pero no cierra W-07 ni permite
inferir latencia interactiva. Log supervisado: `target/agent-out/guarded-20261016-225158-020.out.log`.

### W-10C — append incremental, tres corridas release, 2026-10-16

El harness se ejecutó tres veces bajo `scripts/build-release-guarded.ps1 -W07ReleaseMeasurement`,
con `MinimalSoftwareWindow`, el `sync_rows` real, Windows 10 Home 22H2/build 19045, Ryzen 7 1700X,
Rust 1.96.0 y fixtures sintéticos ya residentes en memoria. Cada fila representa el valor de una
ejecución, en ms; el renderer resume cinco redraws headless de 1280×800. La lista de 100k contiene
un archivo añadido al final con el marcador append-only; las entradas previas permanecen sin
modificar.

| Filas | Pasada | Inicial | No-op p50 | Una fila | Append | Render p50 |
|---:|---:|---:|---:|---:|---:|---:|
| 10.000 | 1 | 31,245 | 0,158 | 2,740 | 2,317 | 1,681 |
| 10.000 | 2 | 26,850 | 0,158 | 2,793 | 2,226 | 1,783 |
| 10.000 | 3 | 27,119 | 0,152 | 2,652 | 2,206 | 1,794 |
| **10.000** | **Mediana** | **27,119** | **0,158** | **2,740** | **2,226** | **1,783** |
| 100.000 | 1 | 277,379 | 1,685 | 26,130 | 21,140 | 1,662 |
| 100.000 | 2 | 261,597 | 1,515 | 26,690 | 21,741 | 1,666 |
| 100.000 | 3 | 260,742 | 1,561 | 26,177 | 21,279 | 1,659 |
| **100.000** | **Mediana** | **261,597** | **1,561** | **26,177** | **21,279** | **1,662** |

La mediana de append a 100k bajó de 561,367 ms en la captura previa a 21,279 ms (~26,4×,
96,2 % menos). El harness previo marcaba el cambio como genérico; el nuevo usa el mismo tamaño y
operación lógica, pero habilita el camino append cuando el prefijo de índices visibles sigue igual.
El caso de inserción intermedia está cubierto por test y conserva el fallback. Esta evidencia mide
solo la construcción/sincronización headless, no presentación Winit, entrada, disco, red ni memoria
de una sesión; no cierra W-07 ni reemplaza W-13. Gate workspace/clippy pasó antes del benchmark;
la compilación optimizada inicial tardó 34m02s y el Job alcanzó 8,60 GiB. Logs supervisados:
`target/agent-out/guarded-20261016-074839-962.*.log`,
`target/agent-out/guarded-20261016-082252-575.*.log` y
`target/agent-out/guarded-20261016-082302-014.*.log`.

### W-07 — callback de teclado Slint en release, 2026-10-16

El test manual `w07_headless_keyboard_callback_measurement` creó `AppWindow` headless, conectó el
callback de producción con `wire_nav_keys` y ejecutó 256 veces el callback Slint generado mediante
`AppWindow::invoke_key("DownArrow", false, false, false)`. El canal opt-in confirmó exactamente 256
muestras `ui.input.keyboard_callback`. En Windows 10 Home 22H2/build 19045, Ryzen 7 1700X, 16 hilos,
Rust 1.96.0 y perfil `release`, la duración total del callback tuvo p50 12,900 µs y máximo 183,700 µs.
El test terminó correctamente en 0,46 s; la primera compilación optimizada tardó 37m18s y alcanzó
8,60 GiB de pico contable en el Job supervisado. Log:
`target/agent-out/guarded-20261016-060837-057.*.log`.

Esta medición mide el callback de producción desde su invocación Slint y su `sync_layout` directo.
No despacha `WindowEvent::KeyPressed`, ni mide foco, Winit, entrada del SO, presentación del frame o
latencia de extremo a extremo. Se eligió esa frontera porque el árbol de controles headless no tenía
foco de teclado y no enrutaría un evento a `FilePanel`; la prueba no afirma que lo haga. Por ello es
evidencia de costo interno del handler, no cierre de W-07. Falta repetir con Naygo enfocado y entrada
real, además de renderer/presentación, CPU/RAM y matriz local/UNC.

### Métricas locales opt-in — 2026-10-13

El binario permite habilitar muestras temporales con `NAYGO_PERF_METRICS=1`. Se escriben
resúmenes en el log local al cerrar la aplicación; no hay envío remoto. La cola admite 512 eventos,
descarta si se llena y registra el total descartado. Cada tipo retiene hasta 2.048 muestras;
publica p95 desde 100 muestras y siempre informa p50, máximo, total recibido y retenido. Los tipos
son constantes del código, y las muestras no incluyen nombres, rutas ni contenido.

Instrumentación disponible: duración de tick, sincronización de filas/layout, cálculo de firma de
filas, construcción desde el estado, conversión a filas Slint y aplicación al modelo estable,
sondeo de listado,
tiempo hasta primer lote o terminal y finalización, solicitud de cancelación, construcción de
preview, carga/render de escenas 3D y espera/duración de trabajos del worker serial de packs.
`listing.cancel_requested` indica la edad del listado al pedir cancelación, no el tiempo hasta que
el worker se detiene. Los tiempos de preview se capturan en el worker e incluyen su trabajo
aislado, pero no separan todavía IPC, conversión, actualización de modelo ni presentación del
renderer. Al cierre, los contadores de preview, validación de rutas y packs muestran enviados,
iniciados, terminados, activos, pico simultáneo y rechazos; la métrica del worker serial añade
espera de cola y tiempo de ejecución.

Desde el 15 de octubre también se capturan duraciones de callbacks de teclado, clic/doble clic/clic
medio en filas y sort de columnas (`ui.input.*_callback`). Como los callbacks de fila y sort llaman
`sync_rows` o `sync_layout`, sus muestras incluyen esa actualización síncrona del modelo. El callback
de teclado incluye su `sync_layout` directo, pero el trabajo que queda para el tick se mide aparte.
Estas duraciones no incluyen el despacho anterior del SO ni la presentación del frame por el
renderer; son una separación del trabajo de callback, no latencia de extremo a extremo.

Esto instrumenta fases, pero no completa la línea base de W-07: aún deben medirse primera fila y
latencia real de interacción, CPU en reposo, memoria del proceso principal y auxiliares, handles,
disco local/UNC y escenarios de preview de la matriz. El opt-in puede introducir una pequeña
contención del canal al cierre; el event loop solo intenta inserciones no bloqueantes.

### Release portable 0.5.2 — reposo opt-in tras arranque, sesión JSON actualizada, 2026-10-16

Se repitió la captura sobre el portable regenerado tras cambiar el formato JSON de `current_dir`.
`NAYGO_PERF_METRICS=1` se definió antes del arranque. La ventana permaneció abierta durante tres
ventanas externas de 30 s (16 muestras por ventana, cada 2 s) y se cerró limpiamente con
`--shutdown`. Se observaron sólo el proceso raíz y ningún mapeo SMB. Cada ventana registró un pico
de 55,6 MiB de working set y 26,5 MiB privados; handles 379/379/378, 21 hilos y cero delta de CPU
durante cada ventana. La CPU acumulada antes de la primera muestra fue 2,641 s.

El log opt-in al cierre registró 6 listados (p50 247.099 µs, máximo 838.673 µs), 20 firmas de filas
(p50 8 µs), 10 sincronizaciones de filas (p50 2.330 µs), 4 sincronizaciones de layout
(p50 2.369 µs) y 5 ticks (p50 5.268 µs, máximo 101.493 µs). La sesión predeterminada carga varios
paneles y auxiliares; no se enviaron entradas, no se montó UNC y estos datos no aíslan un solo panel,
no miden latencia percibida ni cierran W-07. Captura externa:
`target/agent-out/w07-release-20261016-pathpersist-idle.json`; log interno:
`target/agent-out/w07-portable-verify-20261016-pathpersist/naygo-2026-10-16.log`.

### W-07 — captura pasiva de la build con pestañas accesibles, 2026-10-16

Tras instalar el build W-13 se tomaron tres ventanas de 30 s, 16 muestras cada una con intervalo
de 2 s. Se observó un solo proceso Naygo, sin mapeos SMB: pico de working set 60,5/60,6/60,6 MiB,
privado 29,2 MiB, handles 385/387/387 y 26/27/27 hilos. Delta de CPU del árbol: 0/0,05/0 s.
No se habilitó `NAYGO_PERF_METRICS` antes del arranque; no se enviaron entradas y la inactividad
externa no está controlada. La muestra no mide latencia, trabajo de UI, renderer, UNC ni auxiliares
que aparezcan entre intervalos, y no cierra W-07. JSON:
`target/agent-out/w07-installed-w13-20261016.json`.

### W-07 — repetición pasiva de la instalación actual, 2026-10-16

Se repitieron tres ventanas de 30 s sobre el PID instalado 18068, con 16 muestras por ventana e
intervalo de 2 s. Se observó un único proceso, sin mapeos SMB; working set pico de 60,9 MiB,
memoria privada de 29,2 MiB, 385 handles de pico y 24 hilos de pico. Los deltas de CPU fueron
0,094/0,141/0,047 s por ventana. La captura no controla actividad externa ni confirma reposo,
latencia o trabajo de UI; no cierra W-07. Naygo permaneció abierto y respondiendo. JSON:
`target/agent-out/w07-installed-w13-continuation-20261016.json`.

### W-07 — captura pasiva de la instalación abierta, 2026-10-19 23:51–23:53

Tres ventanas de 30 s sobre PID 50260 (build instalada, EXE SHA-256
`960ccee8a5bdfb85eaafcd8fa6112e736d4b050237e7fdfb592f41799671cadc`), con 16 muestras cada una
a intervalos de 2 s. Las tres ventanas observaron un único proceso, sin SMB: working set 62,0 MiB,
privados 43,2 MiB, 368 handles y 22 hilos. CPU delta del árbol: 0,031/0,032/0,015 s por ventana.
La captura fue pasiva: no se controló inactividad, no se habilitaron métricas internas y no se midió
latencia o entrada. La build instalada es anterior al paquete actual de Papelera; estos datos no son
una línea base causal entre versiones y no cierran W-07. El intervalo de 2 s puede omitir procesos
auxiliares de vida muy corta. JSON: `target/agent-out/w07-process-20261019-pid50260-passive.json`.

## Preview y navegación (plan pendiente)

El [plan de rutas/preview](plans/2026-09-09-rutas-preview-y-configuracion.md) propone métricas
opt-in y presupuestos todavía no implementados. Comparar en el mismo equipo/dataset al menos
tres repeticiones con preview apagado, estático y animado; registrar primer contenido/fila,
latencia de interacción, cancelación, CPU en reposo y pico de memoria de proceso principal
MÁS auxiliares. Anotar tamaño comprimido, canvas, buffers y profundidad de colas por separado.

Incluir selección rápida, A → B → A, pausa con cola llena, minimizar/bandeja, fichero corrupto,
red caída, PDF comprimido y TAR.GZ costoso. Contar hilos/procesos/handles antes y después para
detectar trabajo abandonado. El log debe ser acotado y no bloquear cuando se satura.

Los valores 64 MiB de buffers, 768 px, 15 presentaciones/s y 3 ms por recogida de listado son
referencias de diseño, no resultados. El presupuesto comunicado de hasta dos procesos de
320 MiB exige medir alternativas de menor consumo; no confundirlo con «preview de 64 MiB».
Registrar también sort, conversión de modelos y render: un límite de recogida no acota el frame.

### Release instalado tras regeneración L-01A — W-07 pasivo, 2026-10-16

Tras instalar in-place el paquete 0.5.2 más reciente y comprobar que los cuatro JSON de usuario y
el PDB anterior conservaron sus SHA-256, se capturó PID 44816 durante tres ventanas de 30 s
(16 muestras/ventana, intervalo de 2 s). El muestreador vio un proceso raíz, cero mapeos SMB,
62,8 MiB de working set, 31,1 MiB privados, 394 handles y 27 hilos como máximos. Deltas de CPU:
0,359/0,110/0,047 s. La primera ventana empezó unos 36 s después del arranque. Es observación pasiva:
no controla inactividad, no mide UI/Winit ni latencia, y puede omitir procesos más breves que el
intervalo. No cierra W-07. JSON: `target/agent-out/w07-installed-latest-20261016.json`.

### Release portable actual — smoke opt-in de inicio, 2026-10-16

El portable se abrió aislado por 10 s con `NAYGO_PERF_METRICS=1`, respondió y cerró con código 0
mediante `--shutdown`. El log tiene 16 resúmenes: 13 spans más tres contadores de workers. En
`listing`, `cancel_requested` tuvo una muestra de 5.749 µs; `complete` seis (p50 121.869 µs, máximo
1.040.802), `first_batch` seis (121.864/1.040.799 µs) y `poll` seis (35/56 µs). `pack.queue_wait`
tuvo dos muestras (9/15 µs) y `pack.run` dos (287/298 µs). Los spans UI fueron `rows_build` 8
(17/693 µs), `rows_convert` 8 (0/265 µs), `rows_model_apply` 8 (12/170 µs), `rows_signature` 22
(4/73 µs), `sync_layout` 4 (1.360/3.285 µs), `sync_rows` 11 (1.792/4.072 µs) y `tick` 6
(2.239/80.078 µs). Workers: preview 0/0/0, path validation 0/0/0 y pack 2 enviados/iniciados/
terminados, pico activo 1 y 0 rechazados. No hubo `ui.input.*`. Sólo refleja una apertura breve en
el workspace inicial, no un fixture ni una matriz; no mide renderer Winit, latencia percibida,
preview o reposo controlado, y no cierra W-07. El log está dentro de
`target/agent-out/portable-smoke-latest-20261016-142853/`.

### Release instalado activo — W-07 pasivo, 2026-10-16 14:43–14:44

Se muestreó PID 41724 durante tres ventanas de 30 s, con 16 observaciones por ventana y un
intervalo configurado de 2 s. El árbol observado tuvo un solo proceso (sin auxiliares dentro de la
ventana de muestreo), cero mapeos SMB y picos de 78,7 MiB working set, 47,7 MiB privados, 390
handles y 26 hilos. CPU del proceso/árbol por ventana: 0,125/0,093/0,032 s. Naygo respondió al
terminar la captura. No se controló la inactividad ni hubo interacción; la diferencia frente a otras
muestras no se atribuye a una causa y esto no mide latencia, renderer, filesystem ni procesos más
breves que el intervalo. No cierra W-07. JSON: `target/agent-out/w07-installed-latest-20261016-1443.json`.

### Portable aislado con fixture de 10.000 archivos — W-07, 2026-10-16

Se inició el portable actual con `NAYGO_PERF_METRICS=1` y un directorio aislado de 10.000 archivos
vacíos; no se reutilizó el workspace del usuario. En el cierre, el log interno registró 256 muestras
de construcción/conversión/aplicación de filas (`rows_build` p50 313 µs, p95 1.232 µs, máximo
15.705 µs; `rows_convert` 227/1.122/15.352 µs; `rows_model_apply` 160/287/173.890 µs), 508 firmas
de filas (32/316/412 µs), 254 sincronizaciones de filas (2.957/5.079/208.427 µs), cuatro de layout
(1.391/3.045 µs) y 249 ticks (3.060/6.898/219.616 µs). Los máximos muestran trabajo de carga y
actualización de modelos; no son latencias de entrada ni presentación.

La captura de ventana falló dos veces por timeout de `FrameArrived`; no se enviaron teclas y no hubo
muestras `ui.input.*`. Una ventana pasiva de 12 s, siete observaciones, registró 82,1 MiB de working
set, 51,4 MiB privados, 421 handles, 22 hilos, un proceso y 0,172 s de CPU; cero mapeos SMB. El
timeout impide confirmar la interacción o el primer frame. Es evidencia de carga del fixture y
telemetría, no cierra W-07 ni sustituye la matriz manual. JSON: `target/agent-out/w07-cu-20261016/process.json`;
log opt-in en `target/agent-out/w07-cu-20261016/portable/`.

### W-07 — accesibilidad de fixture limpia y límite de interacción, 2026-10-16

Se extrajo el ZIP en una carpeta portable nueva (sin `settings.json` ni `workspace.json`) y se abrió
un fixture sintético de 100 archivos mediante argumento CLI. El árbol de accesibilidad confirmó la
ruta de fixture y 25 filas visibles. Las teclas Tab y Abajo se enviaron al HWND de prueba, pero el
foco siguió reportándose en la región raíz y el log final no registró `ui.input.*`. El clic UIA en
una fila falló porque no había geometría de entrada; la captura gráfica volvió a fallar por timeout
de `FrameArrived`. La instancia se cerró con `--shutdown`; la build instalada se restauró (PID 46948,
hash igual al release). Esto valida apertura CLI y exposición parcial del árbol en un entorno limpio,
pero no interacción del usuario ni cierra W-07. Datos sintéticos en `target/agent-out/w07-clean-20261016/`.
En un segundo arranque, con el workspace de esa carpeta ya guardado, se pasó otro directorio
sintético como argumento; UIA mostró 25 filas `second-*.txt` y ninguna `synthetic-*.txt`, así que el
override de CLI sí prevalece sobre la sesión anterior. La prueba no produjo spans `ui.input.*`.

### Release instalado 0.5.2 con L-01B — W-07 pasivo, 2026-10-16 20:52 UTC

La instancia actual (PID 12532, SHA-256 `c9db8752…`) se observó pasivamente durante tres ventanas
consecutivas de ~30 s. Working set: 62,9–63,1 MiB; memoria privada: 31,3–31,6 MiB; 390 handles y
26 hilos estables. Deltas de CPU: 2,359/2,625/2,781 s por ventana. La captura UIA previa no envió
entrada, pero no se controló que la sesión estuviera inactiva; no atribuir el CPU a inactividad ni a
un trabajo específico. Tampoco se midió latencia, renderer Winit, procesos que vivan menos de la
ventana ni red. El proceso respondió al cerrar la muestra. JSON: `target/agent-out/w07-installed-l01b-20261016.json`.

### Portable aislado de 100 archivos — W-07, 2026-10-16 17:55 UTC

Se abrió una extracción limpia del portable 0.5.2 actual con `NAYGO_PERF_METRICS=1` y un fixture
sintético de 100 archivos por argumento CLI. UIA encontró 23 nombres de fixture en la lista visible.
La captura interna registró `listing.first_batch` p50 45,955 µs / máximo 1.108.163 µs y
`listing.complete` p50 45,964 µs / máximo 1.108.170 µs (12 muestras cada uno); `preview.build`
95.255 µs (una muestra). Para UI: `rows_build` p50 207 µs, `rows_convert` 118 µs,
`rows_model_apply` 33 µs, `rows_signature` 7 µs, `sync_rows` 2.781 µs (máx. 4.499),
`sync_layout` 1.808 µs (máx. 4.725) y `tick` 3.279 µs (máx. 82.979). No hubo spans
`ui.input.*` porque no se enviaron entradas.

`sky.get_window_state` agotó el tiempo de captura (`FrameArrived timed out`; segundo intento:
`window capture timed out: timed out waiting on channel`). Se detuvo la automatización según la guía
de Computer Use. No se hizo clic ni se envió teclado. El portable se cerró mediante `--shutdown`,
los cuatro JSON instalados se restauraron byte a byte desde el respaldo previo y la instalación actual
volvió a abrir, respondiendo como PID 9204 con SHA-256 `c9db8752…`. JSON de proceso:
`target/agent-out/w07-installed-l01b-20261016.json`; log opt-in del fixture:
`target/agent-out/w07-current-release-20261016/portable/naygo-2026-10-16.log`. No cierra W-07.

### Release instalado relanzado tras smoke portable — W-07 pasivo, 2026-10-16 20:57–20:59 UTC

Tras restaurar los cuatro JSON respaldados, se observó el PID 9204 (EXE `c9db8752…`) tres ventanas
de ~30 s, sin interacción deliberada ni lectura UIA durante la medición. Working set 79,1 MiB,
memoria privada 47,5 MiB, 387–389 handles, 25–26 hilos y CPU 0,016/0,031/0,016 s por ventana.
Naygo respondió al terminar. No se controló la inactividad, no mide latencia ni presentación, y no se
observó red. La muestra anterior del PID 12532 mostró CPU mayor luego de inspeccionar UIA; que esa
lectura causara la diferencia es una hipótesis, no una conclusión: los estados de ambos procesos no
se controlaron de forma idéntica. JSON: `target/agent-out/w07-installed-posttest-20261016.json`.

### Instalación vigente PID 52608 — W-07 pasivo, 2026-10-16 23:42–23:43 local

Tres ventanas de 30 s con intervalo de 2 s observaron el release instalado (EXE
`05eee5e4…`), con 16 muestras por ventana y un solo proceso. Working set máximo 71,4 MiB,
privados 41,1 MiB, 368–370 handles y 24 hilos. CPU por ventana: 0/0/0,078 s; no había mapeos
SMB. La instancia respondió al terminar. No se confirmó inactividad ni hubo interacción; la captura
no mide latencia y no cierra W-07. El máximo de árbol coincide con el proceso único; el método y sus
límite de muestreo están en `target/agent-out/w07-process-current-20261016.json`.

### W-07 — rasterizado STL antes/después, 2026-10-17

Repetición 2026-10-30, aspecto sólido predeterminado de 0.5.4 en preparación: el mismo harness
supervisado reportó una mediana de **18,483 ms** en siete rondas (18,398–20,076 ms), con 96.800
triángulos a 1024×768; pico del Job 0,87 GiB. No incluye carga, presentación ni gestos. Es una
sola ejecución en otra fecha, sin condiciones controladas entre versiones; no demuestra una
mejora ni descarta regresiones. La comparación posterior de bordes y malla se detalla abajo. Log:
`target/agent-out/guarded-20261030-013149-907.out.log`.

Medición comparativa 2026-10-30: el harness calienta los tres modos y alterna su orden en siete
rondas sobre la misma escena de 96.800 triángulos a 1024×768. Las medianas fueron sólido
**19,247 ms**, bordes **20,354 ms** y malla **20,074 ms**, respectivamente; rangos 19,010–21,710,
20,198–22,914 y 19,775–22,595 ms. Pico supervisado 0,86 GiB. Es una sola ejecución con escena
sintética: no mide carga, interacción ni presentación, y no establece un límite para otros modelos.
Log: `target/agent-out/guarded-20261030-130903-438.out.log`.

El harness `crates/core/benches/mesh_preview.rs` carga una vez una superficie sintética STL de
96.800 triángulos y mide siete rasterizados por modo a 1024×768 por ejecución. Se ejecuta aisladamente con
`scripts/build-release-guarded.ps1 -W07MeshPreviewMeasurement`, en el perfil `bench` optimizado, con
un Job de 12 GiB y un trabajo Cargo. Mismo Windows 10 Home 22H2/build 19045, Ryzen 7 1700X, Rust
1.96.0 y máquina descrita arriba. No incluye lectura de disco dentro del cronómetro, backend Winit,
presentación ni entrada física.

| Implementación | Ejecuciones | Mediana por ejecución | Rondas del harness |
|---|---:|---:|---|
| Raster previo (línea base tomada antes del cambio) | 1 | 29,771 ms | 28,121–31,912 ms |
| Pesos incrementales por fila y columna | 4 | 20,846 ms (mediana de las cuatro medianas) | 18,662–27,719 ms |

La mediana observada disminuyó 30,0 % en este fixture y equipo. Es evidencia del costo interno del
rasterizador sintético, no un compromiso de latencia en modelos reales ni en la interfaz completa.
La variación entre ejecuciones (20,595–23,290 ms por mediana) recomienda interpretar el porcentaje
como orientación. Logs supervisados locales:
`target/agent-out/guarded-20261017-122620-709.out.log`,
`target/agent-out/guarded-20261017-123016-115.out.log`,
`target/agent-out/guarded-20261017-123230-682.out.log`,
`target/agent-out/guarded-20261017-123258-633.out.log` y
`target/agent-out/guarded-20261017-133525-880.out.log` (no versionados).

### Release instalado tras corregir el crash del drop — W-07 pasivo, 2026-10-17

Se observó el Naygo 0.5.2 recién instalado (PID 23400, EXE SHA-256
`309d751b4c55759cce63b559bdaf58d81ee4befe163f2c6136eb8d100622f30d`) durante tres ventanas de
30 s, con 16 muestras por ventana e intervalo de 2 s. En Windows 10 Home 19045, Ryzen 7 1700X,
16 procesadores lógicos y 63,94 GiB RAM, los máximos por ventana fueron 83,7/83,7/83,8 MiB de
working set, 52,7 MiB privados, 370 handles y 24 hilos. Deltas de CPU: 0,063/0,015/0,047 s.
Se observó un proceso, sin mapeos SMB; Naygo respondió durante y después de la captura.

Es una observación pasiva posterior al arranque, no una sesión de reposo controlado: no se confirmó
inactividad, no mide latencia, presentación Winit ni el uso de archivos, y el intervalo puede omitir
hijos más breves. No cierra W-07. JSON:
`target/agent-out/w07-post-drop-fix-20261017.json`.

### Release E-1 instalado — W-07 pasivo, 2026-10-17 16:46–16:48 local

Tras actualizar in-place con el setup E-1 y verificar el EXE instalado (SHA-256
`97bd204bfacee564f9632658ec89308643bc07808fa15f1d0ce2890ac51ef5bc`), se observó PID 12232 durante
tres ventanas de 30 s, con 16 muestras por ventana e intervalo de 2 s. El árbol observado tuvo un
solo proceso, sin mapeos SMB. El pico del proceso/árbol fue 89,6 MiB de working set, 57,9 MiB
privados, 389 handles y 27 hilos; CPU por ventana: 0/0/0,125 s. Naygo respondió al terminar.

La captura comenzó cerca de un minuto después del arranque y después de inspeccionar UIA. No se
controló la inactividad ni se midió latencia, renderer Winit o interacción; no atribuir el consumo a
un estado de reposo ni a la inspección. El muestreo puede omitir procesos más breves que el intervalo.
No cierra W-07. JSON: `target/agent-out/w07-installed-e1-20261017-1646.json`.

### W-07 — callback de teclado Slint en release, repetición 2026-10-17

Se ejecutó `scripts/build-release-guarded.ps1 -W07InputMeasurement` de forma aislada. El harness
headless conecta `wire_nav_keys` a un `AppWindow` de producción y mide 256 llamadas
`ui.invoke_key("DownArrow")`; la instrumentación opt-in confirmó 256 spans
`ui.input.keyboard_callback`. En Windows 10 Home 22H2/build 19045, Ryzen 7 1700X, Rust 1.96.0 y
perfil `release`, la primera corrida dio p50 7,600 µs y máximo 151,300 µs. Tras compilar, tres
repeticiones secuenciales pasaron (1 aprobado; 398 filtrados cada una); sus p50 fueron 11,300,
7,800 y 8,000 µs, y sus máximos 64,300, 50,700 y 55,800 µs. La mediana de los tres p50 es
**8,000 µs**; el máximo entre las repeticiones fue **64,300 µs**. El Job de compilación alcanzó
8,69 GiB bajo el límite de 12 GiB y terminó en 26m15s; cada repetición incremental tardó cerca de
2,2 s y alcanzó 0,17 GiB.

Esta duración mide `invoke_key`/callback dentro del harness headless: no mide entrada física,
despacho de Windows/Winit, foco, presentación del renderer ni respuesta percibida. No sustituye la
matriz interactiva y no cierra W-07. Log supervisado: `target/agent-out/guarded-20261017-165730-939.out.log`
(primera compilación; no versionado) y las tres repeticiones secuenciales:
`target/agent-out/guarded-20261017-172453-728.out.log`,
`target/agent-out/guarded-20261017-172458-519.out.log` y
`target/agent-out/guarded-20261017-172503-255.out.log` (no versionados).

### Release instalado con ruta de favoritos — W-07 pasivo, 2026-10-17 19:23–19:25 local

Se observó el proceso PID 38744, iniciado a las 19:21:03, con el EXE recién generado e instalado
(SHA-256 `357ee82a0a27b2dbae5cecbe35d63a6394e97315fd384cee9db4dd951d2b1561`). Tres ventanas de
30 s, 16 muestras cada una y un intervalo de 2 s registraron un solo proceso y ningún mapeo SMB.
Pico por ventana: 57,3 MiB working set, 26,8–26,9 MiB privados, 368–370 handles y 24–25 hilos;
CPU: 0 s en las tres ventanas. La aplicación respondió después de la captura.

La muestra es pasiva y cercana al arranque: no se controló la inactividad, no mide input,
latencia/render Winit ni acceso a archivos, y el muestreo puede omitir hijos de duración menor al
intervalo. No cierra W-07. UI Automation de solo lectura sobre la misma ventana expuso 331
descendientes, 240 con nombre y 46 enfocables, sin truncar el árbol; esto es evidencia estructural
para W-13, no una prueba con Narrador. JSON: `target/agent-out/w07-installed-favorites-20261017.json`.

### Release E-5 instalado — W-07 pasivo y UIA, 2026-10-18 01:54–01:56 local

Se observó Naygo 0.5.2 recién instalada (PID 51408, EXE SHA-256
`681b99ae728c0067d45b6d1a5f2bb52c9951bb32bc012ce96c405b7e5d39159f`) durante tres ventanas de
30 s, con 16 muestras cada 2 s. Working set: 76,7–77,9 MiB; memoria privada: 45,5–46,7 MiB;
385–387 handles; 26–27 hilos; CPU delta por ventana: 0,032/0,359/0 s. Se observó un proceso,
sin mapeos SMB. La ventana respondió después de la inspección.

La captura comenzó 79 s después del arranque y no se controló inactividad, por lo que no es una
medición de reposo ni de latencia/presentación. No hubo interacción física. UI Automation de solo
lectura expuso 721 descendientes, 536 con nombre y 55 enfocables; los 55 controles enfocables tienen
nombre accesible y el árbol no se truncó. Es evidencia estructural, no aceptación con Narrador ni
teclado real. JSON: `target/agent-out/w07-installed-e5-20261018.json`.

### Repetición pasiva de E-5 instalado antes de actualizar a E-4 — 2026-10-18 04:23–04:25 local

Se capturó el mismo PID 51408, iniciado a las 01:53 y todavía ejecutando el EXE E-5 con SHA-256
`681b99ae728c0067d45b6d1a5f2bb52c9951bb32bc012ce96c405b7e5d39159f`; no es la build E-4 más
reciente de `dist/`. Tres ventanas de 30 s (16 muestras cada 2 s) dieron 81,8 MiB de working set,
50,1 MiB privados, 380 handles, 23 hilos y CPU delta 0,031/0,032/0,031 s. Se observó un proceso y
no había mapeos SMB. Las cifras permanecieron estables en las tres ventanas.

La observación fue pasiva: no se controló la inactividad ni se interactuó con la aplicación, así que
no establece un perfil de reposo controlado, latencia ni presentación. En ese momento E-4 aún no
estaba instalada; después se actualizó in-place y la app quedó abierta con el nuevo EXE. JSON:
`target/agent-out/w07-current-installed-20261018.json`.

### Release E-4 instalado — W-07 en escritorio bloqueado, 2026-10-18 04:37–04:39 local

Se observó el PID 25124, cuyo EXE instalado coincide por SHA-256 con `target/release/naygo.exe`
(`feba7919f7f308bacb8198b4abf5eb61c1d8fbfafbe5ff1171916caf92376293`). La pantalla de bloqueo se
capturó inmediatamente antes y después de tres ventanas de 30 s; el colector no interactuó con Naygo.
Cada ventana tuvo 16 muestras a intervalos de 2 s. El proceso raíz mantuvo 78,5 MiB de working set,
47,0 MiB privados, 387 handles y 27 hilos. Los deltas de CPU fueron 0,015/0/0,047 s; sólo se observó
el proceso Naygo y no había mapeos SMB.

Esta muestra aporta evidencia de recursos con la sesión de escritorio bloqueada, pero el colector no
mide input, latencia de UI, presentación Winit, navegación ni filesystem. Tampoco prueba ausencia de
trabajo fuera de las ventanas de muestreo. No cierra W-07. Registro no versionado:
`target/agent-out/w07-locked-installed-20261018.json`.

### Build W-13 instalada — W-07 pasivo y UIA, 2026-10-18 12:32–12:33 local

Tras actualizar in-place Naygo 0.5.2, se observó PID 23868 con el EXE SHA-256
`21e97c3557ce5998429c83d0820e3212e5a8c2ebdca77a27eafdc45e1f4fd831`, coincidente con Release.
UIA de solo lectura expuso 330 descendientes (239 con nombre), 46 controles enfocables, todos con
nombre; el árbol no se truncó y el proceso respondió. La consulta ocurrió con la sesión bloqueada y
los diálogos de edición cerrados, así que no verifica individualmente los campos W-13 ni sustituye
Narrador.

La captura pasiva empezó 35 s después del arranque y tomó tres ventanas de 30 s, con 16 muestras a
intervalos de 2 s. Working set: 61,9 MiB; memoria privada: 30,5–30,6 MiB; 385–392 handles;
26–28 hilos; CPU delta: 0,031/0/0,031 s. Se observó un solo proceso y ningún mapeo SMB. UIA precedió
a la captura, pero no se controló inactividad; estas cifras no son una línea base de reposo ni miden
latencia, presentación Winit o acceso a archivos. No cierra W-07. JSON: `target/agent-out/w07-w13-installed-20261018.json`.

### E-6 instalado — observación pasiva, 2026-10-18 17:07–17:08 local

El PID 41292 ejecuta el release E-6 y su EXE instalado coincide con `target/release/naygo.exe`
(`54fed15ac5585b6089a78ce7ebf6e14842b6934f7febd9f118c3bb3a14562a50`). Tres ventanas
consecutivas de 30 s, con 16 muestras cada 2 s, se mantuvieron en 78,2 MiB de working set,
47,1 MiB privados, 388 handles y 26 hilos. El delta CPU fue 0/0/0 s; sólo se observó el proceso
principal y no había mapeos SMB. No se controló la inactividad del PC, así que no se presenta como
reposo controlado ni permite atribuir consumo a un escenario de navegación. No mide latencia,
presentación Winit ni acceso a archivos y no cierra W-07. Registro no versionado:
`target/agent-out/w07-e6-passive-20261018.json`.

### Build instalada PID 48792 — observación pasiva, 2026-10-18 21:28–21:29 local

Tres ventanas consecutivas de 30 s, con 16 muestras cada 2 s, observaron un solo proceso
(`naygo`) y ningún mapeo SMB. El working set se mantuvo en 93,9 MiB, la memoria privada en
53,6 MiB, los handles en 481 y los hilos en 40. El delta de CPU fue 0,078/0/0,016 s.
El EXE instalado (`94070d7b4dea91ebb9670cfb6f63abf63d8644f06556a4ff0a2c569c2fee3746`)
no coincide con el EXE de Release local (`210901465840187a497dca452d56b0cbf843807c11c52079e0bbeca0ac1d4673`).

La captura no interactuó con la aplicación ni controló inactividad; por ello es una observación
pasiva de esta instancia anterior, no una línea base de reposo, una medición de latencia o una
validación de la build actual. No cierra W-07. Registro no versionado:
`target/agent-out/w07-current-passive-20261018.json`.

### Release 0.5.2 recién instalado — W-07 pasivo, 2026-10-18 21:35–21:37 local

El PID 51452 ejecutó el EXE instalado cuyo SHA-256 coincide con Release
(`210901465840187a497dca452d56b0cbf843807c11c52079e0bbeca0ac1d4673`). Tres ventanas de 30 s
(16 observaciones cada 2 s) registraron working set pico de 60,7/60,6/60,6 MiB, privado
29,3/29,2/29,2 MiB, handles 392/387/387 e hilos 29/28/28. Delta de CPU: 0,06/0,09/0,05 s.
Sólo se observó Naygo; no había mapeos SMB.

La captura fue pasiva, sin controlar inactividad ni enviar entrada; no mide latencia, presentación
Winit ni operaciones locales/UNC. Confirma recursos de la build actual en esas ventanas, no una
línea base de reposo controlado y no cierra W-07. Registro no versionado:
`target/agent-out/w07-current-release-installed-20261018.json`.

### Límite de medición de presentación W-07 — Slint 1.16.1

Se revisó el API y el backend exactos fijados por el lockfile. El estado `AfterRendering` de
Slint se notifica antes de presentar el back buffer; además, el renderer por software no
implementa `set_rendering_notifier` y devuelve `Unsupported`. El backend Winit-software llama
`pre_present_notify()` y luego `present_with_damage()` dentro de su ruta interna, sin exponer un
callback de aplicación después de la presentación. Por tanto, un notifier público no serviría
para cerrar la medición de presentación y no se añadirá una métrica con ese nombre. La medición
real de frames presentados requiere instrumentación externa de Windows o una modificación propia
del backend; input a callback/modelo y tiempos de render headless siguen siendo métricas distintas.
Referencias: [RenderingState en Slint 1.16.1](https://docs.rs/slint/1.16.1/slint/enum.RenderingState.html),
[renderer Winit-software 1.16.1](https://github.com/slint-ui/slint/blob/v1.16.1/internal/backends/winit/renderer/sw.rs).

### Release 0.5.3 instalado — captura pasiva W-07, 2026-10-19 00:38–00:40

El PID instalado **48444** ejecutó el EXE 0.5.3 cuyo hash coincide con Release. Tres ventanas de
30 s, con 16 observaciones cada 2 s, vieron un proceso (Naygo) y ningún mapeo SMB. Working set
pico: **76,9 MiB**; memoria privada: **45,3 MiB**; **385 handles**, **26 hilos** y deltas CPU
**0 / 0 / 0,063 s**. Los valores se mantuvieron constantes en las tres ventanas salvo el delta
CPU de la última.

Fue una captura pasiva: no se controló inactividad, no hubo entrada, no se midió latencia ni
presentación Winit, y no se accedió a rutas locales/UNC. Por ello actualiza la observación de
recursos del release 0.5.3 pero no cierra W-07. JSON: `target/agent-out/w07-0.5.3-installed-passive-20261019.json`.

### Repetición de listado local W-07 — 2026-10-19

Se repitió `cargo bench -p naygo-core --bench large_directory --locked -- --json --filesystem`
seis veces en serie, tres antes y tres después de evaluar la eliminación de una lectura atómica
duplicada de cancelación en la admisión rápida del canal. El cambio no mostró mejora medible y se
revirtió; no forma parte del Release 0.5.3. Equipo consultado por separado: Windows 10 Home
10.0.19045, AMD Ryzen 7 1700X, 63,94 GiB visibles y Rust 1.96.0. Las corridas actuales no
incluyeron todas las variables de fingerprint del harness, por lo que esta observación no se trata
como comparación controlada.

| Serie | Mediana externa de 100.000 entradas | Mediana externa de cancelación |
|---|---:|---:|
| Antes de evaluar el cambio | 208,813 ms | 0,0937 ms |
| Después de evaluar el cambio | 208,955 ms | 0,0934 ms |

Las tres medianas individuales de enumeración fueron 213,110 / 205,636 / 208,813 ms antes y
208,955 / 204,604 / 222,886 ms después. La mediana anterior de 161,659 ms (2026-10-15,
0.5.2, misma máquina documentada arriba) deja una diferencia observada aproximada de +29,2 %;
no se identificó ni aisló una causa y los releases/estados de checkout difieren. En ambas series,
las fixtures temporales se eliminaron (`temp_fixtures_remaining=0`). W-07 sigue abierto: falta
repetición bajo fingerprint completo y control de versión/estado, además de la matriz de UI,
preview, CPU/memoria y filesystem local/UNC. Resultados no versionados en
`target/agent-out/w07-filesystem-current-20261019*.jsonl` y
`target/agent-out/w07-filesystem-optimized-20261019-run{1,2,3}.jsonl`.

### W-07 — evaluación de asignación de rutas por fila, 2026-10-19

Con fingerprint completo, tres ejecuciones externas de la implementación previa dieron medianas
de 217,170 / 218,070 / 212,748 ms para 100.000 entradas; su mediana fue 217,170 ms. Una primera
alternativa con `DirEntry::file_name()` produjo 220,479 / 239,730 / 241,949 ms y se reemplazó:
añadía una copia `OsString` por fila y la serie se ralentizó durante la medición. No se atribuye la
diferencia al código porque no se midió carga de fondo por muestra.

La implementación final crea una única `PathBuf` por `DirEntry`, la usa para aplicar el filtro
existente de temporales internos y la mueve a `entry_from_dirent` para construir `Entry`. Así evita
volver a materializar el mismo path sin clonar el basename. Una ejecución de tres pasadas con el
fingerprint completo registró **231,653 / 222,403 / 301,600 ms** para 100.000 entradas. La
variación de esa ejecución es alta y no permite una conclusión de latencia; el cambio se conserva
por eliminar la segunda materialización del path en el código caliente. No se registra como una
mejora temporal demostrada.

Host: Windows 10 Home 10.0.19045, AMD Ryzen 7 1700X, 68.652.761.088 bytes RAM física, Rust 1.96.0,
Naygo 0.5.3. El conteo de temporales del fixture fue cero tras finalizar. Logs no versionados:
`target/agent-out/w07-filesystem-fingerprint-20261019-run{1,2,3}.jsonl`,
`target/agent-out/w07-filesystem-fingerprint-optimized-20261019-run{1,2,3}.jsonl` y
`target/agent-out/w07-filesystem-path-reuse-20261019.jsonl`. W-07 sigue abierto para medición con
carga controlada, interacción UI, presentación y rutas UNC.

### Repetición del listado W-07 — ruta reutilizada, tres corridas, 2026-10-19

Se repitió en serie el modo `--filesystem` del benchmark release 0.5.3, con fixtures nuevas de
10.000 y 100.000 archivos vacíos en cada corrida. Para 100.000, los tiempos totales de tres pasadas
internas y la mediana externa fueron:

| Corrida | Tiempos de listado (ms) | Mediana externa (ms) | Cancelación tras 1.024 filas (ms) |
|---|---|---:|---|
| 1 | 241,956 / 241,865 / 237,678 | 241,865 | 0,098 / 0,104 / 1,099 |
| 2 | 209,971 / 236,086 / 210,594 | 210,594 | 0,086 / 0,098 / 1,087 |
| 3 | 236,860 / 232,889 / 215,475 | 232,889 | 1,058 / 0,087 / 0,147 |

La mediana de las tres medianas externas es **232,889 ms**. La amplitud entre corridas (210,594–
241,865 ms) no permite demostrar una mejora ni una regresión del cambio de reutilización de ruta.
La cancelación alcanzó el estado terminal en las tres series; muestras aisladas rondan 1 ms, sin
convertirse en una garantía de latencia. Se verificó que no quedaran directorios
`naygo-large-directory-*` en `%TEMP%`. El harness reporta CPU, versión de SO y compilador como
`null` en este modo; se toma el fingerprint del host de la sección anterior, pero no se controló
actividad de fondo entre corridas. W-07 sigue abierto. Logs sin versionar:
`target/agent-out/w07-filesystem-path-reuse-repeat-20261019-run{1,2,3}.jsonl`.

### Release 0.5.3 en segundo plano, sin entrada física — 2026-10-19 10:32–10:34

Se observó el PID 42728 durante 46 muestras en **101,2 s**. `GetLastInputInfo` pasó de 302,9 a
404,1 s y Firefox conservó el primer plano al inicio y al final; no se enviaron entradas. Naygo
acumuló **0,172 s de CPU** (aprox. 0,17 % de un núcleo durante la ventana), promedió **78,8 MiB**
de working set y **46,8 MiB** privados, con 381 handles y 24 hilos promedio. El working set máximo
observado fue 78,8 MiB. `Win32_Processor.LoadPercentage` osciló entre **16 y 67 %**, así que el
equipo no estaba globalmente inactivo; esta captura sólo muestra el costo del proceso en segundo
plano mientras no hubo entrada física observada.

Es una muestra local y breve, sin navegación ni preview inducidos, y no mide presentación ni
latencia. No certifica reposo absoluto ni reemplaza la matriz de carpetas locales/UNC. No se atribuye
el CPU más alto de la captura pasiva anterior a una causa concreta. JSON sin versionar:
`target/agent-out/w07-idle-current-20261019.json` (SHA-256
`56c06b3e59be031f88b367ba87fb35ef0b0b0088008302b280aff7a73b52f335`).

### Release 0.5.3 con W-06D instalado — muestra pasiva, 2026-10-19 10:09

Se muestreó PID 42728 durante tres ventanas consecutivas de 30 s, con 16 observaciones por ventana.
El working set pico fue **78,4/78,5/78,7 MiB**, memoria privada **46,6/46,7/46,8 MiB**,
**385 handles** y **26 hilos**. La herramienta observó sólo Naygo, sin procesos descendientes ni
mapeos SMB. Los deltas de CPU fueron **0,89/0,81/0,98 s** por ventana de 30 s.

La captura es pasiva: no se controló actividad/inactividad, entrada de usuario, cambios de ventana,
notificaciones ni carga del sistema. Los deltas CPU no se pueden interpretar como costo en reposo ni
como regresión de W-06D. No mide latencia, presentación Winit ni rendimiento local/UNC. W-07 sigue
abierto. JSON sin versionar: `target/agent-out/w07-0.5.3-w06d-passive-20261019.json` (SHA-256
`27f419d79595c32a894d6e0f8c8aeac75ee5fd4639d99f741553764bec1fcd03`).

### Release 0.5.3 con acciones UIA de W-13 — captura pasiva, 2026-10-19 19:46–19:48 local

Se muestreó la instancia recién instalada PID 44872 en tres ventanas de 30 s, con 16 observaciones
cada una. Naygo fue el único proceso observado y no había mapeos SMB. El árbol de proceso alcanzó
61,7 MiB de working set, 30,1 MiB privados, 387 handles y 26 hilos. El CPU acumulado por ventana fue
0,094/0,063/0,015 s. La lectura UIA previa había encontrado 120 controles enfocables y cero botones
sin `Invoke`.

La captura fue pasiva: no se controló primer plano, entrada, actividad del equipo, preview ni
navegación. Es una muestra de recursos de la entrega actual, no un baseline de reposo ni una medida de
latencia, presentación Winit, filesystem o interacción. W-07 sigue abierto. JSON sin versionar en
`target/agent-out/w07-w13-current-20261019.json` (SHA-256
`d27d1f02d33fc2093c9ca5e737b6a48355c287c590a2ff0ddc39075e38ff9414`).
