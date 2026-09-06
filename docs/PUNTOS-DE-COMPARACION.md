# Puntos de comparación

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Disponible desde **Disposiciones → Puntos de comparación** o buscando ese nombre en **Ctrl+P**.
Un punto `.naygopoint` conserva metadatos de una carpeta en un momento determinado. No contiene
los archivos y **no es un respaldo**: no restaura contenido ni archivos eliminados.

## Capturar y comparar

1. Indica una raíz absoluta, si incluirás subcarpetas y las exclusiones relativas, una por línea.
   Por ejemplo, `tmp` excluye esa carpeta y todo su contenido; `build/log.txt` excluye ese archivo.
   No se admiten comodines ni `..`. La captura incluye archivos ocultos y de sistema accesibles;
   no hereda los filtros visuales del panel. Los enlaces/junctions descubiertos no se recorren.
2. Opcionalmente activa **SHA-256**: lee cada archivo para verificar contenido, por lo que consume
   más I/O que comparar metadatos. No activa servicios residentes, cachés globales ni indexadores.
3. **Capturar y guardar…** pide dónde crear el punto. Elige preferentemente una ubicación fuera
   de la raíz. Si está dentro de ella, Naygo añade su ruta a las exclusiones del punto.
   Nunca reemplaza un documento existente: elige otro nombre.
4. Puedes cerrar Naygo y **Abrir punto…** después. Abrir sólo carga el documento, no examina
   las carpetas ni ejecuta operaciones. Las opciones del punto abierto son de sólo lectura.
5. **Comparar ahora** examina explícitamente el mismo alcance. **Nuevo punto** permite cambiarlo.
   Cerrar/cancelar solicita detener el worker; una captura cancelada no publica un punto parcial.

## Interpretar resultados

| Resultado | Qué significa | Enviar a bandeja |
|---|---|---|
| Agregado | Presente ahora y ausente en una zona examinada del punto anterior | Sólo archivos regulares |
| Modificado | Cambió tipo, tamaño, modificación o SHA-256, según el alcance | Sólo archivos regulares presentes |
| Ausente | Estaba en el punto y ahora falta en una zona examinada | Nunca; sólo informativo |
| Desconocido | Faltan datos, hay enlaces o no se pudo examinar esa zona | Nunca |
| Mismos metadatos | Sin diferencias observadas; no demuestra igualdad de contenido | No aparece en la lista de cambios |
| Mismo contenido SHA-256 | Metadatos coincidentes y hashes coincidentes | No aparece en la lista de cambios |

La modificación de la fecha de una carpeta no se presenta como cambio por sí sola: sus archivos
se comparan individualmente. No se infieren renombrados ni se detectan cambios de permisos/ACL,
streams alternativos, atributos o metadatos que el formato no captura.

Marca archivos con las casillas (teclado: **Tab / Espacio**) y usa **Añadir selección a bandeja**.
Desde la bandeja puedes preparar una entrega. No se agregan directorios completos, evitando incluir
descendientes iguales o excluidos. Los archivos pueden desaparecer después de comparar: el motor
de operaciones revalida sus fuentes, y la comparación no congela ni respalda su contenido.

## Cobertura, costo y cancelación

- Hasta **50.000 entradas**, documento de **32 MiB**, presupuesto de rutas de **8 MiB** y
  **64 exclusiones**. El recorrido usa un margen conservador de rutas para mantenerse acotado.
- Al llegar al límite, el punto queda **parcial**, no completo. Una zona inaccesible o un error de
  lectura también se registra como desconocido; nunca demuestra que se hayan eliminado sus archivos.
- Se muestran como máximo **5.000 cambios** seleccionables/informativos. Los contadores incluyen
  los resultados restantes. Reduce el alcance mediante otro punto para trabajar con ellos.
- Captura, SHA-256, lectura, comparación, exportación y papelera ocurren en workers. No hay I/O
  del recorrido en el hilo de UI. Cancelación comprobada entre entradas y bloques de hash (64 KiB).
  Una llamada bloqueada por el sistema, especialmente en red, puede tardar en devolver el control.
- No es una transacción del filesystem: otros programas pueden cambiar archivos durante el
  recorrido. El hash verifica tamaño/fecha al terminar y no se da por válido si detecta cambios,
  pero no protege contra escrituras simultáneas que restauren exactamente esos metadatos.
- Raíces/alcances incompatibles se rechazan. Si una raíz es inaccesible, la cobertura es parcial.

## Exportar y eliminar puntos

**Exportar copia…** conserva exactamente el punto cargado; no captura de nuevo ni cambia el original
activo. Exportar otra copia dentro de la carpeta examinada puede aparecer como archivo agregado:
elige un destino externo o una carpeta excluida.

**Enviar punto a papelera** pide confirmación y comprueba que el documento abierto no haya cambiado.
Sólo retira ese `.naygopoint`, nunca los archivos que enumera. Si la papelera no está disponible,
se comunica el error y no se recurre al borrado permanente. Una cancelación posterior a la publicación
o retirada ya completada no revierte ese resultado.

Los documentos incluyen rutas, tamaños, fechas y hashes opcionales; revisa su privacidad antes de
compartirlos. No se suben a ningún servicio. No hay catálogo ni apertura automática al iniciar.

## Validación instalada pendiente

Probar con el instalador: diálogo a 100/150/200 % de DPI, Tab/Espacio y Narrador; carpeta sin
permisos y unidad de red realmente desconectada; cancelación de un hash grande; papelera de un
punto temporal. Las pruebas automáticas usan fixtures temporales, cobertura parcial simulada y
errores de lectura reproducibles. Los textos nuevos tienen ES/EN; los otros ocho catálogos incluyen
respaldo EN y requieren traducción editorial.
