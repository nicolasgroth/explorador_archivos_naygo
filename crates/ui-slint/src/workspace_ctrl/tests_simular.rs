// Naygo — WorkspaceCtrl: pruebas «simular usuario» (gestos de punta a punta).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

//! SIMULAR USUARIO — tests de integración de gestos de punta a punta.
//!
//! Cada test crea archivos/carpetas REALES en un tempdir AISLADO (más un `config_dir`
//! propio), simula al usuario operándolos con ATAJOS DE TECLADO, CLIC/MOUSE y ARRASTRE
//! llamando al MISMO código del controlador (`WorkspaceCtrl` / `OpsCtrl`) que disparan esos
//! gestos en la app real (headless, sin abrir ventana) y verifica el resultado en DISCO.
//! Nada toca archivos del sistema del usuario: todo vive bajo `tempfile::tempdir()` y se borra
//! al caer del scope.
//!
//! Estos tests cierran el lazo gesto → controlador → motor de ops → filesystem. La resolución de
//! los modales (confirmar nombre, confirmar borrado, resolver conflicto) se hace por la API de
//! `OpsCtrl`, porque `on_key` SUSPENDE las acciones globales mientras hay un modal abierto (igual
//! que en la app: el teclado lo controla el modal Slint). Por eso el teclado simula el DISPARO del
//! gesto (p. ej. Ctrl+Shift+N abre el modal de carpeta nueva) y el "Aceptar" del modal va por
//! `name_confirm()` / `delete_confirm()` / `resolve_conflict()`, espejando el cableado de `main.rs`.

use super::*;
use naygo_core::ops::ConflictAction;

// --- Helpers (copiados del mod `tests` de arriba y de keys.rs: los mods hermanos no se ven
//     entre sí, así que se replican aquí para que esta suite sea autocontenida) ---

/// Drena los listados hasta que todos terminan (con timeout), simulando los ticks del Timer.
fn drain(c: &mut WorkspaceCtrl) -> bool {
    for _ in 0..2000 {
        if c.pump_listings() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

/// Drena las operaciones de archivo en curso hasta que TODAS terminan (summary presente) o se
/// agota el timeout. Devuelve true si terminaron limpio. NO resuelve modales: para flujos con
/// conflicto, usar `drain_ops_resolving`.
fn drain_ops(c: &mut WorkspaceCtrl) -> bool {
    for _ in 0..4000 {
        c.ops.pump_ops();
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
        {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

/// Posición de VISTA del ítem llamado `name` en el panel activo (índice contra `view_indices`,
/// que es el mismo que consumen el clic y el teclado).
fn active_pos_of(c: &WorkspaceCtrl, name: &str) -> Option<usize> {
    let f = c.ws.active_files()?;
    f.view_indices()
        .iter()
        .position(|&real| f.entries[real].name == name)
}

/// UX4: el resumen de nombres para el modal de confirmación de drop. Umbral: hasta 4 nombres
/// listados; a partir de 5, los primeros 4 + sufijo "y N más" (aquí simulado como " +N").
#[test]
fn pending_drop_names_summary_uno_pocos_muchos() {
    let mk = |paths: Vec<&str>| PendingDrop {
        paths: paths.into_iter().map(std::path::PathBuf::from).collect(),
        dest_dir: std::path::PathBuf::from("C:\\dest"),
        dest_pane: PaneId(0),
        is_move: false,
        count: 0,
    };
    // El sufijo de "y N más" lo simulamos con " +N" para no depender de i18n en el test.
    let more = |n: usize| format!(" +{}", n);

    // 1 elemento: solo su nombre entre comillas angulares.
    assert_eq!(mk(vec!["C:\\a\\foo.txt"]).names_summary(more), "«foo.txt»");

    // Pocos (3): todos listados, sin sufijo.
    assert_eq!(
        mk(vec!["C:\\a\\a.txt", "C:\\a\\b.txt", "C:\\a\\c.txt"]).names_summary(more),
        "«a.txt», «b.txt», «c.txt»"
    );

    // Justo en el umbral (4): los 4, sin sufijo.
    assert_eq!(
        mk(vec!["x\\1", "x\\2", "x\\3", "x\\4"]).names_summary(more),
        "«1», «2», «3», «4»"
    );

    // Muchos (6): primeros 4 + " +2" (quedan 2 sin nombrar).
    assert_eq!(
        mk(vec!["x\\1", "x\\2", "x\\3", "x\\4", "x\\5", "x\\6"]).names_summary(more),
        "«1», «2», «3», «4» +2"
    );
}

/// UX3: con `confirm_drop_between_panes = false`, soltar entre paneles NO deja un drop pendiente
/// esperando confirmación: ejecuta la op directo (no abre el modal kind 3). Pero si el archivo
/// YA EXISTE en el destino, el modal de CONFLICTO sigue apareciendo (son cosas distintas).
#[test]
fn drop_con_confirmacion_off_ejecuta_directo_y_sin_conflicto_copia() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(a.path());
    // Apagar la confirmación de drop.
    c.config.settings.confirm_drop_between_panes = false;
    let (origin, dest) = split_a(&mut c, b.path());
    let area = area();
    c.set_area(area);
    c.ws.set_active(origin);
    let (cx, cy) = pane_center(&c, area, dest);

    // Soltar (Ctrl = copia, determinista). Con la confirmación OFF NO debe quedar pendiente.
    let routed = c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false);
    assert!(
        routed,
        "drop_at enruta igual (devuelve true en ambos modos)"
    );
    assert!(
        c.pending_drop.is_none(),
        "confirmación OFF: no queda un drop esperando el modal kind 3"
    );
    assert!(
        !c.ops.active_ops.is_empty(),
        "confirmación OFF: la op arrancó directo"
    );
    // Sin conflicto (nombre libre en el destino): la op termina y el archivo aterriza.
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        b.path().join("doc.txt").exists(),
        "confirmación OFF, sin conflicto: la copia se ejecutó directo"
    );
}

/// UX3 (parte crítica): aunque la confirmación de drop esté APAGADA, el modal de CONFLICTO
/// (archivo que ya existe en el destino) DEBE seguir apareciendo. El motor se detiene en el
/// conflicto y no sobrescribe sin preguntar — confirmación de drop y conflicto son ortogonales.
#[test]
fn drop_con_confirmacion_off_pero_archivo_existente_pide_conflicto() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("doc.txt"), b"ORIGEN-nuevo").unwrap();
    std::fs::write(b.path().join("doc.txt"), b"DESTINO-viejo").unwrap();
    let (mut c, _cfg) = ctrl_en(a.path());
    c.config.settings.confirm_drop_between_panes = false;
    let (origin, dest) = split_a(&mut c, b.path());
    let area = area();
    c.set_area(area);
    c.ws.set_active(origin);
    let (cx, cy) = pane_center(&c, area, dest);

    // Soltar con la confirmación OFF → arranca directo, sin modal kind 3.
    assert!(c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false));
    assert!(c.pending_drop.is_none(), "no hay confirmación de drop");

    // El motor debe DETENERSE en el conflicto, no sobrescribir.
    let mut pidio_conflicto = false;
    for _ in 0..4000 {
        c.ops.pump_ops();
        if matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::Conflict { .. })
        ) {
            pidio_conflicto = true;
            break;
        }
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        pidio_conflicto,
        "confirmación de drop OFF NO debe saltarse el conflicto: el archivo existe → preguntar"
    );
    assert_eq!(
        std::fs::read_to_string(b.path().join("doc.txt")).unwrap(),
        "DESTINO-viejo",
        "el destino no se toca hasta que el usuario resuelva el conflicto"
    );
}

