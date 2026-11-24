# Validación de distribución — 2026-10-16

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

Build local de Naygo 0.5.2 tras integrar el avance parcial L-02 de rutas XDG y logging Linux,
y el subpaso transaccional de copia de archivos de L-01B.
Este build conserva el comportamiento portable de Windows y no certifica instalación externa,
runner GitHub Windows Server 2025, firma de código ni compilación/ejecución Linux nativa.

## Gate Windows

`scripts/build-release-guarded.ps1 -Verify` completó el workspace con 1.399 tests aprobados,
0 fallidos y 7 ignorados; Clippy con `-D warnings`, formato y `git diff --check` pasaron. El check
cruzado de core/platform para `x86_64-unknown-linux-gnu` y Clippy Linux de esos crates también
pasaron antes del gate. El check cruzado de UI sigue bloqueado por falta de `pkg-config` configurado
con un sysroot Linux.

## Release supervisado

Comando: `scripts/build-release-guarded.ps1`, ejecutado solo tras completar el gate. Cargo release
terminó en 39m22s; Inno Setup en 173,047 s. Pico contable del Job: 10,42 GiB (límite 12 GiB). El
margen de memoria disponible observado nunca bajó de 35 GiB, por encima de los 8 GiB mínimos.

| Artefacto | Tamaño | SHA-256 | Estado |
| --- | ---: | --- | --- |
| `Naygo-0.5.2-portable.zip` | 14.154.043 bytes | `625e3a3b35009b444092bd0dd8a4f3acf199fc174cef2b8160b0f9a2df7afd82` | ZIP legible; sin PDB |
| `Naygo-0.5.2-setup.exe` | 64.761.756 bytes | `d13380cf3cbd84f1ecfe00942d26ade8bfcd24d568ffeebe5b6f67eeabe8bbbe` | Instalador generado |
| `naygo.exe` | 42.772.480 bytes | `b3873dadcb676acc288b99e0eca78ab2769504798520cd6d814c9f52266c5439` | Coincide con el EXE dentro del ZIP |

El ZIP contiene `LEEME.txt`, `LICENSE`, `naygo.exe` y `THIRD-PARTY-NOTICES.md`; no contiene PDB.
`dist/SHA256SUMS.txt` coincide con los dos hashes de distribución. EXE y setup reportan
Authenticode `NotSigned`, pendiente de configurar y ejecutar SignPath (W-14).

## Smoke portable

Se extrajo el ZIP a `target/agent-out/w07-portable-verify-20261016-homefix/` y se comprobó que Naygo 0.5.2
entrara al event loop, creara sus archivos portables y cerrara al recibir `--shutdown`. El log opt-in
indica backend `winit-software` y registra fases de listado, sincronización UI, ticks y workers; el
smoke no fue una matriz de rendimiento. No aparecieron muestras `ui.input.*`, así que medir callbacks
de teclado/filas/sort e interacción real sigue pendiente. Log: `target/agent-out/w07-portable-verify-20261016-homefix/naygo-2026-10-16.log`.

La captura pasiva del mismo portable tomó tres ventanas de 30 s (intervalo 2 s): pico 55,8 MiB de
working set, 26,7 MiB privados, 344 handles y 20 hilos. La primera ventana, que incluye arranque,
sumó 1,953 s de CPU; las dos siguientes sumaron 0 s. Sólo se observó el proceso principal y no había
mapeos SMB. No hubo interacción ni se cargaron escenarios de navegación o preview. JSON en
`target/agent-out/w07-release-20261016-idle.json`; evidencia detallada y límites en
`docs/BENCHMARKS.md`.

## Límites y pendientes

- W-07: medir interacción y renderer sobre escenarios reproducibles, incluida red; el smoke de
  arranque/cierre no cierra el bloque.
- W-12: ejecutar el workflow actualizado en runner Windows Server 2025.
- W-13: aceptación manual de setup, actualización, navegación, Papelera, preview, bandeja,
  unidades, UNC, DPI y Narrador.
- W-14: configuración/aprobación de proyecto y certificado SignPath y verificación remota de ambos
  artefactos firmados.
- L-02: separar consumidores de config/data/cache y completar migración/recuperación; falta check
  de UI, build, ejecución y pruebas nativas Linux.

Logs del supervisor de esta regeneración: `target/agent-out/guarded-20261016-005117-975.*.log`
(gate) y `target/agent-out/guarded-20261016-010225-638.*.log` (release).

## Regeneración tras el subpaso L-01A de sesión JSON

El 16 de octubre se volvió a ejecutar `scripts/build-release-guarded.ps1 -Verify` tras conservar
rutas POSIX no UTF-8 en `FilePanePersist` y en el fingerprint de sesión. Pasaron 1.400 tests,
fallaron 0 y quedaron 7 ignorados; Clippy con `-D warnings`, formato y `git diff --check` pasaron.
El pico contable del gate fue 5,93 GiB. Logs: `target/agent-out/guarded-20261016-030150-691.*.log`.

El build supervisado terminó con código 0. Cargo release tardó 40m32s e Inno Setup 170,875 s;
el pico contable del Job fue 10,42 GiB (límite 12 GiB). El portable ZIP mide 14.159.589 bytes,
el setup 64.945.992 bytes y el EXE incluido en el ZIP 42.786.816 bytes. El EXE extraído del ZIP
coincide byte por byte con `target/release/naygo.exe`.

| Artefacto | SHA-256 | Estado |
| --- | --- | --- |
| `Naygo-0.5.2-portable.zip` | `0b48a5224e902a0139b97d65fda34d87b31807aeec1bb83233936a5f888cd0b3` | ZIP válido; sin PDB |
| `Naygo-0.5.2-setup.exe` | `d0f44f0ad00b002cb363a7388935becb8da55d64546c5b226a902bfecc41c53c` | Generado; sin firma Authenticode |
| `naygo.exe` | `d13a305625dbc968806995c7f210d5abe3c2586baa9f6e8a89d731248463ede6` | Coincide con el EXE extraído del ZIP; sin firma Authenticode |

`dist/SHA256SUMS.txt` contiene los hashes del ZIP y setup. No se instaló, publicó ni firmó ningún
artefacto. Log del supervisor: `target/agent-out/guarded-20261016-031254-489.*.log`.

## Smoke y captura pasiva del portable actual

El ZIP se extrajo a `target/agent-out/w07-portable-verify-20261016-pathpersist/`. Una primera
invocación con `NAYGO_PERF_METRICS=1 --shutdown` terminó con código 0 y registró el backend
`winit-software`. Luego se abrió esa copia aislada en el event loop durante tres ventanas de 30 s
con métricas opt-in; se solicitó cierre limpio mediante `--shutdown`. No se instaló ni se modificó
la sesión del usuario.

En las tres ventanas se observaron sólo el proceso raíz y ningún mapeo SMB: pico de 55,6 MiB de
working set, 26,5 MiB privados, 378–379 handles y 21 hilos. El delta de CPU fue 0 s en cada ventana
de reposo, tras 2,641 s acumulados antes de la primera muestra. Las métricas internas al cierre
registraron 6 listados (p50 247,1 ms, máximo 838,7 ms), 20 cálculos de firma de filas (p50 8 µs),
10 sincronizaciones de filas (p50 2,33 ms), 4 de layout (p50 2,369 ms) y 5 ticks (p50 5,268 ms,
máximo 101,493 ms). La sesión por defecto cargó varios paneles; no se enviaron entradas y el número
de listados no representa un único panel ni una matriz de navegación. Estos datos son una captura
pasiva de arranque/reposo, no cierran W-07 ni miden latencia percibida.

