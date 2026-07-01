# Naygo

Un explorador de archivos rápido y liviano para Windows 10/11, estilo **Commander**
(inspirado en Directory Opus). Paneles dinámicos, navegación por teclado, diez idiomas
incluidos, temas y sets de íconos personalizables.

> **Estado:** UI en Slint (render por software, sin GPU). Funciona: multi-panel con
> redimensionado en vivo (mover un divisor solo afecta a sus dos vecinos; doble clic =
> 50/50), árbol de carpetas, columnas estilo planilla (orden + filtros), renombrado en
> línea y por lotes, plantillas de disposición (Layouts), búsqueda recursiva,
> previsualización (imágenes, SVG, PDF, texto/código y contenido de .zip/.tar/.tar.gz
> como árbol), comprimir y extraer .zip, barra de unidades con espacio libre/usado y
> expulsión segura de USB, drag & drop (interno y con el sistema), abrir una terminal en
> la carpeta, configuración completa (incluido Acerca de + Avanzado), diez idiomas, sets
> de íconos personalizables, bandeja del sistema (la X esconde a la bandeja), atajo
> global Ctrl+Alt+Q para mostrar/ocultar, iniciar con Windows (opcionalmente minimizado)
> y navegación por teclado. La ventana recuerda su tamaño y posición.
>
> **Guía de uso:** [`docs/GUIA-DE-USUARIO.md`](docs/GUIA-DE-USUARIO.md) (o **F1** en la app).
> **Novedades por versión:** [`CHANGELOG.md`](CHANGELOG.md) (y la sección "Acerca de" muestra
> las de la versión instalada).

## Objetivos

- **Rápido ante todo.** Navegar entre carpetas se siente instantáneo, incluso con decenas de
  miles de archivos o discos de red lentos (listado por streaming incremental).
- **Estilo Commander.** Paneles dinámicos acoplables, dos (o más) carpetas a la vez, atajos
  de teclado para todo.
- **Personalizable.** Diez idiomas incluidos (es, en, pt, fr, de, it, zh, ja, ko, hi;
  agregar otro = soltar un archivo), temas con *color sets* intercambiables en caliente y
  sets de íconos configurables objeto por objeto.
- **Cancelable.** Cualquier operación larga (copiar, mover, listar, calcular tamaño) se puede
  detener al instante.
- **Liviano y robusto.** Bajo consumo, y nunca se cae porque un disco de red se desconecte.

## Funcionalidades

- Navegación por paneles dual (o múltiples), con ir atrás/adelante (incluidos los botones
  laterales del mouse) y barra de ruta editable con favoritos.
- Árbol de carpetas con expansión incremental y revelado hasta la carpeta activa.
- Columnas estilo planilla: ordenar, filtrar por tipo de columna y reordenar arrastrando.
- Operaciones entre paneles (copiar, mover, eliminar) con cola opcional, progreso y cancelación.
- Comprimir la selección en un `.zip` y extraer un `.zip` desde el menú contextual, en
  segundo plano, con progreso, cancelación y deshacer seguro.
- Renombrado en línea, en cadena y por lotes.
- Búsqueda recursiva por nombre en la carpeta y sus subcarpetas.
- Previsualización liviana: imágenes, SVG, PDF (texto y metadatos), texto/código y el
  contenido de archivos ZIP.
- Cálculo de tamaño de carpetas bajo demanda.
- Barra de unidades de disco con espacio libre/total y porcentaje usado; ícono propio para
  unidades USB y expulsión segura.
- Integración con Windows: menú contextual del shell, "Abrir con", detección de cambios y de
  dispositivos, drag & drop, bandeja del sistema y arranque opcional con el sistema.
- Sets de íconos: cinco de fábrica (Lucide, Mono, Tabler, Material, Flat Color), cambio de
  ícono por objeto (o un PNG propio) y packs `.naygoset` para compartir.
- Atajo global **Ctrl+Alt+Q** (configurable) que muestra u oculta Naygo desde cualquier
  aplicación.
- La **X** esconde a la bandeja del sistema (salir de verdad: menú de la bandeja); la
  ventana recuerda tamaño, posición y maximizado entre sesiones.
- Configuración completa: apariencia, atajos, previsualización, plantilla de tabla, opciones
  avanzadas y sección "Acerca de".

## Stack

Rust + [Slint](https://slint.dev) (UI con renderizador por software, sin GPU) + el crate
oficial `windows` para la integración con el Shell de Windows. Open source, sin dependencias
de pago.

## Descargar

**[⬇ Descargar Naygo (portable)](https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip)** — siempre apunta a la última versión publicada. También está el [instalador](https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-setup.exe).

## Instalación

- **Portable**: descarga `Naygo-<versión>-portable.zip`, descomprímelo y ejecuta `naygo.exe`.
  No instala nada.
- **Instalador**: ejecuta `Naygo-<versión>-setup.exe` y sigue el asistente (disponible en
  siete idiomas; incluye un paso para elegir el idioma inicial de Naygo entre los diez
  disponibles, preseleccionado según el idioma de Windows, y una casilla opcional "Iniciar
  Naygo al arrancar Windows"). Si ya tienes una versión instalada, el instalador la
  actualiza sin perder tu configuración.

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

## Uso por línea de comandos

`naygo.exe` acepta argumentos opcionales:

```
naygo.exe [<carpeta>] [--theme <id>] [--layout <nombre>] [--help] [--version]
```

- `<carpeta>`: abre Naygo en esa carpeta al iniciar (debe existir). Es lo que usa "Abrir en
  Naygo" del menú contextual del Explorador.
- `--theme <id>`: aplica un tema solo en esa sesión (no cambia tu configuración). Los ids son
  los temas de Configuración → Apariencia (p. ej. `dark-blue`, `winxp`, `macos`, `solarized-dark`).
- `--layout <nombre>`: aplica una plantilla de disposición de paneles (de las del menú Layouts,
  incluidas las tuyas).
- `--tray`: arranca minimizado en la bandeja (sin mostrar la ventana). Es el argumento que
  usa la entrada "Iniciar con Windows" cuando está activo "Iniciar minimizado en la
  bandeja"; si la bandeja está desactivada, se ignora sin efecto.
- `--help`, `--version`: muestran un cuadro con la ayuda o la versión.

Los valores que no existan se ignoran (la app abre igual y deja un aviso).

## Licencia

[MIT](LICENSE) © 2026 **Nicolás Groth / ISGroth**.

Libre de usar, modificar y distribuir. Si te sirve, se agradece mencionar a Nicolás Groth e
ISGroth.

Naygo usa software de terceros, cada uno bajo su propia licencia (todas permisivas; la interfaz
usa [Slint](https://slint.dev) bajo su licencia *royalty-free*). Los avisos están en
[`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).
