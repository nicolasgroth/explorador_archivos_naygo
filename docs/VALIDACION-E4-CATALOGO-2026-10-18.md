# Validación E-4 — recientes persistentes de espacios por tarea

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-18 (America/Santiago). Base: Naygo 0.5.2.

## Cambio

Disposiciones conserva hasta diez rutas MRU absolutas de archivos `.naygospace` en `settings.json`;
el menú muestra hasta cinco. Los settings anteriores se cargan con la lista vacía. La carga y
normalización se integran con el worker inicial asíncrono. El arranque no abre los documentos ni
explora sus carpetas. Si durante la carga se crean nuevos recientes, se mantienen al frente de la
lista persistida. «Limpiar recientes» elimina solo las referencias, nunca los archivos.

La aplicación del espacio sigue siendo explícita: una sesión limpia cambia al terminar la lectura;
si hay cambios pendientes, se mantiene la revisión existente con guardar/descartar/cancelar. No se
autoguardan espacios por tarea ni se guardan listados o bytes del preview en settings.

## Validación

- Round-trip de `task_space_recents` y compatibilidad con settings anteriores: aprobado.
- Prueba UI de límite diez, deduplicación, reapertura, merge con recientes de arranque y limpieza:
  aprobada.
- Prueba de selección rápida con espacio limpio y con cambios pendientes: aprobada.
- `scripts/test-all.ps1`: **1.452 pruebas aprobadas, 8 ignoradas**; Clippy con `-D warnings` y
  formato aprobados.
- `graphify update .`: 9.233 nodos, 17.516 aristas, 856 comunidades.
- Check de `core`/`platform` y Clippy `-D warnings` cruzados para `x86_64-unknown-linux-gnu`:
  aprobados; esto no ejecuta pruebas POSIX ni valida la UI Linux.
- Instalación in-place desde el setup: código de salida 0. El EXE instalado coincide con Release;
  UIA de solo lectura encontró 726 descendientes, 539 con nombre y 55 enfocables, todos con nombre.
  No se enviaron teclas ni clics; esto no sustituye la aceptación visual/manual.
- `settings.json`, `workspace.json`, `favorites.json` y `templates.json` conservaron sus hashes.
  El PDB previo también conservó su SHA-256 con los símbolos desmarcados.
- `git diff --check`: aprobado; solo avisos de normalización CRLF/LF en archivos previamente
  modificados del workspace.

## Distribución Windows

Build Release 0.5.2 supervisado e instalado in-place:

- Pico contable: 10,53 GiB; ThinLTO: 34m28s; compilación del setup: 130,859 s.
- Portable: 14.390.298 bytes, SHA-256
  `db8ae06536589eb833354fc2a71ce0e6379b0f485210580b8c78164cf805ea9b`.
- Setup: 66.525.879 bytes, SHA-256
  `675bc1db70606d689204575964caea657a9da061411b5fc1e11f7d5a163a9113`.
- `target/release/naygo.exe`: 43.523.072 bytes, SHA-256
  `feba7919f7f308bacb8198b4abf5eb61c1d8fbfafbe5ff1171916caf92376293`; coincide con el ejecutable
  extraído del portable.
- 7-Zip verificó las cuatro entradas del ZIP. Los hashes de la distribución están en
  `dist/SHA256SUMS.txt`.
- El setup sigue sin firma Authenticode. Naygo quedó abierta como PID 25124; su working set inicial
  fue 71,2 MiB. La muestra pasiva previa pertenece al proceso E-5 anterior.

## Pendiente

Falta aceptación visual del menú y del gestor, además de probar el flujo real con rutas ausentes o
remotas. El conector de UI de esta sesión no expone ventanas nativas, por lo que no permite afirmar
aceptación visual. Evidencia anterior del primer selector: `docs/VALIDACION-E4-SELECTOR-2026-10-18.md`.
