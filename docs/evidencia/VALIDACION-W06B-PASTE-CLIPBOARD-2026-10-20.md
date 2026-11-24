# W-06B — lectura del portapapeles fuera del hilo UI

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Además de `op_paste`, varias acciones de UI todavía leían o escribían el portapapeles de forma
síncrona: Copiar/Cortar, pegar en la Cesta, copiar rutas/texto/CSV/procedencia y copiar texto del
preview. Un proveedor externo lento o una lista grande podía ocupar el hilo de interfaz.

Se añadió un worker único por sesión con cola acotada y orden FIFO para todas las lecturas y
escrituras del portapapeles. `op_paste` usa el mismo servicio, por lo que su lectura queda detrás de
Copiar/Cortar pendientes y se ejecuta dentro de `paste-target-probe`; pegar en la Cesta solicita la
lectura asíncronamente y aplica el resultado desde el tick. Las rutas se comparten con el worker
mediante `Arc` en vez de duplicar la lista. El indicador de corte y el historial local sólo se
actualizan tras una escritura exitosa; los fallos se registran y muestran mediante toast.

Las dos pruebas focalizadas verifican escritura fuera del hilo llamador, orden de escrituras y que
una lectura encolada vea la escritura anterior: **2 aprobadas, 0 fallidas**.

Validación completa Windows: `scripts/test-all.ps1` terminó con **1.500 tests aprobados, 8
ignorados**, Clippy `-D warnings` y formato correctos después de corregir un aviso de `while_let_loop`.
El check cruzado
`cargo check -p naygo-ui-slint --bin naygo --target x86_64-unknown-linux-gnu --locked` también
terminó correctamente; `build.rs` avisó que el recurso PE no aplica al target Linux. No se
ejecutaron tests en Ubuntu nativo.

La distribución Windows 0.5.3 se regeneró bajo supervisor: release **46m08s**, Inno Setup **176,562 s**.
Los hashes recalculados coinciden con `dist/SHA256SUMS.txt`:

- ZIP portable: `1b8212b02d18e71eb07f6d33e24c92fc63344615e44955d2d11ecb21518b4453`
- Instalador: `998c30daa7422d61e4187603462b575be7641549f189361c3658fd5771333000`
- Release EXE: `868a69dec0f6f79d0e91e54326eb9762cd745921467dd45a72fd5a4000bd2b5a`

El ZIP contiene exactamente `LEEME.txt`, `LICENSE`, `naygo.exe` y `THIRD-PARTY-NOTICES.md`; no
contiene PDB. El `naygo.exe` del ZIP coincide byte a byte con Release. Metadatos PE: producto Naygo,
versión 0.5.3, descripción `Naygo — explorador de archivos`, empresa ISGroth — Nicolás Groth y
copyright esperado. EXE y setup están `NotSigned`; el setup incluye el PDB opcional generado. No se
instaló ni publicó esta distribución. Naygo sigue abierto/respondiente como PID 31304 con el build
instalado anterior (`cb8f28efcd9619463a5977ba6fb771e59656352bb4a49dfe76bb9fb3c0682229`). No se
registró el pico de memoria del build.

Alcance: todas las lecturas/escrituras de clipboard desde callbacks de UI pasan por el worker común;
la implementación Linux del backend nativo y la interoperabilidad con Nautilus/Dolphin siguen fuera
de este cambio y requieren pruebas nativas en Ubuntu.
