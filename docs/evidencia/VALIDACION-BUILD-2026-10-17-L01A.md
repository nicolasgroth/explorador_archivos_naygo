# Validación L-01A y distribución Windows — 2026-10-17

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambio

`delivery::suggested_group_names` usa `path_display::display_os_str` para representar el nombre
de origen. En POSIX, bytes no UTF-8 se muestran como `\xNN` y backslashes literales se escapan por
separado; el `PathBuf` operativo permanece intacto. Una prueba Unix diferencia ambos nombres.

La guía de entregas explica que la etiqueta propuesta es una representación visible, no una ruta
que Naygo vuelva a interpretar. El subpaso queda implementado; la matriz nativa Linux de L-01A sigue
pendiente.

## Verificación

- `cargo fmt --all -- --check`: aprobado.
- `cargo test -p naygo-core delivery::tests --locked --offline`: 10 aprobados en Windows.
- `cargo check -p naygo-core --tests --target x86_64-unknown-linux-gnu --locked --offline`: aprobado;
  incluye compilación del test Unix.
- `cargo clippy -p naygo-core --all-targets --target x86_64-unknown-linux-gnu --locked --offline -- -D warnings`:
  aprobado.
- `scripts/build-release-guarded.ps1 -Verify`: 1.440 pruebas aprobadas, 8 ignoradas; Clippy del
  workspace sin avisos. Pico del Job: 5,99 GiB.
- `scripts/build-release-guarded.ps1`: portable ZIP y setup generados correctamente. Release tardó
  29 minutos, pico del Job 10,48 GiB (límite 12 GiB). Setup exitoso en 120,453 s.
- `dist/SHA256SUMS.txt` coincide con los hashes verificados abajo. Setup sin firma Authenticode.

## Artefactos

| Artefacto | SHA-256 |
|---|---|
| EXE release / portable | `324b7ff873247930079ae65169e30ed51e9cebe4c7599c0a334a34e0add96f4b` |
| `Naygo-0.5.2-portable.zip` | `a19f88b0e687886de10871059ba5876008fc5e9b6d11a734ef70c42196dde2a4` |
| `Naygo-0.5.2-setup.exe` | `5b563482a8126311d40db1ec21a13d3520b63db171eb9597a26f0fb7da38ca7f` |

La nueva distribución no se instaló ni se abrió. La instancia existente continuó intacta, con PID
23400 y EXE SHA-256 `309d751b4c55759cce63b559bdaf58d81ee4befe163f2c6136eb8d100622f30d`.
