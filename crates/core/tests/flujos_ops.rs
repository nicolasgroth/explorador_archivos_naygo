// Naygo — pruebas de regresión de FLUJOS de operaciones de archivo (end-to-end por disco).
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
//
//! Suite de regresión de los flujos de operaciones de archivo del núcleo, ejercitados de
//! PUNTA A PUNTA: se arma el `OpRequest`, se planifica (`plan`), se ejecuta por el motor
//! (`run_plan` o el `spawn` threaded) y se verifica el ESTADO REAL EN DISCO (no solo flags
//! ni el plan). Cubre lo que más se ha roto entre entregas: copiar/mover archivos y carpetas
//! recursivas, crear carpetas anidadas y archivos, borrar permanente, renombrar en lote,
//! decisiones de conflicto (overwrite/skip/rename/rename-to) y el ciclo deshacer.
//!
//! Estos tests viven aquí (integration test, fuera de los módulos) a propósito: ejercitan SOLO
//! la API pública de `naygo_core::ops`, agrupan los flujos compuestos en un único lugar
//! descubrible y NO duplican los tests de unidad ya existentes en `ops/*` (que prueban el plan,
//! las guardas de seguridad y los casos borde de cada función por separado).
//!
//! Papelera: el borrado a papelera lo hace `platform` (Shell API), no `core`. Aquí solo se
//! ejercita el borrado PERMANENTE (`to_trash:false`) y se deja anotado dónde termina el alcance.

use naygo_core::cancel::CancellationToken;
use naygo_core::ops::{
    self, plan, run_plan, spawn, ConflictDecision, ConflictPolicy, OpKind, OpMsg, OpOutcome,
    OpRequest, OpSummary,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Ejecuta un `OpRequest` por el motor SÍNCRONO (`run_plan`) con la política del request y
/// devuelve los mensajes emitidos + el resumen. Es el camino corto para los flujos donde la UI
/// ya resolvió cualquier conflicto antes de spawnear.
fn ejecutar(req: &OpRequest) -> (Vec<OpMsg>, OpSummary) {
    let p = plan(req).expect("el plan debe armarse");
    let token = CancellationToken::new();
    let (tx, rx) = mpsc::channel();
    let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
    let summary = run_plan(&p, &req.kind, req.conflict, &token, &tx, &crx, None);
    drop(tx);
    (rx.into_iter().collect(), summary)
}

/// Lee el contenido de un archivo como bytes (panic con ruta si falla: el mensaje del test lo
/// deja claro). Azúcar para los asertos de estado final.
fn leer(p: &Path) -> Vec<u8> {
    fs::read(p).unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", p.display()))
}

// ───────────────────────────── COPIAR ─────────────────────────────

#[test]
fn copiar_archivo_deja_origen_y_destino_identicos() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("nota.txt");
    fs::write(&src, b"contenido original").unwrap();
    let dst = dir.path().join("destino");
    fs::create_dir(&dst).unwrap();

    let req = ops::transfer(false, vec![src.clone()], dst.clone());
    let (_m, summary) = ejecutar(&req);

    // Copia (no mueve): origen Y destino existen con el mismo contenido.
    assert!(src.exists(), "el origen sigue (copiar no borra)");
    assert_eq!(leer(&dst.join("nota.txt")), b"contenido original");
    assert_eq!(summary.count_done(), 1);
    assert_eq!(summary.count_failed(), 0);
}

