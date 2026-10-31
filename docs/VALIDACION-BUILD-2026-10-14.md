# Validación de distribución — 2026-10-14

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Build supervisado

- Comando: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-release-guarded.ps1`.
- Registro: `target/agent-out/guarded-20261014-115314-553.out.log` y el `.err.log` asociado.
- Toolchain/proyecto: Naygo 0.5.2, Rust 1.96.0, ThinLTO, 16 unidades y un trabajo Cargo.
- Cargo terminó el perfil release en 29 min 02 s; Inno Setup compiló el instalador en 126,078 s.
- Pico contable del Job: 10,42 GiB bajo el límite de 12 GiB. Margen observado del host al pico:
  17,49 GiB; el proceso terminó con más de 25 GiB disponibles.
- El empaquetador avisó que `naygo.exe` y `setup.exe` no tienen firma Authenticode; no había
  certificado local ni configuración SignPath. El estado `NotSigned` de ambos fue verificado.
  Esto no equivale a una ejecución de firma ni valida aún el servicio remoto.

## Artefactos comprobados

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.126.764 bytes | `c7a9f81fb42e3b6d80c86fd8fde7b3a4e496ce980ef5a370b432446f5b2683a1` |
| `Naygo-0.5.2-setup.exe` | 65.170.380 bytes | `005e94354372b6897281202790fac50ea6fbe443d556371b7810db64cbfc9d70` |

Comprobaciones: los dos hashes coinciden con `dist/SHA256SUMS.txt`; el ZIP contiene cuatro
archivos (`naygo.exe`, licencia, LEEME y avisos de terceros), no lleva PDB y el EXE declara
`ProductName=Naygo`, `ProductVersion=0.5.2`. No se ejecutó ni instaló Naygo en la sesión del usuario.

La firma SignPath queda pendiente de aceptar/configurar el proyecto OSS, registrar credenciales y
variables en GitHub, publicar la política exigida por SignPath y completar una ejecución aprobada.
El workflow de release preparado debe verificarse en esa primera ejecución antes de anunciar un
artefacto firmado. La aceptación interactiva W-13 y el run remoto de CI W-12 también siguen abiertos.

## Delta W-07 — worker serial de packs

Tras incorporar contadores y tiempos opt-in para la cola de packs se ejecutaron de nuevo los gates y
la distribución, el 14 de octubre. El comando supervisado terminó correctamente. Cargo release
compiló en 27 min 02 s; Inno Setup terminó en 111,829 s; pico contable del Job: 10,41 GiB (límite
12 GiB). Log: `target/agent-out/guarded-20261014-125418-281.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.128.665 bytes | `f7266557b4d210f2d26ba596ea3f1245990ab2ed6dc8042f24664bf60ccfd8fe` |
| `Naygo-0.5.2-setup.exe` | 65.130.591 bytes | `812178e5d2689972d39ac7e074d3fc64f8888a1df9638a546cb1aef239f48a20` |

Verificación: los hashes coinciden con `dist/SHA256SUMS.txt`; el ZIP conserva los cuatro archivos
esperados y excluye el PDB. `target/release/naygo.exe` informa ProductName Naygo, ProductVersion
0.5.2 y CompanyName ISGroth — Nicolás Groth, pero está `NotSigned`; el instalador también está
`NotSigned`, como se esperaba sin SignPath ni certificado local. La firma Authenticode del ZIP no
aplica (no es un PE). No se ejecutó ni instaló la aplicación.

Gates: `cargo test --workspace --no-fail-fast --locked --jobs 1` (898 core, 22 flujos, 54 platform
con 6 ignorados, 388 UI + 16 integración/render), Clippy global con `-D warnings`, `cargo fmt`,
`git diff --check`, diez catálogos i18n y Graphify update. La instrumentación W-07 queda completa
en el código local, pero las mediciones reales de interacción, CPU/RAM, filesystem local/UNC y la
matriz de preview siguen pendientes; el build no las sustituye.

## Delta W-14 — rechazar firmas Authenticode inválidas

