# L-09A — formatos copy/cut de escritorio

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-20. Segundo subpaso puro de core; todavía no integra el portapapeles del SO.

## Cambio

`naygo_core::clipboard::desktop_formats` añade:

- Nautilus `x-special/gnome-copied-files`: palabra literal `copy` o `cut`, seguida de LF y una
  URI `file:` por línea, sin LF final. El decoder reutiliza la validación de `text/uri-list`,
  rechaza acción desconocida y lista vacía, y devuelve explícitamente el indicador de corte.
- Dolphin `application/x-kde-cutselection`: `0` significa copia y `1` corte; payloads con otros
  bytes o longitud se rechazan. El codec de URI limita entradas a 16 MiB, 100.000 elementos y
  64 KiB por ruta antes de acumular listas no confiables.

La serialización GNOME concuerda con la implementación actual de Nautilus, que serializa el verbo
`cut`/`copy`, luego LF y las URIs, sin añadir terminador: [fuente de Nautilus](https://github.com/GNOME/nautilus/blob/main/src/nautilus-clipboard.c#L890-L918).
La convención KDE se registró previamente en la auditoría de portabilidad del proyecto
([auditoría Linux](auditoria/2026-07-01-portabilidad-linux.md)); falta probarla contra Dolphin real.

## Validación

- `cargo test -p naygo-core clipboard:: --locked`: **34 aprobadas**, 0 fallidas.
- `cargo check -p naygo-core --tests --target x86_64-unknown-linux-gnu --locked`: aprobado.
- `cargo clippy -p naygo-core --all-targets --target x86_64-unknown-linux-gnu --locked -- -D warnings`: aprobado.
- `cargo fmt --all -- --check`: aprobado tras aplicar formato.
- Las pruebas rechazan entradas que exceden tamaño total, cantidad de rutas o longitud individual.
- `graphify update .`: aprobado; grafo refrescado a **9.646 nodos, 18.286 relaciones y 896
  comunidades**. Graphify reportó 29 archivos de datos/i18n sin nodos AST y recomendó actualizar
  etiquetas semánticas de comunidad; el grafo se regeneró correctamente.

No hay runtime Linux nativo disponible en este equipo (WSL detenido por
`HCS_E_HYPER_V_NOT_INSTALLED`). No se verificó interoperabilidad con Nautilus/Dolphin ni propiedad
del clipboard al cerrar Naygo. No se integró controlador; no hay función visible y no se requiere
regenerar `dist/`. L-09A sigue abierto; L-09B no se inicia.
