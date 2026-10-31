# Validación de distribución — 2026-10-15

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

Distribuciones locales de Naygo 0.5.2: primero tras corregir los resultados parciales de Papelera y
el tratamiento de junctions/symlinks de Windows, luego tras el fix W-10B de sincronización
incremental de filas Slint, y finalmente tras corregir validación de nombres POSIX en el core. No
certifica instalación externa, runner GitHub Windows Server 2025 ni firma de código.

## CLI POSIX `args_os()` — 2026-10-15

El arranque ya no usa `std::env::args()`: conserva rutas de argumentos como `OsString`/`PathBuf` y
escapa bytes inválidos sólo al preparar avisos. Flags y valores de tema/plantilla siguen exigiendo
UTF-8. El gate completo pasó con 1.393 tests aprobados, 7 ignorados, Clippy `-D warnings` y formato;
tras el último ajuste de visualización pasaron además los 19 tests CLI Windows, `cargo check` de UI
y Clippy cruzado Linux de core con tests. Los tests que construyen rutas POSIX no UTF-8 compilan
cruzados, pero su ejecución sigue pendiente en Linux nativo.

Build aislado: `scripts/build-release-guarded.ps1`, Cargo release 29m24s, Inno Setup 117,766 s;
pico contable 10,42 GiB frente al límite de 12 GiB. El ZIP se leyó por completo con `ZipArchive`,
contiene LICENSE/notices, excluye PDB y el EXE coincide byte a byte con `target/release/naygo.exe`.

- Portable: `dist/Naygo-0.5.2-portable.zip`, 14.150.538 bytes; SHA-256
  `f01e33c9e6b6f8a40e26376ff2f78dc9a917b98fd803035c1042e643409c7777`.
- Instalador: `dist/Naygo-0.5.2-setup.exe`, 65.185.039 bytes; SHA-256
  `226d53b9abaec92fb247c6c263e93d31f4c0f8a59e8618e69370fefbdaf476a9`.
- EXE del portable: 42.762.240 bytes; SHA-256
  `729e71fc00546136d2063b82a41fe1f73772ef584c1478302f381c37e7d5bc8c`.
- Authenticode setup/EXE: `NotSigned`, pendiente de W-14/SignPath.

Logs: `target/agent-out/guarded-20261015-183027-049.out.log` y
`target/agent-out/guarded-20261015-183027-049.err.log`.

## Corrección de nombres ZIP POSIX — 2026-10-15

`archive_ops::collect_entries` ahora conserva backslash como carácter literal de un nombre POSIX y
rechaza miembros no UTF-8 antes de crear/truncar el ZIP. Pasaron los 1.393 tests del workspace, con
7 ignorados, Clippy `-D warnings`, formato, `cargo check --target x86_64-unknown-linux-gnu
-p naygo-core --tests --locked` y Clippy cruzado Linux de core. Los dos tests Unix nuevos compilan
para Linux, pero requieren ejecución en Linux nativo, que no está disponible en este host.

Build aislado supervisado mediante `scripts/build-release-guarded.ps1`; gate, release y empaquetado
terminaron con código 0. Cargo release: 34m01s; Inno Setup: 133,765 s; pico contable del Job:
10,42 GiB (límite 12 GiB, margen de memoria del host sobre 8 GiB). El portable contiene LICENSE y
`THIRD-PARTY-NOTICES.md`, no contiene PDB; el `naygo.exe` archivado coincide byte a byte con
`target/release/naygo.exe`.

- Portable: `dist/Naygo-0.5.2-portable.zip`, 14.147.994 bytes; SHA-256
  `a574dd3000aec7d38c5db876927f2d045e9870750f50fe495e12c4bd620cbdf5`.
- Instalador: `dist/Naygo-0.5.2-setup.exe`, 65.126.112 bytes; SHA-256
  `6618f43953b917b045df6ef7042fe5f1ba5767730e59cf120732f105e11472fe`.
- EXE del portable: 42.755.072 bytes; SHA-256
  `55becce9dad50a8ae95804abffda8fb2f1908da35150cf662aa49fcd81d1cc9f`.