Se endureció `scripts/build-release.ps1`: sin certificado local permite sólo `NotSigned` o firma
`Valid`; HashMismatch/NotTrusted/UnknownError y otros estados invalidan el empaquetado. Con
certificado, también rechaza firmas presentes que no validan antes de invocar SignTool. En pruebas
de PowerShell, el EXE local `NotSigned` continuó por el camino de desarrollo, un EXE firmado por
Windows con estado `Valid` se preservó, y un `HashMismatch` sintético fue rechazado tanto con como
sin certificado configurado.

Build supervisado: Cargo release 26 min 20 s; Inno Setup 112,156 s; pico Job 10,40 GiB (límite
12 GiB). Registro: `target/agent-out/guarded-20261014-133336-647.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.128.579 bytes | `6e12cb3ac532301008d4d952cbd7fc2efdc59d0aeaed33f3fb71fc9570b895f0` |
| `Naygo-0.5.2-setup.exe` | 65.130.575 bytes | `5bce128b2f6ca7baf0372a3dc9a102976629240c64cb8502ab92a0d0181cffd7` |

Los hashes coinciden con `dist/SHA256SUMS.txt`; el ZIP contiene las cuatro entradas esperadas y no
incluye el PDB. El EXE conserva ProductName Naygo, ProductVersion 0.5.2 y CompanyName ISGroth —
Nicolás Groth; EXE e instalador están `NotSigned`, al no existir certificado/configuración
autorizada. La firma Authenticode consultada sobre el ZIP no aplica porque el ZIP no es un PE. No se
instaló, ejecutó, firmó ni publicó la aplicación.

## Delta L-01A — parser POSIX y regeneración supervisada

Después del subpaso de `path_input`, se repitió el build para mantener actualizada la distribución
Windows. Comando: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release-guarded.ps1`.
Log: `target/agent-out/guarded-20261014-141925-800.out.log` y `.err.log`. Cargo release terminó en
29 min 01 s; Inno Setup en 129,172 s; pico contable 10,41 GiB (límite 12 GiB). El host mantuvo más
de 15 GiB disponibles durante la fase de mayor carga.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.129.085 bytes | `5596592a96c6b79288459fcabe2d94ca322203b037c2fa067e6dde8f172afbfc` |
| `Naygo-0.5.2-setup.exe` | 65.073.335 bytes | `fccf37318bc622c89025c147658ae6632e707e7b64da9f4d64ed4b36ad5453fd` |

Los hashes coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`, `LICENSE`, `LEEME.txt`
y `THIRD-PARTY-NOTICES.md`, sin PDB; el hash del ejecutable embebido coincide con
`target/release/naygo.exe`. El ejecutable declara `ProductName=Naygo`, `ProductVersion=0.5.2` y
`CompanyName=ISGroth — Nicolás Groth <ngroth@gmail.com>`. Ejecutable e instalador reportan
`NotSigned`; la firma Authenticode sigue pendiente de W-14/SignPath. No se instaló ni ejecutó la
aplicación.

## Delta L-01A — identidad POSIX y verificación final, 2026-10-14

El helper de identidad nativa se extendió a guards de operaciones, sync, espacios de tarea,
búsquedas guardadas, cesta, entregas, radar de destinos y retorno a selección. Windows conserva el
criterio histórico case-insensitive. El gate completo pasó con 1.380 tests, 0 fallidos y 7 ignorados;
Clippy, `cargo fmt --check`, `git diff --check` y Graphify también pasaron. Log de gate:
`target/agent-out/guarded-20261014-163645-675.out.log` y `.err.log`; pico supervisado 5,90 GiB.

Se volvió a generar la distribución con `scripts/build-release-guarded.ps1`. Log:
`target/agent-out/guarded-20261014-155535-156.out.log` y `.err.log`. Cargo release terminó en
33 min 47 s; Inno Setup en 127,563 s; pico del Job 10,41 GiB (tope 12 GiB). El host conservó al
menos 13 GiB disponibles durante la compilación.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.130.706 bytes | `fb5937317d136bffaf35a50bac126077eea0832b6069401275eee0e9bd922519` |
| `Naygo-0.5.2-setup.exe` | 64.925.807 bytes | `63d56ce624a39d39a0833601a70f24327f2d11e24ebb83a3e956330879c60596` |

Los hashes coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene los cuatro archivos esperados, no
incluye PDB y el hash de su `naygo.exe` (`4640c0c26c812ba9d7a4de1e4d34f2dfe70a0511b3164f35628f7920e64331e2`)
coincide con `target/release/naygo.exe`. El EXE y el setup siguen `NotSigned`; no se instaló ni
ejecutó la aplicación.

## Delta W-12 — restore de Papelera compatible con runners sin Explorer

El callback `PostDeleteItem` conserva el nombre de parsing del `IShellItem` que Windows creó en la
Papelera. La restauración intenta abrir ese ítem directamente y moverlo con `IFileOperation::MoveItem`;
si el proveedor Shell no admite esa operación, conserva el fallback de enumeración y el verbo
`undelete`. Esto evita depender de que Explorer refresque su vista virtual o registre ese verbo.
El test real de ida/vuelta se quitó de `#[ignore]`, usa un directorio temporal, verifica el ítem
reportado por el callback y restaura el archivo. Pasó localmente. El runner remoto Windows Server
2025 aún debe confirmar el flujo antes de cerrar W-12.

