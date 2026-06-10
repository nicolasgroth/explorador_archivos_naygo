# "Acerca de…" + easter egg dinámico — Diseño (Entrega 2)

> Autor: Nicolás Groth / ISGroth — 2026-06-10. MIT License.

## Qué es

Pedido de Nicolás: una ventana/sección "Acerca de…" que marque la autoría de forma
visible (objetivo del proyecto según CLAUDE.md) y que esconda un **easter egg
dinámico** (animado).

## Diseño

### Sección "Acerca de" en Configuración
Nueva sección al final de la lista de la ventana Configuración (patrón existente:
`settings_window/about.rs` + variante en `SettingsSection`). Contenido:

- **Logo de Naygo** centrado (textura cargada una vez desde el PNG embebido del
  splash, cacheada en `NaygoApp`).
- **Naygo** (heading) + versión (`CARGO_PKG_VERSION`).
- Autor: **Nicolás Groth** (ngroth@gmail.com) — **ISGroth**, Chile.
- Año 2026 · Licencia **MIT**.
- URL del repo (label seleccionable/click abre el navegador via `ctx.open_url`).
- Texto corto: hecho con Rust + egui (créditos mínimos).

Todo por claves i18n (`about.*`) en ES/EN.

### Easter egg: "lluvia de Naygo"
- **Disparador**: 5 clics seguidos sobre el logo (contador se resetea a los 2 s sin
  clic). Nada lo anuncia: es un huevo de pascua.
- **Efecto**: durante ~8 s, dentro de la ventana de Configuración llueven íconos
  (carpetas/archivos del set activo, reusando texturas ya cargadas del
  `IconProvider`) con posiciones/velocidades pseudoaleatorias (semilla = índice;
  sin `rand`, determinista simple con seno/hash). Al fondo, un mensaje que se
  ESCRIBE letra a letra: «Hecho con ♥ en Chile — Nicolás Groth / ISGroth».
- **Bajo consumo**: `request_repaint` SOLO mientras el egg está activo y la ventana
  de Configuración abierta; al expirar (o cerrar la sección) se detiene todo.
- Estado en `NaygoApp`: `egg_clicks: u8`, `egg_last_click: Option<Instant>`,
  `egg_until: Option<Instant>`.

### Arquitectura
- `ui/settings_window/about.rs` (nuevo, UI pura): pinta sección + maneja clics del
  logo + pinta la lluvia con `ui.painter()` sobre el rect de la sección.
- `NaygoApp`: textura del logo (lazy) + estado del egg. Sin cambios en core (la
  autoría/versión salen de `env!` y claves i18n).

### Tests
La sección es UI pura (manual). Verificación en vivo (computer-use): abrir
Configuración → Acerca de, ver autoría; 5 clics al logo → lluvia + mensaje animado;
expira sola a los ~8 s.

## Fuera de alcance
Ventana About independiente del SO (la sección en Configuración cumple y evita otro
viewport); easter eggs adicionales.
