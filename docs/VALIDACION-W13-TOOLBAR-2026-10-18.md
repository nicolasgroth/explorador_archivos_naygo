# W-13 — semántica accesible de toolbar y unidades

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Hallazgo

La revisión estática del componente Slint encontró que `ToolBtn` y `DriveBtn` son controles dibujados
con `Rectangle` y `TouchArea`. Aunque respondían al clic, no publicaban al árbol de UI Automation /
AccessKit un rol de botón, un nombre ni una acción predeterminada. La inspección UIA pasiva anterior
contaba controles enfocables; por eso no era evidencia suficiente sobre estos elementos.

## Cambio

- Los botones de toolbar publican rol `button`, el tooltip descriptivo localizado como nombre,
  estado habilitado y una acción accesible que ejecuta el callback existente.
- Las unidades publican rol `button`, letra y ruta de destino como nombre y la acción de navegación.
- La flecha de expulsión en unidades removibles publica su propio botón y acción, separada de la
  navegación al hacer clic en el cuerpo de la unidad.
- Las acciones de fondo permanecen conectadas a los callbacks ya existentes; no introducen I/O ni
  cambian las operaciones de navegación o expulsión.

## Validación

- `cargo test -p naygo-ui-slint --locked`: tests UI aprobados; incluye compilación completa de Slint.
- `scripts/test-all.ps1`: 1.458 pruebas aprobadas, 8 ignoradas, Clippy sin avisos y formato correcto.
- La sesión Windows seguía bloqueada (`LogonUI` activo). La activación mediante Narrador, el árbol
  UIA en una sesión desbloqueada y la revisión visual siguen pendientes; este gate no cierra W-13.
## Build de distribución

El build supervisado finalizó correctamente el 2026-10-18: pico contable de 10,54 GiB, compilación
release en 26m05s e Inno Setup en 111,110 s. Los SHA-256 se comprobaron contra el manifiesto y el
EXE del ZIP:

| Artefacto | SHA-256 |
| --- | --- |
| `dist/Naygo-0.5.2-portable.zip` | `53dc5eadc77b2c55c838598a6de7a040f7d51549eb66b8bb5377fa07ae6ca573` |
| `dist/Naygo-0.5.2-setup.exe` | `6d46e4e1008a4d2421eed5c2dfce89bdbc1f62e61af60f6795b0d0d82fc91c5c` |
| `target/release/naygo.exe` | `d38ce414942d08bb2de5e04743409b66d62854a4422badc179a00046ea8f402a` |

El EXE dentro del ZIP coincide byte a byte con Release. El ZIP contiene `LEEME.txt`, `LICENSE`,
`naygo.exe` y `THIRD-PARTY-NOTICES.md`; no incluye PDB. `Get-AuthenticodeSignature` reportó
`NotSigned` para el setup. No se instaló la build; PID 23868 sigue ejecutando la versión anterior.
