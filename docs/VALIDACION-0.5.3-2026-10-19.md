# Validación local de Naygo 0.5.3 — 2026-10-19

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Build y gates

- `scripts/test-all.ps1`: **1.464 tests aprobados, 8 ignorados**, Clippy con `-D warnings` y
  formato aprobados.
- `scripts/test-signing.ps1`: pasó con metadatos esperados `Naygo` / `0.5.3`; rechazó nombre,
  versión, publicador y artefacto sin firma incorrectos.
- `graphify update .`: completado. El extractor avisó que los JSON de idiomas no producen nodos
  AST; el grafo quedó actualizado a 9.365 nodos.
- `scripts/build-release-guarded.ps1`: build Release **31m51s**, Inno Setup **122,688 s** y pico
  contable del Job **10,63 GiB**, bajo el límite de 12 GiB. La compilación usó Rust 1.96.0, ThinLTO,
  16 unidades y un job Cargo.

## Artefactos

| Artefacto | SHA-256 | Resultado |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `a36e15e466ef937506bddd130164042726a871f9c927d6172261f29b2d5a169d` | ZIP íntegro, contiene `naygo.exe`, excluye PDB |
| `dist/Naygo-0.5.3-setup.exe` | `2f856465903cd5048c24cafb08f02057864d5e6778c3a679d98d47b73020a98b` | compilación Inno correcta; **sin Authenticode** |
| `target/release/naygo.exe` | `c428cae8eba6cf0fd6949f737917eb7479757cb79ebc79a8b917aabab3436cf2` | ProductName `Naygo`, ProductVersion `0.5.3` |

Los hashes de ZIP y setup coinciden con `dist/SHA256SUMS.txt`.

## Instalación y comprobación local

- El setup se ejecutó silenciosamente con código 0. El EXE instalado en
  `%LOCALAPPDATA%\Programs\Naygo\naygo.exe` tiene el mismo SHA-256 que Release y reporta ProductVersion
  `0.5.3`.
- Naygo quedó abierto y respondiendo como PID **48444**, desde la ruta instalada. La instancia de
  compilación se cerró después; queda una sola instancia.
- UI Automation de solo lectura: **768 descendientes**, 600 con nombre, 140 controles enfocables y
  los 140 nombrados; árbol completo. Esto no sustituye el recorrido manual con Narrador.
- Reinspección pasiva el 2026-10-19 sobre el mismo PID 48444: UIA volvió a dar 768 descendientes,
  600 con nombre y 140/140 enfocables nombrados. W-07 recogió tres ventanas pasivas de 30 s:
  76,9 MiB working set, 45,3 MiB privados, 385 handles, 26 hilos y CPU delta 0/0/0,063 s; un
  proceso observado y sin SMB. No se controló inactividad ni se midió input, latencia o presentación.
  JSON: `target/agent-out/w07-0.5.3-installed-passive-20261019.json`; detalle en
  [`BENCHMARKS.md`](BENCHMARKS.md).
- `settings.json`, `workspace.json`, `favorites.json` y `templates.json` parsean como JSON.
- El `naygo.pdb` previo, de **289.722.368 bytes**, se conservó porque no se seleccionaron símbolos.

## Reconstrucción local posterior — corrección del doble clic W-13

El 2026-10-19 se reforzó el camino del doble clic nativo: ahora recibe Ctrl/Shift de la fila al
soltar el mouse, no del estado de teclado potencialmente obsoleto. La regresión simula Ctrl y Shift
pegados tras un modal, y confirma que un doble clic normal navega dentro del panel actual.

- `scripts/test-all.ps1`: **1.465 tests aprobados, 8 ignorados**, Clippy sin warnings y formato
  aprobados. El test dirigido también pasó.
- `graphify update .`: grafo actualizado a 9.374 nodos; avisó que los JSON de idiomas no producen
  nodos AST y que las etiquetas semánticas necesitan regenerarse.
