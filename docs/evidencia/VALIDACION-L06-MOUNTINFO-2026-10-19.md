# Naygo — preparación del parser mountinfo (L-06)

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

El módulo `crates/platform/src/mountinfo.rs` añade un parser puro que consume un `BufRead` en
streaming y conserva los campos de ruta como bytes. Decodifica los cuatro escapes definidos por
Linux (`space`, tab, newline y backslash), mantiene escapes desconocidos byte por byte, valida
IDs/dispositivo y rutas absolutas, y cuenta/omite líneas malformadas mientras conserva el resto del
snapshot. No abre `/proc` ni se conecta a `drives()`: esa lectura debe integrarse en un worker para
no bloquear el event loop.

Validación del 2026-10-19:

- `cargo test -p naygo-platform --locked mountinfo::tests`: 6/6 pruebas pasaron en Windows.
- `scripts/test-all.ps1`: **1.474 tests aprobados, 8 ignorados**, Clippy sin warnings y formato limpio.
- `cargo check -p naygo-platform --tests --target x86_64-unknown-linux-gnu --locked --jobs 1` y
  Clippy cruzado para todos los targets con `-D warnings`: ambos pasaron.
- `graphify update .`: 9.413 nodos, 17.799 relaciones, 886 comunidades. Graphify reportó etiquetas
  semánticas anteriores (892) y 29 archivos de recursos sin nodos AST; el grafo de código se
  actualizó correctamente.

Esto valida el parser y que compila para Linux; no valida lectura real de `/proc/self/mountinfo`,
clasificación/enumeración de volúmenes, mount watchers ni integración visual. No se recompiló la
distribución 0.5.3 porque este subpaso no está conectado al runtime. Debe completarse la integración
en worker y validarse en Ubuntu 26.04 antes de anunciar soporte.

## Integración de enumeración en `drives()` — 2026-10-19

La función Linux de `platform::drives::drives()` ahora abre `/proc/self/mountinfo` y usa el parser
en el worker de refresco ya existente. Omite pseudofs de la lista rápida; conserva filesystems y
mount points de red conocidos; deduplica mount points apilados con el registro más reciente; ordena
por ruta; y conserva bytes no UTF-8 al construir `PathBuf`. Un error o snapshot vacío mantiene `/`
como fallback. Los tipos no clasificados son `Unknown`, así que no habilitan expulsión.

Validación del delta:

- `scripts/test-all.ps1`: **1.477 tests aprobados, 8 ignorados**, Clippy `-D warnings` y formato
  aprobados. La política de pseudofs/tipos de red ejecutó en Windows; los casos de mountinfo, rutas
  no UTF-8 y duplicados se compilaron para Linux pero requieren ejecución nativa.
- `cargo check -p naygo-platform --tests --target x86_64-unknown-linux-gnu --locked --jobs 1` y
  `cargo clippy -p naygo-platform --all-targets --target x86_64-unknown-linux-gnu --locked --jobs 1
  -- -D warnings`: aprobados.
- La enumeración se ejecuta en el worker existente. No se probaron mounts reales ni la tira de
  unidades en Ubuntu; clasificación de removibles, watcher de montajes e invalidación siguen
  pendientes. El gate Windows no valida el runtime Linux.