JSON externo: `target/agent-out/w07-release-20261016-pathpersist-idle.json`. Log interno:
`target/agent-out/w07-portable-verify-20261016-pathpersist/naygo-2026-10-16.log`.

## W-10C — append incremental y distribución 0.5.2

El 16 de octubre se completó W-10C. `scripts/build-release-guarded.ps1 -Verify` pasó las suites
completas (1.402 tests aprobados, 8 ignorados), Clippy `-D warnings`, formato y diff check; pico del
Job 9,07 GiB. La medición release del `sync_rows` headless corrió tres veces: append de una fila a
100k dio 21,140/21,741/21,279 ms (mediana 21,279 ms), frente a 561,367 ms de la línea base anterior.
No es latencia de UI/Winit ni cierra W-07; los valores, entorno y logs están en `docs/BENCHMARKS.md`.

Luego `scripts/build-release-guarded.ps1` regeneró la distribución bajo Job de 12 GiB y Cargo jobs=1.
Cargo release tardó 28m10s, Inno Setup 129,891 s, y el pico del Job fue 10,42 GiB. No se instaló,
publicó ni firmó ningún artefacto.

| Artefacto | Tamaño | SHA-256 | Verificación |
| --- | ---: | --- | --- |
| `Naygo-0.5.2-portable.zip` | 14.163.847 bytes | `ad71d85a9d7658054f63ed3a97954328b984509d878c2388def0e44a3808da77` | ZIP contiene `naygo.exe`, `LEEME.txt`, `LICENSE` y notices; no incluye PDB |
| `Naygo-0.5.2-setup.exe` | 65.245.900 bytes | `a75fee5e528fc84b01474c9b57b2bc5bf9c041fb49a2a1153613e766bf1f9341` | Authenticode `NotSigned` |
| `naygo.exe` | 42.792.960 bytes | `493ad821665db26384464e87e0537e5afa2dff6de65ce529bb7f3e4d9c940f04` | coincide byte a byte con el ejecutable dentro del ZIP; Authenticode `NotSigned` |

`dist/SHA256SUMS.txt` coincide con los hashes independientes del ZIP y setup. Logs supervisados:
gate `target/agent-out/guarded-20261016-072709-317.*.log`, tres mediciones
`guarded-20261016-074839-962.*.log`, `guarded-20261016-082252-575.*.log`,
`guarded-20261016-082302-014.*.log`, y build
`guarded-20261016-082511-123.*.log`.

## Actualización instalada posterior, 2026-10-16 09:24 -03:00

Con autorización de Nicolás, se actualizó la instalación de usuario que tenía el build 0.5.2
anterior del 14 de septiembre (EXE SHA-256 `c78828504e8f5908a0fb47c31f542d7c52d65a84f98469c3352079809d876f8e`).
Antes de instalar se verificó el SHA-256 del setup contra `dist/SHA256SUMS.txt`; la suma coincidió.
Inno Setup terminó con código 0 y conservó el directorio de instalación previo. Tras la actualización,
el registro de Windows muestra versión 0.5.2 y el EXE instalado tiene SHA-256
`493ad821665db26384464e87e0537e5afa2dff6de65ce529bb7f3e4d9c940f04`, coincidente con el EXE W-10C
del ZIP. Naygo quedó abierto con ventana titulada `Naygo` y el proceso reportó `Responding=True`.

Esto verifica la actualización local y el arranque de la UI. El script de Inno copia el EXE,
documentación/licencia y PDB opcional, no los archivos de configuración; después del update se
encontraron `settings.json`, `favorites.json` y `templates.json` con fechas de modificación previas.
`workspace.json` se actualizó tras arrancar Naygo. No se registró un snapshot previo, así que la
paridad exacta de preferencias/sesión sigue pendiente. Tampoco se probó desinstalación, el resto de
interacciones de W-13 ni el ZIP portable ejecutado por separado. Se dejó el ZIP sin iniciar mientras
la app instalada está abierta por el mutex de instancia única. El setup y el EXE siguen Authenticode
`NotSigned` hasta completar W-14. El log de inicio de la sesión instalada tiene 17 líneas y cero
coincidencias con los marcadores de error/panic/failed consultados; eso no sustituye la aceptación
funcional.

## W-13 — primera actualización AccessKit (previa a pestañas), 2026-10-16

Una inspección de solo lectura de la versión instalada encontraba únicamente la ventana raíz en
Control View, pese a que varios controles Slint ya declaraban rol/etiqueta accesible. Se habilitó
la feature `accessibility` de Slint/AccessKit y se agregó `scripts/inspect-naygo-uia.ps1`.
La feature y sus dependencias de Windows son MIT o Apache-2.0 según `cargo metadata`; esto no
cambia la política/licencia Windows. El inspector pasa PowerShell Parser y no registra nombres,
rutas ni contenido.

El gate supervisado `scripts/build-release-guarded.ps1 -Verify` pasó 1.402 tests y 8 ignorados,
más Clippy `-D warnings`; `cargo fmt --all -- --check`, `git diff --check` y `graphify update .`
también terminaron con código 0. El gate marcó pico de Job 8,60 GiB. La build release supervisada
terminó correctamente: Cargo 35m41s, Inno Setup 147,734s y pico de Job 10,44 GiB (límite 12 GiB;
margen mínimo configurado 8 GiB). Los artefactos son 0.5.2 y siguen sin firma Authenticode por
W-14.

| Artefacto | Tamaño | SHA-256 | Verificación |
| --- | ---: | --- | --- |
| `Naygo-0.5.2-portable.zip` | 14.276.248 bytes | `27579767e6ae1769ab3290c17b68cd0296e75e7c694a512a99071843d7bc89ae` | ZIP legible; incluye notices actualizados, sin PDB |
| `Naygo-0.5.2-setup.exe` | 65.742.962 bytes | `4159100af9e3a0a4a3aff2f67358958834916506dbe93060a2b3e3e988420fc1` | Authenticode `NotSigned` |
| `naygo.exe` | 43.088.896 bytes | `8173d7fac2cf2582a9cdeb4201a321df98288556e7f018d457e43925a89de9d4` | Coincide con `target/release` y el EXE instalado |

Nicolás autorizó instalar la versión nueva y dejarla ejecutándose. Inno Setup silencioso terminó
con código 0 en la instalación existente. El proceso 16628 presenta la ventana `Naygo` y reporta
`Responding=True`. El inspector UIA encontró 318 descendientes: 224 con nombre y 46 enfocables
por teclado, sin truncar el árbol; tipos: 46 botones, 87 imágenes, 6 listas y 179 textos. Esto
confirma que AccessKit expone estructura a UI Automation, pero no es una sesión de Narrador ni
valida el orden/foco de navegación; esa parte manual de W-13 sigue pendiente.

El PDB de `target/release` mide 449.540.096 bytes, pero quedó intacto el PDB previo de la
instalación (289.722.368 bytes, fechado 22 de agosto) porque la tarea opcional `debugsymbols` no
se seleccionó. Nicolás aprobó registrar el caso en el backlog; decidir si futuras actualizaciones
deben reemplazar o retirar símbolos obsoletos. El ZIP actual no se ejecutó en paralelo con la app
instalada debido al mutex de instancia única.

Logs supervisados: gate `target/agent-out/guarded-20261016-094020-107.*.log`, release/paquete
`target/agent-out/guarded-20261016-101051-717.*.log`.

