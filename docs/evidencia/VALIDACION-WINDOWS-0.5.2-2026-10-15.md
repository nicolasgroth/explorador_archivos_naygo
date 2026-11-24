# Hoja de aceptación Windows — Naygo 0.5.2, actualizada 2026-10-17

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Este registro sirve para completar W-07/W-13 en Windows 10/11 con una copia aislada y archivos
de prueba. Incluye algunos resultados automáticos locales; los casos manuales siguen pendientes.
El gate automatizado y la integridad del ZIP se registran en
[la validación E-1 del build](VALIDACION-BUILD-2026-10-17-E1.md); no reemplazan esta matriz.

## Entrega que se prueba

| Artefacto | Tamaño | SHA-256 | Estado |
| --- | ---: | --- | --- |
| `Naygo-0.5.2-portable.zip` | 14.364.941 bytes | `ab8731d4eaa99c224ac49ce29def724aa50386b2e873d1aceb6170ca07858017` | Distribución local L-11/E-3 del 2026-10-17; íntegro, sin PDB; no se ejecutó en paralelo con la instalación |
| `Naygo-0.5.2-setup.exe` | 66.367.927 bytes | `d101e0d1a62d2a263068d5c7975e091f5d9232de5f73e6ca24d59564bbc531e0` | Actualización in-place del 2026-10-17, código 0; Authenticode `NotSigned` |
| `naygo.exe` release/portable | 43.388.928 bytes | `ed1f349116181dc60d3ccc4fa5dbb6ea6969e4c4816145cfe75d6c55ac278a9e` | Coincide con el EXE contenido en el ZIP y con el instalado |
| `naygo.exe` instalado | 43.388.928 bytes | `ed1f349116181dc60d3ccc4fa5dbb6ea6969e4c4816145cfe75d6c55ac278a9e` | PID 56616, iniciado 21:16:30, responde; UIA expuso 691 descendientes, 514 con nombre, 55 enfocables; inspección estructural, no aceptación manual |

## Entorno

| Campo | Valor |
| --- | --- |
| Fecha/hora local | 2026-10-17; distribución L-11/E-3 instalada in-place después de regenerarse |
| Windows (10/11), edición y build | Windows 10 Home, 10.0.19045 |
| Máquina o VM | Equipo local |
| CPU, RAM y almacenamiento | Ryzen 7 1700X; 63,94 GiB RAM; almacenamiento pendiente |
| Escala/DPI | Pendiente |
| Artefacto probado (portable/setup) | El setup de `dist/` actualizó la instalación existente; el portable no se ejecutó por separado |
| Instalación limpia o actualización | Actualización in-place autorizada; setup con código 0 y hash instalado igual al release. No es una instalación aislada |
| Resultado global | Setup y arranque comprobados; W-07 interactivo y W-13 manual siguen pendientes |

## Casos manuales

Marca cada caso como `OK`, `Falla`, `No aplica` o `Pendiente`; agrega la evidencia y devuelve cada
fallo al bloque correspondiente antes de cerrar W-13.

