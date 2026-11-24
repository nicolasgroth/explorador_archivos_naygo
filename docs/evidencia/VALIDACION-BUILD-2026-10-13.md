# Validación del build de distribución — 2026-10-13

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Build inicial W-01–W-06A/B

El build supervisado de Naygo 0.5.2 terminó correctamente el 13 de octubre de 2026 e incluye
W-01–W-06A/B. La compilación release duró 28 min 46 s y el instalador 116,891 s. El pico contable
del Job fue 10,40 GiB, bajo el límite de 12 GiB; el margen disponible mínimo observado durante la
ejecución se mantuvo sobre 26 GiB, por encima del mínimo requerido de 8 GiB. Se usó ThinLTO, 16
unidades y un trabajo Cargo.

Artefactos de esa primera corrida en `dist/` (históricos; sustituidos por la regeneración W-06C):

| Archivo | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.005.278 bytes | `4a859d0c951a044400e387e36f84fe879ebed7c0daa3d2e0c7a70b3b7bc52288` |
| `Naygo-0.5.2-setup.exe` | 64.474.756 bytes | `0f296220c709d08546722424fbec5953a46f418d4c049119d7d0ec8f6845a279` |

Los hashes calculados coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`, `LEEME.txt`,
`LICENSE` y `THIRD-PARTY-NOTICES.md`; no contiene el PDB. Inno Setup verificó y compiló el
instalador. El script informa que está sin firma porque no hay `signtool` ni certificado
configurados. No se instaló ni publicó.

## Comprobaciones ejecutadas

- `cargo test -p naygo-ui-slint --bin naygo --jobs 1`: 359 passed, 0 failed.
- `cargo test -p naygo-core --jobs 1`: 891 unit tests + 22 flujos de operaciones aprobados; 0
  fallidos. Incluye la prueba de resolución diferida del set personalizado.
- `cargo fmt --all -- --check` y `git diff --check`: pasaron.
- Los diez catálogos de idioma pasaron `scripts/check_i18n_lang.py`.
- `graphify update .`: grafo regenerado (8.311 nodos, 15.295 aristas, 818 comunidades). El
  extractor advierte que 29 recursos producen cero nodos; no impidió la actualización.
- El supervisor se ejecutó como `scripts/build-release-guarded.ps1`, log:
  `target/agent-out/guarded-20261013-185029-280.out.log`; stderr:
  `target/agent-out/guarded-20261013-185029-280.err.log`.

En esa corrida, W-06C estaba aprobado y pendiente. La aceptación instalada sigue pendiente en W-13.

## Regeneración W-06C — 2026-10-13

La distribución 0.5.2 se regeneró tras implementar W-06C. El build release supervisado duró
29 min 06 s; Inno Setup compiló el instalador en 108,672 s. Pico contable del Job: 10,40 GiB de
12 GiB; el margen de memoria disponible se mantuvo por encima del mínimo de 8 GiB. ThinLTO, 16
unidades y un trabajo Cargo. El instalador se compiló correctamente, pero queda sin firma porque no
hay `signtool` ni certificado configurados. No se instaló ni publicó.

| Artefacto actual | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.089.263 bytes | `4c7b4b629f06f86c5fc76a129dd3f1193d77f6b661697f3333bd6c206d6c04d8` |
| `Naygo-0.5.2-setup.exe` | 64.878.703 bytes | `3def3db0491a2bf7979a3dd91be7d1873f2246de6e14d7b1fff3a55feb664253` |

Los hashes se recalcularon y coinciden con `dist/SHA256SUMS.txt`. Se abrió el ZIP para comprobar
`naygo.exe`, `LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`; no contiene el PDB. Inno Setup
reportó `Verification successful` y `Successful compile`.

**Comprobaciones de código:** `cargo test --workspace --locked --offline` aprobó 893 tests core,
22 flujos de operaciones, 54 tests platform (6 ignorados), 365 UI y las pruebas de integración
listadas en el log; 0 fallos. También pasaron `cargo check -p naygo-ui-slint`, formato y `git diff
--check`. Los logs de la ejecución final son `target/agent-out/guarded-20261013-210720-254.out.log`
y `.err.log`. Los diez catálogos (`de`, `en`, `es`, `fr`, `hi`, `it`, `ja`, `ko`, `pt`, `zh`)
pasaron `scripts/check_i18n_lang.py`. `graphify update .` actualizó el grafo a 8.366 nodos,
15.443 aristas y 817 comunidades; el extractor sigue advirtiendo que 29 recursos no producen nodos.

El gate combinado `scripts/build-release-guarded.ps1 -Verify` terminó con pruebas aprobadas, pero
Clippy (`--workspace --all-targets -- -D warnings`) reportó 16 avisos en la UI y no pasó. El log está
en `target/agent-out/guarded-20261013-205939-790.err.log`; Nicolás pidió agregarlos al backlog como
M-01. El detalle de las siete clases de avisos se conserva en el plan. Esta limitación no impidió
compilar ni verificar la distribución release.

## Regeneración W-07 — métricas opt-in, 2026-10-13

Build supervisado exitoso. Cargo release duró 26 min 13 s; Inno Setup, 113,359 s. Pico contable
del Job: 10,42 GiB de 12 GiB; mínimo de memoria disponible observado: sobre 30 GiB (umbral: 8 GiB).
Se usó ThinLTO, 16 unidades y un trabajo Cargo. Inno Setup reportó `Successful compile`; el
instalador está sin firma por falta de herramienta/certificado. No se instaló ni publicó.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.099.334 bytes | `9066c8ad156efe935bd9f5350ace9e25dc70e0727dc4c7d8568920a4396d0753d` |
| `Naygo-0.5.2-setup.exe` | 64.854.281 bytes | `2b5990366a4e016e55c4972fee8f8f342743c4c2d369d5ff404fd58d11d0626a` |

Ambos hashes se recalcularon y coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. El log del supervisor es
`target/agent-out/guarded-20261013-215808-535.out.log`; stderr se conserva en el archivo `.err.log`
del mismo nombre base.

Las métricas opt-in pasan cuatro pruebas específicas; `cargo test -p naygo-ui-slint --locked
--offline` aprobó 369 pruebas unitarias y 16 de integración/render. También pasaron formato y
`git diff --check`. `graphify update .` registró 8.400 nodos, 15.491 aristas y 822 comunidades;
persistió la advertencia de 29 recursos sin nodos. El benchmark sintético W-07 está en
`docs/BENCHMARKS.md`. W-07 sigue abierto hasta medir la matriz real de UI, CPU/memoria, auxiliares,
latencia e interacción local/UNC.

## Regeneración M-01 — Clippy limpio, 2026-10-14

Las correcciones de mantenimiento pasaron el build supervisado: Cargo release duró 26 min 18 s e
Inno Setup 107,640 s. Pico contable del Job: 10,42 GiB de 12 GiB; las muestras mostraron más de
33 GiB disponibles frente al mínimo requerido de 8 GiB. ThinLTO, 16 unidades y un trabajo Cargo.
El instalador está sin firma porque no hay `signtool` ni certificado configurados. No se instaló ni
publicó.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.099.949 bytes | `6ff5e6871d55066afb0084b3dbf0bb99be1f58c6ea4c29af3d8d059f24fcc6da` |
| `Naygo-0.5.2-setup.exe` | 64.979.564 bytes | `f8cb9d636eea6297b7705c35640201b5ee4085a7988095b43818ae0f4ef7e9a0` |

Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`; el ZIP contiene `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. Inno Setup reportó `Successful
compile`. El log de supervisor es `target/agent-out/guarded-20261013-234714-197.out.log` y su
`.err.log` asociado.

**Gate:** `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` pasó.
`cargo test --workspace --locked --offline --jobs 1` aprobó 893 core, 22 flujos de operaciones,
54 platform (6 ignorados), 369 UI y 16 pruebas de integración/render; cero fallos. Pasaron
`cargo fmt --all -- --check`, `git diff --check` y los diez catálogos i18n. `graphify update .`
actualizó el grafo a 8.406 nodos, 15.497 aristas y 821 comunidades; sigue el aviso conocido de
29 recursos sin nodos. M-01 queda cerrado. La aceptación instalada W-13 y firma W-14 continúan
pendientes.

## Actualización W-07 — contadores de workers, 2026-10-14

Se añadieron métricas opt-in de ciclo de vida en permisos reales para workers de preview y
validación de rutas: iniciados, terminados, activos al cierre, pico simultáneo y rechazos. La
instrumentación no añade rutas ni nombres de archivos. `cargo clippy --workspace --all-targets
--locked --offline --jobs 1 -- -D warnings` pasó. `cargo test -p naygo-ui-slint --locked --offline
--jobs 1` aprobó 370 unitarias y 16 de integración/render; cero fallos. También pasaron formato,
`git diff --check` y `graphify update .` (8.422 nodos, 15.532 aristas, 823 comunidades; siguen 29
recursos sin nodos).

El empaquetado supervisado concluyó correctamente. Cargo release tardó 25 min 16 s; Inno Setup,
106,484 s. Pico contable del Job: 10,41 GiB de 12 GiB; memoria disponible mínima observada: más de
33 GiB (umbral: 8 GiB). ThinLTO, 16 unidades y un trabajo Cargo. Inno Setup reportó
`Successful compile`; el instalador está sin firma por falta de herramienta/certificado. No se
instaló ni publicó.

| Artefacto actual | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.100.820 bytes | `7429762749a6cd0816c4e3cf80e59a4100fe098e1b4ea1f173effdb0d4fb34b7` |
| `Naygo-0.5.2-setup.exe` | 64.995.711 bytes | `3222466a4804138dbd8e266d4f651d4f7fbdc23eaec71d4629e499a7b1f3d7f2` |

Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. Log del supervisor:
`target/agent-out/guarded-20261014-004110-765.out.log`; stderr en el `.err.log` asociado.
W-07 continúa abierto: falta contador de cola de packs, cierre integrado con métricas habilitadas y
medición real de UI/CPU/memoria/red.

## W-08 — presupuesto de recogida por listado, 2026-10-14

Cada `Listing::poll()` recoge hasta 1.024 entradas o 3 ms. El productor usa una cola de 1.024 filas
con reintentos cancelables y no bloqueantes; el terminal va en un canal lateral no acotado para que
una cola llena no impida notificar cancelación. La UI retiene `Finished` y vuelve a drenar filas
antes de entregarlo, preservando el orden entre canales. Pasaron `cargo test --workspace --locked
--offline --jobs 1` (894 core, 22 flujos, 60 platform/7 ignoradas, 371 UI y 16 integration/render)
y `cargo clippy --workspace --all-targets --locked --offline --jobs 1 -- -D warnings`, formato y
diff. El límite aún es por listado, no round-robin global.

Actualización de equidad 2026-10-14: el tick comparte 1.024 filas/3 ms entre los listados de Files
y árbol; al competir, reserva 512 para cada grupo y rota el primer worker para que paneles o ramas
no queden postergados por el mismo orden de HashMap. Si sólo un grupo está ocupado, consume el
presupuesto completo. Pasó `cargo test --workspace --locked --offline --jobs 1` (894 core, 22 flujos,
60 platform/7 ignoradas, 372 UI + 16 integración/render) y Clippy global con `-D warnings`, formato
y diff. La nueva versión queda pendiente de build supervisado antes de considerarla distribuida.

La distribución con el canal acotado se regeneró correctamente. Cargo release tardó 26 min 03 s e
Inno Setup, 106,000 s. Pico contable del Job: 10,41 GiB de 12 GiB; mínimo de memoria disponible
observado: más de 32 GiB (umbral: 8 GiB). ThinLTO, 16 unidades y un trabajo Cargo. Inno Setup
reportó `Successful compile`; el instalador sigue sin firma porque no hay herramienta/certificado.
No se instaló ni publicó.

| Artefacto actual | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.113.512 bytes | `4b09f38a2634bdd3bfca9c361b97ecc8eff1800466877a0deb80fa5d20333a47` |
| `Naygo-0.5.2-setup.exe` | 65.003.385 bytes | `51e8cd4a072f6582fb5194f9d7e5b977489eb65113c0093085de9fde93039085` |

Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. Log del supervisor:
`target/agent-out/guarded-20261014-022308-917.out.log`; stderr en el `.err.log` asociado.

Actualización de equidad W-08, 2026-10-14. El tick comparte ahora el presupuesto de 1.024 filas/
3 ms entre Files y árbol, con cuotas y rotación probadas. El workspace completo, Clippy con
`-D warnings`, formato y diff pasaron antes del build; Cargo release terminó en 25 min 15 s e Inno
Setup reportó `Successful compile` en 107,953 s. No se instaló ni publicó. El instalador sigue sin
firma porque no hay `signtool` ni certificado configurados.

| Artefacto actual | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.113.748 bytes | `7d2c9a4d24fb9d1fce564d66a8de12d82dfb19b6a7d7a0aba881edc34b22b7d0` |
| `Naygo-0.5.2-setup.exe` | 64.871.629 bytes | `42cb6ed80af974c9a873ac479e3a65cd29733f5705f9d99fb788ea0e132019e2` |

Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`. El ZIP incluye `naygo.exe`, `LEEME.txt`,
`LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. Logs: `target/agent-out/guarded-20261014-030836-531.out.log`
y `.err.log`. W-08 queda abierto sólo para comparar primera fila, p95 de interacción y tiempo total
contra la línea base; W-07 conserva sus mediciones reales pendientes.

## W-09 — admisión de workers y probes, 2026-10-14

El workspace completo pasó: 895 pruebas core, 22 flujos de operaciones, 54 platform (6 ignoradas),
373 UI y 16 integración/render. Clippy global con `-D warnings`, `cargo fmt --check`,
`git diff --check` y `graphify update .` también pasaron. El build supervisado terminó correctamente:
Cargo Release en 26 min 09 s, pico contable del Job 10,40 GiB de 12 GiB, memoria disponible mínima
muestreada sobre 32 GiB (umbral 8 GiB), ThinLTO/16 unidades/jobs=1. Inno Setup reportó
`Successful compile` en 111,157 s. El instalador no tiene firma; no se instaló ni publicó.

| Artefacto actual | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.115.800 bytes | `682864525799f036d4c19b6d267311487bf9b0aa47f0f1b9b8c308a9c1af3d2a` |
| `Naygo-0.5.2-setup.exe` | 64.911.503 bytes | `279d2251401e368c6a625efa0e80f6dd2200c13643dadae86f3fea45c2c014d1` |

Los hashes se recalcularon y coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. Logs: `target/agent-out/guarded-20261014-041601-059.out.log`
y `.err.log`. W-09 queda parcialmente abierto sólo para aceptación visual del estado de espera y
prueba con un `read_dir` de red realmente bloqueado; la syscall no se anuncia como cancelable.

## W-10A — vista cacheada compartida, 2026-10-14

Suite workspace aprobada: 896 pruebas core, 22 flujos de operaciones, 54 platform (6 ignoradas),
373 UI y 16 integración/render. `cargo clippy --workspace --all-targets --locked --offline --jobs 1
-- -D warnings`, `cargo fmt --all -- --check`, `git diff --check` y `graphify update .` pasaron.
El benchmark release registró el hit cacheado compartido en 0,000057 ms (10.000 filas) y
0,000061 ms (100.000); la API que clona Vec marcó 0,002137 ms y 0,021924 ms, respectivamente.

Build supervisado `target/agent-out/guarded-20261014-050802-650.out.log`; Cargo release tardó
26 min 01 s, pico contable 10,41 GiB de 12 GiB y mínimo muestreado de memoria disponible 32,02 GiB
(umbral 8 GiB). ThinLTO, 16 unidades de código y un trabajo Cargo. Inno Setup reportó `Successful
compile` en 106,407 s. El instalador permanece sin firma; no se instaló ni publicó.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.115.698 bytes | `dd9ee400e8dca705b42ee01717ba5a108019e6937408e3fa3a4001efb4bdd978` |
| `Naygo-0.5.2-setup.exe` | 65.089.571 bytes | `750c419faf0f419c61454e4685541f6f0c51cd3db9ed01dbeba18794d79c3143` |

Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene sólo `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB.