### Regeneración tras la carrera de destino en Move Windows

Se integró la interfaz aprobada core↔platform para inyectar el rename no-clobber. Windows usa
`MoveFileExW` sin `MOVEFILE_REPLACE_EXISTING` ni `MOVEFILE_COPY_ALLOWED`: un destino ocupado se
conserva, y los errores cross-volume vuelven al fallback cancelable de copia y retirada del origen.
El cambio reemplaza un intento previo con `FILE_RENAME_INFO` después de que las pruebas UI mostraran
que su buffer manual podía corromper rutas anidadas. Pasaron las dos pruebas del adaptador Win32 y
las regresiones de reintento de carpeta y conflicto al renombrar/mover.

El gate `scripts/build-release-guarded.ps1 -Verify` terminó con 1.417 tests aprobados, 0 fallidos y
8 ignorados; Clippy de workspace con `-D warnings` pasó. `cargo fmt --all -- --check`,
`git diff --check`, check de tests y Clippy cruzados para `naygo-core` y `naygo-platform` en
`x86_64-unknown-linux-gnu` también pasaron. El Job del gate alcanzó 5,12 GiB.

La distribución se regeneró en solitario con `scripts/build-release-guarded.ps1`: Cargo release
terminó en 27m38s, Inno Setup en 113,891 s y el pico del Job fue 10,45 GiB (límite 12 GiB).
ZIP, setup y ejecutable siguen sin firma Authenticode. El ZIP contiene `LEEME.txt`, `LICENSE`,
`naygo.exe` y `THIRD-PARTY-NOTICES.md`, sin PDB; el hash del EXE interno coincide con
`target/release/naygo.exe`.

| Artefacto | Tamaño | SHA-256 | Verificación |
| --- | ---: | --- | --- |
| `Naygo-0.5.2-portable.zip` | 14.306.070 bytes | `82ea62e37d89e7305bd476ba4c1e228e07ca6eb7b7292a39d8036ce82048e369` | ZIP legible; sin PDB |
| `Naygo-0.5.2-setup.exe` | 65.984.178 bytes | `8f483e0aece3910ae95823d2c4537404b94350b1f84e49219872daa701ad4094` | Inno Setup compilado; `NotSigned` |
| `naygo.exe` | 43.143.168 bytes | `dcba385eb0e68de5a6dc9cbf1960f873a5d7fb04edabf8b02d6a553c2325489f` | coincide con el EXE del ZIP; `NotSigned` |

`dist/SHA256SUMS.txt` coincide con ZIP y setup. No se instaló la nueva build; la app que ya estaba
ejecutándose y sus cuatro JSON quedaron intactos. Logs supervisados: gate
`target/agent-out/guarded-20261016-193348-611.*.log`, distribución
`target/agent-out/guarded-20261016-194025-312.*.log`. La verificación del paquete no cierra la
aceptación manual W-07/W-13, SignPath/W-14, ni el reemplazo transaccional completo de árboles.

### Inventario de licencias y reinstalación final

Al revisar el artefacto se comprobó que `THIRD-PARTY-NOTICES.md` aún enumeraba 387 paquetes y
omitía AccessKit. Se regeneró con `cargo license --avoid-dev-deps --avoid-build-deps --filter-platform
x86_64-pc-windows-msvc`: 404 paquetes en 21 grupos, incluidos AccessKit y las nuevas versiones
Windows. Los avisos mantienen MIT/Apache-2.0 para AccessKit y no cambian la licencia de Windows.
El ZIP se verificó con `System.IO.Compression`: contiene `naygo.exe`, `LEEME.txt`, `LICENSE` y
`THIRD-PARTY-NOTICES.md`; el hash interno del EXE coincide con release y no incluye PDB. `7z` no
está disponible en este equipo, por lo que se usó esa comprobación alternativa de lectura y hash.

Se reempaquetó con `scripts/build-release.ps1 -SkipBuild` usando el EXE ya gateado; tras quitar una
línea final vacía de los avisos, la compilación final de Inno terminó correctamente en 148,015 s.
El portable fue verificado con `System.IO.Compression`; `7z` no está instalado en este equipo.
La instalación final terminó con código 0, dejó los notices de 13.086 bytes y el EXE coincide con
el release. Naygo se volvió a iniciar y quedó visible/respondiendo en PID 32448. El inspector repitió
los mismos 318 controles UIA. El PDB
anterior sigue intacto (289.722.368 bytes, 22 de agosto) porque la tarea opcional de símbolos no se
marcó; su política futura permanece en backlog.

### W-07 — captura pasiva de la instalación final

Tras reinstalar y abrir el artefacto final, se capturó el PID 32448 durante tres ventanas de 30 s
(intervalo de 2 s). El máximo de working set fue 60,8 MiB en cada ventana; memoria privada
29,4/29,3/29,3 MiB, handles 387/385/385, threads 27/26/26 y CPU 0/0,047/0,015 s. Sólo se observó
el proceso Naygo, sin descendientes. Es una observación pasiva sin inactividad confirmada, navegación,
preview, interacción ni UNC; no cierra W-07 ni permite atribuir diferencias frente a otras sesiones.
JSON: `target/agent-out/w07-installed-accessibility-20261016.json`; contexto y límites en
`docs/BENCHMARKS.md`.

## W-13 — roles y navegación accesible de pestañas

Se extrajo la barra custom a `crates/ui-slint/ui/tab-bar.slint`. El grupo expone `tab-list`, nombre
localizado e índice de elementos; cada pestaña expone rol `tab`, nombre, selección, índice y acción
por defecto. El botón de cierre tiene nombre/rol y acción propia. Un `FocusScope` permite recorrer
con ←/→ sin activar prematuramente, activar con Enter/Espacio y muestra un borde de foco; el índice
activo llega desde el modelo de paneles. El texto español para la región quedó como “Pestañas”.

`tests/tab_bar_accessibility.rs` verifica con el backend software que el foco recorra, Enter y
Espacio activen la pestaña correspondiente, y el foco no salga del grupo en el extremo. El gate
supervisado `scripts/build-release-guarded.ps1 -Verify` terminó correctamente el 16 de octubre; pasó
1.403 tests (8 ignorados), Clippy `-D warnings`, y formato, con pico Job 5,97 GiB. Logs:
`target/agent-out/guarded-20261016-115045-494.*.log`. `cargo fmt --all -- --check` y `git diff --check`
también pasaron.

La distribución 0.5.2 se regeneró con `scripts/build-release-guarded.ps1` (ThinLTO, 1 job,
supervisor 12 GiB; pico Job 10,45 GiB). El instalador compiló correctamente; no se firmó Authenticode
(W-14 pendiente). SHA-256:

- Portable: `caf21d8d7f19c10835b4ef4c0bf137cb00e85ab0ad117ce059d2b2da0aecbb74`
- Setup: `8ee7110721ea4000b947141ce93feb8330d3935b0a427a9e28d7acde47ebbd12`

El ZIP contiene `LEEME.txt`, `LICENSE`, `naygo.exe` y `THIRD-PARTY-NOTICES.md`, sin PDB. El EXE
extraído del ZIP tiene SHA-256 `c492982e5f70d6a8d1a2e78982035554e6a1065a3a5fbbf3fdd05eb82540bdca`,
igual al ejecutable release e instalado.