- Authenticode del setup: `NotSigned`, esperado mientras W-14 espera configuración y aprobación
  externa de SignPath.

Logs: `target/agent-out/guarded-20261015-155542-502.out.log` y
`target/agent-out/guarded-20261015-155542-502.err.log`.

## Captura segura de rutas en plantillas — 2026-10-15

`TemplateDir::Fixed` conserva `PathBuf`; `Workspace::to_template` rechaza antes de modificar el
catálogo las rutas fijas que no sean UTF-8. La serialización de rutas UTF-8 conserva el JSON string
vigente y la UI localiza el error para los diez idiomas. Pasaron la prueba focal UI de guardar,
aplicar y borrar plantilla (1/1), 110 tests workspace, paridad i18n y check/Clippy cruzados Linux de
core. `cargo test --no-run` con target Linux no pudo enlazar porque el host Windows no tiene `cc`;
el test Unix compila en el check cruzado y sigue pendiente correr en Linux nativo.

`scripts/test-all.ps1`: 1.393 tests aprobados, 7 ignorados; Clippy `-D warnings` y formato pasaron.
La distribución se regeneró con `scripts/build-release-guarded.ps1`; Cargo release 32m10s, Inno Setup
127,875 s, pico contable 10,41 GiB (límite 12 GiB; margen requerido 8 GiB). El ZIP incluye LICENSE y
`THIRD-PARTY-NOTICES.md`, excluye PDB y su `naygo.exe` coincide byte a byte con `target/release`.

- Portable: `dist/Naygo-0.5.2-portable.zip`, 14.149.672 bytes; SHA-256
  `1d765276fc870d86f4d4b558fb4a59de605b176ed0eb5ddbc5f4ff24777988f7`.
- Instalador: `dist/Naygo-0.5.2-setup.exe`, 65.259.568 bytes; SHA-256
  `f9b502542a6ae73ff49291e0ea2753d78b976565497bdacd4c2142493f7fd910`.
- EXE portable: 42.760.192 bytes; SHA-256
  `9c0da2d764aa0936c4bce9463775b0ad712219aa0495b7cb51e991eb07421249`.
- Authenticode: `NotSigned`, mientras W-14 espera aprobación y configuración SignPath externas.

Logs: `target/agent-out/guarded-20261015-170201-567.out.log` y
`target/agent-out/guarded-20261015-170201-567.err.log`.

## Build supervisado — deduplicación POSIX, 2026-10-15

Comando: `scripts/build-release-guarded.ps1`.

- Gate previo: `scripts/test-all.ps1`; 1.393 tests aprobados, 7 ignorados, Clippy `-D warnings` y
  formato aprobados.
- Validación local de `.github/workflows/ci.yml` y `release.yml` con parser YAML; los nueve
  catálogos de idioma pasaron `scripts/check_i18n_lang.py`. Los XML de SignPath están bien formados
  y especifican `ProductName=Naygo`, versión parametrizada y pasos de firma/verificación; la firma
  real sigue pendiente de aprobación/configuración externa.
- El cambio conserva bytes de nombres no UTF-8 al planificar duplicados y resuelve colisiones sin
  reconstruir el componente de ruta como texto. `cargo check --target x86_64-unknown-linux-gnu
  -p naygo-core --tests --locked` y Clippy cruzado Linux de core pasan; el test que crea nombres
  Unix no se ejecutó nativamente porque WSL sigue sin iniciar.
- Cargo release: 32m20s; Inno Setup: 130,219 s.
- Pico contable del Job: 10,42 GiB (límite 12 GiB; margen mínimo requerido del host 8 GiB).
- Portable: `dist/Naygo-0.5.2-portable.zip` (14.148.671 bytes).
- Instalador: `dist/Naygo-0.5.2-setup.exe` (65.117.174 bytes).
- `naygo.exe` del ZIP coincide byte a byte con `target/release/naygo.exe` (42.758.144 bytes;
  SHA-256 `eb51414fe710798866b69d3ec78d4a7202d1f4228bb7176b2ab73b8bbc51c7b5`). El ZIP incluye
  `LICENSE` y `THIRD-PARTY-NOTICES.md`, y no incluye PDB.
