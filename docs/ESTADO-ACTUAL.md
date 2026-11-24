# Naygo — estado actual

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Solo lo vigente. El historial de entregas, gates y hashes anteriores está en el
[estado histórico](evidencia/estado-historico-2026.md); lo pendiente, en el [backlog](BACKLOG.md).

## Versión

- **Publicada:** 0.5.6.
- **En preparación:** 0.5.7, en la rama `fix/integridad-build-0.5.7`, sin publicar. El cambio de
  versión y el encabezado `[0.5.7]` del CHANGELOG los crea Nicolás con `scripts/bump.ps1`; hasta
  entonces las novedades están en `## [Sin publicar]`.
- Plan del lote: [integridad, build y documentación](plans/2026-11-21-integridad-build-y-docs.md).

## Resumen del lote 0.5.7

1. Copiar y mover ya no atraviesan enlaces (junctions, symlinks) al planificar: no hay bucles ni
   borrado del contenido apuntado. Un enlace se mueve como enlace; entre volúmenes queda en el
   origen y el paso se informa como fallido.
2. Antes de sobrescribir o mover se comprueba la identidad real del archivo (mismo archivo por
   otra ruta, hard links, mayúsculas y minúsculas).
3. En Windows la copia usa `CopyFile2`: conserva fechas, atributos y flujos alternativos, y un
   mover entre volúmenes es durable antes de borrar el origen. Rutas de más de 260 caracteres.
4. Los placeholders de nube (OneDrive solo en línea) no se tratan como enlaces; la búsqueda por
   contenido y los puntos de comparación no los descargan. Restaurar desde la Papelera ya no pisa un destino que aparece
   durante la operación.
5. Una subcarpeta ilegible dentro de una copia o un movimiento se informa como fallo y no se borra
   su origen. Un recurso de red caído no bloquea los listados locales (cupos de 8 locales y
   4 remotos).
6. Abrir archivos, terminal y Explorador corre en un hilo propio: ya no bloquea la interfaz.
7. Menú contextual nativo completo ("Enviar a", "Abrir con", íconos de extensiones; implementado,
   pendiente de validación manual). "Nuevo" no aparece: va al backlog.
8. Rendimiento: modelos persistentes en la sincronización de la UI y `RowsModel` por tramos en
   listados grandes. Build: el código Slint generado vive en el crate `naygo-ui-gen` y el release
   usa `debug = "line-tables-only"`.
9. Textos que faltaban por traducir, en los 10 idiomas.

## Cifras del lote

Medidas en el PC de desarrollo; detalle y método en [BENCHMARKS.md](BENCHMARKS.md) (secciones
de compilación, "Memoria en ejecución — línea base", "T12" y "T13").

| Medida | Antes | Después |
|---|---|---|
| Pico de RAM del build release | 10,9 GiB | 6,7 GiB |
| Tamaño del PDB | 473 MB | 177 MB |
| Recompilar tras tocar un `.rs` de UI (dev) | 1 min 01 s | 18,9 s |
| `cargo check` tras tocar un `.rs` de UI | 41 s | 4,4 s |
| CPU en reposo por cada 20 s (ventana abierta) | 5,33 · 2,14 · 1,86 · 0 · 4,47 s | 0,66 · 0,72 · 0,02 · 0,02 · 0,64 s |
| CPU en 12 s tras un cambio en carpeta de 100.000 | 8–12 s | 0,22 s |
| Memoria tras ese evento (WS / privada) | 256 / 249 MiB | 176 / 157 MiB |
| Actualizar / agregar una fila con 100.000 | 28,5 / 22,3 ms | 3,80 / 4,14 ms |

Memoria en ejecución (release, WS / privada): bandeja 47/21 MiB; ventana en reposo 56–59 /
26–28 MiB; carpeta de 100.000 entradas ~185/167 MiB (≈1,4 KiB por fila). Sin crecimiento en
reposo.

Salvedades: tocar un `.slint` en dev quedó más lento (1 min 02 s → 1 min 27 s). El build release
completo tardó ~10 min más que la línea base anterior, sin causa confirmada (la línea base es de
otro commit). El pico de 10,9 GiB es la cifra del build supervisado de 0.5.6, probablemente
medida como compromiso del Job y no como working set.

## Gate

`scripts/test-all.ps1` sobre el commit `d68cee5e`, 2026-11-24: 1790 tests pasaron, 0 fallaron y 12
quedaron ignorados; clippy sin warnings y formato correcto. `scripts/check_i18n_lang.py` dio OK en
los nueve idiomas distintos del español (`en`, `fr`, `de`, `it`, `pt`, `ja`, `ko`, `zh`, `hi`).

## Distribución

Distribución local de avance, generada el 2026-11-24 con `scripts/build-release-guarded.ps1` desde
el commit `d68cee5e`. El binario se identifica como `0.5.6+build.202611240518`: la versión pasa a
0.5.7 cuando el autor ejecute `scripts/bump.ps1`. Los artefactos no llevan firma Authenticode.

| Artefacto | Tamaño | SHA-256 |
|---|---|---|
| `dist/Naygo-0.5.6-portable.zip` | 14,3 MiB | `db3e6d337c68590630572c97d1cbcfcf4a234ca0409d195fd3cf698c5ab78b9c` |
| `dist/Naygo-0.5.6-setup.exe` | 32,6 MiB | `0b444cbb931cb7a2fd146139a0c6c936540d4f53af43a62ff6f967a4a0c8c0db` |

El build release tardó 21 min 02 s con un pico contable del Job de 6,14 GiB. El instalador bajó de
65,8 MiB a 32,6 MiB porque incluye el PDB como componente opcional de símbolos, y el PDB bajó de
473 MB a 177 MB con `debug = "line-tables-only"`.

Sin validar en vivo todavía (ver `BACKLOG.md`, sección 1): abrir archivos a través del hilo del
Shell, el menú contextual nativo y la copia de un archivo descargado a un USB FAT32/exFAT.

## Medición final y decisiones

La medición de memoria y CPU sobre este mismo binario está en `BENCHMARKS.md`, sección "Cierre del
lote — medición final": no hay regresión respecto de las mediciones intermedias del lote. Las
decisiones que se tomaron sin consultar al autor, con su motivo y su costo si resultan
equivocadas, están en [`evidencia/decisiones-lote-0.5.7.md`](evidencia/decisiones-lote-0.5.7.md).
