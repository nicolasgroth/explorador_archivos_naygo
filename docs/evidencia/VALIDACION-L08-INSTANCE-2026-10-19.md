# L-08: instancia Linux y validación del release local

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-19. Cambio local; no publicado.

## Implementación

Se añadió la instancia única Linux en `crates/platform/src/single_instance/linux.rs`. El backend
acepta `XDG_RUNTIME_DIR` sólo cuando es absoluto, corresponde a un directorio real sin symlink,
pertenece al UID efectivo y tiene modo privado 0700. Mantiene un lock exclusivo para arbitrar el
arranque, crea un Unix socket 0600 y elimina únicamente sockets obsoletos verificados. El worker
valida el UID del peer con `SO_PEERCRED`, limita los mensajes a 4.096 bytes y conserva rutas POSIX
como bytes, incluidos nombres no UTF-8. No consulta el filesystem al recibir rutas ni bloquea el
hilo UI; el tick drena una cola acotada a ocho solicitudes y descarta la más antigua ante ráfagas.
`--shutdown` usa el mismo IPC y el guard despierta/cierra al listener durante el cierre.

Cuando el entorno no ofrece un runtime directory seguro, Naygo deja la coordinación de instancia
deshabilitada y sigue disponible. La ruta de Windows no cambia. Los tests añadidos cubren permisos y
symlink del runtime dir, lock y socket obsoleto, cierre, frames inválidos, preservación de rutas no
UTF-8 y envío de wake + ruta. La arquitectura y los límites están documentados en el plan Linux.

## Comprobaciones

- `cargo check -p naygo-platform --tests --target x86_64-unknown-linux-gnu --locked --jobs 1` pasó.
- Clippy `-D warnings` cruzado de platform pasó.
- Check y Clippy `-D warnings` cruzados de `naygo-ui-slint` pasaron con el cambio de drenaje de cola.
- `cargo fmt --all -- --check` pasó.
- `scripts/test-all.ps1` pasó: **1.480 tests aprobados, 8 ignorados**, Clippy limpio y formato correcto.
- `graphify update .` se ejecutó después del cambio de código.
- **No se ejecutaron tests en Linux.** WSL devuelve `HCS_E_HYPER_V_NOT_INSTALLED`; no se habilitó
  virtualización ni se modificaron componentes Windows. Los comandos `cargo check`/Clippy son
  compilación cruzada y no validan sockets, permisos efectivos, carreras ni integración del event loop
  en Ubuntu. L-08 sigue abierto hasta ejecutar su matriz en Ubuntu 26.04 nativo.

## Distribución Windows 0.5.3

Se ejecutó `scripts/build-release-guarded.ps1` sin otras tareas pesadas en paralelo. Release tomó
41m23s; Inno Setup, 199,766 s; el pico del Job fue 10,66 GiB, dentro del límite de 12 GiB. El ZIP
verificado contiene `LEEME.txt`, `LICENSE`, `naygo.exe` y `THIRD-PARTY-NOTICES.md`; no incluye PDB.

| Artefacto | SHA-256 | Firma |
| --- | --- | --- |
| `dist/Naygo-0.5.3-portable.zip` | `19f023b99e9a37656da36cba3d72e322586f3000e691225c33d99db5d3bf4cdf` | No aplica |
| `dist/Naygo-0.5.3-setup.exe` | `c1d3ee78eafe71ca829f506c3a3c548f72369a23ee636987694cacd824cf0687` | Sin firma Authenticode |
| `target/release/naygo.exe` | `960ccee8a5bdfb85eaafcd8fa6112e736d4b050237e7fdfb592f41799671cadc` | Sin firma Authenticode |

El setup se instaló con la autorización previa de Nicolás y se dejó Naygo abierto. A las 21:19
local, el proceso PID 50260 respondía y el EXE instalado tenía el mismo hash que Release. Durante
la instalación se comprobaron y conservaron los hashes de `settings.json`, `workspace.json`,
`favorites.json`, `templates.json` y del `naygo.pdb` previo. No se publicó ni firmó ningún artefacto.

## Pendiente

En Ubuntu 26.04 x86_64: ejecutar los tests Linux nuevos en filesystem local, comprobar dos procesos
concurrentes y dos usuarios, muerte/reinicio de primaria, sockets stale/corruptos, shutdown con
solicitudes pendientes, permisos adversos, rutas no UTF-8, GNOME/Wayland y X11. Confirmar que el
event loop despierta la ventana de forma fiable y que el arranque sin `XDG_RUNTIME_DIR` degrada como
está descrito. Hasta entonces, la implementación no se presenta como validada ni como soporte Linux
disponible.
