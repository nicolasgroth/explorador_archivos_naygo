# Compilar y empaquetar Naygo

Estado del producto: [ESTADO-ACTUAL.md](ESTADO-ACTUAL.md). Antes de cambios funcionales,
seguir [PRUEBAS.md](PRUEBAS.md); no probar una entrega experimental sobre la configuración
de trabajo. Compilar, pasar tests, validar visualmente y empaquetar son estados distintos.

## Prerequisitos

- **Rust 1.96.0** (toolchain MSVC, `x86_64-pc-windows-msvc`), versión fijada en
  `rust-toolchain.toml` y validada en el gate local/CI. Instalar desde <https://rustup.rs>.
  El proyecto usa CRT estático (`.cargo/config.toml`), por lo que
  el `.exe` resultante corre en equipos limpios sin el "Visual C++ Redistributable".
- **Inno Setup** (opcional, solo para generar el instalador): descargar de
  <https://jrsoftware.org/isdl.php>. Tras instalar, asegúrate de que `ISCC.exe` esté
  en el `PATH` (o ejecuta el script desde la consola "Inno Setup" / agrega su carpeta,
  típicamente `C:\Program Files (x86)\Inno Setup 6`, al `PATH`).

## Compilar (desarrollo)

```
cargo build                    # debug
cargo run -p naygo-ui-slint    # corre Naygo (binario `naygo`)
cargo test --workspace --locked # tests reproducibles desde Cargo.lock
```

## Estado del port Linux

El código de arranque ya resuelve XDG para Linux sin consultar el filesystem: la configuración va
en `XDG_CONFIG_HOME/naygo` (o `~/.config/naygo`) y los logs en `XDG_STATE_HOME/naygo` (o
`~/.local/state/naygo`). `$HOME` determina el panel Home; los respaldos de panel y carpeta perdida
respetan la preferencia de Home y no caen en rutas de Windows. Los workers crean los directorios al
guardar settings o logs; en Windows se mantiene la configuración portable junto al EXE. El port aún
no tiene compilación, ejecución ni paquete Linux validados. Requisitos y límites están en el
[plan Linux](plans/2026-10-13-linux-ubuntu-luna.md). El inventario de terceros Windows no aplica a
Linux. Los avisos preliminares para Ayatana AppIndicator GLib y la excepción GPL-3 Linux-only están en
[`../THIRD-PARTY-NOTICES-LINUX.md`](../THIRD-PARTY-NOTICES-LINUX.md); no son aún el inventario final
ni la fuente correspondiente de una entrega Linux.

## Compilar release + empaquetar

Un solo comando arma todo:

```
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

Qué hace, en orden:

1. Lee la versión del `Cargo.toml` raíz (fuente única de verdad).
2. `cargo build --release --locked --jobs 1 -p naygo-ui-slint` → `target\release\naygo.exe` (con ícono,
   metadatos de autoría y CRT estático).
3. Genera `dist\Naygo-<versión>-portable.zip` (`naygo.exe` + `LICENSE` + `LEEME.txt` +
   `THIRD-PARTY-NOTICES.md`).
4. Genera las imágenes del asistente del instalador (`installer\wizard-*.bmp`) desde
   `assets\icons\logo_naygo.png`.
5. Si `ISCC.exe` está disponible, genera `dist\Naygo-<versión>-setup.exe`. Si no,
   avisa y genera solo el ZIP portable cuando no hay setup previo de esa versión. Si existe uno,
   detiene el proceso antes de modificar `dist/` para evitar mezclarlo con un portable nuevo;
   instala Inno Setup o mueve el setup anterior fuera de `dist/` antes de reintentar. El archivo
   de checksums enumera solo artefactos generados en la ejecución actual.

## Compilación supervisada para proteger el equipo

Si el equipo está cargado o hubo una interrupción durante el build:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release-guarded.ps1
```

Invoca el mismo `build-release.ps1`, sin sobreescribir el perfil declarado en Cargo.toml.
Desde el ajuste del 14 de septiembre, release usa ThinLTO/16 unidades, conserva símbolos
completos y optimización de tamaño. Validado en build 202609142135: pico contable 10,44 GiB
bajo límite de 12 GiB, con instalador y portable verificados. No es un techo de memoria
de Naygo ni garantiza igual consumo en otros equipos/toolchains; conservar la supervisión.
Usa un solo trabajo, prioridad baja y un Job Windows que limita la memoria comprometida
del árbol de compilación a 12 GiB. Comprueba cada dos segundos que queden al menos
8 GiB disponibles tanto de RAM como de compromiso; si falta margen, termina únicamente
su árbol de procesos. No modifica paginación, servicios ni otras aplicaciones. Estos
valores son parámetros (`-MemoryGiB`, `-MinAvailableGiB`), no un consumo preasignado.

