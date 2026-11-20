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
//! gesto (p. ej. Ctrl+N abre el modal de carpeta nueva) y el "Aceptar" del modal va por
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
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some()) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

#[test]
fn panel_automatico_se_oculta_tras_copia_y_conserva_historial() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let source = work.path().join("origen.txt");
    let dest = work.path().join("destino");
    std::fs::write(&source, b"contenido").unwrap();
    std::fs::create_dir(&dest).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ensure_ops_pane();
    c.ops.start_op(
        naygo_core::ops::transfer(false, vec![source], dest.clone()),
        "copiar".into(),
        true,
    );
    assert!(drain_ops(&mut c));
    assert!(c.maybe_hide_automatic_operations(true));
    assert!(!c.has_purpose(PanePurpose::Operations));
    assert_eq!(
        std::fs::read(dest.join("origen.txt")).unwrap(),
        b"contenido"
    );
    assert!(c.ops.active_ops.iter().any(|op| op.summary.is_some()));
}

#[test]
fn panel_manual_y_opcion_desactivada_permanecen_visibles() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let source = work.path().join("origen.txt");
    let dest = work.path().join("destino");
    std::fs::write(&source, b"contenido").unwrap();
    std::fs::create_dir(&dest).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.add_pane_of(
        PanePurpose::Operations,
        naygo_core::workspace::layout::Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        },
    );
    c.ops.start_op(
        naygo_core::ops::transfer(false, vec![source.clone()], dest.clone()),
        "copiar".into(),
        true,
    );
    assert!(drain_ops(&mut c));
    assert!(!c.maybe_hide_automatic_operations(true));
    assert!(c.has_purpose(PanePurpose::Operations));
    let id =
        c.ws.panes()
            .iter()
            .find(|p| p.purpose == PanePurpose::Operations)
            .unwrap()
            .id;
    c.close_pane(id);
    c.config.settings.auto_hide_operations = false;
    c.ensure_ops_pane();
    std::fs::remove_file(dest.join("origen.txt")).unwrap();
    c.ops.start_op(
        naygo_core::ops::transfer(false, vec![source], dest),
        "copiar".into(),
        true,
    );
    assert!(drain_ops(&mut c));
    assert!(!c.maybe_hide_automatic_operations(true));
    assert!(c.has_purpose(PanePurpose::Operations));
}

#[test]
fn error_de_planificacion_no_oculta_panel_automatico() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let dest = work.path().join("destino");
    std::fs::create_dir(&dest).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ensure_ops_pane();
    c.ops.start_op(
        naygo_core::ops::transfer(false, vec![work.path().join("inexistente")], dest),
        "copiar".into(),
        true,
    );
    assert!(drain_ops(&mut c));
    assert!(!c.maybe_hide_automatic_operations(true));
    assert!(c.has_purpose(PanePurpose::Operations));
    assert!(c.ops.take_plan_error().is_some());
}

#[test]
fn elementos_omitidos_mantienen_visible_el_panel_automatico() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let source = work.path().join("origen.txt");
    let dest = work.path().join("destino");
    std::fs::write(&source, b"nuevo").unwrap();
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("origen.txt"), b"anterior").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ensure_ops_pane();
    let req = naygo_core::ops::transfer(false, vec![source], dest.clone());
    c.ops.start_op(req, "copiar".into(), true);
    let mut skipped = false;
    for _ in 0..4000 {
        c.ops.pump_ops();
        if let Some(crate::ops_ctrl::OpDialog::Conflict { op_id, .. }) =
            c.ops.pending_dialog.clone()
        {
            c.ops.resolve_conflict(op_id, ConflictAction::Skip, false);
            skipped = true;
        }
        if c.ops.active_ops.iter().any(|op| op.summary.is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(skipped, "debe ofrecer omitir el conflicto");
    assert!(drain_ops(&mut c));
    assert!(!c.maybe_hide_automatic_operations(true));
    assert!(c.has_purpose(PanePurpose::Operations));
    assert_eq!(std::fs::read(dest.join("origen.txt")).unwrap(), b"anterior");
}

#[test]
fn cancelar_confirmacion_cierra_panel_automatico_sin_operacion() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ensure_ops_pane();
    c.ops.pending_dialog = Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
        sources: vec![work.path().join("prueba")],
        permanent: false,
    });
    assert!(!c.maybe_hide_automatic_operations(true));
    c.ops.pending_dialog = None;
    assert!(c.maybe_hide_automatic_operations(true));
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
        staging: None,
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

    // Soltar (copy_forced=Ctrl=copia, determinista). Con la confirmación OFF NO debe quedar pendiente.
    let routed = c.drop_at(cx, cy, false, true, vec![a.path().join("doc.txt")]);
    assert!(
        routed,
        "drop_at enruta igual (devuelve true en ambos modos)"
    );
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Started
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

    // Soltar con la confirmación OFF → arranca directo, sin modal kind 3. (copy_forced=Ctrl.)
    assert!(c.drop_at(cx, cy, false, true, vec![a.path().join("doc.txt")]));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Started
    );
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
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some()) {
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
/// Reproduce el flujo completo drop_at → preconsulta worker → motor y su diálogo de conflicto.
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

    // Soltar el archivo sobre el destino (copy_forced=Ctrl=copia, determinista). UN SOLO POPUP:
    // como HAY conflicto (doc.txt ya existe en el destino), `drop_at` NO deja un drop pendiente de
    // confirmación — ejecuta directo y va al diálogo de CONFLICTO (que ya es la confirmación).
    assert!(c.drop_at(cx, cy, false, true, vec![a.path().join("doc.txt")]));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Started
    );
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
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some()) {
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

    // Drop SIN choque → deja pending_drop (sale el modal de confirmación). copy_forced=Ctrl.
    assert!(c.drop_at(cx, cy, false, true, vec![a.path().join("nuevo.txt")]));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
    assert!(
        c.pending_drop.is_some(),
        "sin conflicto + confirmación ON → se pide confirmar el drop"
    );
    c.cancel_pending_drop();

    // Drop CON choque → NO deja pending_drop (va directo al conflicto, un solo popup). copy_forced=Ctrl.
    assert!(c.drop_at(cx, cy, false, true, vec![a.path().join("choca.txt")]));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Started
    );
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