Con autorización de Nicolás, el setup se ejecutó en actualización in-place y terminó con código 0.
El EXE instalado coincide con SHA-256 `c492982e5f70d6a8d1a2e78982035554e6a1065a3a5fbbf3fdd05eb82540bdca`.
`settings.json`, `favorites.json`, `templates.json` y `workspace.json` coinciden byte a byte con sus
copias tomadas tras el cierre limpio y antes de instalar. El PDB antiguo (289.722.368 bytes) quedó
intacto, conforme al seguimiento pendiente. Naygo se volvió a abrir desde la instalación y responde;
el log nuevo no contiene coincidencias con los marcadores ERROR/PANIC/failed/error consultados.
El inspector UIA del proceso instalado encontró 312 descendientes, 228 con nombre y 46 enfocables;
la enumeración no se truncó. Esto confirma exposición estructural, no sustituye Narrador ni verifica
la experiencia real de foco/orden. La matriz manual de W-13 sigue pendiente. Log del build:
`target/agent-out/guarded-20261016-120114-648.*.log`.

Una captura pasiva W-07 posterior sobre PID 18068 observó un solo proceso en tres ventanas de
30 s (16 muestras/ventana, 2 s): working set pico 60,5/60,6/60,6 MiB, privado 29,2 MiB,
385/387/387 handles, 26/27/27 hilos, CPU de árbol 0/0,05/0 s y cero mapeos SMB. No controla
actividad externa y no habilitó métricas internas. JSON: `target/agent-out/w07-installed-w13-20261016.json`.
La sesión activa no tiene grupos apilados en pestañas (cero entradas de pestañas en el JSON de
sesión), por lo que UIA no encontró `Tab`/`TabItem` en esta ventana. El mapeo local AccessKit asigna
los roles `tab`/`tab-list` a `UIA_TabItem`/`UIA_Tab`, pero su presencia en un grupo real aún requiere
aceptación manual W-13.

Para probar ese mapeo sin tocar la sesión instalada, se abrió una ventana temporal que importa el
`tab-bar.slint` de producción, con tres pestañas sintéticas. En Windows 10 UIA reportó
`ControlType.Tab=1`, `TabItem=3`, `Button=3`, `Text=6`, sin truncar el árbol. Los tres TabItem eran
enfocables y `SelectionItemPattern.IsSelected` devolvió `true`, `false`, `false`, de acuerdo con el
modelo. La ventana probe se cerró; Naygo quedó abierto/respondiendo. Esto valida exportación de
roles/selección del componente, no Narrador ni navegación real dentro del flujo completo de Naygo.

El 16 de octubre se intentó una comprobación adicional de clic y teclado en ese probe aislado. UIA
confirmó las tres pestañas y el clic seleccionó la segunda; la aserción posterior a Enter no devolvió
una pestaña seleccionada y el ensayo quedó inconcluso (la ventana de prueba también expiró por su
temporizador). Por tanto no se toma como evidencia positiva de interacción Windows. La prueba
dirigida `cargo test -p naygo-ui-slint --test tab_bar_accessibility` sí pasó (1/1), pero usa eventos
sintéticos del backend software. W-13 continúa pendiente de validar foco/Enter/Espacio con Narrador y
en la ventana real de Naygo cuando haya un grupo de pestañas disponible.

### Repetición pasiva W-07

El 16 de octubre se repitieron tres ventanas de 30 s sobre el PID instalado 18068 (16 muestras por
ventana, intervalo de 2 s). Se vio un proceso raíz, sin mapeos SMB; working set pico 60,9 MiB,
privado 29,2 MiB, máximo 385 handles y 24 hilos. CPU por ventana: 0,094/0,141/0,047 s. La captura
no confirma reposo controlado, actividad de UI, latencia ni cobertura de procesos auxiliares de vida
corta. Naygo quedó abierto y respondiendo. JSON: `target/agent-out/w07-installed-w13-continuation-20261016.json`.

En una segunda repetición aislada se añadió un estado textual independiente para contrastar la
selección. Windows no permitió poner el probe en primer plano desde el proceso de automatización; el
guard evitó enviar teclas. UIA sí leyó la selección inicial “Panel uno”. Se cerró sólo el probe; la
instancia instalada siguió intacta. El resultado sigue siendo inconcluso, no un fallo demostrado del
componente.

### L-01A — nombres POSIX no UTF-8 y build Windows actualizado

El listado, árbol y búsqueda del core muestran bytes POSIX inválidos como `\\xNN` y duplican la
barra inversa literal; `PathBuf` conserva la ruta operativa original. El gate supervisado
`scripts/build-release-guarded.ps1 -Verify` pasó el 16 de octubre: 1.404 tests aprobados, 8
ignorados, Clippy con `-D warnings` y formato. El pico del gate fue 7,70 GiB. Log:
`target/agent-out/guarded-20261016-132123-470.out.log`.

La distribución se regeneró con `scripts/build-release-guarded.ps1`; el build terminó correctamente
en 117,453 s de compilación de Inno Setup, con pico Job de 10,45 GiB. El ZIP es legible y contiene
`LEEME.txt`, `LICENSE`, `naygo.exe` y `THIRD-PARTY-NOTICES.md`, sin PDB. Hashes SHA-256 del manifiesto:

| Artefacto | Tamaño | SHA-256 | Validación |
|---|---:|---|---|
| `Naygo-0.5.2-portable.zip` | 14.282.443 bytes | `c923059d6ff86e4eb3d854653b9bf92a160b3755c209761fccd7122ce8916946` | ZIP legible; sin PDB |
| `Naygo-0.5.2-setup.exe` | 64.547.990 bytes | `d33efd51123d030c3814deba7d4614ff87f2eca3e796c4eecbe7c26db37354aa` | Compilación correcta; `NotSigned` |

El EXE del ZIP tiene SHA-256 `bfb03176daa0dedf9312d42178ccfa57f1bf96d4437889aad5e4a3ead5bd48ef`, igual al de
`target/release/naygo.exe`. Los dos artefactos coinciden con `dist/SHA256SUMS.txt`.

En la validación original de este paquete no se instaló la regeneración y el PDB quedó intacto.
Después, con la autorización previa de Nicolás, se actualizó in-place desde el setup local; Inno
Setup terminó con código 0. El EXE instalado coincide con SHA-256
`bfb03176daa0dedf9312d42178ccfa57f1bf96d4437889aad5e4a3ead5bd48ef`. `settings.json`,
`favorites.json`, `templates.json` y `workspace.json` conservaron sus SHA-256 pre/post. El PDB
antiguo sigue intacto: 289.722.368 bytes, SHA-256
`eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`. Naygo se abrió desde la
instalación, respondió durante la observación y quedó corriendo (PID 44816 al tomar la muestra).
El EXE instalado y el setup siguen `NotSigned`; no hubo firma ni publicación. El portable ZIP no
se había ejecutado al completar la validación del paquete; la prueba posterior se registra al final.

### W-07 — captura pasiva del release actualizado, 2026-10-16

Después de instalar el release actual, se midieron tres ventanas de 30 s (16 muestras/ventana,
intervalo de 2 s) sobre PID 44816. Hubo un proceso raíz, cero mapeos SMB y pico de 62,8 MiB de
working set, 31,1 MiB privados, 394 handles y 27 hilos. Deltas CPU: 0,359/0,110/0,047 s. La primera
ventana comenzó unos 36 s después del arranque. Es observación pasiva, sin interacción ni reposo
controlado; no mide latencia/UI/Winit, preview, UNC ni auxiliares más breves que el intervalo, y no
cierra W-07. JSON: `target/agent-out/w07-installed-latest-20261016.json`.

### W-13 — intento de observar el probe con Computer Use