/// REGRESIÓN (reportado por Nicolás): arrastrar un archivo a otro panel donde YA EXISTE, y
/// confirmar el drop, debe DETENERSE en el conflicto (preguntar) — no sobrescribir en silencio.
/// Reproduce el flujo completo drop_at → confirm_pending_drop → motor con first_collision.
/// Distingue si el bug está en el MOTOR (este test falla) o solo en la UI/timing (pasa).
#[test]
fn drop_sobre_archivo_existente_confirmado_pide_conflicto_no_sobrescribe() {
    let cfg = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    // El MISMO nombre existe en ambos lados, con contenido DISTINTO.
    std::fs::write(a.path().join("doc.txt"), b"ORIGEN-nuevo").unwrap();
    std::fs::write(b.path().join("doc.txt"), b"DESTINO-viejo").unwrap();
    let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let origin = c.ws.active_id().unwrap();
    let dest = c.split_for_target().unwrap();
    c.open_in_pane(dest, b.path().to_path_buf());
    assert!(drain(&mut c));
    let area = area();
    c.set_area(area);
    c.ws.set_active(origin);
    let (cx, cy) = pane_center(&c, area, dest);

    // Soltar el archivo sobre el destino (Ctrl=copia, determinista). UN SOLO POPUP: como HAY
    // conflicto (doc.txt ya existe en el destino), `drop_at` NO deja un drop pendiente de
    // confirmación — ejecuta directo y va al diálogo de CONFLICTO (que ya es la confirmación).
    assert!(c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false));
    assert!(
        c.pending_drop.is_none(),
        "con conflicto NO se pide confirmación de drop (un solo popup: el de conflicto)"
    );

    // Drenar las ops hasta que el motor PIDA el conflicto (pending_dialog = Conflict). Si en
    // vez de eso la op termina (summary) SIN preguntar, es el bug: sobrescribió en silencio.
    let mut pidio_conflicto = false;
    for _ in 0..4000 {
        c.ops.pump_ops();
        if matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::Conflict { .. })
        ) {
            pidio_conflicto = true;
            break;
        }
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
        {
            break; // terminó sin preguntar
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    assert!(
        pidio_conflicto,
        "copiar sobre un archivo que YA existe debe DETENERSE en el conflicto, no sobrescribir"
    );
    // El destino NO debe haberse pisado mientras se espera la decisión.
    assert_eq!(
        std::fs::read_to_string(b.path().join("doc.txt")).unwrap(),
        "DESTINO-viejo",
        "el destino no se toca hasta que el usuario decida"
    );
}

