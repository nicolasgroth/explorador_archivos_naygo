# Atribución de íconos

Los íconos embebidos en Naygo provienen de proyectos de código abierto, usados bajo
sus licencias (todas permisivas, compatibles con la licencia MIT de Naygo):

- **Lucide (set `lucide`)** — Lucide — <https://github.com/lucide-icons/lucide> — ISC.
- **Tabler (set `tabler`)** — Tabler Icons — <https://github.com/tabler/tabler-icons> — MIT.
- **Material (set `material`)** — Material Design Icons (Google) —
  <https://github.com/google/material-design-icons> — Apache 2.0.
- **Flat Color (set `flat-color`)** — Flat Color Icons (Icons8) —
  <https://github.com/icons8/flat-color-icons> — MIT.
- **Fluent (set `fluent`)** — Fluent UI Emoji (Microsoft) —
  <https://github.com/microsoft/fluentui-emoji> — MIT.
- **Mono (set `mono`)** — Lucide — <https://github.com/lucide-icons/lucide> — ISC.

Los PNG se rasterizaron desde los SVG originales a 48×48 con la herramienta
`crates/core/src/bin/gen_icons.rs` (feature `gen-icons`). Los sets `lucide`, `tabler`,
`material` y `mono` se rasterizan en blanco/gris y la app los tiñe con el color del
tema al pintar; `flat-color` y `fluent` conservan su color original.

Solo se versionan los PNG extraídos en `assets/icons/<set>/`. Los archivos comprimidos
originales (`*.zip`) NO se incluyen en el repositorio (ver `.gitignore`).
