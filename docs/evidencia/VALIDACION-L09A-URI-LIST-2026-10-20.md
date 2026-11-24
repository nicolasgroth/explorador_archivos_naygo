# L-09A — codec inicial para text/uri-list

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-20. Subpaso local de core; no activa integración del portapapeles.

## Alcance

Se añadió `naygo_core::clipboard::uri_list` como codec puro. Codifica rutas absolutas como URIs
`file:` con terminadores CRLF y percent-encoding; decodifica autoridades vacías o `localhost` a
bytes de ruta sin exigir UTF-8. Rechaza esquemas diferentes, hosts remotos, rutas relativas, escapes
inválidos, NUL, controles sin escapar, query/fragment sin escapar y separadores `/` codificados.
No toca el sistema de archivos, no ejecuta URIs y no usa clipboard del SO.

## Validación

- `cargo test -p naygo-core --locked clipboard::uri_list -- --nocapture`: **9/9 aprobadas**.
- `scripts/test-all.ps1`: **1.492 aprobadas, 8 ignoradas**, Clippy sin warnings y formato correcto.
- `cargo check -p naygo-core --target x86_64-unknown-linux-gnu --locked`: aprobado.
- `cargo clippy -p naygo-core --target x86_64-unknown-linux-gnu --all-targets --locked -- -D warnings`: aprobado.
- `graphify update .`: 9.622 nodos, 18.249 relaciones y 902 comunidades.
- `git diff --check`: aprobado; sólo reportó avisos de normalización CRLF preexistentes en otros archivos.

El check cruzado confirma compilación del core para el target Linux; no sustituye ejecución en
Ubuntu. El host continúa sin poder iniciar WSL (`HCS_E_HYPER_V_NOT_INSTALLED`), por lo que este codec
aún requiere pruebas Linux nativas. L-09A sigue abierto: en ese primer subpaso todavía no se habían
implementado los formatos copy/cut. Estos se añadieron en la validación complementaria
[`VALIDACION-L09A-FORMATOS-2026-10-20.md`](VALIDACION-L09A-FORMATOS-2026-10-20.md). Siguen pendientes
el transporte del portapapeles, la propiedad del clipboard y consumir el corte sólo tras una
operación completada. L-09B permanece sin implementar/validar. El codec no cambia el comportamiento
del binario distribuido y no requirió regenerar `dist/`.
