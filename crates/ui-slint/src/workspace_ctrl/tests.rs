// Naygo — WorkspaceCtrl: pruebas unitarias del controlador.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;
use slint::Model;
use std::time::Duration;

#[test]
fn drop_sin_destino_exige_seleccion_y_confirmacion() {
    let work = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let file = source.path().join("archivo.txt");
    std::fs::write(&file, b"dato").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    c.config.settings.confirm_drop_between_panes = false;
    assert!(c.open_external_drop_selector(vec![file.clone()], false, true, false, None));
    assert!(c.pending_drop.is_none());
    c.destination_radar_resolve(usize::MAX);
    assert!(c.pending_drop.is_none());
    assert!(c.destination_radar.is_some());
    assert!(!work.path().join("archivo.txt").exists());
    c.destination_radar_resolve(0);
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        DropProbeEvent::Confirmation
    );
    assert_eq!(c.pending_drop.as_ref().unwrap().dest_dir, work.path());
    assert!(!work.path().join("archivo.txt").exists());
    c.cancel_pending_drop();
    assert!(file.exists());
    assert!(!work.path().join("archivo.txt").exists());
}

#[test]
fn drop_externo_exige_confirmacion_aun_con_conflicto_y_preferencia_desactivada() {
    let work = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let file = source.path().join("archivo.txt");
    std::fs::write(&file, b"nuevo").unwrap();
    std::fs::write(work.path().join("archivo.txt"), b"anterior").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    c.config.settings.confirm_drop_between_panes = false;
    assert!(c.open_external_drop_selector(vec![file.clone()], false, true, false, None));
    c.destination_radar_resolve(0);
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        DropProbeEvent::Confirmation
    );
    assert_eq!(c.pending_drop.as_ref().unwrap().dest_dir, work.path());
    c.cancel_pending_drop();
    assert_eq!(
        std::fs::read(work.path().join("archivo.txt")).unwrap(),
        b"anterior"
    );
    assert_eq!(std::fs::read(file).unwrap(), b"nuevo");
}

#[test]
fn drop_externo_omite_confirmacion_si_se_desactiva_durante_la_sesion() {
    let work = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let file = source.path().join("archivo.txt");
    std::fs::write(&file, b"dato").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    c.config.skip_external_drop_confirmation_session = true;

    assert!(c.open_external_drop_selector(vec![file], false, true, false, None));
    c.destination_radar_resolve(0);
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        DropProbeEvent::Started,
        "el drop externo libre se ejecuta sin el modal genérico durante la sesión"
    );
    assert!(c.pending_drop.is_none());
    assert!(c.config.settings.confirm_external_drops);
    assert!(!work.path().join("archivo.txt").exists());

    for _ in 0..4_000 {
        c.ops.pump_ops();
        if work.path().join("archivo.txt").exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(work.path().join("archivo.txt").exists());
}

#[test]
fn drop_externo_sin_confirmacion_sigue_pidiendo_decision_ante_conflicto() {
    let work = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let file = source.path().join("archivo.txt");
    std::fs::write(&file, b"nuevo").unwrap();
    std::fs::write(work.path().join("archivo.txt"), b"anterior").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    c.config.skip_external_drop_confirmation_session = true;

    assert!(c.open_external_drop_selector(vec![file], false, true, false, None));
    c.destination_radar_resolve(0);
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        DropProbeEvent::Started,
        "el drop omite la confirmación genérica"
    );

    let mut conflict_prompted = false;
    for _ in 0..4_000 {
        c.ops.pump_ops();
        if matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::Conflict { .. })
        ) {
            conflict_prompted = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(conflict_prompted, "el conflicto conserva su diálogo propio");
    assert_eq!(
        std::fs::read(work.path().join("archivo.txt")).unwrap(),
        b"anterior"
    );
}

#[test]
fn drop_ole_externo_rapido_ignora_fila_sin_acreditar_y_confirma_ruta() {
    let work = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let file = source.path().join("archivo.txt");
    std::fs::write(&file, b"nuevo").unwrap();
    std::fs::create_dir(work.path().join("subcarpeta")).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    c.config.settings.confirm_drop_between_panes = false;
    let pane = c.ws.active_id().unwrap();
    let row = active_pos_of(&c, "subcarpeta").unwrap();
    c.set_drag_over_at(Some(pane), Some(140.0));
    c.set_drag_over_row(pane, row as i32);
    let (_, rect) = c
        .pane_rects(area())
        .into_iter()
        .find(|(id, _)| *id == pane)
        .unwrap();
    assert!(c.drop_at_from_ole(
        rect.x + rect.w / 2.0,
        rect.y + rect.h / 2.0,
        0.0,
        None,
        false,
        true,
        false,
        vec![file.clone()],
        None
    ));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        DropProbeEvent::Confirmation
    );
    assert_eq!(c.pending_drop.as_ref().unwrap().dest_dir, work.path());
    c.cancel_pending_drop();
    assert!(!work.path().join("archivo.txt").exists());
    assert_eq!(std::fs::read(file).unwrap(), b"nuevo");
}

#[test]
fn selector_de_drop_rechaza_panel_cuya_ruta_cambio() {
    let work = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let file = source.path().join("archivo.txt");
    std::fs::write(&file, b"dato").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    assert!(c.open_external_drop_selector(vec![file.clone()], false, true, false, None));
    let pane = c.destination_radar.as_ref().unwrap().drop_panes[0];
    c.ws.pane_mut(pane)
        .unwrap()
        .files
        .as_mut()
        .unwrap()
        .current_dir = other.path().to_path_buf();
    c.destination_radar_resolve(0);
    assert!(c.pending_drop.is_none());
    assert!(c.destination_radar.is_none());
    assert!(file.exists());
    assert!(!other.path().join("archivo.txt").exists());
}

#[test]
fn preview_respects_tabs_maximized_panel_window_visibility_and_close() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("one.txt"), "one").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let files = c.active_id().unwrap();
    c.set_area(area());
    c.add_pane_of(PanePurpose::Preview, area());
    let preview = c.active_id().unwrap();
    let now = std::time::Instant::now();
    assert!(c.drive_preview_visible(now, true));
    assert!(c.preview.can_cancel(now));
    assert!(!c.drive_preview_visible(now, false));
    assert!(!c.preview.busy());
    assert!(c.drive_preview_visible(now, true));
    c.toggle_maximize(files);
    assert!(!c.drive_preview_visible(now, true));
    c.toggle_maximize(files);
    assert!(c.drive_preview_visible(now, true));
    c.stack_into(preview, files);
    c.set_active_tab(files);
    assert!(!c.drive_preview_visible(now, true));
    c.set_active_tab(preview);
    assert!(c.drive_preview_visible(now, true));
    assert!(c.cancel_preview());
    assert!(!c.drive_preview_visible(now, true));
    c.close_pane(preview);
    assert!(!c.drive_preview_visible(now, true));
    assert!(c.preview.wanted.is_none());
}

/// Drena los listados hasta que todos terminan (con timeout), simulando los ticks del
/// Timer. Devuelve true si terminaron.
fn drain(c: &mut WorkspaceCtrl) -> bool {
    for _ in 0..2000 {
        if c.pump_listings() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

/// Deja ambos cupos de listado (local y remoto) con una plaza y la ocupa: simula workers que no
/// terminan sin depender de en qué cupo cae la ruta de la prueba.
fn saturate_listing_admissions(
    c: &mut WorkspaceCtrl,
) -> (
    naygo_core::listing::ListingPermit,
    naygo_core::listing::ListingPermit,
) {
    c.listing_admissions = naygo_core::listing::ListingAdmissions::new(1, 1);
    (
        c.listing_admissions.local().try_acquire().unwrap(),
        c.listing_admissions.remote().try_acquire().unwrap(),
    )
}

#[test]
fn tree_reveal_termina_si_falta_un_ancestro_tras_listar() {
    let root = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), config.path().to_path_buf());
    assert!(drain(&mut c));
    let tree = c.ws.add_pane(PanePurpose::Tree, PathBuf::new());
    let mut roots = DirTree::default();
    roots.roots.push(naygo_core::tree::TreeNode::folder(
        root.path().to_path_buf(),
    ));
    c.trees.insert(tree, roots);
    let slot = saturate_listing_admissions(&mut c);
    let missing = root.path().join("off-line").join("child");
    assert!(c.navigate_tree_to(tree, missing.clone()));
    assert_eq!(c.reveal_targets.get(&tree), Some(&missing));
    assert!(
        !c.pump_tree(),
        "la rama en cola todavía puede entregar el ancestro"
    );
    drop(slot);
    assert!(drain(&mut c));
    for _ in 0..2000 {
        if c.pump_tree() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!c.reveal_targets.contains_key(&tree));
    assert!(c.pump_tree());
    let files = c.ws.pane(c.active_files_id().unwrap()).unwrap();
    assert_eq!(files.files.as_ref().unwrap().current_dir, missing);
    assert!(c.pending_listing_error.is_some());
}

#[test]
fn reintentar_carpeta_recuperada_revela_sus_ancestros_en_el_arbol() {
    let root = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let destination = root.path().join("proveedor").join("carpeta");
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), config.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_files_id().unwrap();
    let tree = c.ws.add_pane(PanePurpose::Tree, PathBuf::new());
    let mut roots = DirTree::default();
    roots.roots.push(naygo_core::tree::TreeNode::folder(
        root.path().to_path_buf(),
    ));
    c.trees.insert(tree, roots);
    let drain_tree = |c: &mut WorkspaceCtrl| {
        for _ in 0..2000 {
            if c.pump_tree() {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("el árbol debía terminar de listar");
    };

    assert!(c.navigate_tree_to(tree, destination.clone()));
    assert!(drain(&mut c));
    assert!(drain_missing(&mut c));
    drain_tree(&mut c);
    assert!(c.pane_dir_missing(pane));
    assert_eq!(c.tree_reveal_row(tree), -1);
    let visit =
        c.ws.pane(pane)
            .unwrap()
            .files
            .as_ref()
            .unwrap()
            .navigation_revision;

    std::fs::create_dir_all(&destination).unwrap();
    let file = destination.join("recuperado.txt");
    std::fs::write(&file, b"fixture").unwrap();
    c.missing_folder_retry(pane);
    assert!(drain(&mut c));
    assert!(drain_missing(&mut c));
    drain_tree(&mut c);
    let files = c.ws.pane(pane).unwrap().files.as_ref().unwrap();
    assert_eq!(files.navigation_revision, visit);
    assert!(files.entries.iter().any(|entry| entry.path == file));
    assert!(!c.pane_dir_missing(pane));
    assert_eq!(
        c.tree_cursor_of(tree).as_deref(),
        Some(destination.as_path())
    );
    assert!(
        c.tree_reveal_row(tree) >= 0,
        "el árbol debe revelar la carpeta recuperada"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "validación de lectura con una carpeta OneDrive real y perfil temporal"]
fn onedrive_tree_navega_refresca_y_regresa_en_windows() {
    let root = PathBuf::from(std::env::var_os("NAYGO_TEST_ONEDRIVE").unwrap());
    let start = PathBuf::from(std::env::var_os("NAYGO_TEST_ONEDRIVE_START").unwrap());
    assert!(root.is_dir() && start.is_dir() && start != root && start.starts_with(&root));
    assert!(
        naygo_platform::known_folders::known_folders()
            .iter()
            .any(|folder| {
                naygo_core::path_identity::key(&folder.path)
                    == naygo_core::path_identity::key(&root)
            }),
        "Windows debe registrar la raíz como acceso conocido"
    );
    let config = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(start.clone(), config.path().to_path_buf());
    c.drive_snapshot = naygo_platform::drives::drives();
    let files = c.active_files_id().unwrap();
    c.add_pane_of(PanePurpose::Tree, area());
    let tree =
        c.ws.panes()
            .iter()
            .find(|pane| pane.purpose == PanePurpose::Tree)
            .unwrap()
            .id;
    let drain_view = |c: &mut WorkspaceCtrl, target: &Path| {
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            let files_ready = c.pump_listings();
            let tree_ready = c.pump_tree();
            if files_ready && tree_ready {
                let panel = c.ws.pane(files).unwrap().files.as_ref().unwrap();
                assert!(panel.current_dir == target);
                assert!(
                    c.pending_listing_error.is_none(),
                    "el listado debe terminar completo"
                );
                assert!(c.tree_cursor_of(tree).as_deref() == Some(target));
                assert!(
                    c.tree_reveal_row(tree) >= 0,
                    "el destino debe aparecer en el árbol"
                );
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("el listado o el árbol siguieron ocupados");
    };
    c.sync_trees_for_files(files, start.clone());
    drain_view(&mut c, &start);
    for linked in [false, true] {
        if linked {
            assert!(c.ws.link_tree(tree, files));
        }
        assert!(c.navigate_tree_to(tree, root.clone()));
        drain_view(&mut c, &root);
        c.refresh_all();
        drain_view(&mut c, &root);
        assert!(c.on_go_back());
        drain_view(&mut c, &start);
    }
}

#[test]
fn navegaciones_sin_plaza_coalescen_a_la_ultima_ruta_del_panel() {
    let work = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let latest = tempfile::tempdir().unwrap();
    let mut ctrl = WorkspaceCtrl::new_in(work.path().to_path_buf(), config.path().to_path_buf());
    assert!(drain(&mut ctrl));

    let held = saturate_listing_admissions(&mut ctrl);
    let pane = ctrl.active_id().unwrap();
    for index in 0..100 {
        let destination = if index == 99 {
            latest.path().to_path_buf()
        } else {
            first.path().join(index.to_string())
        };
        ctrl.start_listing(pane, destination);
    }

    assert!(!ctrl.listings.contains_key(&pane));
    assert_eq!(
        ctrl.pending_file_listings.get(&pane),
        Some(&latest.path().to_path_buf())
    );
    drop(held);
    assert!(!ctrl.pump_listings());
    assert!(ctrl.listings.contains_key(&pane));
    assert!(drain(&mut ctrl));
}

/// Cuatro shares colgados ocupan el cupo remoto completo: un quinto listado local entra y termina
/// igual, y un quinto remoto queda en la cola del panel hasta que se libera una plaza remota.
#[cfg(windows)]
#[test]
fn shares_colgados_no_bloquean_lo_local_y_el_remoto_pendiente_no_se_pierde() {
    use naygo_core::icon_kind::DriveKind;
    use naygo_platform::drives::DriveInfo;

    let work = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let local_dir = tempfile::tempdir().unwrap();
    std::fs::write(local_dir.path().join("local.txt"), b"ok").unwrap();
    let mut ctrl = WorkspaceCtrl::new_in(work.path().to_path_buf(), config.path().to_path_buf());
    assert!(drain(&mut ctrl));

    let local_letter = naygo_core::path_locality::drive_letter(local_dir.path()).unwrap();
    // Letra de red ficticia: no existe en el equipo, así que el listado remoto falla rápido en
    // vez de ir a la red cuando por fin obtiene plaza.
    let remote_letter = ('M'..='Z')
        .rev()
        .find(|letter| *letter != local_letter && !Path::new(&format!("{letter}:\\")).exists())
        .expect("hay una letra libre para la unidad de red ficticia");
    let drive = |letter: char, kind| DriveInfo {
        path: PathBuf::from(format!("{letter}:\\")),
        label: format!("{letter}:\\"),
        kind,
    };
    ctrl.drive_snapshot = vec![
        drive(local_letter, DriveKind::Fixed),
        drive(remote_letter, DriveKind::Network),
    ];
    let hung: Vec<_> = (0..naygo_core::listing::REMOTE_LISTING_SLOTS)
        .map(|_| ctrl.listing_admissions.remote().try_acquire().unwrap())
        .collect();

    let local_pane = ctrl.active_id().unwrap();
    ctrl.start_listing(local_pane, local_dir.path().to_path_buf());
    assert!(
        ctrl.listings.contains_key(&local_pane),
        "el listado local no espera al cupo remoto"
    );

    let remote_pane = ctrl
        .ws
        .add_pane(PanePurpose::Files, work.path().to_path_buf());
    let remote_dir = PathBuf::from(format!("{remote_letter}:\\equipo"));
    ctrl.start_listing(remote_pane, remote_dir.clone());
    assert!(!ctrl.listings.contains_key(&remote_pane));
    assert_eq!(
        ctrl.pending_file_listings.get(&remote_pane),
        Some(&remote_dir)
    );

    for _ in 0..2000 {
        ctrl.pump_listings();
        if !ctrl.listings.contains_key(&local_pane) {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!ctrl.listings.contains_key(&local_pane), "el local terminó");
    let names: Vec<_> = ctrl
        .ws
        .pane(local_pane)
        .and_then(|pane| pane.files.as_ref())
        .unwrap()
        .entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    assert_eq!(names, ["local.txt"]);
    assert_eq!(
        ctrl.pending_file_listings.get(&remote_pane),
        Some(&remote_dir),
        "el remoto sigue en cola mientras el cupo remoto está lleno"
    );

    let mut hung = hung;
    drop(hung.pop());
    assert!(!ctrl.pump_listings());
    assert!(!ctrl.pending_file_listings.contains_key(&remote_pane));
    assert!(
        ctrl.listings.contains_key(&remote_pane),
        "el remoto pendiente toma la plaza liberada"
    );
    assert!(drain(&mut ctrl));
    drop(hung);
}

fn wait_for_workspace_persistence(c: &WorkspaceCtrl) {
    for ticket in c.additional_persistence_tickets() {
        naygo_core::config::wait_settings_saved(&ticket, Duration::from_secs(5))
            .expect("la persistencia asíncrona termina en la prueba");
    }
}

#[test]
fn navegar_invalida_resaltados_de_otra_carpeta_con_el_mismo_nombre() {
    let work = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let target = work.path().join("destino");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("informe.txt"), b"actual").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), config.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    c.watchers.unwatch(id.0);
    let stale_path = work.path().join("informe.txt");
    c.visit_changes.insert(
        id,
        vec![naygo_core::listing_changes::ListingChange {
            path: stale_path.clone(),
            kind: naygo_core::listing_changes::ChangeKind::New,
        }],
    );
    c.visit_highlights
        .insert(id, std::collections::HashSet::from([stale_path.clone()]));

    c.start_listing(id, target);
    assert!(!c.visit_changes.contains_key(&id));
    assert!(!c.visit_highlights.contains_key(&id));
    assert!(drain(&mut c));
    assert!(!c
        .visit_highlights
        .get(&id)
        .is_some_and(|paths| paths.contains(&stale_path)));
}

#[test]
fn maximizar_es_presentacion_y_restaurar_conserva_layout_y_modelos() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.active_id().unwrap();
    c.add_pane_of(PanePurpose::Preview, area());
    let preview = c.active_id().unwrap();
    let original = c.pane_rects(area());
    let layout = serde_json::to_string(&c.ws.layout).unwrap();
    let count = c.ws.panes().len();
    assert!(c.toggle_maximize(preview));
    assert_eq!(c.pane_rects(area()), vec![(preview, area())]);
    assert!(c.split_handles(area()).is_empty());
    assert_eq!(c.ws.panes().len(), count);
    assert_eq!(serde_json::to_string(&c.ws.layout).unwrap(), layout);
    assert!(c.toggle_maximize(preview));
    assert_eq!(c.pane_rects(area()), original);
    assert!(c.toggle_maximize(preview));
    c.set_active(first);
    assert_eq!(c.maximized_pane, None);
    assert_eq!(c.pane_rects(area()), original);
}

#[test]
fn maximizar_invalido_no_muta_y_cerrar_restaura() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(!c.toggle_maximize(PaneId(u64::MAX)));
    c.add_pane_of(PanePurpose::Preview, area());
    let preview = c.active_id().unwrap();
    assert!(c.run_action(Action::ToggleMaximizePane));
    assert_eq!(c.maximized_pane, Some(preview));
    c.close_pane(preview);
    assert_eq!(c.maximized_pane, None);
    assert!(!c.pane_rects(area()).is_empty());
}

#[test]
fn bandeja_flechas_sin_foco_empiezan_en_extremos() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    let a = tmp.path().join("a.txt");
    let b = tmp.path().join("b.txt");
    c.basket.add([a.clone(), b.clone()]);
    assert!(c.basket_move_selection(1));
    assert_eq!(c.basket_selected_path(), Some(a.clone()));
    c.basket_selection.focused = None;
    assert!(c.basket_move_selection(-1));
    assert_eq!(c.basket_selected_path(), Some(b.clone()));
    assert!(c.basket_move_selection(1));
    assert_eq!(c.basket_selected_path(), Some(b));
    c.basket_clear();
    assert!(!c.basket_move_selection(1));
}

#[test]
fn bandeja_multiseleccion_conserva_marcas_sin_robar_contexto_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let files = c.active_id().unwrap();
    c.add_pane_of(PanePurpose::Basket, area());
    let basket = c.active_id().unwrap();
    let paths: Vec<_> = ["a", "b", "c"]
        .into_iter()
        .map(|p| tmp.path().join(p))
        .collect();
    c.basket.add(paths.clone());
    assert!(!c.basket_request_transfer(false));
    c.basket_select(0);
    c.run_action(Action::ExtendDown);
    assert_eq!(c.selected_paths(), paths[..2]);
    c.run_action(Action::FocusDownKeep);
    assert_eq!(c.basket_selected_path(), Some(paths[2].clone()));
    assert_eq!(c.basket_action_paths(), paths[..2]);
    c.set_active(files);
    assert!(c.basket_selected_path().is_none());
    assert_eq!(c.basket_action_paths(), paths[..2]);
    c.set_active(basket);
    assert_eq!(c.metadata_target(), Some(paths[2].clone()));
    c.basket_delete();
    assert!(
        matches!(&c.ops.pending_dialog, Some(crate::ops_ctrl::OpDialog::ConfirmDelete { sources, permanent: false }) if sources == &paths[..2])
    );
    assert_eq!(c.basket.len(), 3);
    assert!(c.basket_remove_selected());
    assert_eq!(c.basket.items(), &paths[2..]);
}

#[test]
fn entrega_revisada_se_invalida_y_publica_con_deshacer_solo_del_destino() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let input = work.path().join("input.txt");
    std::fs::write(&input, b"delivery content").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), config.path().to_path_buf());
    c.basket.add([input.clone()]);
    c.basket_select(0);
    assert!(c.delivery_open());
    c.basket_clear();
    let destination = work.path().join("delivery");
    c.delivery_prepare(
        destination.clone(),
        1,
        PathBuf::new(),
        false,
        true,
        Default::default(),
    );
    for _ in 0..3000 {
        if c.pump_delivery() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(c.delivery.plan.is_some(), "{}", c.delivery.error);
    assert!(!destination.exists());
    c.delivery_invalidate();
    assert!(!c.delivery_execute());
    c.delivery_prepare(
        destination.clone(),
        1,
        PathBuf::new(),
        false,
        true,
        Default::default(),
    );
    for _ in 0..3000 {
        if c.pump_delivery() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(c.delivery_execute());
    for _ in 0..3000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(destination.join("input.txt").is_file());
    assert!(input.is_file());
    let published = std::fs::canonicalize(&destination).unwrap();
    assert!(
        matches!(c.ops.undo_history.last().unwrap().actions.as_slice(),
        [naygo_core::ops::undo::UndoAction::TrashCreated { path }] if path == &published)
    );
    assert!(!c.delivery.open);
    c.pump_delivery();
    assert_eq!(c.delivery_result_location(false), Some(published));
    assert!(c.delivery_return_to_selection());
    assert_eq!(c.basket_action_paths(), vec![input]);
    assert!(c.basket_context);
}

#[test]
fn radar_bandeja_congela_fuentes_antes_de_cambiar_el_conjunto() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    c.last_area = area();
    let first = tmp.path().join("primero.txt");
    c.basket.add([first.clone()]);
    c.basket_select_all();
    c.basket_request_transfer(false);
    c.basket.add([tmp.path().join("posterior.txt")]);
    assert_eq!(c.destination_radar.as_ref().unwrap().sources, vec![first]);
}

#[test]
fn entrega_fallida_conserva_error_y_no_habilita_abrir_destino() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let input = work.path().join("source.txt");
    std::fs::write(&input, b"original").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), config.path().to_path_buf());
    c.basket.add([input.clone()]);
    c.basket_select(0);
    assert!(c.delivery_open());
    let destination = work.path().join("out");
    c.delivery_prepare(
        destination.clone(),
        1,
        PathBuf::new(),
        false,
        false,
        Default::default(),
    );
    for _ in 0..3000 {
        if c.pump_delivery() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(c.delivery.plan.is_some());
    std::fs::write(&input, b"changed after review").unwrap();
    assert!(c.delivery_execute());
    for _ in 0..3000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    c.pump_delivery();
    assert!(!destination.exists());
    assert!(c.ops.undo_history.is_empty());
    assert!(c.delivery_result_location(false).is_none());
    let summary = c.ops.active_ops.last().unwrap().summary.as_ref().unwrap();
    assert!(
        matches!(&summary.items[0].outcome, naygo_core::ops::OpOutcome::Failed(error) if error.contains("source changed"))
    );
    assert!(c
        .delivery_results
        .latest
        .as_ref()
        .unwrap()
        .message
        .contains("source changed"));
    c.basket_clear();
    assert!(c.delivery_return_to_selection());
    assert_eq!(c.basket_action_paths(), vec![input]);
}

