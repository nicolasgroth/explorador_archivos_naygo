# Validación W-17 — renombrado sin reemplazo

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-18 (America/Santiago). Base: Naygo 0.5.2.

## Cambio

Renombrar y renombrar por lotes delegan la publicación al adaptador atómico sin reemplazo que
también usan los movimientos. Si otro proceso crea el destino después de planificar, el paso falla
y conserva tanto el origen como el elemento externo. En Windows, cuando el cambio solo modifica
mayúsculas/minúsculas, el motor mantiene la llamada de rename normal para actualizar la grafía del
mismo nombre lógico.

## Validación

- Regresión `rename_no_pisa_destino_que_aparece_despues_de_planificar`: aprobada en Windows.
- Regresión `rename_solo_case_actualiza_la_grafia_en_windows`: aprobada en Windows.
- `scripts/test-all.ps1`: 1.458 pruebas aprobadas, 8 ignoradas; Clippy sin avisos y formato aprobado.
- `cargo check -p naygo-core --tests --target x86_64-unknown-linux-gnu --locked`: aprobado. El test
  POSIX se compila para Linux, pero no se ejecutó en un host Linux nativo.
- `git diff --check` y `cargo fmt --all -- --check`: aprobados. Git mostró solo avisos informativos
  preexistentes de conversión CRLF/LF.
- `graphify update .`: completado; 9.294 nodos, 17.608 aristas y 879 comunidades.

## Distribución Windows

`scripts/build-release-guarded.ps1` terminó correctamente con pico supervisado de 10,53 GiB;
compilación release de 30m25s e Inno Setup de 118,360 s.

| Artefacto | SHA-256 |
| --- | --- |
| `dist/Naygo-0.5.2-portable.zip` | `1552d003ac8ab5535a6ac1b24cb5ed37da505565bcba90a32c749cc112a4e87a` |
| `dist/Naygo-0.5.2-setup.exe` | `c9f551be8d260354da351438fe9e61604807cf81ba4f0a05d47bbd6c1ee13de5` |
| `target/release/naygo.exe` | `c1b079ae296ba17ed772657b64ac987efa8f5e95fd718e2f0cde547048ab4df1` |

Los hashes del ZIP y el setup coinciden con `dist/SHA256SUMS.txt`; `naygo.exe` dentro del ZIP
coincide con el ejecutable de Release. El ZIP contiene `LEEME.txt`, `LICENSE`, `naygo.exe` y
`THIRD-PARTY-NOTICES.md`; no contiene PDB. El setup permanece sin firma Authenticode. La build no se
instaló: PID 23868 continúa ejecutando la versión instalada previamente, SHA-256
`21e97c3557ce5998429c83d0820e3212e5a8c2ebdca77a27eafdc45e1f4fd831`.