#[test]
fn copiar_carpeta_recursiva_recrea_todo_el_arbol() {
    // Flujo crítico: copiar una carpeta con subcarpetas anidadas debe recrear la jerarquía
    // COMPLETA en el destino, archivo por archivo, con el contenido intacto.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("proyecto");
    fs::create_dir_all(src.join("sub/profundo")).unwrap();
    fs::write(src.join("raiz.txt"), b"r").unwrap();
    fs::write(src.join("sub/medio.txt"), b"m").unwrap();
    fs::write(src.join("sub/profundo/hoja.txt"), b"h").unwrap();
    let dst = dir.path().join("salida");
    fs::create_dir(&dst).unwrap();

    let req = ops::transfer(false, vec![src.clone()], dst.clone());
    let (_m, summary) = ejecutar(&req);

    let base = dst.join("proyecto");
    assert_eq!(leer(&base.join("raiz.txt")), b"r");
    assert_eq!(leer(&base.join("sub/medio.txt")), b"m");
    assert_eq!(
        leer(&base.join("sub/profundo/hoja.txt")),
        b"h",
        "hoja anidada"
    );
    // El summary registra un ítem Done por CADA paso ejecutado: las 3 carpetas (proyecto, sub,
    // profundo) + los 3 archivos = 6. (Las carpetas no suman a `total_files` del plan, pero sí
    // generan un paso Done.) Lo importante es que NADA falló.
    assert_eq!(summary.count_done(), 6, "3 carpetas + 3 archivos");
    assert_eq!(summary.count_failed(), 0, "sin fallos");
    // El origen quedó intacto (no se movió).
    assert!(src.join("sub/profundo/hoja.txt").exists());
}

#[test]
fn copiar_varios_origenes_a_la_vez() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"aaa").unwrap();
    fs::write(&b, b"bbb").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();

    let req = ops::transfer(false, vec![a, b], dst.clone());
    let (_m, summary) = ejecutar(&req);

    assert_eq!(leer(&dst.join("a.txt")), b"aaa");
    assert_eq!(leer(&dst.join("b.txt")), b"bbb");
    assert_eq!(summary.count_done(), 2);
}

// ───────────────────────────── MOVER ─────────────────────────────

#[test]
fn mover_archivo_quita_el_origen() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("mueve.txt");
    fs::write(&src, b"datos").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();

    let req = ops::transfer(true, vec![src.clone()], dst.clone()); // is_move = true
    assert_eq!(req.kind, OpKind::Move, "transfer(true) arma un Move");
    let (_m, summary) = ejecutar(&req);

    assert!(!src.exists(), "mover BORRA el origen");
    assert_eq!(leer(&dst.join("mueve.txt")), b"datos");
    assert_eq!(summary.count_done(), 1);
}

#[test]
fn mover_carpeta_recursiva_traslada_los_archivos_al_destino() {
    // Mover una carpeta con subcarpetas: el árbol COMPLETO aparece en el destino con su contenido,
    // y los ARCHIVOS de origen ya no están en su lugar viejo.
    //
    // Nota de contrato (importante para no romper este test al evolucionar el motor): `run_plan`
    // mueve archivo por archivo (un `fs::rename` por archivo) y NO emite un paso para borrar el
    // esqueleto de carpetas de origen ya vacías. Por eso, a nivel del MOTOR puro, las carpetas de
    // origen quedan vacías tras el move (los archivos sí se fueron). La limpieza de ese esqueleto,
    // si se desea, es responsabilidad de una capa superior, no del motor. Este test fija ese
    // comportamiento real en vez de asumir un borrado que el motor no hace.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("carpeta");
    fs::create_dir_all(src.join("sub")).unwrap();
    fs::write(src.join("a.txt"), b"a").unwrap();
    fs::write(src.join("sub/b.txt"), b"b").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();

    let req = ops::transfer(true, vec![src.clone()], dst.clone());
    let (_m, summary) = ejecutar(&req);

    // Destino: árbol completo con contenido.
    let base = dst.join("carpeta");
    assert_eq!(leer(&base.join("a.txt")), b"a");
    assert_eq!(leer(&base.join("sub/b.txt")), b"b");
    // Origen: los ARCHIVOS se fueron (movidos), no quedaron copias.
    assert!(!src.join("a.txt").exists(), "el archivo raíz se movió");
    assert!(
        !src.join("sub/b.txt").exists(),
        "el archivo anidado se movió"
    );
    assert_eq!(summary.count_failed(), 0, "sin fallos");
}

// ──────────────────────── BORRAR (permanente) ────────────────────────

