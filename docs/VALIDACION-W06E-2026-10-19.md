# Validación W-06E — temas personalizados fuera de UI

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambio

Guardar y borrar temas personalizados se despacha al worker de packs; los callbacks UI solo preparan
la operación y aplican el resultado/catálogo al recibirlo. El guardado ahora usa
`naygo_core::fs_util::write_atomic`, de modo que una interrupción no trunca el JSON vigente. En un
renombrado, el archivo anterior se elimina después de escribir el nuevo. El borrado continúa siendo
idempotente cuando el archivo ya no existe.

## Verificación

- `cargo test -p naygo-ui-slint --bin naygo --locked --offline packs::worker_tests`: **4 passed**,
  incluidos guardado atómico con renombrado, borrado idempotente, carga de catálogos y exportación
  de configuración en worker.
- `scripts/test-all.ps1`: **1.480 tests aprobados, 8 ignorados**, Clippy `-D warnings` y formato
  pasaron.
- `git diff --check`: pasó (avisos CRLF preexistentes en otros archivos).

## Distribución Windows 0.5.3

`scripts/build-release-guarded.ps1` completó Release en **40m08s** e Inno Setup en **195,625 s**;
pico del Job **10,65 GiB** bajo el límite de 12 GiB. Versión File/Product: **0.5.3**.

| Artefacto | SHA-256 | Verificación |
|---|---|---|
| `dist/Naygo-0.5.3-portable.zip` | `06137391a1b317a815a6a68f422563831a7d8633df7593176bfc300ab2df6b92` | Manifiesto coincide; 4 entradas, sin PDB |
| `dist/Naygo-0.5.3-setup.exe` | `2303e71a3da3bec0422b7ddae685310ce4957e9e7a91b543e3dfa8db6e62cca6` | Manifiesto coincide; Inno compiló |
| `target/release/naygo.exe` | `dda9e59883c170b6d966cf6fccd6de2504a7df76f5596bad4707e20baf4d59fb` | Coincide con el EXE dentro del ZIP |

Setup y EXE siguen sin Authenticode. El build se regeneró, pero no se instaló ni publicó; la
instancia PID 42728 continúa abierta con el ejecutable W-06D. El PDB de diagnóstico se conserva
fuera del ZIP.

La actualización del catálogo y de settings ocurre al volver al hilo UI; la persistencia de settings
usa el escritor asíncrono ya existente. Esta validación no afirma aceptación manual del editor de
temas.
