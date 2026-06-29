# Expulsar USB con paneles abiertos — diseño

> Naygo — explorador de archivos. Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
> SPDX-License-Identifier: MIT
> Feature #2 de 4 pedidas el 2026-06-28. Rama `feat/iconos-personalizables`.

## Motivación

Hoy se puede expulsar un disco USB (`on_eject_drive` → modal de confirmación →
`eject_drive`), pero si un panel tiene una carpeta abierta en ese disco:

1. Windows suele rechazar la expulsión con "en uso" (`EjectOutcome::InUse`), porque
   el explorador mantiene handles (el `current_dir` del panel, posiblemente un watcher).
2. Aunque la expulsión funcione, los paneles quedan apuntando a una ruta muerta sin
   aviso claro.

Nicolás pidió: que el popup de expulsar avise si hay paneles con esa ruta abierta, y
que esos paneles se suelten/recarguen antes de desconectar (ofreciendo elegir otra
carpeta, como el aviso de "carpeta no encontrada" que ya existe).

## Objetivos

1. **Detectar y avisar**: el modal de confirmación de expulsión informa si hay paneles
   con carpetas abiertas en ese disco, listando las rutas.
2. **Soltar antes de expulsar**: al confirmar, esos paneles se "sueltan" del disco
   (pasan al aviso in-place "elegir otra carpeta") y sus watchers se cierran, ANTES de
   llamar a `eject_drive`. Así se evita el "en uso" causado por la propia app.
3. **Resultado honesto**: si la expulsión falla igual (proceso externo), avisar claro y
   dejar los paneles soltados (no revertir).

### No-objetivos (YAGNI)

- No se cierran los paneles completos (solo se sueltan de la carpeta).
- No se revierte el estado de los paneles si la expulsión falla.
- No se intenta forzar la expulsión ni matar procesos externos.

## Decisiones (brainstorming)

- Avisar + soltar/recargar antes de expulsar (combina las dos ideas de Nicolás).
- Los paneles soltados van al aviso in-place "elegir otra carpeta" (reusa el patrón
  existente de "carpeta no encontrada"), con texto adaptado.
- Si la expulsión falla tras soltar: avisar claro, dejar los paneles fuera (no revertir).

## Arquitectura

Respeta las 3 capas. La detección de paneles es lógica pura (testeable); el resto es
cableado de UI sobre mecanismos existentes.

### Capa `core` / `workspace_ctrl` (UI controller, pero con lógica testeable)

