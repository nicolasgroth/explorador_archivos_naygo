// Naygo — WorkspaceCtrl: pruebas unitarias del controlador.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::*;

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
    c.column_toggle(id, crate::bridge::column_kind_to_int(naygo_core::columns::ColumnKind::Created));
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
    }
    let s_mod = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s_pre_mod, s_mod, "mutar el tamaño de una entry cambia la firma");

    // Marcar una ruta como CORTADA cambia la firma (la fila se atenúa).
    let s_pre_cut = c.rows_signature(id, secs, now).unwrap();
    c.ops.set_cut(&[work.path().join("a.txt")]);
    let s_cut = c.rows_signature(id, secs, now).unwrap();
    assert_ne!(s_pre_cut, s_cut, "cortar una ruta cambia la firma");

    // Resaltado FRESCO vigente → None (no cachear: el fundido cambia cada tick).
    c.watchers
        .mark_fresh(id.0, vec![work.path().join("a.txt")], now);
    assert!(
        c.rows_signature(id, secs, now).is_none(),
        "con una fila fresca vigente, la firma es None (no cachear)"
    );
    // Pasado el tiempo de resaltado, vuelve a ser Some (cacheable de nuevo).
    let later = now + std::time::Duration::from_secs(secs + 1);
    assert!(
        c.rows_signature(id, secs, later).is_some(),
        "vencido el resaltado, vuelve a ser cacheable"
    );
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
    c.add_pane_split();
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
    c.add_pane_split();
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
    // bajo el cursor), no la elección mover/copiar (eso ya lo cubre dnd::decide_drop_action).
    let routed = c.drop_at(
        cx,
        cy,
        true,  // ctrl → copiar
        false, // shift
        vec![a.path().join("doc.txt")],
        false, // move_hint del OLE (sin Shift al soltar)
    );
    assert!(routed, "drop_at debe enrutar (no caer al fallback)");
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
    c.ws.pane_mut(pane_a).and_then(|p| p.files.as_mut()).unwrap().selected = vec![0];
    c.ws.pane_mut(pane_b).and_then(|p| p.files.as_mut()).unwrap().selected = vec![0];
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
    // ctrl=false, shift=false (estado de la app stale durante el modal), PERO move_hint=true
    // (Shift REAL al soltar, reportado por el OLE). Debe MOVER → el original desaparece.
    let routed = c.drop_at(cx, cy, false, false, vec![a.path().join("doc.txt")], true);
    assert!(routed, "drop_at debe enrutar");
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
    // ctrl=false, shift=false, move_hint=false → la decisión depende del disco. Mismo disco → Mover.
    let routed = c.drop_at(cx, cy, false, false, vec![a.path().join("doc.txt")], false);
    assert!(routed, "drop_at debe enrutar");
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
    // Soltar (Ctrl=copia para que el dato sea determinista).
    let routed = c.drop_at(cx, cy, true, false, vec![a.path().join("doc.txt")], false);
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
    c.save_current_template("Mi setup");
    assert!(c
        .layout_templates()
        .iter()
        .any(|(n, builtin)| n == "Mi setup" && !builtin));
    // Persistió: un controlador nuevo (mismo config_dir) la ve.
    let c2 = WorkspaceCtrl::new_in(work.path().to_path_buf(), cfg.path().to_path_buf());
    assert!(c2.templates.user.iter().any(|t| t.name == "Mi setup"));

    // Borrarla.
    c.delete_template("Mi setup");
    assert!(!c.layout_templates().iter().any(|(n, _)| n == "Mi setup"));
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
    c1.add_pane_split();
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
    let nuevas =
        c.apply_watch_events(id, &[naygo_core::listing::DirEvent::Created(nuevo.clone())]);
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
    assert_eq!(req.0, id);
    assert_eq!(req.1, pos);
    assert_eq!(req.2, 0, "etapa inicial = nombre sin extensión");
    // Confirmar el rename y bombear la op hasta completarla (mismo patrón que ops_ctrl).
    assert!(c.rename_commit(id, pos, "renombrado.txt"));
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
    // Re-listar para reflejar el rename antes de seguir.
    c.start_listing(id, tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let pos_b = active_pos_of(&c, "b.txt").expect("b.txt visible");
    let next = c.rename_chain(id, pos_b, "b.txt", 1);
    assert!(next.is_some(), "encadena a una fila válida");
    let req2 = c.take_rename_request().expect("chain reabre el editor");
    assert_eq!(req2.2, 0, "el chain selecciona el nombre sin extensión");
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
    c.add_pane_split(); // segundo Files, queda activo
    let second = c.active_id().unwrap();
    assert_ne!(first, second);
    // Agregar un Árbol y activarlo (simula clic en el panel Carpetas).
    c.add_pane_of(PanePurpose::Tree);
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
    assert!(!c.pane_dir_missing(id), "al inicio la carpeta existe");
    std::fs::remove_dir_all(&sub).unwrap(); // "sacar el USB"
    // El estado "missing" se cachea y se recalcula en eventos reales (aquí simulamos el
    // evento de cambio de discos / tick que dispara el refresco).
    c.refresh_missing_cache();
    assert!(
        c.pane_dir_missing(id),
        "el panel detecta su carpeta perdida"
    );
    assert_eq!(c.ws.active_files().unwrap().current_dir, sub);
    // "Subir al ancestro existente": el panel queda en tmp (el padre que sigue vivo).
    c.missing_folder_go_ancestor(id);
    assert!(!c.pane_dir_missing(id), "ya no está perdida tras subir");
    assert_eq!(c.ws.active_files().unwrap().current_dir, tmp.path());
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
    assert!(!c.on_row_clicked(id, pos, base));
    assert!(!c.on_row_clicked(id, pos, base + Duration::from_millis(900)));
    assert_eq!(c.path_of(id), tmp.path().display().to_string());

    // Dos clics RÁPIDOS (dentro de 700 ms) SÍ navegan. Siguen la misma línea de tiempo.
    let t1 = base + Duration::from_secs(5);
    assert!(!c.on_row_clicked(id, pos, t1), "1er clic: selecciona");
    assert!(
        c.on_row_clicked(id, pos, t1 + Duration::from_millis(150)),
        "2do clic rápido: doble-clic → navega"
    );
    assert!(drain(&mut c));
    let rows = c.rows_of(c.active_id().unwrap(), 8, std::time::Instant::now());
    assert!(rows.iter().any(|r| r.name == "dentro.txt"));
}

