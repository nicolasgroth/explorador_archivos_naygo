# Validación cruzada del binding Linux GLib — 2026-10-17

## Avance

Se añadió un backend FFI local mínimo para Ayatana AppIndicator GLib, con carga dinámica mediante
`libloading` y ejecución del loop/eventos en un worker. La dependencia `tray-icon` queda limitada a
Windows. La integración Linux sigue en desarrollo; este registro no declara una alfa Linux aceptada.

## Verificación

- `cargo check --target x86_64-unknown-linux-gnu --tests --offline` en el harness aislado del módulo:
  aprobado.
- `cargo clippy --target x86_64-unknown-linux-gnu --tests --offline -- -D warnings` en el mismo
  harness: aprobado.
- El cleanup defensivo del `user_data` asignado a callbacks GLib fue verificado en el harness:
  ante un handler ID cero, se recupera el `Box` que todavía no tomó ownership GObject.
- `scripts/test-all.ps1` en Windows: 1.447 pruebas aprobadas, 8 ignoradas; Clippy `-D warnings` y
  formato aprobados. La corrida descubrió y permitió corregir la llamada inicial de la bandeja y el
  `dead_code` de `TrayMsg::Availability` al compilar Windows.
- `graphify update .`: actualizado a 9.192 nodos, 17.432 aristas y 856 comunidades.
- `scripts/build-release-guarded.ps1`: aprobado; pico contable Job 10,50 GiB, límite 12 GiB y
  margen mínimo 8 GiB. Release: 28 min 56 s. Inno Setup: 114,156 s.
- ZIP `Naygo-0.5.2-portable.zip`: 14.364.914 bytes, SHA-256
  `95040c016e89bd07b4f78dc51e1cd211c9445a1e5f017bcea070e0ff52b93833`.
- Setup `Naygo-0.5.2-setup.exe`: 66.426.270 bytes, SHA-256
  `ee7c1e0da5f1d6f6d0e2f45edb3d4d54300e602a902f5771f753659c00ceb942`.
- EXE release y `naygo.exe` dentro del ZIP concuerdan: SHA-256
  `d9ecae0e64a34aa63a764108ae7dfc3390faa4e5f7ad0a40940940594cedaf87`. Ambos artefactos
  concuerdan con `dist/SHA256SUMS.txt` y el ZIP incluye `LICENSE` y `THIRD-PARTY-NOTICES.md`.
- No se instaló, publicó ni firmó. La instancia instalada anterior quedó abierta y respondiendo
  como PID 56616, EXE SHA-256 `ed1f349116181dc60d3ccc4fa5dbb6ea6969e4c4816145cfe75d6c55ac278a9e`.
  El paquete nuevo continúa sin firma Authenticode y no se instaló.

## Límites y siguiente puerta

El harness solo prueba compilación/typecheck de Rust, no las declaraciones ABI, resolución de
sonames, carga en runtime, conexión D-Bus, icono, menú ni acciones. El cross-check de UI completa
Linux (`cargo check -p naygo-ui-slint --target x86_64-unknown-linux-gnu --tests --locked --jobs 1`)
se detiene en `yeslogic-fontconfig-sys`: el host no tiene `pkg-config` y sysroot Linux configurados.
WSL en este host informa
`HYPERV_NOT_INSTALLED`; no se modificó la configuración de Windows. Antes de cerrar L-03 se necesita
build y ejecución nativos en Ubuntu 26.04 GNOME/Wayland y al menos un derivado, con pruebas de
degradación segura.

## Revalidación de toolchain cruzado, 2026-10-18

El target `x86_64-unknown-linux-gnu` está instalado en rustup. Desde Windows pasaron:

- `cargo check -p naygo-core -p naygo-platform --all-targets --target x86_64-unknown-linux-gnu --locked --jobs 1`.
- `cargo clippy -p naygo-core -p naygo-platform --all-targets --target x86_64-unknown-linux-gnu --locked --jobs 1 -- -D warnings`.

La comprobación incluyó todos los targets disponibles de `core` y `platform`, con código de salida
0. No enlaza ni ejecuta los tests; tampoco compila `naygo-ui-slint`, el FFI GLib o las dependencias
nativas GTK/fontconfig. En el mismo host `wsl -d Ubuntu` vuelve a fallar con
`HYPERV_NOT_INSTALLED`; por tanto no hubo validación ABI/runtime, de D-Bus ni de escritorio. El
cross-check mejora la evidencia de compatibilidad de las capas compartidas, pero L-03 no se cierra.

## Check cruzado de la UI Linux completa, 2026-10-18

Slint 1.16 incluye `fontique` 0.8; su backend Fontconfig enlazaba el build script `yeslogic-fontconfig-sys`
con `pkg-config` y bloqueaba el check cruzado aun cuando el tray ya no dependía de GTK. Se habilitó
`fontique/fontconfig-dlopen` como dependencia exclusiva de `target_os = "linux"` en
`crates/ui-slint/Cargo.toml`. Esto conserva la carga dinámica de `libfontconfig.so.1` y el fallback
de Fontique cuando la biblioteca no está disponible, sin requerir headers ni `pkg-config` durante
el cross-check. Windows no recibe esta feature.

