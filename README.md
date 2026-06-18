# Naygo

Un explorador de archivos rápido y liviano para Windows 10/11, estilo **Commander**
(inspirado en Directory Opus). Paneles dinámicos, navegación por teclado, multi-idioma
y temas personalizables.

> **Estado:** UI en Slint (render por software, sin GPU). Funciona: multi-panel con
> redimensionado en vivo, árbol de carpetas, columnas estilo planilla (orden + filtros),
> renombrado en línea y por lotes, plantillas de disposición (Layouts), búsqueda recursiva,
> previsualización (imágenes, SVG, PDF, texto/código y contenido de ZIP), barra de unidades
> con espacio libre/usado y expulsión segura de USB, drag & drop (interno y con el sistema),
> abrir una terminal en la carpeta, configuración completa (incluido Acerca de + Avanzado),
> bandeja del sistema, iniciar con Windows y navegación por teclado.
>
> **Guía de uso:** [`docs/GUIA-DE-USUARIO.md`](docs/GUIA-DE-USUARIO.md) (o **F1** en la app).
> **Novedades por versión:** [`CHANGELOG.md`](CHANGELOG.md) (y la sección "Acerca de" muestra
> las de la versión instalada).

## Objetivos

- **Rápido ante todo.** Navegar entre carpetas se siente instantáneo, incluso con decenas de
  miles de archivos o discos de red lentos (listado por streaming incremental).
- **Estilo Commander.** Paneles dinámicos acoplables, dos (o más) carpetas a la vez, atajos
  de teclado para todo.
- **Personalizable.** Multi-idioma (español e inglés de base, fácil agregar más) y temas con
  *color sets* intercambiables en caliente.
- **Cancelable.** Cualquier operación larga (copiar, mover, listar, calcular tamaño) se puede
  detener al instante.
- **Liviano y robusto.** Bajo consumo, y nunca se cae porque un disco de red se desconecte.

## Funcionalidades

- Navegación por paneles dual (o múltiples), con ir atrás/adelante (incluidos los botones
  laterales del mouse) y barra de ruta editable con favoritos.
- Árbol de carpetas con expansión incremental y revelado hasta la carpeta activa.
- Columnas estilo planilla: ordenar, filtrar por tipo de columna y reordenar arrastrando.
- Operaciones entre paneles (copiar, mover, eliminar) con cola opcional, progreso y cancelación.
- Renombrado en línea, en cadena y por lotes.
- Búsqueda recursiva por nombre en la carpeta y sus subcarpetas.
- Previsualización liviana: imágenes, SVG, PDF (texto y metadatos), texto/código y el
  contenido de archivos ZIP.
- Cálculo de tamaño de carpetas bajo demanda.
- Barra de unidades de disco con espacio libre/total y porcentaje usado; ícono propio para
  unidades USB y expulsión segura.
- Integración con Windows: menú contextual del shell, "Abrir con", detección de cambios y de
  dispositivos, drag & drop, bandeja del sistema y arranque opcional con el sistema.
- Configuración completa: apariencia, atajos, previsualización, plantilla de tabla, opciones
  avanzadas y sección "Acerca de".

## Stack

Rust + [Slint](https://slint.dev) (UI con renderizador por software, sin GPU) + el crate
oficial `windows` para la integración con el Shell de Windows. Open source, sin dependencias
de pago.

## Instalación

- **Portable**: descarga `Naygo-<versión>-portable.zip`, descomprímelo y ejecuta `naygo.exe`.
  No instala nada.
- **Instalador**: ejecuta `Naygo-<versión>-setup.exe` y sigue el asistente. Si ya tienes una
  versión instalada, el instalador la actualiza sin perder tu configuración.

La primera vez, Windows SmartScreen puede advertir "editor desconocido" (el `.exe` no está
firmado): haz clic en **"Más información" → "Ejecutar de todos modos"**.

## Compilar desde el código

Necesitas Rust (toolchain MSVC). Para compilar y empaquetar, consulta
[`docs/BUILD.md`](docs/BUILD.md) y [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md).

Resumen:
- Compilar release: `cargo build --release` (el binario queda en `target/release/naygo.exe`).
- Empaquetar portable + instalador: `scripts\build-release.ps1`.
- Publicar una versión nueva: `scripts\bump.ps1` (sube la versión según los commits, mueve el
  CHANGELOG y crea el tag; agrega `-Push` para publicar).

## Licencia

[MIT](LICENSE) © 2026 **Nicolás Groth / ISGroth**.

Libre de usar, modificar y distribuir. Si te sirve, se agradece mencionar a Nicolás Groth e
ISGroth.
