# L-05: capacidad de elevación y distribución Windows — 2026-10-18

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambio

`can_run_as_administrator()` sólo informa capacidad disponible en Windows. En otras plataformas,
incluido Linux, devuelve `false`, de modo que la interfaz oculta «Ejecutar como administrador» para
`.exe` y `.com`. Naygo no reemplaza la acción por `sudo`. La comprobación no consulta el disco.

## Validación

- `scripts/test-all.ps1`: 1.453 pruebas aprobadas, 8 ignoradas; Clippy de todos los targets sin
  avisos y `cargo fmt --all -- --check` aprobado.
- `cargo test -p naygo-platform --locked --quiet`: 67 aprobadas, 6 ignoradas.
- `cargo check -p naygo-platform --all-targets --target x86_64-unknown-linux-gnu --locked --offline --jobs 1`
  y Clippy equivalente con `-D warnings`: aprobados. Esto compila el test POSIX, pero no lo ejecuta;
  no hubo ejecución Linux nativa.
- `graphify update .`: completado. Graphify avisó que algunos archivos de traducción no producen
  nodos AST; el grafo se reconstruyó correctamente.
- `scripts/build-release-guarded.ps1`: completado. Pico del Job 10,52 GiB; Rust release 28m26s e
  Inno Setup 117,844 s. No se instaló ni ejecutó la distribución.

## Artefactos verificados

`dist/SHA256SUMS.txt` concuerda con los hashes calculados directamente:

| Artefacto | SHA-256 |
| --- | --- |
| `Naygo-0.5.2-portable.zip` | `f6935af5c6018e5b8a4de43c4dc4362411278e34ca1aec592ec9cb713887556e` |
| `Naygo-0.5.2-setup.exe` | `edfc10def89298e1021718e400e3af507b697883a7b55d17b82986a279d7570d` |

El empaquetador avisó que el instalador carece de firma Authenticode. La firma depende del trabajo
SignPath W-14 y sigue pendiente.

## Alcance pendiente

Este subpaso corrige la disponibilidad de una acción de UI y no completa L-05. Abrir por defecto,
«Abrir con», terminal seguro, diálogos y validación en Ubuntu 26.04 nativo siguen pendientes. En
particular, la prueba `no_ofrece_elevacion_windows_en_otras_plataformas` debe ejecutarse en Linux.