La ventana activa de Naygo no expuso roles `tab-list`/`tab-item` en su árbol UIA; la sesión guardada
no tiene grupos apilados de pestañas. Se lanzó el probe aislado de tres pestañas, pero la captura de
frame agotó el tiempo dos veces. No se envió entrada; se cerró únicamente el proceso temporal. La
prueba interactiva con teclado/Narrador permanece pendiente y este intento no es evidencia de un
fallo del componente.

### W-07 — smoke del portable actual y sesión restaurada, 2026-10-16

El ZIP `Naygo-0.5.2-portable.zip` (SHA-256
`c923059d6ff86e4eb3d854653b9bf92a160b3755c209761fccd7122ce8916946`) se extrajo en una carpeta
temporal aislada bajo `target/agent-out/portable-smoke-latest-20261016-142853`. Su EXE tiene el
mismo SHA-256 `bfb03176daa0dedf9312d42178ccfa57f1bf96d4437889aad5e4a3ead5bd48ef` que release e
instalación. Con `NAYGO_PERF_METRICS=1`, la app abrió una ventana, respondió durante 10 s y cerró
limpiamente tras `--shutdown` (código 0). El perfil portable sólo creó datos en esa carpeta.

El log interno registró 16 resúmenes opt-in: 13 spans y tres contadores de workers. `listing`
registró `cancel_requested` (1 muestra, 5.749 µs), `complete` (6, p50 121.869 µs, máximo
1.040.802), `first_batch` (6, 121.864/1.040.799 µs) y `poll` (6, 35/56 µs). `pack.queue_wait`
tuvo 2 muestras (9/15 µs) y `pack.run` 2 (287/298 µs). UI: `rows_build` 8 (17/693 µs),
`rows_convert` 8 (0/265 µs), `rows_model_apply` 8 (12/170 µs), `rows_signature` 22 (4/73 µs),
`sync_layout` 4 (1.360/3.285 µs), `sync_rows` 11 (1.792/4.072 µs) y `tick` 6 (2.239/80.078 µs).
Los contadores de workers registraron preview 0/0/0, path validation 0/0/0 y pack 2 enviados,
2 iniciados, 2 terminados, pico activo 1 y 0 rechazados. No hubo entrada de usuario (`ui.input.*`);
son datos de una apertura breve en el workspace inicial, no un fixture ni una matriz, y no cierran W-07.

Después se volvió a iniciar la instalación normal y quedó abierta/respondiendo (PID 41724,
versión 0.5.2); su EXE conserva el hash de release. Los hashes de los cuatro JSON de usuario y del
PDB antiguo coincidieron antes/después del smoke portable. El PDB sigue intacto (289.722.368 bytes);
la decisión de actualizar o retirar ese archivo sigue pendiente en backlog.

### W-13 — reinspección no interactiva de la instalación activa, 2026-10-16

Una inspección UI Automation de solo lectura sobre PID 41724 encontró la ventana principal y 731
descendientes: 541 con nombre y 55 enfocables (todos con nombre); el árbol no se truncó. El proceso
respondía y el EXE instalado tenía SHA-256
`bfb03176daa0dedf9312d42178ccfa57f1bf96d4437889aad5e4a3ead5bd48ef`. Tipos expuestos: 55 botones,
146 imágenes, seis listas y 524 textos. Esta sesión no tenía grupos apilados de pestañas, por lo que
no expuso `Tab`/`TabItem`; la observación confirma el árbol general de la build vigente, pero no los
roles de pestañas, navegación por teclado, DPI ni Narrador. No se envió entrada ni se cambió el estado
de la ventana. El PDB y la configuración no se modificaron.

### W-07 — repetición pasiva sobre la instalación activa, 2026-10-16 14:43–14:44

Sobre el mismo PID 41724 se tomaron tres ventanas de 30 s (16 observaciones por ventana, intervalo
de 2 s). El árbol tuvo un único proceso, cero mapeos SMB y picos de 78,7 MiB working set, 47,7 MiB
privados, 390 handles y 26 hilos. Deltas CPU de proceso y árbol: 0,125/0,093/0,032 s. Naygo seguía
respondiendo al terminar. No se controló la inactividad ni hubo interacción; no mide latencia,
renderer, filesystem ni auxiliares más breves que el intervalo, y no cierra W-07. Registro JSON:
`target/agent-out/w07-installed-latest-20261016-1443.json`.

### Revalidación local W-12/W-14, 2026-10-16

En el checkout actual, `cargo test -p naygo-platform --locked trash::tests` pasó 8/8 pruebas,
incluidos restore directo, recibos parciales y junctions; `python scripts/check_action_pins.py`
confirmó las referencias inmutables de CI/release; y `scripts/test-signing.ps1` pasó las pruebas
sintéticas de identidad `SignPath Foundation`, sensibilidad a mayúsculas y rechazo de artefacto sin
firma. Esto no equivale a un run remoto Windows Server 2025 ni a una firma real. No se usaron
credenciales, no se publicó y no se cambió la instalación activa.

### L-02 — separación de íconos en XDG data y distribución renovada, 2026-10-16

Los íconos nuevos se importan a `data/naygo/icons`; lectura, catálogos y exportación prefieren data
y conservan fallback de la ubicación anterior `config/naygo/icons`. Los JSON de packs siguen en
config. No se migran ni eliminan archivos existentes; Windows mantiene `data_dir == config_dir`.
Pasaron 13 pruebas focales de `naygo-core` (catálogo, resolución y config), 8 pruebas de packs UI,
incluida importación a data + exportación combinada con un PNG heredado; `cargo check` y Clippy
cruzados `-D warnings` de core para `x86_64-unknown-linux-gnu` pasan. El check UI Linux sigue
pendiente por ausencia de `pkg-config` y sysroot Linux en este host. La tentativa actual de
`cargo check -p naygo-ui-slint --target x86_64-unknown-linux-gnu --locked -j 1` se detuvo en
`glib-sys 0.18.1`, que reporta que `pkg-config` no está configurado para cross-compilation; no es
evidencia de un error del código UI ni equivale a compilación Linux aprobada.

El gate protegido `scripts/build-release-guarded.ps1 -Verify` pasó con 1.408 pruebas, 0 fallidas,
8 ignoradas y Clippy sin avisos; el formato también pasó. Pico del gate: 3,49 GiB. El release
supervisado regeneró ZIP y setup; pico Job 10,46 GiB (límite 12 GiB) y margen disponible mínimo
observado sobre 14 GiB. Inno Setup compiló correctamente en 133,046 s.

| Artefacto | Tamaño | SHA-256 | Validación |
|---|---:|---|---|
| `Naygo-0.5.2-portable.zip` | 14.288.030 bytes | `1c91b9dfca51948c5c28f9b7e404d3d218a2bced40405afe405124ffd7fc47b8` | ZIP legible; sin PDB |
| `Naygo-0.5.2-setup.exe` | 65.888.176 bytes | `56b14bd77600e49a2d2d555e48929ecfd6d0edec7b59c63770ec7f4ddc696f29` | Generado; `NotSigned` |
| `naygo.exe` | 43.127.296 bytes | `95cf4b0abcd2e15f1d305c47e3a437a553ab40bc823583746187532514644490` | Coincide con release/ZIP/instalación |

