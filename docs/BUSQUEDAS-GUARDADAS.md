# Búsquedas guardadas multiraíz

Naygo 0.5.0 · Nicolás Groth / ISGroth · MIT

SPDX-License-Identifier: MIT

Desde **F3 → Consultas guardadas / varias carpetas…**, o buscando ese comando con **Ctrl+P**.

## Preparar, guardar y ejecutar

1. Da un nombre a la consulta e introduce una carpeta absoluta por línea (hasta 64).
2. Ajusta nombre o patrón Windows (`*`, `?`), contenido, mayúsculas y recursión.
   Dejar nombre y contenido vacíos permite filtrar únicamente por tamaño o fecha.
3. Tamaños: enteros en bytes, opcionales; los límites son inclusivos y solo admiten archivos.
   Fecha de modificación: cualquiera, hoy, esta semana desde el lunes, últimas 168 o 720 horas.
4. **Guardar como / exportar…** crea un archivo nuevo `.naygosearch`; **Actualizar** guarda
   en el documento previamente cargado. Ninguna de estas acciones ejecuta la búsqueda.
5. **Buscar** evalúa las fechas al ejecutar y comienza el recorrido. **Detener** lo cancela.
   El botón Buscar del panel repite exactamente los criterios de esa ejecución; el editor
   muestra sus campos avanzados por separado de los campos básicos deshabilitados.

**Cargar consulta…** lee los criterios y permite revisar o cambiar rutas antes de buscar.
No hay indexador, vigilancia global ni búsqueda al arrancar. Los resultados no se guardan:
son una instantánea, no una lista que se actualice sola al cambiar el disco.

## Resultados y contexto

Un clic selecciona y actualiza Preview/Propiedades; doble clic o Enter abre con el programa
predeterminado. Flechas, Inicio/Fin y AvPág/RePág desplazan el foco. Shift extiende la selección,
Ctrl+clic alterna elementos y Ctrl+A selecciona todos los resultados recibidos.
El scroll se conserva mientras llegan coincidencias. La carpeta de cada resultado multiraíz
se muestra con ruta completa para distinguir archivos homónimos.

**Añadir selección a la bandeja** reúne únicamente las coincidencias marcadas y muestra la bandeja.
Desde allí puedes copiar/mover a paneles o preparar una entrega con el flujo existente.
Los atajos destructivos del panel de resultados no se redirigen al último panel Files.
Ctrl+P, F1 y Ctrl+Tab mantienen sus funciones globales.

## Alcance y límites visibles

- Hasta 5.000 resultados globales y 100.000 directorios por ejecución, no por raíz.
- Raíces repetidas o anidadas se deduplican léxicamente con recursión. Sin recursión,
  una raíz hija explícita sigue siendo un ámbito independiente.
- No se siguen enlaces/junctions descubiertos. Una raíz elegida explícitamente puede ser un enlace;
  no se deduplican alias físicos ni archivos duros por identidad del disco.
- Búsqueda por contenido: solo texto UTF-8 de hasta 16 MiB; binarios, codificaciones no admitidas,
  enlaces y archivos mayores se omiten de ese filtro y se cuentan como contenido omitido.
- Se muestran fecha/hora de ejecución, raíces efectivas, errores de lectura, contenido omitido,
  cobertura parcial y límites alcanzados. Cancelar conserva los resultados recibidos y los
  identifica como parciales; una ruta inaccesible no se presenta como búsqueda completa.
- Hoy/esta semana usan el calendario local al ejecutar, incluida la transición de horario de
  verano. Los últimos 7/30 días son ventanas móviles de 168/720 horas.
- Cancelación cooperativa entre llamadas al filesystem: un I/O de red que Windows todavía
  no devuelve puede tardar en detenerse. No hay promesa de timeout duro por llamada.

## Ctrl+P, espacios y privacidad

Las consultas cargadas o guardadas aparecen como accesos de carga en Ctrl+P durante la sesión.
Guardar/actualizar un `.naygospace` incluye hasta 64 referencias a esos archivos. Tras reiniciar,
abre el espacio explícitamente para recuperar los accesos; esto no lee ni ejecuta las consultas.

En espacios relativos se reubica el archivo de consulta referenciado, **no las raíces internas
de sus criterios**. Carga la consulta y revísalas antes de ejecutarla en otro equipo. Los espacios
antiguos sin referencias de búsqueda siguen siendo legibles.

**Quitar referencia** retira el acceso del contexto actual, pero no borra el archivo; actualiza
el espacio si deseas conservar ese cambio. Una `.naygolist` es una lista estática de rutas;
una `.naygosearch` contiene condiciones que se vuelven a evaluar.

Los documentos son JSON versionados de hasta 256 KiB. Contienen nombres, rutas y texto de consulta:
revísalos antes de compartirlos; no están cifrados. No contienen scripts ejecutables ni resultados.
Guardar como no sobrescribe. Actualizar verifica la revisión leída y usa publicación atómica.
La detección de cambios ajenos no constituye un bloqueo distribuido contra editores concurrentes.

## Validación instalada pendiente

Probar dos raíces con PDF por fecha, cancelar un recorrido largo, seleccionar resultados y navegar
con teclado, comprobar Preview/Propiedades y bandeja, guardar espacio y recuperarlo después de
reiniciar. Verificar DPI/Narrador y una unidad de red realmente inaccesible en la máquina de destino.
Las pruebas automáticas no sustituyen esa validación interactiva.
