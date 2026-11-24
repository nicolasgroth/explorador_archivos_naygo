# Naygo 0.5.2 — modo económico y retención del preview (RP-5B)

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

- Booleano `preview_economical` aditivo en esquema 4: ausente = false. Persiste desde
  Previsualización, participa en su búsqueda y se propaga a lectores internos.
- Normal conserva estáticos/SVG hasta 2048 px, animación 768 px / 15 presentaciones/s
  y raster 3D 800 × 600. Económico: 1024 px, 512 px / 10/s y 512 × 384 respectivamente.
  No cambia proporciones, composición, reglas por extensión ni modo de ajuste.
- Buffer Slint único por resultado compartido por todos los VM de imagen. Sin copia
  RGBA nueva por tick/listado. Un nuevo frame sí requiere su propia imagen.
- 64 MiB compartidos para retención: reservas conservadoras de imagen/texto y capacidades
  de escenas. Escena individual hasta 48 MiB; el permiso acompaña a su último Arc.
  Error de admisión terminal/reintento; cambiar calidad invalida/cancela la generación.

## Límites del alcance

No es un límite de RSS global. No incluye temporales de parsers/decoders, gramáticas,
buffers en tránsito ni copias internas del renderer durante reemplazo. Los dos lectores
aislados mantienen su presupuesto separado de 192 MiB cada uno; los retirados siguen
ocupando plaza. La carga/parseo 3D ocurre antes de admitir la escena retenida.

Siguen pendientes: I/O bloqueado de texto/3D, presupuestos de trabajo temporal completos,
mediciones comparables de consumo total, RP-6 (PDF/TAR/texto/SVG/métricas) y aceptación
instalada del toggle, múltiples paneles, escalas de pantalla, Narrador y red.
No se instala sobre la sesión personal ni se publica a GitHub en esta entrega.

## Validación y distribución

- Suite definitiva: **1.317 aprobadas, cero fallidas, seis ignoradas preexistentes**.
  Desglose: core 878 + integración core 22 + platform 54 + UI 349 + integración/UI 14.
  Registro: `target/agent-out/economy-verified-tests.log`. Primera pasada también aprobada:
  `economy-tests.log`; la segunda verifica el estado formateado final.
- Clippy workspace/all-targets con `-D warnings`: limpio (7m08s), `economy-clippy.log`.
- Diez idiomas verificados; formato/diff correctos. Sin dependencias nuevas.
- Respaldo de setup/portable/checksums previo a reemplazo, comparado por SHA-256:
  `target/agent-out/baseline-052-before-rp5b/`.

- Grafo AST actualizado sin llamadas semánticas: 8.025 nodos, 14.609 relaciones,
  807 comunidades. 29 avisos conocidos de fuentes sin nodos; `economy-graphify.log`.
- Distribución **0.5.2+build.202609100757**, generada el 10 de septiembre a las
  **08:26:17 America/Santiago**. Release en 28m58s, Inno en 95,110 s; build ejecutado
  solo con un job. Registro: `economy-release.log`.
- SHA256SUMS coincide; prueba 7-Zip correcta y cuatro entradas del portable comparadas
  por hash con sus fuentes: EXE, LICENSE, LEEME y THIRD-PARTY-NOTICES.
  Metadatos EXE 0.5.2, autoría/empresa/MIT correctos. PDB generado, opcional en setup,
  no incluido en ZIP. Instalador sin firma (no hay certificado configurado).
- Cinco pruebas del lector repetidas contra el EXE release: **5 aprobadas**, 7,51 s;
  `economy-release-readers.log`. Incluyen PNG/SVG económicos, GIF económico animado,
  WebP/GIF/APNG normales, pausa, fin/cancelación, PDF/comprimidos y entradas inválidas.
  Son muestras sintéticas: no certifican consumo global ni toda la matriz de codecs.
- Portable +17.648 bytes respecto de la entrega 0.5.2 anterior. Sin instalación ni
  publicación remota; sólo se reemplazaron paquetes generados, con respaldo recuperable.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| Instalador | 52703896 | `c56c8609d0fdef860621e8117320678a54bd1aa922a3a051d63d813ac45ca0c1` |
| Portable | 13300222 | `9f73a0dc0748cc99dc50624facc5b5e2476f3eb16122e770252963bad99ac18b` |
| EXE | 36567040 | `0211c9201c92b9b0fb6d0133d25d5d27f46b648ef49de2e57769f81b7427b90f` |
| PDB | 342831104 | `c4648b8f8b188053bf2d8b610a61070fff08fa3a7bb771a37282c96e2183acde` |
