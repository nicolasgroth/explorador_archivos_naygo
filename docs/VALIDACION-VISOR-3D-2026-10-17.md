# Validación del visor STL/3MF y distribución Windows — 2026-10-17

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Cambios

- La órbita usa una orientación completa de cámara: arrastrar en horizontal rota sobre el eje
  vertical visible y arrastrar en vertical rota sobre el eje derecho visible, incluso después de
  inclinar la figura. La cámara se compone sin reconstruir ángulos Euler, y conserva el zoom y el
  botón de restaurar.
- El rasterizador actualiza los pesos de cada triángulo por incrementos horizontales y verticales;
  evita tres divisiones por píxel. El benchmark sintético release bajó de 29,771 ms a una mediana de
  20,846 ms para 96.800 triángulos a 1024×768 (-30,0 % en este equipo). El método y sus límites están
  en [BENCHMARKS](BENCHMARKS.md).
- Se conservan carga aislada, geometría retenida, cancelación y coalescencia de gestos.
- El visor actual de geometría usa un solo color celeste con sombreado por normales. El lector 3MF
  presenta primero una miniatura incrustada cuando existe; el primer gesto intenta cargar el modelo
  geométrico y sustituye la imagen si el parser puede leerlo. No existe aún un selector para volver
  a la miniatura ni se aplican los colores/materiales 3MF a la geometría.
- El control general de toolbar para «solo íconos» ya permitía mostrar icono y etiqueta, y los
  botones que abren acciones secundarias muestran su chevrón; no se añadió una preferencia redundante.

## Verificación

- `cargo test -p naygo-core mesh_preview::tests --locked`: 13 aprobados; incluye composición de
  órbita en ejes de cámara tras inclinar el modelo y equivalencia de cobertura/píxeles del raster.
- Prueba focal de ciclo de vida Slint aprobada: gestos coalescentes, carga, cancelación y estado.
- Gate `scripts/test-all.ps1`: 1.440 pruebas aprobadas, 8 ignoradas, Clippy sin avisos y formato OK.
- Los diez catálogos de idioma pasaron `scripts/check_i18n_lang.py` (sin cambios de claves en esta
  actualización).
- `graphify update .` completó: 9.032 nodos, 17.135 enlaces, 843 comunidades. Reportó que algunos
  JSON no producen nodos AST, sin afectar el código ni las validaciones de idioma.

## Distribución e instalación local

- Versión: 0.5.2, portable ZIP y setup regenerados por `scripts/build-release-guarded.ps1`.
- Build release final: 28m05s; pico supervisado: 10,48 GiB (límite 12 GiB, margen mínimo 8 GiB).
- SHA-256 portable: `7167ffd95b9212ecff607a899e4844eb4921fd83adf69d991e758daae829f1ac`.
- SHA-256 setup: `708617684b8a1d3f9a4d3e06e9bd315d60cd4cac7788be5c1042be547202a5d8`.
- SHA-256 `naygo.exe`: `75ee9ac520f7fa53fd0639cbbc56de93eebdc66e270d4b0ccdc4979f62a9e542`; coincide con el
  ejecutable contenido en el ZIP y con el instalado.
- El instalador no se firmó con Authenticode.
- PID instalado y abierto: 16368; Windows informa que responde.
- Los SHA-256 de `settings.json`, `workspace.json`, `favorites.json`, `templates.json` y del PDB
  antiguo coinciden con el respaldo previo. Directorio: `%LOCALAPPDATA%\Naygo-backups\pre-final-camera-orbit-20261017-140859`.

## Pendiente de aceptación

Falta que Nicolás pruebe con un STL/3MF real si la órbita se siente natural. La reducción de raster
se midió con una superficie sintética; comprobar modelos reales y la presentación Winit permanece
dentro de W-07. Los controles de color y la selección explícita entre miniatura JPG y geometría son
propuestas de producto, todavía sin implementar ni sumar al backlog.