- EXE y setup reportan Authenticode `NotSigned`, esperado mientras W-14 no tenga proyecto y
  certificado SignPath aprobados.

SHA-256 (`dist/SHA256SUMS.txt`):

```text
3991b7fe84d1fe9ebe3519e75d71bd963b42d13a8b9b85ee5f1cd8b8bfea93c6  Naygo-0.5.2-portable.zip
dceb311b92fc9ec02f0b627e2f6e94080bc1a57d4b4d5d4e0822e7ede32ae6f6  Naygo-0.5.2-setup.exe
```

Logs del supervisor:

- `target/agent-out/guarded-20261015-144714-837.out.log`
- `target/agent-out/guarded-20261015-144714-837.err.log`

## Verificación

- `cargo test -p naygo-platform --locked trash::tests -- --nocapture`: 5 aprobados; incluye ida y
  vuelta de un junction que conserva su carpeta destino y archivo centinela. También prueba que los
  resultados parciales mantienen recibos y atribuyen errores por posición de origen.
- `cargo check -p naygo-platform --tests --locked --target x86_64-pc-windows-msvc`: aprobado.
- `cargo check -p naygo-ui-slint --tests --locked --target x86_64-pc-windows-msvc`: aprobado.
- `cargo test -p naygo-ui-slint --locked ops_ctrl:: --no-run`: todos los binarios de test UI compilaron.
- `cargo test -p naygo-ui-slint --locked ops_ctrl::batch_delete::tests::resultados_parciales_mapean_por_indice_si_la_ruta_es_relativa -- --exact`: 1 aprobado.
- `scripts/test-all.ps1` tras W-10B: 1.393 aprobados, 7 ignorados; Clippy con `-D warnings` y
  formato aprobados.
- `graphify update .`: grafo actualizado. La extracción AST informa 29 catálogos JSON sin nodos,
  igual que el comportamiento documentado de graphify.
- Benchmark headless `sync_rows`, tres ejecuciones `diag`: 100k, mediana de 31,874 ms al modificar
  una fila frente a la muestra anterior de 353,568 ms; carga y append requieren comparar contra
  baseline controlado. Detalle y límites en `BENCHMARKS.md`.
- Prueba focalizada `workspace_ctrl::tests::watch_events_usa_la_metadata_preresuelta`: aprobada
  después del gate/build; además de la metadata pre-resuelta, verifica que el evento `Modified`
  estable se exponga como delta de ruta para el sync selectivo.

## Build supervisado — nombres POSIX, 2026-10-15

Comando: `scripts/build-release-guarded.ps1`.

- Gate previo: `scripts/test-all.ps1`; 1.393 tests aprobados, 7 ignorados, Clippy `-D warnings` y
  formato aprobados.
- Cambio incluido: validación por reglas POSIX, backslash literal y deduplicación que conserva bytes
  nativos no UTF-8. Los 11 tests de nombres Windows pasaron; `cargo check --target
  x86_64-unknown-linux-gnu -p naygo-core --tests --locked` y Clippy cruzado de core para Linux
  también pasaron. Los tests Unix no se ejecutaron nativamente porque WSL sigue sin iniciar.
- Cargo release: 44m29s; Inno Setup: 124,688 s.
- Pico contable del Job: 10,41 GiB (límite 12 GiB; margen mínimo requerido del host 8 GiB).
- Portable: `dist/Naygo-0.5.2-portable.zip` (14.147.796 bytes).
- Instalador: `dist/Naygo-0.5.2-setup.exe` (65.234.508 bytes).
- `naygo.exe` del ZIP coincide byte a byte con `target/release/naygo.exe` (42.758.656 bytes;
  SHA-256 `3815fc67a5b3a005e6d29c4f5d85f9fc9bda71d3c67e8824fa6da2923d50bb13`). El ZIP incluye
  `THIRD-PARTY-NOTICES.md` y no incluye PDB.
