# Arreglo del subsistema de copiar/mover + panel de progreso — Diseño

> Naygo (explorador de archivos Rust + Slint, render software, Windows).
> Autor: Nicolás Groth / ISGroth. Fecha: 2026-06-22.
> Bloque de 5 fases. El motor `core::ops` está sano (confirmado con tests); este bloque arregla
> el cableado UI, agrega progreso real + pausa, y construye el panel de operaciones rico.

## Contexto y diagnóstico (ya confirmado)

Tras probar `target\release\naygo.exe`, el usuario reportó que copiar/mover —la función esencial—
está roto. Un workflow de diagnóstico de 5 agentes + debugging sistemático confirmaron la causa
raíz de cada bug. **El motor `core::ops` copia archivos completos sin truncar** (test
`ask_overwrite_copia_archivo_grande_completo` lo prueba: copia 1 MiB multi-chunk byte a byte por el
camino Ask+Overwrite). Los bugs son de la capa UI, estado de modificadores, y granularidad del
progreso — NO del motor.

### Los 5 bugs (causa raíz verificada)

| # | Síntoma | Causa raíz (archivo:línea) |
|---|---------|----------------------------|
| 1 | Doble clic en carpeta solo ofrece "abrir en otro panel", no navega | `ctrl_down` se setea en cada keydown (`workspace_ctrl.rs:4016`) y NUNCA se resetea en keyup → queda pegado en `true` tras un Ctrl+C; el doble clic entra por la rama `if self.ctrl_down` (`:3502`). |
| 4 | No hay ventana de progreso (barra/bytes/velocidad/ETA) | `copy_buffered` (`engine.rs:318-334`) NO emite `OpMsg::Progress` durante la copia (solo una vez antes, `:107`). El VM `OpRowVm`/`OpRowData` (`types.slint:225`, `ops_ctrl.rs:652`) solo tiene 5 campos; descarta bytes/velocidad/ETA/archivo-actual. |
| 3 | Pegar+sobrescribir copió ~5 MB de 95 GB y paró | NO es el motor (test lo prueba). Es Bug 4: copia en curso invisible parece detenida → el usuario la dio por muerta. El parcial quedó sin borrar porque el proceso terminó sin pasar por las ramas de limpieza (`Ok(false)`/`Err`). |
| 5 | Falta panel de operaciones (cola/cancelar/total/historial) | Backend de cola existe (`OpsMode::Queue`, `ops_ctrl.rs:191`); la UI Slint es una barra plana de 30px (`ops-panel.slint:33`). `Settings.ops_display` existe pero solo la variante Panel se montó. |
| 2 | Drag&drop entre paneles no hace nada | No migrado de egui. `on_row_drag_out` (`main.rs:1252`) solo arranca OLE hacia el SO; el drop intra-app (panel→panel) no existe. |

## Decisiones tomadas (con el usuario)

- **Pausa real**: suspender y reanudar la copia en vivo (no solo cancelar).
- **Panel acoplado** (`PanePurpose::Operations`, como Árbol/Preview), que **auto-aparece** al iniciar
  una op, mostrando **en curso + cola + historial reciente**.
- **Drag nativo de Slint** entre paneles (no OLE para intra-app): mismo disco **mueve**, otro disco
  **copia**, Ctrl fuerza copiar, Shift fuerza mover (reusa `core::dnd::decide_drop_action`).
- **Panel data-driven y modular** (el VM lleva TODOS los campos; render de piezas reutilizables),
  preparado para crecer; SIN editor de personalización por el usuario por ahora (YAGNI).

## Arquitectura general

El motor `core::ops` se mantiene. Se le agrega: (a) progreso por bytes durante la copia, (b) estado
de pausa. La velocidad/ETA y todo el panel viven en la UI (el motor reporta bytes crudos, se queda
puro). El drag nativo reusa `core::dnd` y `ops::transfer`. Nada se reescribe desde cero.

---

## Fase 1 — Bug 1: reset de modificadores

**Core:** ninguno. **UI:**
- Cablear `FocusScope.key-released` en Slint (hoy solo `key-pressed`) → callback `on_key_release`
  en el controlador que resetea `ctrl_down`/`shift_down` a `false` según la tecla liberada (o a
  `false` ambos en cada keyup como red de seguridad simple).
- Defensa extra: limpiar los modificadores al abrir cualquier modal y, si Slint lo expone, al
  perder foco la ventana.

**Testing:** estado de UI, no testeable en core; verificación en VM (Ctrl+C → doble clic en carpeta
→ navega).

---

## Fase 2 — Bug 4 + Bug 3: progreso por bytes + pausa + robustez (core)

**`core::ops::engine`:**
- **Progreso durante la copia.** `copy_buffered` emite `OpMsg::Progress` con throttle (cada ~100 ms
  o cada ~N MB, NO en cada chunk de 256 KB) con bytes acumulados del archivo actual + del total del
  plan. El `OpProgress` ya tiene `bytes_done/bytes_total/files_done/files_total/current`
  (`ops/mod.rs:93`); ahora se emite seguido.
- **Pausa real.** El `CancellationToken` (o un mecanismo hermano `PauseToken`) gana estado
  `paused`. En el loop de `copy_buffered`, si está pausado, el hilo espera (condvar / park, sin
  quemar CPU) hasta reanudar o cancelar, SIN cerrar el archivo. Métodos `pause()`/`resume()`.
- **Velocidad/ETA NO en el motor.** El motor solo reporta bytes; la UI deriva velocidad media+pico
  y ETA con muestras (bytes, timestamp). Mantener el core puro y testeable.
