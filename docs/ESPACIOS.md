# Espacios por tarea

Naygo 0.5.0 · Nicolás Groth / ISGroth · MIT

Abre **Disposiciones → Espacios por tarea**, o busca ese comando con **Ctrl+P**.

## Guardar y recuperar una tarea

1. Organiza tus paneles y reúne las referencias en la bandeja.
2. Usa **Guardar como / exportar…** y elige un archivo nuevo `.naygospace`.
   Su nombre identifica la tarea. Puedes guardarlo en tu carpeta de proyectos o compartirlo.
3. **Actualizar espacio** guarda el estado actual en el archivo activo; no hay autoguardado
   sobre los espacios nombrados. El gestor muestra el nombre, ubicación y un `*` si hay cambios.
4. **Abrir / importar…** lee otro espacio y muestra un resumen sin aplicarlo todavía.
   Elige **Guardar actual y abrir** o **Descartar estado actual y abrir**. Si la sesión actual
   nunca se guardó, usa primero Guardar como. Cerrar/Esc descarta la revisión, no tu sesión.

El nombre activo también aparece en el menú y tooltip de Disposiciones. La comprobación de cambios
se hace al abrir el gestor y al terminar una acción, no serializando la bandeja en cada frame.
Para alternar tareas, selecciona sus archivos con el diálogo nativo; no hay un catálogo automático
ni exploración de proyectos al arrancar. Tras reiniciar, abre explícitamente el espacio guardado:
la restauración automática de sesión existente sigue siendo independiente y no incluye la bandeja.

Una disposición antigua puede convertirse sin alterar su plantilla: aplícala, completa sus rutas
y bandeja, y usa Guardar como. No es necesario editar JSON ni volver a organizar los paneles.

## Qué conserva

- Disposición y proporciones, pestañas, tipos de panel, panel activo y enlaces árbol → archivos.
- Rutas, columnas, orden, filtros de tabla y filtro por tipeo (incluido mostrar solo coincidencias).
- Referencias de la bandeja, en orden. No duplica ni empaqueta los archivos.
- Hasta 64 referencias a consultas .naygosearch para cargar desde Ctrl+P, sin ejecutarlas.
  Una raíz portable reubica esas referencias, no las raíces internas del documento de consulta.

No conserva listados, selección/scroll de archivos, bytes de preview, resultados de búsqueda,
comparaciones efímeras ni cola/historial de operaciones. Las operaciones ya iniciadas siguen
siendo globales, con sus fuentes y destinos capturados antes del cambio. Abrir un documento
no ejecuta búsquedas, recetas ni operaciones; confirmar sí lista las carpetas de los nuevos paneles.

## Portabilidad y privacidad

La **raíz opcional** al guardar como convierte las rutas a relativas: todas deben estar dentro
de esa raíz. Al abrir un espacio relativo en otro equipo, escribe la nueva raíz antes de elegir
el archivo. Vacío conserva la raíz original. Actualizar mantiene la raíz usada al abrir.
Los espacios guardados con rutas absolutas no admiten sustitución de raíz: guárdalos primero como
otro espacio relativo. No se buscan automáticamente carpetas equivalentes.

El documento contiene nombres y rutas, incluida la raíz original incluso en modo relativo.
Revísalo antes de compartirlo; **no es un formato anonimizado ni cifrado**. Nunca incluye contraseñas
de servicios, contenido de archivos ni scripts ejecutables como parte del modelo de espacios.

Las referencias de staging de arrastres virtuales se rechazan al guardar: cópialas primero a una
carpeta real y reemplaza las referencias en la bandeja. No se materializan ni se omiten sin avisar.
Las rutas ausentes se conservan: el gestor no verifica red/discos referenciados; después de confirmar,
los paneles las listan en workers y muestran sus errores normales sin bloquear la UI.

## Integridad y límites

El gestor muestra los últimos diez espacios guardados o aplicados durante esta sesión, sin
duplicados. Pulsar uno vuelve a leer el archivo en un worker y abre la revisión; no sustituye
el espacio actual hasta confirmar. «Limpiar» sólo quita referencias del historial, no archivos.
El historial reciente es efímero: no se escriben rutas adicionales en disco ni se indexan carpetas.

JSON versionado, máximo 4 MB, 64 paneles y 10.000 referencias. Guardar como nunca sustituye un
archivo existente: usa otro nombre o Actualizar sobre el abierto. Actualizar vuelve a leer y validar
el documento y compara su SHA-256 con la revisión conocida; archivos corruptos, de versión no
compatible o cambiados externamente no se sobrescriben. Puedes abrir la nueva versión o exportar
tu estado con otro nombre. El guardado usa un temporal exclusivo y publicación atómica en un worker.

La comparación de revisión protege frente a cambios externos detectados antes de publicar;
no constituye un bloqueo distribuido contra editores o sincronizadores concurrentes. Evita editar
el mismo espacio desde dos equipos a la vez. Cancelar es cooperativo: una publicación que ya terminó
no se revierte; su resultado se conserva en el gestor. No hay deshacer de un guardado explícito.

La suite cubre serialización, raíces, documentos inválidos/límites, colisiones, cancelación,
reapertura, cambios sin guardar y una copia que continúa durante el cambio de espacio.
Pendiente validar el diálogo instalado con teclado, DPI/Narrador y una unidad de red realmente caída.
