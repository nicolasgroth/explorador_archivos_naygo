# Validación W-16 — iconos asociados por Windows

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

La configuración **Usar íconos del sistema para archivos** viene apagada y sólo aparece en Windows.
Cuando se activa, Naygo consulta el icono pequeño asociado a cada extensión mediante Windows Shell,
en un worker y con `SHGFI_USEFILEATTRIBUTES`; no abre ni inspecciona la ruta del archivo. Las
consultas se deduplican por extensión y la caché admite hasta 512 tipos distintos. Si la consulta no
entrega una imagen, continúa el icono ya usado por Naygo. Desactivar la opción vuelve al aspecto
habitual y descarta resultados pendientes.

La imagen se copia a píxeles RGBA y los handles GDI/Shell se liberan antes de devolver el resultado.
La UI sólo recibe imágenes ya preparadas. Agrupa los resultados hasta vaciar las consultas
pendientes y reconstruye las filas una vez por tanda, evitando refrescos repetidos en directorios
con muchos tipos. Los ajustes antiguos que no contienen el nuevo campo mantienen el valor `false`.

## Verificación

El gate completo `scripts/test-all.ps1` pasó en Windows:

- 1.438 pruebas aprobadas y 8 ignoradas.
- Clippy con `-D warnings`: limpio.
- `cargo fmt --all --check`: correcto.
- Incluye compilación y prueba Win32 del icono `.txt` de 16×16 px, longitud RGBA y canal alfa, y
  rechazo de sintaxis de ruta en el parámetro de extensión.

El build final de distribución se ejecutó solo mediante `scripts/build-release-guarded.ps1`: ThinLTO,
un job de Cargo y prioridad baja. Pico contable final del Job: 10,48 GiB, bajo el límite de
12 GiB. Rust release terminó en 27 min 04 s; Inno Setup compiló el instalador en 118,109 s.

## Artefactos locales

| Artefacto | Tamaño | SHA-256 | Estado |
|---|---:|---|---|
| `dist/Naygo-0.5.2-portable.zip` | 14.360.680 bytes | `493c1389b4e350b7d356a182845829987d9f6b41c7e1ac43ea3126f993062514` | ZIP válido; incluye licencia y notices, sin PDB |
| `dist/Naygo-0.5.2-setup.exe` | 66.291.303 bytes | `8f34fb0f420789c04672b826fd17efb3e835ec4018673ff7fc4c326a3877a876` | Inno Setup correcto; Authenticode `NotSigned` |
| EXE del ZIP e instalación | 43.330.560 bytes | `3ff790b76e8eb6095afa72047256c12813f9e646cbe8d7a1db657a2dca94a261` | Coinciden byte por byte; versión de producto 0.5.2 |

El setup actualizó in-place la instalación de usuario y terminó con código 0. Antes de instalar, Naygo
se cerró mediante `--shutdown`; se respaldaron los archivos de configuración en
`target/agent-out/install-backup-20261017-system-icons-batched/`. Los hashes pre/post coinciden:

| Archivo | SHA-256 conservado |
|---|---|
| `settings.json` | `5eae204a5c83bd856a8ad139a99b73bb15b25171e0072f62e3e13710553d1737` |
| `workspace.json` | `295c885de016d04a3de8936baf278de0896d3682f46d351345f94d3b997cf2b4` |
| `favorites.json` | `0cf87ca76089e1fecff18433f9697783f6751724b6008c693a0d3106ee6d52cf` |
| `templates.json` | `7557750482a01e650db278b14e55d56d656004a775b40b6e7e29d93bb1da43e0` |
| `naygo.pdb` | `eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07` |

La versión instalada arrancó como PID 39032, `Responding=True`, a las 08:55:02 del 17 de octubre; el
JSON conservado muestra `use_system_file_icons=false`, por lo que se mantiene apagado.
Esto verifica instalación y arranque; no constituye aceptación visual manual del nuevo control.
Logs del build final supervisado: `target/agent-out/guarded-20261017-082510-296.out.log` y `.err.log`.