#[test]
fn entrega_conflictos_bloquean_publicar_hasta_revisar_la_resolucion() {
    let config = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), config.path().to_path_buf());
    for name in ["one", "two"] {
        let folder = work.path().join(name);
        std::fs::create_dir(&folder).unwrap();
        let file = folder.join("same.txt");
        std::fs::write(&file, name).unwrap();
        c.basket.add([file]);
    }
    c.basket_select_all();
    assert!(c.delivery_open());
    assert_eq!(c.delivery.group_names.lines().count(), 2);
    let dest = work.path().join("out.zip");
    c.delivery_prepare(
        dest.clone(),
        1,
        PathBuf::new(),
        true,
        false,
        Default::default(),
    );
    for _ in 0..3000 {
        if c.pump_delivery() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(c.delivery.plan.is_none());
    assert!(c.delivery.report.contains("same.txt"));
    assert!(!c.delivery_execute());
    c.delivery_prepare(
        dest.clone(),
        1,
        PathBuf::new(),
        true,
        false,
        naygo_core::delivery::ReviewOptions {
            rename_flat_duplicates: true,
            ..Default::default()
        },
    );
    for _ in 0..3000 {
        if c.pump_delivery() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(c.delivery.plan.is_some(), "{}", c.delivery.report);
    assert!(c.delivery.report.contains("same (2).txt"));
    assert!(!dest.exists(), "revisar no publica");
    c.delivery_invalidate();
    assert!(!c.delivery_execute(), "editar obliga a revisar de nuevo");
}

#[test]
fn bandeja_metadata_basica_llega_del_worker_y_descarta_resultado_anterior() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("prueba.txt");
    let dir = tmp.path().join("carpeta");
    std::fs::write(&file, "12345").unwrap();
    std::fs::create_dir(&dir).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    c.basket.add([file.clone(), dir.clone()]);
    for (index, path) in [file, dir].into_iter().enumerate() {
        c.basket_select(index);
        // El dato anterior nunca se presenta como metadata de la nueva ruta.
        assert!(c.inspector_info().modified.is_empty());
        c.request_metadata(path);
        for _ in 0..3000 {
            if c.pump_meta() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(c.pump_meta());
        let info = c.inspector_info();
        assert_eq!(info.is_dir, index == 1);
        assert!(!info.modified.is_empty());
        if index == 0 {
            assert_eq!(info.size, naygo_core::format::human_size(5));
        }
    }
}

/// Drena el probe async de "carpeta no encontrada" hasta que aplica sus resultados (con
/// timeout), simulando los ticks del Timer. Devuelve true si no quedó probe en vuelo.
fn drain_missing(c: &mut WorkspaceCtrl) -> bool {
    for _ in 0..2000 {
        if c.pump_missing_probe() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

fn drain_sync(c: &mut WorkspaceCtrl) -> bool {
    for _ in 0..3000 {
        if c.pump_sync() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

fn drain_text_transform(c: &mut WorkspaceCtrl) -> bool {
    for _ in 0..3000 {
        if c.pump_text_transform() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    false
}

#[test]
fn vista_profunda_orden_y_filtro_comparten_identidad_y_orden_de_exportacion() {
    use super::listing_export::ExportDestination;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let keep_a = a.join("visible-z.txt");
    let hidden_a = a.join("oculto.txt");
    let keep_b = b.join("visible-a.txt");
    for path in [&keep_a, &hidden_a, &keep_b] {
        std::fs::write(path, b"contenido").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let first = c.deep_position_of(pane, &keep_a).unwrap();
    let second = c.deep_position_of(pane, &hidden_a).unwrap();
    assert!(c.deep_select_at(pane, first, false, false));
    assert!(c.deep_select_at(pane, second, true, false));
    c.sort_by_kind(pane, 0);
    let ordered: Vec<_> = (0..c.deep_visible_count(pane))
        .map(|pos| c.deep_visible_entry(pane, pos).unwrap().path.clone())
        .collect();
    assert_eq!(
        ordered,
        [&a, &hidden_a, &keep_a, &b, &keep_b]
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    );
    assert_eq!(c.deep_focused_position(pane), Some(1));
    c.column_menu_open(pane, 0, 0.0, 0.0);
    c.column_filter_set_text("visible");
    c.column_filter_apply();
    assert!(c.deep_focused_position(pane).is_none());
    assert!(c.deep_complete_selection(pane).is_none());
    let rows = c.rows_of(pane, 0, std::time::Instant::now());
    assert_eq!(
        rows.iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
        ["visible-z.txt", "visible-a.txt"]
    );
    c.deep_select_at(pane, 1, false, false);
    c.open_row_context_menu(pane, 1, 0.0, 0.0);
    assert_eq!(c.context_targets(), vec![keep_b]);
    c.deep_select_at(pane, 1, true, false);
    c.open_folder_context_menu(pane, 0.0, 0.0);
    let destination = output.path().join("visible.csv");
    assert!(c.start_listing_export(';', ExportDestination::File(destination.clone())));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let csv = std::fs::read_to_string(destination).unwrap();
    assert!(csv.find("visible-z.txt").unwrap() < csv.find("visible-a.txt").unwrap());
    assert!(!csv.contains("oculto.txt"));
}

#[test]
fn vista_profunda_exporta_lote_sin_reemplazar_si_se_cancela() {
    use super::listing_export::ExportDestination;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    let one_dir = root.path().join("uno");
    let two_dir = root.path().join("dos");
    std::fs::create_dir(&one_dir).unwrap();
    std::fs::create_dir(&two_dir).unwrap();
    let one = one_dir.join("a;uno.txt");
    let two = two_dir.join("b.txt");
    let other = root.path().join("no-elegido.txt");
    for p in [&one, &two, &other] {
        std::fs::write(p, b"contenido").unwrap();
    }
    let destination = output.path().join("resultado.csv");
    std::fs::write(&destination, b"anterior").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let p1 = c.deep_position_of(pane, &one).unwrap();
    let p2 = c.deep_position_of(pane, &two).unwrap();
    c.deep_select_at(pane, p1, false, false);
    c.deep_select_at(pane, p2, true, false);
    c.open_row_context_menu(pane, p1, 0.0, 0.0);
    assert!(c.start_listing_export(';', ExportDestination::File(destination.clone())));
    c.cancel_listing_export();
    assert!(c.pump_listing_export());
    assert_eq!(std::fs::read(&destination).unwrap(), b"anterior");
    c.open_row_context_menu(pane, p1, 0.0, 0.0);
    assert!(c.start_listing_export(';', ExportDestination::File(destination.clone())));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(c.listing_export.is_none());
    let bytes = std::fs::read(&destination).unwrap();
    assert!(bytes.starts_with(&[0xef, 0xbb, 0xbf]));
    let csv = std::str::from_utf8(&bytes[3..]).unwrap();
    assert!(csv.contains("\"a;uno.txt\""));
    assert!(csv.contains(&one.display().to_string()) && csv.contains(&two.display().to_string()));
    assert!(!csv.contains("no-elegido.txt"));
    assert!(csv.ends_with("\r\n"));
    c.open_folder_context_menu(pane, 0.0, 0.0);
    assert!(c.start_listing_export(';', ExportDestination::File(destination.clone())));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        bytes,
        "el menú de carpeta respeta la selección"
    );
    c.deep_select_at(pane, p1, true, false);
    c.deep_select_at(pane, p2, true, false);
    c.open_folder_context_menu(pane, 0.0, 0.0);
    assert!(c.start_listing_export(';', ExportDestination::File(destination.clone())));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let all_bytes = std::fs::read(&destination).unwrap();
    assert!(std::str::from_utf8(&all_bytes[3..])
        .unwrap()
        .contains("no-elegido.txt"));
    c.open_path_folder_context_menu(pane, one_dir.clone(), 0.0, 0.0);
    assert!(!c.start_listing_export(';', ExportDestination::File(destination.clone())));
    c.open_folder_context_menu(pane, 0.0, 0.0);
    assert!(c.start_listing_export(';', ExportDestination::File(destination.clone())));
    c.deep_cancel();
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
    }
    assert!(
        c.listing_export.is_none(),
        "un recorrido reemplazado invalida la instantánea"
    );
    assert!(c.pending_listing_error.is_some());
    assert_eq!(std::fs::read(&destination).unwrap(), all_bytes);
}

#[test]
fn exportar_listado_usa_vista_o_seleccion_y_columnas_visibles() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a;uno.txt"), b"a").unwrap();
    std::fs::write(work.path().join("b.txt"), b"bb").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));

    let all = c.export_listing_csv(';').expect("panel activo");
    assert_eq!(all.lines().count(), 3, "encabezado + dos filas");
    assert_eq!(all.lines().next().unwrap().matches(';').count(), 4);
    assert!(all.contains("\"a;uno.txt\""), "CSV escapa el separador");
    assert!(all.contains("b.txt"));

    let output = tempfile::tempdir().unwrap();
    let csv_path = output.path().join("listado.csv");
    let pane = c.active_id().unwrap();
    c.open_folder_context_menu(pane, 0.0, 0.0);
    assert!(c.start_listing_export(
        ';',
        super::listing_export::ExportDestination::File(csv_path.clone())
    ));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let bytes = std::fs::read(&csv_path).unwrap();
    assert_eq!(&bytes[..3], &[0xef, 0xbb, 0xbf]);
    assert_eq!(std::str::from_utf8(&bytes[3..]).unwrap(), all);

    c.ws.active_files_mut().unwrap().select_single(0);
    let selected = c.export_listing_csv(';').unwrap();
    assert_eq!(selected.lines().count(), 2, "encabezado + una fila");
    c.open_folder_context_menu(pane, 0.0, 0.0);
    assert!(c.start_listing_export(
        ';',
        super::listing_export::ExportDestination::File(csv_path.clone())
    ));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let bytes = std::fs::read(&csv_path).unwrap();
    assert_eq!(std::str::from_utf8(&bytes[3..]).unwrap(), selected);
    let files = c.ws.active_files_mut().unwrap();
    files.clear_selection();
    c.typeahead = "b".to_string();
    c.filter_hide_nonmatches = true;
    c.sync_visual_filter();
    let filtered = c.export_listing_csv(';').unwrap();
    assert!(filtered.contains("b.txt") && !filtered.contains("a;uno.txt"));
    c.open_folder_context_menu(pane, 0.0, 0.0);
    assert!(c.start_listing_export(
        ';',
        super::listing_export::ExportDestination::File(csv_path.clone())
    ));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let bytes = std::fs::read(&csv_path).unwrap();
    assert_eq!(std::str::from_utf8(&bytes[3..]).unwrap(), filtered);
}

#[test]
fn exportacion_normal_respeta_orden_y_rechaza_cambios_en_dialogo_y_snapshot() {
    use super::listing_export::ExportDestination;
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let output = tempfile::tempdir().unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(work.path().join(name), b"x").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    let a = active_pos_of(&c, "a.txt").unwrap();
    let b = active_pos_of(&c, "b.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(b);
    c.ws.active_files_mut().unwrap().select_toggle(a);
    let expected = c.export_listing_csv(',').unwrap();
    let destination = output.path().join("orden.csv");
    c.open_row_context_menu(pane, b, 0.0, 0.0);
    let intent = c.capture_export_intent().unwrap();
    assert!(c.export_intent_is_current(&intent));
    c.open_row_context_menu(pane, b, 0.0, 0.0);
    assert!(
        !c.export_intent_is_current(&intent),
        "un menú nuevo invalida el anterior"
    );
    assert!(c.start_listing_export(',', ExportDestination::File(destination.clone())));
    for _ in 0..4000 {
        if c.pump_listing_export() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let bytes = std::fs::read(&destination).unwrap();
    assert_eq!(
        std::str::from_utf8(&bytes[3..]).unwrap(),
        expected,
        "mantiene orden de selección"
    );
    c.open_folder_context_menu(pane, 0.0, 0.0);
    let intent = c.capture_export_intent().unwrap();
    c.ws.active_files_mut().unwrap().select_single(a);
    assert!(
        !c.export_intent_is_current(&intent),
        "cambia la selección mientras se elige archivo"
    );
    c.open_folder_context_menu(pane, 0.0, 0.0);
    assert!(c.start_listing_export(',', ExportDestination::File(destination.clone())));
    c.ws.active_files_mut().unwrap().entries_changed();
    for _ in 0..100 {
        if c.pump_listing_export() {
            break;
        }
    }
    assert!(c.listing_export.is_none());
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        bytes,
        "una vista invalidada no reemplaza el archivo"
    );
}

#[test]
fn exportacion_csv_muestra_tipo_carpeta_aunque_el_nombre_tenga_puntos() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir(work.path().join("1.The Buddha")).unwrap();
    std::fs::create_dir(work.path().join("2.3DBenchy")).unwrap();
    std::fs::write(work.path().join("pieza.stl"), b"stl").unwrap();

    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    c.config.set_language(naygo_core::i18n::LangId::new("es"));
    assert!(drain(&mut c));
    let csv = c.export_listing_csv(';').expect("panel activo");

    assert!(csv.contains(";Carpeta;"));
    assert!(!csv.contains(";buddha;"));
    assert!(!csv.contains(";3dbenchy;"));
    assert!(csv.contains(";stl;"));
}

#[test]
fn bandeja_reune_seleccion_y_la_deduplica() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"a").unwrap();
    std::fs::write(work.path().join("b.txt"), b"b").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ws.active_files_mut().unwrap().select_all();
    assert_eq!(c.basket_add_selected(), 2);
    assert_eq!(c.basket_add_selected(), 0);
    assert_eq!(c.basket_rows().len(), 2);
    assert!(c.basket_remove(0));
    assert_eq!(c.basket.len(), 1);
}

#[test]
fn asistente_sync_planifica_en_worker_y_publica_preview() {
    let cfg = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let left = tmp.path().join("left");
    let right = tmp.path().join("right");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    std::fs::write(left.join("nuevo.txt"), b"hola").unwrap();
    let mut c = WorkspaceCtrl::new_in(left.clone(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.ws.active_id().unwrap();
    let second = c.ws.add_pane(PanePurpose::Files, right.clone());
    c.ws.layout.split_leaf(first, SplitDir::Horizontal, second);
    c.ws.set_active(first);
    c.last_active_files = Some(first);
    assert!(c.sync_open());
    assert!(drain_sync(&mut c));
    let vm = c.sync_vm();
    assert!(vm.active);
    assert_eq!(vm.rows.row_count(), 1);
    assert!(vm.can_apply);
    c.sync_close();
}

#[test]
fn vista_profunda_transformar_texto_conserva_lote_y_relista() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let a_dir = work.path().join("a");
    let b_dir = work.path().join("b");
    std::fs::create_dir(&a_dir).unwrap();
    std::fs::create_dir(&b_dir).unwrap();
    let a = a_dir.join("uno.txt");
    let b = b_dir.join("dos.txt");
    let untouched = work.path().join("intacto.txt");
    for path in [&a, &b, &untouched] {
        std::fs::write(path, b"uno\r\ndos\r\n").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let pa = c.deep_position_of(pane, &a).unwrap();
    let pb = c.deep_position_of(pane, &b).unwrap();
    let pc = c.deep_position_of(pane, &untouched).unwrap();
    c.deep_select_at(pane, pa, false, false);
    c.deep_select_at(pane, pb, true, false);
    c.open_row_context_menu(pane, pa, 0.0, 0.0);
    c.deep_select_at(pane, pc, false, false);
    assert!(c.text_transform_open());
    c.close_context_menu();
    assert_eq!(c.text_transform.as_ref().unwrap().paths.len(), 2);
    assert!(drain_text_transform(&mut c));
    c.text_transform_set_line(1);
    assert!(c.text_transform_apply());
    assert!(drain_text_transform(&mut c));
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(std::fs::read(&a).unwrap(), b"uno\ndos\n");
    assert_eq!(std::fs::read(&b).unwrap(), b"uno\ndos\n");
    assert_eq!(std::fs::read(&untouched).unwrap(), b"uno\r\ndos\r\n");
    let entry = c
        .deep_job
        .as_ref()
        .unwrap()
        .items
        .iter()
        .find(|(entry, _)| entry.path == a)
        .unwrap();
    assert_eq!(entry.0.size, Some(8));
    c.text_transform_close();
    c.deep_job
        .as_mut()
        .unwrap()
        .selected_paths
        .insert(work.path().join("ausente.txt"));
    assert!(!c.text_transform_open());
    assert!(c.take_deep_selection_notice());
}

#[test]
fn transformar_texto_desde_controlador_es_async_y_seguro() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let path = work.path().join("a.txt");
    std::fs::write(&path, b"uno\r\ndos\r\n").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pos = active_pos_of(&c, "a.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    assert!(c.text_transform_open());
    assert!(drain_text_transform(&mut c));
    c.text_transform_set_line(1);
    assert!(c.text_transform_apply());
    assert!(drain_text_transform(&mut c));
    assert_eq!(std::fs::read(&path).unwrap(), b"uno\ndos\n");
    assert!(c.text_transform_vm().finished);
}

#[test]
fn dialogo_sync_deja_conflictos_visibles_pero_no_seleccionables() {
    let cfg = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let left = tmp.path().join("left");
    let right = tmp.path().join("right");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    std::fs::write(left.join("dato.txt"), b"izquierda").unwrap();
    std::fs::write(right.join("dato.txt"), b"derecha-distinta").unwrap();
    let mut c = WorkspaceCtrl::new_in(left, cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.ws.active_id().unwrap();
    let second = c.ws.add_pane(PanePurpose::Files, right);
    c.ws.layout.split_leaf(first, SplitDir::Horizontal, second);
    c.ws.set_active(first);
    c.last_active_files = Some(first);
    assert!(c.sync_open());
    c.sync_set_mode(2);
    assert!(drain_sync(&mut c));
    let vm = c.sync_vm();
    assert_eq!(vm.mode, 2);
    assert_eq!(vm.rows.row_count(), 1);
    let row = vm.rows.row_data(0).unwrap();
    assert!(!row.enabled, "el conflicto exige una decisión consciente");
    assert!(
        !row.selected,
        "nunca se preselecciona una dirección arbitraria"
    );
    assert!(!vm.can_apply, "solo conflictos no habilitan Aplicar");
}

#[test]
fn dialogo_sync_replanifica_al_activar_borrado_de_sobrantes() {
    let cfg = tempfile::tempdir().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let left = tmp.path().join("left");
    let right = tmp.path().join("right");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    std::fs::write(right.join("extra.txt"), b"extra").unwrap();
    let mut c = WorkspaceCtrl::new_in(left, cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.ws.active_id().unwrap();
    let second = c.ws.add_pane(PanePurpose::Files, right);
    c.ws.layout.split_leaf(first, SplitDir::Horizontal, second);
    c.ws.set_active(first);
    c.last_active_files = Some(first);
    assert!(c.sync_open());
    assert!(drain_sync(&mut c));
    assert_eq!(c.sync_vm().rows.row_count(), 0, "apagado por defecto");
    let empty_plan = c.sync_rows_fingerprint();
    c.sync_set_delete_extras(true);
    assert!(c.sync_vm().planning, "el cambio dispara un worker nuevo");
    let planning = c.sync_rows_fingerprint();
    assert_ne!(
        planning, empty_plan,
        "replanificar cambia la huella de filas"
    );
    assert!(drain_sync(&mut c));
    let planned = c.sync_rows_fingerprint();
    assert_ne!(planned, planning, "llegar el plan cambia la huella");
    assert_eq!(
        c.sync_rows_fingerprint(),
        planned,
        "sin cambios, la huella es estable (las filas no se rearman por tick)"
    );
    let vm = c.sync_vm();
    assert!(vm.delete_extras);
    assert_eq!(vm.rows.row_count(), 1);
    let row = vm.rows.row_data(0).unwrap();
    assert!(row.enabled && row.selected);
    assert!(vm.can_apply);
    c.sync_toggle_item(0);
    assert_ne!(
        c.sync_rows_fingerprint(),
        planned,
        "marcar una fila cambia la huella"
    );
    assert!(!c.sync_rows_vm()[0].selected);
    c.sync_close();
    assert_eq!(c.sync_rows_fingerprint(), None);
}

#[test]
fn dialogo_texto_refleja_todas_las_opciones_en_su_view_model() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"uno\r\ndos\r\n").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pos = active_pos_of(&c, "a.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    assert!(c.text_transform_open());
    assert!(drain_text_transform(&mut c));
    c.text_transform_set_line(2);
    c.text_transform_set_encoding(4);
    c.text_transform_set_final(1);
    c.text_transform_set_trim(true);
    let vm = c.text_transform_vm();
    assert!(vm.active && vm.can_apply);
    assert_eq!(vm.count, 1);
    assert_eq!(vm.target_line, 2);
    assert_eq!(vm.target_encoding, 4);
    assert_eq!(vm.final_newline, 1);
    assert!(vm.trim);
    assert!(vm.source.contains("UTF-8") && vm.source.contains("CRLF"));
}

#[test]
fn dialogo_texto_binario_invalido_informa_y_bloquea_aplicar() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("bin.dat"), b"abc\0def").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pos = active_pos_of(&c, "bin.dat").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    assert!(c.text_transform_open());
    assert!(drain_text_transform(&mut c));
    let vm = c.text_transform_vm();
    assert!(vm.active);
    assert!(!vm.can_apply);
    assert!(
        !vm.source.is_empty(),
        "el diagnóstico expone un error visible"
    );
    c.text_transform_close();
    assert!(!c.text_transform_vm().active);
}

#[test]
fn bandeja_vacia_no_inicia_operaciones_y_limpia_su_estado_visual() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    assert!(c.basket_rows().is_empty());
    assert!(!c.basket_transfer_to(work.path().join("destino"), false));
    assert!(!c.basket_delete());
    assert!(c.basket_rows().is_empty());
}

fn active_pos_of(c: &WorkspaceCtrl, name: &str) -> Option<usize> {
    let f = c.ws.active_files()?;
    f.view_indices()
        .iter()
        .position(|&real| f.entries[real].name == name)
}

#[test]
fn parse_size_acepta_sufijos_y_vacio() {
    assert_eq!(parse_size(""), None);
    assert_eq!(parse_size("  "), None);
    assert_eq!(parse_size("10"), Some(10));
    assert_eq!(parse_size("2kb"), Some(2048));
    assert_eq!(parse_size("2 KB"), Some(2048));
    assert_eq!(parse_size("1m"), Some(1024 * 1024));
    assert_eq!(parse_size("1.5M"), Some(1024 * 1024 + 512 * 1024));
    assert_eq!(parse_size("3gb"), Some(3 * 1024 * 1024 * 1024));
    assert_eq!(parse_size("1,024"), Some(1024));
    assert_eq!(parse_size("no"), None);
}

#[test]
fn fecha_ymd_round_trip_en_utc() {
    // En UTC (tz=0): 2026-06-16 al inicio del día, y de vuelta a la misma cadena.
    let t = parse_date_ymd("2026-06-16", 0, false).unwrap();
    assert_eq!(fmt_date_ymd(t, 0), "2026-06-16");
    // Fin del día sigue siendo el mismo día (23:59:59).
    let end = parse_date_ymd("2026-06-16", 0, true).unwrap();
    assert_eq!(fmt_date_ymd(end, 0), "2026-06-16");
    assert!(end > t, "fin de día es posterior al inicio");
    // Inválidas y vacías → None.
    assert_eq!(parse_date_ymd("", 0, false), None);
    assert_eq!(parse_date_ymd("2026-13-01", 0, false), None);
    assert_eq!(parse_date_ymd("nope", 0, false), None);
}

/// El menú de columna aplica un filtro de texto sobre Name y la vista se refiltra sola; al
/// quitarlo, vuelven todas las filas. Cubre column_menu_open → set_text → apply → clear.
#[test]
fn menu_de_columna_filtra_y_limpia_por_nombre() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("informe.pdf"), b"x").unwrap();
    std::fs::write(work.path().join("notas.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    assert_eq!(c.ws.active_files().unwrap().view_len(), 2);

    // Abrir menú sobre la columna Name (kind 0), pasar a filtro, escribir y aplicar.
    c.column_menu_open(id, 0, 0.0, 0.0);
    c.column_menu_to_filter();
    c.column_filter_set_text("informe");
    c.column_filter_apply();
    assert!(c.column_menu.is_none(), "aplicar cierra el menú");
    assert_eq!(
        c.ws.active_files().unwrap().view_len(),
        1,
        "el filtro deja solo informe.pdf"
    );
    assert!(!c.no_matches(id), "hay una coincidencia");

    // Quitar el filtro: vuelven las dos filas.
    c.column_menu_open(id, 0, 0.0, 0.0);
    c.column_menu_clear_filter();
    assert_eq!(c.ws.active_files().unwrap().view_len(), 2);
}

/// La carpeta destino de "abrir terminal aquí" es la subcarpeta seleccionada si la hay, y si
/// no, la carpeta del panel activo.
#[test]
fn terminal_dir_usa_carpeta_seleccionada_o_actual() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let sub = work.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(work.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));

    // Sin selección de carpeta → la carpeta del panel.
    assert_eq!(c.terminal_dir().as_deref(), Some(work.path()));

    // Seleccionar la subcarpeta → terminal apunta a la subcarpeta.
    let pos = active_pos_of(&c, "sub").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    // El menú contextual toma la selección como objetivo.
    c.open_context_menu(c.ws.active_id().unwrap(), 0.0, 0.0);
    assert_eq!(c.terminal_dir().as_deref(), Some(sub.as_path()));

    // Seleccionar un ARCHIVO → cae a la carpeta del panel (no se abre terminal en un archivo).
    c.column_menu_close();
    let posf = active_pos_of(&c, "a.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(posf);
    c.open_context_menu(c.ws.active_id().unwrap(), 0.0, 0.0);
    assert_eq!(c.terminal_dir().as_deref(), Some(work.path()));
}

/// O-1: la firma de filas por panel detecta TODO lo que cambia una fila pintada y, sin
/// cambios, es estable entre llamadas (para saltarse la reconstrucción). Cubre: estabilidad,
/// selección, foco, columnas, formato de tamaño y de fecha, contenido de las entries (Modified
/// in situ), conjunto cortado, y el caso "fresco" → None (no cachear).
#[test]
fn rows_signature_detecta_cambios_y_es_estable() {
    use std::time::Instant;
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"x").unwrap();
    std::fs::write(work.path().join("b.txt"), b"yy").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    let now = Instant::now();
    let secs = c.highlight_secs();

    // Sin nada fresco, la firma es Some y ESTABLE entre dos llamadas idénticas.
    let s0 = c.rows_signature(id, secs, now).expect("sin fresh: Some");
    let s0b = c.rows_signature(id, secs, now).expect("Some");
    assert_eq!(s0, s0b, "dos llamadas sin cambios → misma firma");

    // Cambiar la SELECCIÓN cambia la firma.
    let pos = active_pos_of(&c, "a.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    let s_sel = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s0, s_sel, "seleccionar cambia la firma");
    // Y vuelve a ser estable tras el cambio.
    assert_eq!(s_sel, c.rows_signature(id, secs, now).unwrap());

    // Cambiar el FOCO (sin tocar la selección, Ctrl+↓) cambia la firma.
    c.ws.active_files_mut().unwrap().move_focus_keep(1);
    let s_focus = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s_sel, s_focus, "mover el foco cambia la firma");

    // Cambiar las COLUMNAS visibles cambia la firma (Created está oculta por defecto → mostrar).
    c.column_toggle(
        id,
        crate::bridge::column_kind_to_int(naygo_core::columns::ColumnKind::Created),
    );
    let s_cols = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s_focus, s_cols, "cambiar columnas visibles cambia la firma");

    // Cambiar el FORMATO DE TAMAÑO cambia la firma (afecta el texto de la celda Size).
    let before = c.rows_signature(id, secs, now).unwrap();
    c.config.settings.size_format = naygo_core::format::SizeFormat::Bytes;
    let s_size = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(before, s_size, "cambiar size_format cambia la firma");

    // Cambiar el FORMATO DE FECHA cambia la firma.
    c.config.settings.date_format = naygo_core::format::DateFormat::DmyDate;
    let s_date = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s_size, s_date, "cambiar date_format cambia la firma");

    // Mutar una entry IN SITU (como un DirEvent::Modified: mismo len, distinto tamaño) cambia
    // la firma. Es el caso que un hash solo-por-len se perdería (fila desactualizada).
    let s_pre_mod = c.rows_signature(id, secs, now).unwrap();
    {
        let f = c.ws.active_files_mut().unwrap();
        if let Some(e) = f.entries.iter_mut().find(|e| e.name == "a.txt") {
            e.size = Some(99_999);
        }
        f.entries_changed();
    }
    let s_mod = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(
        s_pre_mod, s_mod,
        "mutar el tamaño de una entry cambia la firma"
    );

    // Marcar una ruta como CORTADA cambia la firma (la fila se atenúa).
    let s_pre_cut = c.rows_signature(id, secs, now).unwrap();
    // Esta prueba comprueba la firma, no el portapapeles global (puede estar ocupado
    // por otra aplicación o prueba). La marca equivale a un corte ya aceptado por el SO.
    c.ops.cut_set.insert(work.path().join("a.txt"));
    let s_cut = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s_pre_cut, s_cut, "cortar una ruta cambia la firma");

    // Resaltado FRESCO vigente: la firma lo incluye como CONJUNTO (antes era `None` y cada tick
    // reconstruía todas las filas). Cambia al empezar, es estable mientras dura y vuelve al
    // valor previo al vencer.
    let s_pre_fresh = c.rows_signature(id, secs, now).unwrap();
    c.watchers
        .mark_fresh(id.0, vec![work.path().join("a.txt")], now);
    let s_fresh = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s_pre_fresh, s_fresh, "empezar un resaltado cambia la firma");
    let mid = now + std::time::Duration::from_millis(secs * 500);
    assert_eq!(
        c.rows_signature(id, secs, mid),
        Some(s_fresh),
        "mientras el resaltado dura, la firma es estable (cacheable)"
    );
    assert_eq!(
        c.rows_context_signature(id, secs, now),
        c.rows_context_signature(id, secs, mid),
        "el contexto de deltas no depende del resaltado"
    );
    let later = now + std::time::Duration::from_secs(secs + 1);
    assert_eq!(
        c.rows_signature(id, secs, later),
        Some(s_pre_fresh),
        "vencido el resaltado, vuelve la firma previa"
    );
}

