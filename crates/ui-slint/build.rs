// Naygo — compila los .slint de la capa UI Slint y, en Windows, embebe el ícono y los
// metadatos del ejecutable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
fn main() {
    // El compilador de la UI declarativa supera el stack principal de 1 MiB de MSVC.
    // Esta reserva sólo existe durante el build; no aumenta el stack/RAM de Naygo.
    std::thread::Builder::new()
        .name("slint-compiler".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| slint_build::compile("ui/app-window.slint").expect("compilar app-window.slint"))
        .expect("iniciar compilador Slint")
        .join()
        .expect("compilador Slint");

    // ID de build con fecha-hora local (YYYYMMDDHHMM), estampado en el binario como variable de
    // entorno de compilación. La versión mostrada queda `X.Y.Z+build.YYYYMMDDHHMM` (metadato de
    // build de semver): incrementa monotónicamente en cada compilación, así se distingue sin
    // ambigüedad qué build se está probando. Si no se puede leer la hora, se usa "unknown".
    let build_id = build_timestamp();
    println!("cargo:rustc-env=NAYGO_BUILD_ID={build_id}");
    // Cambios reales invalidan el ID. El empaquetador además cambia un nonce para que
    // cada distribución tenga su propio ID incluso sin cambios de código. No usar una
    // ruta inexistente: obligaba a recompilar toda la UI incluso al repetir una prueba.
    for path in [
        "src",
        "ui",
        "../core/src",
        "../platform/src",
        "../../Cargo.toml",
        "../../Cargo.lock",
        "../../CHANGELOG.md",
        "../../LICENSE",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=NAYGO_BUILD_NONCE");

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
        res.set("CompanyName", "ISGroth — Nicolás Groth <ngroth@gmail.com>");
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth. MIT License.",
        );
        if let Err(e) = res.compile() {
            // No abortar el build por esto: sin ícono el .exe igual funciona. Solo avisar.
            println!("cargo:warning=No se pudo embeber el ícono del .exe: {e}");
        }
    }
}

/// Fecha-hora local actual como `YYYYMMDDHHMM`, para el ID de build. Se obtiene del reloj del
/// sistema vía su propio comando (así respeta el huso local sin añadir dependencias de zona
/// horaria al build). Si algo falla, devuelve "unknown" (el build no debe romperse por esto).
fn build_timestamp() -> String {
    let out = if cfg!(windows) {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-Date -Format yyyyMMddHHmm"])
            .output()
    } else {
        std::process::Command::new("date")
            .args(["+%Y%m%d%H%M"])
            .output()
    };
    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            // Validar que sean 12 dígitos; si no, "unknown" (no confiar en salida rara).
            if s.len() == 12 && s.chars().all(|c| c.is_ascii_digit()) {
                s
            } else {
                "unknown".to_string()
            }
        }
        _ => "unknown".to_string(),
    }
}
