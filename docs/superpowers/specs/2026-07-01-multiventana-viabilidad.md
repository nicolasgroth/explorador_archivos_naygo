# Multi-ventana (ventanas top-level independientes) — Informe de viabilidad

> Investigación previa a diseño. Fecha: 2026-07-01. NO es un spec de implementación:
> es el análisis para decidir si/cuándo abordar el proyecto. Estado: **viable, diferido**.

## Pregunta

¿Puede Naygo soportar varias ventanas independientes del SO (como varias ventanas
del Explorador de Windows, cada una con su entrada en la barra de tareas), en vez de
la única ventana actual?

## Veredicto: VIABLE, pero es un proyecto grande (varios lotes), no una feature

### Parte A — ¿Slint lo soporta? SÍ (Slint 1.16)

- Multi-ventana es soporte **oficial desde Slint 1.7** (Naygo está en 1.16). Se
  instancian varias `AppWindow` (`::new()`), `.show()` en cada una, y **un solo event
  loop** (`run_event_loop()`) las maneja a todas. Hay `run_event_loop_until_quit()`
  pensado para apps de bandeja que siguen vivas sin ventanas — encaja con "X→bandeja".
- **Limitación**: los `global` singletons de Slint son por-instancia de componente, no
  compartidos entre ventanas. Para Naygo es manejable (el estado real vive en Rust, no
  en globals de Slint), pero obliga a propagar tema/idioma a cada ventana desde Rust.

### Riesgo técnico principal — DESCARTADO por spike en la VM

El render por SOFTWARE + Windows fue exactamente la combinación que rompió multi-ventana
en Slint (discussion #8823: las ventanas sin foco se congelaban), arreglado en PR #8828
(~Slint 1.8). Como Naygo FUERZA el render por software (sin GPU, para VMs), había que
confirmar el fix en esa config exacta.

**Spike ejecutado (2026-07-01)**: `examples/dos_ventanas.rs` — dos ventanas top-level
sobre el render por software, cada una con un contador por Timer. **Resultado en la VM
sin GPU de Nicolás: la ventana SIN foco sigue redibujando (el contador sube).** El bug
NO se reproduce en 1.16. Riesgo de raíz descartado. (El example se eliminó tras
confirmar; era descartable.)

### Parte B — Acoplamiento de Naygo a ventana única (dónde está el trabajo)

Todo el estado vive en **un** `WorkspaceCtrl` (`Rc<RefCell<>>`, `main.rs:232`) + **una**
`AppWindow` (`main.rs:228`). Puntos a rediseñar, por dificultad:

- **Fácil** — `platform/window_geometry.rs` y `window.rs` (`bring_to_front`) ya son
  per-HWND. Config/i18n/temas son globales (correcto compartirlos entre ventanas).
- **Medio** — Geometría persistida (`Settings.window` singular → por ventana). Sesión
  (`workspace.json` único → lista de ventanas o archivo por ventana). Tray (handlers
  globales del proceso → decidir a qué ventana afectan sus acciones).
- **Difícil** — El `WorkspaceCtrl` único **es** casi toda la app (config, ops, listings,
  trees, watchers, sesión, preview). Separar "estado de proceso" (config, temas, ops de
  archivo) de "estado de ventana" (paneles, listings, watchers) toca `main.rs` (~5500
  líneas), el ctrl, la sesión, el tray y el ciclo de vida. Migrar `ui.run()` →
  `run_event_loop()` cambia el arranque/cierre de raíz.

## Decisiones de diseño pendientes (para el brainstorm futuro)

1. Sesión/geometría: ¿por ventana (recomendado) o compartida?
2. Tray (uno por proceso): su "Abrir" ¿trae la última activa? ¿"Nuevo panel" abre ventana
   nueva o divide la activa? ¿submenú listando ventanas?
3. Config/tema/idioma: compartida (recomendado) + propagación en caliente a N ventanas.
4. **Hotkey global**: ¿a qué ventana trae al frente? (última activa, ciclar, abrir nueva).
   El hotkey que se implementa ahora asume UNA ventana; cuando llegue multi-ventana
   necesitará esta regla.
5. Cerrar la última ventana: ¿sale la app o va a bandeja? (`run_event_loop` vs
   `run_event_loop_until_quit`).
6. Ops de archivo entre ventanas: ¿copiar de un panel de la ventana A a uno de la B?
   Eso vuelve las ops estado de proceso, no de ventana.

## Estimación: grande — 3 a 5 lotes

1. Refactor del ctrl a "proceso vs ventana" + migración a `run_event_loop` ← el más caro
   y arriesgado; bloquea al resto.
2. Sesión/geometría por ventana.
3. Tray + hotkey + ciclo de vida multi-ventana.
4. Propagación de config/tema/idioma a N ventanas.
5. Ops de archivo entre ventanas (opcional/posterior).

## Recomendación

**Diferido, no descartado.** No hay muro técnico. Cuando se aborde, empezar por el lote 1
(refactor del ctrl), que es el que desbloquea todo. El spike ya quitó el único riesgo que
podía invalidar el proyecto entero, así que la decisión es puramente de prioridad/esfuerzo.