Gate local: `scripts/build-release-guarded.ps1 -Verify` pasó con 1.381 tests, 0 fallidos, 6
ignorados y Clippy sin avisos. También pasaron formato, `git diff --check`, catálogos i18n y Graphify.
Registros: `target/agent-out/guarded-20261014-174030-400.out.log` y `.err.log` (pico 8,36 GiB).
La distribución se generó con `scripts/build-release-guarded.ps1`; Cargo release terminó en 28 min
56 s, Inno Setup en 116,891 s y el pico contable fue 10,41 GiB (límite 12 GiB). Registros:
`target/agent-out/guarded-20261014-180051-018.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.134.305 bytes | `20b66ba5990b8a7ced8f269b799414672cec5cd55e32d9fba5e7709c7354ff18` |
| `Naygo-0.5.2-setup.exe` | 65.200.888 bytes | `7c9d913931093a7b2d139628c351d0efb45b20c7a922774576c135963feafc17` |

Los hashes coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `LEEME.txt`, `LICENSE`, `naygo.exe`
y `THIRD-PARTY-NOTICES.md`, sin PDB; el hash del ejecutable interno coincide con
`target/release/naygo.exe` (`6820d7e77e5799bdc6761730a3178fc2f507bf113350dd3b824fd393c70e2aa3`).
`ProductName=Naygo`, `ProductVersion=0.5.2`, `CompanyName=ISGroth — Nicolás Groth <ngroth@gmail.com>`;
EXE y setup permanecen `NotSigned`. No se instaló, ejecutó, firmó ni publicó la aplicación.

## Delta L-03 — limpiar warnings cruzados y regenerar Windows

El target `x86_64-unknown-linux-gnu` reveló diez warnings por helpers/imports/tests que sólo usa
Windows; Clippy cruzado encontró tres llamadas a `drop` que sólo cierran recursos Windows. Se
limitaron esos elementos por plataforma y se añadieron tests para los stubs Linux de espacio,
carpetas conocidas y Papelera. `cargo check -p naygo-core -p naygo-platform --tests --target
x86_64-unknown-linux-gnu --locked --jobs 1` y el Clippy equivalente con `-D warnings` pasan sin
avisos. Esto sólo valida compilación cruzada. WSL Ubuntu falla con `HCS_E_HYPERV_NOT_INSTALLED`; no
se hizo build ni ejecución nativa Linux. La suite Windows completa pasó con 1.381 tests, 6
ignorados y Clippy limpio; el gate registró pico 5,91 GiB. Log:
`target/agent-out/guarded-20261014-184002-732.out.log` y `.err.log`. `cargo test -p naygo-platform
--lib --locked --jobs 1` pasó 54 tests, con 6 ignorados.

La distribución Windows se regeneró con `scripts/build-release-guarded.ps1`. Cargo release terminó
en 28 min 06 s, Inno Setup en 124,453 s y el pico contable fue 10,42 GiB (límite 12 GiB). Log:
`target/agent-out/guarded-20261014-185240-185.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.134.394 bytes | `0a272196a57d66ec8bb9a424d04a4579b4845ff2a1df11a6e6abe3b41ccf03c9` |
| `Naygo-0.5.2-setup.exe` | 65.170.947 bytes | `4b95920aba900aafa4c0216342da4b72f6e1f396e9b2edc779c4e845820e2493` |