/// UN SOLO POPUP COHERENTE (decisión de Nicolás): con la confirmación de drop ENCENDIDA, un
/// drop SIN choque deja un drop pendiente (sale el modal "¿Copiar?"); un drop CON choque NO lo
/// deja (va directo al diálogo de conflicto, sin cadena de dos popups).
#[test]
fn drop_un_solo_popup_segun_haya_conflicto() {
    let cfg = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("nuevo.txt"), b"x").unwrap();
    std::fs::write(a.path().join("choca.txt"), b"x").unwrap();
    std::fs::write(b.path().join("choca.txt"), b"ya-existe").unwrap();
    let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
    // Confirmación de drop ENCENDIDA (default).
    assert!(c.config.settings.confirm_drop_between_panes);
    assert!(drain(&mut c));
    let origin = c.ws.active_id().unwrap();
    let dest = c.split_for_target().unwrap();
    c.open_in_pane(dest, b.path().to_path_buf());
    assert!(drain(&mut c));
    let area = area();
    c.set_area(area);
    c.ws.set_active(origin);
    let (cx, cy) = pane_center(&c, area, dest);

    // Drop SIN choque → deja pending_drop (sale el modal de confirmación).
    assert!(c.drop_at(cx, cy, true, false, vec![a.path().join("nuevo.txt")], false));
    assert!(
        c.pending_drop.is_some(),
        "sin conflicto + confirmación ON → se pide confirmar el drop"
    );
    c.cancel_pending_drop();

    // Drop CON choque → NO deja pending_drop (va directo al conflicto, un solo popup).
    assert!(c.drop_at(cx, cy, true, false, vec![a.path().join("choca.txt")], false));
    assert!(
        c.pending_drop.is_none(),
        "con conflicto → sin confirmación de drop (el conflicto es la confirmación)"
    );
}

/// El char unicode de una tecla especial de Slint, como String (lo que llega a `on_key`).
/// Copiado de keys.rs:92.
fn key_char(k: slint::platform::Key) -> String {
    let s: slint::SharedString = k.into();
    s.to_string()
}

/// Área de trabajo conocida y estable para el hit-testing de paneles (clic/arrastre).
fn area() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    }
}

/// Centro (cx, cy) del rect del panel `id` dentro de `a` (para apuntar clic/drop a ESE panel).
fn pane_center(c: &WorkspaceCtrl, a: Rect, id: PaneId) -> (f32, f32) {
    let (_, r) = c
        .pane_rects(a)
        .into_iter()
        .find(|(p, _)| *p == id)
        .expect("el panel tiene rect");
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// Arranca un controlador AISLADO apuntando a `start`, con un `config_dir` temporal propio, y
/// drena el primer listado. Devuelve `(ctrl, tmp_cfg)`; el `tmp_cfg` se retiene para que el dir
/// no se borre antes de tiempo.
fn ctrl_en(start: &std::path::Path) -> (WorkspaceCtrl, tempfile::TempDir) {
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(start.to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c), "el listado inicial debe terminar");
    (c, cfg)
}

/// Abre un SEGUNDO panel apuntando a `dir` (split) y deja el ORIGEN activo. Devuelve
/// `(origin_id, dest_id)`. Espeja `split_for_target` + `open_in_pane`, el camino de "abrir en
/// otro panel".
fn split_a(c: &mut WorkspaceCtrl, dir: &std::path::Path) -> (PaneId, PaneId) {
    let origin = c.ws.active_id().unwrap();
    let dest = c
        .split_for_target()
        .expect("se pudo dividir en dos paneles");
    c.open_in_pane(dest, dir.to_path_buf());
    assert!(drain(c), "el listado del segundo panel debe terminar");
    c.ws.set_active(origin);
    (origin, dest)
}

// ============================ 1. Crear carpeta con Ctrl+Shift+N ============================

/// GESTO: el usuario pulsa Ctrl+Shift+N (atajo de "nueva carpeta"), escribe el nombre y
/// confirma. RESULTADO: la carpeta existe en disco. Cubre on_key(chord NewDir) → modal
/// NameInput(NewDir) → name_changed → name_confirm → motor → refresh.
#[test]
fn crear_carpeta_con_ctrl_shift_n() {
    let work = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());

    // Atajo Ctrl+Shift+N: abre el modal de nombre para NUEVA CARPETA.
    c.on_key("n", true, true, false);
    assert!(
        matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::NameInput {
                purpose: crate::ops_ctrl::NamePurpose::NewDir,
                ..
            })
        ),
        "Ctrl+Shift+N debe abrir el modal de NUEVA CARPETA"
    );

    // El usuario escribe el nombre y confirma (Aceptar / Enter del modal).
    c.ops.name_changed("Documentos".into());
    c.ops.name_confirm("Comprimir");
    assert!(drain_ops(&mut c), "la creación de la carpeta debe terminar");
    c.refresh_active();
    assert!(drain(&mut c));

    let creada = work.path().join("Documentos");
    assert!(creada.is_dir(), "la carpeta nueva debe existir en disco");
    assert!(
        active_pos_of(&c, "Documentos").is_some(),
        "la carpeta nueva aparece en la vista tras refrescar"
    );
}

// ===================== 1b. Refrescar (F5) NO duplica las filas =============================