- `scripts/build-release-guarded.ps1`: Release **27m49s**, Inno Setup **117,125 s**, pico del Job
  **10,64 GiB** con límite de 12 GiB.
- Hashes actuales, que reemplazan los de la build inicial de esta versión:

| Artefacto | SHA-256 | Resultado |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `2104b4ece6f4f635d029935971ec2e6fac1b043b47829fad1abca80761c69ef6` | 4 entradas, EXE coincide con Release, sin PDB |
| `dist/Naygo-0.5.3-setup.exe` | `dbc952bae567e08c975509fcf1d751ef665b818b8b5fd7156fe06f549dcbe66e` | Inno compilado, sin Authenticode |
| `target/release/naygo.exe` | `006c4a3d5762c4b4de09db57e3e5eef458e067c1a37c30df3c0c22bc0799693a` | ProductName `Naygo`, ProductVersion `0.5.3` |

ZIP y setup coinciden con `dist/SHA256SUMS.txt`; el instalador no está firmado. La reconstrucción
no se instaló. El proceso PID **48444** sigue ejecutando el binario previo con SHA-256
`c428cae8eba6cf0fd6949f737917eb7479757cb79ebc79a8b917aabab3436cf2`, por lo que esta sección
valida el paquete y el código, no su aceptación visual en la app instalada.

## Pendientes de aceptación y distribución

- W-13 aún requiere instalar/probar esta reconstrucción con Narrador, teclado/foco, DPI y recorrido
  de diálogos/acciones.
- W-14 aún requiere aprobación y configuración de SignPath Foundation, credenciales y firma real de
  ambos artefactos (`naygo.exe` incluido en el ZIP y `setup.exe`). No se publicó ni creó etiqueta.
- W-07 sigue pendiente de interacción física, medición de presentación y matriz local/UNC.
- Los pendientes Windows adicionales —prueba remota W-12 en Server 2025 y reproducción manual del
  drop E-2 desde 7-Zip— requieren sus entornos/fixture de aceptación; la versión local queda lista
  para probar esos escenarios.

## Reconstrucción final — caché acotada de espacio Linux

Después de limitar la caché Linux del footer a 128 rutas y a ocho consultas simultáneas, el gate
completo pasó **1.467 tests aprobados y 8 ignorados**, Clippy Windows, formato y Clippy cruzado Linux
sin warnings. Release supervisado: **28m43s**, Inno Setup **119,297 s**, pico **10,66 GiB** con
límite de 12 GiB.

| Artefacto vigente | SHA-256 | Resultado |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `70f734e99d893f774f4c0c5ae132ac5d584a7ff746dd637fa1320704f23d9782` | 4 entradas, excluye PDB |
| `dist/Naygo-0.5.3-setup.exe` | `252a748db7892a7d9a83c62c5469827ebee45026b09a1118354160257a210c79` | Inno correcto, sin Authenticode |
| `target/release/naygo.exe` | `a9a3fb2c8be9cc17d5fa8724711f941a8ec4d14abec9eb7f9e927af19cfd3f80` | ProductVersion `0.5.3` |

El manifiesto coincide con los hashes del ZIP y setup; el EXE dentro del ZIP coincide byte por byte
con Release. Esta reconstrucción no se instaló; la instancia PID 48444 sigue usando el binario
anterior de 0.5.3. La ejecución Linux nativa continúa pendiente por WSL deshabilitado.

## Reconstrucción local — optimización del listado W-07

El 2026-10-19 se ajustó el worker para materializar `DirEntry::path()` una sola vez, usarla en el
filtro de temporales internos y moverla después a la construcción de la fila. La política de
filtrado y el `Entry.path` que requiere la UI no cambian. Esta sección registra una primera versión
intermedia del ajuste; la implementación final reutiliza la ruta en ambos pasos y se volverá a
empaquetar tras el gate completo.

