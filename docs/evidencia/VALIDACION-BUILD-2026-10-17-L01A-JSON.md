# Validación L-01A — JSON reversible de rutas POSIX — 2026-10-17

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambio

`path_wire` extiende la representación reversible `unix-bytes-v1` a favoritos, cestas y referencias
de espacios, búsquedas guardadas y puntos de comparación. Las rutas UTF-8 mantienen el formato JSON
string existente; los lectores aceptan el formato histórico. Las etiquetas desconocidas, hex inválido
y campos adicionales se rechazan.

El comparador de puntos usa claves de ruta nativas también para zonas inaccesibles. La validación
relativa examina separadores y segmentos `.`/`..` en los bytes POSIX originales para conservar los
rechazos previos sin convertir nombres no UTF-8 a texto con reemplazos. Ningún valor mostrado se usa
para reconstruir una ruta operativa.

## Verificación local

- `cargo test -p naygo-core --locked --offline`: 944 unitarios y 22 flujos aprobados en Windows.
- `cargo check -p naygo-core --tests --target x86_64-unknown-linux-gnu --locked --offline`: aprobado;
  compila los tests condicionados a Unix, que no se pueden ejecutar en este host Windows.
- `cargo clippy -p naygo-core --all-targets --target x86_64-unknown-linux-gnu --locked --offline -- -D warnings`:
  aprobado.
- `cargo fmt --all -- --check` y `git diff --check`: aprobados; Git avisó sobre line endings CRLF/LF
  en archivos preexistentes del árbol de trabajo.

## Gate y distribución Windows

- `scripts/build-release-guarded.ps1 -Verify`: gate del workspace y Clippy `-D warnings` completaron
  correctamente; pico Job 8,53 GiB. Esta corrida compila y ejecuta en Windows; el check cruzado
  separado es el que compila los tests condicionados a Unix.
- `scripts/build-release-guarded.ps1`: release ThinLTO generado en 30m37s; pico Job 10,48 GiB.
  Inno Setup terminó en 126,250 s. El portable contiene `LEEME.txt`, licencia, `naygo.exe` y notices.
- El hash del EXE dentro del ZIP coincide con `target/release/naygo.exe`.
- `Get-AuthenticodeSignature`: setup `NotSigned`; no se configuró firma.
- Checksums del manifiesto coinciden con ZIP y setup.
- Setup ejecutado en modo silencioso con código 0 en la instalación de usuario ya autorizada. El EXE
  instalado tiene versión 0.5.2, SHA-256 igual al nuevo release y proceso PID 28044 respondiendo.
  El PDB existente (`naygo.pdb`, fechado 2026-08-22) quedó intacto al no elegir símbolos.
- La reproducción manual 7-Zip→Naygo no se completó: `obj_1_tinker.obj.stl` ya existe en la carpeta
  destino con fecha previa; la automatización agotó timeout al capturar Naygo dos veces. No se
  sobrescribió el archivo. El bug E-2 sigue pendiente de aceptación interactiva.

## Artefactos

| Artefacto | SHA-256 |
|---|---|
| EXE release / portable | `ddc1ff74434cc46448f89617c95723121f9f310aa9115b2153bd3ab04446c4f9` |
| `Naygo-0.5.2-portable.zip` | `e949366eb70e7f8172330ade6cb447d809d51984d5a66b13277cc90f2d77351a` |
| `Naygo-0.5.2-setup.exe` | `bf49e370b1a8bd95b6905a817979b46467f6780dd3288eeb415a736558b374cf` |

La ejecución nativa de pruebas POSIX sigue pendiente; la compilación cruzada no la sustituye.