/// REGRESIÓN (bug reportado por Nicolás): al pulsar F5 sobre un panel ya poblado, las filas
/// se DUPLICABAN porque `pump_listings` hacía `entries.extend(batch)` sin vaciar primero las
/// entries previas. RESULTADO esperado: tras refrescar, el panel tiene EXACTAMENTE los mismos
/// archivos que el disco, sin copias. Cubre el flag `Listing::fresh` + el `clear()` del primer
/// lote en `pump_listings`.
#[test]
fn refrescar_f5_no_duplica_las_filas() {
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"a").unwrap();
    std::fs::write(work.path().join("b.txt"), b"b").unwrap();
    std::fs::create_dir(work.path().join("sub")).unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());

    let n0 = c.ws.active_files().unwrap().entries.len();
    assert_eq!(n0, 3, "el listado inicial trae los 3 ítems");

    // Refrescar varias veces (F5): cada refresco debe REEMPLAZAR, no acumular.
    for _ in 0..3 {
        assert!(c.refresh_active(), "F5 inicia el re-listado");
        assert!(drain(&mut c), "el re-listado termina");
        assert_eq!(
            c.ws.active_files().unwrap().entries.len(),
            3,
            "refrescar NO debe duplicar: siguen siendo 3 ítems"
        );
    }
}

/// REGRESIÓN: refrescar una carpeta que QUEDÓ VACÍA (todos sus ítems se borraron desde fuera)
/// debe dejar el panel vacío, no con las filas viejas. Cubre el `clear()` de la rama `done`
/// cuando el listado nuevo no emitió ningún lote.
#[test]
fn refrescar_carpeta_que_quedo_vacia_limpia_las_filas() {
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"a").unwrap();
    std::fs::write(work.path().join("b.txt"), b"b").unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());
    assert_eq!(c.ws.active_files().unwrap().entries.len(), 2);

    // Se borran los archivos por fuera y se refresca.
    std::fs::remove_file(work.path().join("a.txt")).unwrap();
    std::fs::remove_file(work.path().join("b.txt")).unwrap();
    assert!(c.refresh_active());
    assert!(drain(&mut c));

    assert_eq!(
        c.ws.active_files().unwrap().entries.len(),
        0,
        "tras refrescar, la carpeta vacía no debe conservar las filas viejas"
    );
}

// =============== 1c. Ciclo operación → refresh NO deja la vista inconsistente ===============

/// REGRESIÓN integral del bug de F5 en su escenario REAL: el usuario opera (crea/copia/borra)
/// y luego refresca. Crear varios archivos y refrescar varias veces NO debe duplicar ni perder
/// filas: la vista siempre refleja EXACTAMENTE lo que hay en disco.
#[test]
fn crear_y_refrescar_repetido_mantiene_la_vista_consistente() {
    let work = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());
    assert_eq!(c.ws.active_files().unwrap().entries.len(), 0);

    // Crear 4 archivos por fuera y refrescar: la vista debe tener exactamente 4.
    for i in 0..4 {
        std::fs::write(work.path().join(format!("f{i}.txt")), b"x").unwrap();
    }
    assert!(c.refresh_active());
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().entries.len(), 4);

    // Refrescar 5 veces más sin cambios: sigue en 4 (no acumula).
    for _ in 0..5 {
        assert!(c.refresh_active());
        assert!(drain(&mut c));
    }
    assert_eq!(
        c.ws.active_files().unwrap().entries.len(),
        4,
        "refrescos repetidos no deben duplicar ni perder filas"
    );

    // Agregar uno más y refrescar: pasa a 5.
    std::fs::write(work.path().join("f4.txt"), b"x").unwrap();
    assert!(c.refresh_active());
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().entries.len(), 5);
}

/// REGRESIÓN: copiar un archivo al OTRO panel y luego refrescar el panel DESTINO no debe
/// duplicar la fila recién llegada. (El destino se re-lista tras la operación; este test cierra
/// el ciclo operación-en-un-panel → refresh-del-otro.)
#[test]
fn copiar_al_otro_panel_y_refrescar_destino_no_duplica() {
    let cfg = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let origin = c.ws.active_id().unwrap();
    let dest = c.split_for_target().unwrap();
    c.open_in_pane(dest, b.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    c.ws.set_active(origin);

    // Copiar al otro panel.
    c.ws.active_files_mut().unwrap().select_all();
    c.op_to_other(false);
    assert!(drain_ops(&mut c));

    // Refrescar el destino dos veces: la fila copiada aparece UNA sola vez.
    c.ws.set_active(dest);
    for _ in 0..2 {
        assert!(c.refresh_active());
        assert!(drain(&mut c));
    }
    let dest_entries = c.ws.active_files().unwrap().entries.len();
    assert_eq!(
        dest_entries, 1,
        "el destino tiene 1 archivo, no duplicado tras refrescar"
    );
}

/// REGRESIÓN: eliminar un archivo (permanente, en el tempdir) y refrescar debe BAJAR el conteo,
/// no dejar la fila fantasma. Cierra el ciclo eliminar → refresh.
#[test]
fn eliminar_y_refrescar_baja_el_conteo() {
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("uno.txt"), b"x").unwrap();
    std::fs::write(work.path().join("dos.txt"), b"x").unwrap();
    std::fs::write(work.path().join("tres.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());
    assert_eq!(c.ws.active_files().unwrap().entries.len(), 3);

    // Seleccionar "uno.txt" y eliminarlo permanente (queda en el tempdir, no en la papelera).
    let pos = active_pos_of(&c, "uno.txt").expect("uno.txt está en la vista");
    c.ws.active_files_mut().unwrap().select_single(pos);
    c.op_delete(true);
    c.ops.delete_confirm();
    assert!(drain_ops(&mut c));
    c.refresh_active();
    assert!(drain(&mut c));

    assert_eq!(
        c.ws.active_files().unwrap().entries.len(),
        2,
        "tras eliminar y refrescar, quedan 2 archivos"
    );
    assert!(
        active_pos_of(&c, "uno.txt").is_none(),
        "el archivo eliminado ya no está en la vista"
    );
}

// ============================== 2. Crear archivo con Ctrl+N ===============================

/// GESTO: Ctrl+N (atajo "nuevo archivo"), nombre, confirmar. RESULTADO: el archivo existe.
#[test]
fn crear_archivo_con_ctrl_n() {
    let work = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());

    c.on_key("n", true, false, false); // Ctrl+N → nuevo archivo
    assert!(
        matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::NameInput {
                purpose: crate::ops_ctrl::NamePurpose::NewFile,
                ..
            })
        ),
        "Ctrl+N debe abrir el modal de NUEVO ARCHIVO"
    );

    c.ops.name_changed("apuntes.txt".into());
    c.ops.name_confirm("Comprimir");
    assert!(drain_ops(&mut c), "la creación del archivo debe terminar");
    c.refresh_active();
    assert!(drain(&mut c));

    let creado = work.path().join("apuntes.txt");
    assert!(creado.is_file(), "el archivo nuevo debe existir en disco");
}