Desde Windows pasaron con `--locked --offline --jobs 1`:

- `cargo check -p naygo-ui-slint --all-targets --target x86_64-unknown-linux-gnu`.
- `cargo clippy -p naygo-ui-slint --target x86_64-unknown-linux-gnu -- -D warnings`.

El check `--all-targets` incluyó el binario y sus tests. Clippy no reportó avisos Rust. El build
script mostró el aviso esperado de que `windres` no está instalado para incrustar el ícono PE; esa
herramienta solo aplica a builds Windows. La compilación cruzada no enlaza ni ejecuta el programa,
ni valida la resolución runtime de Fontconfig, GLib/AppIndicator, D-Bus o Winit.

El test Windows del workspace pasó en una repetición focal del 2026-10-18: 1.452 aprobados,
8 ignorados. También pasaron por separado `cargo clippy --workspace --all-targets --locked -- -D warnings`
y `cargo fmt --all -- --check`. Clippy usa `clippy-driver.exe` en Windows; su análisis de la UI tomó
2m16s y terminó con código 0. Falta regenerar portable/setup bajo supervisor por los cambios de
manifest y `cfg`.

## Revisión estática del contrato AppIndicator, 2026-10-18

Se compararon las declaraciones FFI de `crates/ui-slint/src/tray/linux.rs` con el header upstream
incluido en `target/agent-out/ayatana-appindicator-glib-upstream/src/ayatana-appindicator.h`.
Coinciden los argumentos de `app_indicator_new_with_path`, `set_status`, `set_menu`, `set_actions`,
`set_title`, `set_tooltip` y `set_secondary_activate_target`; los tipos C de categoría/estado son
enum basados en `int`, y `set_actions` recibe `GSimpleActionGroup *`. El ejemplo upstream
`examples/simple-client.c` confirma el namespace `indicator.<nombre>` que el menú Rust usa para
enlazar las acciones.

Esto reduce el riesgo de una declaración AppIndicator incorrecta, pero es solo una inspección
estática del header y del ejemplo de esa revisión upstream. No valida los prototipos GLib/Gio
restantes contra los headers de la versión binaria Ubuntu, el ABI en ejecución, la carga de sonames,
el registro D-Bus ni la compatibilidad del menú en GNOME/Wayland y derivados. L-03 permanece abierto.

## Corrección del ciclo de vida del source GLib, 2026-10-18