/// Clic en el encabezado de un panel secundario: ordena ESE panel, sin robar el foco ni tocar
/// el orden del panel que estaba activo. Regresión de `sort_by_kind` usando active_files_mut().
#[test]
fn ordenar_por_header_respeta_el_panel_del_clic() {
    let cfg = tempfile::tempdir().unwrap();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    std::fs::write(left.path().join("left-small.txt"), b"x").unwrap();
    std::fs::write(left.path().join("left-large.txt"), vec![0u8; 20]).unwrap();
    std::fs::write(right.path().join("right-small.txt"), b"x").unwrap();
    std::fs::write(right.path().join("right-large.txt"), vec![0u8; 20]).unwrap();

    let mut c = WorkspaceCtrl::new_in(left.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let active = c.active_id().unwrap();
    c.add_pane_split(area());
    let clicked = c.active_id().unwrap();
    c.navigate_pane_to(clicked, right.path().to_path_buf());
    assert!(drain(&mut c));
    // Dejar el panel izquierdo activo como en el caso real: se clickea el header del derecho.
    c.set_active(active);

    c.sort_by_kind(clicked, 2); // Size

    assert_eq!(c.active_id(), Some(active), "ordenar no roba el foco");
    let active_sort = c.ws.pane(active).unwrap().files.as_ref().unwrap().sort;
    let clicked_files = c.ws.pane(clicked).unwrap().files.as_ref().unwrap();
    assert_eq!(active_sort.key, naygo_core::fs_model::SortKey::Name);
    assert_eq!(clicked_files.sort.key, naygo_core::fs_model::SortKey::Size);
    assert!(clicked_files.sort.ascending);
}

#[test]
fn comparar_paneles_marca_distintos_y_exclusivos_y_luego_limpia() {
    let cfg = tempfile::tempdir().unwrap();
    let left_dir = tempfile::tempdir().unwrap();
    let right_dir = tempfile::tempdir().unwrap();
    std::fs::write(left_dir.path().join("igual.txt"), b"abc").unwrap();
    std::fs::write(right_dir.path().join("igual.txt"), b"abc").unwrap();
    std::fs::write(left_dir.path().join("cambio.bin"), b"1").unwrap();
    std::fs::write(right_dir.path().join("cambio.bin"), b"12345").unwrap();
    std::fs::write(left_dir.path().join("solo-izq.txt"), b"x").unwrap();
    std::fs::write(right_dir.path().join("solo-der.txt"), b"x").unwrap();

    let mut c = WorkspaceCtrl::new_in(left_dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let left = c.ws.active_id().unwrap();
    c.add_pane_split(area());
    let right = *c.ws.files_panes().last().unwrap();
    assert!(c.navigate_pane_to(right, right_dir.path().to_path_buf()));
    assert!(drain(&mut c));

    // Normalizar la fecha del par igual: la comparación de fechas es deliberadamente exacta.
    let same_time = std::time::SystemTime::UNIX_EPOCH;
    for id in [left, right] {
        let f = c.ws.pane_mut(id).unwrap().files.as_mut().unwrap();
        f.entries
            .iter_mut()
            .for_each(|e| e.modified = Some(same_time));
        f.entries_changed();
    }
    c.ws.set_active(left);
    assert!(c.toggle_compare_panels());
    assert_eq!(c.comparison[&left][&left_dir.path().join("cambio.bin")], 1);
    assert_eq!(
        c.comparison[&right][&right_dir.path().join("cambio.bin")],
        1
    );
    assert_eq!(
        c.comparison[&left][&left_dir.path().join("solo-izq.txt")],
        2
    );
    assert_eq!(
        c.comparison[&right][&right_dir.path().join("solo-der.txt")],
        2
    );
    assert!(!c.comparison[&left].contains_key(&left_dir.path().join("igual.txt")));

    // Las acciones deben usar el panel que originó el clic, aun si el otro sigue activo.
    assert!(c.comparison_select_for(right, 2));
    let selected_right = c.selected_paths_of(right);
    assert_eq!(selected_right, vec![right_dir.path().join("solo-der.txt")]);
    assert!(c.comparison_toggle_link(right));
    assert!(c.comparison_link_enabled);

    assert!(c.toggle_compare_panels());
    assert!(c.comparison.is_empty());
    assert!(c.comparison_pair.is_none());
    assert!(!c.comparison_link_enabled);
}

#[test]
fn bandeja_naygolist_preserva_referencias_ausentes() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    let missing = dir.path().join("ya-no-esta.txt");
    c.basket.add([missing.clone()]);
    let text = c
        .basket_naygolist_json(Some(dir.path().to_path_buf()))
        .unwrap();
    c.basket_clear();
    assert_eq!(c.basket_import_naygolist_json(&text).unwrap(), 1);
    assert_eq!(c.basket.items(), &[missing]);
}

/// C3: agregar/alternar/aliasar/quitar reglas de previsualización; persisten.
#[test]
fn reglas_de_preview_crud() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    // Agregar una extensión nueva (normaliza el punto y mayúsculas). Nace en Auto (modo 0).
    c.preview_rule_add(".SIF");
    assert!(c
        .preview_rules()
        .iter()
        .any(|(e, on, v, _)| e == "sif" && *on && *v == 0));
    // No duplica.
    c.preview_rule_add("sif");
    assert_eq!(
        c.preview_rules()
            .iter()
            .filter(|(e, _, _, _)| e == "sif")
            .count(),
        1
    );
    // Forzar el modo a Código + lenguaje XML (índice 0 en CodeLang::all()).
    c.preview_rule_set_view_mode("sif", 3);
    c.preview_rule_set_view_lang("sif", 0);
    assert!(c
        .preview_rules()
        .iter()
        .any(|(e, _, v, l)| e == "sif" && *v == 3 && *l == 0));
    // Alternar.
    c.preview_rule_toggle("sif");
    assert!(c
        .preview_rules()
        .iter()
        .any(|(e, on, _, _)| e == "sif" && !*on));
    c.config
        .wait_for_settings_save(std::time::Duration::from_secs(5))
        .unwrap();
    // Persistió: reabrir y la regla sigue (modo Código + XML).
    let c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(c2
        .preview_rules()
        .iter()
        .any(|(e, _, v, l)| e == "sif" && *v == 3 && *l == 0));
    // Quitar.
    c.preview_rule_remove("sif");
    assert!(!c.preview_rules().iter().any(|(e, _, _, _)| e == "sif"));
}

/// C4: guardar la tabla del panel activo como plantilla → un panel Files nuevo nace con esas
/// columnas (no las default). Limpiar la plantilla restaura el comportamiento por defecto.
#[test]
fn plantilla_de_tabla_por_defecto_se_aplica_a_paneles_nuevos() {
    use naygo_core::columns::ColumnKind;
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Ocultar "Extensión" en el panel activo y guardarlo como plantilla.
    c.ws.active_files_mut()
        .unwrap()
        .table
        .toggle_visible(ColumnKind::Extension);
    c.save_default_table_from_active();
    assert!(c.config.settings.default_table.is_some());
    // Un panel nuevo (split) hereda la plantilla: Extensión oculta.
    c.add_pane_split(area());
    let new_id = *c.ws.files_panes().last().unwrap();
    let ext_visible =
        c.ws.pane(new_id)
            .unwrap()
            .files
            .as_ref()
            .unwrap()
            .table
            .columns
            .iter()
            .find(|col| col.kind == ColumnKind::Extension)
            .unwrap()
            .visible;
    assert!(
        !ext_visible,
        "el panel nuevo hereda la plantilla (Extensión oculta)"
    );
    // Limpiar la plantilla.
    c.clear_default_table();
    assert!(c.config.settings.default_table.is_none());
}

/// Cerrar un panel: con dos paneles, `close_pane` quita uno y deja el otro; el último panel
/// NO se puede cerrar (can_close_pane = false) para no dejar la ventana vacía.
#[test]
fn cerrar_panel_quita_uno_y_protege_el_ultimo() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Un solo panel: no se puede cerrar.
    let first = *c.ws.files_panes().first().unwrap();
    assert!(!c.can_close_pane(first));
    // Agregar un segundo panel y cerrarlo: vuelve a quedar uno.
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let second = *c.ws.files_panes().last().unwrap();
    assert!(c.can_close_pane(second));
    c.close_pane(second);
    assert_eq!(c.ws.panes().len(), 1, "queda un solo panel tras cerrar");
    assert!(c.ws.pane(second).is_none(), "el panel cerrado ya no existe");
    // El que queda no se puede cerrar.
    let remaining = *c.ws.files_panes().first().unwrap();
    assert!(!c.can_close_pane(remaining));
}

/// F3 calcula el tamaño de la carpeta del panel: spawnea el worker, se drena hasta terminar,
/// y la barra de estado muestra el total.
#[test]
fn calcular_tamano_de_carpeta() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.bin"), vec![0u8; 1000]).unwrap();
    std::fs::write(work.path().join("b.bin"), vec![0u8; 2000]).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Sin foco en una subcarpeta → calcula la carpeta del panel (work).
    c.compute_size_active();
    assert!(c.size_job.is_some());
    // Drenar el worker hasta que termine.
    for _ in 0..3000 {
        if c.pump_sizes() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let job = c.size_job.as_ref().unwrap();
    assert!(job.done);
    assert_eq!(job.bytes, 3000, "suma de los dos archivos");
    // La barra de estado anexa el resultado (el nombre de la carpeta calculada).
    let name = work
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        c.status_line().contains(&name),
        "el resultado del cálculo aparece en el status: {}",
        c.status_line()
    );
}

/// El buscador F3 acepta una raíz explícita, filtra nombre+contenido en un worker y conserva
/// rutas relativas para que el resultado sea utilizable como una carpeta virtual.
#[test]
fn busqueda_f3_por_nombre_y_contenido_desde_raiz_elegida() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let root = work.path().join("base");
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(root.join("cosa.txt"), "sin la palabra requerida").unwrap();
    let expected = sub.join("cosa_final.txt");
    std::fs::write(&expected, "texto AQUÍ buscable").unwrap();
    std::fs::write(sub.join("otra.log"), "AQUÍ pero extensión distinta").unwrap();

    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.start_search(
        "*cosa*.txt".into(),
        root.to_string_lossy().into_owned(),
        "aquí".into(),
        true,
        true,
        true,
    );
    for _ in 0..3000 {
        if c.pump_search() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let job = c.search_job.as_ref().expect("la búsqueda queda abierta");
    assert!(job.done);
    assert_eq!(job.hits.len(), 1);
    assert_eq!(job.hits[0].entry.path, expected);
    assert_eq!(job.hits[0].rel_dir, "sub");

    c.reveal_search_hit(0);
    assert!(
        c.ws.panes().iter().any(|pane| {
            pane.files
                .as_ref()
                .is_some_and(|files| files.current_dir == sub)
        }),
        "revelar crea o reutiliza un panel Files para la carpeta contenedora"
    );
    assert!(c.search_open(), "revelar no cierra los resultados");
}

/// F3 crea un panel Search real del workspace, no un diálogo superpuesto. Al cerrarlo, el job
/// asociado se cancela para no dejar un worker o resultados huérfanos en la sesión.
#[test]
fn busqueda_f3_es_panel_acoplable_y_se_limpia_al_cerrar() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));

    c.open_search_pane(area());
    let search =
        c.ws.panes()
            .iter()
            .find(|pane| pane.purpose == PanePurpose::Search)
            .map(|pane| pane.id)
            .expect("F3 agrega un panel Search al workspace");
    assert_eq!(c.ws.active_id(), Some(search));
    assert!(
        c.search_open(),
        "el panel parte listo para recibir la consulta"
    );

    c.close_pane(search);
    assert!(
        !c.ws
            .panes()
            .iter()
            .any(|pane| pane.purpose == PanePurpose::Search),
        "cerrar el panel lo retira del layout"
    );
    assert!(
        !c.search_open(),
        "cerrar el panel libera la búsqueda asociada"
    );
}

/// «Abrir carpeta contenedora» no adivina el panel al que navegar: con varios exploradores
/// disponibles deja la elección pendiente para que el selector de destino la muestre al usuario.
#[test]
fn revelar_resultado_de_busqueda_pide_panel_destino() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let hit = work.path().join("resultado.txt");
    std::fs::write(&hit, "contenido").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.add_pane_split(area());
    c.add_pane_split(area());
    assert!(drain(&mut c));

    c.start_search(
        "resultado.txt".into(),
        work.path().to_string_lossy().into_owned(),
        String::new(),
        true,
        false,
        false,
    );
    for _ in 0..3000 {
        if c.pump_search() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(c.search_job.as_ref().unwrap().hits.len(), 1);

    c.reveal_search_hit(0);
    assert!(
        c.pending_pick.is_some(),
        "hay dos paneles Files alternativos: se debe preguntar el destino"
    );
    assert_eq!(c.pending_pick.as_ref().unwrap().candidates.len(), 2);
}

/// El worker de metadata lee las dimensiones de un PNG enfocado: `request_metadata` lanza el
/// job, se drena con `pump_meta` hasta terminar, y `meta_fields()` trae la clave i18n + el valor.
#[test]
fn metadata_de_imagen_reporta_dimensiones() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    // PNG 4x2 mínimo (el proveedor de imágenes de core lee solo la cabecera).
    let png = work.path().join("pic.png");
    image::RgbaImage::new(4, 2).save(&png).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Pedir la metadata del PNG y drenar el worker hasta que responda.
    c.request_metadata(png.clone());
    assert!(c.meta_loading(), "el job arranca en estado de lectura");
    let mut done = false;
    for _ in 0..3000 {
        if c.pump_meta() {
            done = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(done, "el worker de metadata debe terminar");
    assert!(!c.meta_loading(), "ya no está leyendo");
    // Se compara contra la CLAVE i18n (la traducción ocurre al construir el VM), no contra el
    // literal: el runner de CI corre en inglés y no debe romper por el idioma.
    assert!(
        c.meta_fields()
            .iter()
            .any(|(key, value)| key == "meta.dimensions" && value == "4 × 2"),
        "debe reportar las dimensiones 4 × 2: {:?}",
        c.meta_fields()
    );
    // Enfocar el mismo path no relanza (el job sigue siendo el mismo, ya terminado).
    c.request_metadata(png.clone());
    assert!(!c.meta_loading(), "no relanza para el mismo archivo");
    // Limpiar cancela el job y deja los campos vacíos.
    c.clear_metadata();
    assert!(c.meta_fields().is_empty());
    assert!(!c.meta_loading());
}

/// Navegación por teclado del árbol: ↓ mueve el cursor, → expande, Enter navega el panel
/// Files a la carpeta del cursor.
#[test]
fn arbol_teclado_cursor_expande_y_navega() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let sub = work.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Crear un panel Árbol cuya raíz sea `work` (insertamos un DirTree controlado).
    let tree_id = c.ws.add_pane(PanePurpose::Tree, std::path::PathBuf::new());
    let mut t = DirTree::from_drives(&[(
        work.path().to_path_buf(),
        "work".into(),
        naygo_core::icon_kind::DriveKind::Fixed,
    )]);
    // Cargar el hijo `sub` bajo la raíz.
    t.begin_loading(work.path());
    t.push_child(work.path(), sub.clone());
    t.finish_loading(work.path(), naygo_core::tree::NodeOutcome::Done);
    t.collapse(work.path()); // arrancar colapsado
    c.trees.insert(tree_id, t);

    // Cursor inicial = primera raíz (work). → expande la raíz.
    c.tree_key(tree_id, "right");
    assert!(
        c.trees
            .get(&tree_id)
            .unwrap()
            .node_at(work.path())
            .unwrap()
            .expanded,
        "→ expande la raíz"
    );
    // ↓ baja el cursor a `sub` (ya visible bajo la raíz expandida).
    c.tree_key(tree_id, "down");
    assert_eq!(c.tree_cursor_of(tree_id).as_deref(), Some(sub.as_path()));
    // Enter navega el panel Files activo a `sub`.
    assert!(c.tree_key(tree_id, "enter"));
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
}

/// La ayuda (F1) lista atajos activos (con chord no vacío) e incluye el propio F1.
#[test]
fn ayuda_lista_atajos_activos() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    let rows = c.help_shortcuts();
    assert!(!rows.is_empty(), "hay atajos");
    assert!(
        rows.iter().all(|(_, chord)| !chord.is_empty()),
        "solo acciones con atajo asignado"
    );
    assert!(
        rows.iter().any(|(_, chord)| chord == "F1"),
        "F1 (ayuda) está en la lista"
    );
}

/// Atrás/adelante de teclado: navegar a una subcarpeta, volver con go_back, re-avanzar con
/// go_forward. Replica el historial estilo navegador.
#[test]
fn teclado_atras_y_adelante() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let sub = work.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.navigate_active_to(sub.clone());
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
    // Atrás → vuelve a work.
    assert!(c.on_go_back());
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().current_dir, work.path());
    // Adelante → re-entra a sub.
    assert!(c.on_go_forward());
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
    // Sin más adelante: no-op.
    assert!(!c.on_go_forward());
}

/// El menú ▾ de historial salta a la entrada elegida traduciendo el índice del menú (cercano→
/// lejano) al índice de la pila. Construye A → B → C, retrocede al medio y verifica que las
/// listas de atrás/adelante y los saltos por índice de menú caigan en la carpeta correcta.
#[test]
fn menu_de_historial_salta_por_indice() {
    let cfg = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let cc = root.path().join("c");
    for d in [&a, &b, &cc] {
        std::fs::create_dir(d).unwrap();
    }
    // Arranca en root, luego navega a → b → c (cursor en c, índice 3 de la pila).
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    for d in [&a, &b, &cc] {
        c.navigate_active_to(d.clone());
        assert!(drain(&mut c));
    }
    assert_eq!(c.ws.active_files().unwrap().current_dir, cc);
    // Atrás: cercano→lejano = [b, a, root]. Adelante: vacío.
    assert_eq!(
        c.back_history_entries(),
        vec![b.clone(), a.clone(), root.path().to_path_buf()]
    );
    assert!(c.forward_history_entries().is_empty());
    // Saltar al ítem 1 del menú de atrás (a). Tras esto hay atrás (root) y adelante (b, c).
    assert!(c.go_back_history(1));
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().current_dir, a);
    assert_eq!(c.back_history_entries(), vec![root.path().to_path_buf()]);
    assert_eq!(c.forward_history_entries(), vec![b.clone(), cc.clone()]);
    // Saltar al ítem 1 del menú de adelante (c, el más lejano).
    assert!(c.go_forward_history(1));
    assert!(drain(&mut c));
    assert_eq!(c.ws.active_files().unwrap().current_dir, cc);
    // Índice fuera de rango en cualquiera de los dos: no-op.
    assert!(!c.go_forward_history(0));
    assert!(!c.go_back_history(9));
}

/// "Mover al otro panel" con dos paneles: copia la selección al directorio del otro panel
/// (una op deshacible). Verifica que el archivo aparezca en el destino.
#[test]
fn mover_al_otro_panel_con_dos_paneles() {
    let cfg = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Segundo panel apuntando a `b`.
    let origin = c.ws.active_id().unwrap();
    let dest = c.split_for_target().unwrap();
    c.open_in_pane(dest, b.path().to_path_buf());
    assert!(drain(&mut c));
    // Dar un área para que resolve_target tenga rects de ambos paneles.
    c.set_area(Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    });
    c.ws.set_active(origin);
    // Seleccionar el archivo y copiarlo al otro panel (move=false).
    c.ws.active_files_mut().unwrap().select_all();
    c.op_to_other(false);
    c.destination_radar_resolve(0);
    for _ in 0..2000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(b.path().join("doc.txt").exists(), "se copió al otro panel");
    assert!(
        a.path().join("doc.txt").exists(),
        "el original sigue (copia)"
    );
}

/// Regresión del bug de drop entre paneles: `drop_at` debe enrutar al panel que está BAJO
/// el punto, NO al panel activo (origen). Construye 2 paneles con área conocida, calcula un
/// punto que cae claramente dentro del rect del panel destino (el que NO es el origen) y
/// verifica que la op de copia aterriza en la carpeta de ese panel.
///
/// Cubre el ruteo posterior a la conversión de coordenadas (que sí ocurre con coords ya en
/// el sistema de contenido). La conversión ScreenToClient en sí necesita un HWND real y se
/// verifica a mano en la VM; aquí se blinda que coords realistas dentro del destino → ese
/// panel, y no el fallback.
#[test]
fn drop_at_enruta_al_panel_bajo_el_cursor_no_al_activo() {
    let cfg = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("doc.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Segundo panel apuntando a `b` (el destino del drop).
    let origin = c.ws.active_id().unwrap();
    let dest = c.split_for_target().unwrap();
    c.open_in_pane(dest, b.path().to_path_buf());
    assert!(drain(&mut c));
    // Área conocida; el panel activo sigue siendo el origen (`a`).
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    c.ws.set_active(origin);
    // Localizar el rect del panel destino (`dest`) y apuntar a su CENTRO. Así el punto cae
    // dentro de ese panel (no del origen) y la zona es Center → drop sobre el panel.
    let panes = c.pane_rects(area);
    let (_, dest_rect) = panes
        .iter()
        .find(|(id, _)| *id == dest)
        .copied()
        .expect("el panel destino tiene rect");
    let cx = dest_rect.x + dest_rect.w / 2.0;
    let cy = dest_rect.y + dest_rect.h / 2.0;
    // Sanity: ese punto NO cae dentro del rect del panel origen.
    let (_, origin_rect) = panes
        .iter()
        .find(|(id, _)| *id == origin)
        .copied()
        .expect("el panel origen tiene rect");
    let dentro_origen = cx >= origin_rect.x
        && cx < origin_rect.x + origin_rect.w
        && cy >= origin_rect.y
        && cy < origin_rect.y + origin_rect.h;
    assert!(!dentro_origen, "el punto debe caer fuera del panel origen");
    // Soltar el archivo de `a` sobre el panel destino (`b`). Forzamos COPIA con Ctrl para que
    // el aserto sea determinista sea cual sea el disco: sin modificadores, soltar dentro del
    // mismo disco MUEVE (regla del Explorador), y `a`/`b` viven en el mismo disco temporal.
    // `move_hint=false` (sin Shift del OLE), si no el move_hint forzaría Mover y rompería el
    // aserto de copia. Lo que prueba este test es el RUTEO (que el archivo aterriza en `b`,
    // bajo el cursor), no la elección mover/copiar (eso ya lo cubre dnd::decide_drop).
    let routed = c.drop_at(
        cx,
        cy,
        false, // move_hint del OLE (sin Shift al soltar)
        true,  // copy_forced (Ctrl del OLE) → copiar
        vec![a.path().join("doc.txt")],
    );
    assert!(routed, "drop_at debe enrutar (no caer al fallback)");
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
    // CONFIRMAR AL SOLTAR: `drop_at` ya no ejecuta; deja el drop pendiente. La op real arranca
    // al confirmar (lo que hace el botón Copiar/Mover del modal). Sanity: nada se copió todavía.
    assert!(
        c.pending_drop.is_some(),
        "el drop queda pendiente de confirmar"
    );
    assert!(
        !b.path().join("doc.txt").exists(),
        "antes de confirmar no se copió nada"
    );
    assert!(c.confirm_pending_drop(), "confirmar arranca la op");
    assert!(c.pending_drop.is_none(), "el pendiente se consumió");
    // Drenar la op y verificar que la copia aterrizó en el panel destino (`b`), no en `a`.
    for _ in 0..2000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        b.path().join("doc.txt").exists(),
        "se copió al panel destino (b), bajo el cursor"
    );
    assert!(
        a.path().join("doc.txt").exists(),
        "el original sigue (copia)"
    );
}

#[test]
fn drop_at_sobre_bandeja_agrega_referencias_sin_crear_movimiento() {
    let cfg = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let file = source.path().join("pieza.stl");
    std::fs::write(&file, b"solid x\nendsolid x\n").unwrap();
    let mut c = WorkspaceCtrl::new_in(source.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    c.add_pane_of(PanePurpose::Basket, area);
    let basket =
        c.ws.panes()
            .iter()
            .find(|pane| pane.purpose == PanePurpose::Basket)
            .map(|pane| pane.id)
            .unwrap();
    let (_, rect) = c
        .pane_rects(area)
        .into_iter()
        .find(|(id, _)| *id == basket)
        .unwrap();

    assert_eq!(c.pane_at(rect.x + 5.0, rect.y + 5.0), Some(basket));
    assert!(c.drop_at(
        rect.x + rect.w / 2.0,
        rect.y + rect.h / 2.0,
        false,
        false,
        vec![file.clone()],
    ));
    assert_eq!(c.basket.items(), &[file]);
    assert!(
        c.ops.active_ops.is_empty(),
        "la bandeja no inicia operaciones"
    );
    assert!(c.pending_drop.is_none(), "la bandeja no abre confirmación");
}

/// Regresión: arrastrar desde un panel que NO está activo debe tomar los archivos de ESE
/// panel (el del gesto), no del activo. Antes `selected_paths()` leía siempre el panel activo,
/// así que arrastrar desde el inactivo no movía nada (obligaba a un clic extra para activarlo).
#[test]
fn selected_paths_of_toma_el_panel_pedido_no_el_activo() {
    let cfg = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("en_a.txt"), b"x").unwrap();
    std::fs::write(b.path().join("en_b.txt"), b"y").unwrap();
    let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane_a = c.ws.active_id().unwrap();
    let pane_b = c.split_for_target().unwrap();
    c.open_in_pane(pane_b, b.path().to_path_buf());
    assert!(drain(&mut c));
    // Seleccionar el archivo en cada panel (la fila 0 de su vista).
    c.ws.pane_mut(pane_a)
        .and_then(|p| p.files.as_mut())
        .unwrap()
        .selected = vec![0];
    c.ws.pane_mut(pane_b)
        .and_then(|p| p.files.as_mut())
        .unwrap()
        .selected = vec![0];
    // Activo = A. Arrastrar desde B (el inactivo) debe devolver el archivo de B, no el de A.
    c.ws.set_active(pane_a);
    let desde_b = c.selected_paths_of(pane_b);
    assert_eq!(desde_b.len(), 1);
    assert!(
        desde_b[0].ends_with("en_b.txt"),
        "arrastrar desde el panel inactivo B toma su archivo, no el del activo A: {:?}",
        desde_b
    );
    // Y desde A devuelve el de A (sanity).
    let desde_a = c.selected_paths_of(pane_a);
    assert!(desde_a[0].ends_with("en_a.txt"));
}

/// Regresión del Shift/Ctrl "pegado": tras un modal o el bucle de un arrastre, el key-release de
/// un modificador puede NO llegar y `shift_down`/`ctrl_down` quedan en true. El bug que veía el
/// usuario: al hacer clic en una fila "se marcaban varios" (selección por rango fantasma). El fix
/// de raíz: `on_row_clicked` usa los modificadores del PROPIO evento de clic (parámetros
/// `ctrl`/`shift`), NO el estado sticky del controlador. Así, aunque `shift_down` esté pegado, un
/// clic real SIN Shift (shift=false) selecciona UNA sola fila.
#[test]
fn on_row_clicked_usa_modificadores_del_evento_no_el_estado_sticky() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        std::fs::write(dir.path().join(n), b"x").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    let now = std::time::Instant::now();
    // Clic simple en la fila 0 → una sola seleccionada.
    c.on_row_clicked(id, 0, false, false, now);
    assert_eq!(c.ws.active_files().unwrap().selected.len(), 1);
    // Simular Shift PEGADO en el estado sticky (como si un modal/drag se hubiera tragado el release).
    c.on_key("", false, true, false); // ctrl=false, shift=true → shift_down = true
                                      // El clic REAL llega SIN Shift (shift=false en el parámetro). Antes del fix, on_row_clicked
                                      // leía el sticky y seleccionaba por rango (4 filas). Ahora usa el parámetro → UNA sola fila.
    c.on_row_clicked(id, 3, false, false, now);
    assert_eq!(
        c.ws.active_files().unwrap().selected.len(),
        1,
        "el clic usa el modificador del evento (shift=false), no el sticky pegado → 1 fila, no rango"
    );
    // Y un clic CON Shift (parámetro shift=true) sí extiende el rango, como debe.
    c.on_row_clicked(id, 0, false, true, now);
    assert_eq!(
        c.ws.active_files().unwrap().selected.len(),
        4,
        "clic con Shift real extiende: de la fila 3 a la 0 → 4 filas"
    );
}

