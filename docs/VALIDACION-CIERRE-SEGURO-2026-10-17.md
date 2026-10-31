# Cierre seguro cuando no hay vía de retorno — 2026-10-17

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambio

La preferencia «Cerrar a la bandeja» conserva el proceso solo si queda una forma funcional de
mostrar de nuevo la ventana: ícono de bandeja activo o atajo global realmente registrado. Si ambas
vías faltan, cerrar persiste la sesión y sale; evita ocultar una ventana sin retorno. En Linux el
atajo global aún es un stub, así que un tray ausente ya no deja la aplicación inaccesible. En
Windows las dos vías siguen siendo alternativas.

## Verificación

- Test de la tabla de decisión: sin preferencia, salir; con bandeja o hotkey funcional, ocultar; sin
  ninguna vía, salir.
- `cargo test -p naygo-ui-slint --bin naygo --locked --offline`: 397 aprobados, 2 ignorados.
- Gate `scripts/build-release-guarded.ps1 -Verify`: 1.447 pruebas aprobadas, 8 ignoradas, sin fallos;
  Clippy `-D warnings` y formato limpios. Pico contable 5,96 GiB.
- `cargo fmt --all -- --check` y `git diff --check`: aprobados; este último solo informó avisos de
  normalización CRLF en archivos preexistentes.
- `scripts/build-release-guarded.ps1`: release compilado en 26m18s; pico 10,49 GiB. Inno Setup
  compiló en 115,922 s. EXE del ZIP idéntico al release, ZIP sin PDB y con `LICENSE`/notices.
- Al cerrar el build, los artefactos quedaron en `dist/` y no se instalaron ni firmaron. Después,
  el usuario autorizó actualizar la instalación existente; el setup local sigue sin firma. La
  aceptación visual del tray Linux requiere Ubuntu 26.04/derivado nativo y no se realizó.

## Artefactos

| Artefacto | SHA-256 |
|---|---|
| EXE release / portable | `ed1f349116181dc60d3ccc4fa5dbb6ea6969e4c4816145cfe75d6c55ac278a9e` |
| `Naygo-0.5.2-portable.zip` | `ab8731d4eaa99c224ac49ce29def724aa50386b2e873d1aceb6170ca07858017` |
| `Naygo-0.5.2-setup.exe` | `d101e0d1a62d2a263068d5c7975e091f5d9232de5f73e6ca24d59564bbc531e0` |

Authenticode permanece pendiente de credenciales/configuración SignPath (W-14).

## Instalación posterior — 2026-10-17 21:16 (America/Santiago)

Con autorización previa se actualizó in-place desde `dist/Naygo-0.5.2-setup.exe`; no se publicó ni
firmó el artefacto. El PID anterior 38744 recibió `--shutdown` y cerró limpiamente. Inno Setup terminó
con código 0. La versión instalada declara 0.5.2 y su SHA-256 coincide con el release:
`ed1f349116181dc60d3ccc4fa5dbb6ea6969e4c4816145cfe75d6c55ac278a9e`. El proceso PID 56616 quedó
abierto con ventana principal; UI Automation de solo lectura expuso 691 descendientes, 514 con nombre
y 55 enfocables. El log del día contiene un único `PANIC`, a las 12:58, previo a la instalación, y
ningún marcador `error`/`fatal` posterior. Esta observación no valida Narrador ni interacción.

Antes/después del instalador conservaron SHA-256 idéntico `settings.json`, `workspace.json`,
`favorites.json`, `templates.json` y el PDB preexistente
`eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`. El respaldo independiente
está en `target/agent-out/l11-install-preserve-20261017-211618-8d2bd046`.