- **Robustez del parcial (Bug 3).** Al cancelar/fallar se borra el parcial (ya existe,
  `engine.rs:302-311`) — verificar robusto. El journal (ya existe) cubre el caso de proceso muerto:
  al arrancar, si hay un parcial de op no terminada, el modal "Retomar" (ya existe) ofrece
  retomar/limpiar.

**Testing (core):** `copy_buffered` emite ≥N progresos para un archivo multi-chunk; pausar detiene
el avance de bytes y reanudar lo continúa hasta completar; cancelar a media copia borra el parcial.
Todo con archivos sintéticos (~1 MB), determinista.

---

## Fase 3 — Bug 4/5: VM ampliado + panel de operaciones rico (UI)

**VM ampliado (`OpRowVm` en types.slint + `OpRowData` en ops_ctrl):** de 5 campos a todos —
`bytes_done`, `bytes_total`, `files_done`, `files_total`, `current_file`, `percent`, `speed_avg`,
`speed_peak`, `eta_secs`, `elapsed_secs`, `status` (en cola/en curso/pausada/esperando-decisión/
hecha/con-errores), desglose del resumen (copiados/saltados/fallidos). Render de piezas
reutilizables (fila de op, barra, bloque de datos).

**Panel acoplado nuevo (`PanePurpose::Operations`):**
- Se registra como purpose nuevo (como Tree/Preview/Inspector). Se añade al menú "Panel ▾".
- **Auto-aparece** al iniciar una op si no está en el layout. Este bloque implementa la variante
  **Panel acoplado** (el default). Las variantes `Modal` y `AlwaysVisible` de `Settings.ops_display`
  quedan FUERA de alcance de este bloque (no se renderizan aún; se mapearían en un bloque futuro).
- **Tres zonas** (del mockup aprobado):
  - **En curso:** ítem actual, barra global con %, "copiado X de Y" (12,4 GB / 94,9 GB), velocidad
    media + pico, transcurrido, restante (ETA), botones **Pausar / Saltar / Cancelar** (Reanudar
    cuando está pausada).
  - **Cola:** ops pendientes, cancelables.
  - **Historial reciente:** terminadas con su resultado (✓ N copiados · ⚠ M movidos, K saltados/
    fallidos). Tope ~20.
- Cifras: `core::format` para bytes; helpers nuevos para velocidad (MB/s) y tiempo (mm:ss / hh:mm).

**Controlador (`ops_ctrl`):** acumula muestras (bytes, timestamp) por op para derivar velocidad
media+pico y ETA; métodos `pause_op`/`resume_op`/`skip_op` (cancel_op ya existe); historial reciente.

**i18n triple es/en (sin voseo, sin reutilizar claves):** En curso / En cola / Historial / Pausar /
Reanudar / Saltar / Cancelar / "X de Y" / "~{t} restante" / "N copiados, M saltados, K fallidos".

---

## Fase 4 — Bug 2: drag&drop intra-app (drag nativo Slint)

- **Inicio del arrastre:** al arrastrar una fila, marcar un payload interno con `selected_paths()`.
  El arrastre nativo de Slint lleva el payload (NO OLE para intra-app).
- **Zona de suelte por panel:** cada panel Files recibe el drop; lee el payload y llama a
  `core::dnd::decide_drop_action(ctrl, shift, same_drive)` (ya existe) → mismo disco mueve, otro
  copia, Ctrl/Shift fuerzan → enruta a `ops::transfer` (motor robustecido en Fase 2).
- **Feedback visual:** el panel destino se resalta (borde de acento) mientras el arrastre pasa por
  encima.
- **No-op:** soltar sobre el mismo panel de origen no hace nada.
- **OLE hacia el SO se mantiene aparte** (sacar a Explorer sigue con OLE; conviven sin pisarse).
- **Riesgo (anotado):** el drag&drop es lo más delicado de Slint en render software. Si el drag
  nativo no detecta bien la zona destino, fallback de hit-testing por coordenadas de panel. Validar
  temprano en la implementación.

**Testing:** `decide_drop_action` ya tiene tests en core; el enrutado a `ops::transfer` se valida en
la VM (la mecánica de drag Slint no es testeable headless).

---

## Fase 5 — Cierre

Gate completo (fmt + test + clippy --workspace -D warnings), CHANGELOG, guía de usuario, dist.
**Verificación crítica en la VM:** copiar la VM de 95 GB real con conflicto+sobrescribir y confirmar
(a) progreso avanzando visible, (b) copia completa, (c) pausa/reanuda. Plus: Bug 1 (Ctrl+C→doble
clic navega), Bug 2 (arrastre panel→panel copia/mueve), Bug 5 (cola e historial).

## Orden de implementación (impacto/riesgo)

1. **Fase 1** (Bug 1): trivial, alta visibilidad.
2. **Fase 2** (core: progreso + pausa + robustez): desbloquea Bug 3 y es prerequisito del panel.
3. **Fase 3** (VM + panel rico): la vista sobre el motor robustecido.
4. **Fase 4** (drag&drop): el más nuevo e incierto, aislado y casi al final.
5. **Fase 5** (cierre + dist + verificación VM).

## Fuera de alcance (YAGNI)

- Editor de personalización del panel por el usuario (plantilla/tokens como el footer): se hace
  data-driven ahora, la personalización por usuario queda como posible bloque futuro.
- Ventana del SO separada para el progreso (se eligió panel acoplado).
- Reescritura del motor `core::ops` (está sano).