## W-11A — límites y parse único del preview PDF, 2026-10-14

`cargo test -p naygo-ui-slint --locked --offline --jobs 1 pdf_` pasó las cuatro pruebas dirigidas.
La suite workspace (896 core, 22 flujos, 54 platform/6 ignoradas, 376 UI + 16 integración/render),
Clippy global con `-D warnings`, formato, diff y Graphify pasaron antes del empaquetado.

Build supervisado `target/agent-out/guarded-20261014-064512-790.out.log`; Cargo release terminó en
29 min 54 s con pico del Job de 10,41 GiB de 12 GiB y margen libre mínimo observado de 16,19 GiB
(umbral 8 GiB), ThinLTO/16 unidades/jobs=1. Inno Setup reportó `Successful compile` en 134,969 s.
El instalador sigue sin firma por falta de `signtool`/certificado. No se instaló ni publicó.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.116.608 bytes | `237ae27e8d481460b64ffa8cec29a8fd9742934ea289e9784bda15a0dd73a9a7` |
| `Naygo-0.5.2-setup.exe` | 64.961.078 bytes | `a4bb0aa4e26d9c38214bf244f2365a008e6e3baa61c153acf38a50324469d0a7` |

Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene sólo `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. La medición con corpus PDF y aceptación
instalada continúan pendientes.

## W-11B — límites del preview ZIP/TAR/GZIP, 2026-10-14

`cargo test --workspace --no-fail-fast --locked --offline --jobs 1` pasó: 896 core, 22 flujos de
operaciones, 54 platform (6 ignoradas), 384 UI y 16 pruebas de integración/render. Las 28 pruebas
del módulo preview pasan, incluida la expansión TAR.GZ que se corta con listado parcial. Clippy
global con `-D warnings`, `cargo fmt --all -- --check`, diff, paridad de los diez idiomas y Graphify
pasaron.

Build supervisado `target/agent-out/guarded-20261014-080649-329.out.log`; Cargo release tardó
37 min 22 s, pico contable del Job 10,41 GiB de 12 GiB y margen libre mínimo observado de 16,23 GiB
(umbral 8 GiB), ThinLTO/16 unidades/jobs=1. Inno Setup reportó `Successful compile` en 182,125 s.
El instalador sigue sin firma por falta de `signtool`/certificado. No se instaló ni publicó.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.126.285 bytes | `8512630f7bf668004db1a0dd54e29eef21d26c607547f961125003fc10c35cb1` |
| `Naygo-0.5.2-setup.exe` | 64.852.734 bytes | `b49038c0d0fa8a1505159016f13007e5e1b2f5dbeccc49863c0474622072913d` |

Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene sólo `naygo.exe`,
`LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`, sin PDB. Aceptación instalada y validación con
filesystem/red reales siguen pendientes.

## W-11C — límites de texto/SVG/imágenes, 2026-10-14

El workspace completo pasó: 898 core, 22 flujos de operaciones, 60 platform (6 ignoradas), 387 UI,
pruebas de integración y render. Clippy global con `-D warnings`, formato, diff y paridad de los diez
catálogos pasan. Las pruebas dirigidas validan cortes UTF-8/UTF-16, SVG con 4.000 elementos,
canvas explícito, PNG de anchura 16.385 px rechazado por el límite del decoder, animación pausada,
cambio rápido y proceso aislado. Graphify se actualizó a 8.539 nodos / 15.861 aristas; sus avisos
conocidos de JSON sin nodos y nombres de comunidades sin refrescar siguen informativos.

El corte del buffer ya no transforma un texto UTF-8 válido en Windows-1252 cuando el tope cae dentro
de un carácter. Para UTF-16, descarta una unidad impar y un surrogate alto final sin pareja. El
canvas SVG se detecta con búsqueda ASCII directamente sobre el buffer, evitando dos copias de hasta
20 MiB. El estrés sintético cubre los límites, no sustituye la comparación de latencia/memoria con
corpus real, pendiente dentro de W-07.

Build supervisado `target/agent-out/guarded-20261014-091924-182.out.log`; Cargo release terminó en
45 min 44 s, con pico contable de 10,41 GiB de 12 GiB y margen mínimo observado de 18,22 GiB,
ThinLTO/16 unidades/jobs=1. Inno Setup reportó `Successful compile` en 176,422 s. El instalador
sigue sin firma por falta de `signtool`/certificado; no se instaló ni publicó. El smoke local del
EXE se omitió porque había una instancia Naygo en ejecución; el workflow de release probará
`--shutdown` desde un runner efímero después de extraer el ZIP.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.126.462 bytes | `3cc905233ce9d5e0791e9fb6902425eb21fdc9e50c098be8a8d02c283b2b9511` |
| `Naygo-0.5.2-setup.exe` | 65.047.090 bytes | `21ad9c124e88bdcfa4b9268a3e62d960df513253160f609828b96913ecc88cb7` |

Los hashes coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`, `LEEME.txt`, `LICENSE`
y `THIRD-PARTY-NOTICES.md`, sin PDB; ISCC confirmó que el setup incorpora `naygo.pdb` para
diagnóstico. Metadatos verificados: producto Naygo, versión 0.5.2, autor ISGroth — Nicolás Groth.
La prueba sintética de UI de 30 casos terminó en 5,54 s; no representa una métrica de interacción.