Los hashes coinciden con `dist/SHA256SUMS.txt`. El ZIP conserva las cuatro entradas esperadas, sin
PDB; el ejecutable embebido y `target/release/naygo.exe` comparten el hash
`8179d101b6757f42455766dcfcea7229e1a2f67361797f846d82f0a1a217a53b`. ProductName/Version/Company
siguen siendo Naygo/0.5.2/ISGroth — Nicolás Groth; EXE y setup permanecen `NotSigned`. No se
instaló, ejecutó, firmó ni publicó la aplicación. W-12 aún requiere su run remoto y L-03 requiere
compilación/ejecución nativa Ubuntu.

## Evidencia Windows W-07 — repetición sintética

El 14 de octubre se repitió tres veces el benchmark de modelo en Windows 10 Home 19045, Ryzen 7
1700X, Rust 1.96.0, `x86_64-pc-windows-msvc`, perfil `bench`; el JSONL completo está en
`target/agent-out/w07-windows-repeat-20261014-193105.jsonl`. Las medianas externas de 100.000 filas
fueron 124,9877 ms (vista inicial), 2,4532 ms (sort por tamaño), 128,6979 ms (filtro+sort) y
20,3729 ms (índice de cambios). Comparadas con la base del 13 de octubre, variaron +6,2 %, +9,4 %,
+7,0 % y +1,0 %, respectivamente. Ninguna supera el umbral orientativo de 15 %; al ser sintéticas,
estas medidas no prueban la latencia de interfaz, el uso de memoria/CPU ni el comportamiento local/UNC.
No se regeneró la distribución: esta delta sólo actualiza evidencia y documentación W-07.

## Gate Windows — helper XDG y regresión de Papelera

`powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-release-guarded.ps1 -Verify`
terminó con código 0 el 14 de octubre. Pasaron 904 tests core, 22 flujos de operaciones, 54 tests
platform (6 ignorados), 388 tests unitarios UI y 16 tests UI de integración/render. Clippy global
con `-D warnings` pasó; el pico contable fue 9,91 GiB (límite 12 GiB). El test de ida/vuelta de
Papelera quedó cubierto en la suite Windows.

La distribución se regeneró con `powershell.exe -NoProfile -ExecutionPolicy Bypass -File
scripts/build-release-guarded.ps1`. Cargo release terminó en 28m28s; Inno Setup compiló el
instalador en 115,625 s. Pico contable del Job: 10,41 GiB (límite 12 GiB). Logs:
`target/agent-out/guarded-20261014-195858-086.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.134.488 bytes | `4897fd4175ed7f2ff43687f5785fd4d79b775095432f595fb43ea389955f5a60` |
| `Naygo-0.5.2-setup.exe` | 65.150.434 bytes | `32f07063c596adbe99a8457655ed92ebf611d6bf985e6a4432fcf415b2edc2fe` |

Los hashes coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene `naygo.exe`, `LICENSE`, `LEEME.txt`
y `THIRD-PARTY-NOTICES.md`, sin PDB; el EXE embebido y `target/release/naygo.exe` coinciden en
SHA-256 (`f7e43e2aad2cb718edb5c25a569fc26933033300fce9d374e700415e0cefbaa4`). ProductName,
ProductVersion y CompanyName son `Naygo`, `0.5.2` e `ISGroth — Nicolás Groth <ngroth@gmail.com>`.
El EXE y setup permanecen `NotSigned`; no se ejecutó, instaló, firmó ni publicó la app. W-12 todavía
necesita un run remoto con la corrección de Papelera.

## Delta W-07 — desglose de sincronización Slint

Se agregaron spans opt-in `ui.rows_signature`, `ui.rows_build`, `ui.rows_convert` y
`ui.rows_model_apply`, además de conservar `ui.sync_rows` como total. La captura apagada no lee el
reloj para estos spans. Esta instrumentación no modifica el modelo; hace falta ejecutar Naygo con
`NAYGO_PERF_METRICS=1` y recopilar una sesión interactiva para decidir W-10B. No se ejecutó la app
en la sesión del usuario.

Gate: `scripts/build-release-guarded.ps1 -Verify` pasó con 904 tests core, 22 flujos de operaciones,
54 platform (6 ignorados), 388 UI y 16 tests de integración/render; Clippy global `-D warnings`
limpio; pico Job 5,94 GiB. El test `trash::tests::enviar_un_archivo_a_papelera` volvió a pasar.
Logs: `target/agent-out/guarded-20261014-203338-862.out.log` y `.err.log`.

La distribución se regeneró con el comando supervisado. Cargo release terminó en 27m34s, Inno Setup
en 115,328 s y el pico fue 10,41 GiB (límite 12 GiB). Logs:
`target/agent-out/guarded-20261014-204342-479.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.135.771 bytes | `0fa4be67f83afbe1ed4c1e912547af755e9817ecf4e6fc31396fb2b2c8f9e334` |
| `Naygo-0.5.2-setup.exe` | 65.136.467 bytes | `b59af753eb8419f6218321d432584bf026034eded51840157b9ec19166c08f34` |