// ================================ 1. Crear carpeta con Ctrl+N ================================

/// GESTO: el usuario pulsa Ctrl+N (atajo de "nueva carpeta"), escribe varias líneas y
/// confirma. RESULTADO: las carpetas existen en disco. Debe abrir el MISMO modal multilínea
/// del botón y menú contextual, no el diálogo histórico de nombre único.
#[test]
fn crear_carpeta_con_ctrl_n() {
    let work = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());

    // Atajo Ctrl+N: abre el editor de nuevas carpetas multilínea.
    c.on_key("n", true, false, false);
    assert!(
        c.new_folder_open(),
        "Ctrl+N debe abrir el modal multilínea de NUEVAS CARPETAS"
    );

    // Dos carpetas simples y una anidada, como se escribe en el editor real.
    c.new_folder_set_text("Documentos\nProyectos\\Naygo");
    c.new_folder_apply();
    assert!(drain_ops(&mut c), "la creación de carpetas debe terminar");
    c.refresh_active();
    assert!(drain(&mut c));

    let creada = work.path().join("Documentos");
    assert!(creada.is_dir(), "la carpeta nueva debe existir en disco");
    assert!(
        work.path().join("Proyectos").join("Naygo").is_dir(),
        "Ctrl+N conserva la creación anidada del editor multilínea"
    );
    assert!(
        active_pos_of(&c, "Documentos").is_some(),
        "la carpeta nueva aparece en la vista tras refrescar"
    );
}

/// GESTO: el usuario selecciona un archivo y pulsa Ctrl+D. RESULTADO: el motor planifica una
/// copia con nombre único en la misma carpeta y la deja disponible para deshacer.
#[test]
fn duplicar_con_ctrl_d_crea_copia_en_la_misma_carpeta() {
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("informe.txt"), b"contenido").unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());
    let pane = c.ws.active_id().expect("hay un panel activo al arrancar");
    let pos = active_pos_of(&c, "informe.txt").expect("el archivo aparece en el panel");
    c.on_row_clicked(pane, pos, false, false, std::time::Instant::now());

    c.on_key("d", true, false, false);
    assert!(drain_ops(&mut c), "la duplicación debe terminar");

    let copy = work.path().join("informe - copia.txt");
    assert_eq!(std::fs::read(&copy).unwrap(), b"contenido");
    let duplicate_label = c.config.t("action.duplicate");
    assert!(
        c.ops
            .undo_history
            .iter()
            .any(|entry| entry.label == duplicate_label),
        "la duplicación conserva una entrada para deshacer"
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

#[test]
fn refresh_one_then_all_preserves_active_panel_and_paths() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(a.path());
    let (origin, dest) = split_a(&mut c, b.path());
    std::fs::write(a.path().join("a.txt"), b"a").unwrap();
    std::fs::write(b.path().join("b.txt"), b"b").unwrap();
    assert!(c.refresh_pane(dest));
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_id(), Some(origin));
    assert!(c
        .ws
        .pane(origin)
        .unwrap()
        .files
        .as_ref()
        .unwrap()
        .entries
        .is_empty());
    assert_eq!(
        c.ws.pane(dest)
            .unwrap()
            .files
            .as_ref()
            .unwrap()
            .entries
            .len(),
        1
    );
    c.refresh_all();
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_id(), Some(origin));
    for (id, path) in [(origin, a.path()), (dest, b.path())] {
        let files = c.ws.pane(id).unwrap().files.as_ref().unwrap();
        assert_eq!(files.current_dir, path);
        assert_eq!(files.entries.len(), 1);
    }
}