#[test]
fn doble_clic_normal_no_hereda_ctrl_atrapado_por_un_modal() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("carpeta");
    std::fs::create_dir(&folder).unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    let pos = active_pos_of(&c, "carpeta").unwrap();

    // Simula Ctrl pegado porque el modal tomó el foco antes del key-release. Los eventos de
    // ambos clics llegan explícitamente sin Ctrl, así que deben navegar en este panel.
    c.ctrl_down = true;
    let now = std::time::Instant::now();
    assert!(!c.on_row_clicked(id, pos, false, false, now));
    assert!(c.on_row_clicked(id, pos, false, false, now + Duration::from_millis(120)));
    assert_eq!(c.ws.active_files().unwrap().current_dir, folder);
}

#[test]
fn doble_clic_nativo_usa_modificadores_del_mouse_y_no_estado_sticky() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("carpeta");
    std::fs::create_dir(&folder).unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    let pos = active_pos_of(&c, "carpeta").unwrap();

    // Simula que el key-release se perdió mientras un modal tenía el foco. El gesto de mouse
    // entrega Ctrl=false; el callback nativo debe navegar aquí en vez de abrir otro panel.
    c.ctrl_down = true;
    c.shift_down = true;
    assert!(c.on_row_double_clicked_native(id, pos, false, false));
    assert_eq!(c.ws.active_files().unwrap().current_dir, folder);
    assert!(!c.ctrl_down && !c.shift_down);
}

/// El flujo de comprimir desde el menú: `op_compress_prompt` abre el modal de nombre con el
/// nombre por defecto, y al confirmarlo (`ops.name_confirm`) arranca la op `Compress` que, al
/// drenarla, crea el `.zip` (con la extensión asegurada) en la carpeta activa.
#[test]
fn op_compress_prompt_y_confirm_crean_el_zip() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("doc.txt"), b"contenido").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    // Seleccionar la fila 0 (doc.txt) del panel activo.
    c.ws.pane_mut(id)
        .and_then(|p| p.files.as_mut())
        .unwrap()
        .selected = vec![0];
    // Abrir el modal: el nombre por defecto debe ser "doc.zip" (un único archivo).
    c.op_compress_prompt();
    assert!(
        matches!(
            c.ops.pending_dialog,
            Some(crate::ops_ctrl::OpDialog::NameInput {
                purpose: crate::ops_ctrl::NamePurpose::Compress { .. },
                ..
            })
        ),
        "se abrió el modal de nombre de comprimir"
    );
    // Escribir un nombre SIN extensión y confirmar: name_confirm debe agregar ".zip".
    c.ops.name_changed("paquete".to_string());
    let compress_label = c.config.t("ops.kind_compress");
    assert!(
        c.ops.name_confirm(&compress_label),
        "confirmar arranca la op de comprimir"
    );
    // Drenar la op de zip hasta que termine.
    for _ in 0..2000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        work.path().join("paquete.zip").is_file(),
        "se creó el .zip con la extensión asegurada"
    );
}

/// `sel_is_single_zip` es true solo cuando la selección es exactamente un archivo `.zip`
/// (sin importar mayúsculas); una carpeta, otro tipo, o varios ítems → false.
#[test]
fn sel_is_single_zip_solo_con_un_zip() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.ZIP"), b"x").unwrap();
    std::fs::write(work.path().join("b.txt"), b"y").unwrap();
    std::fs::create_dir(work.path().join("carpeta")).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    let pos = |c: &WorkspaceCtrl, name: &str| active_pos_of(c, name).unwrap();
    // Solo a.ZIP seleccionado → true.
    let p = pos(&c, "a.ZIP");
    c.ws.pane_mut(id)
        .and_then(|p| p.files.as_mut())
        .unwrap()
        .selected = vec![p];
    assert!(c.sel_is_single_zip(), "un único .ZIP cuenta como zip");
    // b.txt → false.
    let p = pos(&c, "b.txt");
    c.ws.pane_mut(id)
        .and_then(|p| p.files.as_mut())
        .unwrap()
        .selected = vec![p];
    assert!(!c.sel_is_single_zip(), "un .txt no es zip");
    // carpeta → false.
    let p = pos(&c, "carpeta");
    c.ws.pane_mut(id)
        .and_then(|p| p.files.as_mut())
        .unwrap()
        .selected = vec![p];
    assert!(!c.sel_is_single_zip(), "una carpeta no es zip");
    // a.ZIP + b.txt (dos ítems) → false.
    let pz = pos(&c, "a.ZIP");
    let pt = pos(&c, "b.txt");
    c.ws.pane_mut(id)
        .and_then(|p| p.files.as_mut())
        .unwrap()
        .selected = vec![pz, pt];
    assert!(!c.sel_is_single_zip(), "varios ítems no es un único zip");
}

/// `pane_at` resuelve el panel Files bajo un punto de contenido (mismo hit-test que `drop_at`,
/// para resaltar EN VIVO el panel mientras se arrastra encima). El centro de cada panel cae en
/// ESE panel; un punto fuera de toda área devuelve `None`. Además `set_drag_over` solo reporta
/// `true` cuando el valor CAMBIA (la UI re-pinta solo en el cambio, no en cada `DragOver`).
#[test]
fn pane_at_resuelve_el_panel_files_bajo_el_cursor() {
    let cfg = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(a.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let origin = c.ws.active_id().unwrap();
    let dest = c.split_for_target().unwrap();
    c.open_in_pane(dest, b.path().to_path_buf());
    assert!(drain(&mut c));
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    let panes = c.pane_rects(area);
    let center = |id| {
        let (_, r) = panes.iter().find(|(p, _)| *p == id).copied().unwrap();
        (r.x + r.w / 2.0, r.y + r.h / 2.0)
    };
    // El centro de cada panel resuelve a ESE panel.
    let (ox, oy) = center(origin);
    let (dx, dy) = center(dest);
    let captured = c.capture_external_drop_destination(dx, dy, dy).unwrap();
    assert_eq!(captured.pane_id, dest.0);
    assert_eq!(captured.path, b.path());
    assert!(c
        .capture_external_drop_destination(-50.0, -50.0, -50.0)
        .is_none());
    assert_eq!(
        c.pane_at(ox, oy),
        Some(origin),
        "centro del origen → origen"
    );
    assert_eq!(
        c.pane_at(dx, dy),
        Some(dest),
        "centro del destino → destino"
    );
    // Fuera de toda área → None (no se resalta nada).
    assert_eq!(c.pane_at(-50.0, -50.0), None, "fuera de todo panel → None");
    // set_drag_over solo cambia (true) cuando el valor es distinto.
    assert!(c.set_drag_over(Some(dest)), "primera vez: cambia");
    assert!(!c.set_drag_over(Some(dest)), "mismo valor: no cambia");
    assert!(c.set_drag_over(None), "limpiar: cambia");
    assert_eq!(c.drag_over_pane(), None);
    assert!(c.refresh_pane(dest));
    assert!(c.external_destination_still_open(&captured));
    assert!(c.navigate_pane_to(dest, a.path().to_path_buf()));
    assert!(c.navigate_pane_to(dest, b.path().to_path_buf()));
    assert!(!c.external_destination_still_open(&captured));
}

#[test]
fn captura_externa_rechaza_bandeja() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    c.add_pane_of(PanePurpose::Basket, area);
    let basket =
        c.ws.panes()
            .iter()
            .find(|pane| pane.purpose == PanePurpose::Basket)
            .unwrap()
            .id;
    let (_, rect) = c
        .pane_rects(area)
        .into_iter()
        .find(|(id, _)| *id == basket)
        .unwrap();
    let (x, y) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    assert_eq!(c.pane_at(x, y), Some(basket));
    assert!(c.capture_external_drop_destination(x, y, y).is_none());
    assert_eq!(c.drop_hover_pane(x, y, false), None);
    assert_eq!(c.drop_hover_pane(x, y, true), Some(basket));
}

/// D-1: al soltar dentro del MISMO panel sobre una fila-carpeta, el hit-test de FilePanel deja
/// esa subcarpeta como destino. Debe seguir usando el flujo normal: Ctrl fuerza copiar, el drop
/// queda pendiente de confirmación y solo al confirmar toca el filesystem.
#[test]
fn drop_at_mismo_panel_sobre_subcarpeta_usa_la_carpeta_como_destino() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("documento.txt"), b"contenido").unwrap();
    std::fs::create_dir(work.path().join("archivo")).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.ws.active_id().unwrap();
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    let (_, rect) = c
        .pane_rects(area)
        .into_iter()
        .find(|(id, _)| *id == pane)
        .expect("rect del panel activo");
    // Simula el reporte del callback `drop-target-row`: el índice es de VISTA, por lo que no
    // depende de cómo el listado ordene internamente las entries.
    let folder_row = active_pos_of(&c, "archivo").expect("subcarpeta visible");
    c.set_drag_over_at(Some(pane), Some(140.0));
    c.set_drag_over_row(pane, folder_row as i32);
    let routed = c.drop_at(
        rect.x + rect.w / 2.0,
        rect.y + rect.h / 2.0,
        false,
        true,
        vec![work.path().join("documento.txt")],
    );
    assert!(routed, "el drop intra-panel sobre carpeta se enruta");
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
    let pending = c.pending_drop.as_ref().expect("espera confirmación");
    assert_eq!(pending.dest_dir, work.path().join("archivo"));
    assert!(!pending.is_move, "Ctrl conserva la regla de copiar");
    assert!(c.confirm_pending_drop());
    for _ in 0..2000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(work.path().join("archivo/documento.txt").exists());
    assert!(work.path().join("documento.txt").exists());
}

fn painted_drop_rows(c: &mut WorkspaceCtrl, pane: PaneId) -> slint::ModelRc<crate::RowData> {
    let rows: Vec<_> = c
        .rows_of(pane, 0, std::time::Instant::now())
        .into_iter()
        .map(crate::vm_builders::to_row_data)
        .collect();
    slint::ModelRc::new(slint::VecModel::from(rows))
}

#[test]
fn ole_mueve_archivo_y_carpeta_en_mismo_panel_sin_hover_y_permite_deshacer() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let root = work.path();
    let target = root.join("b-destino");
    let dir = root.join("c-origen");
    let file = root.join("documento.txt");
    std::fs::create_dir(root.join("a-vecino")).unwrap();
    std::fs::create_dir(&target).unwrap();
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("hijo.txt"), b"hijo").unwrap();
    std::fs::write(&file, b"documento").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.set_area(area());
    let top = 100.0;
    let row = active_pos_of(&c, "b-destino").unwrap();
    let client_y = top + row as f32 * 22.0 - 22.0 + 11.0;
    c.set_drop_viewport(
        pane,
        crate::DropViewport {
            left: 0.0,
            top,
            width: 800.0,
            height: 400.0,
            scroll_y: -22.0,
            row_height: 22.0,
        },
    );
    c.set_drag_over(None);
    let rows = painted_drop_rows(&mut c, pane);
    assert!(c.drop_at_from_ole(
        100.0,
        client_y - crate::tick::TOP_BAR_H,
        client_y,
        Some(&rows),
        false,
        false,
        true,
        vec![file.clone(), dir.clone()],
        None,
    ));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        DropProbeEvent::Confirmation
    );
    let pending = c.pending_drop.as_ref().unwrap();
    assert!(pending.is_move);
    assert_eq!(pending.dest_dir, target);
    assert!(c.confirm_pending_drop());
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!file.exists() && !dir.exists());
    assert_eq!(
        std::fs::read(target.join("documento.txt")).unwrap(),
        b"documento"
    );
    assert_eq!(
        std::fs::read(target.join("c-origen/hijo.txt")).unwrap(),
        b"hijo"
    );
    let undo = c.ops.undo_history.last().unwrap();
    assert_eq!(
        undo.actions.len(),
        2,
        "la carpeta se registró como una unidad"
    );
    assert!(c.op_undo_last());
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(std::fs::read(file).unwrap(), b"documento");
    assert_eq!(std::fs::read(dir.join("hijo.txt")).unwrap(), b"hijo");
    assert_eq!(std::fs::read_dir(target).unwrap().count(), 0);
}

/// D-1 no debe permitir una operación imposible: una carpeta nunca puede soltarse sobre sí
/// misma (la misma protección de rutas también cubre descendientes).
#[test]
fn drop_at_mismo_panel_rechaza_carpeta_sobre_si_misma() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::create_dir(work.path().join("origen")).unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.ws.active_id().unwrap();
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    let (_, rect) = c
        .pane_rects(area)
        .into_iter()
        .find(|(id, _)| *id == pane)
        .expect("rect del panel activo");
    let origin = work.path().join("origen");
    // Validar el caso exacto (origen sobre sí mismo) que sí tiene fila en la vista.
    let origin_row = active_pos_of(&c, "origen").unwrap();
    c.set_drag_over_at(Some(pane), Some(140.0));
    c.set_drag_over_row(pane, origin_row as i32);
    assert!(
        !c.drop_at(
            rect.x + rect.w / 2.0,
            rect.y + rect.h / 2.0,
            false,
            false,
            vec![origin]
        ),
        "soltar carpeta sobre sí misma es no-op"
    );
    assert!(c.pending_drop.is_none());
}

/// El `move_hint` del OLE (Shift presionado al SOLTAR) fuerza MOVER aunque los flags de
/// teclado de la app lleguen en false (lo que pasa durante el bucle modal de DoDragDrop, que
/// se traga los eventos de teclado). Regresión del bug "arrastré con Shift y copió en vez de
/// mover: creó en destino pero no borró el origen".
#[test]
fn drop_at_move_hint_del_ole_fuerza_mover() {
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
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    c.ws.set_active(origin);
    let panes = c.pane_rects(area);
    let (_, dest_rect) = panes
        .iter()
        .find(|(id, _)| *id == dest)
        .copied()
        .expect("el panel destino tiene rect");
    let cx = dest_rect.x + dest_rect.w / 2.0;
    let cy = dest_rect.y + dest_rect.h / 2.0;
    // copy_forced=false, PERO move_hint=true (Shift REAL al soltar, reportado por el OLE). Debe
    // MOVER → el original desaparece. (El estado de teclado de la app ya no se pasa: llega stale.)
    let routed = c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")]);
    assert!(routed, "drop_at debe enrutar");
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
    // El drop queda pendiente: debe ser MOVER (el move_hint del OLE manda).
    assert_eq!(
        c.pending_drop.as_ref().map(|p| p.is_move),
        Some(true),
        "el drop pendiente es Mover por el move_hint"
    );
    assert!(c.confirm_pending_drop(), "confirmar arranca la op");
    for _ in 0..2000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        b.path().join("doc.txt").exists(),
        "el archivo aterrizó en el destino"
    );
    assert!(
        !a.path().join("doc.txt").exists(),
        "el original se MOVIÓ (ya no está en el origen) por el move_hint del OLE"
    );
}

/// Regla del Explorador: soltar SIN modificadores ni move_hint, dentro del MISMO disco, MUEVE
/// por defecto (no copia). `a` y `b` son tempdirs del mismo volumen del sistema, así que
/// `same_drive` es true y `decide_drop_action(false, false, true)` = Move. El archivo aterriza
/// en el destino y desaparece del origen. (El test de ruteo fuerza Ctrl=copia justo para evitar
/// esta ambigüedad; este test fija el comportamiento por defecto del mismo disco.)
#[test]
fn drop_at_mismo_disco_sin_modificadores_mueve_por_defecto() {
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
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    c.ws.set_active(origin);
    let panes = c.pane_rects(area);
    let (_, dest_rect) = panes
        .iter()
        .find(|(id, _)| *id == dest)
        .copied()
        .expect("el panel destino tiene rect");
    let cx = dest_rect.x + dest_rect.w / 2.0;
    let cy = dest_rect.y + dest_rect.h / 2.0;
    // move_hint=false, copy_forced=false → la decisión depende del disco. Mismo disco → Mover.
    let routed = c.drop_at(cx, cy, false, false, vec![a.path().join("doc.txt")]);
    assert!(routed, "drop_at debe enrutar");
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
    assert!(c.confirm_pending_drop(), "confirmar arranca la op");
    for _ in 0..2000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(b.path().join("doc.txt").exists(), "aterrizó en el destino");
    assert!(
        !a.path().join("doc.txt").exists(),
        "mismo disco sin modificadores: se MOVIÓ (el original ya no está)"
    );
}

/// CONFIRMAR AL SOLTAR (PUNTO 1b): un drop entre paneles NO ejecuta la operación hasta que el
/// usuario confirme. `drop_at` solo deja el drop pendiente; CANCELAR lo descarta sin tocar el
/// disco. Regresión del bug "arrastré sin querer una carpeta a otro panel y empezó a copiarla".
#[test]
fn drop_at_no_ejecuta_hasta_confirmar_y_cancelar_descarta() {
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
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    c.set_area(area);
    c.ws.set_active(origin);
    let panes = c.pane_rects(area);
    let (_, dest_rect) = panes
        .iter()
        .find(|(id, _)| *id == dest)
        .copied()
        .expect("el panel destino tiene rect");
    let cx = dest_rect.x + dest_rect.w / 2.0;
    let cy = dest_rect.y + dest_rect.h / 2.0;
    // Soltar (copy_forced=Ctrl=copia para que el dato sea determinista).
    let routed = c.drop_at(cx, cy, false, true, vec![a.path().join("doc.txt")]);
    assert!(routed, "drop_at debe enrutar");
    // NADA se ejecutó todavía: el drop está pendiente y no hay op alguna.
    let pd = c.pending_drop.as_ref().expect("drop pendiente");
    assert_eq!(pd.count, 1);
    assert!(!pd.is_move, "Ctrl → copiar");
    assert_eq!(pd.dest_dir, b.path());
    assert!(
        c.ops.active_ops.is_empty(),
        "antes de confirmar no arrancó ninguna op"
    );
    // CANCELAR descarta sin copiar.
    c.cancel_pending_drop();
    assert!(c.pending_drop.is_none(), "el pendiente se descartó");
    for _ in 0..200 {
        c.ops.pump_ops();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        !b.path().join("doc.txt").exists(),
        "cancelar no copió nada al destino"
    );
    assert!(a.path().join("doc.txt").exists(), "el origen quedó intacto");
}

/// El batch-rename: abrir con la selección, editar el spec (plantilla + contador) y aplicar.
/// La op renombra los archivos en disco (verificado tras drenar las ops).
#[test]
fn vista_profunda_batch_renombra_lote_y_reconcilia_resultados() {
    use naygo_core::keymap::Action;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let folder = root.path().join("sub");
    let other = root.path().join("otro");
    std::fs::create_dir(&folder).unwrap();
    std::fs::create_dir(&other).unwrap();
    let a = folder.join("a.txt");
    let b = folder.join("b.txt");
    let occupied = folder.join("foto1.txt");
    let outside = other.join("c.txt");
    for path in [&a, &b, &occupied, &outside] {
        std::fs::write(path, b"original").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let pa = c.deep_position_of(pane, &a).unwrap();
    let pb = c.deep_position_of(pane, &b).unwrap();
    let pc = c.deep_position_of(pane, &outside).unwrap();
    c.deep_select_at(pane, pa, false, false);
    c.deep_select_at(pane, pc, true, false);
    c.run_action(Action::BatchRename);
    assert!(c.batch.is_some(), "el lote puede incluir padres distintos");
    c.batch_close();
    c.deep_select_at(pane, pa, false, false);
    c.deep_select_at(pane, pb, true, false);
    c.deep_poll();
    assert!(
        !c.deep_job.as_ref().unwrap().partial,
        "un Done real sigue completo"
    );
    let (tx, rx) = std::sync::mpsc::channel();
    drop(tx);
    let job = c.deep_job.as_mut().unwrap();
    job.rx = rx;
    job.done = false;
    c.deep_poll();
    assert!(
        c.deep_job.as_ref().unwrap().partial,
        "desconexión sin Done es parcial"
    );
    c.run_action(Action::BatchRename);
    assert!(c.batch.is_none() && c.take_deep_selection_notice());
    c.deep_job.as_mut().unwrap().partial = false;
    c.run_action(Action::BatchRename);
    assert!(c.batch.is_some());
    c.batch_set_template("foto{n}");
    assert!(
        !c.batch_can_apply(),
        "detecta un nombre ocupado en el padre real"
    );
    c.batch_set_template("nuevo{n}");
    assert!(c.batch_can_apply());
    let preview = c.batch_preview();
    let focused_destination = preview
        .iter()
        .find(|row| row.path == b)
        .unwrap()
        .new_name
        .clone();
    let focused_destination = folder.join(focused_destination);
    c.batch_apply();
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    c.reconcile_completed_operations();
    assert_eq!(c.deep_position_of(pane, &focused_destination), Some(pb));
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let job = c.deep_job.as_ref().unwrap();
    assert_eq!(job.focused_path.as_ref(), Some(&focused_destination));
    assert_eq!(c.deep_position_of(pane, &focused_destination), Some(pb));
    assert_eq!(job.selected_paths.len(), 2);
    for row in preview {
        assert_eq!(
            std::fs::read(folder.join(row.new_name)).unwrap(),
            b"original"
        );
    }
    assert!(!a.exists() && !b.exists());
    assert!(occupied.exists() && outside.exists());
}

#[test]
fn vista_profunda_batch_renombra_en_carpetas_distintas_y_aisla_colisiones() {
    use naygo_core::batch_rename::RowStatus;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let a_dir = root.path().join("a");
    let b_dir = root.path().join("b");
    std::fs::create_dir(&a_dir).unwrap();
    std::fs::create_dir(&b_dir).unwrap();
    let a = a_dir.join("a.txt");
    let b = b_dir.join("b.txt");
    let occupied = a_dir.join("igual.txt");
    for path in [&a, &b, &occupied] {
        std::fs::write(path, b"original").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let pa = c.deep_position_of(pane, &a).unwrap();
    let pb = c.deep_position_of(pane, &b).unwrap();
    c.deep_select_at(pane, pa, false, false);
    c.deep_select_at(pane, pb, true, false);
    c.batch_open();
    c.batch_set_template("igual");
    let rows = c.batch_preview();
    assert_eq!(
        rows.iter().find(|row| row.path == a).unwrap().status,
        RowStatus::Collision
    );
    assert_eq!(
        rows.iter().find(|row| row.path == b).unwrap().status,
        RowStatus::Ok
    );
    assert!(!c.batch_can_apply());
    c.batch_set_template("nuevo{n}");
    assert!(c.batch_can_apply());
    let rows = c.batch_preview();
    let focus = b_dir.join(&rows.iter().find(|row| row.path == b).unwrap().new_name);
    c.batch_apply();
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    c.reconcile_completed_operations();
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    for row in rows {
        let destination = row.path.parent().unwrap().join(row.new_name);
        assert_eq!(std::fs::read(&destination).unwrap(), b"original");
        assert!(c
            .deep_job
            .as_ref()
            .unwrap()
            .selected_paths
            .contains(&destination));
    }
    assert_eq!(
        c.deep_job.as_ref().unwrap().focused_path.as_ref(),
        Some(&focus)
    );
    assert!(!a.exists() && !b.exists());
    assert_eq!(std::fs::read(occupied).unwrap(), b"original");
}

#[test]
fn batch_rename_abre_edita_y_aplica() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"x").unwrap();
    std::fs::write(work.path().join("b.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // Seleccionar todo y abrir el batch-rename.
    c.ws.active_files_mut().unwrap().select_all();
    c.batch_open();
    assert!(c.batch.is_some(), "se abrió la ventana");

    // Plantilla "foto{n}" con contador desde 1 → foto1.txt, foto2.txt.
    c.batch_set_template("foto{n}");
    let rows = c.batch_preview();
    assert_eq!(rows.len(), 2);
    let nuevos: Vec<&str> = rows.iter().map(|r| r.new_name.as_str()).collect();
    assert!(nuevos.contains(&"foto1.txt") && nuevos.contains(&"foto2.txt"));
    assert!(c.batch_can_apply());

    // Aplicar y drenar la op; los archivos quedan renombrados en disco.
    c.batch_apply();
    assert!(c.batch.is_none(), "aplicar cierra la ventana");
    for _ in 0..2000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(work.path().join("foto1.txt").exists() || work.path().join("foto2.txt").exists());
    assert!(!work.path().join("a.txt").exists() && !work.path().join("b.txt").exists());
}

/// Una plantilla que produce el mismo nombre para todos marca colisión y no deja aplicar.
#[test]
fn batch_rename_colision_no_deja_aplicar() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"x").unwrap();
    std::fs::write(work.path().join("b.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ws.active_files_mut().unwrap().select_all();
    c.batch_open();
    c.batch_set_template("igual"); // ambos → "igual.txt" → colisión
    assert!(!c.batch_can_apply());
}

/// Aplicar una plantilla built-in reconstruye el workspace con sus paneles, y guardar la
/// disposición actual como plantilla de usuario persiste (se ve en otro controlador).
#[test]
fn plantillas_aplicar_guardar_y_borrar() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    assert_eq!(c.ws.panes().len(), 1, "arranca con un panel");

    // Aplicar "Dual-pane" (4 paneles: árbol + 2 files + inspector).
    c.apply_template("Dual-pane", 100);
    assert!(drain(&mut c));
    assert_eq!(c.ws.panes().len(), 4);
    assert_eq!(c.ws.files_panes().len(), 2);
    // Quedó registrado en recientes.
    assert_eq!(
        c.templates.recents.first().map(|r| r.name.as_str()),
        Some("Dual-pane")
    );

    // Guardar la disposición actual como plantilla de usuario.
    c.save_current_template("Mi setup").unwrap();
    assert!(c
        .layout_templates()
        .iter()
        .any(|(n, builtin)| n == "Mi setup" && !builtin));
    wait_for_workspace_persistence(&c);
    // Persistió: un controlador nuevo (mismo config_dir) la ve.
    let c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(c2.templates.user.iter().any(|t| t.name == "Mi setup"));

    // Borrarla.
    c.delete_template("Mi setup");
    assert!(!c.layout_templates().iter().any(|(n, _)| n == "Mi setup"));
    wait_for_workspace_persistence(&c);
}

/// Primera ejecución (sin sesión guardada): `apply_first_run_layout` arma la disposición
/// clásica: árbol + dos paneles de archivos + propiedades + vista previa (5 paneles).
#[test]
fn primera_ejecucion_arma_layout_clasico() {
    use naygo_core::workspace::PanePurpose;
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.panes().len(),
        1,
        "new_in sigue arrancando con un panel"
    );

    c.apply_first_run_layout();
    assert!(drain(&mut c));
    assert_eq!(c.ws.panes().len(), 5, "árbol + 2 files + props + preview");
    assert_eq!(c.ws.files_panes().len(), 2, "dos paneles de archivos");
    let has = |p: PanePurpose| c.ws.panes().iter().any(|x| x.purpose == p);
    assert!(has(PanePurpose::Tree), "hay árbol");
    assert!(has(PanePurpose::Inspector), "hay propiedades");
    assert!(has(PanePurpose::Preview), "hay vista previa");
}

/// `ensure_ops_pane` agrega un panel de Operaciones si no hay; es idempotente (no agrega un
/// segundo) y NO roba el foco al panel Files activo (el usuario estaba operando ahí).
#[test]
fn ensure_ops_pane_agrega_una_vez_y_no_roba_foco() {
    use naygo_core::workspace::PanePurpose;
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let files_id = c.ws.active_id();
    assert!(
        !c.has_purpose(PanePurpose::Operations),
        "no hay panel de ops"
    );

    c.ensure_ops_pane();
    assert!(
        c.has_purpose(PanePurpose::Operations),
        "se agregó el panel de ops"
    );
    assert_eq!(
        c.ws.active_id(),
        files_id,
        "el activo sigue siendo el panel Files (no robó foco)"
    );
    let ops_count =
        c.ws.panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Operations)
            .count();
    assert_eq!(ops_count, 1, "exactamente un panel de operaciones");

    // Segunda llamada: idempotente, no agrega otro.
    c.ensure_ops_pane();
    let ops_count2 =
        c.ws.panes()
            .iter()
            .filter(|p| p.purpose == PanePurpose::Operations)
            .count();
    assert_eq!(ops_count2, 1, "sigue habiendo exactamente uno");
}