- EXE y setup reportan Authenticode `NotSigned`, esperado mientras W-14 no tenga proyecto y
  certificado SignPath aprobados.

SHA-256 (`dist/SHA256SUMS.txt`):

```text
16eac1e1a222c6db3bd439e529753014aa597d41506ba17f77ef8c929909b4ee  Naygo-0.5.2-portable.zip
a99a80f0f19a06ebafd133d9ff24738c53b7534885f6d71804ddf949e7aad97e  Naygo-0.5.2-setup.exe
```

Logs del supervisor:

- `target/agent-out/guarded-20261015-133123-967.out.log`
- `target/agent-out/guarded-20261015-133123-967.err.log`

Las corridas W-10B y Papelera anteriores quedan abajo como evidencia histórica; sus hashes ya no
describen `dist/`.

## Build supervisado — W-10B con delta del watcher, 2026-10-15

Comando: `scripts/build-release-guarded.ps1`.

- Gate previo: `scripts/test-all.ps1` con 1.393 aprobados, 7 ignorados, Clippy sin warnings y formato.
- Cargo release: 31m36s; Inno Setup: 122,422 s.
- Pico contable del Job: 10,42 GiB (límite 12 GiB; margen mínimo de host 8 GiB).
- Portable: `dist/Naygo-0.5.2-portable.zip` (14.148.049 bytes).
- Instalador: `dist/Naygo-0.5.2-setup.exe` (65.168.000 bytes).
- `naygo.exe` de 42.757.120 bytes dentro del ZIP coincide byte a byte con
  `target/release/naygo.exe` (SHA-256 `0e2b67d06ab0bf84dceec1517c037c2500903a2151570047aa462a53d5d0a99f`).
  El ZIP incluye licencia/notices y no incluye PDB.
- EXE y setup reportan Authenticode `NotSigned`, esperado mientras W-14 no tenga proyecto y
  certificado SignPath aprobados.

SHA-256 (`dist/SHA256SUMS.txt`):

```text
d521d49d4b5268709031f72efc9a4e334e90608f5efb91d4d6477caa31b29295  Naygo-0.5.2-portable.zip
53cdc46a96f24e6b3d132467d2dbfb814834b51517759ad2c31dac58d8286db8  Naygo-0.5.2-setup.exe
```

Logs del supervisor:

- `target/agent-out/guarded-20261015-120744-568.out.log`
- `target/agent-out/guarded-20261015-120744-568.err.log`

La corrida W-10B previa queda abajo como evidencia histórica; sus hashes ya no describen `dist/`.

## Build supervisado — W-10B, 2026-10-15 (iteración anterior)

Comando: `scripts/build-release-guarded.ps1`.

- El supervisor inicia Windows PowerShell 5.1 con su ruta de módulos del sistema aislada de los
  módulos PowerShell 7 heredados del entorno Codex. Se detectó un conflicto de `ExtendedTypeData`
  al cargar `Microsoft.PowerShell.Security`; esta corrida completó las comprobaciones Authenticode e
  integridad local sin ese conflicto (los artefactos siguen reportando `NotSigned`).
- Cargo release: 31m33s; Inno Setup: 128,656 s.
- Pico contable del Job: 10,41 GiB (límite 12 GiB; margen mínimo de host 8 GiB).
- Portable: `dist/Naygo-0.5.2-portable.zip` (14.138.806 bytes).
- Instalador: `dist/Naygo-0.5.2-setup.exe` (64.686.205 bytes).
- `naygo.exe` dentro del ZIP coincide byte a byte con `target/release/naygo.exe` (SHA-256
  `41e6f8014a87488e6186c2c8494088f070df54faeaa5389a2da8574065a1ca96`). El ZIP incluye
  licencia y notices, y no incluye el PDB.
- Los dos artefactos reportan Authenticode `NotSigned`, esperado mientras W-14 no tenga proyecto/
  certificado aprobado.