#[test]
fn refresh_all_restores_expanded_tree_branches_and_finishes_for_deleted_nodes() {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let gone = a.join("gone");
    std::fs::create_dir_all(&gone).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let tree_id = c.ws.add_pane(PanePurpose::Tree, PathBuf::new());
    let mut tree = DirTree::default();
    tree.roots.push(naygo_core::tree::TreeNode::folder(
        root.path().to_path_buf(),
    ));
    c.trees.insert(tree_id, tree);
    fn drain_tree(c: &mut WorkspaceCtrl) {
        for _ in 0..4000 {
            if c.pump_tree() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("tree refresh did not finish");
    }
    for path in [root.path(), a.as_path(), gone.as_path()] {
        c.tree_expand(tree_id, path.to_path_buf());
        drain_tree(&mut c);
    }
    let added = a.join("new");
    std::fs::create_dir(&added).unwrap();
    std::fs::remove_dir(&gone).unwrap();
    c.refresh_all();
    drain_tree(&mut c);
    assert!(drain(&mut c));
    let tree = c.trees.get(&tree_id).unwrap();
    assert!(tree.node_at(&a).unwrap().expanded);
    assert!(tree.node_at(&added).is_some());
    assert!(tree.node_at(&gone).is_none());
    assert!(c.tree_refresh_pending.is_empty());

    c.refresh_all();
    c.tree_collapse(tree_id, root.path().to_path_buf());
    drain_tree(&mut c);
    assert!(!c.trees[&tree_id].node_at(root.path()).unwrap().expanded);
    assert!(c.tree_refresh_pending.is_empty());
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
    c.destination_radar_resolve(0);
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

// =========================== 2. Crear archivo con Ctrl+Shift+N ===========================

/// GESTO: Ctrl+Shift+N (atajo "nuevo archivo"), nombre, confirmar. RESULTADO: el archivo existe.
#[test]
fn crear_archivo_con_ctrl_shift_n() {
    let work = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(work.path());

    c.on_key("n", true, true, false); // Ctrl+Shift+N → nuevo archivo
    assert!(
        matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::NameInput {
                purpose: crate::ops_ctrl::NamePurpose::NewFile { .. },
                ..
            })
        ),
        "Ctrl+Shift+N debe abrir el modal de NUEVO ARCHIVO"
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

#[test]
fn nav052_create_selects_after_completion_and_refreshes_tree_without_watcher() {
    let root = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let files = c.active_files_id().unwrap();
    let tree_id = c.ws.add_pane(PanePurpose::Tree, PathBuf::new());
    let mut tree = DirTree::default();
    tree.roots.push(naygo_core::tree::TreeNode::folder(
        root.path().to_path_buf(),
    ));
    c.trees.insert(tree_id, tree);
    c.tree_expand(tree_id, root.path().to_path_buf());
    for _ in 0..2000 {
        if c.pump_tree() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    c.set_active(files);
    c.ws.active_files_mut().unwrap().restored_scroll_row = Some(0);
    c.ctrl_down = true;
    c.shift_down = true;
    c.new_folder_open_active();
    assert!(
        !c.ctrl_down && !c.shift_down,
        "abrir el modal limpia modificadores viejos"
    );
    c.new_folder_set_text("new folder");
    c.ctrl_down = true;
    c.shift_down = true;
    c.new_folder_apply();
    assert!(
        !c.ctrl_down && !c.shift_down,
        "crear con el modal limpia modificadores"
    );
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert_eq!(c.ws.active_files().unwrap().restored_scroll_row, None);
    assert!(drain(&mut c));
    for _ in 0..2000 {
        if c.pump_tree() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let f = c.ws.pane(files).unwrap().files.as_ref().unwrap();
    assert_eq!(
        f.focused_view_entry().unwrap().path,
        root.path().join("new folder")
    );
    assert_eq!(f.selected.len(), 1);
    assert_eq!(f.restored_scroll_row, f.focused);
    assert!(c.trees[&tree_id]
        .node_at(&root.path().join("new folder"))
        .is_some());
    assert_eq!(c.trees[&tree_id].active_path.as_deref(), Some(root.path()));

    // El gesto siguiente al modal debe navegar en el mismo panel aunque el key-release de Ctrl
    // no haya llegado mientras se confirmaba la creación (p. ej. Ctrl+Enter).
    let created_pos = active_pos_of(&c, "new folder").expect("la carpeta creada queda en la vista");
    assert!(c.on_row_double_clicked_native(files, created_pos, false, false));
    assert_eq!(c.ws.active_id(), Some(files));
    assert_eq!(
        c.ws.active_files().unwrap().current_dir,
        root.path().join("new folder"),
        "el doble clic normal posterior entra en la carpeta en el panel actual"
    );
    assert!(drain(&mut c));

    c.new_folder_open_active();
    assert!(c.new_folder_text().is_empty());
}

#[test]
fn new_folder_does_not_activate_the_previous_row_before_completion() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("old")).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let pos = active_pos_of(&c, "old").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.run_action(naygo_core::keymap::Action::Activate);
    assert_eq!(c.ws.active_files().unwrap().current_dir, root.path());
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    c.run_action(naygo_core::keymap::Action::Activate);
    assert_eq!(
        c.ws.active_files().unwrap().current_dir,
        root.path().join("new")
    );
}

#[test]
fn refresh_started_before_creation_cannot_restore_an_old_activation_target() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let pos = active_pos_of(&c, "old").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    let panel = c.active_files_id().unwrap();
    // Reproducir que el refresco capture el contexto anterior antes de abrir el modal y
    // termine después de confirmar, sin procesar aún los resultados de la operación.
    c.start_listing(panel, root.path().to_path_buf());
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    assert!(drain(&mut c));
    c.run_action(naygo_core::keymap::Action::Activate);
    assert!(c.take_creation_wait_notice());
    assert!(
        !c.take_creation_wait_notice(),
        "el aviso se consume una sola vez"
    );
    assert_eq!(
        c.ws.active_files().unwrap().current_dir,
        root.path(),
        "Enter no debe abrir la fila anterior mientras la creación no se ha reconciliado"
    );
}

#[test]
fn explicit_click_allows_activation_while_folder_creation_is_pending() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "old").unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.on_row_clicked(panel, pos, false, false, std::time::Instant::now());
    c.run_action(naygo_core::keymap::Action::Activate);
    assert_eq!(c.ws.active_files().unwrap().current_dir, old);
}

