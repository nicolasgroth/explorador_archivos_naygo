// Naygo — modelo de columnas del file panel (puro, sin egui ni Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

//! Define qué columnas existen y el estado de tabla de un panel (qué columnas se
//! ven, en qué orden, su ancho) más los filtros activos. Puro y testeable.

use serde::{Deserialize, Serialize};

/// Qué columna. Extensible: agregar variante + su extractor a futuro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ColumnKind {
    Name,
    Extension,
    Size,
    Modified,
    Created,
}