#[test]
fn borrar_permanente_archivo_lo_elimina() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("borrame.txt");
    fs::write(&f, b"x").unwrap();

    // to_trash:false → borrado PERMANENTE (lo que ejecuta el motor; la papelera la hace platform).
    let req = ops::delete(vec![f.clone()], false);
    assert_eq!(req.kind, OpKind::Delete { to_trash: false });
    let (_m, summary) = ejecutar(&req);

    assert!(!f.exists(), "el archivo se borró");
    assert_eq!(summary.count_done(), 1);
}

#[test]
fn borrar_permanente_carpeta_recursiva() {
    // Borrar una carpeta poblada con subcarpetas debe eliminar el árbol entero.
    let dir = tempfile::tempdir().unwrap();
    let carpeta = dir.path().join("a_borrar");
    fs::create_dir_all(carpeta.join("sub")).unwrap();
    fs::write(carpeta.join("x.txt"), b"x").unwrap();
    fs::write(carpeta.join("sub/y.txt"), b"y").unwrap();

    let req = ops::delete(vec![carpeta.clone()], false);
    let (_m, _s) = ejecutar(&req);

    assert!(!carpeta.exists(), "la carpeta y su contenido se borraron");
}

// ──────────────────────── RENOMBRAR / BATCH ────────────────────────

#[test]
fn renombrar_archivo_cambia_el_nombre_en_disco() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("viejo.txt");
    fs::write(&src, b"contenido").unwrap();

    let req = ops::rename(src.clone(), "nuevo.txt".to_string());
    let (_m, summary) = ejecutar(&req);

    assert!(!src.exists(), "el nombre viejo ya no está");
    let nuevo = dir.path().join("nuevo.txt");
    assert_eq!(leer(&nuevo), b"contenido", "mismo contenido, nuevo nombre");
    assert_eq!(summary.count_done(), 1);
}

#[test]
fn batch_rename_aplica_todos_los_nombres_en_disco() {
    // El plan de batch-rename ya está testeado (orden por dependencia, ciclos). Aquí se ejercita
    // la EJECUCIÓN: tras correr por el motor, los archivos quedan renombrados en disco.
    let dir = tempfile::tempdir().unwrap();
    let f1 = dir.path().join("uno.txt");
    let f2 = dir.path().join("dos.txt");
    let f3 = dir.path().join("tres.txt");
    fs::write(&f1, b"1").unwrap();
    fs::write(&f2, b"2").unwrap();
    fs::write(&f3, b"3").unwrap();

    let req = ops::batch_rename(
        vec![f1.clone(), f2.clone(), f3.clone()],
        vec![
            "a_001.txt".to_string(),
            "a_002.txt".to_string(),
            "a_003.txt".to_string(),
        ],
    );
    assert!(
        matches!(req.kind, OpKind::BatchRename { .. }),
        "transfer arma un BatchRename"
    );
    let (_m, summary) = ejecutar(&req);

    // Los nombres viejos se fueron; los nuevos existen con el contenido correcto.
    assert!(
        !f1.exists() && !f2.exists() && !f3.exists(),
        "nombres viejos fuera"
    );
    assert_eq!(leer(&dir.path().join("a_001.txt")), b"1");
    assert_eq!(leer(&dir.path().join("a_002.txt")), b"2");
    assert_eq!(leer(&dir.path().join("a_003.txt")), b"3");
    assert_eq!(summary.count_done(), 3);
}

#[test]
fn batch_rename_en_cadena_se_ordena_por_dependencia() {
    // Caso filoso REAL: un "shift" en cadena foto1→foto2, foto2→foto3, foto3→foto4. Sin ordenar,
    // renombrar foto1→foto2 pisaría foto2. El plan los ordena por dependencia (de atrás hacia
    // adelante) y el resultado en disco es el corrimiento limpio, sin perder datos.
    let dir = tempfile::tempdir().unwrap();
    let f1 = dir.path().join("foto1.jpg");
    let f2 = dir.path().join("foto2.jpg");
    let f3 = dir.path().join("foto3.jpg");
    fs::write(&f1, b"uno").unwrap();
    fs::write(&f2, b"dos").unwrap();
    fs::write(&f3, b"tres").unwrap();

    let req = ops::batch_rename(
        vec![f1.clone(), f2.clone(), f3.clone()],
        vec![
            "foto2.jpg".to_string(),
            "foto3.jpg".to_string(),
            "foto4.jpg".to_string(),
        ],
    );
    let (_m, summary) = ejecutar(&req);

    // El corrimiento se concretó sin pisar datos: el contenido viajó un lugar adelante.
    assert_eq!(leer(&f2), b"uno", "foto2 ahora tiene lo de foto1");
    assert_eq!(leer(&f3), b"dos", "foto3 ahora tiene lo de foto2");
    assert_eq!(
        leer(&dir.path().join("foto4.jpg")),
        b"tres",
        "foto4 ← foto3"
    );
    // foto1 quedó libre (su contenido se movió a foto2).
    assert!(!f1.exists(), "el primer nombre quedó libre");
    assert_eq!(summary.count_done(), 3);
}