#[test]
fn paneles_auxiliares_unicos_no_se_duplican() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    for purpose in [
        PanePurpose::Basket,
        PanePurpose::Favorites,
        PanePurpose::Preview,
        PanePurpose::Operations,
    ] {
        c.add_pane_of(purpose, area());
        c.add_pane_of(purpose, area());
        assert_eq!(
            c.ws.panes().iter().filter(|p| p.purpose == purpose).count(),
            1
        );
    }
}

#[test]
fn operaciones_recupera_ubicacion_despues_de_reiniciar() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ensure_ops_pane();
    let ops =
        c.ws.panes()
            .iter()
            .find(|p| p.purpose == PanePurpose::Operations)
            .unwrap()
            .id;
    let old_rect = c
        .pane_rects(area())
        .into_iter()
        .find(|(id, _)| *id == ops)
        .unwrap()
        .1;
    c.close_pane(ops);
    c.save_session();
    let mut restored = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(restored.load_session());
    restored.ensure_ops_pane();
    let reopened = restored
        .ws
        .panes()
        .iter()
        .find(|p| p.purpose == PanePurpose::Operations)
        .unwrap()
        .id;
    let new_rect = restored
        .pane_rects(area())
        .into_iter()
        .find(|(id, _)| *id == reopened)
        .unwrap()
        .1;
    assert_eq!(old_rect, new_rect);
}

#[test]
fn huella_de_sesion_detecta_cambio_de_tamano_sin_cambiar_paneles() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.ensure_ops_pane();
    let before = c.session_fingerprint();
    c.ws.layout.set_divider(&[], 0, 0.7);
    assert_ne!(before, c.session_fingerprint());
}

/// Cuando NO hay sesión guardada, `load_session` devuelve `false` (señal de primera
/// ejecución); cuando sí la hay, devuelve `true`.
#[test]
fn load_session_reporta_si_restauro() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    // Sin sesión previa: false.
    let mut c1 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c1));
    assert!(!c1.load_session(), "sin sesión previa → false");
    // Guardar una sesión y reabrir: true.
    c1.save_session();
    let mut c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(c2.load_session(), "con sesión guardada → true");
}

/// "Mover →" desde el menú reordena la columna en el orden visual completo.
#[test]
fn menu_de_columna_mueve_la_columna() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    // Orden por defecto: Name, Extension, Size, Modified, Created. Mover Extension (índice 1)
    // a la derecha → queda detrás de Size.
    c.column_menu_open(id, 1, 0.0, 0.0); // kind 1 = Extension
    c.column_menu_move(1);
    let order: Vec<_> =
        c.ws.pane(id)
            .unwrap()
            .files
            .as_ref()
            .unwrap()
            .table
            .columns
            .iter()
            .map(|col| col.kind)
            .collect();
    assert_eq!(order[1], naygo_core::columns::ColumnKind::Size);
    assert_eq!(order[2], naygo_core::columns::ColumnKind::Extension);
    assert!(c.column_menu.is_none(), "mover cierra el menú");
}

/// Un filtro que no coincide con nada marca `no_matches` (aviso "sin coincidencias"), sin
/// confundirlo con una carpeta vacía.
#[test]
fn filtro_sin_coincidencias_se_detecta() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    c.column_menu_open(id, 0, 0.0, 0.0);
    c.column_menu_to_filter();
    c.column_filter_set_text("zzz-no-existe");
    c.column_filter_apply();
    assert_eq!(c.ws.active_files().unwrap().view_len(), 0);
    assert!(c.no_matches(id), "filtro vació la vista → aviso");
}

/// F4: la sesión (paneles + carpetas + disposición) se guarda al cerrar y se restaura al
/// abrir. Dividir en dos paneles, navegar uno a una subcarpeta, guardar, y reconstruir en
/// un controlador nuevo (mismo config_dir) restaura los dos paneles con sus carpetas.
#[test]
fn sesion_guarda_y_restaura_dos_paneles() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let sub = work.path().join("sub");
    std::fs::create_dir(&sub).unwrap();

    // Controlador 1: arranca con un panel en `work`, lo divide (segundo panel), y navega
    // el panel activo (el nuevo) a la subcarpeta.
    let mut c1 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c1));
    c1.add_pane_split(area());
    assert_eq!(c1.ws.panes().len(), 2, "tras dividir hay dos paneles");
    assert!(drain(&mut c1));
    c1.navigate_active_to(sub.clone());
    assert!(drain(&mut c1));
    // El layout referencia los dos paneles.
    assert_eq!(
        c1.ws.layout.pane_ids().len(),
        2,
        "el layout tiene dos hojas"
    );
    c1.save_session();

    // Controlador 2: mismo config_dir; load_session reemplaza el arranque por la sesión.
    let mut c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    c2.load_session();
    assert_eq!(c2.ws.panes().len(), 2, "se restauran los dos paneles");
    assert_eq!(
        c2.ws.layout.pane_ids().len(),
        2,
        "se restaura la disposición"
    );
    let dirs: Vec<std::path::PathBuf> = c2
        .ws
        .panes()
        .iter()
        .filter_map(|p| p.files.as_ref().map(|f| f.current_dir.clone()))
        .collect();
    assert!(
        dirs.contains(&work.path().to_path_buf()),
        "un panel quedó en la carpeta raíz"
    );
    assert!(
        dirs.contains(&sub),
        "el otro panel quedó en la subcarpeta navegada"
    );
}

/// F5A: un evento del watcher (Created) agrega la entrada al panel sin re-listar y la
/// reporta como nueva (para resaltarla).
#[test]
fn watch_events_agregan_entry() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("viejo.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    let nuevo = tmp.path().join("nuevo.txt");
    std::fs::write(&nuevo, b"y").unwrap();
    let nuevas = c.apply_watch_events(id, &[naygo_core::listing::DirEvent::Created(nuevo.clone())]);
    assert_eq!(nuevas, vec![nuevo.clone()]);
    assert!(c
        .rows_of(id, 8, std::time::Instant::now())
        .iter()
        .any(|r| r.name == "nuevo.txt"));
    // MEJORA 1: el archivo recién llegado queda SELECCIONADO (resaltado como selección) y la
    // fila correspondiente lo refleja. Su posición de vista se calculó tras reordenar.
    let f = c.ws.pane(id).and_then(|p| p.files.as_ref()).unwrap();
    let nuevo_pos = f
        .view_indices()
        .iter()
        .position(|&real| f.entries[real].path == nuevo)
        .expect("nuevo.txt está en la vista");
    assert!(
        f.is_selected(nuevo_pos),
        "el archivo recién llegado queda seleccionado"
    );
    assert!(
        c.rows_of(id, 8, std::time::Instant::now())
            .iter()
            .any(|r| r.name == "nuevo.txt" && r.selected),
        "la fila del nuevo se pinta como seleccionada"
    );
}

/// M-2: un evento de watcher REZAGADO de OTRA carpeta (no la que muestra el panel ahora) se
/// descarta y NO mete una fila fantasma. Reproduce el caso al navegar A→B (mismo PaneId): el
/// watcher de A se reemplaza, pero eventos de A ya encolados pueden drenarse tras repoblar B.
#[test]
fn watch_events_descarta_los_de_otra_carpeta() {
    let tmp = tempfile::tempdir().unwrap();
    // Carpeta B (la que el panel mostrará) y carpeta A (de donde vendrá el evento rezagado).
    let dir_b = tmp.path().join("B");
    let dir_a = tmp.path().join("A");
    std::fs::create_dir(&dir_b).unwrap();
    std::fs::create_dir(&dir_a).unwrap();
    std::fs::write(dir_b.join("propio.txt"), b"x").unwrap();
    // Un archivo real en A: el evento es legítimo en A, pero el panel ya está en B.
    let ajeno = dir_a.join("ajeno.txt");
    std::fs::write(&ajeno, b"y").unwrap();
    // El panel arranca y se queda en B.
    let mut c = WorkspaceCtrl::new_in(dir_b.clone(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    // Llega un evento rezagado de A (otra carpeta): debe descartarse.
    let nuevas = c.apply_watch_events(id, &[naygo_core::listing::DirEvent::Created(ajeno.clone())]);
    assert!(
        nuevas.is_empty(),
        "el evento de otra carpeta no produce arrivals"
    );
    assert!(
        !c.rows_of(id, 8, std::time::Instant::now())
            .iter()
            .any(|r| r.name == "ajeno.txt"),
        "el archivo de la carpeta anterior NO aparece como fila fantasma"
    );
    // Sanity: la fila propia de B sigue ahí (el filtro no descartó de más).
    assert!(
        c.rows_of(id, 8, std::time::Instant::now())
            .iter()
            .any(|r| r.name == "propio.txt"),
        "la fila legítima de la carpeta actual se mantiene"
    );
}

/// 6D: el rename inline. `op_rename` marca el pedido sobre la fila enfocada; `rename_commit`
/// renombra el archivo en disco; `rename_chain` confirma y avanza a la fila siguiente.
#[test]
fn rename_inline_y_en_cadena() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    std::fs::write(tmp.path().join("b.txt"), b"y").unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    // Enfocar "a.txt" y pedir rename (F2): debe marcar el pedido en esa posición.
    let pos = active_pos_of(&c, "a.txt").expect("a.txt visible");
    c.ws.active_files_mut().unwrap().select_single(pos);
    c.op_rename();
    let req = c
        .take_rename_request()
        .expect("F2 marcó el pedido de rename");
    assert_eq!(req.pane, id);
    assert_eq!(req.pos, pos);
    assert_eq!(req.stage, 0, "etapa inicial = nombre sin extensión");
    assert_eq!(req.selection, (0, 1), "selecciona solo 'a'");
    // Confirmar el rename y bombear la op hasta completarla (mismo patrón que ops_ctrl).
    assert!(c.rename_commit(id, req.session, "renombrado.txt"));
    for _ in 0..4000 {
        let done = c.ops.pump_ops();
        if done && !c.ops.active_ops.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        tmp.path().join("renombrado.txt").exists(),
        "el archivo se renombró en disco"
    );
    assert!(!tmp.path().join("a.txt").exists());
    // Rename en cadena hacia abajo desde "b.txt": confirma (sin cambio) y devuelve la
    // posición avanzada con un nuevo pedido en etapa 0.
    // El resultado F2 conserva la fila y el cursor aunque el nombre nuevo ordene después.
    c.reconcile_completed_operations();
    assert!(drain(&mut c));
    let files = c.ws.active_files().unwrap();
    assert_eq!(files.focused, Some(pos));
    assert_eq!(files.selected, vec![pos]);
    assert_eq!(files.focused_view_entry().unwrap().name, "renombrado.txt");
    let pos_b = active_pos_of(&c, "b.txt").expect("b.txt visible");
    c.ws.active_files_mut().unwrap().select_single(pos_b);
    c.op_rename();
    let chain_req = c.take_rename_request().expect("abre rename para chain");
    let next = c.rename_chain(id, chain_req.session, "b.txt", 1);
    assert!(next.is_some(), "encadena a una fila válida");
    let req2 = c.take_rename_request().expect("chain reabre el editor");
    assert_eq!(req2.stage, 0, "el chain selecciona el nombre sin extensión");
}

#[test]
fn rename_usa_ruta_estable_y_commit_es_unico() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"a").unwrap();
    std::fs::write(tmp.path().join("b.txt"), b"b").unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    let pos_a = active_pos_of(&c, "a.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos_a);
    c.op_rename();
    let req = c.take_rename_request().unwrap();

    // El watcher/listado incorpora una fila que se ordena antes y desplaza el índice de a.txt.
    std::fs::write(tmp.path().join("0.txt"), b"0").unwrap();
    c.start_listing(id, tmp.path().to_path_buf());
    assert!(drain(&mut c));
    assert_ne!(active_pos_of(&c, "a.txt"), Some(req.pos));

    assert!(c.rename_commit(id, req.session, "estable.txt"));
    assert!(
        !c.rename_commit(id, req.session, "duplicado.txt"),
        "la pérdida de foco posterior no confirma por segunda vez"
    );
    for _ in 0..4000 {
        if c.ops.pump_ops() && !c.ops.active_ops.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(tmp.path().join("estable.txt").exists());
    assert!(
        tmp.path().join("0.txt").exists(),
        "no renombra la fila desplazada"
    );
    assert!(!tmp.path().join("duplicado.txt").exists());
}

#[test]
fn f2_repetido_cicla_seleccion_y_carpeta_selecciona_todo() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("canción.txt"), b"x").unwrap();
    std::fs::create_dir(tmp.path().join("carpeta.con.puntos")).unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));

    let pos = active_pos_of(&c, "canción.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    c.op_rename();
    let first = c.take_rename_request().unwrap();
    assert_eq!(first.selection, (0, 8));
    c.op_rename();
    let second = c.take_rename_request().unwrap();
    assert_eq!(second.stage, 1);
    assert_eq!(second.selection, (9, 12));
    c.op_rename();
    let third = c.take_rename_request().unwrap();
    assert_eq!(third.stage, 2);
    assert_eq!(third.selection, (0, 12));
    assert_eq!(c.rename_cycle_selection("niño.log"), Some((0, 5)));
    assert_eq!(c.rename_cycle_selection("niño.log"), Some((6, 9)));
    assert!(c.rename_session_is_active(third.pane, third.session));
    c.rename_cancel();

    let folder_pos = active_pos_of(&c, "carpeta.con.puntos").unwrap();
    c.ws.active_files_mut().unwrap().select_single(folder_pos);
    c.op_rename();
    let folder = c.take_rename_request().unwrap();
    assert_eq!(folder.selection, (0, folder.name.len()));
    assert_eq!(c.rename_cycle_selection("nueva.carpeta"), Some((0, 13)));
}

/// 6F: rubber-band. select_rect_range selecciona el rango inclusivo de filas; aditivo (Ctrl)
/// suma a lo ya seleccionado.
#[test]
fn rubber_band_selecciona_rango() {
    let tmp = tempfile::tempdir().unwrap();
    for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        std::fs::write(tmp.path().join(n), b"x").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    // Rectángulo de la fila 0 a la 2 (inclusive) → 3 seleccionadas.
    c.select_rect_range(id, 0, 2, false);
    let sel = c.ws.active_files().unwrap().selected.len();
    assert_eq!(sel, 3);
    // Aditivo: sumar la fila 3 conserva las anteriores → 4.
    c.select_rect_range(id, 3, 3, true);
    assert_eq!(c.ws.active_files().unwrap().selected.len(), 4);
    // No-aditivo: reemplaza con una sola.
    c.select_rect_range(id, 1, 1, false);
    assert_eq!(c.ws.active_files().unwrap().selected.len(), 1);
}

/// La path-bar: breadcrumbs de la carpeta del panel + autocompletado del editor.
#[test]
fn pathbar_segmentos_y_autocompletado() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("Alpha")).unwrap();
    std::fs::create_dir(tmp.path().join("alfajor")).unwrap();
    std::fs::create_dir(tmp.path().join("Beta")).unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    // Breadcrumbs: el último segmento es el nombre de la carpeta actual.
    let segs = c.path_segments_of(id);
    assert!(!segs.is_empty());
    assert_eq!(segs.last().unwrap().1, tmp.path().display().to_string());
    // Autocompletado: tecleando "<tmp>\al" matchea Alpha y alfajor (case-insensitive).
    let buffer = format!("{}\\al", tmp.path().display());
    let sugg = c.path_autocomplete(&buffer);
    assert!(sugg.iter().any(|s| s == "Alpha"));
    assert!(sugg.iter().any(|s| s == "alfajor"));
    assert!(!sugg.iter().any(|s| s == "Beta"));
}

/// La navegación desde un panel auxiliar (Árbol) va al ÚLTIMO panel Files activo, no al
/// primero. Regresión del bug "clic en disco cambia el primer panel".
#[test]
fn navega_al_ultimo_files_activo_no_al_primero() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.active_id().unwrap(); // primer Files
    c.add_pane_split(area()); // segundo Files, queda activo
    let second = c.active_id().unwrap();
    assert_ne!(first, second);
    // Agregar un Árbol y activarlo (simula clic en el panel Carpetas).
    c.add_pane_of(PanePurpose::Tree, area());
    let tree = c.active_id().unwrap();
    c.set_active(tree);
    // Navegar desde el árbol → debe ir al SEGUNDO Files (el último activo), no al primero.
    c.navigate_active_to(sub.clone());
    assert_eq!(
        c.ws.pane(second)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone()),
        Some(sub.clone())
    );
    assert_ne!(
        c.ws.pane(first)
            .and_then(|p| p.files.as_ref())
            .map(|f| f.current_dir.clone()),
        Some(sub)
    );
}

#[test]
fn marcar_un_files_sincroniza_el_arbol_comun_con_su_carpeta() {
    let tmp = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.active_id().unwrap();
    c.add_pane_split(area());
    let second = c.active_id().unwrap();
    c.open_in_pane(second, other.path().to_path_buf());
    assert!(drain(&mut c));
    c.add_pane_of(PanePurpose::Tree, area());
    let tree = c.active_id().unwrap();

    c.set_active(first);
    assert_eq!(
        c.trees
            .get(&tree)
            .and_then(|model| model.active_path.as_ref()),
        Some(&tmp.path().to_path_buf())
    );
    c.set_active(second);
    assert_eq!(
        c.trees
            .get(&tree)
            .and_then(|model| model.active_path.as_ref()),
        Some(&other.path().to_path_buf())
    );
    assert_eq!(
        c.tree_cursor.get(&tree),
        Some(&other.path().to_path_buf()),
        "el cursor de teclado acompaña la selección visible del árbol"
    );
}

#[test]
fn arbol_dedicado_navega_solo_su_panel_enlazado() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.active_id().unwrap();
    c.add_pane_split(area());
    let linked_files = c.active_id().unwrap();
    c.add_pane_of(PanePurpose::Tree, area());
    let tree = c.active_id().unwrap();
    assert!(c.toggle_tree_link(tree));
    assert_eq!(c.ws.linked_files(tree), Some(linked_files));

    // Cambiar el Files global activo no cambia el destino fijo del árbol dedicado.
    c.set_active(first);
    assert!(c.navigate_tree_to(tree, target.clone()));
    assert_eq!(c.path_of(linked_files), target.display().to_string());
    assert_eq!(c.path_of(first), tmp.path().display().to_string());
}

#[test]
fn explorador_enlazado_crea_pareja_persistente_y_ponderada() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    c.add_linked_browser();

    let tree =
        c.ws.panes()
            .iter()
            .find(|p| p.purpose == PanePurpose::Tree)
            .map(|p| p.id)
            .unwrap();
    let linked_files = c.ws.linked_files(tree).expect("árbol dedicado");
    assert!(c.ws.is_link_member(tree));
    assert!(c.ws.is_link_member(linked_files));
    assert_eq!(c.active_id(), Some(linked_files));

    let rects: std::collections::HashMap<_, _> = c.pane_rects(area()).into_iter().collect();
    let tree_rect = rects.get(&tree).unwrap();
    let files_rect = rects.get(&linked_files).unwrap();
    assert!(tree_rect.x < files_rect.x, "el árbol queda a la izquierda");
    assert!(
        tree_rect.w < files_rect.w,
        "el árbol usa la fracción angosta"
    );

    let restored = Workspace::from_persist(&c.session_persist()).unwrap();
    assert_eq!(restored.linked_files(tree), Some(linked_files));
}

/// Árbol+Files de `add_linked_browser` es un bloque: cerrar cualquiera de los dos quita ambos,
/// conserva el panel previo y no deja enlaces ni hojas de layout huérfanos.
#[test]
fn cerrar_explorador_enlazado_cierra_ambos_miembros() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let original = c.active_id().unwrap();
    c.add_linked_browser();

    let (tree, files) = c.ws.tree_links().into_iter().next().unwrap();
    assert!(c.can_close_pane(tree));
    assert!(c.can_close_pane(files));
    c.close_pane(tree);

    assert!(c.ws.pane(original).is_some());
    assert!(c.ws.pane(tree).is_none());
    assert!(c.ws.pane(files).is_none());
    assert!(c.ws.tree_links().is_empty());
    assert_eq!(c.ws.layout.pane_ids(), vec![original]);
}

/// El botón X de una pestaña usa la misma semántica de bloque para un explorador enlazado.
#[test]
fn cerrar_pestana_de_explorador_enlazado_cierra_toda_la_pareja() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let original = c.active_id().unwrap();
    c.add_linked_browser();
    let (tree, files) = c.ws.tree_links().into_iter().next().unwrap();

    c.close_tab(files);

    assert!(c.ws.pane(original).is_some());
    assert!(c.ws.pane(tree).is_none());
    assert!(c.ws.pane(files).is_none());
}

/// F5B: si un panel quedó parado en una carpeta que desapareció (p. ej. se sacó el USB), el
/// panel marca su carpeta como "perdida" (aviso IN-PLACE, sin popup global). Elegir "subir al
/// ancestro existente" lo reubica a la carpeta superior que exista y limpia el aviso.
#[test]
fn carpeta_perdida_se_detecta_por_panel_y_sube_al_ancestro() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("usb");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new_in(sub.clone(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    c.refresh_missing_cache();
    assert!(drain_missing(&mut c));
    assert!(!c.pane_dir_missing(id), "al inicio la carpeta existe");
    std::fs::remove_dir_all(&sub).unwrap(); // "sacar el USB"
                                            // El estado "missing" se cachea y se recalcula async en eventos reales (aquí
                                            // simulamos el evento de cambio de discos + los ticks que drenan el probe).
    c.refresh_missing_cache();
    assert!(drain_missing(&mut c));
    assert!(
        c.pane_dir_missing(id),
        "el panel detecta su carpeta perdida"
    );
    assert!(
        c.pane_has_existing_ancestor(id),
        "el worker conserva la ruta del ancestro para el botón subir"
    );
    assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
    c.missing_folder_retry(id);
    assert!(
        c.listings.contains_key(&id),
        "reintentar siempre delega al listing worker"
    );
    assert!(drain(&mut c));
    assert!(
        c.pane_dir_missing(id),
        "si sigue ausente, se conserva el aviso"
    );
    // "Subir al ancestro existente": el panel queda en tmp (el padre que sigue vivo).
    c.missing_folder_go_ancestor(id);
    // El aviso se limpia de inmediato: la entrada cacheada era de la carpeta VIEJA y queda
    // invalidada al navegar (sin esperar al probe nuevo).
    assert!(!c.pane_dir_missing(id), "ya no está perdida tras subir");
    assert_eq!(c.ws.active_files().unwrap().current_dir, tmp.path());
}

#[test]
fn respaldos_de_home_usan_la_preferencia_del_usuario() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("unidad-desconectada");
    let mut c = WorkspaceCtrl::new_in(missing.clone(), tmp.path().to_path_buf());
    c.config.settings.home_dir = home.path().to_string_lossy().into_owned();
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();

    assert_eq!(c.configured_home_dir(), home.path());

    c.missing_folder_close_pane(id);
    assert_eq!(c.ws.active_files().unwrap().current_dir, home.path());
    assert_eq!(c.template_home(), home.path());
    assert!(drain(&mut c));

    // Crear un panel cuando el activo no es Files debe conservar Home como su ruta inicial.
    c.ws = Workspace::new();
    let anchor = c.ws.add_pane(PanePurpose::Operations, PathBuf::new());
    c.ws.set_active(anchor);
    c.add_pane_split_dir(SplitDir::Horizontal, false);
    assert_eq!(c.ws.active_files().unwrap().current_dir, home.path());
    assert!(drain(&mut c));
}

/// REVEAL: al fijar un destino, el árbol expande progresivamente los ancestros hasta él.
#[test]
fn arbol_revela_la_carpeta_objetivo() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a");
    let b = a.join("b");
    std::fs::create_dir_all(&b).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    // Árbol con raíz manual en tmp (no dependemos de las unidades reales).
    let tree_id = c.ws.add_pane(PanePurpose::Tree, std::path::PathBuf::new());
    let mut t = DirTree::default();
    t.roots
        .push(naygo_core::tree::TreeNode::folder(tmp.path().to_path_buf()));
    c.trees.insert(tree_id, t);
    // Pedir revelar tmp/a/b: debe expandir tmp (raíz) y luego tmp/a.
    c.reveal_targets.insert(tree_id, b.clone());
    c.pump_reveal();
    // Drenar los workers de árbol + avanzar el reveal hasta que no quede target.
    for _ in 0..4000 {
        let done = c.pump_tree();
        if done {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // tmp/a debe haber quedado expandido (su hijo "b" es el destino, queda visible).
    let a_expanded = c
        .trees
        .get(&tree_id)
        .and_then(|t| t.node_at(&a))
        .map(|n| n.expanded && n.children.is_some())
        .unwrap_or(false);
    assert!(a_expanded, "el ancestro tmp/a quedó expandido (revela b)");
    assert!(
        c.reveal_targets.is_empty(),
        "el target se limpió al completar el reveal"
    );
}

/// REGRESIÓN (heredada de F1): navegar a una carpeta repuebla la vista del panel
/// activo (el listado de la carpeta nueva se arranca al navegar).
#[test]
fn navegar_repuebla_la_vista() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("dentro.txt"), b"x").unwrap();

    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c), "listado inicial termina");
    let id = c.active_id().unwrap();
    let pos = active_pos_of(&c, "sub").expect("'sub' visible");
    assert!(c.on_row_double_clicked(id, pos), "doble clic navega");
    assert!(drain(&mut c), "listado de sub termina");
    let rows = c.rows_of(c.active_id().unwrap(), 8, std::time::Instant::now());
    assert!(
        rows.iter().any(|r| r.name == "dentro.txt"),
        "la vista refleja la carpeta nueva (no vacía)"
    );
}

/// El doble-clic detectado en Rust (dos on_row_clicked rápidos en la misma fila) navega;
/// dos clics LENTOS no.
#[test]
fn doble_clic_en_rust_navega() {
    use std::time::{Duration, Instant};
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("dentro.txt"), b"x").unwrap();

    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    let pos = active_pos_of(&c, "sub").expect("'sub' visible");

    // Una sola línea de tiempo sintética (mezclar Instant::now() con offsets daría
    // instantes incoherentes). Base + offsets crecientes.
    let base = Instant::now();
    // Dos clics LENTOS (separados > 700 ms) NO navegan: solo seleccionan.
    assert!(!c.on_row_clicked(id, pos, false, false, base));
    assert!(!c.on_row_clicked(id, pos, false, false, base + Duration::from_millis(900)));
    assert_eq!(c.path_of(id), tmp.path().display().to_string());

    // Dos clics RÁPIDOS (dentro de 700 ms) SÍ navegan. Siguen la misma línea de tiempo.
    let t1 = base + Duration::from_secs(5);
    assert!(
        !c.on_row_clicked(id, pos, false, false, t1),
        "1er clic: selecciona"
    );
    assert!(
        c.on_row_clicked(id, pos, false, false, t1 + Duration::from_millis(150)),
        "2do clic rápido: doble-clic → navega"
    );
    assert!(drain(&mut c));
    let rows = c.rows_of(c.active_id().unwrap(), 8, std::time::Instant::now());
    assert!(rows.iter().any(|r| r.name == "dentro.txt"));
}

/// Clic-medio sobre una fila-CARPETA la abre en un panel NUEVO (siempre divide, aunque ya haya
/// otro panel disponible): a diferencia de Shift+Enter/Ctrl+doble-clic, nunca reusa un panel
/// existente.
#[test]
fn clic_medio_en_carpeta_abre_panel_nuevo() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let origin = c.active_id().unwrap();
    let pos = active_pos_of(&c, "sub").expect("'sub' visible");
    c.set_area(area());

    assert!(
        c.on_row_middle_clicked(origin, pos),
        "clic-medio en carpeta abre panel nuevo"
    );
    assert!(drain(&mut c));

    // Ahora hay DOS paneles Files: el origen sigue en tmp, el nuevo quedó activo en sub.
    assert_eq!(c.ws.files_panes().len(), 2, "se creó un panel nuevo");
    let new_id = c.active_id().unwrap();
    assert_ne!(new_id, origin, "el panel nuevo quedó activo, no el origen");
    assert_eq!(c.path_of(origin), tmp.path().display().to_string());
    assert_eq!(c.path_of(new_id), sub.display().to_string());
}

