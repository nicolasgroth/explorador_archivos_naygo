// Naygo — compila los .slint de la capa UI Slint y, en Windows, embebe el ícono y los
// metadatos del ejecutable.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
/// Manifest de aplicación con únicamente `longPathAware`.
#[cfg(windows)]
const LONG_PATH_MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </windowsSettings>
  </application>
</assembly>
"#;

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
    println!("cargo:rerun-if-changed=ui");

    emit_build_id();

    // Las versiones del .exe salen de Cargo.toml. Cargo ya rastrea `src/` y las dependencias;
    // declarar esas rutas aquí obligaba a re-ejecutar el build script (y recompilar toda la UI)
    // ante cualquier cambio de código.
    println!("cargo:rerun-if-changed=../../Cargo.toml");

    // En Windows: embeber el ícono de la app + metadatos del .exe (producto, versión, autor),
    // así el explorador y la barra de tareas muestran el ícono propio de Naygo en vez del
    // genérico. La ruta del .ico es relativa a la raíz del repo (dos niveles arriba del crate).
    #[cfg(windows)]
    {
        let ico = "../../assets/icons/naygo_icon.ico";
        println!("cargo:rerun-if-changed={ico}");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico);
        // Manifest mínimo: solo declara longPathAware (rutas > MAX_PATH en APIs que lo respetan,
        // si el sistema tiene LongPathsEnabled). No toca DPI, comctl ni trustInfo.
        res.set_manifest(LONG_PATH_MANIFEST);
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

/// ID de build estampado en el binario (`NAYGO_BUILD_ID`). Lo fija el empaquetador
/// (`scripts/build-release.ps1`) con la fecha-hora local `YYYYMMDDHHMM`; la versión mostrada
/// queda `X.Y.Z+build.YYYYMMDDHHMM` (metadato de build de semver). Fuera del empaquetado (o si el
/// valor no son exactamente 12 dígitos) vale `dev`: así un build de desarrollo no se re-ejecuta
/// por reloj y nunca se confunde con uno distribuido.
fn emit_build_id() {
    println!("cargo:rerun-if-env-changed=NAYGO_BUILD_ID");
    let build_id = std::env::var("NAYGO_BUILD_ID")
        .ok()
        .filter(|id| id.len() == 12 && id.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or_else(|| "dev".to_string());
    println!("cargo:rustc-env=NAYGO_BUILD_ID={build_id}");
}