El hijo espera una señal antes de ejecutar compiladores; los límites se aplican y verifican
primero. Si se cierra el supervisor, su Job termina los descendientes. Los logs separados
quedan en `target/agent-out/guarded-<fecha>.{out,err}.log`. Un corte preventivo o un límite
alcanzado es un build fallido, no una entrega; conservar/verificar el paquete anterior.
El contador de pico del Job es diagnóstico, no RSS, y puede incluir una reserva rechazada.
No garantiza evitar bloqueos del sistema debidos a drivers u otras causas ajenas al build.

**Intentos previos del 14 de septiembre:** los límites de 12 y 16 GiB resultaron insuficientes
para compilar RP-4B2 con fat LTO/una unidad. Ambos intentos terminaron antes del
empaquetado; RP-5B quedó respaldado. El perfil ThinLTO autorizado después completó el build
con 12 GiB. No reintentar automáticamente aumentando límites si otro build falla: revisar
las condiciones y validar sus trade-offs antes de generar una entrega.
Ver [evidencia de recuperación](VALIDACION-RP4B2-2026-09-14.md) y
[ajuste autorizado/auditoría de espacio](VALIDACION-BUILD-2026-09-14.md).

Modos de comprobación sin empaquetar:

```powershell
# Reserva de prueba de 512 MiB: debe rechazarse dentro de un Job de 256 MiB.
powershell -ExecutionPolicy Bypass -File scripts\build-release-guarded.ps1 -Probe -MemoryGiB 0.25
# Suite y Clippy con las dependencias ya disponibles localmente, bajo el mismo supervisor.
powershell -ExecutionPolicy Bypass -File scripts\build-release-guarded.ps1 -Verify
# Harness headless W-07/W-10B en perfil release optimizado, bajo el mismo supervisor.
powershell -ExecutionPolicy Bypass -File scripts\build-release-guarded.ps1 -W07ReleaseMeasurement
# Callback de teclado Slint W-07 en perfil release, bajo el mismo supervisor.
powershell -ExecutionPolicy Bypass -File scripts\build-release-guarded.ps1 -W07InputMeasurement
```

## Artefactos (en `dist\`, no versionado)

Ejecutar el empaquetador SOLO, sin otra compilación pesada: release usa ThinLTO, 16 unidades
y un solo trabajo Cargo. El build ID se renueva mediante un nonce del script. Se genera `naygo.pdb`
junto al EXE; el instalador lo ofrece opcional/desmarcado y el ZIP no lo incluye (T-3).
El script genera `SHA256SUMS.txt`. Si se configura un certificado Authenticode autorizado,
firma y verifica `naygo.exe` antes de incluirlo en el ZIP/instalador, y firma/verifica el instalador
al terminar. `-ExecutableOnly` permite detenerse tras compilar para una firma remota; `-SkipBuild`
empaqueta el ejecutable ya firmado sin recompilar. Ver [distribución](DISTRIBUTION.md).

La distribución 0.5.2 del 14 de octubre está registrada en
[validación del build](VALIDACION-BUILD-2026-10-14.md). Comprobar fecha, versión/build, hashes,
integridad ZIP y coincidencia del EXE empaquetado
con el release. Si falta Inno o falla el instalador, no anunciar una entrega completa.
Cambios sólo documentales no requieren nueva versión ni EXE; las guías empaquetadas se
actualizan al producir la siguiente distribución funcional.

| Archivo | Qué es |
|---|---|
| `Naygo-<versión>-portable.zip` | Versión portable: descomprimir y ejecutar, sin instalar. |
| `Naygo-<versión>-setup.exe` | Instalador (asistente, accesos directos, desinstalador). |

## Troubleshooting

- **"ISCC.exe no encontrado"**: Inno Setup no está instalado o no está en el `PATH`.
  Instálalo (link arriba) y vuelve a ejecutar el script; el ZIP portable se genera igual sin Inno.
- **El `.exe` no muestra el ícono**: vuelve a ejecutar `cargo build --release
  -p naygo-ui-slint` (el ícono se embebe vía `winresource` en
  `crates/ui-slint/build.rs`). Explorer cachea íconos; prueba en otra carpeta.
- **Error al generar las BMP del asistente**: el script usa `System.Drawing` de .NET;
  en Windows 10/11 normal está disponible. En ediciones recortadas (Server Core),
  genera las BMP a mano y vuelve a correr.
- **Falla `cargo build`**: confirma que Rustup instaló el toolchain fijado en
  `rust-toolchain.toml` y el target MSVC `x86_64-pc-windows-msvc`.