/// Clic-medio sobre una fila-ARCHIVO no hace nada: no abre el archivo ni crea un panel.
#[test]
fn clic_medio_en_archivo_no_hace_nada() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let origin = c.active_id().unwrap();
    let pos = active_pos_of(&c, "a.txt").expect("'a.txt' visible");
    c.set_area(area());

    assert!(
        !c.on_row_middle_clicked(origin, pos),
        "clic-medio sobre un archivo no hace nada"
    );
    assert_eq!(c.ws.files_panes().len(), 1, "no se creó ningún panel");
}

/// Shift+Enter sobre la carpeta enfocada la abre en OTRO panel (el origen NO navega), vía
/// `run_action(Action::OpenFocusedOtherPane)`. Con un solo panel, divide y usa el nuevo.
#[test]
fn shift_enter_abre_carpeta_enfocada_en_otro_panel() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let origin = c.active_id().unwrap();
    let pos = active_pos_of(&c, "sub").expect("'sub' visible");
    c.with_active(|f| f.select_single(pos));
    c.set_area(area());

    assert!(
        c.run_action(naygo_core::keymap::Action::OpenFocusedOtherPane),
        "Shift+Enter abre en otro panel"
    );
    assert!(drain(&mut c));

    let panes = c.ws.files_panes();
    assert_eq!(panes.len(), 2, "se dividió (no había otro panel)");
    // El origen NO navegó (ni pierde el foco: split_for_target lo deja activo): sigue en tmp.
    assert_eq!(c.path_of(origin), tmp.path().display().to_string());
    assert_eq!(c.active_id(), Some(origin), "el foco se queda en el origen");
    let new_id = panes.into_iter().find(|&p| p != origin).unwrap();
    assert_eq!(c.path_of(new_id), sub.display().to_string());
}

/// Shift+Enter sin nada enfocado (o enfocado un archivo) no hace nada.
#[test]
fn shift_enter_sin_carpeta_enfocada_no_hace_nada() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let pos = active_pos_of(&c, "a.txt").expect("'a.txt' visible");
    c.with_active(|f| f.select_single(pos));
    c.set_area(area());

    assert!(!c.run_action(naygo_core::keymap::Action::OpenFocusedOtherPane));
    assert_eq!(c.ws.files_panes().len(), 1, "no se creó ningún panel");
}

/// Agregar un panel divide el layout y deja DOS paneles Files; el nuevo queda activo.
#[test]
fn agregar_panel_divide_y_deja_dos() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.active_id().unwrap();
    c.add_pane_split(area());
    assert!(drain(&mut c));
    // Dos paneles Files en el layout, y el activo es el nuevo (distinto del primero).
    assert_eq!(c.ws.files_panes().len(), 2);
    assert_ne!(c.active_id(), Some(first), "el panel nuevo queda activo");
    // El área se reparte en dos rects no vacíos.
    let area = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    let rects = c.pane_rects(area);
    assert_eq!(rects.len(), 2);
    assert!(rects.iter().all(|(_, r)| r.w > 1.0 && r.h > 1.0));
    // Y hay un splitter entre ellos.
    assert_eq!(c.split_handles(area).len(), 1);
}

/// Agregar un panel especial (no-Files) crea el purpose correcto y NO arranca un
/// listado de archivos (los auxiliares no listan). El Tree inicializa su DirTree.
#[test]
fn agregar_panel_especial_no_lista_archivos() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let files_listings_antes = c.listings.len();
    c.add_pane_of(PanePurpose::Tree, area());
    // Se agregó un panel Tree y no aumentaron los listados de archivos.
    assert!(c.ws.panes().iter().any(|p| p.purpose == PanePurpose::Tree));
    assert_eq!(
        c.listings.len(),
        files_listings_antes,
        "un panel Tree no arranca listado de archivos"
    );
    // El Tree tiene su DirTree (con al menos una raíz, si el sistema tiene unidades).
    let tree_id =
        c.ws.panes()
            .iter()
            .find(|p| p.purpose == PanePurpose::Tree)
            .unwrap()
            .id;
    assert!(c.trees.contains_key(&tree_id));
}

/// Tab conserva el recorrido Commander sólo entre Files; Ctrl+Tab recorre todos los paneles
/// del workspace, incluidos los auxiliares. Es importante para disposiciones densas donde el
/// usuario quiere llegar a Favoritos, Árbol o Preview sin usar el mouse.
#[test]
fn ctrl_tab_cicla_todos_los_paneles_y_tab_solo_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let files = c.active_id().unwrap();
    c.add_pane_of(PanePurpose::Tree, area());
    let tree = c.active_id().unwrap();
    c.add_pane_of(PanePurpose::Favorites, area());
    let favorites = c.active_id().unwrap();
    c.add_pane_of(PanePurpose::Search, area());
    let search = c.active_id().unwrap();

    let tab: slint::SharedString = slint::platform::Key::Tab.into();
    assert!(c.on_key(tab.as_str(), true, false, false));
    assert_eq!(c.active_id(), Some(files));
    assert!(c.on_key(tab.as_str(), true, false, false));
    assert_eq!(c.active_id(), Some(tree));
    assert!(c.on_key(tab.as_str(), true, false, false));
    assert_eq!(c.active_id(), Some(favorites));
    assert!(c.on_key(tab.as_str(), true, false, false));
    assert_eq!(c.active_id(), Some(search));
    // El recorrido no termina en Buscar: vuelve a los Files y sigue siendo circular.
    assert!(c.on_key(tab.as_str(), true, false, false));
    assert_eq!(c.active_id(), Some(files));

    // Tab no abandona un panel especial para recorrer auxiliares; mantiene el convenio
    // Commander de saltar exclusivamente entre paneles Files.
    assert!(!c.on_key(tab.as_str(), false, false, false));
    assert_eq!(c.active_id(), Some(files));
}

/// El inspector refleja el ítem enfocado del panel Files activo, aunque el panel activo
/// sea un panel especial.
#[test]
fn inspector_lee_el_files_activo() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("dato.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    // Enfocar la primera fila de la vista.
    if let Some(f) = c.ws.active_files_mut() {
        f.select_single(0);
    }
    let info = c.inspector_info();
    assert!(info.present, "hay un ítem enfocado");
}

/// La selección de Bandeja es una fuente de primer nivel para Preview/Propiedades, pero no
/// exige I/O en el callback del clic: los workers existentes reciben la ruta en el tick.
#[test]
fn bandeja_seleccionada_alimenta_inspector_y_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("reunido.txt");
    std::fs::write(&path, b"contenido").unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    assert_eq!(c.basket.add([path.clone()]), 1);
    assert!(c.basket_select(0));

    let info = c.inspector_info();
    assert!(info.present);
    assert_eq!(info.name, "reunido.txt");
    assert_eq!(c.metadata_target(), Some(path));
    assert!(c.basket_rows().first().is_some_and(|row| row.selected));
}

/// Copiar/Mover desde Bandeja ofrece los paneles Files abiertos por el mismo radar usado por
/// F-keys, y marca explícitamente que la operación debe tomar rutas de la bandeja.
#[test]
fn bandeja_abre_radar_con_paneles_visibles_como_destino() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("reunido.txt");
    std::fs::write(&path, b"contenido").unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    c.last_area = area();
    assert_eq!(c.basket.add([path]), 1);

    c.basket_select_all();
    assert!(c.basket_request_transfer(false));
    let radar = c.destination_radar.as_ref().expect("radar abierto");
    assert_eq!(radar.origin, DestinationRadarOrigin::Basket);
    assert!(!radar.move_files);
    assert!(
        !radar.candidates.is_empty(),
        "el panel Files abierto se ofrece primero"
    );
}

/// Navegar desde un favorito mueve el panel Files activo y lo registra en recientes.
#[test]
fn navegar_desde_favorito() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    c.favorites.toggle(&sub);
    assert!(c.go_favorite(0), "navega al favorito 0");
    assert!(drain(&mut c));
    assert_eq!(
        c.ws.active_files().map(|f| f.current_dir.clone()),
        Some(sub.clone())
    );
    // La carpeta nueva quedó en recientes (al frente).
    assert_eq!(c.recents.list().first(), Some(&sub));
}

/// El árbol editable de favoritos: crear grupo, mover un favorito dentro, expandir/colapsar,
/// renombrar y eliminar; todo persiste a `favorites.json` (un controlador nuevo lo restaura).
#[test]
fn favoritos_arbol_editable_y_persistente() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("cfg");
    let a = tmp.path().join("a");
    let b = tmp.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let mut c = WorkspaceCtrl::new_in(a.clone(), cfg.clone());
    assert!(drain(&mut c));
    c.favorites.add_favorite(&a);
    c.favorites.add_favorite(&b);

    // Crear grupo en la raíz y mover el favorito `b` dentro.
    c.fav_new_group("", "Trabajo");
    let opts = c.fav_group_options();
    assert_eq!(opts.len(), 1);
    let (np, gid) = opts[0].clone();
    assert_eq!(np, "Trabajo");
    c.fav_move_node(false, "", &b.display().to_string(), &gid);

    // Colapsado por defecto: solo se ven `a` y el grupo (la hoja `b` está oculta).
    let rows = c.fav_tree_rows();
    assert!(rows.iter().any(|r| r.is_group && r.name == "Trabajo"));
    assert!(!rows.iter().any(|r| r.path == b.display().to_string()));

    // Expandir el grupo revela la hoja con sangría.
    c.fav_toggle_expand("Trabajo");
    let rows = c.fav_tree_rows();
    let inner = rows.iter().find(|r| r.path == b.display().to_string());
    assert!(inner.is_some() && inner.unwrap().depth == 1);

    // Renombrar el grupo (re-mapea la expansión: sigue expandido tras renombrar).
    let gid = c
        .fav_group_options()
        .into_iter()
        .find(|(n, _)| n == "Trabajo")
        .unwrap()
        .1;
    c.fav_rename_group(&gid, "Proyectos");
    assert!(c.fav_expanded.contains("Proyectos"));
    let rows = c.fav_tree_rows();
    assert!(rows.iter().any(|r| r.is_group && r.name == "Proyectos"));
    // Sigue expandido: la hoja interna se ve.
    assert!(rows
        .iter()
        .any(|r| r.path == b.display().to_string() && r.depth == 1));

    wait_for_workspace_persistence(&c);
    // Persistió: un controlador nuevo (mismo config_dir) ve el grupo renombrado con su hoja.
    let mut c2 = WorkspaceCtrl::new_in(a.clone(), cfg.clone());
    assert!(drain(&mut c2));
    assert!(c2.favorites.contains(&b));
    assert!(c2
        .favorites
        .roots()
        .iter()
        .any(|n| matches!(n, FavNode::Group { name, .. } if name == "Proyectos")));

    // Eliminar el grupo borra su hoja interna; el favorito de la raíz queda.
    let gid = c2
        .fav_group_options()
        .into_iter()
        .find(|(n, _)| n == "Proyectos")
        .unwrap()
        .1;
    c2.fav_delete_node(true, &gid, "");
    assert!(!c2.favorites.contains(&b));
    assert!(c2.favorites.contains(&a));
}

/// "Mover a…" no debe ofrecer mover un grupo dentro de sí mismo ni de sus descendientes.
#[test]
fn fav_move_targets_excluye_el_grupo_y_sus_descendientes() {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("cfg");
    let a = tmp.path().join("a");
    std::fs::create_dir(&a).unwrap();
    let mut c = WorkspaceCtrl::new_in(a.clone(), cfg);
    assert!(drain(&mut c));
    // Estructura: "Trabajo" (root) con subgrupo "Sub"; y "Personal" (root) aparte.
    c.fav_new_group("", "Trabajo");
    let trabajo = c
        .fav_group_options()
        .into_iter()
        .find(|(n, _)| n == "Trabajo")
        .unwrap()
        .1;
    c.fav_new_group(&trabajo, "Sub");
    c.fav_new_group("", "Personal");

    let labels: Vec<String> = c.fav_group_options().into_iter().map(|(n, _)| n).collect();
    assert!(labels.contains(&"Trabajo".to_string()));
    assert!(labels.contains(&"Trabajo/Sub".to_string()));
    assert!(labels.contains(&"Personal".to_string()));

    // Mover el GRUPO "Trabajo": destinos válidos = solo "Personal" (no él mismo, no su "Sub").
    let targets: Vec<String> = c
        .fav_move_targets(true, &trabajo)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    assert!(!targets.contains(&"Trabajo".to_string()), "no a sí mismo");
    assert!(
        !targets.contains(&"Trabajo/Sub".to_string()),
        "no a un descendiente"
    );
    assert!(targets.contains(&"Personal".to_string()), "sí a un hermano");

    // Para una HOJA (favorito): sin restricción, cualquier grupo es destino válido.
    let leaf_targets = c.fav_move_targets(false, "");
    assert_eq!(
        leaf_targets.len(),
        3,
        "una hoja puede ir a cualquiera de los 3 grupos"
    );
}

/// Expandir una rama del árbol la marca expandida y, tras drenar, puebla sus hijos;
/// colapsar la vuelve a cerrar.
#[test]
fn arbol_expande_colapsa_y_puebla() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("rama");
    std::fs::create_dir(&sub).unwrap();
    std::fs::create_dir(sub.join("hoja")).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    // Crear un panel Tree con una raíz manual apuntando a tmp (no dependemos de las
    // unidades reales del sistema para el test).
    let tree_id = c.ws.add_pane(PanePurpose::Tree, std::path::PathBuf::new());
    let mut t = DirTree::default();
    t.roots
        .push(naygo_core::tree::TreeNode::folder(tmp.path().to_path_buf()));
    c.trees.insert(tree_id, t);

    c.tree_expand(tree_id, tmp.path().to_path_buf());
    // Drenar el worker del árbol.
    for _ in 0..2000 {
        if c.pump_tree() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let rows = c.tree_rows(tree_id);
    assert!(
        rows.iter().any(|r| r.name == "rama"),
        "la rama aparece como hijo"
    );
    // Colapsar: la raíz deja de estar expandida.
    c.tree_collapse(tree_id, tmp.path().to_path_buf());
    let node_expanded = c
        .trees
        .get(&tree_id)
        .and_then(|t| t.node_at(tmp.path()))
        .map(|n| n.expanded)
        .unwrap();
    assert!(!node_expanded, "la raíz quedó colapsada");
}

fn area() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    }
}

/// resolve_target: 1 panel → NeedsSplit; 2 → Direct(el otro); 3+ → Pick.
#[test]
fn resolve_target_segun_cantidad_de_paneles() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    // Un solo panel → hay que dividir.
    assert_eq!(c.resolve_target(a, area()), PaneTarget::NeedsSplit);
    // Dos paneles → destino directo (el otro).
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    assert_eq!(c.resolve_target(b, area()), PaneTarget::Direct(a));
    // Tres paneles → selector (Pick con 2 candidatos).
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let third = c.active_id().unwrap();
    match c.resolve_target(third, area()) {
        PaneTarget::Pick(cands) => assert_eq!(cands.len(), 2),
        other => panic!("esperaba Pick, fue {other:?}"),
    }
}

/// Swap intercambia las carpetas de dos paneles.
#[test]
fn swap_intercambia_carpetas() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("otra");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    // Mandar b a la subcarpeta.
    c.open_in_pane(b, sub.clone());
    assert!(drain(&mut c));
    let dir_a_antes = c.path_of(a);
    let dir_b_antes = c.path_of(b);
    c.swap_panes(a, b);
    assert_eq!(c.path_of(a), dir_b_antes);
    assert_eq!(c.path_of(b), dir_a_antes);
}

/// Con 3+ paneles, una acción deja un pending_pick; elegir el número lo aplica.
#[test]
fn selector_pendiente_y_resolucion() {
    let tmp = tempfile::tempdir().unwrap();
    let sub = tmp.path().join("dest");
    std::fs::create_dir(&sub).unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    c.add_pane_split(area());
    assert!(drain(&mut c));
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let origin = c.active_id().unwrap();
    // Clonar desde el origen: 3 paneles → queda pendiente el selector con 2 candidatos.
    let acted = c.request_action(PaneAction::Clone, origin, area());
    assert!(!acted, "no actúa de inmediato: espera la elección");
    assert!(c.pending_pick.is_some());
    let candidates = c.pending_pick.as_ref().unwrap().candidates.clone();
    assert_eq!(candidates.len(), 2);
    // Elegir el panel 1: clona la carpeta del origen ahí.
    assert!(c.pick_resolve(1));
    assert!(drain(&mut c));
    assert!(c.pending_pick.is_none(), "el selector se cerró");
    assert_eq!(c.path_of(candidates[0]), c.path_of(origin));
}

/// Apilar el origen sobre otro panel los agrupa en pestañas; el origen queda activo.
#[test]
fn apilar_crea_un_grupo_de_pestanas() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    // Apilar b sobre a: quedan en un grupo de 2.
    c.stack_into(b, a);
    let groups = c.tab_groups();
    assert_eq!(groups.len(), 1);
    let (members, active) = &groups[0];
    assert_eq!(members.len(), 2);
    assert!(members.contains(&a) && members.contains(&b));
    // El miembro activo es el apilado (b).
    assert_eq!(members[*active], b);
}

/// set_active_tab cambia la pestaña visible del grupo.
#[test]
fn cambiar_pestana_activa() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    c.stack_into(b, a);
    // Activar a: pasa a ser la pestaña visible.
    c.set_active_tab(a);
    let (members, active) = c.tab_groups()[0].clone();
    assert_eq!(members[active], a);
    assert_eq!(c.active_id(), Some(a));
}

/// Cerrar una pestaña la quita; con una sola restante el grupo se colapsa a hoja.
#[test]
fn cerrar_pestana_colapsa_el_grupo() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    c.stack_into(b, a);
    assert_eq!(c.tab_groups().len(), 1);
    // Cerrar b: queda solo a, el grupo desaparece.
    c.close_tab(b);
    assert!(c.tab_groups().is_empty());
    assert!(c.ws.pane(b).is_none(), "el panel cerrado ya no existe");
    assert!(c.ws.pane(a).is_some(), "el otro sigue");
}

/// Soltar un panel en el CENTRO de otro los apila como pestañas.
#[test]
fn drop_en_el_centro_apila() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    // Layout: [a | b] en 800x600. Soltar a en el centro de b.
    let ar = area();
    let rects = c.pane_rects(ar);
    let b_rect = rects.iter().find(|(id, _)| *id == b).unwrap().1;
    let (cx, cy) = (b_rect.x + b_rect.w / 2.0, b_rect.y + b_rect.h / 2.0);
    assert!(c.perform_drop(a, cx, cy, ar));
    // Quedan agrupados.
    let groups = c.tab_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0.len(), 2);
}

/// Soltar un panel sobre sí mismo no hace nada.
#[test]
fn drop_sobre_si_mismo_es_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    let ar = area();
    let r = c.pane_rects(ar)[0].1;
    assert!(!c.perform_drop(a, r.x + r.w / 2.0, r.y + r.h / 2.0, ar));
    assert!(c.tab_groups().is_empty());
}

/// Soltar en un borde divide; el panel arrastrado queda en el lado correspondiente.
#[test]
fn drop_en_borde_divide() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let a = c.active_id().unwrap();
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    // Apilar primero a+b para tener un grupo, luego sacar 'a' soltándolo en un borde.
    c.stack_into(a, b);
    assert_eq!(c.tab_groups().len(), 1);
    let ar = area();
    let rects = c.pane_rects(ar);
    // El grupo ocupa todo; soltar 'a' en el borde derecho lo separa en un split.
    let r = rects[0].1;
    let (px, py) = (r.x + r.w - 5.0, r.y + r.h / 2.0);
    assert!(c.perform_drop(a, px, py, ar));
    // Ya no hay grupo (a salió); hay dos paneles en un split.
    assert!(c.tab_groups().is_empty());
    assert_eq!(c.pane_rects(ar).len(), 2);
}

/// Esc (vía pick_cancel) cierra el selector sin actuar.
#[test]
fn cancelar_selector() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    c.add_pane_split(area());
    assert!(drain(&mut c));
    c.add_pane_split(area());
    assert!(drain(&mut c));
    let origin = c.active_id().unwrap();
    c.request_action(PaneAction::Clone, origin, area());
    assert!(c.pending_pick.is_some());
    c.pick_cancel();
    assert!(c.pending_pick.is_none());
}

/// Vista profunda: activar sobre un árbol temporal, drenar hasta completar y cancelar.
#[test]
fn vista_profunda_metadata_y_tamano_usan_foco_profundo_no_lista_normal() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let normal = root.path().join("normal.txt");
    let dir = root.path().join("sub");
    std::fs::write(&normal, b"normal").unwrap();
    std::fs::create_dir(&dir).unwrap();
    let nested = dir.join("interior.txt");
    std::fs::write(&nested, b"profundo").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let normal_pos = active_pos_of(&c, "normal.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(normal_pos);
    let file_pos = c.deep_position_of(pane, &nested).unwrap();
    c.deep_select_at(pane, file_pos, false, false);
    assert_eq!(c.metadata_target(), Some(nested));
    assert_eq!(c.inspector_info().name, "interior.txt");
    let dir_pos = c.deep_position_of(pane, &dir).unwrap();
    c.deep_select_at(pane, dir_pos, false, false);
    assert_eq!(c.metadata_target(), None);
    assert_eq!(c.inspector_info().path, dir.display().to_string());
    c.compute_size_active();
    let job = c.size_job.as_ref().expect("worker de tamaño");
    assert_eq!(job.target, dir);
    job.token.cancel();
}

#[test]
fn vista_profunda_doble_clic_no_abre_otra_fila_si_cambia_el_orden() {
    use naygo_core::fs_model::{Entry, EntryKind};
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    let mk = |path: PathBuf| Entry {
        name: path.file_name().unwrap().to_string_lossy().into(),
        path,
        kind: EntryKind::Directory,
        size: None,
        modified: None,
        created: None,
        hidden: false,
        system: false,
    };
    *c.deep_job.as_mut().unwrap().items = vec![(mk(a), 0), (mk(b), 0)];
    let now = std::time::Instant::now();
    assert!(!c.on_row_clicked(pane, 0, false, false, now));
    c.deep_job.as_mut().unwrap().items.swap(0, 1);
    assert!(
        !c.on_row_double_clicked_native(pane, 0, false, false),
        "el evento nativo no debe usar la fila nueva"
    );
    assert!(!c.on_row_clicked(
        pane,
        0,
        false,
        false,
        now + std::time::Duration::from_millis(40)
    ));
    assert!(!c.on_row_double_clicked_native(pane, 0, false, false));
    assert!(c.is_deep_active(pane));
    assert_eq!(c.ws.active_files().unwrap().current_dir, root.path());
}

#[test]
fn vista_profunda_comprime_lote_de_una_misma_carpeta() {
    use std::io::Read;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let dir = root.path().join("sub");
    std::fs::create_dir(&dir).unwrap();
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    std::fs::write(&a, b"uno").unwrap();
    std::fs::write(&b, b"dos").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let p1 = c.deep_position_of(pane, &a).unwrap();
    let p2 = c.deep_position_of(pane, &b).unwrap();
    c.deep_select_at(pane, p1, false, false);
    c.deep_select_at(pane, p2, true, false);
    c.open_row_context_menu(pane, p1, 0.0, 0.0);
    c.op_compress_prompt();
    c.close_context_menu();
    match &c.ops.pending_dialog {
        Some(crate::ops_ctrl::OpDialog::NameInput {
            purpose: crate::ops_ctrl::NamePurpose::Compress { sources },
            ..
        }) => {
            assert_eq!(sources.len(), 2);
            assert!(sources.contains(&a) && sources.contains(&b));
        }
        _ => panic!("debe ofrecer el lote completo"),
    }
    c.ops.name_changed("paquete".to_string());
    let label = c.config.t("ops.kind_compress");
    assert!(c.ops.name_confirm(&label));
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(root.path().join("paquete.zip")).unwrap())
            .unwrap();
    let mut first = String::new();
    zip.by_name("a.txt")
        .unwrap()
        .read_to_string(&mut first)
        .unwrap();
    let mut second = String::new();
    zip.by_name("b.txt")
        .unwrap()
        .read_to_string(&mut second)
        .unwrap();
    assert_eq!(first, "uno");
    assert_eq!(second, "dos");
    c.reconcile_completed_operations();
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(c
        .deep_position_of(pane, &root.path().join("paquete.zip"))
        .is_some());
    drop(zip);
    let nested_zip = dir.join("anidado.zip");
    std::fs::copy(root.path().join("paquete.zip"), &nested_zip).unwrap();
    c.restart_deep_listing(None);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let pos = c.deep_position_of(pane, &nested_zip).unwrap();
    c.deep_select_at(pane, pos, false, false);
    assert!(c.sel_is_single_zip());
    c.op_extract_here();
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read(dir.join("anidado").join("a.txt")).unwrap(),
        b"uno"
    );
    assert_eq!(
        std::fs::read(dir.join("anidado").join("b.txt")).unwrap(),
        b"dos"
    );
}

#[test]
fn vista_profunda_copia_lote_de_carpetas_distintas_al_otro_panel() {
    let root = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let one = a.join("uno.txt");
    let two = b.join("dos.txt");
    std::fs::write(&one, b"uno").unwrap();
    std::fs::write(&two, b"dos").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    let other = c.split_for_target().unwrap();
    c.open_in_pane(other, dest.path().to_path_buf());
    assert!(drain(&mut c));
    c.set_area(area());
    c.ws.set_active(pane);
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let p1 = c.deep_position_of(pane, &one).unwrap();
    let p2 = c.deep_position_of(pane, &two).unwrap();
    c.deep_select_at(pane, p1, false, false);
    c.deep_select_at(pane, p2, true, false);
    c.op_compress_prompt();
    assert!(c.ops.pending_dialog.is_some());
    c.ops.name_changed("mezcla".to_string());
    assert!(c.ops.name_confirm(&c.config.t("ops.kind_compress")));
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(root.path().join("mezcla.zip")).unwrap()).unwrap();
    assert!(zip.by_name("a/uno.txt").is_ok());
    assert!(zip.by_name("b/dos.txt").is_ok());
    drop(zip);
    c.op_to_other(false);
    let radar = c.destination_radar.as_ref().expect("destino explícito");
    assert_eq!(radar.sources.len(), 2);
    assert!(radar.sources.contains(&one) && radar.sources.contains(&two));
    let dest_index = radar
        .candidates
        .iter()
        .position(|candidate| candidate.path == dest.path())
        .unwrap();
    c.destination_radar_resolve(dest_index);
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(std::fs::read(dest.path().join("uno.txt")).unwrap(), b"uno");
    assert_eq!(std::fs::read(dest.path().join("dos.txt")).unwrap(), b"dos");
    assert!(one.exists() && two.exists());
    assert!(c.op_duplicate());
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(std::fs::read(a.join("uno - copia.txt")).unwrap(), b"uno");
    assert_eq!(std::fs::read(b.join("dos - copia.txt")).unwrap(), b"dos");
    c.reconcile_completed_operations();
    assert!(
        c.deep_complete_selection(pane).is_none(),
        "no operar con selección parcialmente relistada"
    );
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(c
        .deep_position_of(pane, &a.join("uno - copia.txt"))
        .is_some());
    let restored = c.deep_complete_selection(pane).unwrap();
    assert_eq!(restored.len(), 2);
    assert!(restored.contains(&one) && restored.contains(&two));
    let move_dest = dest.path().join("movidos");
    std::fs::create_dir(&move_dest).unwrap();
    let p1 = c.deep_position_of(pane, &one).unwrap();
    let p2 = c.deep_position_of(pane, &two).unwrap();
    c.deep_select_at(pane, p1, false, false);
    c.deep_select_at(pane, p2, true, false);
    c.op_to_other(true);
    c.destination_radar_resolve_path(move_dest.clone());
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    c.reconcile_completed_operations();
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!one.exists() && !two.exists());
    assert_eq!(std::fs::read(move_dest.join("uno.txt")).unwrap(), b"uno");
    assert_eq!(std::fs::read(move_dest.join("dos.txt")).unwrap(), b"dos");
    assert!(c.deep_position_of(pane, &one).is_none() && c.deep_position_of(pane, &two).is_none());
}

