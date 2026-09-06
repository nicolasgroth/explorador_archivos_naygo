# Preparar una entrega en Naygo

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Selección y bandeja

Reúne referencias desde diferentes carpetas con la bandeja temporal. Ctrl+clic alterna una
marca, Shift extiende un rango y Ctrl+A marca todo. Las flechas cambian foco y preview;
Ctrl+flecha conserva las marcas. Puedes arrastrar el grupo marcado a un panel abierto.

Copiar, mover, borrar y preparar entregas usan **solo los marcados**. El encabezado muestra
marcados/total. «Quitar» retira referencias; «Vaciar» retira toda la bandeja. Ninguna de estas
dos acciones borra originales. El botón de Papelera sí opera sobre originales y pide confirmación.
Guardar `.naygolist` conserva toda la bandeja como una lista estática, no como una búsqueda.

## Carpeta nueva o ZIP

1. Marca las fuentes y pulsa **Preparar entrega**, o busca ese comando en Ctrl+P desde un
   panel de archivos. Abrir el asistente congela el conjunto de fuentes elegido.
2. Elige la ruta completa de un destino **nuevo**. El selector «…» permite elegir su carpeta
   contenedora; puedes editar el nombre. Activa ZIP si corresponde.
3. Elige la estructura:
   - **Grupos por origen:** cada fuente recibe un prefijo numerado, conserva sus descendientes
     y las carpetas vacías. Permite reunir homónimos de distintas carpetas. Puedes editar los
     nombres, uno por línea y en el orden numerado de las fuentes; en archivos conserva la extensión.
   - **Archivos al mismo nivel:** reúne archivos sin subcarpetas. Rechaza nombres repetidos,
     incluso si solo difieren en mayúsculas; no representa carpetas vacías. La revisión muestra
     conjuntamente los homónimos. Activar «Proponer nombres únicos» propone sufijos `(2)`, `(3)`,
     etc. sin descartar archivos ni renombrar originales; exige revisar los nombres antes de publicar.
   - **Conservar rutas relativas:** requiere una raíz común elegida expresamente. Ningún origen
     puede quedar fuera de ella. No se infiere una raíz entre discos distintos.
4. Opcionalmente activa **SHA-256**. Añade lecturas de los originales y de las copias; permanece
   desactivado por defecto para no penalizar las entregas normales.
5. Pulsa **Revisar contenido**. Comprueba origen, nombre relativo, tamaño y destino. Cambiar
   cualquier opción invalida la revisión anterior. Se muestran hasta 500 entradas, con un
   indicador si hay más; el inventario se limita a 50.000 entradas para acotar memoria.
6. Pulsa **Publicar entrega**. Aquí «publicar» significa crear el destino elegido: Naygo no sube
   contenido a un servicio. El progreso y la cancelación quedan en **Operaciones**. En modo cola,
   el botón espera a que no haya otras operaciones en curso.
7. El resultado aparece sin robar el foco. **Abrir carpeta de salida** abre un panel nuevo (para
   ZIP, abre su carpeta contenedora); **Copiar ubicación** copia la ruta completa de la carpeta o
   ZIP. **Volver a fuentes en bandeja** recupera las referencias y sus marcas, sin quitar las
   referencias que ya estaban allí. Si falló o se canceló, no se habilitan acciones de salida.

## Garantías y límites

- Los originales no se mueven ni se borran. El contenido se prepara en un temporal propio,
  junto al destino, y se publica después de completar todos los pasos. En Windows, un destino
  que apareció desde la revisión causa un error; nunca se sobrescribe silenciosamente.
- Cancelar o fallar retira el temporal propio. Si el sistema impide limpiarlo, se registra su
  ruta en el log. Cerrar el asistente antes de publicar cancela su preparación.
- El inventario revisado no incorpora archivos que aparezcan más tarde. Se comprueban tamaño,
  modificación y tipo antes de ejecutar; SHA-256, si se pidió, también comprueba contenido.
- No se siguen enlaces simbólicos ni junctions encontrados al recorrer las fuentes.
- El ZIP usa el inventario congelado, no un segundo recorrido que pueda omitir archivos sin avisar.
- El destino incluye `naygo-manifest.json` y `naygo-files.txt`. El manifiesto contiene rutas
  relativas, tamaños y resultados; SHA-256 se agrega cuando se activó. No incluye rutas privadas
  absolutas de los originales. Esos nombres de manifiesto están reservados.
- Deshacer retira únicamente la carpeta o ZIP creado. No es un respaldo de los originales.
- Los homónimos se revisan conjuntamente (hasta 500 filas visibles, con indicación si hay más).
  Los nombres de grupo se corrigen manualmente; en plano existe propuesta numerada explícita.
  Otros errores de lectura/ruta se informan al encontrarlos y no permiten publicar. No hay
  sobreescritura, mezcla automática de carpetas ni omisión de archivos como resolución de conflictos.
- La tarjeta corresponde al último resultado y se puede cerrar. El historial de operaciones
  conserva el estado y los errores por destino; una entrega fallida no se presenta como «hecho: 0».

## Validación manual del instalador

Con archivos de prueba: seleccionar varios, cambiar de panel y volver; arrastrar el grupo;
quitar referencias y comprobar que los originales siguen presentes. Probar una entrega con
homónimos en modo plano y en grupos; comprobar manifiesto y ZIP; cancelar una entrega grande;
modificar una fuente después de revisarla; crear un destino de igual nombre antes de publicar.
Revisar teclado, scroll y acciones a DPI 100/125/150/200 % y en paneles estrechos.
