# UI Automation pasiva de Naygo 0.5.3 — 2026-10-20

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Resultado

Se ejecutó `scripts/inspect-naygo-uia.ps1 -ProcessId 43624 -MaxDepth 8 -MaxElements 1200` sobre
la ventana ya abierta de Naygo. La inspección consulta el árbol Control View y la disponibilidad de
patrones; no activa controles ni lee nombres de archivos, rutas o valores. El proceso respondió y
el árbol no quedó truncado.

| Medida | Resultado |
| --- | ---: |
| PID | 43624 |
| Versión | 0.5.3 |
| Descendientes UIA | 356 |
| Elementos con nombre | 295 |
| Enfocables por teclado | 120 |
| Enfocables con nombre | 120 |
| Enfocables sin nombre | 0 |
| Botones con patrón Invoke | 69 de 69 |
| Árbol truncado | No |

El SHA-256 del EXE instalado coincide con `target/release/naygo.exe`:
`868a69dec0f6f79d0e91e54326eb9762cd745921467dd45a72fd5a4000bd2b5a`.

## Alcance pendiente

Esto acredita exposición estructural UIA de la ventana principal del release actual. No valida la
calidad del texto que anuncia un lector de pantalla, la navegación real con Narrador, foco tras
acciones, diálogos cerrados ni flujos con teclado. W-13 permanece abierto hasta completar esos
recorridos en la sesión interactiva. Resultado de conteos, sin nombres ni valores, en
`target/agent-out/w13-installed-053-20261020-passive.json`.
