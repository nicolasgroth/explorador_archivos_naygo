# Naygo 0.5.2 — selección, creación y refresco

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance — 2026-09-10

- Selección y scroll a la carpeta hija al subir; contexto por ruta al refrescar.
- Creación confirmada → listado → selección del resultado, actualización de rama del árbol.
  Navegar antes de terminar no vuelve a la carpeta anterior ni roba el panel activo.
- TextEdit del modal restablecido al abrir/cerrar; conserva creación multilínea/anidada.
- Nombre de archivo pegado con segundos y puntos, sin cambiar plantillas ni hora UTC.
  Reserva atómica del nombre y conservación de los resultados de pegados simultáneos.
- Resaltado continuo del nombre; espacio sobrante sólo al final de la celda.
- Reconciliación de eventos durante listados, registro tardío del watcher, error/rescan y
  operaciones terminadas. Metadata faltante nunca se consulta en disco desde el tick.

Conserva la base RP-5A local y las preferencias existentes, sin cambio de esquema.
No se mezclan con esta entrega el modo económico ni los presupuestos globales de preview.

## Validación

Fuentes definitivas: **1.308 pruebas aprobadas**, cero fallos y seis ignoradas preexistentes
(875 core + 22 integración core + 54 platform + 344 UI + 13 integración/visual UI).
Clippy `--workspace --all-targets -- -D warnings` sin advertencias (4 min 18 s), formato y
diff correctos. Logs: `target/agent-out/nav052-verified-tests.log` y `nav052-clippy.log`.
La prueba visual inicialmente omitía fijar el
tamaño de la ventana headless y no producía píxeles; se corrigió el arnés, sin excluirla
ni debilitar sus aserciones. La ejecución directa corregida pasó y su captura se inspeccionó:
`target/agent-out/nav052-highlight.png`. El mismo test verifica reapertura del TextEdit.

Diez idiomas verificados; diff sin errores de espacios. Grafo actualizado por AST:
7.980 nodos, 14.521 aristas, 806 comunidades (`nav052-graphify-final.log`). Se conservan 29 avisos conocidos por
archivos JSON/otros sin nodos; no se ejecutó extracción semántica externa.

Las pruebas utilizan directorios temporales propios y datos sintéticos. No se modificó
ningún archivo personal, la instalación de Naygo ni la configuración del usuario.
No se realizó aceptación visual completa en la aplicación instalada/DPI/red real.

## Distribución

Build **0.5.2+build.202609100054** completado con `scripts/build-release.ps1`, una tarea
de compilación y sin otras tareas pesadas en paralelo. Rust release: 25 min 50 s; Inno: 84,125 s.
Instalador terminado el **2026-09-10 01:19:50 local**. Log: `target/agent-out/nav052-release.log`.
Distribución RP-5A respaldada con hashes cotejados en
`target/agent-out/baseline-051-before-nav052/`. No se ha instalado ni publicado a GitHub.

| Artefacto | Bytes | SHA-256 |
|---|---:|---|
| `dist/Naygo-0.5.2-setup.exe` | 52.633.801 | `cbc3509a37c43bb361789885ee57ee4ae351649976a3f0da7e13079e61883f88` |
| `dist/Naygo-0.5.2-portable.zip` | 13.282.574 | `e975989c9ac32cc9eaab52c8ede36082948651e0fd6628cb5475118c140755fd` |
| `target/release/naygo.exe` | 36.508.160 | `86ef07b43973d7e35982f4bca24597a205c30a6bcf2692149812cb575fc2658f` |
| `target/release/naygo.pdb` | 342.405.120 | `faa7cfda16511c7a062f70f0ac655515e445076588e71f0e5c212827a44a1e08` |

`SHA256SUMS.txt` cotejado con ambos paquetes. `7z t` aprobó el portable; las cuatro entradas
(EXE, LICENSE, LEEME y avisos de terceros) también coinciden por hash con sus originales.
Metadatos del EXE: versión 0.5.2, Nicolás Groth/ISGroth/email/MIT correctos. PDB opcional
en instalador, no incluido en portable. Instalador sin firma digital (sin certificado configurado),
como las entregas anteriores. No se extrajo ni instaló el setup. Portable: 11.382 bytes
adicionales frente a RP-5A, sin nuevas dependencias de producción.

Cuatro pruebas adicionales del lector con el EXE release aprobadas, cero fallos (8,93 s),
incluidos PNG/SVG/ZIP/PDF, entradas inválidas, pausa, finalización y cancelación.
Log: `target/agent-out/nav052-release-reader-tests.log`. La muestra privada se usó sólo
en lectura: 55 frames, 1.830 ms de decodificación/IPC sin esperas de presentación y cancelación
en 33 ms. No es duración de reproducción UI ni un benchmark de memoria global.

## Límites

Los filtros de visibilidad siguen vigentes; una carpeta oculta por ellos no se fuerza a
aparecer. En creación anidada el panel revela el primer nivel bajo su ruta actual; el árbol
mantiene como activa la carpeta abierta. Las carpetas sin soporte de vigilancia nativa
siguen necesitando refresco explícito para cambios externos posteriores. No se añadió
sondeo periódico de discos, indexación global ni dependencia nueva.