Los checksums coinciden con `dist/SHA256SUMS.txt`; el ZIP trae los cuatro archivos esperados y no
incluye PDB. El EXE embebido coincide con `target/release/naygo.exe` (SHA-256
`c59caca4accbf58c6d8df402ac30196eaf712b7a3ec031ae3684d59ecd69d318`); ProductName/Version/Company
siguen como Naygo/0.5.2/ISGroth — Nicolás Groth. Ambos ejecutables permanecen `NotSigned`.

## Refuerzo W-12 — restore sin proveedor de enumeración

El 14 de octubre se cambió `restore_from_trash` para resolver primero el nombre de parsing retenido
por `PostDeleteItem`; `FOLDERID_RecycleBinFolder` y `BHID_EnumItems` se consultan únicamente si el
recibo no conserva esa identidad. Así, la restauración normal ya no requiere que el runner tenga un
enumerador de Papelera funcional. `powershell.exe -NoProfile -ExecutionPolicy Bypass -File
scripts/build-release-guarded.ps1 -Verify` terminó con código 0: suite del workspace aprobada,
incluido `trash::tests::enviar_un_archivo_a_papelera`, Clippy global `-D warnings` limpio y pico
contable de 7,82 GiB (límite 12 GiB). Logs: `target/agent-out/guarded-20261014-212120-997.out.log`
y `.err.log`.

Después se regeneró la distribución con `powershell.exe -NoProfile -ExecutionPolicy Bypass -File
scripts/build-release-guarded.ps1`. Cargo release terminó en 32m03s; Inno Setup terminó en 138,453 s;
pico contable del Job: 10,42 GiB (límite 12 GiB). Logs:
`target/agent-out/guarded-20261014-213842-166.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.136.171 bytes | `0177efa1ce616c32f29c0c1195a3dba05b5b33ac772f67ed5f296a4a6c41711a` |
| `Naygo-0.5.2-setup.exe` | 65.004.421 bytes | `646c300fab661b9bc52c32d6eddea96c8a2fb8be60e10963b2b07063270655c3` |

Los valores coinciden con `dist/SHA256SUMS.txt`. El ZIP conserva exactamente `LEEME.txt`, `LICENSE`,
`naygo.exe` y `THIRD-PARTY-NOTICES.md`, sin PDB. El ejecutable dentro del ZIP coincide con
`target/release/naygo.exe` (SHA-256 `4e4e577f14013dd4fd71f7e46ba29001595372c55294bbdba562b689874e51a1`).
Producto y versión son Naygo/0.5.2; el instalador mantiene padding en campos PE y la validación
normaliza espacios finales. Ambos artefactos siguen `NotSigned`. No se instaló ni ejecutó la app;
W-12 aún requiere una corrida real de GitHub Actions en Windows Server 2025.

## Prueba dirigida de retención FIFO del recibo

La suite Windows pasó `trash::tests::cache_de_recibos_conserva_los_mas_recientes_en_orden_fifo`:
al llegar a 1.025 claves, la más antigua se expulsa; reinsertar una ruta la conserva por encima de
claves más antiguas. Este test y la ida/vuelta de Papelera pasaron junto a la suite completa y
Clippy global `-D warnings`, en el gate supervisado iniciado a las 22:34 del 14 de octubre; pico
5,93 GiB (límite 12 GiB). Log: `target/agent-out/guarded-20261014-223445-490.err.log`.

La distribución se regeneró con `scripts/build-release-guarded.ps1`. Cargo release terminó en
31m11s; Inno Setup en 114,485 s; pico contable 10,41 GiB (límite 12 GiB). Logs:
`target/agent-out/guarded-20261014-225043-828.out.log` y `.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.137.766 bytes | `3442b7bb43874a82b085f6b2419f418e88737bfee96d4d4dc917ca5d3fc1eb20` |
| `Naygo-0.5.2-setup.exe` | 65.137.036 bytes | `04acfcc6086b8a5ace40e30ce1444fe4ee505d01825b159ee06415b0f3934ffa` |