#[test]
fn explicit_typeahead_allows_activation_while_folder_creation_is_pending() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.on_key("o", false, false, false);
    c.run_action(naygo_core::keymap::Action::Activate);
    assert_eq!(c.ws.active_files().unwrap().current_dir, old);
}

#[test]
fn explicit_arrow_selection_allows_activation_while_creation_is_pending() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.run_action(naygo_core::keymap::Action::MoveDown);
    c.run_action(naygo_core::keymap::Action::Activate);
    assert_eq!(c.ws.active_files().unwrap().current_dir, old);
}

#[test]
fn arrow_without_rows_does_not_release_folder_creation_guard() {
    let root = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.run_action(naygo_core::keymap::Action::MoveDown);
    assert!(c.pending_created_activation.contains_key(&panel));
}

#[test]
fn folder_creation_guard_is_removed_when_navigating_away() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    assert!(c.pending_created_activation.contains_key(&panel));
    c.open_in_pane(panel, other.path().to_path_buf());
    assert!(!c.pending_created_activation.contains_key(&panel));
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.pane(panel)
            .unwrap()
            .files
            .as_ref()
            .unwrap()
            .current_dir,
        other.path()
    );
}

#[test]
fn folder_creation_guard_releases_after_cancellation_and_refresh() {
    let root = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    assert!(c.pending_created_activation.contains_key(&panel));
    let op_id = c.ops.active_ops.last().unwrap().id;
    c.ops.cancel_op(op_id as i32);
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    assert!(!c.block_pending_created_activation(panel));
}

#[test]
fn folder_creation_guard_waits_for_reconciliation_and_listing() {
    let root = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    c.ops.ops_mode = crate::ops_ctrl::OpsMode::Queue;
    let panel = c.active_files_id().unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("one\ntwo");
    c.new_folder_apply();
    assert!(drain_ops(&mut c));
    assert!(
        c.block_pending_created_activation(panel),
        "terminales aún no reconciliados"
    );
    c.reconcile_completed_operations();
    assert!(
        c.block_pending_created_activation(panel),
        "el listado aún no se aplicó"
    );
    assert!(drain(&mut c));
    assert!(!c.block_pending_created_activation(panel));
    assert!(root.path().join("one").is_dir());
    assert!(root.path().join("two").is_dir());
}

#[test]
fn folder_creation_guard_is_cleared_when_a_layout_replaces_the_workspace() {
    let root = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    c.pending_created_activation
        .insert(panel, root.path().to_path_buf());
    c.creation_selection.insert(
        panel,
        CreationSelection {
            dir: root.path().to_path_buf(),
            targets: Default::default(),
            overflow: false,
            overridden: true,
        },
    );
    assert!(c.apply_template_ephemeral("Dual-pane"));
    assert!(c.pending_created_activation.is_empty());
    assert!(c.creation_selection.is_empty());
}

#[test]
fn late_folder_creation_preserves_explicit_selection() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "old").unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.on_row_clicked(panel, pos, false, false, std::time::Instant::now());
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        old
    );
}

#[test]
fn new_folder_modal_opens_without_synchronous_destination_probe() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let (mut c, _cfg) = ctrl_en(root.path());
    c.ws.active_files_mut().unwrap().current_dir = missing.clone();
    c.new_folder_open_active();
    assert!(c.new_folder_open());
    c.new_folder_set_text("child");
    c.new_folder_apply();
    assert!(drain_ops(&mut c));
    assert!(c
        .ops
        .active_ops
        .iter()
        .any(|op| op.summary.as_ref().is_some_and(|s| s.count_failed() > 0)));
    assert!(!missing.exists());
}

