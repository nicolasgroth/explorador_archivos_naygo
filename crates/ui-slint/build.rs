// Naygo — compila los .slint de la capa UI Slint y, en Windows, embebe el ícono y los
// metadatos del ejecutable.
// Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
fn main() {
    slint_build::compile("ui/app-window.slint").expect("compilar app-window.slint");

    // En Windows: embeber el ícono de la app + metadatos del .exe (producto, versión, autor),
    // así el explorador y la barra de tareas muestran el ícono propio de Naygo en vez del
    // genérico. La ruta del .ico es relativa a la raíz del repo (dos niveles arriba del crate).
    #[cfg(windows)]
    {
        let ico = "../../assets/icons/naygo_icon.ico";
        println!("cargo:rerun-if-changed={ico}");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico);
        res.set("ProductName", "Naygo");
        res.set("FileDescription", "Naygo — explorador de archivos");
        res.set("CompanyName", "ISGroth — Nicolás Groth");
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.",
        );
        if let Err(e) = res.compile() {
            // No abortar el build por esto: sin ícono el .exe igual funciona. Solo avisar.
            println!("cargo:warning=No se pudo embeber el ícono del .exe: {e}");
        }
    }
}