SHA-256 (`dist/SHA256SUMS.txt`):

```text
8ae48644c18080c1cf8f12a16ee50f5f453d80ca27a1c497cfb4f66b9b9c5b8e  Naygo-0.5.2-portable.zip
83c37c4591ee3c2fc3e458794ec21f29691a33409614d9a8dfba1a477e7323aa  Naygo-0.5.2-setup.exe
```

Logs del supervisor:

- `target/agent-out/guarded-20261015-072104-283.out.log`
- `target/agent-out/guarded-20261015-072104-283.err.log`

La corrida anterior, de las correcciones de Papelera, queda abajo como evidencia histórica; sus
hashes ya no describen los archivos actuales de `dist/`.

## Build supervisado — Papelera, anterior

Comando: `powershell -ExecutionPolicy Bypass -File scripts/build-release-guarded.ps1`.

- Cargo release: 25m04s; Inno Setup: 106,688 s.
- Pico contable del Job: 10,42 GiB (límite 12 GiB; margen mínimo de host 8 GiB).
- Portable: `dist/Naygo-0.5.2-portable.zip` (14.140.165 bytes).
- Instalador: `dist/Naygo-0.5.2-setup.exe` (65.166.412 bytes).
- En esa corrida, `naygo.exe` del ZIP se verificó contra el EXE release de entonces (SHA-256
  `e4db1914c3329e152a2ff6e424b61e7cd4720dae416735791c70df010605cb4d`). El ZIP incluía licencia y
  notices, y no incluía el PDB.
- Firma Authenticode: EXE y setup reportan `NotSigned`; es lo esperado mientras W-14 no tenga
  certificado/proyecto SignPath aprobado.

SHA-256 de los artefactos (`dist/SHA256SUMS.txt`):

```text
556f38e1cb18aaa6bbbb70fb675e59efdc2e6d67371a95e90921f37a7d12355c  Naygo-0.5.2-portable.zip
1d482cbde6e933c8657551283e18d455df3ea0b19c9ac0ee308a0fa300429a6a  Naygo-0.5.2-setup.exe
```

Logs del supervisor:

- `target/agent-out/guarded-20261015-031533-309.out.log`
- `target/agent-out/guarded-20261015-031533-309.err.log`

## Pendiente

- Ejecutar el workflow actualizado en GitHub Actions sobre Windows Server 2025.
- Completar aceptación instalada Windows 10/11 y las mediciones interactivas W-07/W-13.
  La instancia activa que se observó durante tres muestras pasivas es un build instalado el
  14 de septiembre (hash `c78828504e8f5908a0fb47c31f542d7c52d65a84f98469c3352079809d876f8e`),
  distinto al EXE de esta entrega (`d3208423b3684d1b7e3ef373c4baa3ee057cbfdf3d35ad16718ce8e51f7af3fc`);
  sus métricas no deben atribuirse a este release.
- Configurar y ejecutar la firma Authenticode real cuando SignPath apruebe proyecto y credenciales.

## Rebuild Windows — junctions colgantes en Papelera, 2026-10-15

`restore_from_trash` y las comprobaciones posteriores al fallback usaban `Path::exists()`, que sigue
el destino de un enlace. Ahora inspeccionan la entrada con `symlink_metadata`, por lo que un junction
colgante continúa bloqueando el destino de restauración y no se confunde con un elemento reciclado.
La nueva prueba crea un junction, elimina su destino y comprueba que `path_entry_exists` sigue viendo
la entrada. También pasaron las seis pruebas `trash::tests`, incluidos el round-trip real de Papelera,
el lote parcial y el junction con su destino intacto.

El gate supervisado `scripts/build-release-guarded.ps1 -Verify` pasó: 1.394 tests aprobados, 7
ignorados, Clippy `-D warnings`; `cargo fmt --all -- --check` y `git diff --check` pasaron. El release
supervisado terminó con código 0; Cargo release tardó 40m25s, Inno Setup 174,313 s y el pico del Job
fue 10,42 GiB (límite 12 GiB, margen mínimo del host 8 GiB). La comprobación SHA-256 confirmó que
`SHA256SUMS.txt` coincide con los dos artefactos y que `naygo.exe` dentro del ZIP es byte por byte
igual al ejecutable release.