#[test]
fn right_click_selected_preserves_entire_batch_and_unselected_replaces_it() {
    let root = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(root.path().join(name), b"x").unwrap();
    }
    let (mut c, _cfg) = ctrl_en(root.path());
    let pane = c.active_files_id().unwrap();
    let a = active_pos_of(&c, "a.txt").unwrap();
    let b = active_pos_of(&c, "b.txt").unwrap();
    let other = active_pos_of(&c, "c.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(a);
    c.ws.active_files_mut().unwrap().select_toggle(b);
    let selected_before = c.selected_paths();
    c.last_click = Some((pane, a, std::time::Instant::now()));
    c.open_row_context_menu(pane, a, 10.0, 20.0);
    assert_eq!(c.context_targets(), selected_before);
    assert_eq!(c.selected_paths(), selected_before);
    assert_eq!(c.ws.active_files().unwrap().focused, Some(a));
    assert!(
        c.last_click.is_none(),
        "el clic derecho no debe contar como clic izquierdo"
    );
    c.close_context_menu();
    c.open_row_context_menu(pane, other, 10.0, 20.0);
    assert_eq!(c.context_targets(), vec![root.path().join("c.txt")]);
    assert_eq!(c.selected_paths(), vec![root.path().join("c.txt")]);
    c.close_context_menu();
    c.open_row_context_menu(pane, usize::MAX, 10.0, 20.0);
    assert!(
        c.context_menu.is_none(),
        "una fila obsoleta no debe apuntar a otra"
    );
}

#[test]
fn right_click_uses_selection_of_clicked_inactive_panel() {
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    std::fs::write(left.path().join("left.txt"), b"x").unwrap();
    for name in ["uno.txt", "dos.txt"] {
        std::fs::write(right.path().join(name), b"x").unwrap();
    }
    let (mut c, _cfg) = ctrl_en(left.path());
    let left_id = c.active_files_id().unwrap();
    let right_id = c.split_for_target().unwrap();
    c.open_in_pane(right_id, right.path().to_path_buf());
    assert!(drain(&mut c));
    let files = c.ws.pane_mut(right_id).unwrap().files.as_mut().unwrap();
    files.select_single(0);
    files.select_toggle(1);
    let selected = c.selected_paths_of(right_id);
    c.ws.set_active(left_id);
    c.open_row_context_menu(right_id, 0, 0.0, 0.0);
    assert_eq!(c.context_targets(), selected);
    assert_eq!(c.ws.active_id(), Some(right_id));
    assert_eq!(selected.len(), 2);
}

#[test]
fn terminal_context_keeps_captured_missing_directory_and_reports_error() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let (mut c, _cfg) = ctrl_en(root.path());
    let tree = c.ws.add_pane(PanePurpose::Tree, PathBuf::new());
    c.open_path_folder_context_menu(tree, missing.clone(), 0.0, 0.0);
    assert_eq!(c.terminal_dir().as_deref(), Some(missing.as_path()));
    c.ctx_open_terminal(0);
    for _ in 0..200 {
        if let Some(error) = c.take_shell_error() {
            assert!(error.contains("missing"));
            assert!(!missing.exists());
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("el worker no entregó el error");
}

#[test]
fn explorer_missing_target_wakes_ui_and_reports_its_own_path() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing-explorer");
    let (mut c, _cfg) = ctrl_en(root.path());
    let wakes = std::sync::Arc::new(AtomicUsize::new(0));
    let wakes_for_worker = wakes.clone();
    c.set_folder_shell_waker(std::sync::Arc::new(move || {
        wakes_for_worker.fetch_add(1, Ordering::SeqCst);
    }));
    let panel = c.active_files_id().unwrap();
    c.open_path_folder_context_menu(panel, missing, 0.0, 0.0);
    c.ctx_open_explorer();
    for _ in 0..200 {
        if wakes.load(Ordering::SeqCst) > 0 {
            assert!(c.take_shell_error().unwrap().contains("missing-explorer"));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("el worker no despertó la interfaz");
}

#[test]
fn terminal_context_does_not_switch_when_active_panel_path_changes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("file.txt"), b"x").unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "file.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    c.open_context_menu(panel, 0.0, 0.0);
    c.ws.active_files_mut().unwrap().current_dir = root.path().join("other");
    assert_eq!(c.terminal_dir().as_deref(), Some(root.path()));
}

#[test]
fn folder_shell_rejects_extra_request_while_worker_is_blocked() {
    let root = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let (sender, receiver) = mpsc::channel();
    c.folder_shell_rx = Some(receiver);
    c.terminal_active(0);
    assert!(c.folder_shell_rx.is_some());
    assert!(c.take_shell_error().is_some());
    sender.send(Ok(())).unwrap();
    assert!(c.take_shell_error().is_none());
    assert!(c.folder_shell_rx.is_none());
}