#[test]
fn batch_rename_intercambio_circular_es_rechazado_por_el_plan() {
    // Contrato v1 (documentado en plan_batch_rename): un ciclo puro a↔b NO se soporta — no hay
    // forma de ordenarlo sin un temporal. El plan debe FALLAR (no corromper datos intentándolo).
    // Este test fija ese límite: si algún día se soporta, este test avisará para revisarlo.
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"soy-a").unwrap();
    fs::write(&b, b"soy-b").unwrap();

    let req = ops::batch_rename(
        vec![a.clone(), b.clone()],
        vec!["b.txt".to_string(), "a.txt".to_string()],
    );
    assert!(
        plan(&req).is_err(),
        "el ciclo a↔b no es planificable en v1 (se rechaza, no se corrompe)"
    );
    // Los archivos quedaron intactos (nunca se ejecutó nada).
    assert_eq!(leer(&a), b"soy-a");
    assert_eq!(leer(&b), b"soy-b");
}

// ──────────────────────── CREAR carpetas / archivos ────────────────────────

#[test]
fn parse_de_varias_lineas_separa_validas_e_invalidas() {
    // El cuadro "nueva(s) carpeta(s)" acepta varias líneas; cada una es una carpeta (anidada si
    // lleva `\` o `/`). `parse_new_folders` es puro: separa válidas de inválidas conservando el
    // orden. Esto alimenta la creación real (cada Valid → un create_dir_all). Cubrir el parser es
    // cubrir el contrato de "crear múltiples subcarpetas anidadas".
    let text = "Documentos\n  proyecto/sub/profundo  \n\nmala:carpeta\n..";
    let specs = ops::parse_new_folders(text);
    assert_eq!(specs.len(), 4, "4 líneas no vacías (la vacía se ignora)");
    // 1) simple, recortada.
    assert_eq!(specs[0], ops::FolderSpec::Valid("Documentos".to_string()));
    // 2) anidada: separadores normalizados a `\`.
    assert_eq!(
        specs[1],
        ops::FolderSpec::Valid("proyecto\\sub\\profundo".to_string())
    );
    // 3) carácter prohibido (`:`).
    assert!(
        matches!(
            specs[2],
            ops::FolderSpec::Invalid {
                reason: ops::NewFolderError::InvalidChars,
                ..
            }
        ),
        "':' es inválido en Windows"
    );
    // 4) traversal (`..`).
    assert!(
        matches!(
            specs[3],
            ops::FolderSpec::Invalid {
                reason: ops::NewFolderError::Traversal,
                ..
            }
        ),
        "'..' no se permite"
    );
}

#[test]
fn crear_carpeta_simple_por_el_motor() {
    // `ops::create(dir, name, is_dir=true)` arma un CreateDir cuyo destino es `dir.join(name)`.
    let dir = tempfile::tempdir().unwrap();
    let req = ops::create(dir.path().to_path_buf(), "NuevaCarpeta".to_string(), true);
    assert!(matches!(req.kind, OpKind::CreateDir { .. }));
    let (_m, summary) = ejecutar(&req);
    assert!(
        dir.path().join("NuevaCarpeta").is_dir(),
        "la carpeta existe"
    );
    assert_eq!(summary.count_done(), 1);
}

