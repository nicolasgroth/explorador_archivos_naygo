# Sincronización, bandeja, transformación de texto y preview 3D

Naygo — diseño aprobado el 16 de agosto de 2026.

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
SPDX-License-Identifier: MIT

## Objetivos

Agregar cuatro flujos de alto valor sin cambiar la prioridad del producto: navegación rápida,
bajo consumo, trabajo cancelable y cero I/O en el hilo de UI.

## 1. Asistente de sincronización

- Compara recursivamente las carpetas de dos paneles `Files` en un worker.
- Modos: izquierda a derecha, derecha a izquierda y bidireccional.
- El plan distingue copia nueva, actualización, conflicto y eliminación opcional de sobrantes.
- La eliminación queda desactivada por defecto y requiere confirmación explícita.
- Comparación rápida: tipo, tamaño y fecha modificada. El hash queda como verificación opcional
  posterior para casos ambiguos; no se calcula durante la navegación normal.
- El plan es cancelable, ignora enlaces de directorio para evitar ciclos y tolera entradas que
  desaparecen.
- La ejecución se traduce al motor de operaciones existente para reutilizar progreso, conflictos,
  cola y cancelación. El deshacer global queda fuera de esta primera versión porque una
  sincronización puede sobrescribir contenido en ambos sentidos y exige historial de bytes.

## 2. Bandeja temporal

- Nuevo panel `Basket` dentro del workspace.
- Guarda rutas absolutas deduplicadas, no copias de archivos ni handles abiertos.
- Se alimenta desde la selección del panel activo o mediante drag & drop.
- Permite quitar elementos, limpiar y usar su contenido como origen de copiar/mover/comprimir/
  eliminar.
- Los elementos inexistentes se marcan discretamente y pueden limpiarse.
- Es temporal y no se serializa en la sesión. Una futura “colección guardada” será otra feature.

## 3. Transformación de texto

- Detecta `CRLF`, `LF`, `CR` clásico y mezcla de finales de línea.
- Destinos: Windows (`CRLF`), Unix/macOS moderno (`LF`) y Mac clásico (`CR`).
- Codificaciones iniciales: preservar, UTF-8, UTF-8 con BOM, UTF-16 LE y UTF-16 BE.
- Opciones adicionales: asegurar/quitar salto final y eliminar espacio final.
- Lee el archivo por bloques, impone un límite defensivo de 128 MiB y normaliza fuera del hilo de
  UI. Usa archivo temporal vecino y reemplazo solo al completar; la cancelación nunca deja un
  parcial como archivo final. El respaldo transaccional se elimina tras publicar correctamente.
- Rechaza binarios y datos inválidos en vez de corromperlos silenciosamente.
- Antes de aplicar muestra diagnóstico, destino y número de archivos.

## 4. Preview STL/3MF

- Preview estático isométrico por CPU; no requiere GPU ni DLL externa.
- STL ASCII/binario y geometría base de 3MF (`3D/3dmodel.model` dentro del ZIP).
- El worker parsea, calcula límites y rasteriza a RGBA; Slint solo recibe la imagen final.
- Muestra dimensiones X/Y/Z y cantidad de triángulos. 3MF respeta la unidad declarada; STL se
  informa como unidad no especificada.
- Límites defensivos de bytes, triángulos y resolución; debounce y cancelación existentes.
- Primera versión: geometría y sombreado simple. Rotación continua, materiales avanzados,
  reparación de malla y laminado quedan fuera de alcance.

## Validación

- Tests puros para planificación, deduplicación, detección/conversión de texto y parsers/raster.
- Tests de controladores para rutas de UI y ausencia de I/O directo en callbacks.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test --workspace`.
- `graphify update .` y build de distribución con `scripts/build-release.ps1` al finalizar.