#[test]
#[ignore = "medición sintética manual; no es una prueba de rendimiento en CI"]
fn benchmark_vista_profunda_navegacion_y_filas() {
    use naygo_core::fs_model::{Entry, EntryKind};
    use naygo_core::keymap::Action;
    use std::time::Instant;
    for count in [10_000, 100_000] {
        let root = tempfile::tempdir().unwrap();
        let cfg = tempfile::tempdir().unwrap();
        let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let pane = c.active_id().unwrap();
        c.deep_start(pane);
        while !c.deep_job.as_ref().unwrap().done {
            c.deep_poll();
            std::thread::yield_now();
        }
        let job = c.deep_job.as_mut().unwrap();
        *job.items = (0..count)
            .map(|i| {
                let name = format!("archivo-{i:06}.txt");
                (
                    Entry {
                        path: root.path().join(&name),
                        name,
                        kind: EntryKind::File,
                        size: Some(64),
                        modified: None,
                        created: None,
                        hidden: false,
                        system: false,
                    },
                    1,
                )
            })
            .collect();
        let start = job.items[count - 50].0.path.clone();
        job.focused_path = Some(start.clone());
        job.selected_paths.insert(start);
        // Una ronda de calentamiento; las muestras no incluyen la creación del corpus.
        assert!(c.run_action(Action::MoveDown));
        let mut moves = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            assert!(c.run_action(Action::MoveDown));
            moves.push(t.elapsed().as_secs_f64() * 1_000.0);
        }
        let _ = c.rows_of(pane, 0, Instant::now());
        let mut rows = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            let result = c.rows_of(pane, 0, Instant::now());
            rows.push(t.elapsed().as_secs_f64() * 1_000.0);
            assert_eq!(result.len(), count);
        }
        let mut signatures = Vec::new();
        let mut footers = Vec::new();
        for _ in 0..20 {
            let t = Instant::now();
            assert!(c.rows_signature(pane, 0, Instant::now()).is_some());
            signatures.push(t.elapsed().as_secs_f64() * 1_000.0);
            let t = Instant::now();
            let _ = c.footer_text_of(pane);
            footers.push(t.elapsed().as_secs_f64() * 1_000.0);
        }
        moves.sort_by(f64::total_cmp);
        rows.sort_by(f64::total_cmp);
        signatures.sort_by(f64::total_cmp);
        footers.sort_by(f64::total_cmp);
        let sort_start = Instant::now();
        c.sort_by_kind(pane, 0);
        c.sort_by_kind(pane, 0);
        let sort_ms = sort_start.elapsed().as_secs_f64() * 500.0;
        c.column_menu_open(pane, 0, 0.0, 0.0);
        c.column_filter_set_text("archivo-000");
        let filter_start = Instant::now();
        c.column_filter_apply();
        assert_eq!(c.deep_visible_count(pane), 1_000);
        let filter_ms = filter_start.elapsed().as_secs_f64() * 1_000.0;
        println!("deep-bench n={count} move_ms={moves:?} rows_ms={rows:?} signature_ms={signatures:?} footer_ms={footers:?} sort_mean_ms={sort_ms:.3} filter_ms={filter_ms:.3}");
    }
}

#[test]
fn vista_profunda_drop_externo_captura_y_valida_identidad() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let origin = tempfile::tempdir().unwrap();
    let folder = root.path().join("carpeta");
    let other = root.path().join("otra");
    let source = origin.path().join("documento.txt");
    std::fs::create_dir(&folder).unwrap();
    std::fs::create_dir(&other).unwrap();
    std::fs::write(&source, b"contenido").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.set_area(area());
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let (_, rect) = c
        .pane_rects(area())
        .into_iter()
        .find(|(id, _)| *id == pane)
        .unwrap();
    let (x, y) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    let client_y = y + 34.0;
    assert!(c
        .capture_external_drop_destination(x, y, client_y)
        .is_none());
    c.set_drag_over_at(Some(pane), Some(client_y));
    let row = c.deep_position_of(pane, &folder).unwrap();
    c.set_drag_over_row(pane, row as i32);
    assert!(c
        .capture_external_drop_destination(x, y, client_y + 1.0)
        .is_none());
    let captured = c.capture_external_drop_destination(x, y, client_y).unwrap();
    assert_eq!(captured.path, folder);
    assert_eq!(captured.panel_path, root.path());
    assert!(captured.deep_folder);
    let a = c
        .deep_job
        .as_ref()
        .unwrap()
        .items
        .iter()
        .position(|(e, _)| e.path == folder)
        .unwrap();
    let b = c
        .deep_job
        .as_ref()
        .unwrap()
        .items
        .iter()
        .position(|(e, _)| e.path == other)
        .unwrap();
    c.deep_job.as_mut().unwrap().items.swap(a, b);
    assert!(c
        .capture_external_drop_destination(x, y, client_y)
        .is_none());
    c.set_drag_over(None);
    assert!(c.external_destination_still_open(&captured));
    assert!(c.queue_selected_external_drop(
        pane,
        captured.path.clone(),
        false,
        true,
        vec![source.clone()],
        None
    ));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
    assert_eq!(c.pending_drop.as_ref().unwrap().dest_dir, folder);
    assert!(c.confirm_pending_drop());
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read(folder.join("documento.txt")).unwrap(),
        b"contenido"
    );
    assert!(source.exists());
    c.navigate_pane_to(pane, origin.path().to_path_buf());
    assert!(!c.external_destination_still_open(&captured));
}

#[test]
fn vista_profunda_drop_interno_acredita_carpeta_y_rechaza_fila_cambiada() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let origin = tempfile::tempdir().unwrap();
    let folder = root.path().join("carpeta");
    let other = root.path().join("otra");
    let source = origin.path().join("documento.txt");
    std::fs::create_dir(&folder).unwrap();
    std::fs::create_dir(&other).unwrap();
    std::fs::write(&source, b"contenido").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.set_area(area());
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let row = c.deep_position_of(pane, &folder).unwrap();
    let (_, rect) = c
        .pane_rects(area())
        .into_iter()
        .find(|(id, _)| *id == pane)
        .unwrap();
    let (x, y) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    let client_y = y + crate::tick::TOP_BAR_H;
    let painted = painted_drop_rows(&mut c, pane);
    assert!(!c.drop_at_from_ole(
        x,
        y,
        client_y,
        Some(&painted),
        false,
        true,
        true,
        vec![source.clone()],
        None
    ));
    let top = client_y - (row as f32 + 0.5) * 22.0;
    let viewport = crate::DropViewport {
        left: rect.x,
        top,
        width: rect.w,
        height: rect.h,
        scroll_y: 0.0,
        row_height: 22.0,
    };
    c.set_drop_viewport(pane, viewport);
    assert!(!c.drop_at_from_ole(
        x,
        y,
        client_y,
        Some(&painted),
        false,
        true,
        true,
        vec![folder.clone()],
        None
    ));
    let other_index = c
        .deep_job
        .as_ref()
        .unwrap()
        .items
        .iter()
        .position(|(e, _)| e.path == other)
        .unwrap();
    let folder_index = c
        .deep_job
        .as_ref()
        .unwrap()
        .items
        .iter()
        .position(|(e, _)| e.path == folder)
        .unwrap();
    c.deep_job
        .as_mut()
        .unwrap()
        .items
        .swap(other_index, folder_index);
    assert!(!c.drop_at_from_ole(
        x,
        y,
        client_y,
        Some(&painted),
        false,
        true,
        true,
        vec![source.clone()],
        None
    ));
    assert!(c.pending_drop.is_none());
    let row = c.deep_position_of(pane, &folder).unwrap();
    let client_y = top + (row as f32 + 0.5) * 22.0;
    let painted = painted_drop_rows(&mut c, pane);
    assert!(c.drop_at_from_ole(
        x,
        client_y - crate::tick::TOP_BAR_H,
        client_y,
        Some(&painted),
        false,
        true,
        true,
        vec![source.clone()],
        None
    ));
    assert_eq!(
        c.wait_drop_collision_probe_for_test(),
        crate::workspace_ctrl::DropProbeEvent::Confirmation
    );
    assert_eq!(c.pending_drop.as_ref().unwrap().dest_dir, folder);
    assert!(c.confirm_pending_drop());
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read(folder.join("documento.txt")).unwrap(),
        b"contenido"
    );
    assert!(source.exists());
}

#[test]
fn vista_profunda_crear_carpeta_protege_enter_y_respeta_seleccion_explicita() {
    use naygo_core::keymap::Action;
    for override_selection in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let cfg = tempfile::tempdir().unwrap();
        let old = root.path().join("anterior");
        let new = root.path().join("nueva");
        std::fs::create_dir(&old).unwrap();
        let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
        assert!(drain(&mut c));
        let pane = c.active_id().unwrap();
        c.deep_start(pane);
        for _ in 0..4000 {
            c.deep_poll();
            if c.deep_job.as_ref().is_some_and(|job| job.done) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let old_pos = c.deep_position_of(pane, &old).unwrap();
        c.deep_select_at(pane, old_pos, false, false);
        c.new_folder_open_active();
        c.new_folder_set_text("nueva");
        c.new_folder_apply();
        assert!(!c.run_action(Action::Activate));
        assert!(c.take_creation_wait_notice());
        if override_selection {
            assert!(c.deep_select_at(pane, old_pos, false, false));
        }
        for _ in 0..4000 {
            if c.ops.pump_ops() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        c.reconcile_completed_operations();
        assert!(drain(&mut c));
        for _ in 0..4000 {
            c.deep_poll();
            if c.deep_job.as_ref().is_some_and(|job| job.done) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let expected = if override_selection { &old } else { &new };
        assert_eq!(
            c.deep_job.as_ref().unwrap().focused_path.as_ref(),
            Some(expected)
        );
        assert!(c.run_action(Action::Activate));
        assert_eq!(c.ws.active_files().unwrap().current_dir, *expected);
    }
}

#[test]
fn vista_profunda_borra_lote_de_distintas_carpetas_y_conserva_navegacion() {
    use naygo_core::keymap::Action;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let one = a.join("uno.txt");
    let two = b.join("dos.txt");
    let keep = root.path().join("conservar.txt");
    std::fs::write(&one, b"uno").unwrap();
    std::fs::write(&two, b"dos").unwrap();
    std::fs::write(&keep, b"intacto").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let parent = c.deep_position_of(pane, &a).unwrap();
    let p1 = c.deep_position_of(pane, &one).unwrap();
    let p2 = c.deep_position_of(pane, &two).unwrap();
    c.deep_select_at(pane, parent, false, false);
    c.deep_select_at(pane, p1, true, false);
    c.op_delete(true);
    assert!(
        c.ops.pending_dialog.is_none(),
        "no reducir un lote con carpeta e hijo"
    );
    assert!(c.take_deep_selection_notice());
    c.deep_select_at(pane, p1, false, false);
    c.deep_select_at(pane, p2, true, false);
    c.op_delete(true);
    let sources = match &c.ops.pending_dialog {
        Some(crate::ops_ctrl::OpDialog::ConfirmDelete {
            sources,
            permanent: true,
        }) => sources,
        _ => panic!("debe confirmar el lote"),
    };
    assert_eq!(sources.len(), 2);
    assert!(sources.contains(&one) && sources.contains(&two));
    assert!(c.ops.delete_confirm());
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    c.reconcile_completed_operations();
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|j| j.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(!one.exists() && !two.exists());
    assert_eq!(std::fs::read(&keep).unwrap(), b"intacto");
    assert_eq!(c.active_id(), Some(pane));
    assert!(c.run_action(Action::MoveDown));
    assert!(c.focused_view_of(pane) >= 0);
}

#[test]
fn vista_profunda_seleccion_por_ruta_reorden_y_filtro_no_entrega_subconjunto() {
    use naygo_core::fs_model::{Entry, EntryKind};
    use naygo_core::keymap::Action;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.config.settings.show_hidden = false;
    c.deep_start(pane);
    let mk = |name: &str, hidden| Entry {
        name: name.into(),
        path: root.path().join(name),
        kind: EntryKind::File,
        size: Some(1),
        modified: None,
        created: None,
        hidden,
        system: false,
    };
    *c.deep_job.as_mut().unwrap().items = vec![
        (mk("a", false), 0),
        (mk("b", false), 0),
        (mk("c", false), 0),
    ];
    assert!(c.deep_select_at(pane, 0, false, false));
    assert!(c.deep_select_at(pane, 2, true, false));
    assert_eq!(
        c.deep_complete_selection(pane).unwrap(),
        vec![root.path().join("a"), root.path().join("c")]
    );
    c.deep_job.as_mut().unwrap().items.swap(0, 1);
    assert_eq!(
        c.deep_complete_selection(pane).unwrap(),
        vec![root.path().join("a"), root.path().join("c")]
    );
    let rows = c.rows_of(pane, 0, std::time::Instant::now());
    assert!(!rows[0].selected && rows[1].selected && rows[2].selected);
    c.open_row_context_menu(pane, 1, 0.0, 0.0);
    assert_eq!(
        c.context_targets(),
        vec![root.path().join("a"), root.path().join("c")]
    );
    assert_eq!(
        c.operation_sources(),
        c.context_targets(),
        "el menú conserva el lote capturado"
    );
    assert_eq!(c.basket_add_selected(), 2);
    assert!(
        c.basket.items().contains(&root.path().join("a"))
            && c.basket.items().contains(&root.path().join("c"))
    );
    let other = c.split_for_target().unwrap();
    c.set_active(other);
    assert!(
        c.context_menu.is_none(),
        "el menú no debe sobrevivir a un cambio de panel"
    );
    assert!(c.context_targets().is_empty());
    c.set_active(pane);
    c.deep_job.as_mut().unwrap().items[2].0.hidden = true;
    assert!(
        c.deep_complete_selection(pane).is_none(),
        "no operar solo sobre a si c quedó oculto"
    );
    c.deep_select_at(pane, 0, true, false);
    assert!(
        c.deep_complete_selection(pane).is_none(),
        "Ctrl no descarta una marca oculta silenciosamente"
    );
    c.deep_job.as_mut().unwrap().items[2].0.hidden = false;
    assert!(c.run_action(Action::SelectAll));
    assert_eq!(c.deep_complete_selection(pane).unwrap().len(), 3);
    c.select_rect_range(pane, 0, 1, false);
    assert_eq!(
        c.deep_complete_selection(pane).unwrap(),
        vec![root.path().join("b"), root.path().join("a")]
    );
    assert!(c.run_action(Action::FocusHome));
    assert!(c.run_action(Action::ExtendDown));
    assert_eq!(
        c.deep_complete_selection(pane).unwrap(),
        vec![root.path().join("b"), root.path().join("a")]
    );
    assert!(c.run_action(Action::FocusDownKeep));
    assert_eq!(c.deep_complete_selection(pane).unwrap().len(), 2);
    assert!(c.run_action(Action::ToggleFocused));
    assert_eq!(c.deep_complete_selection(pane).unwrap().len(), 3);
}

#[test]
fn vista_profunda_salir_invalida_sesion_f2_y_callback_tardio() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let source = root.path().join("archivo.txt");
    std::fs::write(&source, b"original").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let pos = c.deep_position_of(pane, &source).unwrap();
    c.deep_focus_at(pane, pos);
    c.op_rename();
    let request = c.take_rename_request().unwrap();
    c.deep_cancel();
    assert!(!c.rename_session_is_active(pane, request.session));
    assert!(!c.rename_commit(pane, request.session, "otro.txt"));
    assert_eq!(std::fs::read(source).unwrap(), b"original");
    assert!(!root.path().join("otro.txt").exists());
}

#[test]
fn vista_profunda_f2_destino_ocupado_conserva_archivos_y_foco() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let dir = root.path().join("sub");
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("origen.txt");
    std::fs::write(&source, b"origen").unwrap();
    std::fs::write(dir.join("ocupado.txt"), b"existente").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let pos = c.deep_position_of(pane, &source).unwrap();
    c.deep_focus_at(pane, pos);
    c.op_rename();
    let request = c.take_rename_request().unwrap();
    assert!(c.rename_commit(pane, request.session, "ocupado.txt"));
    let mut finished = false;
    for _ in 0..4000 {
        if c.ops.pump_ops() {
            finished = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(finished, "el worker terminó");
    c.reconcile_completed_operations();
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(std::fs::read(&source).unwrap(), b"origen");
    assert_eq!(
        std::fs::read(dir.join("ocupado.txt")).unwrap(),
        b"existente"
    );
    assert_eq!(
        c.deep_job.as_ref().unwrap().focused_path.as_deref(),
        Some(source.as_path())
    );
}

#[test]
fn vista_profunda_f2_renombra_carpeta_anidada_y_relista_sus_hijos() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let folder = root.path().join("padre").join("viejo");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("hijo.txt"), b"contenido").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    for _ in 0..4000 {
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| job.done) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let pos = c.deep_position_of(pane, &folder).expect("carpeta visible");
    assert!(c.deep_focus_at(pane, pos));
    let f2: slint::SharedString = slint::platform::Key::F2.into();
    c.on_key(f2.as_str(), false, false, false);
    let request = c.take_rename_request().expect("F2 profundo abre editor");
    assert_eq!(request.source, folder);
    assert_eq!(
        c.deep_visible_entry(pane, pos + 1).unwrap().path,
        folder.join("hijo.txt")
    );
    assert!(
        c.rename_chain(pane, request.session, "nuevo", 1).is_none(),
        "no abrir un hijo con ruta obsoleta"
    );
    assert!(c.take_rename_request().is_none());
    let renamed = root.path().join("padre").join("nuevo");
    for _ in 0..6000 {
        c.ops.pump_ops();
        c.reconcile_completed_operations();
        c.deep_poll();
        if c.deep_job.as_ref().is_some_and(|job| {
            job.done
                && job
                    .items
                    .iter()
                    .any(|(e, _)| e.path == renamed.join("hijo.txt"))
        }) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        std::fs::read(renamed.join("hijo.txt")).unwrap(),
        b"contenido"
    );
    assert!(!folder.exists());
    assert!(c.is_deep_active(pane));
    assert!(c
        .deep_position_of(pane, &renamed.join("hijo.txt"))
        .is_some());
    assert!(c.deep_position_of(pane, &folder).is_none());
    assert_eq!(
        c.deep_job.as_ref().unwrap().focused_path.as_deref(),
        Some(renamed.as_path())
    );
}

#[test]
fn pie_profundo_invalida_cache_por_entradas_seleccion_y_visibilidad() {
    use naygo_core::fs_model::{Entry, EntryKind};
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.config.settings.footer_enabled = true;
    c.config.settings.show_hidden = false;
    c.deep_start(pane);
    let mk = |name: &str, kind, hidden, size| Entry {
        name: name.into(),
        path: root.path().join(name),
        kind,
        size,
        modified: None,
        created: None,
        hidden,
        system: false,
    };
    *c.deep_job.as_mut().unwrap().items = vec![
        (mk("archivo", EntryKind::File, false, Some(42)), 0),
        (mk("carpeta", EntryKind::Directory, false, None), 0),
        (mk("oculto", EntryKind::File, true, Some(100)), 0),
    ];
    c.footer_text_of(pane);
    let cached = &c
        .deep_job
        .as_ref()
        .unwrap()
        .footer_cache
        .as_ref()
        .unwrap()
        .data;
    assert_eq!(
        (
            cached.total_count,
            cached.file_count,
            cached.dir_count,
            cached.sel_count
        ),
        (2, 1, 1, 0)
    );
    assert!(c.deep_select_at(pane, 0, false, false));
    c.footer_text_of(pane);
    let cached = &c
        .deep_job
        .as_ref()
        .unwrap()
        .footer_cache
        .as_ref()
        .unwrap()
        .data;
    assert_eq!((cached.sel_count, cached.marked_bytes), (1, 42));
    c.deep_job.as_mut().unwrap().items[0].0.size = Some(75);
    c.footer_text_of(pane);
    assert_eq!(
        c.deep_job
            .as_ref()
            .unwrap()
            .footer_cache
            .as_ref()
            .unwrap()
            .data
            .marked_bytes,
        75
    );
    c.config.settings.show_hidden = true;
    c.footer_text_of(pane);
    assert_eq!(
        c.deep_job
            .as_ref()
            .unwrap()
            .footer_cache
            .as_ref()
            .unwrap()
            .data
            .total_count,
        3
    );
    c.restart_deep_listing(None);
    assert!(c.deep_job.as_ref().unwrap().footer_cache.is_none());
}

#[test]
fn firma_profunda_invalida_mutaciones_reemplazos_y_visibilidad() {
    use naygo_core::fs_model::{Entry, EntryKind};
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    let mk = |name: &str| Entry {
        name: name.into(),
        path: root.path().join(name),
        kind: EntryKind::File,
        size: Some(1),
        modified: None,
        created: None,
        hidden: false,
        system: false,
    };
    let now = std::time::Instant::now();
    let sig = |ctrl: &WorkspaceCtrl| ctrl.rows_signature(pane, 0, now).unwrap();
    let empty = sig(&c);
    c.deep_job.as_mut().unwrap().items.push((mk("a"), 0));
    let one = sig(&c);
    assert_ne!(empty, one);
    assert_eq!(one, sig(&c));
    c.deep_job.as_mut().unwrap().items[0].0.size = Some(2);
    let changed = sig(&c);
    assert_ne!(one, changed);
    c.deep_job.as_mut().unwrap().items.push((mk("b"), 1));
    let two = sig(&c);
    c.deep_job.as_mut().unwrap().items.swap(0, 1);
    assert_ne!(two, sig(&c));
    let before_visibility = sig(&c);
    c.config.settings.show_hidden = !c.config.settings.show_hidden;
    assert_ne!(before_visibility, sig(&c));
    let before_replacement = sig(&c);
    let same_items = c.deep_job.as_ref().unwrap().items.to_vec();
    c.deep_job.as_mut().unwrap().items = same_items.into();
    assert_ne!(before_replacement, sig(&c));
}

#[test]
fn vista_profunda_flechas_conservan_seleccion_si_no_hay_filas_visibles() {
    use naygo_core::fs_model::{Entry, EntryKind};
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.config.settings.show_hidden = false;
    c.deep_start(pane);
    let path = root.path().join("oculto");
    let job = c.deep_job.as_mut().unwrap();
    *job.items = vec![(
        Entry {
            name: "oculto".into(),
            path: path.clone(),
            kind: EntryKind::File,
            size: None,
            modified: None,
            created: None,
            hidden: true,
            system: false,
        },
        0,
    )];
    job.focused_path = Some(path.clone());
    job.selected_paths.insert(path.clone());
    let revision = job.selection_revision;
    assert!(!c.deep_select_move(pane, 1, false, false));
    let job = c.deep_job.as_ref().unwrap();
    assert_eq!(job.selection_revision, revision);
    assert_eq!(job.selected_paths.len(), 1);
    assert!(job.selected_paths.contains(&path));
}

#[test]
fn vista_profunda_cursor_sigue_ruta_y_enter_abre_fila_visible() {
    use naygo_core::fs_model::{Entry, EntryKind};
    use naygo_core::keymap::Action;
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.config.settings.show_hidden = false;
    c.deep_start(pane);
    let mk = |path: std::path::PathBuf, hidden| Entry {
        name: path.file_name().unwrap().to_string_lossy().to_string(),
        path,
        kind: EntryKind::Directory,
        size: None,
        modified: None,
        created: None,
        hidden,
        system: false,
    };
    let hidden = mk(root.path().join("hidden"), true);
    *c.deep_job.as_mut().unwrap().items = vec![
        (hidden, 0),
        (mk(a.clone(), false), 0),
        (mk(b.clone(), false), 1),
    ];
    assert!(c.run_action(Action::FocusEnd));
    assert_eq!(c.focused_view_of(pane), 1);
    assert!(c.run_action(Action::FocusHome));
    assert_eq!(c.focused_view_of(pane), 0);
    assert!(c.run_action(Action::MoveDown));
    assert_eq!(c.focused_view_of(pane), 1);
    let before = c.rows_of(pane, 0, std::time::Instant::now());
    assert!(before[1].focused);
    assert!(c.selected_paths().is_empty());
    c.deep_job.as_mut().unwrap().items[2].0.hidden = true;
    assert_eq!(c.focused_view_of(pane), -1);
    assert!(
        !c.run_action(Action::Activate),
        "una ruta filtrada no debe abrir otra fila"
    );
    c.deep_job.as_mut().unwrap().items[2].0.hidden = false;
    c.deep_job.as_mut().unwrap().items.swap(1, 2);
    assert_eq!(
        c.focused_view_of(pane),
        0,
        "el foco conserva la ruta al reordenar"
    );
    assert!(c.rows_of(pane, 0, std::time::Instant::now())[0].focused);
    c.set_area(area());
    assert!(c.run_action(Action::OpenFocusedOtherPane));
    assert!(c
        .ws
        .panes()
        .iter()
        .any(|p| p.id != pane && p.files.as_ref().is_some_and(|f| f.current_dir == b)));
    assert!(c.is_deep_active(pane));
    c.ws.set_active(pane);
    assert!(c.run_action(Action::Activate));
    assert_eq!(c.ws.active_files().unwrap().current_dir, b);
    assert!(!c.is_deep_active(pane));
}

#[test]
fn vista_profunda_no_debe_operar_sobre_seleccion_normal_obsoleta() {
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("normal.txt"), b"original").unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.ws.active_files_mut().unwrap().select_single(0);
    assert_eq!(c.selected_paths(), vec![root.path().join("normal.txt")]);
    c.op_rename();
    let previous = c.take_rename_request().unwrap();
    c.deep_start(pane);
    assert!(c.is_deep_active(pane));
    assert!(!c.rename_session_is_active(pane, previous.session));
    assert!(!c.rename_commit(pane, previous.session, "wrong.txt"));
    assert!(
        c.selected_paths().is_empty(),
        "la selección normal no representa las filas profundas"
    );
    c.op_rename();
    assert!(
        c.take_rename_request().is_none(),
        "F2 no debe elegir un archivo de la lista normal"
    );
    let f2: slint::SharedString = slint::platform::Key::F2.into();
    c.on_key(f2.as_str(), false, false, false);
    assert!(
        c.take_rename_request().is_none(),
        "sin cursor profundo F2 no debe elegir otro archivo"
    );
    c.on_row_clicked(pane, 0, false, false, std::time::Instant::now());
    assert!(
        c.ws.active_files().unwrap().focused.is_none(),
        "el clic profundo no enfoca una fila normal"
    );
    c.op_delete(true);
    assert!(
        c.ops.pending_dialog.is_none(),
        "Supr no debe ofrecer borrar una selección obsoleta"
    );
    c.open_row_context_menu(pane, 0, 0.0, 0.0);
    assert!(c.context_menu.is_none());
    c.set_area(area());
    c.set_drag_over_row(pane, 0);
    let (_, rect) = c
        .pane_rects(area())
        .into_iter()
        .find(|(id, _)| *id == pane)
        .unwrap();
    assert!(!c.drop_at_from_ole(
        rect.x + rect.w / 2.0,
        rect.y + rect.h / 2.0,
        0.0,
        None,
        false,
        true,
        true,
        vec![root.path().join("normal.txt")],
        None
    ));
    assert!(c.pending_drop.is_none());
    assert!(root.path().join("normal.txt").exists());
}

