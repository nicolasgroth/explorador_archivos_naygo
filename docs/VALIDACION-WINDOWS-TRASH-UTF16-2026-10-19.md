# Papelera Windows: identidad UTF-16 y recibos reconstruidos

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-19. Corrección local; empaquetada en 0.5.3, no instalada, firmada ni publicada.

## Hallazgo y corrección

La Papelera empareja callbacks de `IFileOperation`, orígenes, fallos parciales y recibos usando
`windows_paths_equal`. La implementación anterior convertía cada ruta con `to_string_lossy()` y
comparaba el texto ignorando mayúsculas. Dos rutas distintas que contengan unidades UTF-16
inválidas podían convertirse al mismo U+FFFD y asociar un resultado de Papelera con el origen
equivocado.

Una prueba de regresión Windows construye rutas con unidades sustitutas aisladas diferentes.
**Antes del fix falló** porque ambas rutas se consideraban iguales; tras el cambio pasa usando
`CompareStringOrdinal` sobre slices UTF-16, sin traducción a UTF-8. Se conserva la equivalencia
insensible a caja y se normalizan `/` y `\\` antes de comparar. La identidad exacta y las reglas
normales también tienen cobertura.

En la misma revisión encontré otro caso dentro de la ruta de respaldo de Deshacer: el Shell puede
devolver `DeletedFrom` con mayúsculas o separadores distintos a los de la ruta guardada. El
emparejamiento estricto de `Path` fallaba aunque la carpeta fuera equivalente en Windows. El
respaldo ahora compara carpeta y nombre mediante la misma equivalencia ordinal UTF-16; una prueba
confirma que acepta mayúsculas y separadores alternativos y rechaza carpeta o nombre incorrectos.

## Validación

- Reproducción previa: `cargo test -p naygo-platform --locked comparacion_de_rutas_no_colapsa_utf16_invalido_distinto -- --nocapture` falló en la aserción esperada.
- Tras el fix, la misma prueba pasó: **1 aprobada**.
- Las pruebas de paridad de caja y separadores se añadieron para cubrir el comportamiento existente.
- `cargo test -p naygo-platform --locked trash::tests -- --nocapture`: **11/11 aprobadas**.
- Gate `scripts/test-all.ps1`: **1.483 tests aprobados, 8 ignorados**, Clippy sin warnings y formato correcto.
- Los nueve catálogos de idioma (`de`, `en`, `fr`, `hi`, `it`, `ja`, `ko`, `pt`, `zh`) pasan.
- `graphify update .`: 9.593 nodos, 18.202 relaciones y 897 comunidades. El grafo advierte que cambió la agrupación/etiquetado comunitario; no se regeneraron etiquetas LLM.
- `scripts/build-release-guarded.ps1`: Release **51m07s**, Inno Setup **199,000 s**, pico Job
  **10,66 GiB** (límite 12 GiB). La ejecución terminó correctamente.
- ZIP verificado con 7-Zip: cuatro entradas (`LEEME.txt`, `LICENSE`, `naygo.exe`,
  `THIRD-PARTY-NOTICES.md`), sin PDB. El hash del ejecutable extraído del ZIP coincide con Release.
- Los dos binarios resultaron `NotSigned`; no se solicitaron credenciales ni se publicó nada.

| Artefacto | SHA-256 |
| --- | --- |
| `dist/Naygo-0.5.3-portable.zip` | `03755670fade2a1cc6adc36a008a916c222915d1f4550aeeb9f108372422d679` |
| `dist/Naygo-0.5.3-setup.exe` | `fa6fa2cb3f7542243a9076d060466cc0c7cc3c707a506cc0fa2233ff2651fd00` |
| `target/release/naygo.exe` | `cb8f28efcd9619463a5977ba6fb771e59656352bb4a49dfe76bb9fb3c0682229` |

La build inicialmente no se instaló. En una continuación autorizada, el 2026-10-20 se ejecutó
`dist/Naygo-0.5.3-setup.exe` en modo silencioso; terminó con código 0. Antes se guardó una copia de
los cuatro JSON y el PDB en
`target/agent-out/install-preserve-053-20261020-wtrash/`. Los cinco SHA-256 se mantuvieron idénticos,
los cuatro JSON parsean y el ejecutable instalado coincide con Release:
`cb8f28efcd9619463a5977ba6fb771e59656352bb4a49dfe76bb9fb3c0682229`. Naygo se inició tras la
actualización y quedó abierto/respondiente como PID 31304, ProductVersion 0.5.3. El setup y el
portable siguen sin firma y no se publicaron. La sesión Windows permanece bloqueada, así que no se
afirma aceptación visual ni UI Automation.
- Los tests son Windows-only y se ejecutaron en este host Windows; todavía falta el run remoto `windows-2025` requerido por W-12.

## Límites

El cambio sólo corrige identidad/equivalencia de rutas en la asociación de resultados de Papelera;
no reemplaza la aceptación de Papelera en Windows Server 2025, el test manual con Papelera real, ni
la verificación remota del workflow. La firma y publicación permanecen pendientes y no forman parte
de esta corrección.