| Caso | Qué comprobar | Resultado / evidencia |
| --- | --- | --- |
| Instalar aislado | Instalar el setup en VM o cuenta de prueba; abrir, cerrar y desinstalar sin afectar datos personales. | Pendiente: la prueba de hoy fue una actualización in-place de la instalación existente, no una instalación aislada |
| Portable | Extraer a una carpeta de prueba; abrir y cerrar; confirmar que no crea ni requiere instalación. | Parcial histórico: el ZIP anterior `625e3a3b…f7afd82` abrió/cerró aislado. El ZIP final con AccessKit está verificado, pero no se inició mientras Naygo instalado permanece abierto |
| Actualización | Si existe una versión previa de prueba, actualizar y confirmar que conserva preferencias y datos. | Parcial: setup L-11 terminó con código 0 y dejó el EXE instalado idéntico al release. PID 56616 responde; UIA inspeccionó su árbol sin truncarlo. `settings.json`, `workspace.json`, `favorites.json`, `templates.json` y el PDB preexistente conservaron sus hashes; evidencia en `VALIDACION-CIERRE-SEGURO-2026-10-17.md`. No es instalación aislada |
| Arranque/rutas | Abrir desde acceso directo y con carpeta inicial; probar ruta inexistente y recuperar sin bloquear la UI. | Pendiente |
| Paneles | Navegar, cambiar panel activo, dividir/cerrar/reordenar y restaurar layout/sesión. | Pendiente |
| Teclado | Typeahead, selección, navegación, atajos, edición de ruta, AltGr/IME y Escape/Enter. | Pendiente |
| Listados grandes | Usar carpetas de prueba de 10k/100k entradas; observar primera respuesta, scroll, sort, filtro y cancelación. | Pendiente |
| Copiar/mover/renombrar | Probar entre paneles, conflicto, homónimos, cancelar y refrescar sin pérdida ni duplicación. | Pendiente |
| Extensión en carpetas | En `extensions/` del fixture, confirmar que `1.The Buddha` y `2.3DBenchy` muestran «Carpeta», se agrupan en filtro/conteo, orden ascendente/descendente y CSV; `piece.stl` sigue mostrando `stl` y `no-extension` conserva la categoría vacía. | Pendiente en la L-11 instalada: E-1 está incluido; falta aceptación manual. El fixture sintético se puede generar con el script indicado abajo |
| Arrastre desde 7-Zip | Generar con `scripts/prepare-e2-7zip-fixture.ps1`; abrir `e2-drop-fixture.zip` en 7-Zip y arrastrar la carpeta `E2 fixture` al destino vacío. Confirmar que Naygo no se cierra, aparecen `sample.obj` y `readme.txt` y el OBJ coincide con el SHA-256 del `LEEME.txt`. Repetir arrastrando el OBJ solo. | E-2 está incluido en la L-11 instalada. Falta probar el fixture desde la ventana GUI de 7-Zip; la extracción por CLI no sustituye esa aceptación. El log histórico confirma el cierre en la build anterior, no si el OBJ alcanzó a copiarse. |
| Papelera | En una carpeta temporal, reciclar/restaurar archivo, lote parcial y junction con destino centinela intacto. | Pendiente |
| Preview | PNG/JPEG/SVG, GIF/WebP/APNG animado, PDF, ZIP/TAR.GZ, texto largo y archivo corrupto; cancelar y reintentar. | Pendiente |
| Bandeja | Minimizar, reabrir, cerrar a bandeja y salir desde el menú/atajo. | Pendiente |
| Unidad extraíble | Con dispositivo de prueba, detectar, navegar y expulsar; no usar el disco del sistema. | Pendiente |
| UNC/red | Compartido disponible, lento y desconectado; comprobar que la UI sigue respondiendo y permite cancelar. | Pendiente |
| DPI/ventana | Repetir a 100/150/200 % y con ventana estrecha; revisar toolbar, diálogos, previews y texto recortado. | Pendiente |
| Narrador | Recorrer ventanas, controles, diálogos y estado de operaciones con Narrador; registrar etiquetas/foco ausentes. | Pendiente |

### Fixtures desechables

El generador `scripts/prepare-w13-filesystem-fixtures.ps1` deja carpetas reproducibles dentro de
`target/agent-out/`, sin sobrescribir ni eliminar datos. Ejecutar desde la raíz del checkout:

```powershell
scripts\prepare-w13-filesystem-fixtures.ps1
```

El valor por defecto crea `listing-10000` y `listing-100000`, además de `ops/source` y
`ops/destination` con fixtures de copia, movimiento, conflicto y nombre Unicode. También crea
`extensions/` con carpetas `1.The Buddha` y `2.3DBenchy`, un STL y un archivo sin extensión, junto a
`drop-7zip-fixture.zip` con un OBJ pequeño y un TXT de contenido sintético. El comando imprime la
ruta raíz y guarda `README.txt` y `manifest.json` allí. No abrir estas carpetas con una instancia
personal; usarlas sólo en la sesión aislada de aceptación. Para comprobar el generador sin producir
el corpus completo, se puede pasar `-FileCounts 50`.

