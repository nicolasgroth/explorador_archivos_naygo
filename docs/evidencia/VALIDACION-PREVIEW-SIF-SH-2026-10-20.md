# Validación de vista previa `.sif` y `.sh` — 2026-10-20

## Hallazgo

La captura de Naygo 0.5.3 en otro equipo muestra `.sif` configurado en modo «Automático» y
`.sh` en modo «Ver como texto». Ambos aparecen habilitados. `.sif` no se veía porque faltaba
en la lista de extensiones que Auto clasifica como texto. Para `.sh`, el panel muestra
«No se pudo leer» en una ruta de la unidad `Z:` en otro equipo.

## Cambio aplicado en el checkout

- Se añadió `.sif` a las extensiones de texto reconocidas por el modo automático.
- Se incrementó la versión interna de configuración a 5. Al migrar una configuración v4,
  Naygo agrega la regla automática de `.sif` y conserva las reglas personales, incluida la
  elección «Ver como texto» para `.sh`.
- Se añadieron pruebas para la clasificación automática de `.sif`, la migración v4 y la lectura
  de un fixture `.sh` local.
- Se actualizó la guía, el changelog, el backlog y el estado actual.

## Interpretación del error `.sh`

`.sh` ya estaba incluido en las extensiones de texto y el mensaje «No se pudo leer» se produce
cuando el lector no puede abrir o leer el origen. La prueba local de un `.sh` confirma que el modo
texto funciona en este checkout. No tenemos acceso a la unidad `Z:` ni al equipo de la captura,
así que no se puede determinar si el error viene de conectividad, permisos, disponibilidad de la
ruta u otra condición del origen. El mensaje por sí solo no permite distinguirlas.

La siguiente comprobación útil en el equipo afectado es copiar el mismo archivo a una ruta local
(por ejemplo, el Escritorio) y probarlo allí. Si funciona localmente, el problema queda acotado a
la ruta/unidad original. Si falla también localmente, se necesita el log de Naygo de esa sesión
para identificar el error del sistema operativo.

## Verificación

- `scripts/test-all.ps1`: **1.503 pruebas aprobadas, 8 ignoradas**; Clippy y formato pasaron.
- `graphify update .`: completado.
- `git diff --check`: pasó.
- No se generó una nueva distribución. `scripts/build-release-guarded.ps1` detuvo el build
  cuando la memoria libre cayó a 7,91 GiB, por debajo del mínimo de 8 GiB. No quedaron procesos
  de compilación activos ni un directorio `dist/portable-stage` parcial.
- Los ZIP y setup 0.5.3 anteriores se conservaron intactos. Sus SHA-256 siguen siendo:
  - Portable ZIP: `1b8212b02d18e71eb07f6d33e24c92fc63344615e44955d2d11ecb21518b4453`
  - Setup: `998c30daa7422d61e4187603462b575be7641549f189361c3658fd5771333000`

La corrección está en el árbol de trabajo y ha pasado el gate, pero requiere regenerar la
distribución 0.5.3 cuando haya margen de memoria suficiente. No se instaló ni publicó ningún
artefacto nuevo.
