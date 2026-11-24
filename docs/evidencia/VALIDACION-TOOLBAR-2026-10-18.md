# Validación E-5 — organización de la barra de herramientas

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-18 (America/Santiago). Base: Naygo 0.5.2.

## Cambio

La barra ya disponía de controles, tooltips y separadores. Se hizo más clara su jerarquía y se
reordenaron los grupos para que la progresión de izquierda a derecha sea: navegación → ubicación →
búsqueda/visibilidad/recientes/favoritos → paneles y disposiciones → acciones → unidades →
Configuración. La ruta se mantiene flexible y ahora acompaña a la navegación; Configuración queda al
extremo derecho. Los separadores ganaron 2 px de espacio horizontal, conservan el color del tema y
no incorporan tareas, lecturas de disco ni trabajo adicional por frame. La guía explica el orden y
el uso de tooltips en modo de solo iconos.

## Validación automática

- `cargo fmt --all -- --check`: aprobado.
- `cargo test -p naygo-ui-slint --locked --test toolbar_path`: 1 aprobado.
- `scripts/test-all.ps1`: 1.450 aprobados, 8 ignorados; Clippy con `-D warnings` y formato aprobados.
- `7z t dist/Naygo-0.5.2-portable.zip`: cuatro entradas verificadas, sin errores.
- El hash del `naygo.exe` dentro del ZIP coincide con `target/release/naygo.exe`.
- `git diff --check`: aprobado (Git emitió avisos informativos de conversión CRLF/LF en archivos del
  árbol de trabajo).
- `graphify update .`: grafo actualizado a 9.208 nodos, 17.472 aristas y 860 comunidades. Conserva
  el aviso de 29 archivos fuente sin nodos, incluidos JSON; la extracción es AST-only.

## Distribución Windows

`scripts/build-release-guarded.ps1` terminó correctamente: pico supervisado 10,49 GiB, compilación
release 33m07s e Inno Setup 114,843 s. Los tres hashes se contrastaron con `dist/SHA256SUMS.txt`.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/Naygo-0.5.2-portable.zip` | 14.375.254 | `cdfa552c39b5daddac8f50cb51138bed888f0c825fd6674ecaa961d53009f53c` |
| `dist/Naygo-0.5.2-setup.exe` | 66.317.440 | `60881e95f543fd37411a94683ec13cd176075e27787f30b30316939c7dad997f` |
| `target/release/naygo.exe` | 43.424.768 | `681b99ae728c0067d45b6d1a5f2bb52c9951bb32bc012ce96c405b7e5d39159f` |

La instalación se realizó con autorización previa mediante el setup; el código de salida fue 0.
Naygo quedó abierto como PID 51408 desde
`%LOCALAPPDATA%\Programs\Naygo\naygo.exe`. El SHA-256 instalado coincide con el
release (`681b99ae728c0067d45b6d1a5f2bb52c9951bb32bc012ce96c405b7e5d39159f`). El setup continúa sin
firma Authenticode.

## Pendiente de aceptación

Ver la toolbar en pantalla con varios temas y tamaños de ventana, confirmar que la ruta conserva
espacio útil y que todos los grupos se distinguen sin ocultar unidades ni botones. El cambio mantiene
el modo compacto y los tooltips; esta revisión visual aún no se ejecutó.
