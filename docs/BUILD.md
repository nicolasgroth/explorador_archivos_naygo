# Compilar y empaquetar Naygo

## Prerequisitos

- **Rust** (toolchain MSVC, `x86_64-pc-windows-msvc`). Instalar desde
  <https://rustup.rs>. El proyecto usa CRT estático (`.cargo/config.toml`), por lo que
  el `.exe` resultante corre en equipos limpios sin el "Visual C++ Redistributable".
- **Inno Setup** (opcional, solo para generar el instalador): descargar de
  <https://jrsoftware.org/isdl.php>. Tras instalar, asegurate de que `ISCC.exe` esté
  en el `PATH` (o ejecutá el script desde la consola "Inno Setup" / agregá su carpeta,
  típicamente `C:\Program Files (x86)\Inno Setup 6`, al `PATH`).

## Compilar (desarrollo)

```
cargo build            # debug
cargo run -p naygo-ui  # corre Naygo
cargo test --workspace # tests
```

## Compilar release + empaquetar

Un solo comando arma todo:

```
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

Qué hace, en orden:

1. Lee la versión del `Cargo.toml` raíz (fuente única de verdad).
2. `cargo build --release` → `target\release\naygo.exe` (con ícono, metadatos de
   autoría y CRT estático).
3. Genera `dist\Naygo-<versión>-portable.zip` (`naygo.exe` + `LICENSE` + `LEEME.txt`).
4. Genera las imágenes del asistente del instalador (`installer\wizard-*.bmp`) desde
   `assets\icons\logo_naygo.png`.
5. Si `ISCC.exe` está disponible, genera `dist\Naygo-<versión>-setup.exe`. Si no,
   avisa (con el link de descarga) y deja igual el ZIP portable.

## Artefactos (en `dist\`, no versionado)

| Archivo | Qué es |
|---|---|
| `Naygo-<versión>-portable.zip` | Versión portable: descomprimir y ejecutar, sin instalar. |
| `Naygo-<versión>-setup.exe` | Instalador (asistente, accesos directos, desinstalador). |

## Troubleshooting

- **"ISCC.exe no encontrado"**: Inno Setup no está instalado o no está en el `PATH`.
  Instalalo (link arriba) y reintentá; el ZIP portable se genera igual sin Inno.
- **El `.exe` no muestra el ícono**: rehacé `cargo build --release` (el ícono se
  embebe vía `crates/ui/app.rc`). Explorer cachea íconos; probá en otra carpeta.
- **Error al generar las BMP del asistente**: el script usa `System.Drawing` de .NET;
  en Windows 10/11 normal está disponible. En ediciones recortadas (Server Core),
  generá las BMP a mano y volvé a correr.
- **Falla `cargo build`**: confirmá el toolchain MSVC (`rustup default
  stable-x86_64-pc-windows-msvc`).
