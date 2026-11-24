# L-05: abrir con la aplicación predeterminada en Linux — 2026-10-18

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambio

En Linux, `open_default()` inicia `xdg-open` y retorna sin esperar a que termine la aplicación
externa. La ruta va como un único argumento de `Command`, sin shell. Las rutas relativas se vuelven
absolutas respecto del directorio de trabajo del proceso para que un nombre que empieza con `-` no
se interprete como opción. No se canonicaliza la ruta ni se siguen symlinks desde Naygo; se conserva
la representación nativa del `OsStr`, incluidos bytes no UTF-8. Windows conserva su implementación
`ShellExecuteW`.

La integración XDG resuelve la aplicación predeterminada para el archivo o URL; el escritorio puede
delegar la apertura en un portal. El [manual de `xdg-open`](https://portland.freedesktop.org/doc/xdg-open.html)
documenta la aplicación preferida para cada archivo/URL y su uso dentro de una sesión de escritorio.
El [portal OpenURI](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.OpenURI.html)
define `OpenFile` para abrir archivos locales; `xdg-open` mantiene la selección de la integración a
cargo del entorno instalado.

## Validación local

- `cargo fmt --all -- --check`: aprobado.
- `cargo test -p naygo-platform --locked open::linux_impl::tests -- --nocapture`: 2/2 aprobadas en
  Windows. Cubren una ruta relativa que empieza con guion y contiene espacios, comillas, `%`, salto
  de línea y Unicode; y una ruta absoluta con `..` que debe mantenerse sin canonicalizar.
- `cargo check -p naygo-platform --all-targets --target x86_64-unknown-linux-gnu --locked --offline --jobs 1`:
  aprobado; compila el punto de entrada Linux y todos los targets de test.
- `cargo clippy -p naygo-platform --all-targets --target x86_64-unknown-linux-gnu --locked --offline --jobs 1 -- -D warnings`:
  aprobado.
- El test POSIX para bytes de ruta no UTF-8 compila al cruzar todos los targets Linux, pero no se
  ejecutó en este host Windows.

## Límites

No se inició `xdg-open`: el host no tiene un escritorio Linux funcional. Por tanto, faltan validar en
Ubuntu 26.04 `xdg-utils`, el portal/backend real, una asociación ausente y los fallos que sucedan
después de iniciar el proceso. El `spawn` sólo confirma que se pudo lanzar `xdg-open`; Naygo no
recibe el resultado posterior del handler. «Abrir con» y las terminales configurables siguen
pendientes en Linux. Este subpaso no cierra L-05.

## Gate y distribución Windows

- `scripts/test-all.ps1`: 1.455 pruebas aprobadas, 8 ignoradas; Clippy sin avisos y formato
  aprobados.
- `scripts/build-release-guarded.ps1`: correcto. Pico supervisado 10,53 GiB, release 26m49s e
  Inno Setup 113,797 s. El setup no tiene firma Authenticode.
- El portable contiene `naygo.exe`, `LEEME.txt`, `LICENSE` y `THIRD-PARTY-NOTICES.md`; no contiene
  PDB. El SHA-256 del ejecutable dentro del ZIP coincide con `target/release/naygo.exe`:
  `db9cb73aecca82f622eab0285ffc0fed756cb30d2461f07a2d2290161ce331c1`.
- `dist/Naygo-0.5.2-portable.zip` SHA-256:
  `f3a41466491d43936b5fa304cde5d5745d628a581c110a798254992c751c3c13`.
- `dist/Naygo-0.5.2-setup.exe` SHA-256:
  `154065c92e6ccc77c16b444422b00766081ee871bd9b4012cde1493b197dc7d8`.
- Los hashes coinciden con `dist/SHA256SUMS.txt`. La distribución no se instaló ni ejecutó.

El cambio y el gate local Windows quedan cerrados; la aceptación nativa Linux descrita en «Límites»
sigue pendiente.