- `scripts/test-all.ps1`: **1.467 pruebas aprobadas, 8 ignoradas**, Clippy sin warnings y formato
  correcto. La prueba dirigida de nombres reservados y la prueba de cancelación del canal también
  pasaron.
- `cargo check --workspace --tests --target x86_64-unknown-linux-gnu --locked --jobs 1` y Clippy
  cruzado con `-D warnings`: ambos pasaron. Sigue sin ejecutarse en Linux nativo porque WSL no inicia.
- `graphify update .`: grafo de código actualizado a 9.386 nodos.
- Build supervisado con límite de 12 GiB: Release **28m43s**, Inno Setup **116,735 s**, pico del
  Job **10,64 GiB**. No se instaló ni publicó.

| Artefacto vigente | SHA-256 | Resultado |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `8204d6140b96b1fe0786ee7730f9969e6e837ca4b3f11e46c4a726d4a5ca7b41` | 4 entradas, excluye PDB |
| `dist/Naygo-0.5.3-setup.exe` | `656abd14db678c704ce268bd19f099bc845c447336bc84132c9aa3f9916d7a68` | Inno compilado, `NotSigned` |
| `target/release/naygo.exe` | `d4757f0c1eb76c887a432f6a1547a4efda447d0c8428a17ccf1a7abc41b0b616` | ProductVersion `0.5.3` |

El ZIP y el setup coinciden con `dist/SHA256SUMS.txt`; el EXE del ZIP coincide byte por byte con
Release. La optimización elimina una ruta completa temporal por entrada ordinaria, pero la medición
local fue ruidosa y no demuestra una ganancia de latencia. El instalador no tiene Authenticode. La
instancia anterior PID 48444 sigue abierta; esta reconstrucción no se instaló.

## Reconstrucción vigente — ruta única por entrada W-07

La versión final del cambio usa una sola `PathBuf` al filtrar temporales internos y construir el
`Entry`, sin una segunda materialización ni clonar el nombre.

- `scripts/test-all.ps1`: **1.467 pruebas aprobadas, 8 ignoradas**, Clippy sin warnings y formato.
- Linux cruzado: `cargo check --workspace --tests --target x86_64-unknown-linux-gnu --locked --jobs 1`
  y Clippy para todos los targets pasaron. El warning del build-script de icono solo indica que no
  se embebe el recurso Windows al compilar el target Linux.
- `graphify update .`: grafo actualizado a **9.389 nodos**.
- Build supervisado con límite de 12 GiB: Release **28m51s**, Inno Setup **115,375 s**, pico del
  Job **10,64 GiB**. No se instaló ni publicó.

| Artefacto vigente | SHA-256 | Resultado |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `0835022c0c932139f0d0d2a20c55fdca4daa00faef7bdb689adbac7e94b6450e` | 4 entradas, excluye PDB |
| `dist/Naygo-0.5.3-setup.exe` | `ac66ca998c7c35e7e3280ed69631e631f3cf66dd6e854659e18d7e7e152356e9` | Inno compilado, `NotSigned` |
| `target/release/naygo.exe` | `8957fa2404c4f8fd8557a0e5255246c963142646dea7272c0195dc732b3d8d3a` | ProductVersion `0.5.3` |

`dist/SHA256SUMS.txt` coincide con portable y setup; el EXE del ZIP coincide byte por byte con
Release y no contiene PDB. La medición W-07 local sigue siendo inconclusa por su variación entre
pasadas; el ajuste evita una segunda materialización de ruta, sin atribuirle una ganancia de
latencia. La instancia PID 48444 permanece en el 0.5.3 anterior.

## Reconstrucción final — controles accesibles de preview, árbol y Ctrl+P (W-13)

Tras los cambios de accesibilidad descritos en `VALIDACION-W13-CONTROLES-2026-10-18.md`, el gate
completo pasó **1.468 tests aprobados, 8 ignorados**, Clippy sin warnings y formato. También pasaron
el check y Clippy cruzados para todos los targets Linux; la compilación cruzada solo avisa que no
puede incrustar el recurso `.exe` de Windows. `graphify update .` terminó con 9.393 nodos y 17.762
relaciones.