// ===================== 3. Crear varias carpetas anidadas (a\b\c) ==========================

/// GESTO: abrir el cuadro "nueva(s) carpeta(s)" (botón de la toolbar) y escribir varias líneas,
/// cada una una carpeta SUELTA, y aplicar. RESULTADO: todas existen en disco. Cubre el alta
/// MULTILÍNEA (varias carpetas de una vez), que es el camino distinto del modal de nombre simple.
#[test]
fn crear_varias_carpetas_multilinea() {
    let work = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());

    c.new_folder_open_active();
    assert!(
        c.new_folder_open(),
        "el cuadro de nuevas carpetas debe abrir"
    );
    // Tres carpetas en tres líneas (más una línea en blanco, que se ignora).
    c.new_folder_set_text("alfa\nbeta\n\ngamma");
    assert_eq!(
        c.new_folder_counts(),
        (3, 0),
        "tres líneas válidas (la vacía no cuenta)"
    );
    c.new_folder_apply();
    assert!(drain_ops(&mut c), "la creación múltiple debe terminar");

    for n in ["alfa", "beta", "gamma"] {
        assert!(
            work.path().join(n).is_dir(),
            "la carpeta '{n}' de la línea correspondiente debe existir"
        );
    }
}

/// GESTO: en el cuadro de nuevas carpetas, escribir una RUTA anidada con separadores
/// (`a\b\c`) y aplicar. RESULTADO: la cadena anidada existe en disco.
///
/// REGRESIÓN ARREGLADA. Antes `ops::plan()` validaba el nombre completo con
/// `ops::names::is_valid_name`, que PROHÍBE el `\` (está en FORBIDDEN), así que la ruta
/// relativa anidada (`nivel1\nivel2\nivel3`) caía en `Err(InvalidName)` y `start_op` la
/// descartaba en silencio. Ahora el plan de `CreateDir` valida POR COMPONENTE
/// (`names::relative_components`, que rechaza `.`/`..`/vacío/absoluta) y arma el destino
/// uniendo cada componente sobre la carpeta destino; el motor lo crea con `create_dir_all`.
#[test]
fn crear_carpetas_anidadas_con_separadores() {
    let work = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());

    c.new_folder_open_active();
    c.new_folder_set_text("nivel1\\nivel2\\nivel3");
    assert_eq!(
        c.new_folder_counts(),
        (1, 0),
        "la línea anidada se considera válida en el parseo del cuadro"
    );
    c.new_folder_apply();
    assert!(drain_ops(&mut c), "la creación anidada debe terminar");

    let anidada = work.path().join("nivel1").join("nivel2").join("nivel3");
    assert!(
        anidada.is_dir(),
        "la cadena de carpetas anidadas debe existir: {}",
        anidada.display()
    );
}

// ===================== 4. Mover archivo entre 2 paneles con F6 ============================

/// GESTO: con dos paneles, seleccionar un archivo en el origen y pulsar F6 (mover al otro
/// panel). RESULTADO: el archivo está en el destino y YA NO en el origen.
#[test]
fn mover_archivo_al_otro_panel_con_f6() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("informe.txt"), b"contenido").unwrap();
    let (mut c, _cfg) = ctrl_en(src.path());
    let (origin, _dest) = split_a(&mut c, dst.path());
    c.set_area(area()); // F6 resuelve el otro panel con `last_area`: fijarla hace el test determinista

    // Seleccionar el archivo en el panel origen (clic de fila simple).
    c.ws.set_active(origin);
    let pos = active_pos_of(&c, "informe.txt").unwrap();
    c.on_row_clicked(origin, pos, std::time::Instant::now());

    // F6 = MoveToOther. Con dos paneles, el destino se resuelve directo y la op arranca.
    c.on_key(&key_char(slint::platform::Key::F6), false, false, false);
    assert!(drain_ops(&mut c), "el movimiento debe terminar");

    assert!(
        dst.path().join("informe.txt").exists(),
        "el archivo aterrizó en el panel destino"
    );
    assert!(
        !src.path().join("informe.txt").exists(),
        "el archivo se MOVIÓ (ya no está en el origen)"
    );
}

// ===================== 5. Copiar archivo entre 2 paneles =================================

