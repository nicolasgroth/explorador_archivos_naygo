# Explorador de archivos

Un explorador de archivos rápido y liviano para Windows 10/11, estilo **Commander**
(inspirado en Directory Opus). Paneles dinámicos, navegación por teclado,
multi-idioma y temas personalizables.

> **Estado:** UI en Slint (render por software, sin GPU). Funciona: multi-panel con
> redimensionado en vivo, árbol, columnas estilo Excel (orden + filtros), renombrado
> por lotes, plantillas de disposición (Layouts), drag & drop (interno + con el SO),
> abrir terminal en la carpeta, configuración completa (incl. Acerca de + Avanzado),
> bandeja del sistema e iniciar con Windows, y navegación por teclado.
> **Guía de uso:** [`docs/GUIA-DE-USUARIO.md`](docs/GUIA-DE-USUARIO.md) (o **F1** en la app).

## Objetivos

- **Rápido ante todo.** Navegar entre carpetas se siente instantáneo, incluso con
  decenas de miles de archivos o discos de red lentos (listado por streaming
  incremental).
- **Estilo Commander.** Paneles dinámicos dockables, dos (o más) carpetas a la
  vez, atajos de teclado para todo.
- **Personalizable.** Multi-idioma (ES + EN de base, fácil agregar más) y temas
  con *color sets* intercambiables en caliente.
- **Cancelable.** Cualquier operación larga (copiar, mover, listar, calcular
  tamaño) se puede detener al instante.
- **Liviano y robusto.** Bajo consumo, y nunca se cae porque un disco de red se
  desconecte.

## Stack

Rust + [Slint](https://slint.dev) (UI con renderizador por software, sin GPU) +
el crate oficial `windows` para la integración con el Shell de Windows. 100% open
source, sin dependencias de pago.

## Instalación / Build

Para usar Naygo:

- **Portable**: descargá `Naygo-<versión>-portable.zip`, descomprimí y ejecutá
  `naygo.exe`. No instala nada.
- **Instalador**: ejecutá `Naygo-<versión>-setup.exe` y seguí el asistente.

La primera vez, Windows SmartScreen puede advertir "editor desconocido" (el `.exe` no
está firmado): hacé clic en **"Más información" → "Ejecutar de todos modos"**.

Para compilar y empaquetar desde el código, ver
[`docs/BUILD.md`](docs/BUILD.md) y [`docs/DISTRIBUTION.md`](docs/DISTRIBUTION.md).

## Licencia

[MIT](LICENSE) © 2026 **Nicolás Groth / ISGroth**.

Libre de usar, modificar y distribuir. Si te sirve, se agradece mencionar a
Nicolás Groth e ISGroth.