- Portable: `dist/Naygo-0.5.2-portable.zip`, 14.150.538 bytes; SHA-256
  `875ae438ea4583cde5c3e4fc136a6db22f5a4fc9f5df18f3dcc493805dbcaddd`.
- Instalador: `dist/Naygo-0.5.2-setup.exe`, 65.185.039 bytes; SHA-256
  `38b0eef186ce398016f5c9eb5b817e954462c06f067288eb29d71a7e2c9d2b31`.
- EXE portable/release: 42.763.776 bytes; SHA-256
  `d3208423b3684d1b7e3ef373c4baa3ee057cbfdf3d35ad16718ce8e51f7af3fc`.
- EXE y setup: Authenticode `NotSigned`, mientras W-14 espera configuración/credenciales SignPath.

Logs: `target/agent-out/guarded-20261015-200333-737.out.log` y
`target/agent-out/guarded-20261015-200333-737.err.log`.

El proceso instalado antiguo seguía activo tras el gate; no se cerró ni se instaló esta entrega. La
medición W-07 interactiva y la aceptación W-13 permanecen pendientes.

Después de generar el release se agregó una prueba de regresión adicional, sin cambios en código de
producto: `restore_rechaza_junction_colgante_como_destino_ocupado` crea un junction colgante, verifica
que el restore informe “destino ocupado” y que no intente enumerar la Papelera. Las siete pruebas
`trash::tests` y `cargo fmt --all -- --check` pasaron. El gate completo y los artefactos listados arriba
corresponden al código de producto ya empaquetado; el test posterior solo amplía la cobertura.

## Rebuild supervisado — Papelera fail-closed y pins W-12, 2026-10-15

La comprobación de ocupación ahora devuelve `Result<bool, io::Error>`: sólo `NotFound` significa
que la ruta no existe. `PermissionDenied` u otro error de `symlink_metadata` interrumpe el borrado,
restore o fallback en vez de convertir una ruta desconocida en ausencia, éxito o recibo reciclado.
Las 8 pruebas `trash::tests` pasaron, incluida la regresión portable para `PermissionDenied` y los
casos Windows de junction colgante. También pasó
`cargo check -p naygo-platform --tests --locked --target x86_64-unknown-linux-gnu`.

El gate supervisado `scripts/build-release-guarded.ps1 -Verify` pasó: 1.396 tests, 0 fallidos, 7
ignorados; Clippy con `-D warnings`, formato y `git diff --check` limpios. El pico del Job fue
8,39 GiB. Log: `target/agent-out/guarded-20261015-205432-427.out.log` (stderr:
`target/agent-out/guarded-20261015-205432-427.err.log`).

La distribución se regeneró después de este cambio con el supervisor, sin tareas pesadas paralelas.
Cargo release tardó 40m15s, Inno Setup 173,203 s y el pico del Job fue 10,41 GiB, bajo el límite de
12 GiB y con el margen mínimo de host de 8 GiB. `SHA256SUMS.txt` coincide con ambos artefactos y el
EXE del ZIP coincide byte por byte con `target/release/naygo.exe`.

- Portable: `dist/Naygo-0.5.2-portable.zip`, 14.151.457 bytes; SHA-256
  `41ad1a73be080a878d518fa7ac1cce619dc11b2fd0cb835a7cf7e0a4f55e99c8`.
- Instalador: `dist/Naygo-0.5.2-setup.exe`, 64.604.175 bytes; SHA-256
  `07ae92e348dafecd57cb604f68341f91a101c44993bfac3ed442690872fb35fc`.
- EXE portable/release: 42.767.360 bytes; SHA-256
  `9e8bb7d8aab4e5ff5d2b40bab72c7ac1c33a8a5fc6e59a32d3c35929333357ef`.