/// GESTO: con dos paneles, seleccionar un archivo y disparar "copiar al otro panel"
/// (`op_to_other(false)`, la acción CopyToOther). RESULTADO: el archivo está en AMBOS paneles.
#[test]
fn copiar_archivo_al_otro_panel() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("foto.txt"), b"data").unwrap();
    let (mut c, _cfg) = ctrl_en(src.path());
    let (origin, _dest) = split_a(&mut c, dst.path());
    c.set_area(area()); // que `op_to_other` resuelva el otro panel por geometría (como en la app)

    c.ws.set_active(origin);
    let pos = active_pos_of(&c, "foto.txt").unwrap();
    c.on_row_clicked(origin, pos, std::time::Instant::now());

    // Copiar al otro panel (CopyToOther no trae atajo por defecto; se dispara por el método,
    // igual que el botón/menú lo cablea en main.rs). `op_to_other` devuelve false para Transfer
    // por diseño (no arranca un LISTADO), así que la op se verifica por el resultado en disco.
    c.op_to_other(false);
    assert!(drain_ops(&mut c), "la copia debe terminar");

    assert!(
        dst.path().join("foto.txt").exists(),
        "la copia aterrizó en el destino"
    );
    assert!(
        src.path().join("foto.txt").exists(),
        "el original sigue en el origen (es COPIA, no movimiento)"
    );
}

// ===================== 6. Mover archivo arrastrando (drop con move_hint) ==================

/// GESTO: arrastrar un archivo del origen y SOLTARLO sobre el centro del panel destino, con el
/// `move_hint` del OLE (Shift al soltar) → mover. RESULTADO: el archivo está en el destino y no
/// en el origen. Apunta el drop por hit-testing al rect del panel destino (como en la app).
#[test]
fn mover_archivo_arrastrando_al_panel_destino() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("clip.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(src.path());
    let (_origin, dest) = split_a(&mut c, dst.path());

    let a = area();
    c.set_area(a);
    let (cx, cy) = pane_center(&c, a, dest);

    // Soltar sobre el panel destino con move_hint=true (Shift real del OLE) → MOVER.
    let routed = c.drop_at(
        cx,
        cy,
        false,
        false,
        vec![src.path().join("clip.txt")],
        true,
    );
    assert!(routed, "el drop debe enrutar al panel destino");
    // CONFIRMAR AL SOLTAR (PUNTO 1b): el drop entre paneles ahora pide confirmación antes de
    // ejecutar; lo confirmamos (equivale a pulsar Mover en el modal).
    assert!(c.confirm_pending_drop(), "confirmar arranca el movimiento");
    assert!(
        drain_ops(&mut c),
        "el movimiento por arrastre debe terminar"
    );

    assert!(
        dst.path().join("clip.txt").exists(),
        "el archivo arrastrado aterrizó en el destino"
    );
    assert!(
        !src.path().join("clip.txt").exists(),
        "el archivo se MOVIÓ por el arrastre"
    );
}

// ============== 7. Arrastrar en el mismo disco sin modificadores = mover ==================

/// GESTO: soltar SIN Ctrl/Shift ni move_hint sobre el panel destino, en el MISMO disco
/// (tempdirs del mismo volumen). RESULTADO: regla del Explorador → MUEVE por defecto.
#[test]
fn arrastrar_mismo_disco_sin_modificadores_mueve() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("nota.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(src.path());
    let (_origin, dest) = split_a(&mut c, dst.path());

    let a = area();
    c.set_area(a);
    let (cx, cy) = pane_center(&c, a, dest);

    // ctrl=false, shift=false, move_hint=false → mismo disco → Mover.
    let routed = c.drop_at(
        cx,
        cy,
        false,
        false,
        vec![src.path().join("nota.txt")],
        false,
    );
    assert!(routed, "el drop debe enrutar");
    // CONFIRMAR AL SOLTAR (PUNTO 1b): el drop entre paneles pide confirmación antes de ejecutar.
    assert!(c.confirm_pending_drop(), "confirmar arranca el movimiento");
    assert!(drain_ops(&mut c), "la operación debe terminar");

    assert!(
        dst.path().join("nota.txt").exists(),
        "aterrizó en el destino"
    );
    assert!(
        !src.path().join("nota.txt").exists(),
        "mismo disco sin modificadores: se MOVIÓ"
    );
}

// ============== 8. Eliminar permanente con Shift+Supr ====================================

/// GESTO: seleccionar una fila y pulsar Shift+Supr (borrado PERMANENTE), luego confirmar.
/// RESULTADO: el archivo ya no existe en disco. Se usa `permanent` para que el borrado quede
/// DENTRO del tempdir (no toca la papelera real del SO; aislado e irreversible pero seguro).
#[test]
fn eliminar_permanente_con_shift_supr() {
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("basura.txt"), b"x").unwrap();
    std::fs::write(work.path().join("queda.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());
    let id = c.ws.active_id().unwrap();

    // Seleccionar la fila a borrar (clic simple).
    let pos = active_pos_of(&c, "basura.txt").unwrap();
    c.on_row_clicked(id, pos, std::time::Instant::now());

    // Shift+Supr → DeletePermanent: abre el modal de confirmación de borrado permanente.
    c.on_key(&key_char(slint::platform::Key::Delete), false, true, false);
    assert!(
        matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
                permanent: true,
                ..
            })
        ),
        "Shift+Supr debe pedir confirmación de borrado PERMANENTE"
    );

    // Confirmar el borrado (botón Eliminar del modal).
    c.ops.delete_confirm();
    assert!(drain_ops(&mut c), "el borrado debe terminar");

    assert!(
        !work.path().join("basura.txt").exists(),
        "el archivo borrado ya no existe en disco"
    );
    assert!(
        work.path().join("queda.txt").exists(),
        "el otro archivo NO se tocó"
    );
}