#[test]
fn crear_archivo_vacio_por_el_motor() {
    let dir = tempfile::tempdir().unwrap();
    let req = ops::create(dir.path().to_path_buf(), "vacio.txt".to_string(), false);
    assert!(matches!(req.kind, OpKind::CreateFile { .. }));
    let (_m, summary) = ejecutar(&req);
    let f = dir.path().join("vacio.txt");
    assert!(f.is_file(), "se creó el archivo");
    assert_eq!(leer(&f).len(), 0, "el archivo nuevo está vacío");
    assert_eq!(summary.count_done(), 1);
}

// ──────────────────────── CONFLICTOS (políticas no-interactivas) ────────────────────────

#[test]
fn conflicto_skip_conserva_el_existente() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    fs::write(&src, b"NUEVO").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();
    fs::write(dst.join("a.txt"), b"VIEJO").unwrap();

    let mut req = ops::transfer(false, vec![src], dst.clone());
    req.conflict = ConflictPolicy::Skip;
    let (_m, summary) = ejecutar(&req);

    assert_eq!(
        leer(&dst.join("a.txt")),
        b"VIEJO",
        "Skip no pisa el existente"
    );
    assert_eq!(summary.count_skipped(), 1);
}

#[test]
fn conflicto_overwrite_pisa_el_existente() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    fs::write(&src, b"NUEVO").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();
    fs::write(dst.join("a.txt"), b"VIEJO").unwrap();

    let mut req = ops::transfer(false, vec![src], dst.clone());
    req.conflict = ConflictPolicy::Overwrite;
    let (_m, _s) = ejecutar(&req);

    assert_eq!(leer(&dst.join("a.txt")), b"NUEVO", "Overwrite reemplaza");
}

#[test]
fn conflicto_rename_crea_copia_con_sufijo_sin_tocar_el_existente() {
    // Política Rename: el destino ya tiene a.txt; la copia entra como "a (1).txt" (dedup) y el
    // archivo existente NO se toca.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    fs::write(&src, b"NUEVO").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();
    fs::write(dst.join("a.txt"), b"VIEJO").unwrap();

    let mut req = ops::transfer(false, vec![src], dst.clone());
    req.conflict = ConflictPolicy::Rename;
    let (_m, summary) = ejecutar(&req);

    assert_eq!(
        leer(&dst.join("a.txt")),
        b"VIEJO",
        "el existente se conserva"
    );
    // El sufijo de dedup empieza en (2): "a.txt" ocupado → "a (2).txt".
    assert_eq!(
        leer(&dst.join("a (2).txt")),
        b"NUEVO",
        "la copia entró con sufijo (2)"
    );
    assert_eq!(summary.count_done(), 1);
}

// ──────────────────────── MOTOR THREADED (spawn) ────────────────────────

#[test]
fn spawn_copia_en_hilo_y_emite_done() {
    // El camino `spawn` (worker real en un hilo) es el que usa la app. Aquí se ejercita de punta a
    // punta: lanza el worker, espera el `Done(summary)` por el canal y verifica el disco.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    fs::write(&src, b"hola threaded").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();

    let req = ops::transfer(false, vec![src], dst.clone());
    let p = plan(&req).unwrap();
    let token = CancellationToken::new();
    let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
    let (rx, handle) = spawn(p, req.kind.clone(), req.conflict, token, crx, None);

    // El último mensaje del worker debe ser Done con el resumen.
    let msgs: Vec<OpMsg> = rx.into_iter().collect();
    handle.join().expect("el worker termina sin panic");
    let last = msgs.last().expect("hubo al menos un mensaje");
    match last {
        OpMsg::Done(s) => assert_eq!(s.count_done(), 1, "un archivo copiado"),
        otro => panic!("se esperaba Done, llegó {otro:?}"),
    }
    assert_eq!(leer(&dst.join("a.txt")), b"hola threaded");
}

