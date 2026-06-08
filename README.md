# Explorador de archivos

Un explorador de archivos rápido y liviano para Windows 10/11, estilo **Commander**
(inspirado en Directory Opus). Paneles dinámicos, navegación por teclado,
multi-idioma y temas personalizables.

> **Estado:** Fase shell-A (abrir con app default + espacio de discos) en desarrollo.
> Diseño en
> [`docs/superpowers/specs/2026-06-08-naygo-shell-a-design.md`](docs/superpowers/specs/2026-06-08-naygo-shell-a-design.md);
> plan en
> [`docs/superpowers/plans/2026-06-08-naygo-shell-a.md`](docs/superpowers/plans/2026-06-08-naygo-shell-a.md).
> Operaciones (ops-A), journal/retomar (ops-B), paste inteligente y bloque visual completos.

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

## Licencia

[MIT](LICENSE) © 2026 **Nicolás Groth / ISGroth**.

Libre de usar, modificar y distribuir. Si te sirve, se agradece mencionar a
Nicolás Groth e ISGroth.