// ============== 9. Seleccionar todo con Ctrl+A ===========================================

/// GESTO: Ctrl+A (seleccionar todo). RESULTADO: la selección abarca TODAS las filas de la vista.
#[test]
fn seleccionar_todo_con_ctrl_a() {
    let work = tempfile::tempdir().unwrap();
    for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        std::fs::write(work.path().join(n), b"x").unwrap();
    }
    let (mut c, _cfg) = ctrl_en(work.path());
    let total = c.ws.active_files().unwrap().view_len();
    assert_eq!(total, 4, "precondición: 4 filas en la vista");

    c.on_key("a", true, false, false); // Ctrl+A → SelectAll

    let f = c.ws.active_files().unwrap();
    assert_eq!(
        f.selection_count(),
        total,
        "Ctrl+A selecciona todas las filas de la vista"
    );
    assert_eq!(
        c.selected_paths().len(),
        total,
        "todas las rutas quedan en la selección efectiva"
    );
}

// ============== 10. Selección por rectángulo (rubber-band) ===============================

/// GESTO: arrastre de selección por rectángulo sobre un rango de filas (`select_rect_range`).
/// RESULTADO: queda seleccionado exactamente ese rango inclusivo, y nada fuera de él.
#[test]
fn seleccion_por_rectangulo_marca_un_rango() {
    let work = tempfile::tempdir().unwrap();
    // Nombres con prefijo numérico para un orden estable y predecible en la vista.
    for n in ["1.txt", "2.txt", "3.txt", "4.txt", "5.txt"] {
        std::fs::write(work.path().join(n), b"x").unwrap();
    }
    let (mut c, _cfg) = ctrl_en(work.path());
    let id = c.ws.active_id().unwrap();
    assert_eq!(c.ws.active_files().unwrap().view_len(), 5);

    // Rubber-band desde la fila 1 hasta la 3 (inclusive), sin Ctrl (reemplaza la selección).
    c.select_rect_range(id, 1, 3, false);

    let f = c.ws.active_files().unwrap();
    assert_eq!(
        f.selection_count(),
        3,
        "el rectángulo abarca 3 filas (1..=3)"
    );
    assert!(!f.is_selected(0), "la fila 0 queda fuera del rango");
    assert!(f.is_selected(1) && f.is_selected(2) && f.is_selected(3));
    assert!(!f.is_selected(4), "la fila 4 queda fuera del rango");
}

// ============== 11. Doble clic en carpeta + Backspace para volver ========================