La revisión encontró que `socket_ready` devolvía `FALSE` al procesar HUP/EOF/error, lo que ordena a
GLib destruir el source y ejecutar su `GDestroyNotify`; al regresar `g_main_loop_run`, el worker
intentaba llamar `g_source_remove(source_id)` sobre ese ID ya destruido. GLib documenta que `FALSE`
retira el source automáticamente y que eliminar un ID inexistente es un error de programación
([callback](https://docs.gtk.org/glib/callback.SourceFunc.html),
[`g_source_remove`](https://docs.gtk.org/glib/type_func.Source.remove.html)).

El callback ahora devuelve `TRUE` después de pedir el cierre del loop, manteniendo válido el source
hasta que el worker lo quite una sola vez tras salir de `g_main_loop_run`. Los retornos `FALSE` se
conservan únicamente cuando no existe estado para solicitar el cierre (puntero de callback nulo),
caso defensivo fuera del flujo normal de ownership.

**Validación local, 2026-10-18:** el harness aislado pasó `cargo check --target x86_64-unknown-linux-gnu
--tests --offline` y Clippy `-D warnings`. También pasaron la UI Linux completa (check de todos los
targets y Clippy `-D warnings`), el test Windows del workspace (1.452 aprobados, 8 ignorados),
Clippy Windows de todos los targets y formato. Son checks cruzados o lógicos; la máquina no ofrece
runtime Linux/GLib ni GNOME/Wayland, por lo que no prueban ABI, D-Bus o escritorio. La aceptación
nativa de L-03 sigue abierta. La distribución Windows correspondiente se regeneró bajo supervisor;
ver sus hashes en la sección de empaquetado.

## Distribución Windows posterior al fix, 2026-10-18

`scripts/build-release-guarded.ps1` regeneró portable/setup tras los cambios Linux y `cfg`. El Job
alcanzó 10,53 GiB (límite 12 GiB; mínimo libre 8 GiB); release tardó 27m22s e Inno Setup 115,360 s.
El setup terminó correctamente, sin firmar ni instalar. Los checks locales de W-14 también pasaron.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/Naygo-0.5.2-portable.zip` | 14.390.524 | `eaf8ae2b14c09ac8de412a05571de6bcabd61d72dd20a4fc1627cb583118bc39` |
| `dist/Naygo-0.5.2-setup.exe` | 66.587.568 | `1814dce3ad6255aaafce5a5f24e82568b20e2e40b57fa959b927d15209f0f64a` |
| `target/release/naygo.exe` | 43.522.048 | `2fe75bc81a1f3e9482735d817437030d29f4a554d674b5081d053137f5efd44a` |

Los dos hashes de distribución coinciden con `dist/SHA256SUMS.txt`. El ZIP se abrió y descomprimió
completamente con `System.IO.Compression.ZipArchive`: contiene `LEEME.txt`, `LICENSE`, `naygo.exe`
y `THIRD-PARTY-NOTICES.md`; el hash del EXE interno coincide con `target/release/naygo.exe` y no
incluye PDB. `Get-AuthenticodeSignature` informa `NotSigned` para el EXE y el setup. No se instaló ni
publicó esta build. Los checks y límites constan en la sección anterior.

Revalidación W-14 sobre los artefactos regenerados: `scripts/test-signing.ps1` pasó los controles de
metadatos PE/Inno, nombre/versión inválidos, identidad SignPath y artefacto sin firma;
`scripts/check_action_pins.py` también pasó. Esto confirma preparación local, no firma los archivos
ni sustituye el proyecto/credenciales, la aprobación o la ejecución remota.

## Auditoría de firmas FFI contra headers Ubuntu, 2026-10-18

Se compararon todas las funciones FFI cargadas por `tray/linux.rs` con los headers AppIndicator GLib
2.0.1 y GLib/GObject/Gio 2.88. El header AppIndicator salió del tar upstream 2.0.1, cuya firma se
verificó en el inventario L-00; el `.deb` oficial `libayatana-appindicator-glib-dev` 2.0.1-1build1
también se descargó y su SHA-256 `7a103011ca0ceacf91d3a4c23dbf59fa7c93aec6183a8b51b2f2407750157c13`
coincide con el índice Ubuntu autenticado en `linux-glib-l00`. Los headers GLib/GObject/Gio son los
del tag upstream GLib 2.88.0, la versión base del runtime candidato Ubuntu 26.04.

El cruce verificó el orden y tipo de argumentos de `g_unix_fd_add_full`, `g_main_loop_*`,
`g_source_remove`, `g_signal_connect_data`, `g_object_unref`, menús y acciones Gio, además de las
siete funciones AppIndicator ya inventariadas arriba. Encontró tipos Rust declarados como `c_uint`
para los enums C `GIOCondition` y `GConnectFlags`; aunque ambos tienen el mismo ancho que `int` en
el ABI Linux y los valores actuales eran no negativos, se corrigieron a `c_int` para corresponder al
tipo C exacto. Se renombró el callback al tipo específico `GUnixFDSourceFunc` y se ajustó su
condición al mismo enum C. `guint` permanece correctamente como `c_uint` para IDs de source; `gulong`
se mantiene como `c_ulong` para IDs de señales.

Validación posterior al cambio: `cargo check -p naygo-ui-slint --all-targets --target
x86_64-unknown-linux-gnu --locked --offline --jobs 1`, Clippy Linux `-D warnings`, y
`scripts/test-all.ps1` en Windows pasaron. El gate Windows reportó 1.464 pruebas aprobadas,
8 ignoradas, Clippy sin avisos y formato correcto. El grafo se actualizó a 9.357 nodos, 17.710
aristas y 884 comunidades. La compilación cruzada de todos los targets Linux y Clippy `-D warnings`
comprueban que el callback Rust coincide con el tipo de función declarado en la FFI. Esto no
reemplaza la prueba ABI en runtime:
la carga de sonames, D-Bus, menú, acciones y escritorio todavía requiere Ubuntu 26.04 nativo. Los
headers descargados y el `.deb` de inspección están bajo `target/agent-out/linux-glib-l00/` y no se
incluyen en la distribución.

## Distribución Windows tras el ajuste FFI, 2026-10-18

`scripts/build-release-guarded.ps1` terminó correctamente con el Job limitado a 12 GiB; el pico fue
10,64 GiB y se mantuvo el margen mínimo de memoria. Release tardó 29m29s e Inno Setup 199,687 s.
Los artefactos quedaron en `dist/`; no se instaló, firmó ni publicó la build.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/Naygo-0.5.2-portable.zip` | 14.499.461 | `9b81852df27c7383bc413cbb6e212bfe41b7742e5c141a207a5f9bfd9695779e` |
| `dist/Naygo-0.5.2-setup.exe` | 67.024.969 | `fccfd7a82fe80a1e200a2989dcc042c32abde9d96d6e00f69abf869346570ee4` |
| `target/release/naygo.exe` | 43.854.336 | `210901465840187a497dca452d56b0cbf843807c11c52079e0bbeca0ac1d4673` |

Los hashes ZIP/setup coinciden con `dist/SHA256SUMS.txt`. El ZIP se leyó íntegramente sin errores;
contiene cuatro entradas, excluye el PDB y su `naygo.exe` coincide byte a byte con el ejecutable de
release. Authenticode informa `NotSigned` para setup y EXE, según el estado vigente de W-14.