- EXE y setup: Authenticode `NotSigned`, pendiente de configuración y credenciales SignPath.

W-12 también fija a commits inmutables las cinco acciones externas usadas por CI/release. Las
referencias exactas de checkout, upload-artifact, rust-toolchain, rust-cache y SignPath devolvieron
el mismo SHA al consultarlas en la API de commits de GitHub; `python scripts/check_action_pins.py`
pasó. No se ejecutó ni publicó el workflow remoto.

Log del release: `target/agent-out/guarded-20261015-211208-766.out.log` (stderr:
`target/agent-out/guarded-20261015-211208-766.err.log`). No se cerró la instancia instalada ni se
instaló este artefacto. Aceptación manual W-07/W-13 y CI remota W-12 siguen pendientes.

## Rebuild W-07 — callbacks de entrada, 2026-10-15

Se añadieron spans opt-in de callback a teclado, clic/doble clic/clic medio en filas y ordenamiento
de columnas. El test focalizado de métricas pasó (6/6); el gate supervisado posterior pasó con
1.396 pruebas, 0 fallidas y 7 ignoradas, Clippy con `-D warnings`, formato y `git diff --check`.
El build release optimizado terminó en 42m37s y el instalador en 190,141 s. El pico contable del
Job fue 10,42 GiB, bajo el límite de 12 GiB, con margen de host superior a 8 GiB. Logs:
`target/agent-out/guarded-20261015-220831-671.out.log` y
`target/agent-out/guarded-20261015-221741-959.out.log` (sus archivos `.err.log` correspondientes).

Se autorizó cerrar la instancia antigua e iniciar el portable actual con `NAYGO_PERF_METRICS=1`.
El PID instalado 23856 cerró por `--shutdown`; el portable se ejecutó desde
`target/agent-out/w07-portable-verify-20261015` y también cerró limpiamente por `--shutdown`.
Se confirmó en el log que el opt-in se activa y vuelca métricas locales de listado, construcción y
actualización de filas, sincronización y tick. La automatización intentó teclado, clic en el panel y
Ctrl+P/Escape, pero no produjo muestras `ui.input.*_callback`; esa aceptación interactiva queda
pendiente y no se toma como línea base de W-07. El log temporal queda junto al ejecutable portable.

La captura pasiva corta del mismo build (3 ventanas de 5 s, intervalo 1 s) registró 55,7 MiB de
working set, 26,7 MiB privados, 343 handles, 20 threads y deltas CPU 0,031/0/0 s. Sólo se observó
el proceso raíz; la sesión era de arranque, por lo que estas cifras son smoke y no línea base.
Registro: `target/agent-out/w07-portable-20261015-230955.json`.

Como comprobaciones locales independientes, `scripts/test-signing.ps1` pasó los casos sintéticos de
identidad exacta y rechazo de artefactos/publicadores no válidos; `python scripts/check_action_pins.py`
confirmó los pins SHA configurados. No se envió el workflow ni se solicitó firma real.

El ZIP expandido contiene un `naygo.exe` idéntico byte por byte a `target/release/naygo.exe`.
`dist/SHA256SUMS.txt` coincide con:

- Portable: `dist/Naygo-0.5.2-portable.zip`, 14.151.941 bytes; SHA-256
  `8b2dc3d9e812f26da9e053c1aa145a2e583b31b00f606d875fee3e1250d377f3`.
- Instalador: `dist/Naygo-0.5.2-setup.exe`, 65.071.223 bytes; SHA-256
  `4c8f65ff1d5df2fd35a628ebe5e8e2c258032dc0f5f66385af6dabe555c57700`.
- `naygo.exe`: 42.767.872 bytes; SHA-256
  `a28106734d5b42ef85ba7d330e7f31be2ee9e787778cb0ed9c0d27337e98cd5b`.

EXE y setup siguen sin firma Authenticode. No se instaló ni publicó el artefacto. W-07 conserva
pendientes la interacción reproducible (incluidos los callbacks nuevos), renderer, CPU/RAM de
auxiliares y la matriz local/UNC.
