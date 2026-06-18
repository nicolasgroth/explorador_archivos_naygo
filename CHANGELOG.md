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
