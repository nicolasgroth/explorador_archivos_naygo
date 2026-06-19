# Changelog

Todas las novedades de Naygo se documentan en este archivo.

El formato se basa en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto sigue [Versionado Semántico](https://semver.org/lang/es/).

## [Sin publicar]
### Añadido
- Vista profunda: un modo del panel que lista, de forma plana y con sangría por
  profundidad, todo el contenido de la carpeta actual y sus subcarpetas (recursivo).
  Se activa con el botón de la barra del panel; aparece por streaming y se puede
  cancelar. El doble clic en una carpeta sale del modo y navega a ella.
- Ícono de historial de carpetas en la barra de herramientas: despliega un menú con
  las carpetas visitadas recientemente y, al elegir una, navega el panel activo.
- La cantidad de carpetas recientes recordadas ahora es configurable (1–100, por
  defecto 50) en la sección Avanzado de la configuración.
- Tooltips explicativos en todos los botones de la barra de herramientas y de los paneles.
- Guía para agregar idiomas (`docs/AGREGAR-IDIOMA.md`): basta soltar un `.json` en `lang/`.
- Registro (log) con más contexto para diagnosticar caídas: marca de tiempo en hora local
  legible, "migas de pan" de las acciones recientes, y al ocurrir un error se vuelca el
  estado (carpetas abiertas, tema, idioma, entorno). Todo local, sin telemetría.
- Seis temas de color nuevos con más personalidad: Windows XP, macOS, Verde sobre azul,
  Solarized Dark, Terminal ámbar y Ciruela.
- En la primera ejecución, Naygo arranca con la disposición clásica (árbol + dos paneles de
  archivos + Propiedades + Vista previa) en vez de un solo panel.
- Argumentos de línea de comandos: `naygo.exe <carpeta>` abre esa carpeta al iniciar, y
  `--theme`/`--layout` aplican un tema o una disposición solo para esa sesión. `--help` y
  `--version` muestran la información correspondiente. Útil para accesos directos y para
  "Abrir en Naygo".
### Cambiado
- Los avisos y confirmaciones internos (confirmar expulsar una unidad USB, errores al
  importar/exportar packs) ahora usan un diálogo con el tema de Naygo en vez del cuadro
  nativo del sistema. El mensaje de cierre por error inesperado es más claro y el detalle
  técnico queda en el registro.
- El registro de diagnóstico ahora se guarda por día: `naygo-AAAA-MM-DD.log` (antes un único
  `naygo.log`), para no mezclar las corridas y diagnosticar más fácil.
- El aviso de "carpeta no encontrada" ahora refresca el panel al instante al pulsar cualquier
  opción (antes había que redimensionar); "Subir un nivel" solo aparece si hay una carpeta
  superior a la que ir; y al volver atrás a una unidad ausente se ven de nuevo las opciones.
- Los paneles de Propiedades y Vista previa se marcan como panel activo al hacer clic en
  cualquier parte de su cuerpo, no solo en la barra de título.
- El menú de plantillas de disposición aparece junto a su botón en la barra (antes salía lejos).
### Corregido
- Caída (o cuelgue con la ventana en blanco) al previsualizar archivos de texto con líneas muy
  largas, frecuente en logs: el renderizador por software no podía posicionar glifos tan a la
  derecha y la app se cerraba con un error. Ahora la Vista previa ajusta las líneas a la columna
  y recorta las larguísimas (igual para el texto extraído de PDF).
- Mayor robustez en equipos y máquinas virtuales sin GPU: Naygo fuerza el renderizador por
  software desde el arranque, sin depender de una GPU acelerada.

## [0.1.0] — 2026-06-18
### Añadido
- Navegación de archivos tipo Commander: paneles dinámicos acoplables, dual-pane,
  ir atrás/adelante (incluidos los botones laterales del mouse).
- Árbol de carpetas con expansión incremental, revelado hasta la carpeta activa y
  navegación por teclado.
- Listado por streaming incremental y cancelable; el filesystem hostil (red caída,
  permisos, rutas que desaparecen) no tumba la app.
- Columnas dinámicas estilo planilla: ordenar, filtrar por tipo de columna y
  reordenar arrastrando.
- Operaciones de archivo entre paneles (copiar, mover, eliminar) con cola opcional,
  progreso y cancelación.
- Renombrado en línea y en cadena, y ventana de renombrado por lotes.
- Búsqueda recursiva por nombre en la carpeta y sus subcarpetas.
- Previsualización liviana: imágenes, SVG (rasterizado), PDF (texto y metadatos),
  texto/código y listado de contenido de archivos ZIP.
- Cálculo de tamaño de carpetas bajo demanda.
- Barra de unidades de disco con espacio libre/total y porcentaje usado; ícono
  propio para unidades USB y expulsión segura desde un menú.
- Detección de discos duros externos USB como extraíbles (por tipo de bus).
- Integración con Windows: menú contextual del shell, "Abrir con", watcher de
  carpeta, detección de dispositivos, arrastrar y soltar, ícono de bandeja y
  arranque opcional con el sistema.
- Internacionalización (español e inglés) y temas intercambiables en caliente, con
  galería de selección y packs de usuario.
- Configuración como ventana nativa: apariencia, atajos, previsualización, plantilla
  de tabla, opciones avanzadas y sección "Acerca de".
- Distribución como ejecutable portable e instalador (Inno Setup) con CRT estático.
