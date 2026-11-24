# Reinspección pasiva de UI Automation — 2026-10-19

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Resultado

Se ejecutó `scripts/inspect-naygo-uia.ps1 -ProcessId 42728 -MaxDepth 8 -MaxElements 1200`
contra la instancia que ya estaba abierta. No se activó la ventana ni se enviaron clics o teclas.
El inspector recorre Control View y sólo registra conteos y tipos; no lee ni registra nombres,
rutas o contenido de controles.

| Medida | Resultado |
| --- | ---: |
| PID | 42728 |
| Versión de archivo | 0.5.3 |
| Descendientes UIA | 980 |
| Elementos con nombre | 785 |
| Enfocables por teclado | 219 |
| Enfocables con nombre | 219 |
| Tipos enfocables sin nombre | Ninguno |
| Árbol truncado | No |

Tipos expuestos: Button 84, CheckBox 1, Image 165, List 6, ListItem 134 y Text 590.
El proceso respondió al terminar la inspección.

## Alcance y limitaciones

El SHA-256 del EXE instalado es
`6015599ce33acd41b5763611cc4fbfd84a49ce6a81df0652cbc3394eef3a2c60`; no coincide con
`target/release/naygo.exe` (`dda9e59883c170b6d966cf6fccd6de2504a7df76f5596bad4707e20baf4d59fb`).
La instancia no representa el Release 0.5.3 W-06E recién reconstruido. El conteo confirma que, en
la ventana principal de la instalación abierta, todos los controles enfocables expuestos tienen
nombre accesible. No comprueba la calidad del texto anunciado, los diálogos cerrados, las acciones
de controles no enfocables ni la navegación con Narrador; W-13 sigue pendiente de aceptación manual
en una sesión desbloqueada y sobre la distribución actual.