#[test]
fn empty_area_context_keeps_missing_panel_directory_for_creation() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    c.ws.active_files_mut().unwrap().current_dir = missing.clone();
    c.open_folder_context_menu(panel, 12.0, 34.0);
    assert_eq!(c.context_targets(), vec![missing.clone()]);
    c.ctx_new_folder();
    assert_eq!(c.new_folder.as_ref().unwrap().dir, missing);
    c.new_folder_set_text("child");
    c.new_folder_apply();
    assert!(drain_ops(&mut c));
    assert!(c.ops.active_ops.iter().any(|op| op
        .summary
        .as_ref()
        .is_some_and(|summary| summary.count_failed() > 0)));
    assert!(!missing.exists());
}

#[test]
fn folder_creation_context_keeps_missing_tree_target_without_fallback() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let (mut c, _cfg) = ctrl_en(root.path());
    let tree = c.ws.add_pane(PanePurpose::Tree, PathBuf::new());
    c.open_path_folder_context_menu(tree, missing.clone(), 0.0, 0.0);
    c.ctx_new_folder();
    assert_eq!(c.new_folder.as_ref().unwrap().dir, missing);
    c.new_folder_set_text("child");
    c.new_folder_apply();
    assert!(drain_ops(&mut c));
    assert!(c
        .ops
        .active_ops
        .iter()
        .any(|op| op.summary.as_ref().is_some_and(|s| s.count_failed() > 0)));
    assert!(
        !root.path().join("child").exists(),
        "no crear en el panel activo como fallback"
    );
    assert!(!missing.exists());
}

#[test]
fn folder_creation_watcher_preserves_explicit_selection() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    c.config.settings.new_items_at_end = true;
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "old").unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.on_row_clicked(panel, pos, false, false, std::time::Instant::now());
    assert!(drain_ops(&mut c));
    let created = root.path().join("new");
    c.apply_watch_events(
        panel,
        &[naygo_core::listing::DirEvent::Created(created.clone())],
    );
    assert!(c.ws.active_files().unwrap().highlighted.contains(&created));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        old
    );
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        old
    );
}

#[test]
fn folder_creation_watcher_still_selects_unrelated_arrivals() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "old").unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.on_row_clicked(panel, pos, false, false, std::time::Instant::now());
    assert!(drain_ops(&mut c));
    let created = root.path().join("new");
    let other = root.path().join("other");
    std::fs::create_dir(&other).unwrap();
    c.apply_watch_events(
        panel,
        &[
            naygo_core::listing::DirEvent::Created(created),
            naygo_core::listing::DirEvent::Created(other.clone()),
        ],
    );
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        other
    );
}

#[test]
fn folder_creation_overflow_preserves_selection_conservatively() {
    let state = CreationSelection {
        dir: PathBuf::from("root"),
        targets: Default::default(),
        overflow: true,
        overridden: true,
    };
    assert!(state.protects(std::path::Path::new("root/any")));
}

#[test]
fn folder_creation_selection_survives_an_inflight_refresh() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "old").unwrap();
    c.start_listing(panel, root.path().to_path_buf());
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    c.on_row_clicked(panel, pos, false, false, std::time::Instant::now());
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        old
    );
}

#[test]
fn folder_creation_selection_survives_reconciliation_listing() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "old").unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("new");
    c.new_folder_apply();
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    // El primer lote todavía no se recogió: el usuario elige la fila que sigue visible.
    c.on_row_clicked(panel, pos, false, false, std::time::Instant::now());
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        old
    );
}

#[test]
fn multiple_folder_creations_preserve_an_explicit_selection() {
    let root = tempfile::tempdir().unwrap();
    let old = root.path().join("old");
    std::fs::create_dir(&old).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    c.ops.ops_mode = crate::ops_ctrl::OpsMode::Queue;
    let panel = c.active_files_id().unwrap();
    let pos = active_pos_of(&c, "old").unwrap();
    c.new_folder_open_active();
    c.new_folder_set_text("one\ntwo");
    c.new_folder_apply();
    c.on_row_clicked(panel, pos, false, false, std::time::Instant::now());
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        old
    );
    assert!(root.path().join("one").is_dir());
    assert!(root.path().join("two").is_dir());
}

#[test]
fn late_folder_creation_preserves_explicit_keyboard_selection() {
    for by_typing in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("old");
        std::fs::create_dir(&old).unwrap();
        let (mut c, _cfg) = ctrl_en(root.path());
        c.new_folder_open_active();
        c.new_folder_set_text("new");
        c.new_folder_apply();
        if by_typing {
            c.on_key("o", false, false, false);
        } else {
            c.run_action(naygo_core::keymap::Action::MoveDown);
        }
        assert!(drain_ops(&mut c));
        c.reconcile_completed_operations();
        assert!(drain(&mut c));
        assert_eq!(
            c.ws.active_files()
                .unwrap()
                .focused_view_entry()
                .unwrap()
                .path,
            old
        );
    }
}