Se actualizó in-place desde el setup con `debugsymbols` desmarcada (tareas previas conservadas).
Setup salió con código 0 y el PDB mantuvo su hash `eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`
(289.722.368 bytes; fecha 22 de agosto). Naygo volvió a abrir y responder como PID 14640. UI Automation
vio 312 descendientes, 228 con nombre y 46 enfocables; esto no sustituye Narrador ni la aceptación
manual. En esta etapa sólo se había validado «símbolos desmarcados»; la validación posterior de
reemplazo y desinstalación está registrada en el apartado W-15 que sigue.

### W-15 — semántica del PDB en upgrade y desinstalación, 2026-10-16

La actualización in-place del setup de distribución, con la tarea `debugsymbols` desmarcada en las
tareas anteriores, terminó con código 0 y conservó el SHA-256 del PDB antiguo. Para probar los otros
caminos sin compartir AppId ni directorio, se compiló un setup temporal con GUID independiente y se
instaló bajo `target/agent-out/w15-isolated/app`: sin símbolos no creó PDB; al simular un PDB viejo y
actualizar con la tarea desmarcada, conservó su SHA-256; al repetir con `debugsymbols` marcada,
reemplazó el marcador por el PDB release exacto (`a5e2403ae9fc6779a0356c5037d380c1e8a4850f8e7e64764204e17e5f5add71`);
el desinstalador del sandbox terminó con código 0 y eliminó el PDB registrado. La entrada Inno usa
`ignoreversion` y está condicionada por la tarea; no se necesita una regla de borrado para el upgrade
sin símbolos. El registro del desinstalador indica `NoRepair=1`, así que no hay flujo de reparación
ofrecido que validar.

El instalador de prueba conserva `CloseApplications=force` del script original y cerró por nombre el
Naygo real aunque tuviera AppId/directorio distintos; no cambió su instalación, configuración ni PDB.
Se volvió a iniciar la build actual (PID 25520; EXE coincide con el SHA-256 del release); UIA leyó
731 descendientes, 541 con nombre y 55 enfocables, sin enviar entrada. El PDB instalado real conserva
su SHA-256 `eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`. W-15 queda cubierto
para instalación sin símbolos, upgrades con/sin símbolos y desinstalación aislada.

### W-07 — validación de CLI con portable limpio

Se repitió en un portable recién extraído, sin sesión persistida, con 100 archivos sintéticos. El
árbol UIA mostró 25 filas visibles y la ruta del fixture, confirmando que `naygo.exe <carpeta>` se
aplicó. Tab/Abajo no movieron el foco de la región raíz ni produjeron métricas `ui.input.*`. UIA no
pudo hacer clic porque no había geometría de entrada disponible; la captura de frame agotó el tiempo.
La prueba se cerró sin operaciones de archivos y se restauró la instalación (PID 46948; hash del EXE
coincide con release). No equivale a interacción validada ni cierra W-07. Artefactos sintéticos en
`target/agent-out/w07-clean-20261016/`.
Un segundo lanzamiento pasó otro directorio por CLI con la sesión sintética anterior guardada; UIA
mostró 25 filas del nuevo fixture y cero de la sesión previa. Esto comprueba el override CLI pero no
la navegación por teclado; el log opt-in acumulado no registró `ui.input.*`. La instalación quedó
abierta como PID 18080 con el mismo hash de release.

### W-07 — portable aislado con fixture sintética de 10.000 archivos

El portable de distribución se abrió desde una carpeta aislada con `NAYGO_PERF_METRICS=1` y
10.000 archivos vacíos. Al cerrarlo con `--shutdown`, el log registró 256 muestras de build,
conversión y aplicación de filas; 508 firmas, 254 sincronizaciones de filas, cuatro de layout y
249 ticks. Los p50/máximos fueron: `rows_build` 313/15.705 µs, `rows_convert` 227/15.352 µs,
`rows_model_apply` 160/173.890 µs, `sync_rows` 2.957/208.427 µs y `tick` 3.060/219.616 µs.
Es trabajo interno durante la carga; no mide despacho de entrada ni presentación Winit.

La captura de ventana falló dos veces por timeout de `FrameArrived`; no se enviaron teclas y no se
registraron `ui.input.*`. Captura pasiva de 12 s (7 muestras): 82,1 MiB working set, 51,4 MiB
privados, 421 handles, 22 hilos, un proceso, 0,172 s CPU y cero mapeos SMB. No confirma primer
frame ni interacción y no cierra W-07. El JSON y log opt-in están en
`target/agent-out/w07-cu-20261016/`. Al concluir, se reabrió el release instalado sin alterar sus
archivos; PID 48100 y EXE SHA-256 `95cf4b0abcd2e15f1d305c47e3a437a553ab40bc823583746187532514644490`.

### L-01B — publicación transaccional de copias, 2026-10-16

Las copias de archivo se escriben a un temporal en el directorio destino y se publican sólo al
completar todos los bytes. Cancelar a mitad conserva el origen y la versión previa del destino;
el temporal parcial se elimina. La publicación no-clobber falla si otro proceso crea el destino
durante la copia. Para movimientos cross-volume, el origen se retira sólo después de publicar; el
movimiento rápido Linux usa `RENAME_NOREPLACE`. Se preservan permisos del archivo, incluido el
modo ejecutable POSIX.

Validación Windows: `cargo test -p naygo-core --lib` pasó 916 pruebas; `crates/platform/tests/flujos_ops.rs`
pasó 22. El gate protegido pasó 1.408 pruebas (8 ignoradas), Clippy `-D warnings` y formato.
`cargo check -p naygo-core --tests --target x86_64-unknown-linux-gnu --locked --offline` y
`cargo fmt --all -- --check` pasaron; no fue posible ejecutar Linux nativo porque WSL sigue
bloqueado por Hypervisor Platform.

El build release supervisado pasó con pico 10,45 GiB (límite 12 GiB; margen mínimo 8 GiB). Cargo
terminó en 32m18s; tras regenerar el inventario de licencias, el empaquetado final de Inno Setup
tomó 132,187 s. Artefactos sin Authenticode:

| Artefacto | Tamaño | SHA-256 | Validación |
|---|---:|---|---|
| `Naygo-0.5.2-portable.zip` | 14.301.184 bytes | `466877627e11cc30d90242a0359160bb7d5debb00620a60610f24e40e6d0e4d3` | ZIP legible; incluye EXE y avisos, sin PDB |
| `Naygo-0.5.2-setup.exe` | 65.955.641 bytes | `106e34c32bc77aa5b5200cf35885d44f7f8e873722c4048c15cae75a7506e25b` | Generado; `NotSigned` |
| `naygo.exe` | 43.136.000 bytes | `c9db875278305e660ca6db92bbc5bf63e5ee6b65120dbb1e9dcec814e27f6ab0` | Release de origen del ZIP |

Se regeneró `THIRD-PARTY-NOTICES.md` con `cargo license` para Windows: 406 paquetes,
incluyendo `tempfile 3.27.0` (Apache-2.0 OR MIT). Con la autorización previa de Nicolás, el setup
se instaló in-place en código 0 y la tarea de símbolos quedó desmarcada. El PDB antiguo conservó
SHA-256 `eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`. Naygo quedó abierto
y respondiendo como PID 12532; el EXE instalado coincide con el release (`c9db8752…`). No se
publicó ni firmó. L-01B sigue parcial: la comprobación Windows para un destino ya existente tiene
carrera entre check y rename, el reemplazo de árboles completos no es transaccional y faltan EXDEV
real, fallos de disco/permisos y aceptación Linux nativa.