/// Agregar un panel divide el layout y deja DOS paneles Files; el nuevo queda activo.
#[test]
fn agregar_panel_divide_y_deja_dos() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
    let mut c = WorkspaceCtrl::new(tmp.path().to_path_buf());
    assert!(drain(&mut c));
    let first = c.active_id().unwrap();
    c.add_pane_split();
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
    c.add_pane_of(PanePurpose::Tree);
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
    c.add_pane_split();
    assert!(drain(&mut c));
    let b = c.active_id().unwrap();
    assert_eq!(c.resolve_target(b, area()), PaneTarget::Direct(a));
    // Tres paneles → selector (Pick con 2 candidatos).
    c.add_pane_split();
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
    c.add_pane_split();
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
    c.add_pane_split();
    assert!(drain(&mut c));
    c.add_pane_split();
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
    c.add_pane_split();
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
    c.add_pane_split();
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
    c.add_pane_split();
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
    c.add_pane_split();
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
    c.add_pane_split();
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
    c.add_pane_split();
    assert!(drain(&mut c));
    c.add_pane_split();
    assert!(drain(&mut c));
    let origin = c.active_id().unwrap();
    c.request_action(PaneAction::Clone, origin, area());
    assert!(c.pending_pick.is_some());
    c.pick_cancel();
    assert!(c.pending_pick.is_none());
}

/// Vista profunda: activar sobre un árbol temporal, drenar hasta completar y cancelar.
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
    assert_eq!(en_disco.len(), 1, "el panel inicial está en el disco del tempdir");
    // panes_on_drive sobre un disco que seguro no es el del tempdir => vacío.
    // Elegir una letra distinta a la del tempdir:
    let otra_letra = if drive_root.to_string_lossy().to_uppercase().starts_with('Z') { 'Y' } else { 'Z' };
    let otro_disco = std::path::PathBuf::from(format!("{otra_letra}:\\"));
    let fuera = c.panes_on_drive(&otro_disco);
    assert!(fuera.is_empty(), "ningún panel está en un disco inexistente");
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