La reconstrucción supervisada alcanzó un pico de Job de **10,65 GiB** bajo el límite de 12 GiB.
Release tomó **27m57s** e Inno Setup **120,094 s**.

| Artefacto | SHA-256 | Verificación |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `81db72979ab3eff99523798f5ca80aef55f79e7eda1b2c89453ab41a91e8c9b9` | Manifiesto coincidente; ZIP íntegro, 4 entradas, sin PDB |
| `dist/Naygo-0.5.3-setup.exe` | `ab80b50e4f065f7e2ea78dffdca2a7bf944cf9de7ffaf1bb638f6baaf7c15860` | Manifiesto coincidente; Inno compiló, sin Authenticode |
| `target/release/naygo.exe` | `8b797c1fb6fd4028fa02727fdfae89f605126ae704735a7461260d08f23af199` | ProductVersion 0.5.3; coincide con el EXE del ZIP; sin Authenticode |

La build se instaló in-place el 2026-10-19 con autorización previa y quedó abierta como PID 44896.
El EXE instalado coincide con Release (`8b797c1fb6fd4028fa02727fdfae89f605126ae704735a7461260d08f23af199`)
y tiene ventana principal. Antes de instalar se respaldaron los cuatro JSON en
`target/agent-out/install-preserve-053-20261019-073712/`; los SHA-256 de settings, workspace,
favoritos, plantillas y PDB fueron idénticos antes y después. Los cuatro JSON parsean. UIA pasiva
encontró 375 descendientes, 302 nombrados, 113/113 controles enfocables nombrados, sin anónimos y
sin truncamiento. No se publicó ni firmó. Narrador y el recorrido manual siguen pendientes para
W-13; W-14 aún requiere configuración/credenciales y aprobación SignPath para firmar ambos artefactos.

## Reconstrucción e instalación W-06D

El 2026-10-19 se volvió a generar 0.5.3 con la enumeración de unidades/USB y la expulsión segura
fuera del hilo de UI. El gate completo pasó 1.476 tests (8 ignorados), Clippy y formato. El build
supervisado tomó 37m32s en Release y 142,110 s en Inno Setup; el Job alcanzó 10,65 GiB bajo el
límite de 12 GiB.

| Artefacto | SHA-256 | Resultado |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `7f7f70368099e85332547770d3ed975723d23da94548545adf3705b04e8d5fa8` | Manifiesto coincidente; contiene EXE, sin PDB |
| `dist/Naygo-0.5.3-setup.exe` | `fa19ff5bdc9cb672a6688a56ceb02d2621eff1a29b9bca8d16bc15de09500540` | Inno compilado; sin Authenticode |
| `target/release/naygo.exe` | `6015599ce33acd41b5763611cc4fbfd84a49ce6a81df0652cbc3394eef3a2c60` | ProductVersion 0.5.3; sin Authenticode |

El setup se instaló in-place con código 0 y Naygo se dejó abierto como PID 42728; la ventana
principal responde y el EXE instalado coincide con Release. UIA pasiva del proceso instalado
encontró 821 descendientes, 654 nombrados y 190/190 controles enfocables con nombre, sin truncar.
Se conservaron los hashes de los cuatro JSON de usuario y del PDB; los JSON parsean y hay copia
preventiva en `target/agent-out/install-preserve-053-20261019-w06d/`. Narrador, aceptación manual y
prueba de expulsión con USB real siguen pendientes.

## Reconstrucción posterior — L-06 y regresión de navegación, 2026-10-19

El worker de la barra ya consume la enumeración Linux L-06; la distribución Windows se regeneró
como exige el flujo, sin instalarla. También se extendió el test de controlador para crear una
carpeta, seleccionarla al terminar y navegar a ella con doble clic normal en el mismo panel.

