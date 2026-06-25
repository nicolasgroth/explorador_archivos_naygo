# Pruebas automáticas de Naygo

> Naygo — explorador de archivos para Windows.
> Copyright (c) 2026 Nicolás Groth / ISGroth. Licencia MIT.

Este documento explica cómo correr la suite de pruebas completa de Naygo y qué cubre.
El objetivo es no perder funcionalidad ya construida entre entregas: cada flujo crítico
del sistema tiene pruebas automáticas que se ejecutan con un solo comando.

## Correr todo (un solo comando)

```powershell
scripts\test-all.ps1
```

El script ejecuta, en orden, las tres puertas de calidad que deben estar verdes antes de
cada commit, e imprime un resumen legible al final:

1. **Tests** — `cargo test --workspace` (todos los tests de todos los crates).
2. **Clippy** — `cargo clippy --workspace --all-targets -- -D warnings` (warnings = error).
3. **Formato** — `cargo fmt --all --check`.

Opciones:

| Opción       | Efecto                                                        |
|--------------|--------------------------------------------------------------|
| `-NoLint`    | Solo tests (salta clippy y fmt). Iteración rápida.           |
| `-FailFast`  | Se detiene en la primera puerta que falle.                  |

El script usa `CARGO_BUILD_JOBS=2` (la prioridad del proyecto es correr bien en equipos
modestos) y está escrito para PowerShell 5.1 (sin `&&` ni `||`). Devuelve código de salida
`0` si todo pasó y `1` si algo falló, así que sirve también para CI o para un hook de git.

### Solo los tests, a mano

```powershell
$env:CARGO_BUILD_JOBS = "2"; cargo test --workspace
```

Para un crate puntual:

```powershell
cargo test -p naygo-core             # lógica pura
cargo test -p naygo-ui-slint         # controladores de UI (sin ventana)
cargo test -p naygo-core --test flujos_ops   # solo los flujos end-to-end de operaciones
```

## Qué cubre la suite

Las pruebas se reparten en tres capas, igual que el código:

- **`naygo-core`** — lógica pura, 100 % testeable sin Windows ni UI. Es la mayor parte de
  la cobertura.
- **`naygo-platform`** — integración con Windows. Algunos tests están marcados `#[ignore]`
  porque requieren recursos reales del sistema operativo (papelera, discos, Shell); se
  corren a mano cuando hace falta.
- **`naygo-ui-slint`** — los **controladores** de la UI (`workspace_ctrl`, `ops_ctrl`,
  `config_ctrl`, `bridge`, `preview`), ejercitados **sin abrir una ventana real**. Lo
  puramente visual (render Slint) no es testeable en automático y se verifica a mano.

### Subsistemas con pruebas

| Subsistema | Qué se prueba |
|---|---|
| **Operaciones de archivo** | Copiar/mover/eliminar (permanente)/renombrar/crear, de punta a punta, verificando el estado **real en disco**. Plan + ejecución por el motor síncrono (`run_plan`) y por el motor en hilo (`spawn`). |
| **Conflictos** | Sobrescribir, saltar, renombrar (sufijo `(N)`), renombrar con nombre elegido (`RenameTo`), aplicar-a-todos. Conflicto de **carpeta** (fusionar/reemplazar/saltar/cancelar). |
| **Seguridad del motor** | Nunca borra un origen al "Reemplazar" (guardas cinturón-y-tirantes en `folder_conflicts` y en `run_plan`). El `pre_delete` jamás toca un origen ni un ancestro de origen. |
| **Cancelación** | Cancelar antes de empezar, durante el escaneo del plan y durante la espera. Una copia cancelada no deja basura. |
| **Batch-rename** | Plan ordenado por dependencia (corrimientos en cadena), rechazo de ciclos puros `a↔b`, ejecución real en disco. |
| **Deshacer** | Round-trip ejecutado: mover/renombrar → construir el inverso → re-emitir → verificar que el disco vuelve a su estado previo. Borrar no es deshacible (contrato v1). |
| **Drag & drop (lógica)** | `drop_at` enruta al panel bajo el cursor (no al activo); `move_hint` del OLE fuerza mover; mismo disco sin modificadores mueve por defecto; rubber-band; `same_drive` con rutas mixtas/UNC. |
| **Listado y vista** | Streaming, atributos ocultos/sistema, `is_visible` (filtro ocultos/sistema/dotfiles), `compute_view_indices` (filtro + orden + selección alineada), vista profunda recursiva, tamaño de carpeta, búsqueda recursiva. |
| **Navegación** | Historial atrás/adelante, favoritos (árbol de grupos anidados + migración del formato plano viejo), recientes, caché de carpetas. |
| **Config / Settings** | Persistencia round-trip (serde), migración de settings viejos a defaults nuevos, import/export de packs, temas (catálogo de 5, tema de usuario round-trip, `theme_slug`, `is_builtin_id`), i18n (paridad es/en). |
| **Selección** | `select_single`, `select_range`, `select_rect_range`, modificadores; archivos nuevos seleccionados al llegar. |
| **Estado de UI (controlador)** | `any_modal_open`, `pending_dialog`, cancelar conflicto, máquina de estados de la operación (Planning → conflicto → copia → fin), id estable de op tras reordenar la cola. |

### Dónde viven los tests

- La mayoría son módulos `#[cfg(test)]` **dentro** del archivo de producción que prueban
  (patrón estándar del repo). Por ejemplo, los tests del motor están en
  `crates/core/src/ops/engine.rs`, los del controlador de operaciones en
  `crates/ui-slint/src/ops_ctrl.rs`.
- Los **flujos compuestos end-to-end** de operaciones de archivo viven aparte, en
  `crates/core/tests/flujos_ops.rs` (integration test): arman el `OpRequest`, lo ejecutan
  por el motor y verifican el estado final en disco con carpetas temporales (`tempfile`).
  Es el lugar al que mirar para entender, de un vistazo, qué flujos de archivo están
  garantizados.

## Antes de cada commit

La regla del proyecto es: build limpio + tests pasando + clippy sin warnings + formato OK.
`scripts\test-all.ps1` verifica las tres cosas de una sola vez.
