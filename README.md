# Explorador de archivos

Un explorador de archivos rápido y liviano para Windows 10/11, estilo **Commander**
(inspirado en Directory Opus). Paneles dinámicos, navegación por teclado,
multi-idioma y temas personalizables.

> **Estado:** Drag & drop (interno entre paneles + con el SO: Explorer↔Naygo),
> persistencia del layout del dock y varios pulidos agregados. Diseño en
> [`docs/superpowers/specs/2026-06-09-naygo-dnd-pulidos-design.md`](docs/superpowers/specs/2026-06-09-naygo-dnd-pulidos-design.md).
> Pendiente: "Acerca de…" (Entrega 2) y bandeja del sistema + iniciar con Windows (Entrega 3).

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

Rust + [egui](https://github.com/emilk/egui) (UI por GPU) + el crate oficial
`windows` para la integración con el Shell de Windows. 100% open source, sin
dependencias de pago.

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
