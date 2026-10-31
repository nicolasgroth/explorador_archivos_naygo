# Validación L-02 — importación de configuración entre plataformas

Fecha: 2026-10-18 (America/Santiago)

## Cambio

Al importar un `.naygoconf`, el worker valida los ajustes de ruta antes de escribirlos. Omite rutas
de inicio y rutas recientes que no sean absolutas para el sistema destino, sin convertir sintaxis
Windows en rutas POSIX ni al revés. En Linux, también apaga autoinicio y atajo global si el archivo
los tenía activos, ya que estas integraciones aún no están disponibles. Al terminar, la UI presenta
un aviso localizado con los campos omitidos. Los ajustes comunes, keybindings y rutas nativas válidas
se mantienen. El formato de configuración exportado no contiene preferencias de terminal.

## Verificación

- Cuatro pruebas dirigidas cubren import Windows→Linux, preservación de paths POSIX, import Windows
  sin cambios y filtrado antes de escribir el archivo.
- `cargo test -p naygo-ui-slint --locked --offline import_`: 10 pasaron.
- `scripts/test-all.ps1`: 1.464 aprobadas, 8 ignoradas; Clippy sin avisos y formato correcto.
- `cargo check -p naygo-ui-slint --all-targets --target x86_64-unknown-linux-gnu --locked --offline --jobs 1`:
  aprobado.
- `cargo clippy -p naygo-ui-slint --target x86_64-unknown-linux-gnu --locked --offline --jobs 1 -- -D warnings`:
  aprobado.
- Paridad i18n: diez idiomas aprobados. `git diff --check` no detectó errores; Git informó solo
  conversiones CRLF/LF en archivos ya editados.
- `graphify update .`: 9.350 nodos, 17.704 relaciones y 893 comunidades.

## Paquete Windows

El cambio de importación es compartido, por lo que se regeneró la distribución Windows con
`scripts/build-release-guarded.ps1`. El límite del Job fue 12 GiB, con umbral de 8 GiB libres; pico
10,63 GiB. Release terminó en 28m35s e Inno Setup en 116,890 s.

- `dist/Naygo-0.5.2-portable.zip` — SHA-256
  `3f919371d9452a1b60041e159ea41664599b360c0480335617f7c1b15740ed5b`.
- `dist/Naygo-0.5.2-setup.exe` — SHA-256
  `955b5aa5d822024ff129af7d686c74090d3bea61be39cee119de723308a19564`.
- `target/release/naygo.exe` — SHA-256
  `5e11f7bdf0fccd112d64c9703c277118d2e2abd7b097050692e4a7b14cfd8fe8`.
- Ambos artefactos coinciden con `dist/SHA256SUMS.txt`. `zipfile.testzip()` devolvió `None`; el
  ejecutable incluido coincide con Release y el ZIP no contiene PDB.
- `Get-AuthenticodeSignature` informa `NotSigned` para el setup. No se instaló ni publicó.

## Límites

Los tests de sanitización se ejecutan en Windows con el destino simulado; la compilación cruzada
Linux confirma el código, no su runtime. Falta repetir import/export en Ubuntu 26.04 nativo y comprobar
el diálogo con una sesión real. L-02 y L-03 siguen abiertos.