W-07 actualizado, 2026-10-16: lectura pasiva del EXE instalado L-01B (PID 12532, 0.5.2) durante
3×30 s: working set 62,9–63,1 MiB, privados 31,3–31,6 MiB, 390 handles, 26 hilos y CPU 2,359 /
2,625 / 2,781 s. No se confirmó inactividad ni hubo interacción; no mide latencia ni presentación.
La prueba se detuvo antes de navegar o cambiar el estado de la sesión. El test interactivo de
W-07/W-13 requiere un workspace aislado o una sesión de prueba autorizada. Detalle/JSON en
`docs/BENCHMARKS.md` y
`target/agent-out/w07-installed-l01b-20261016.json`.

W-07 portable aislado, 2026-10-16 17:55 UTC: fixture CLI de 100 archivos en extracción limpia del
0.5.2 actual; UIA contó 23 filas visibles. `listing.first_batch`/`complete` tuvieron p50 45,955 /
45,964 µs, máx. 1,108,163 / 1,108,170 µs (12 muestras); `preview.build` 95,255 µs. `sync_rows`
p50 2,781 µs, `sync_layout` 1,808 µs, `tick` 3,279 µs. Sin `ui.input.*`. La captura de ventana
falló dos veces (`FrameArrived timed out`; `window capture timed out: timed out waiting on channel`),
por lo que no se enviaron clics/teclas y W-07 sigue abierto. El portable se cerró; se restauraron los
cuatro JSON del perfil instalado y la app actual volvió a abrir como PID 9204 respondiendo. Evidencia
en `docs/BENCHMARKS.md`; fixtures/log en `target/agent-out/w07-current-release-20261016/`.

Tras el smoke portable, los cuatro JSON instalados se restauraron byte a byte y el release actual se
relanzó como PID 9204. Sin inspección UIA durante el muestreo, 3×30 s registraron 79,1 MiB working
set, 47,5 MiB privados, 387–389 handles, 25–26 hilos y CPU 0,016/0,031/0,016 s. Respondía al
terminar. No se controló inactividad ni se midió latencia; no cierra W-07. La lectura anterior del
PID 12532 había registrado más CPU después de la inspección UIA, pero la causa no se aisló. JSON en
`target/agent-out/w07-installed-posttest-20261016.json`, resumen en `docs/BENCHMARKS.md`.

W-12 — reconsulta de solo lectura de Actions, 2026-10-16: `python scripts/check_action_pins.py`
pasó. `gh run list --limit 10` no mostró corridas posteriores a Preview Implementation review
(9-09, run 34364248486, failure); el último CI visible sigue siendo 9-07 (run 34155852541,
failure). La consulta no ejecutó ni publicó workflows. Después se descargó en `target/agent-out/`
`actionlint` v1.7.12 desde el release oficial y el SHA-256 del ZIP Windows amd64
(`6e7241b51e6817ea6a047693d8e6fed13b31819c9a0dd6c5a726e1592d22f6e9`) coincidió con el archivo
de checksums publicado en ese release. `actionlint` no emitió diagnósticos al validar `ci.yml` y
`release.yml`; el ejecutable no se instaló en el sistema. El commit local de octubre aún no tiene
evidencia de CI en Windows Server 2025; falta publicar la rama con autorización o ejecutar el workflow
actual en un runner Windows compatible.

## L-01B — reemplazo de árboles Windows: gate y distribución regenerada

Gate protegido ejecutado el 2026-10-16: 1.423 tests pasaron, 0 fallaron y 8 quedaron ignorados en
15 suites; Clippy workspace con `-D warnings`, formato, `git diff --check` y checks/Clippy cruzados
Linux para `naygo-core` y `naygo-platform` pasaron. Pruebas focales antes del gate: engine 48/48 y
journal 15/15. El proceso supervisado compiló el release en 27 min 34 s y el setup en 114,5 s; el
pico contable máximo fue 10,45 GiB, bajo el límite de 12 GiB.

Distribución 0.5.2 regenerada en `dist/`, no instalada ni abierta en este equipo:

| Artefacto | Tamaño | SHA-256 | Verificación |
|---|---:|---|---|
| `Naygo-0.5.2-portable.zip` | 14.315.707 bytes | `283ff049a1d93c37acb1aa24174f2bd86c3f49445789135598d5a6b947935b76` | ZIP válido; contiene LEEME, LICENSE, EXE y notices; sin PDB |
| `Naygo-0.5.2-setup.exe` | 65.997.533 bytes | `2bbeaf2a5bfe6034a751db28e3624449d4d682df30edf50771eb31e677eae105` | Compilación correcta; Authenticode `NotSigned` |
| `naygo.exe` | 43.174.912 bytes | `f42312f253ee8468ac75e0944fff113b1d2c4f52b1a2e688a53f2d22ba71367e` | Hash coincide con `naygo.exe` extraído del ZIP |

`dist/SHA256SUMS.txt` se actualizó y coincide con esos dos artefactos. No se ejecutó setup ni portable
tras la regeneración; la instancia instalada existente quedó intacta. L-01B continúa parcial hasta
la aceptación manual, recuperación ante pérdida de energía y los casos de disco/permisos y movimiento
cross-volume.

## L-01B — recuperación tras terminación de proceso, 2026-10-16

Se añadió un manifiesto validado y persistente antes de crear el staging, con marcador en el árbol
nuevo. Al volver a listar el destino o su padre, un worker recupera la operación: restaura el árbol
anterior si el proceso terminó entre los dos renames, o concluye limpieza si la marca confirma que
el nuevo ya se publicó. Una colisión concurrente no se sobrescribe; se conserva el respaldo y se
informa. La consulta ocurre fuera del hilo UI y respeta cancelación. El manifiesto limita nombres a
componentes directos, y el directorio reservado sólo se usa cuando su sentinel de propiedad coincide.

El gate protegido terminó en 1.433 tests aprobados, 8 ignorados, 0 fallidos; Clippy workspace con
`-D warnings`, formato y `git diff --check` pasaron. También pasaron `cargo check --tests` y Clippy
cruzados Linux para core/platform. En pruebas de proceso hijo se ejecutó `std::process::exit` justo
después de cada rename (sin destructores); el proceso padre recuperó el estado correcto. Pasan además
los tests de interrupción durante staging, colisión tardía, operación activa y directorio reservado.
No es una prueba de pérdida de energía ni garantiza durabilidad ante corte eléctrico.

La distribución Windows 0.5.2 correspondiente a este delta se regeneró el 2026-10-16 con
`scripts/build-release-guarded.ps1`. Release optimizado: 30 min 27 s; compilación del setup: 131,641 s;
pico contable del Job: 10,45 GiB, bajo el tope de 12 GiB.

Después de añadir el preflight de metadatos a `build-release.ps1`, el empaquetado se repitió con
`scripts/build-release.ps1 -SkipBuild`: reutilizó el EXE ya validado, regeneró ZIP y setup en 127,859 s
de Inno Setup, y el build quedó sin firma. Esta corrida no recompiló Rust y no tomó una medición de
pico de memoria.

| Artefacto | Tamaño | SHA-256 | Verificación |
|---|---:|---|---|
| `Naygo-0.5.2-portable.zip` | 14.920.350 bytes | `333214cbe3a203458b7bc7b7177cfd7721f953f04db6af3974bdb6f8fe9d8aee` | ZIP válido; LEEME, LICENSE, EXE y notices; sin PDB |
| `Naygo-0.5.2-setup.exe` | 66.098.383 bytes | `1eb112b77185f8e825153155268509803025a50ddc10685e400f2089941233af` | Compilación correcta; Authenticode `NotSigned` |
| `naygo.exe` | 43.209.216 bytes | `05eee5e4de2c21bba31df06a37c50e3769e01c8df02d3b572ff6a451b1e7cc36` | Hash coincide con el EXE extraído del ZIP; Authenticode `NotSigned` |