/// GESTO: doble clic sobre una CARPETA (entra) y luego Backspace (sube/vuelve). RESULTADO: el
/// panel cambió a la subcarpeta y volvió a la carpeta original. (Doble clic SOLO sobre carpetas
/// en tests: sobre un archivo abriría el programa del SO.)
#[test]
fn navegar_con_doble_clic_y_volver_con_backspace() {
    let work = tempfile::tempdir().unwrap();
    let sub = work.path().join("subcarpeta");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("dentro.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());
    let id = c.ws.active_id().unwrap();
    assert_eq!(c.active_dir().as_deref(), Some(work.path()));

    // Doble clic sobre la carpeta → navega dentro.
    let pos = active_pos_of(&c, "subcarpeta").unwrap();
    c.on_row_double_clicked(id, pos);
    assert!(drain(&mut c), "el listado de la subcarpeta debe terminar");
    assert_eq!(
        c.active_dir().as_deref(),
        Some(sub.as_path()),
        "el doble clic entró a la subcarpeta"
    );

    // Backspace = GoUp → vuelve a la carpeta de arriba.
    c.on_key(
        &key_char(slint::platform::Key::Backspace),
        false,
        false,
        false,
    );
    assert!(drain(&mut c), "el listado al volver debe terminar");
    assert_eq!(
        c.active_dir().as_deref(),
        Some(work.path()),
        "Backspace volvió a la carpeta original"
    );
}

// ============== 12. Conflicto al mover: Sobrescribir =====================================

/// GESTO: mover un archivo a un panel donde YA existe ese nombre → aparece el conflicto por
/// ítem → el usuario elige "Sobrescribir". RESULTADO: el contenido del origen queda en el
/// destino (pisó al viejo). Cubre el lazo conflicto → resolve_conflict(Overwrite).
#[test]
fn conflicto_al_mover_sobrescribir() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("dato.txt"), b"NUEVO").unwrap();
    std::fs::write(dst.path().join("dato.txt"), b"VIEJO").unwrap();
    let (mut c, _cfg) = ctrl_en(src.path());
    let (origin, _dest) = split_a(&mut c, dst.path());
    c.set_area(area());

    c.ws.set_active(origin);
    let pos = active_pos_of(&c, "dato.txt").unwrap();
    c.on_row_clicked(origin, pos, std::time::Instant::now());

    // Mover al otro panel (F6 / op_to_other(true)): arranca la op, que chocará en el destino.
    // (op_to_other devuelve false para Transfer por diseño; el efecto se ve en el conflicto/disco.)
    c.op_to_other(true);

    // Bucle de drenado que resuelve el conflicto por ítem con Sobrescribir en cuanto aparece.
    let mut resuelto = false;
    let mut termino = false;
    for _ in 0..4000 {
        c.ops.pump_ops();
        if !resuelto {
            if let Some(crate::ops_ctrl::OpDialog::Conflict { op_id, .. }) =
                c.ops.pending_dialog.clone()
            {
                c.ops
                    .resolve_conflict(op_id, ConflictAction::Overwrite, false);
                resuelto = true;
            }
        }
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
        {
            termino = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(resuelto, "debe aparecer y resolverse el conflicto");
    assert!(termino, "la operación debe terminar tras resolver");

    assert_eq!(
        std::fs::read_to_string(dst.path().join("dato.txt")).unwrap(),
        "NUEVO",
        "Sobrescribir dejó el contenido del ORIGEN en el destino"
    );
    assert!(
        !src.path().join("dato.txt").exists(),
        "al mover, el origen desaparece tras resolver"
    );
}

// ============== 13. Conflicto al mover: Renombrar con nombre nuevo =======================

/// GESTO: mover un archivo a un panel donde ya existe el nombre → conflicto → el usuario elige
/// un NOMBRE NUEVO (RenameTo). RESULTADO: existe el nombre nuevo en el destino con el contenido
/// del origen y el archivo ORIGINAL del destino queda intacto.
#[test]
fn conflicto_al_mover_renombrar_con_nombre_nuevo() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("doc.txt"), b"ORIGEN").unwrap();
    std::fs::write(dst.path().join("doc.txt"), b"DESTINO").unwrap();
    let (mut c, _cfg) = ctrl_en(src.path());
    let (origin, _dest) = split_a(&mut c, dst.path());
    c.set_area(area());

    c.ws.set_active(origin);
    let pos = active_pos_of(&c, "doc.txt").unwrap();
    c.on_row_clicked(origin, pos, std::time::Instant::now());
    // op_to_other devuelve false para Transfer por diseño; el efecto se ve en el conflicto/disco.
    c.op_to_other(true);

    let mut resuelto = false;
    let mut termino = false;
    for _ in 0..4000 {
        c.ops.pump_ops();
        if !resuelto {
            if let Some(crate::ops_ctrl::OpDialog::Conflict { op_id, .. }) =
                c.ops.pending_dialog.clone()
            {
                c.ops.resolve_conflict(
                    op_id,
                    ConflictAction::RenameTo("doc-copia.txt".into()),
                    false,
                );
                resuelto = true;
            }
        }
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some())
        {
            termino = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(resuelto, "debe aparecer y resolverse el conflicto");
    assert!(termino, "la operación debe terminar tras resolver");

    assert_eq!(
        std::fs::read_to_string(dst.path().join("doc-copia.txt")).unwrap(),
        "ORIGEN",
        "el archivo movido quedó con el nombre nuevo y el contenido del origen"
    );
    assert_eq!(
        std::fs::read_to_string(dst.path().join("doc.txt")).unwrap(),
        "DESTINO",
        "el archivo original del destino quedó INTACTO"
    );
}

// ============== 14. move_hint fuerza mover aunque ctrl/shift lleguen false ================

/// REGRESIÓN (bug del Shift): durante el bucle modal del OLE, los flags de teclado de la app
/// llegan en false porque el modal se traga los eventos. El `move_hint` (Shift REAL al soltar,
/// que reporta el OLE) DEBE forzar MOVER igualmente. GESTO: drop con ctrl=false, shift=false,
/// move_hint=true. RESULTADO: movido (creado en destino, borrado del origen).
#[test]
fn move_hint_fuerza_mover_con_modificadores_stale() {
    let src = tempfile::tempdir().unwrap();
    let dst = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("reg.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(src.path());
    let (_origin, dest) = split_a(&mut c, dst.path());

    let a = area();
    c.set_area(a);
    let (cx, cy) = pane_center(&c, a, dest);

    // Modificadores de la app stale (false), pero move_hint del OLE = true → MOVER.
    let routed = c.drop_at(cx, cy, false, false, vec![src.path().join("reg.txt")], true);
    assert!(routed, "el drop debe enrutar");
    // CONFIRMAR AL SOLTAR (PUNTO 1b): el drop entre paneles pide confirmación antes de ejecutar.
    // El move_hint queda guardado en el pendiente, así que confirmar MUEVE igual.
    assert!(c.confirm_pending_drop(), "confirmar arranca el movimiento");
    assert!(drain_ops(&mut c), "la operación debe terminar");

    assert!(
        dst.path().join("reg.txt").exists(),
        "el archivo aterrizó en el destino"
    );
    assert!(
        !src.path().join("reg.txt").exists(),
        "el move_hint forzó MOVER pese a ctrl/shift en false (regresión del Shift)"
    );
}

#[test]
fn mark_y_pane_was_ejected() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    let id = c.ws.active_id().expect("hay un panel activo al arrancar");
    assert!(!c.pane_was_ejected(id), "recién creado no está marcado como expulsado");
    c.mark_pane_ejected(id);
    assert!(c.pane_was_ejected(id), "tras marcar, pane_was_ejected debe ser true");
    // navegar a una carpeta válida limpia el flag:
    c.start_listing(id, work.path().to_path_buf());
    assert!(!c.pane_was_ejected(id), "navegar limpia el flag de expulsado");
}