El 16 de octubre se preparó y verificó el corpus completo en
`target/agent-out/w13-filesystem-20261016-014835/`: 10.000 y 100.000 archivos vacíos, cuatro
fixtures de origen y uno de destino. Aún no se ejecutó la matriz interactiva con estas carpetas.

## Medición W-07

La captura pasiva `scripts/capture-w07-process.ps1` (schema v3) muestrea el proceso principal y sus
descendientes al inicio y con el intervalo indicado, por defecto cada 2 segundos. Reporta el conjunto
de procesos observados y los picos de recursos del árbol durante cada ventana. Procesos más breves
que el intervalo aún pueden no aparecer; el muestreo agrega una sobrecarga pequeña. El working set
sumado puede duplicar páginas compartidas y el delta CPU sólo cubre identidades observadas en dos
muestras consecutivas. La aceptación sigue requiriendo el release 0.5.2 y las interacciones indicadas
abajo.

El 15 de octubre el parser de PowerShell pasó y una prueba con proceso hijo sintético tomó cinco
muestras durante 4 s (intervalo de 1 s); el hijo terminó antes del cierre, pero quedó en
`observed_processes`. JSON: `target/agent-out/w07-process-sampling-smoke.json`. Esta prueba valida el
capturador y no mide el release ni la interacción.

Para capturar las métricas internas, la instancia debe iniciarse con `NAYGO_PERF_METRICS=1`; la variable
se establece antes del proceso y una instancia ya abierta no la hereda. En PowerShell, desde la carpeta
extraída o instalada que se prueba:

```powershell
$env:NAYGO_PERF_METRICS = '1'
Start-Process -FilePath '.\naygo.exe' -Wait
```

Usa una sesión dedicada de prueba para evitar que el mutex de instancia única envíe el lanzamiento a
un Naygo ya abierto. Al salir, conserva el log local de métricas y registra el path sin incluir rutas
privadas en este documento. Repite navegación equivalente tres veces con mismo equipo, carpeta de
prueba y build. Añade resultados aquí o enlaza un registro separado:

| Escenario | Primera fila/contenido | Latencia de acción | CPU | Memoria principal + auxiliares | Handles/hilos | Resultado |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Reposo tras arranque, 3 x 30 s | No aplica | Sin interacción | Primera ventana 1,953 s; luego 0 s/30 s | Raíz: pico 55,8 MiB working set / 26,7 MiB privados; sin auxiliares | 344 / 20 | Parcial, un run; sin SMB; JSON `w07-release-20261016-idle.json`; no cierra W-07 |
| Instalación W-10C actualizada, observación 3 x 30 s | No aplica | Sin interacción | 0,062/0,047/0,047 s por ventana | Árbol observado: pico 72,1 MiB working set / 41,3 MiB privados; solo proceso raíz | 357 / 25 | Parcial histórica; sin SMB ni inactividad confirmada; JSON `w07-installed-w10c-20261016-0924.json`; no cierra W-07 |
| Instalación final AccessKit, observación 3 x 30 s | No aplica | Sin interacción | 0/0,047/0,015 s por ventana | Solo proceso raíz: 60,8 MiB working set / 29,3–29,4 MiB privados | 385–387 / 26–27 | Parcial; no confirma inactividad ni mide interacción/UNC; JSON `w07-installed-accessibility-20261016.json`; no cierra W-07 |
| Apertura CLI de carpeta 10k, tres runs | Agregada: p50 45,759 ms hasta primer lote | Sin interacción | No capturada; callback `ui.tick` p50 26,091 ms | No capturada | No capturados | `ui.sync_rows` p50 15,087 ms; carpeta confirmada; logs en `w07-listing-release-20261016-100k/`; parcial |
| Apertura CLI de carpeta 100k, tres runs | Agregada: p50 49,970 ms hasta primer lote | Sin interacción | Proceso raíz 48,156/29,876/29,891 CPU-s en ventanas de 30 s; callback `ui.tick` p50 139,940 ms | Proceso raíz: pico 265,7 MiB working set / 257,9 MiB privados; sin auxiliares | 347 / 21 | `ui.sync_rows` p50 118,962 ms; `--shutdown` no cerró en 20 s bajo captura y la instancia aislada se terminó; investigar, no cierra W-07 |
| Carpeta local grande, preview apagado | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente |
| Carpeta local grande, preview estático | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente |
| Release E-1 pasivo | No aplica | No hubo interacción | No medida como latencia | Pico 89,6 MiB WS / 57,9 MiB privado | 389 handles / 27 hilos | Tres ventanas 30 s; un proceso, cero SMB, CPU 0/0/0,125 s; responde. Pasivo, no controló inactividad; no cierra W-07. `w07-installed-e1-20261017-1646.json` |
| Append streaming a 100k (headless) | No aplica, fixture residente en memoria | Append p50 21,279 ms en 3 corridas | No capturada | No capturada | No capturados | W-10C aprobado y empaquetado; evidencia de `sync_rows`, no cierra W-07 |
| Navegación A → B → A / cancelar | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente |
| UNC disponible/lenta/caída | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente | Pendiente |