- `scripts/test-all.ps1`: **1.477 tests aprobados, 8 ignorados**, Clippy sin warnings y formato.
- Linux cruzado: check y Clippy `-D warnings` para `naygo-platform` pasaron. Esto no valida la
  lectura real de `/proc` ni el runtime Ubuntu.
- Build supervisado: Release **34m58s**, Inno Setup **145,172 s**, pico del Job **10,65 GiB**,
  bajo el límite de 12 GiB.

| Artefacto | SHA-256 | Verificación |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `df10b324d2d4ce39ff279937d9d6e50b2048591a0eeb542fd2a441a93b6bb5f9` | Manifiesto; 4 entradas, excluye PDB |
| `dist/Naygo-0.5.3-setup.exe` | `7eca3b54f92e82a3684f54cefb42d112cfb57452ac14155c7591e7e28670e731` | Manifiesto; Inno compiló, sin Authenticode |
| `target/release/naygo.exe` | `cf73acd1917a48057bd971da7af1e9efb2d835d2acabeae653c32543197004ec` | ProductVersion 0.5.3; coincide con el EXE del ZIP |

Se verificó byte por byte que el EXE portable coincide con Release y que `dist/SHA256SUMS.txt`
coincide con ambos artefactos. El setup y el ejecutable siguen sin firma. No se instaló ni publicó
esta reconstrucción; la instancia PID 42728 sigue abierta con el EXE W-06D anterior.

## Reconstrucción L-07 — Papelera Linux, 2026-10-19

`scripts/build-release-guarded.ps1` completó Release en **42m14s** e Inno Setup en **159,329 s**;
pico del Job **10,65 GiB** bajo el límite de 12 GiB. La distribución 0.5.3 incluye la extensión
compatible del recibo de deshacer; el backend Linux permanece condicionado a aceptación nativa.

| Artefacto | SHA-256 | Verificación |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `4223bb3317c8988ca1fa180dd1ba722fee19043b23477e609f95485bd9199e32` | Manifiesto; cuatro entradas, excluye PDB |
| `dist/Naygo-0.5.3-setup.exe` | `b9be1868c7e0e2fd075cbc26deef6ee3ac244fffddb4ae2520a782decdec7ea8` | Manifiesto; Inno compiló |
| `target/release/naygo.exe` | `56876a94cef124c913366e0beb543d9768ad66acfd76b338a29ba2aa307ab8f1` | ProductVersion 0.5.3; igual al EXE del ZIP |

Setup y EXE siguen sin Authenticode. La distribución se regeneró pero no se instaló ni publicó;
Naygo PID 42728 permanece abierto con la build anterior. El PDB de diagnóstico se genera y se
conserva fuera del ZIP.

## Reconstrucción W-06E — temas personalizados, 2026-10-19

El guardado/borrado de temas continúa fuera de callbacks; el guardado atómico y las pruebas de worker
pasaron el gate completo (**1.480 tests, 8 ignorados**, Clippy y formato). Build supervisado: Release
**40m08s**, Inno Setup **195,625 s**, pico Job **10,65 GiB** bajo el tope de 12 GiB.

| Artefacto | SHA-256 | Verificación |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `06137391a1b317a815a6a68f422563831a7d8633df7593176bfc300ab2df6b92` | Manifiesto coincide; 4 entradas, sin PDB |
| `dist/Naygo-0.5.3-setup.exe` | `2303e71a3da3bec0422b7ddae685310ce4957e9e7a91b543e3dfa8db6e62cca6` | Manifiesto coincide; Inno compiló |
| `target/release/naygo.exe` | `dda9e59883c170b6d966cf6fccd6de2504a7df76f5596bad4707e20baf4d59fb` | ProductVersion 0.5.3; coincide con el EXE del ZIP |

Setup y EXE siguen sin Authenticode. Esta reconstrucción no se instaló ni publicó; PID 42728 sigue
abierto con W-06D. El PDB se conserva fuera del ZIP.
