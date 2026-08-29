// Naygo — proveedor de metadata para modelos STL/3MF.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use super::{MetadataField, MetadataProvider};
use std::path::Path;

pub struct MeshMeta;

impl MetadataProvider for MeshMeta {
    fn extensions(&self) -> &'static [&'static str] {
        &["stl", "3mf"]
    }

    fn read(&self, path: &Path) -> Vec<MetadataField> {
        let token = crate::CancellationToken::new();
        let Ok(info) = crate::mesh_preview::inspect_file(path, &token) else {
            return Vec::new();
        };
        vec![
            MetadataField {
                label_key: "meta.dimensions",
                value: format!(
                    "{:.2} × {:.2} × {:.2}",
                    info.dimensions[0], info.dimensions[1], info.dimensions[2]
                ),
            },
            MetadataField {
                label_key: "meta.triangles",
                value: info.triangle_count.to_string(),
            },
            MetadataField {
                label_key: "meta.unit",
                value: info.unit,
            },
        ]
    }
}