La captura pasiva previa de PID 23856 corresponde al ejecutable instalado el 14 de septiembre, no a esta
entrega. No usarla como línea base W-07 de 0.5.2.

## Actualización W-13 AccessKit básica del 16 de octubre (build anterior)

Después del primer update W-10C se detectó que Windows UI Automation sólo veía la ventana raíz. La
distribución posterior habilitó AccessKit y se instaló con autorización; Naygo quedó abierto y
respondiendo. El inspector de solo lectura expone 318 descendientes (224 con nombre, 46 enfocables),
sin truncar el árbol. Esto confirma estructura expuesta, no lectura real ni orden de foco. Sigue
pendiente la prueba manual con Narrador, orden de foco y navegación por teclado. La captura pasiva
3×30 s de la instalación final registró 60,8 MiB de working set y 29,3–29,4 MiB privados, sin
interacción ni inactividad confirmada. El detalle de hashes, gate, setup y PDB obsoleto está en
[`VALIDACION-BUILD-2026-10-16.md`](VALIDACION-BUILD-2026-10-16.md).

La distribución W-13 reemplazó la build AccessKit básica: EXE instalado SHA-256
`c492982e5f70d6a8d1a2e78982035554e6a1065a3a5fbbf3fdd05eb82540bdca`. Tras instalarla y abrirla, UIA
expuso 312 descendientes (228 con nombre, 46 enfocables; árbol sin truncar). La inspección no activa
Narrador ni sustituye la aceptación manual de foco y teclado. Naygo quedó abierto y respondiendo.
Un probe UIA aislado que importó el `TabBar` de producción expuso un `Tab`, tres `TabItem` enfocables
y estados seleccionados `true/false/false`; confirmó el puente Slint/AccessKit a UIA, pero no la
aceptación manual de Narrador ni del flujo completo de ventanas.

## Inspección estructural del release instalado — 2026-10-17

La instancia instalada con el menú de rutas de favoritos es PID 38744, EXE SHA-256
`357ee82a0a27b2dbae5cecbe35d63a6394e97315fd384cee9db4dd951d2b1561`. El inspector de solo lectura
`scripts/inspect-naygo-uia.ps1` reportó 331 descendientes, 240 con nombre y 46 enfocables, sin
truncamiento. Confirma que AccessKit entrega estructura a UI Automation en la build actual; no
valida Narrador, nombres anunciados, orden de foco, operaciones ni el menú abierto. La aceptación
manual W-13 permanece pendiente.

## Revalidación automatizada — 2026-10-18

En el checkout local, `cargo test -p naygo-platform --locked trash::tests` pasó las ocho pruebas,
incluidos recibos parciales, destino ocupado y junctions. La regresión
`drop_target::windows_impl::tests::hdrop_temporal_de_7zip_se_detecta_y_se_snapshottea` pasó 1/1; las
siete pruebas UI que coinciden con `drop_at_` también pasaron. La prueba de HDROP confirma la
detección y captura del bloque temporal de 7-Zip; ninguna de estas pruebas reproduce el arrastre real
desde su ventana. En esta revisión `naygo.exe` PID 25124 responde y `LogonUI` sigue activo, por lo
que la reproducción manual, la aceptación visual y Narrador continúan pendientes.