**`panes_on_drive(root: &Path) -> Vec<(PaneId, PathBuf)>`** (nuevo, en `workspace_ctrl.rs`).
Itera los paneles Files (patrón existente: `self.ws...filter_map(|p| p.files.as_ref()
.map(|f| (p.id, f.current_dir.clone())))`) y devuelve los cuya `current_dir` está DENTRO
del disco `root`. La comparación:
- **case-insensitive** (Windows: `E:\` == `e:\`).
- por **componente de ruta** / prefijo de raíz de volumen, no por substring (`E:\foto`
  está en `E:\`; `EE:\x` NO está en `E:\`). Implementación: normalizar ambas a la letra
  de unidad (primer componente) y comparar esa letra, o usar `starts_with` sobre
  componentes normalizados. Extraer un helper puro `path_is_on_drive(path, drive_root)
  -> bool` que sea testeable sin estado.

**`release_pane_from_drive(id: PaneId)`** (nuevo). Pone el panel `id` en el estado
"elegir otra carpeta" (el mismo aviso in-place que hoy decide `pane_dir_missing`), pero
FORZADO: no depende de que la carpeta ya no exista físicamente (la expulsión es inminente,
la carpeta aún existe en el instante de soltar). Marca el panel como "soltado por
expulsión" para que el builder del PaneVm muestre el aviso con el texto adaptado. También
debe soltar el watcher de ese panel (para no mantener un handle sobre el disco). Reusa la
maquinaria de `pane_dir_missing` / el builder del PaneVm; añade una bandera de estado
(p.ej. un `HashSet<PaneId>` de "ejected" o un campo en el estado del panel) que el builder
consulta junto a `pane_dir_missing`.

### Capa `ui-slint`

**`on_eject_drive` (main.rs ~3937)**: antes de armar el `MessageVm`, llamar a
`panes_on_drive(root)`. Si está vacío → modal actual sin cambios. Si tiene N paneles →
el `body` del modal incluye el aviso enriquecido (N + lista de rutas, recortada si son
muchas) y el `confirm_label` pasa a "Expulsar de todos modos". Guardar los paneles
afectados junto al `pending_eject` (para soltarlos en la confirmación).

**`on_message_confirm` (kind 1 = eject)**: al confirmar:
1. Para cada panel afectado (guardado): `release_pane_from_drive(id)`.
2. `sync` del layout para que el aviso in-place aparezca.
3. `eject_drive(root)`.
4. Según `EjectOutcome`:
   - `Ok` → toast `drive_eject_ok` (ya existe) o uno nuevo "puedes retirarlo".
   - `InUse` / `Failed` → aviso `drive_eject_in_use` adaptado: "sigue en uso por otro
     programa; reintenta o usa Quitar hardware de Windows". Los paneles quedan soltados.
   - `refresh_drives()` para actualizar la tira de discos.

### i18n

Claves nuevas (ES+EN, parity test `es_en_tienen_las_mismas_claves`):
- `drive.eject_with_panes`: cuerpo del modal cuando hay paneles ("El disco {drive} tiene
  {n} panel(es) con carpetas abiertas. Al expulsar, se cerrarán de su carpeta.").
- `drive.eject_anyway`: "Expulsar de todos modos".
- `pane.ejected_choose`: texto del aviso in-place tras soltar ("El disco fue expulsado.
  Elige otra carpeta.").
- `drive.eject_in_use_external`: aviso de fallo ("No se pudo expulsar: el disco sigue en
  uso por otro programa. Intenta de nuevo o usa «Quitar hardware con seguridad» de
  Windows."). (Puede reemplazar o complementar `drive_eject_in_use`.)
Texto neutral, sin voseo.

## Componentes y responsabilidades

| Unidad | Responsabilidad | Depende de |
|--------|-----------------|------------|
| `path_is_on_drive(path, drive_root)` | ¿una ruta está en un disco? (puro, case-insensitive, por componente) | — |
| `WorkspaceCtrl::panes_on_drive` | paneles Files cuyo dir está en el disco | path_is_on_drive |
| `WorkspaceCtrl::release_pane_from_drive` | soltar panel a "elegir carpeta" + cerrar watcher | estado del panel, watchers |
| builder del PaneVm | mostrar aviso in-place con texto "expulsado" | bandera ejected |
| `on_eject_drive` (main.rs) | enriquecer el modal con la lista de paneles | panes_on_drive |
| `on_message_confirm` (main.rs) | soltar paneles → expulsar → resultado | release_pane_from_drive, eject_drive |

## Errores

Conforme a "el filesystem es hostil" / "la app nunca cae":
- `panes_on_drive` con rutas raras (UNC, sin letra de unidad) → no incluye esos paneles
  (un share de red no está "en" un USB local). No panic.
- `release_pane_from_drive` sobre un id inválido → no-op.
- Expulsión fallida → aviso claro, estado consistente (paneles soltados, no revertidos).

## Testing

- `path_is_on_drive`: `E:\foto` en `E:\` (sí), `e:\x` en `E:\` (sí, case-insensitive),
  `C:\` en `E:\` (no), `EE:\x` en `E:\` (no), UNC `\\srv\share` en `E:\` (no), la raíz
  misma `E:\` en `E:\` (sí).
- `panes_on_drive`: con 2 paneles (uno en E:, uno en C:) y disco E: → devuelve solo el de E:.
- `release_pane_from_drive`: el panel queda en estado "elegir carpeta" (la bandera ejected
  se activa; el builder del PaneVm lo refleja). Verificable por el estado del controller.
- El flujo completo (modal enriquecido + soltar + expulsar + fallo) se verifica en vivo en
  la VM (requiere un USB real para el outcome de Windows).

## Trade-offs decididos

- **Soltar antes de expulsar** (vs solo avisar): evita el "en uso" causado por la propia
  app, que es la causa más común del rechazo. Más código pero resuelve el problema de raíz.
- **No revertir si falla**: simplicidad; el usuario quería sacar el disco, dejar los
  paneles fuera es coherente con esa intención.
- **Reusar el aviso in-place**: cero flujo nuevo de UI; el usuario ya conoce ese aviso.

## Fuera de alcance / fases futuras

- Las otras 2 mejoras pedidas (preview de ZIP, auditoría i18n + idiomas) van en specs
  separados.
- Forzar la expulsión / desmontar a la fuerza: no se hace (riesgo de corrupción).