`dist/SHA256SUMS.txt` coincide con los hashes de ZIP y setup. El build guardado produjo log en
`target/agent-out/guarded-20261016-220116-636.out.log` y diagnósticos en el `.err.log` con el mismo
prefijo. No se instaló ni publicó este delta; la instancia instalada quedó intacta como PID 31084.
No se abrió el portable: la instancia única de sesión ya está ocupada, y ejecutar otra copia podría
activarla o cerrar esa instancia al enviar `--shutdown`. El smoke UI de esta distribución queda
pendiente de una sesión aislada o de cerrar Naygo manualmente.

## Instalación local de la distribución 0.5.2 más reciente — 2026-10-16 23:37

Con autorización previa de Nicolás se actualizó in-place la instalación de usuario al setup local.
Antes se confirmó que el setup coincidía con `dist/SHA256SUMS.txt` y que el EXE del ZIP tenía SHA-256
`05eee5e4de2c21bba31df06a37c50e3769e01c8df02d3b572ff6a451b1e7cc36`. El proceso anterior terminó
tras solicitar `--shutdown`. Se respaldaron `settings.json`, `workspace.json`, `favorites.json` y
`templates.json` en `target/agent-out/install-backup-20261016-latest/`; los cuatro hashes pre/post
coinciden exactamente. La tarea opcional de símbolos quedó desmarcada y `naygo.pdb` conservó tamaño
289.722.368 bytes y SHA-256 `eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`.

Tras la instalación, el EXE instalado coincide byte por byte con el portable y corre como PID 52608,
`Responding=True`, iniciado a las 23:37:25. Hashes del paquete: ZIP
`333214cbe3a203458b7bc7b7177cfd7721f953f04db6af3974bdb6f8fe9d8aee`, setup
`1eb112b77185f8e825153155268509803025a50ddc10685e400f2089941233af`. Ambos artefactos siguen sin
firma Authenticode; no se publicó ni firmó. El wrapper de PowerShell no devolvió un código de salida
legible, por lo que la verificación del resultado se basa en el hash instalado, la metadata del
desinstalador y el proceso activo. Esto confirma instalación/arranque, no la matriz manual W-13 ni
W-07 interactivo.

## Revalidación Win32 de la carrera de destino — 2026-10-16

Se añadió una prueba de contención sobre directorios temporales: en 32 iteraciones compite el
`MoveFileExW` sin reemplazo con `create_dir` sobre el mismo destino. Cada iteración comprueba que
gana exactamente una operación y que el origen o el directorio ganador conserva su contenido.
Pasaron los cuatro tests de `move_rename`, incluida una prueba sintética que confirma que
`HRESULT_FROM_WIN32(ERROR_NOT_SAME_DEVICE)` se conserva como `CrossesDevices` para activar el
fallback cancelable; también pasaron las regresiones core de destino ocupado y fallback entre
volúmenes, y `cargo fmt --all -- --check`. Es una prueba del contrato Win32 local; no reemplaza la
aceptación manual de la UI. Sólo cambió código de pruebas y documentación, por lo que no se regeneró
el ejecutable de distribución.

## W-07 — repetición de rasterizado headless en perfil release, 2026-10-16

`powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release-guarded.ps1
-W07ReleaseMeasurement` compiló el test en perfil optimizado (31 min 38 s) y ejecutó la prueba
`w10c_headless_sync_measurement` con resultado 1/1 aprobado. El Job quedó limitado a 12 GiB y su
pico contable final fue 8,67 GiB. En una pasada, las mediciones a 10k/100k filas fueron: sync
inicial 28,227/269,938 ms; no-op 0,157/1,479 ms; modificación de una fila 2,844/27,949 ms; append
2,216/21,101 ms; render p50 1,810/2,301 ms. Equipo: Windows 10 Home 22H2/build 19045, Ryzen 7
1700X, 16 hilos lógicos, Rust 1.96.0. La tabla no se mezcla con las medianas de tres corridas
anteriores. El harness crea datos en memoria y usa `MinimalSoftwareWindow`: no mide I/O, despacho
Winit, presentación física ni entrada real. W-07 sigue abierto. Log: `target/agent-out/
guarded-20261016-225158-020.out.log`.

## W-14 — preflight local de metadatos SignPath

Una prueba aislada con Inno Setup 6.7.3 fijó explícitamente `VersionInfoProductName=Naygo` y
`VersionInfoProductVersion=0.5.2`; `FileVersionInfo` aun así devuelve `ProductName` con 55 espacios
ASCII finales (longitud 60) y `ProductVersion` con 45 (longitud 50). Los archivos actuales de Naygo
EXE y setup, en cambio, reportan los valores exactos tras quitar sólo el padding final.

`NaygoSigning.psm1` ahora comprueba esos atributos y rechaza diferencias de nombre/versión,
incluidos espacios iniciales. `build-release.ps1` y el workflow release ejecutan el chequeo antes
de enviar y después de recibir cada firma; `scripts/test-signing.ps1` cubre el caso padded y las
negativas. La prueba y la comprobación de los dos PE de `dist/` pasaron; los pins de Actions también
pasaron su verificador. SignPath no documenta en su referencia el comportamiento del comparador ante
padding final: la aceptación externa sigue pendiente y las restricciones XML no se relajaron.

Revalidación local adicional del 2026-10-16: `Assert-NaygoSignPathProductMetadata` aceptó el EXE
de release y el setup vigentes como `Naygo` 0.5.2. Ambos continúan `NotSigned`: EXE
`05eee5e4de2c21bba31df06a37c50e3769e01c8df02d3b572ff6a451b1e7cc36` (43.209.216 bytes) y setup
`1eb112b77185f8e825153155268509803025a50ddc10685e400f2089941233af` (66.098.383 bytes). Esto
confirma metadatos y estado de los archivos locales, no la validación remota de SignPath ni una
firma válida.

Revisión oficial adicional de W-14, 2026-10-16: SignPath documenta `Allowed branch names` y una
referencia de rama para origin verification, mientras que su guía GitHub no especifica cómo se
trata un workflow iniciado por `push` de tag. El workflow local usa `push.tags: v*.*.*`; por lo tanto,
no se puede afirmar que el patrón actual sea aceptado por la política. Confirmar con SignPath la
referencia/patrón permitido antes de configurar o habilitar firmas. No se cambió el workflow ni se
solicitaron firmas. Fuentes: [origin verification](https://docs.signpath.io/origin-verification/),
[conector GitHub](https://docs.signpath.io/trusted-build-systems/github) y
[configuración de proyectos](https://docs.signpath.io/projects).

## W-07 — captura pasiva del release instalado actual, 2026-10-16 23:42–23:43 local

El proceso PID 52608 (EXE SHA-256 `05eee5e4…`) respondió durante tres ventanas de 30 s, con
intervalo de muestreo de 2 s. Se observó un solo proceso: working set máximo 71,4 MiB, memoria
privada 41,1 MiB, 368–370 handles, 24 hilos y CPU 0/0/0,078 s por ventana; no había mapeos SMB.
No se confirmó inactividad ni se midió interacción, latencia o presentación. Es una observación
pasiva parcial y no cierra W-07. Registro JSON: `target/agent-out/w07-process-current-20261016.json`.
