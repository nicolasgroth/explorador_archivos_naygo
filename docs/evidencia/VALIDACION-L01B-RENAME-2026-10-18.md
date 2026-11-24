# L-01B — renombrado seguro del destino en conflicto

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-18 (America/Santiago). Base: Naygo 0.5.2.

## Cambio

La acción «Renombrar antiguo» aparta el destino actual a un nombre deduplicado antes de guardar el
archivo entrante en la ruta original. Antes, el nombre libre se comprobaba y luego se usaba
`std::fs::rename`; en POSIX, un proceso concurrente podía crear el nombre de respaldo en ese
intervalo y ser reemplazado por el rename. Ahora el motor usa `NoReplaceRename`: Linux publica con
`RENAME_NOREPLACE`, y la UI Windows inyecta el adaptador `MoveFileExW` sin reemplazo ni flags de copia.
Si el nombre de respaldo se ocupa, el paso falla y conserva el destino anterior, la fuente entrante
y el archivo externo.

## Validación

- Pruebas dirigidas `cargo test -p naygo-core --locked ask_rename_existing`: 2 aprobadas, 0 fallidas.
  Incluyen la decisión normal y un nombre de respaldo ocupado durante la carrera.
- `cargo test -p naygo-core --locked ops::retry::tests`: 3 aprobadas en Windows.
- `cargo check -p naygo-core --tests --target x86_64-unknown-linux-gnu --locked`: aprobado; incluye
  la prueba Unix que confirma que redirigir la carpeta destino por symlink invalida la revisión.
  Esa prueba todavía no se ejecutó en Linux nativo.
- `scripts/test-all.ps1`: 1.456 aprobadas, 8 ignoradas; Clippy sin avisos y formato aprobado.
- `git diff --check`: aprobado. Git emitió avisos informativos de normalización CRLF/LF en archivos
  preexistentes del árbol de trabajo.

## Distribución Windows

`scripts/build-release-guarded.ps1` terminó con salida 0. Pico supervisado: 10,53 GiB. Cargo release:
31 min 13 s; Inno Setup: 124,766 s. El ZIP tiene cuatro entradas (`LEEME.txt`, `LICENSE`, `naygo.exe`
y `THIRD-PARTY-NOTICES.md`), no incluye PDB, y la lectura completa de `naygo.exe` coincide por
SHA-256 con `target/release/naygo.exe`. Los hashes publicados coinciden con `dist/SHA256SUMS.txt`.

| Artefacto | SHA-256 |
| --- | --- |
| `dist/Naygo-0.5.2-portable.zip` | `7a3a7091b3b91062464895fe07a424e3939e0e65aece9578b11902bb295a9f52` |
| `dist/Naygo-0.5.2-setup.exe` | `876b82dc31893b69518dce8930ea623666bf4da017714afd0402f96cabfe7608` |
| `target/release/naygo.exe` | `346fa16e50cad51499a54e765d440929e127119a893736f3d0ea63de04e6cc1b` |

El setup reporta `NotSigned`. No se instaló ni se publicó esta build. La instancia instalada anterior
continúa abierta; su interfaz no se aceptó visualmente porque Windows está bloqueado.

## Pendiente

L-01B sigue abierto para ejecución nativa de la prueba Unix, pruebas de volúmenes/permisos/energía y
aceptación manual de Replace. W-07/W-13 conservan sus pruebas interactivas pendientes.
