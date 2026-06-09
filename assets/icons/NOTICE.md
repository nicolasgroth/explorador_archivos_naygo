# Atribución de íconos

Los íconos embebidos en Naygo provienen de proyectos de código abierto, usados bajo
sus licencias (todas permisivas, compatibles con la licencia MIT de Naygo):

- **Flat (set `flat`)** — Flat Color Icons (Icons8) —
  <https://github.com/icons8/flat-color-icons> — MIT.
- **Fluent (set `fluent`)** — Fluent UI Emoji (Microsoft) —
  <https://github.com/microsoft/fluentui-emoji> — MIT.
- **Mono (set `mono`)** — Lucide — <https://github.com/lucide-icons/lucide> — ISC.

Los PNG se rasterizaron desde los SVG originales a 32×32. Para el set Mono (Lucide,
trazo `currentColor`) se fijó un gris claro al rasterizar; la app aplica el tinte del
tema al pintar. Algunos íconos sin equivalente en un pack quedan como marcador de
posición generado (ver `crates/ui/src/bin/gen_icons.rs`).

Solo se versionan los PNG extraídos en `assets/icons/{flat,fluent,mono}/`. Los archivos
comprimidos originales (`*.zip`) NO se incluyen en el repositorio (ver `.gitignore`).