#[test]
fn vista_profunda_doble_clic_usa_indice_visible_filtrado() {
    use naygo_core::fs_model::{Entry, EntryKind};
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let hidden = root.path().join("hidden");
    let visible = root.path().join("visible");
    std::fs::create_dir(&hidden).unwrap();
    std::fs::create_dir(&visible).unwrap();
    let mut c = WorkspaceCtrl::new_in(root.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.config.settings.show_hidden = false;
    let pane = c.active_id().unwrap();
    c.deep_start(pane);
    let mk = |path: PathBuf, hidden| Entry {
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
        path,
        kind: EntryKind::Directory,
        size: None,
        modified: None,
        created: None,
        hidden,
        system: false,
    };
    *c.deep_job.as_mut().unwrap().items =
        vec![(mk(hidden, true), 0), (mk(visible.clone(), false), 0)];
    assert!(c.on_row_middle_clicked(pane, 0));
    assert_eq!(c.ws.active_files().unwrap().current_dir, visible);
    assert!(c.on_row_double_clicked(pane, 0));
    assert_eq!(c.ws.active_files().unwrap().current_dir, visible);
}

/// Verifica el ciclo completo: deep_start → is_deep_active → deep_poll → deep_items →
/// deep_cancel → ya no activo ni con ítems.
#[test]
fn vista_profunda_activa_acumula_y_cancela() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("a.txt"), b"x").unwrap();
    fs::write(root.join("sub/b.txt"), b"y").unwrap();

    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(root.to_path_buf(), cfg.path().to_path_buf());
    let id = c.ws.active_id().unwrap();

    c.deep_start(id);
    assert!(c.is_deep_active(id));
    let mut tries = 0;
    while !c
        .deep_job
        .as_ref()
        .map(|d| d.done || d.cancelled)
        .unwrap_or(true)
        && tries < 2000
    {
        c.deep_poll();
        std::thread::sleep(std::time::Duration::from_millis(2));
        tries += 1;
    }
    c.deep_poll();
    // El árbol tiene 3 entradas: a.txt, sub, sub/b.txt
    assert_eq!(
        c.deep_items().len(),
        3,
        "deben llegar exactamente 3 entradas (a.txt, sub, sub/b.txt)"
    );
    c.deep_cancel();
    assert!(!c.is_deep_active(id));
    assert!(c.deep_items().is_empty());
}

/// `any_modal_open` (keep-alive del timer): en reposo es false; con un modal/overlay del
/// controlador abierto es true; al cerrarlo vuelve a false. Es el predicado que mantiene vivo
/// el bucle de UI mientras hay un popup, para que su hover y primer clic respondan al instante.
#[test]
fn any_modal_open_refleja_los_overlays_del_controlador() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.config.settings.confirm_trash = true;

    // Reposo: sin modales → false (el timer puede dormir como antes; bajo consumo intacto).
    assert!(!c.any_modal_open(), "en reposo no hay modales abiertos");

    // Modal de confirmar borrado (lo reportó el usuario): seleccionar y abrirlo → true.
    let pos = active_pos_of(&c, "a.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    c.op_delete(false);
    assert!(
        c.ops.pending_dialog.is_some(),
        "op_delete abre el diálogo de confirmación"
    );
    assert!(
        c.any_modal_open(),
        "con un OpDialog abierto, el timer sigue vivo"
    );

    // Cerrar el diálogo → vuelve a reposo.
    c.ops.pending_dialog = None;
    assert!(!c.any_modal_open(), "al cerrar el modal vuelve a dormir");

    // El menú contextual (clic derecho) también cuenta: necesita hover vivo.
    c.open_context_menu(c.ws.active_id().unwrap(), 0.0, 0.0);
    assert!(
        c.any_modal_open(),
        "el menú contextual mantiene vivo el timer"
    );
    c.close_context_menu();
    assert!(!c.any_modal_open());

    // La ayuda (F1) cuenta como overlay.
    c.help_open = true;
    assert!(c.any_modal_open(), "la ayuda (F1) mantiene vivo el timer");
    c.help_open = false;
    assert!(!c.any_modal_open());
}

/// Esc cierra el menú que está por encima antes de que la misma tecla afecte el listado bajo él.
#[test]
fn esc_cierra_menu_contextual_y_menu_de_columna() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    c.column_menu_open(id, 0, 10.0, 10.0);
    c.open_context_menu(id, 10.0, 10.0);
    let esc = crate::keys::escape_char().to_string();
    assert!(c.on_key(&esc, false, false, false));
    assert!(c.context_menu.is_none());
    assert!(c.column_menu.is_none());
}

/// Con la confirmación de Papelera desactivada, borrar debe seguir registrando el
/// inverso: antes se lanzaba con `record_undo=false` y Ctrl+Z parecía no hacer nada.
#[test]
fn delete_directo_a_papelera_conserva_el_deshacer() {
    let cfg = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    c.config.settings.confirm_trash = false;
    let pos = active_pos_of(&c, "a.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);

    c.op_delete(false);

    assert!(c.ops.pending_dialog.is_none());
    assert_eq!(c.ops.active_ops.len(), 1);
    assert!(
        c.ops.active_ops[0].plan_record_undo,
        "el borrado directo debe conservar el recibo para Ctrl+Z"
    );
}

#[test]
fn path_is_on_drive_casos() {
    use std::path::Path;
    let on = |p: &str, d: &str| path_is_on_drive(Path::new(p), Path::new(d));
    assert!(on(r"E:\foto", r"E:\"));
    assert!(on(r"E:\a\b\c", r"E:\"));
    assert!(on(r"E:\", r"E:\"));
    assert!(on(r"e:\x", r"E:\"));
    assert!(on(r"E:\x", r"e:\"));
    assert!(!on(r"C:\", r"E:\"));
    assert!(!on(r"C:\foto", r"E:\"));
    assert!(!on(r"EE:\x", r"E:\"));
    assert!(!on(r"\\srv\share\x", r"E:\"));
}

#[test]
fn panes_on_drive_filtra_por_disco() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    // El panel inicial está en `work` (en el disco del tempdir, p.ej. C:).
    // Derivar la letra de ese disco desde la ruta real del tempdir:
    let work_path = work.path().to_path_buf();
    let drive_root = {
        // Raíz del disco del tempdir: "<letra>:\"
        let s = work_path.to_string_lossy();
        let letter = s.chars().next().unwrap();
        std::path::PathBuf::from(format!("{letter}:\\"))
    };
    // panes_on_drive sobre el disco del tempdir => incluye el panel inicial
    let en_disco = c.panes_on_drive(&drive_root);
    assert_eq!(
        en_disco.len(),
        1,
        "el panel inicial está en el disco del tempdir"
    );
    // panes_on_drive sobre un disco que seguro no es el del tempdir => vacío.
    // Elegir una letra distinta a la del tempdir:
    let otra_letra = if drive_root.to_string_lossy().to_uppercase().starts_with('Z') {
        'Y'
    } else {
        'Z'
    };
    let otro_disco = std::path::PathBuf::from(format!("{otra_letra}:\\"));
    let fuera = c.panes_on_drive(&otro_disco);
    assert!(
        fuera.is_empty(),
        "ningún panel está en un disco inexistente"
    );
}

#[test]
fn release_pane_watcher_quita_el_watcher() {
    let work = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    let id = c.ws.active_id().expect("hay un panel activo al arrancar");
    // Waker no-op: no hay UI que despertar en el test.
    let waker: naygo_platform::dir_watch::Waker = std::sync::Arc::new(|| {});
    c.watchers.watch(id.0, work.path().to_path_buf(), waker);
    assert!(
        c.watchers.watched_panes().contains(&id.0),
        "el watcher está activo tras watch()"
    );
    c.release_pane_watcher(id);
    assert!(
        !c.watchers.watched_panes().contains(&id.0),
        "release_pane_watcher debe quitar el watcher del panel"
    );
}

/// Mover el divisor 0 de un split de 3 paneles en fila solo reparte peso entre sus DOS
/// vecinos (paneles 0 y 1); el 3er panel (no vecino de ese divisor) conserva su ancho.
/// Armamos el árbol a mano con 3 paneles Files reales en una fila plana para probar un
/// split con 2+ divisores de forma directa (mismo patrón que el test equivalente en core:
/// `set_divider_solo_afecta_a_los_dos_vecinos` en `crates/core/src/workspace/layout.rs`).
#[test]
fn solo_vecinos_resto_fijo() {
    let tmp = tempfile::tempdir().unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let p0 = c.active_id().unwrap();
    // Dos paneles Files más, reales (creados vía el controlador, no ids inventados).
    let p1 = c.ws.add_pane(PanePurpose::Files, tmp.path().to_path_buf());
    let p2 = c.ws.add_pane(PanePurpose::Files, tmp.path().to_path_buf());
    // Arma directamente una fila de 3 en el MISMO split (2 divisores), reemplazando el
    // layout de a lo sumo 1 hoja que dejó `WorkspaceCtrl::new`.
    c.ws.layout.root = Some(DockNode::Split {
        dir: SplitDir::Horizontal,
        children: vec![DockNode::Leaf(p0), DockNode::Leaf(p1), DockNode::Leaf(p2)],
        weights: vec![1.0, 1.0, 1.0],
    });

    let area = area();
    let w2_antes = c
        .pane_rects(area)
        .into_iter()
        .find(|(id, _)| *id == p2)
        .unwrap()
        .1
        .w;

    let handles = c.split_handles(area);
    assert_eq!(handles.len(), 2, "3 paneles en fila → 2 divisores");
    let h0 = &handles[0];
    assert_eq!(
        h0.divider, 0,
        "primer handle es el divisor 0 del split raíz"
    );

    // Mover el divisor 0 hacia la izquierda (más peso para el panel 1, menos para el 0).
    c.set_divider(&h0.path.clone(), h0.divider, 0.2);

    let rects = c.pane_rects(area);
    let w0 = rects.iter().find(|(id, _)| *id == p0).unwrap().1.w;
    let w1 = rects.iter().find(|(id, _)| *id == p1).unwrap().1.w;
    let w2 = rects.iter().find(|(id, _)| *id == p2).unwrap().1.w;
    assert!(w0 < w1, "el vecino izquierdo del divisor 0 se achica");
    assert!(
        (w2 - w2_antes).abs() < 1.0,
        "el panel 2 (no vecino del divisor 0) conserva su ancho: {w2} vs {w2_antes}"
    );
}

/// Navegación sin chequeo síncrono: el registro en recientes queda DIFERIDO al resultado del
/// listado. Una carpeta buena entra a recientes cuando su listado termina OK; una ruta muerta
/// (no existe) navega igual (el aviso in-place informa) pero NO ensucia recientes.
#[test]
fn reciente_se_deriva_del_resultado_del_listado() {
    let tmp = tempfile::tempdir().unwrap();
    let buena = tmp.path().join("buena");
    std::fs::create_dir(&buena).unwrap();
    let muerta = tmp.path().join("no_existe");
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let antes = c.recents.list().len();

    // Navegar a la muerta: navega (devuelve true) pero tras el listado fallido NO es reciente.
    assert!(c.navigate_active_to(muerta.clone()));
    assert!(drain(&mut c));
    assert_eq!(c.recents.list().len(), antes, "la ruta muerta no entra");
    assert!(
        !c.recents.list().contains(&muerta),
        "la ruta muerta no ensucia recientes"
    );

    // Navegar a la buena: tras el listado exitoso SÍ queda registrada (al frente).
    assert!(c.navigate_active_to(buena.clone()));
    assert!(drain(&mut c));
    assert_eq!(c.recents.list().first(), Some(&buena));
}

/// El aviso "carpeta no encontrada" se calcula en el probe async: navegar a una ruta muerta
/// marca el panel como perdido SOLO tras drenar el probe (nunca con I/O en el hilo de UI).
#[test]
fn navegar_a_ruta_muerta_marca_missing_tras_el_probe() {
    let tmp = tempfile::tempdir().unwrap();
    let muerta = tmp.path().join("no_existe");
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    assert!(c.navigate_active_to(muerta));
    assert!(drain_missing(&mut c));
    assert!(
        c.pane_dir_missing(id),
        "el probe async marca la carpeta perdida"
    );
    // Tiene un ancestro existente (tmp) al que subir.
    assert!(c.pane_has_existing_ancestor(id));
}

/// F5A + metadata pre-resuelta: `apply_watch_events_resolved` usa el Entry que el hilo del
/// watcher ya leyó, en vez de volver a tocar el disco. Se prueba pre-cargando un Entry con un
/// tamaño distinto del real: el panel debe quedar con el del mapa.
#[test]
fn watch_events_usa_la_metadata_preresuelta() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.active_id().unwrap();
    let path = tmp.path().join("a.txt");
    let initial_revision = c.ws.active_files().unwrap().entries_revision();
    // El fixture debe conservar identidad, tipo y visibilidad del listado (como el watcher real)
    // para verificar el delta puntual, y cambiar solo el tamaño marcador.
    let mut prefetched =
        c.ws.active_files()
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .unwrap()
            .clone();
    prefetched.size = Some(999);
    let mut resolved = std::collections::HashMap::new();
    resolved.insert(path.clone(), Some(prefetched));
    c.apply_watch_events_resolved(
        id,
        &[naygo_core::listing::DirEvent::Modified(path.clone())],
        &resolved,
    );
    let f = c.ws.active_files().unwrap();
    let entry = f.entries.iter().find(|e| e.path == path).unwrap();
    assert_eq!(entry.size, Some(999), "usa la metadata pre-resuelta");
    assert_eq!(
        f.entries_rows_changed_since(initial_revision),
        Some(vec![path]),
        "un Modified con identidad y visibilidad estables habilita el refresco puntual"
    );
}

/// Regresión del bug "tipear `_`/`!`/`#` en el editor de rename REEMPLAZA todo el texto":
/// las teclas tipeadas en el editor inline se filtraban al despachador global (`on_key`),
/// donde el typeahead movía la selección → el sync re-montaba la fila y el editor volvía
/// con el nombre original seleccionado. La guarda: con `rename_active` abierto, `on_key`
/// es no-op (la vía principal es el gate del FilePanel en .slint; esta es la red en Rust).
#[test]
fn on_key_con_rename_activo_es_noop_y_no_gatilla_typeahead() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for n in ["alpha.txt", "_zeta.txt"] {
        std::fs::write(dir.path().join(n), b"x").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();
    // Enfocar/seleccionar la fila 0 y abrir la sesión de rename sobre ella.
    c.on_row_clicked(id, 0, false, false, std::time::Instant::now());
    c.op_rename();
    assert!(
        c.rename_active.is_some(),
        "la sesión de rename quedó abierta"
    );
    // Teclear "_" con Shift (como en el layout latino): antes del fix esto corría el
    // typeahead y seleccionaba "_zeta.txt". Ahora on_key es no-op durante el rename.
    let handled = c.on_key("_", false, true, false);
    assert!(
        !handled,
        "on_key ignora la tecla mientras el editor de rename está abierto"
    );
    assert!(
        c.typeahead.is_empty(),
        "el typeahead NO corrió durante el rename"
    );
    let f = c.ws.active_files().unwrap();
    assert!(
        f.selected.len() == 1 && f.selected.contains(&0),
        "la selección no se movió: sigue solo la fila 0 (renombrada), {:?}",
        f.selected
    );
    // Y la sesión sigue viva: nada la cerró por debajo.
    assert!(c.rename_active.is_some());
}

#[test]
fn cerrar_panel_de_rename_cancela_sesion_y_libera_teclado() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let original_path = dir.path().join("alpha.txt");
    std::fs::write(&original_path, b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let original = c.ws.active_id().unwrap();
    c.on_row_clicked(original, 0, false, false, std::time::Instant::now());
    c.op_rename();
    let request = c.take_rename_request().unwrap();
    c.add_pane_split_dir(naygo_core::workspace::layout::SplitDir::Horizontal, false);
    let other = c.ws.active_id().unwrap();
    assert_ne!(original, other);
    c.close_pane(original);
    assert!(c.ws.pane(original).is_none());
    assert!(c.rename_active.is_none());
    assert!(!c.rename_commit(original, request.session, "changed.txt"));
    c.on_key("x", false, false, false);
    assert_eq!(
        c.typeahead, "x",
        "el panel restante vuelve a recibir teclas"
    );
    assert!(original_path.exists());
    assert!(!dir.path().join("changed.txt").exists());
}

#[test]
fn rename_pendiente_en_otro_panel_permite_navegar_y_preserva_sesion() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for name in ["alpha.txt", "beta.txt"] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.active_id().unwrap();
    c.on_row_clicked(first, 0, false, false, std::time::Instant::now());
    c.op_rename();
    let request = c.take_rename_request().unwrap();
    c.add_pane_split_dir(naygo_core::workspace::layout::SplitDir::Horizontal, false);
    let second = c.active_id().unwrap();
    assert!(drain(&mut c));
    c.on_row_clicked(second, 0, false, false, std::time::Instant::now());
    let down: slint::SharedString = slint::platform::Key::DownArrow.into();
    c.on_key(down.as_str(), false, false, false);
    assert_eq!(
        c.ws.active_files().unwrap().focused,
        Some(1),
        "flecha navega en el otro panel"
    );
    assert!(c.rename_session_is_active(first, request.session));
    c.op_rename();
    assert_eq!(c.active_id(), Some(first), "F2 vuelve al editor pendiente");
    assert!(c.rename_session_is_active(first, request.session));
    assert!(
        c.take_rename_request().is_none(),
        "el borrador no se reinicializa"
    );
}

#[test]
fn f2_en_otra_pestana_revela_pestana_con_rename_pendiente() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("alpha.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().join("settings"));
    assert!(drain(&mut c));
    let original = c.active_id().unwrap();
    c.ws.active_files_mut().unwrap().select_single(0);
    c.op_rename();
    let session = c.take_rename_request().unwrap().session;
    c.add_pane_split_dir(naygo_core::workspace::layout::SplitDir::Horizontal, false);
    let other = c.active_id().unwrap();
    c.stack_into(other, original);
    c.op_rename();
    assert_eq!(c.active_id(), Some(original));
    let groups = c.tab_groups();
    assert!(
        groups
            .iter()
            .any(|(members, index)| members.get(*index) == Some(&original)),
        "el panel del editor debe ser la pestaña visible"
    );
    assert!(c.rename_session_is_active(original, session));
}

#[test]
fn navegar_panel_de_rename_cancela_sesion_pero_refrescar_la_conserva() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("alpha.txt"), b"x").unwrap();
    let dest = tmp.path().join("destino");
    std::fs::create_dir(&dest).unwrap();
    let mut c = WorkspaceCtrl::new_in(tmp.path().to_path_buf(), tmp.path().join("settings"));
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    let pos = active_pos_of(&c, "alpha.txt").unwrap();
    c.ws.active_files_mut().unwrap().select_single(pos);
    c.op_rename();
    let req = c.take_rename_request().unwrap();
    c.start_listing(pane, tmp.path().to_path_buf());
    assert!(
        c.rename_session_is_active(pane, req.session),
        "refresh conserva edición"
    );
    c.open_in_pane(pane, dest);
    assert!(
        c.rename_active.is_none(),
        "navegar cancela la edición anterior"
    );
    assert!(!c.rename_commit(pane, req.session, "changed.txt"));
    assert!(tmp.path().join("alpha.txt").exists());
    assert!(!tmp.path().join("changed.txt").exists());
}

#[test]
fn rename_encadenado_sin_fuente_visible_no_adivina_siguiente_fila() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for name in ["alpha.txt", "beta.txt"] {
        std::fs::write(dir.path().join(name), b"x").unwrap();
    }
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    let pane = c.active_id().unwrap();
    c.ws.active_files_mut().unwrap().select_single(0);
    c.op_rename();
    let request = c.take_rename_request().unwrap();
    let files = c.ws.active_files_mut().unwrap();
    files.entries.retain(|entry| entry.path != request.source);
    files.entries_changed();
    assert_eq!(
        c.rename_chain(pane, request.session, &request.name, 1),
        None
    );
    assert!(c.rename_active.is_none());
    assert!(c.take_rename_request().is_none());
    assert!(request.source.exists());
    assert!(dir.path().join("beta.txt").exists());
}

/// Filtro visual por tipeo: tipear marca (solo visual) las filas que CONTIENEN el texto
/// (case/acento-insensible), el salto de foco cae al primer "contiene" si no hay prefijo,
/// el footer anuncia el filtro con el conteo, Esc lo limpia primero que nada, y navegar a
/// otra carpeta también (un refresh de la misma carpeta lo conserva).
#[test]
fn filtro_por_tipeo_marca_salta_persiste_y_se_limpia() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("alpha.txt"), b"x").unwrap();
    std::fs::write(dir.path().join("mi beta final.txt"), b"x").unwrap();
    std::fs::create_dir(dir.path().join("betacam")).unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    c.config.set_language(naygo_core::i18n::LangId::new("es"));
    assert!(drain(&mut c));
    let id = c.ws.active_id().unwrap();

    // Tipear "beta": ningún nombre EMPIEZA con "beta"... salvo la carpeta "betacam" (prefijo).
    for ch in ["b", "e", "t", "a"] {
        c.on_key(ch, false, false, false);
    }
    assert!(c.filter_active(), "el buffer de tipeo activó el filtro");
    // Salto: gana el prefijo ("betacam"), no el "contiene" ("mi beta final.txt").
    let f = c.ws.active_files().unwrap();
    let focused_name = f
        .focused
        .and_then(|p| f.view_entry_at(p))
        .map(|e| e.name.clone());
    assert_eq!(focused_name.as_deref(), Some("betacam"));

    // Marcado visual: rows_of marca los dos que contienen "beta" (archivo + carpeta).
    let rows = c.rows_of(id, 0, std::time::Instant::now());
    let marked: Vec<&str> = rows
        .iter()
        .filter(|r| r.filter_match)
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(marked.len(), 2, "marca archivo y carpeta: {marked:?}");
    assert_eq!(c.filter_match_count, 2);

    // Footer: anuncia el filtro con el buffer y el conteo (i18n es).
    let footer = c.footer_text_of(id);
    assert!(
        footer.contains("filtro: \"beta\"") && footer.contains("2 coincidencias"),
        "footer con filtro y conteo: {footer}"
    );

    // Un refresh de la MISMA carpeta CONSERVA el filtro.
    c.refresh_active();
    assert!(c.filter_active(), "refresh no limpia el filtro");
    assert!(drain(&mut c));

    // Esc limpia el filtro PRIMERO (antes que cerrar búsqueda o cancelar listado).
    let esc = crate::keys::escape_char().to_string();
    c.on_key(&esc, false, false, false);
    assert!(!c.filter_active(), "Esc limpió el filtro");
    let rows = c.rows_of(id, 0, std::time::Instant::now());
    assert!(rows.iter().all(|r| !r.filter_match), "sin tinte tras Esc");

    // Navegar a OTRA carpeta limpia el filtro.
    for ch in ["a", "l"] {
        c.on_key(ch, false, false, false);
    }
    assert!(c.filter_active());
    let sub = dir.path().join("betacam");
    c.navigate_pane_to(id, sub);
    assert!(!c.filter_active(), "navegar limpió el filtro");
}

/// El salto de foco del typeahead: si ningún nombre empieza con la aguja, cae al primer
/// nombre que la CONTIENE (en orden de vista). Case/acento-insensible.
#[test]
fn typeahead_salta_por_contiene_si_no_hay_prefijo() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("alpha.txt"), b"x").unwrap();
    std::fs::write(dir.path().join("mi canción favorita.mp3"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    // "cancion" sin tilde: ningún nombre empieza así; "mi canción favorita.mp3" la contiene.
    for ch in ["c", "a", "n", "c", "i", "o", "n"] {
        c.on_key(ch, false, false, false);
    }
    let f = c.ws.active_files().unwrap();
    let focused_name = f
        .focused
        .and_then(|p| f.view_entry_at(p))
        .map(|e| e.name.clone());
    assert_eq!(
        focused_name.as_deref(),
        Some("mi canción favorita.mp3"),
        "salta al contiene, sin tilde y sin prefijo"
    );
}

/// Char unicode de una tecla especial de Slint como String (mismo helper que tests_simular).
fn key_str(k: slint::platform::Key) -> String {
    let s: slint::SharedString = k.into();
    s.to_string()
}

/// Con el filtro visual activo, ↓/↑ recorren las COINCIDENCIAS (no de a una fila), con
/// wrap-around. Pedido del usuario: Tab sigue para paneles; las flechas ciclan matches.
#[test]
fn flechas_ciclan_coincidencias_del_filtro() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    // Solo archivos (sin carpetas) para un orden de vista alfabético predecible:
    // "aa match uno", "bb", "cc match dos", "dd".
    std::fs::write(dir.path().join("aa match uno.txt"), b"x").unwrap();
    std::fs::write(dir.path().join("bb.txt"), b"x").unwrap();
    std::fs::write(dir.path().join("cc match dos.txt"), b"x").unwrap();
    std::fs::write(dir.path().join("dd.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new_in(dir.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(drain(&mut c));
    for ch in ["m", "a", "t", "c", "h"] {
        c.on_key(ch, false, false, false);
    }
    assert!(c.filter_active());
    let focused_name = |c: &WorkspaceCtrl| -> String {
        let f = c.ws.active_files().unwrap();
        f.focused
            .and_then(|p| f.view_entry_at(p))
            .map(|e| e.name.clone())
            .unwrap_or_default()
    };
    // El salto inicial va a la PRIMERA aparición en la vista.
    assert_eq!(focused_name(&c), "aa match uno.txt");
    let down = key_str(slint::platform::Key::DownArrow);
    let up = key_str(slint::platform::Key::UpArrow);
    // ↓ salta a la siguiente coincidencia (se salta "bb.txt").
    c.on_key(&down, false, false, false);
    assert_eq!(focused_name(&c), "cc match dos.txt");
    // ↓ desde la última envuelve a la primera.
    c.on_key(&down, false, false, false);
    assert_eq!(focused_name(&c), "aa match uno.txt");
    // ↑ vuelve a la anterior (también con wrap).
    c.on_key(&up, false, false, false);
    assert_eq!(focused_name(&c), "cc match dos.txt");
    // Sin filtro, ↓ vuelve a moverse de a una fila.
    let esc = crate::keys::escape_char().to_string();
    c.on_key(&esc, false, false, false);
    assert!(!c.filter_active());
    c.on_key(&down, false, false, false);
    assert_eq!(
        focused_name(&c),
        "dd.txt",
        "sin filtro, ↓ avanza una fila normal"
    );
}

#[test]
fn carga_de_sesion_tardia_no_pisa_navegacion_del_usuario() {
    let initial_dir = tempfile::tempdir().unwrap();
    let user_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let saved_config_dir = tempfile::tempdir().unwrap();
    let mut ctrl = WorkspaceCtrl::new_in(
        initial_dir.path().to_path_buf(),
        config_dir.path().to_path_buf(),
    );
    ctrl.apply_first_run_layout();
    ctrl.mark_startup_session_baseline();

    let saved = WorkspaceCtrl::new_in(
        initial_dir.path().to_path_buf(),
        saved_config_dir.path().to_path_buf(),
    )
    .session_persist();
    assert!(ctrl.navigate_active_to(user_dir.path().to_path_buf()));

    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    ctrl.startup_load_rx = Some(rx);
    tx.send(Ok(crate::startup_load::StartupData {
        settings: naygo_core::config::Settings::default(),
        i18n: naygo_core::i18n::I18n::embedded(&naygo_core::i18n::LangId::new("es")),
        keymap: naygo_core::keymap::KeyMap::defaults(),
        workspace: Some(saved),
        favorites: Favorites::default(),
        templates: naygo_core::workspace::TemplateStore::default(),
        first_launch_save_ticket: None,
    }))
    .unwrap();

    assert!(ctrl.pump_startup_load());
    assert_eq!(
        ctrl.ws.active_files().unwrap().current_dir,
        user_dir.path(),
        "el snapshot viejo no reemplaza la navegación que ocurrió mientras esperaba"
    );
}
