# Corrección de columna Extensión y distribución Windows — 2026-10-17

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambio

Las carpetas ya no toman el último segmento de su nombre como extensión. La tabla y el CSV muestran
la etiqueta localizada «Carpeta»; el filtro y el conteo las agrupan en una categoría interna
reservada, y ordenar por extensión agrupa todas las carpetas sin depender de cuántos puntos tenga su
nombre. La categoría vacía sigue siendo para archivos sin extensión. `Other` conserva su sufijo cuando
lo tiene; si no, comparte la categoría sin extensión.

## Verificación

- `cargo fmt --all -- --check`: aprobado.
- `cargo test -p naygo-core extension --locked --offline`: 23 aprobados; incluye conteo, filtro y
  orden ascendente/descendente para carpetas con puntos.
- `cargo test -p naygo-ui-slint --bin naygo extension_cell_does_not_treat_dotted_folder_as_file_type --locked --offline`:
  1 aprobado.
- `cargo test -p naygo-ui-slint --bin naygo exportacion_csv_muestra_tipo_carpeta_aunque_el_nombre_tenga_puntos --locked --offline`:
  1 aprobado.
- `scripts/build-release-guarded.ps1 -Verify`: 1.445 pruebas aprobadas, 8 ignoradas, Clippy con
  `-D warnings` limpio. Pico del Job: 10,27 GiB, límite 12 GiB.
- `scripts/build-release-guarded.ps1`: portable ZIP y setup generados. Release tardó 29m27s, con
  pico de 10,47 GiB; el setup compiló correctamente en 118,547 s.
- `git diff --check`: aprobado. No se añadieron claves i18n; la etiqueta de carpeta usa traducciones
  existentes en los diez idiomas.
- Setup sin firma Authenticode.

## Artefactos

| Artefacto | SHA-256 |
|---|---|
| EXE release / portable | `97bd204bfacee564f9632658ec89308643bc07808fa15f1d0ce2890ac51ef5bc` |
| `Naygo-0.5.2-portable.zip` | `655522dd93e7a27f4248ce1a2407d0b32f1725f496e57133b603be8884eb7920` |
| `Naygo-0.5.2-setup.exe` | `bfe3b909ccaf3433f50fdcbb52ab0e7e881ec4deadd06d7c6acfa98a68e97329` |

Al cerrar la validación de build, la distribución estaba en `dist/` y no se había instalado. Después,
con la autorización previa del usuario, el setup se ejecutó en modo silencioso como actualización
in-place: código 0, sin reinicio. El EXE instalado tiene el mismo SHA-256 del release, inició como
PID 12232 y respondió. UI Automation expuso 729 descendientes, 546 con nombre y 55 enfocables; árbol
sin truncar. El PDB preexistente sigue presente (289.722.368 bytes, fecha 2026-08-22); el setup no
instaló símbolos nuevos. No se ejecutó el ZIP de forma separada ni se completó todavía la prueba
manual de drop 7-Zip o la aceptación de la columna Extensión. La instalación queda abierta con E-1.