#[test]
fn spawn_cancelado_antes_de_empezar_emite_cancelled_y_no_copia() {
    // Cancelación universal: si el token ya está cancelado al spawnear, el worker no copia nada y
    // emite Cancelled (no Done).
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    fs::write(&src, b"x").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();

    let req = ops::transfer(false, vec![src], dst.clone());
    let p = plan(&req).unwrap();
    let token = CancellationToken::new();
    token.cancel(); // cancelado de entrada
    let (_ctx, crx) = mpsc::channel::<ConflictDecision>();
    let (rx, handle) = spawn(p, req.kind.clone(), req.conflict, token, crx, None);

    let msgs: Vec<OpMsg> = rx.into_iter().collect();
    handle.join().unwrap();
    assert!(
        matches!(msgs.last(), Some(OpMsg::Cancelled(_))),
        "el último mensaje es Cancelled"
    );
    assert!(
        !dst.join("a.txt").exists(),
        "no se copió nada (cancelado de entrada)"
    );
}

// ──────────────────────── DESHACER (round-trip ejecutado) ────────────────────────

/// Helper: corre una lista de `OpRequest` (los que produce `to_requests`) por el motor, en orden.
/// Las acciones de deshacer que mandan a PAPELERA (`TrashCreated`) usan `to_trash:true`, que el
/// motor de core NO ejecuta (la papelera la hace platform); por eso este round-trip se prueba con
/// MOVER (cuyo inverso es MoveBack, 100% ejecutable por core).
fn correr_requests(reqs: &[OpRequest]) {
    for r in reqs {
        let _ = ejecutar(r);
    }
}

#[test]
fn deshacer_un_move_restaura_el_origen_en_disco() {
    // Round-trip completo del deshacer ejecutado: mover a.txt a dst/, construir el inverso
    // (build_undo), validarlo, re-emitirlo (to_requests) y correrlo → el archivo vuelve a su
    // lugar original y el destino queda vacío. Los tests de undo existentes verifican la FORMA del
    // inverso; este verifica que EJECUTARLO realmente revierte el disco.
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.txt");
    fs::write(&src, b"vuelve a casa").unwrap();
    let dst = dir.path().join("dst");
    fs::create_dir(&dst).unwrap();

    // 1) Mover.
    let req = ops::transfer(true, vec![src.clone()], dst.clone());
    let (_m, summary) = ejecutar(&req);
    assert!(
        !src.exists() && dst.join("a.txt").exists(),
        "el move se concretó"
    );

    // 2) Construir y validar el inverso a partir del request + summary reales.
    let actions = ops::undo::build_undo(&req, &summary).expect("mover es deshacible");
    ops::undo::validate(&actions).expect("el inverso aún aplica");

    // 3) Re-emitir como OpRequests y correrlos por el motor.
    let reqs = ops::undo::to_requests(&actions);
    correr_requests(&reqs);

    // 4) Estado final: el archivo volvió a su sitio; el destino quedó sin él.
    assert_eq!(leer(&src), b"vuelve a casa", "el origen se restauró");
    assert!(
        !dst.join("a.txt").exists(),
        "el destino quedó sin el archivo (deshacer lo devolvió)"
    );
}

#[test]
fn deshacer_un_rename_revierte_el_nombre_en_disco() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("original.txt");
    fs::write(&src, b"c").unwrap();

    let req = ops::rename(src.clone(), "renombrado.txt".to_string());
    let (_m, summary) = ejecutar(&req);
    let renombrado = dir.path().join("renombrado.txt");
    assert!(renombrado.exists() && !src.exists());

    let actions = ops::undo::build_undo(&req, &summary).expect("rename es deshacible");
    ops::undo::validate(&actions).unwrap();
    correr_requests(&ops::undo::to_requests(&actions));

    assert!(src.exists(), "el nombre original volvió");
    assert!(!renombrado.exists(), "el nombre renombrado se fue");
}

#[test]
fn deshacer_un_delete_no_esta_disponible() {
    // Contrato v1: borrar NO es deshacible (restaurar de papelera requiere Shell API aparte).
    // build_undo debe devolver None para un Delete.
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.txt");
    fs::write(&f, b"x").unwrap();
    let req = ops::delete(vec![f], false);
    let summary = OpSummary {
        items: vec![(PathBuf::from("a.txt"), OpOutcome::Done)],
        ..Default::default()
    };
    assert!(
        ops::undo::build_undo(&req, &summary).is_none(),
        "borrar no es deshacible en v1"
    );
}
