// Naygo — build script: embebe los metadatos de autoría en el .exe (solo Windows).
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.

fn main() {
    // Solo en Windows se compila el recurso de versión; en otros SO es no-op.
    #[cfg(target_os = "windows")]
    {
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