Los hashes coinciden con `dist/SHA256SUMS.txt`. El ZIP contiene únicamente `LEEME.txt`, `LICENSE`,
`naygo.exe` y `THIRD-PARTY-NOTICES.md`; no incluye PDB. El EXE embebido coincide con el release
(SHA-256 `56ae11d9e48d97ee8e32062002abb0dd08527117d408e3e2d24617cb944d037f`). Ambos ejecutables
siguen `NotSigned` y sus metadatos son Naygo/0.5.2; el instalador declara ISGroth. No se instaló ni
ejecutó la app. W-12 aún requiere una corrida GitHub Actions en Windows Server 2025.

## Prueba dirigida de ausencia del enumerador

El test `trash::tests::enviar_un_archivo_a_papelera` ejecutó en Windows el borrado y restore reales,
inyectando un enumerador que devuelve error si se llama. Pasó y confirmó cero llamadas: el recibo
capturado por `PostDeleteItem` restauró el archivo directamente. El gate supervisado iniciado a las
23:28 del 14 de octubre pasó con suite workspace y Clippy global `-D warnings`; pico 5,89 GiB.
Log: `target/agent-out/guarded-20261014-232844-545.err.log`. La distribución final incluye esta
última delta. W-12 sigue pendiente únicamente del run remoto Windows Server 2025.

## Distribución final con el refuerzo de enumeración diferida

El build supervisado `scripts/build-release-guarded.ps1` terminó correctamente el 15 de octubre.
Cargo release tardó 26m28s; Inno Setup, 111,860 s; el pico del Job fue 10,41 GiB, bajo el límite
de 12 GiB. Logs: `target/agent-out/guarded-20261014-233853-739.out.log` y
`target/agent-out/guarded-20261014-233853-739.err.log`.

| Artefacto | Tamaño | SHA-256 |
|---|---:|---|
| `Naygo-0.5.2-portable.zip` | 14.137.651 bytes | `ca4f6f397a4b788175fb5b41e31af4a89b27500cf08f48a531d7584106be9f7a` |
| `Naygo-0.5.2-setup.exe` | 65.167.664 bytes | `cf012be267fe2e63032fee6cee8df89fbabf245f7914a541548f153681e3370d` |

Los hashes coinciden con `dist/SHA256SUMS.txt`; el EXE del ZIP coincide con `target/release/naygo.exe`
(SHA-256 `dc4a9faffc305bb9d18cc83462d5036ad6c240a7741622f062428bfee07acd5f`). El ZIP contiene
`LEEME.txt`, `LICENSE`, `naygo.exe` y `THIRD-PARTY-NOTICES.md`, sin PDB. EXE y setup siguen
`NotSigned`. No se instaló ni ejecutó la app. W-12 requiere el run remoto Windows Server 2025.

## Gate de cobertura del fallback de Papelera — 2026-10-15

El test Windows `trash::tests::recibo_reconstruido_reporta_si_falta_el_enumerador_de_papelera`
fuerza la ausencia del proveedor virtual y verifica que el fallback se intenta exactamente una vez,
que no se provoca panic y que el error mantiene tanto el estado de Papelera como el detalle del
proveedor. Pasó también el round-trip directo que exige cero enumeraciones. Gate supervisado:
suite workspace (904 core, 22 ops, 56 platform/6 ignorados, 388 UI, integración/render) y Clippy
global `-D warnings`; pico Job 5,89 GiB. Formato, diff y Graphify pasaron. Log:
`target/agent-out/guarded-20261015-001236-148.out.log` (detalles del supervisor en el `.err.log`).
Fue un cambio de pruebas: no alteró el ejecutable; la distribución final 0.5.2 registrada arriba
sigue siendo la correspondiente. Esta prueba no sustituye el workflow remoto Windows Server 2025.
