# Validación de errores ReadDir y distribución Windows — 2026-10-18

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Corrección

La planificación de Copy y Move ya no descarta errores individuales del iterador `ReadDir`.
Si una entrada no puede leerse, la operación devuelve `SourceUnreadable` con la ruta de su carpeta
antes de ejecutar cambios. El borrado recursivo usa el mismo mapeo de error. Una prueba inyecta
`PermissionDenied` para demostrar que la enumeración incompleta detiene la planificación.

## Validación automática

- `cargo test -p naygo-core --locked ops::plan::tests`: 20/20 pruebas aprobadas.
- `scripts/test-all.ps1`: 1.453 pruebas aprobadas, 8 ignoradas; Clippy con `-D warnings` y formato
  correctos.
- `cargo check --target x86_64-unknown-linux-gnu -p naygo-core -p naygo-platform --tests --locked --jobs 1`:
  pasó desde Windows.
- Clippy cruzado de `naygo-core` y `naygo-platform` para `x86_64-unknown-linux-gnu`, con todos los
  targets y `-D warnings`: pasó.
- `git diff --check`: pasó.

Los checks cruzados solo validan `core` y `platform`; no son una compilación ni aceptación Linux
nativa.

## Distribución local

Build supervisado con `scripts/build-release-guarded.ps1`, Rust 1.96.0, `--locked`, un trabajo Cargo,
ThinLTO y 16 unidades de código. `cargo build --release` terminó correctamente en 27 min 40 s;
Inno Setup 6.7.3 terminó en 121,188 s. El pico de memoria del proceso Rust fue 10,70 GiB, bajo el
límite supervisado de 12 GiB. La firma local quedó desactivada al generar los paquetes.

| Artefacto | Tamaño | SHA-256 | Resultado |
| --- | ---: | --- | --- |
| `Naygo-0.5.2-portable.zip` | 14.390.269 bytes | `12373404a43f73125c2d26ac9b58171ca6afdb671f3046d8c995fb5d4714bdc7` | ZIP leído y descomprimido; 4 entradas; coincide con `SHA256SUMS.txt` |
| `Naygo-0.5.2-setup.exe` | 66.586.814 bytes | `afd8c672338b01a7b0fcdde452ba63171db5861466f31bac9e6ce2b222ae7f92` | Inno Setup exitoso; coincide con `SHA256SUMS.txt`; Authenticode `NotSigned` |
| `naygo.exe` release | 43.521.536 bytes | `d2c656e35487354b5e5eeb91d6a1f9b77b87246efc459952ba82c4970d799b50` | Coincide con el ejecutable del portable |
| `naygo.pdb` release | 454.225.920 bytes | — | Incluido en el instalador; excluido del ZIP portable |

No se instaló ni inició esta build. La instancia abierta conserva la distribución instalada anterior.
La aceptación visual, las rutas ausentes/remotas, Narrador y las pruebas manuales de W-13 siguen
pendientes; esta validación no cierra W-07 ni W-13.
