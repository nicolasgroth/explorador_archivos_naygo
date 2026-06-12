# Modo de bajo consumo / render sin GPU — Diseño (2026-06-12)

> Tras llevar Naygo a una VM sin GPU, el consumo de CPU era alto. El fix previo
> (`245834a`) eliminó el repaint en reposo. Este diseño ataca el consumo DURANTE EL
> USO (hover/scroll) y hace a Naygo viable en equipos sin GPU dedicada, en línea con
> el principio del proyecto: velocidad y bajo consumo, lo visual subordinado.

## Contexto y medición

egui es de **modo inmediato**: cada frame recalcula y re-tesela TODA la UI en CPU; la
GPU solo rasteriza el resultado. Medido en debug:
- Reposo: ~0% (arreglado, la UI duerme).
- Repaint continuo (simula mouse-move/scroll sostenido): **94% de un núcleo CON GPU**.
  Sin GPU, además se rasteriza por software → mucho peor (el 53% que vio Nicolás).

Conclusión: el costo del repaint continuo es alto **con o sin GPU**. La solución es
**repintar menos y más barato**, no añadir GPU.

## 1. Límite de framerate del hover (para TODOS)

Hoy, mover el mouse sobre la UI pide `request_repaint()` → 60+ fps. Cambiarlo a
`request_repaint_after(33ms)` (~30 fps) mientras el puntero se mueve. La fila bajo el
cursor se sigue resaltando con fluidez, a la mitad del costo. Aplica SIEMPRE (con y sin
GPU): es imperceptible y beneficia a todos (el 94% medido baja a ~la mitad).

Archivo: `crates/ui/src/app.rs` (bloque `pointer_moving`).

## 2. Detección de render por software

Al arrancar, leer `GL_RENDERER` del contexto glow (`eframe` reexporta `glow`;
`CreationContext.gl: Option<Arc<glow::Context>>`). Si el nombre contiene un marcador de
software, marcar `software_render = true`.

- Clasificación PURA en `core` (testeable): `fn is_software_renderer(name: &str) -> bool`
  → `true` si el nombre (en minúscula) contiene: `llvmpipe`, `software`, `swiftshader`,
  `microsoft basic render`, `gdi generic`, `softpipe`.
- Lectura del string en `ui` (en `NaygoApp::new`, vía `cc.gl`), guardada en un campo
  `software_render: bool`.

```rust
// core/src/render_hint.rs
pub fn is_software_renderer(renderer_name: &str) -> bool {
    let n = renderer_name.to_lowercase();
    ["llvmpipe", "software", "swiftshader", "microsoft basic render",
     "gdi generic", "softpipe"].iter().any(|m| n.contains(m))
}
```

## 3. Setting `low_power_mode` + modo efectivo

`Settings.low_power_mode: LowPowerMode { Auto, Always, Never }` (default `Auto`,
serde retro-compat). Modo EFECTIVO:
- `Always` → bajo consumo ON.
- `Never` → OFF.
- `Auto` → ON si `software_render`.

```rust
// core/src/config: enum + helper
pub enum LowPowerMode { Auto, Always, Never }
// en ui: fn low_power_active(&self) -> bool
//   match self.settings.low_power_mode {
//     Always => true, Never => false, Auto => self.software_render }
```

## 4. Qué hace el modo bajo consumo activo

Cuando `low_power_active()`:
1. **Animaciones off**: `ctx.style_mut(|s| s.animation_time = 0.0)` aplicado al
   arrancar/cambiar el modo. Quita los fades de hover/selección que disparan ráfagas de
   ~5 repaints por interacción. Con GPU es imperceptible; sin GPU ahorra mucho.
2. (El hover a 30fps del punto 1 ya aplica a todos, así que no es exclusivo del modo.)

Modo normal (GPU): animaciones por defecto de egui (comportamiento actual).

El modo se aplica de forma idempotente cada frame o ante cambio del setting (barato:
solo setea `animation_time`). Se sigue el patrón del watcher de tema/íconos que ya
detecta diferencias y aplica en caliente.

## 5. Selector en Configuración

Configuración → Avanzado: un `segmented` (widget existente) con Auto / Siempre / Nunca,
con un hint que explique "reduce el consumo de CPU en equipos sin tarjeta gráfica
dedicada". Persiste por el watcher de settings existente.

i18n ES/EN: etiqueta del setting, las 3 opciones, el hint.

## 6. Pregunta de bienvenida en el primer arranque

En el PRIMER arranque (no existe `settings.json`), tras cargar, mostrar un diálogo
modal de bienvenida UNA vez: "¿Cómo prefieres que Naygo use los recursos?" con 3
botones: **Bajo consumo** (low_power = Always), **Todo activo** (Never), **Automático**
(Auto, recomendado). Al elegir, se setea `low_power_mode` y se persiste.

- Funciona igual para instalador Y portable (vive en la app, no en Inno Setup).
- `NaygoApp::new` ya calcula `!settings_exists` y persiste settings de inmediato; hay
  que capturar ese booleano en un campo `show_welcome: bool` ANTES del save, y pintar
  el diálogo en `update` mientras sea `true` (como los otros modales diferidos).
- El diálogo es ligero (egui `Window` modal centrada); no bloquea workers.

## Tests

- core::render_hint: `is_software_renderer` (llvmpipe/swiftshader/basic render → true;
  "NVIDIA GeForce"/"Intel UHD"/"AMD Radeon" → false; case-insensitive).
- core::config: round-trip de `low_power_mode`; settings viejo sin el campo → `Auto`.
- Verificación en vivo (Nicolás): en la VM, el consumo durante scroll/mouse-move baja;
  el selector Auto/Siempre/Nunca cambia el comportamiento; el diálogo de bienvenida
  aparece solo la 1ª vez.

## Fuera de alcance

- Cambiar el backend de render (glow→wgpu): no ataca la raíz, incierto.
- Reescribir la UI en modo retenido (Win32/GTK): semanas, descartado.
- Preguntar en el instalador (Inno Setup): se decidió preguntar en el primer arranque
  de la app, que cubre instalador y portable sin duplicar lógica.
- Throttling del scroll por separado: el límite de hover a 30fps + animaciones-off ya
  cubren el grueso; si tras medir el scroll sigue caro, se evalúa después.