#[test]
fn new_folder_shortcut_uses_tree_cursor_as_destination() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let tree_id = c.ws.add_pane(PanePurpose::Tree, PathBuf::new());
    let mut tree = DirTree::default();
    tree.roots
        .push(naygo_core::tree::TreeNode::folder(child.clone()));
    c.trees.insert(tree_id, tree);
    c.set_active(tree_id);
    c.on_key("n", true, false, false);
    assert_eq!(c.new_folder.as_ref().unwrap().dir, child);
    c.new_folder_set_text("created");
    c.new_folder_apply();
    assert!(drain_ops(&mut c));
    assert!(child.join("created").is_dir());
    assert!(!root.path().join("created").exists());
}

#[test]
fn nav052_up_and_refresh_keep_path_identity_after_sort() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("z child");
    std::fs::create_dir(&child).unwrap();
    std::fs::create_dir(root.path().join("a sibling")).unwrap();
    let (mut c, _cfg) = ctrl_en(&child);
    assert!(c.on_go_up());
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        child
    );
    assert_eq!(c.ws.active_files().unwrap().selected.len(), 1);
    std::fs::create_dir(root.path().join("b inserted")).unwrap();
    c.refresh_active();
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files()
            .unwrap()
            .focused_view_entry()
            .unwrap()
            .path,
        child
    );
    assert_eq!(c.ws.active_files().unwrap().selected.len(), 1);
}

#[test]
fn nav052_watch_during_listing_reconciles_and_rescan_reloads() {
    let root = tempfile::tempdir().unwrap();
    let gone = root.path().join("gone.txt");
    std::fs::write(&gone, "old").unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    let id = c.active_files_id().unwrap();
    c.start_listing(id, root.path().to_path_buf());
    std::fs::remove_file(&gone).unwrap();
    c.apply_watch_events_resolved(
        id,
        &[naygo_core::listing::DirEvent::Removed(gone.clone())],
        &Default::default(),
    );
    assert!(c.dirty_listings.contains(&id));
    assert!(drain(&mut c));
    assert!(c
        .ws
        .active_files()
        .unwrap()
        .entries
        .iter()
        .all(|e| e.path != gone));
    let arrived = root.path().join("arrived.txt");
    std::fs::write(&arrived, "new").unwrap();
    c.apply_watch_events_resolved(id, &[], &Default::default());
    assert!(drain(&mut c));
    assert!(c
        .ws
        .active_files()
        .unwrap()
        .entries
        .iter()
        .any(|e| e.path == arrived));
}

#[test]
fn batch_rename_keeps_rows_keyboard_selection_and_scroll_until_refresh() {
    let root = tempfile::tempdir().unwrap();
    let file = |name: &str| root.path().join(name);
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        std::fs::write(file(name), name).unwrap();
    }
    let (mut c, _cfg) = ctrl_en(root.path());
    let pane = c.active_files_id().unwrap();
    {
        let files = c.ws.active_files_mut().unwrap();
        files.select_single(1);
        files.select_range_to(2);
        files.set_scroll_top_position(1);
    }
    // b → z must happen before c → b. Resetting the watcher generation prevents a delayed
    // native event for b → z from treating the new b (formerly c) as the previous b.
    c.reconcile_watchers(std::sync::Arc::new(|| {}));
    assert_eq!(c.watchers.current_dir(pane.0), Some(root.path()));
    c.ops.start_op(
        naygo_core::ops::batch_rename(
            vec![file("b.txt"), file("c.txt")],
            vec!["z.txt".into(), "b.txt".into()],
        ),
        "rename".into(),
        true,
    );
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(c.watchers.current_dir(pane.0).is_none());
    assert!(c.watchers.drain().is_empty());
    assert!(drain(&mut c));
    let files = c.ws.active_files().unwrap();
    assert_eq!(files.view_entry_at(1).unwrap().path, file("z.txt"));
    assert_eq!(files.focused, Some(2));
    assert_eq!(files.focused_view_entry().unwrap().path, file("b.txt"));
    assert_eq!(files.selected, vec![1, 2]);
    assert_eq!(files.anchor, Some(1));
    assert_eq!(files.scroll_anchor_path.as_ref(), Some(&file("z.txt")));
    assert_eq!(files.restored_scroll_row, Some(1));

    assert!(c.refresh_pane(pane));
    assert!(drain(&mut c));
    let files = c.ws.active_files().unwrap();
    assert_eq!(files.view_entry_at(3).unwrap().path, file("z.txt"));
    assert_eq!(files.focused_view_entry().unwrap().path, file("b.txt"));
    assert_eq!(files.anchor, Some(3));
    assert!(files.is_selected(1));
    assert!(files.is_selected(3));
}

