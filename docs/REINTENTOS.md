# Revisar y reintentar archivos fallidos

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

En Operaciones, «Solo fallidos» filtra únicamente el historial: la cola y las operaciones
activas siguen visibles. Toda operación con fallos ofrece detalle, incluso si no completó
ningún archivo. Allí puedes filtrar los elementos fallidos y leer el motivo.

Para una copia o movimiento con archivos fallidos conocidos, «Revisar fallidos…» abre un
asistente con tipo, cantidad elegible/total fallido y cada par de rutas origen/destino. La
preparación sólo lee metadata en un worker; no copia, mueve ni crea carpetas. Confirmar vuelve
a comprobar tamaño, fecha de modificación y rutas físicas. Si algo cambió, debes cerrar y
revisar otra vez. El motor pregunta por conflictos, respeta la cola y permite cancelar.

Límites deliberados:

- Sólo archivos regulares de una copia/movimiento con request original y origen conocido.
  No se repiten pasos hechos/saltados, directorios completos ni borrados/ZIP/renombrados.
- Conserva cada destino exacto; no reconstruye la selección original ni reenumera árboles.
- Si un origen no existe, no es regular o no puede leerse, la preparación se rechaza.
- Máximo 10.000 archivos por revisión. No escribe historial adicional en disco.
- El reintento usa el motor de planes exactos, con journal pero sin undo. Se advierte antes
  de confirmar: borrar una copia sobrescrita no recuperaría el contenido anterior.
- No se retiran carpetas vacías del origen tras un reintento de movimiento de archivos.
- Las revisiones no son un bloqueo del filesystem ni una garantía frente a cambios
  concurrentes posteriores a la última comprobación; los errores del motor se muestran.
- Las operaciones sin request recuperable (incluido el propio plan de reintento) mantienen
  su detalle de errores, pero no ofrecen otro reintento automático.

No se añadió indexación, dependencia nueva, temporizador residente ni I/O en el render.