#[test]
fn files_discovered_by_watcher_and_rescan_stay_at_end_until_sort() {
    let root = tempfile::tempdir().unwrap();
    let file = |name: &str| root.path().join(name);
    for name in ["m.txt", "z.txt"] {
        std::fs::write(file(name), name).unwrap();
    }
    let (mut c, _cfg) = ctrl_en(root.path());
    let pane = c.active_files_id().unwrap();
    for name in ["b.txt", "a.txt"] {
        std::fs::write(file(name), name).unwrap();
        c.apply_watch_events(pane, &[naygo_core::listing::DirEvent::Created(file(name))]);
    }
    std::fs::write(file("0.txt"), "0").unwrap();
    c.apply_watch_events_resolved(pane, &[], &Default::default());
    assert!(drain(&mut c));
    c.ws.active_files_mut().unwrap().clear_highlight();
    assert_eq!(active_pos_of(&c, "m.txt"), Some(0));
    assert_eq!(active_pos_of(&c, "b.txt"), Some(2));
    assert_eq!(active_pos_of(&c, "a.txt"), Some(3));
    assert_eq!(active_pos_of(&c, "0.txt"), Some(4));
    c.on_sort_by(pane, "name");
    assert_eq!(active_pos_of(&c, "z.txt"), Some(0));
    assert_eq!(active_pos_of(&c, "m.txt"), Some(1));
    assert_eq!(active_pos_of(&c, "0.txt"), Some(4));
}

#[test]
fn nav052_create_finishing_after_navigation_does_not_steal_focus() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let (mut c, _cfg) = ctrl_en(root.path());
    c.new_folder_open_active();
    c.new_folder_set_text("nested\\child");
    c.new_folder_apply();
    let id = c.active_files_id().unwrap();
    c.open_in_pane(id, other.path().to_path_buf());
    assert!(drain_ops(&mut c));
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().current_dir, other.path());
    assert!(c.ws.active_files().unwrap().selected.is_empty());
}

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
    c.on_row_clicked(origin, pos, false, false, std::time::Instant::now());

    // F6 = MoveToOther. Con dos paneles, el destino se resuelve directo y la op arranca.
    c.on_key(&key_char(slint::platform::Key::F6), false, false, false);
    c.destination_radar_resolve(0);
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
    c.on_row_clicked(origin, pos, false, false, std::time::Instant::now());

    // Copiar al otro panel (CopyToOther no trae atajo por defecto; se dispara por el método,
    // igual que el botón/menú lo cablea en main.rs). `op_to_other` devuelve false para Transfer
    // por diseño (no arranca un LISTADO), así que la op se verifica por el resultado en disco.
    c.op_to_other(false);
    c.destination_radar_resolve(0);
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
    let routed = c.drop_at(cx, cy, true, false, vec![src.path().join("clip.txt")]);
    assert!(routed, "el drop debe enrutar al panel destino");
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
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

    // move_hint=false, copy_forced=false → mismo disco → Mover.
    let routed = c.drop_at(cx, cy, false, false, vec![src.path().join("nota.txt")]);
    assert!(routed, "el drop debe enrutar");
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
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
    c.on_row_clicked(id, pos, false, false, std::time::Instant::now());

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
    c.on_row_clicked(origin, pos, false, false, std::time::Instant::now());

    // Mover al otro panel (F6 / op_to_other(true)): arranca la op, que chocará en el destino.
    // (op_to_other devuelve false para Transfer por diseño; el efecto se ve en el conflicto/disco.)
    c.op_to_other(true);
    c.destination_radar_resolve(0);

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
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some()) {
            termino = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(resuelto, "debe aparecer y resolverse el conflicto");
    assert!(termino, "la operación debe terminar tras resolver");
    assert_eq!(
        c.ops.active_ops[0].summary.as_ref().unwrap().count_failed(),
        0,
        "Move con RenameTo debe terminar sin errores: {:?}",
        c.ops.active_ops[0].summary
    );

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
    c.on_row_clicked(origin, pos, false, false, std::time::Instant::now());
    // op_to_other devuelve false para Transfer por diseño; el efecto se ve en el conflicto/disco.
    c.op_to_other(true);
    c.destination_radar_resolve(0);

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
        if !c.ops.active_ops.is_empty() && c.ops.active_ops.iter().all(|o| o.summary.is_some()) {
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

    // copy_forced=false, pero move_hint del OLE = true (Shift real al soltar) → MOVER.
    let routed = c.drop_at(cx, cy, true, false, vec![src.path().join("reg.txt")]);
    assert!(routed, "el drop debe enrutar");
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
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
    assert!(
        !c.pane_was_ejected(id),
        "recién creado no está marcado como expulsado"
    );
    c.mark_pane_ejected(id);
    assert!(
        c.pane_was_ejected(id),
        "tras marcar, pane_was_ejected debe ser true"
    );
    // navegar a una carpeta válida limpia el flag:
    c.start_listing(id, work.path().to_path_buf());
    assert!(
        !c.pane_was_ejected(id),
        "navegar limpia el flag de expulsado"
    );
}
