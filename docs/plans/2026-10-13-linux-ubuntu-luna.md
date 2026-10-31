# Naygo — plan Linux para Ubuntu LTS y derivados, GPT-6 Luna

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-14; actualización 2026-10-19; decisión de mínimo actualizada el 2026-10-15. Estado: **mínimo de L-00 elegido;
validación nativa pendiente; backend GLib de tray implementado parcialmente; L-08 tiene implementación y checks cruzados, ejecución nativa pendiente**.
Nicolás eligió Ubuntu LTS y derivados, x86_64 como prioridad inicial.
Complementa [el plan de mejoras](2026-10-13-mejoras-y-ejecucion-luna.md).

## Alcance propuesto y decisiones

Reutilizar core, platform y ui-slint en el mismo repositorio. Mantener renderer software,
navegación rápida, workers cancelables, i18n, MIT y autoría visible. No reescribir la app,
no sustituir Slint ni crear un fork Linux que diverja de Windows.

Mínimo de alfa: Ubuntu **26.04 LTS x86_64**. GNOME/Wayland es prioritario, con X11 donde el
escritorio lo ofrezca; Kubuntu o Mint quedan como posibles pruebas de derivado. Construir
en la base mínima validada para no introducir una glibc demasiado nueva en el paquete.
Estas dos LTS constan en la [lista oficial de Ubuntu](https://ubuntu.com/project/docs/release-team/list-of-releases/).
La elección exacta de derivado y qué combinaciones recibirán soporte se cierra en L-00.

| Decisión | Propuesta | Estado |
| --- | --- | --- |
| Plataforma inicial | Ubuntu LTS/derivados, x86_64 | Elegida por Nicolás |
| Versión mínima de alfa | Ubuntu 26.04 LTS x86_64 | Elegida por Nicolás el 2026-10-15 |
| Escritorios/derivados certificados | GNOME/Wayland prioritario; X11 y un derivado por definir | Pendiente de L-00 |
| Artefacto inicial | `.deb` + checksums; AppImage en hito posterior | Propuesta para L-00 |
| Configuración instalada | XDG, directorios escribibles del usuario | Propuesta para L-02 |
| Portabilidad de config Windows | Importación revisada, no compartir config viva | Propuesta para L-02 |
| Dependencias | Crates permisivas; inventario de librerías nativas | Regla vigente; enlace nativo por resolver |
| Tray/hotkey/posición de ventana | Capacidad por sesión, degradación explícita | Propuesta para L-03/L-11 |
| Preview en alfa de navegación | Deshabilitado hasta supervisor Linux validado | Condición técnica de este plan |
| Rutas no UTF-8 | Identidad nativa preservada; jamás reconstruir la ruta desde el texto mostrado con sustituciones | Contrato de L-01 |
| Discos/red | Montajes existentes; sin gestor de credenciales nuevo | Propuesta de alcance v1 |

La decisión Linux del 15 de octubre autoriza GPL-3 para la edición Linux de la alfa si resulta
necesario para migrar la bandeja al backend GLib; Windows conserva MIT. En L-00 todavía se debe
inventariar la licencia y forma de enlace de las bibliotecas realmente requeridas, comprobar los
derechos de todos los aportes, preparar fuente correspondiente y notices, y reflejar con claridad
el alcance por plataforma. Esta decisión no es una garantía legal ni autoriza a publicar sin esa
revisión.

## Evidencia actual y diferencia con julio

`docs/auditoria/2026-07-01-portabilidad-linux.md` y el análisis de decisión son antecedentes.
Sus compilaciones/estimaciones se refieren a julio, no a esta base. No asumir que sus versiones
de crates o degradaciones siguen iguales, ni afirmar que aquí se compiló/ejecutó Linux.

Inspección local del 13 de octubre:

- `Cargo.lock`: Slint 1.16.1, winit 0.30.13, rfd 0.15.4 y tray-icon 0.24.1.
- La dependencia `windows` está condicionada a Windows en platform; `.cargo/config.toml`
  restringe CRT estático al target MSVC. `build.rs` condiciona recursos Win32.
- Abrir archivos, papelera, portapapeles, hotkey y otras integraciones tienen stubs no-Windows.
  `drives()` devuelve sólo `/`; la instancia única no-Windows siempre se declara primaria.
  Compilar stubs no entrega esas funciones.
- `preview_process::reader_contained` devuelve false fuera de Windows y `Job::attach`
  devuelve Unsupported. **Texto/imagen/PDF/comprimidos/STL/3MF/animación necesitan un
  supervisor Linux; no habilitar fallback de lectura en proceso.**
- `core/ops/plan.rs` y `undo.rs` usan claves `to_string_lossy().to_lowercase()`.
  `config::resolve_home_dir` usa USERPROFILE/C:\\ y `portable_dir` apunta al EXE.
  `path_input` tiene reglas de entrada de Windows. Compilar core no asegura semántica POSIX.
- La UI conserva `tray-icon` sin selección de target: revisar dependencias nativas e integración
  del event loop antes de habilitar bandeja. No iniciar GTK dentro del hilo Slint por intuición.

Slint documenta soporte winit en Linux X11/Wayland y renderer software; eso respalda conservar
el stack, no certifica la paridad interactiva de Naygo ni APIs nuevas en 1.16.1.
[Backend oficial](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/).
Para cambios usar fuente/API de las versiones **fijadas**, no copiar ejemplos `latest` sin contrastar.

**Intento local 2026-10-14, revalidado tras el cambio W-07:** el target
`x86_64-unknown-linux-gnu` está instalado en el toolchain fijado `1.96.0`. Pasaron de nuevo
`cargo check --tests` para core/platform y Clippy cruzado con `-D warnings` pasan ahora sin avisos.
La primera corrida de tests cruzados mostró 10 warnings de imports/helpers Windows-only; Clippy
encontró además tres `drop_non_drop` al compilar guards/jobs vacíos Linux. Se limitaron a Windows
los imports y los cierres explícitos de recursos, y se agregaron tests para los stubs de espacio,
carpetas conocidas y Papelera. La cobertura platform Windows (54 tests, 6 ignorados) también pasa.
Esto comprueba compilación/lints cruzados, no link final ni comportamiento nativo.

La instancia Ubuntu está registrada en WSL 2. Revisión del 2026-10-16: `wsl --list --quiet` la
enumera y `wsl --status` indica versión 2; al iniciar, WSL informa que no puede arrancar porque
Virtual Machine Platform/virtualización no está habilitada (`HCS_E_HYPERV_NOT_INSTALLED`).
`Win32_ComputerSystem.HypervisorPresent` es `False`, aunque Windows reporta las extensiones de
virtualización del Ryzen 7 1700X activas. La consulta de estado de componentes opcionales requiere
elevación; no pude confirmar cuál feature falta y no cambié la configuración. Sólo está disponible
el comando WSL; no hay Docker, Podman, QEMU ni VirtualBox. Tampoco
hay sysroot o `pkg-config` Linux desde Windows para compilar la UI. El `cargo tree` y metadata
filtrados por `x86_64-unknown-linux-gnu` confirman que `tray-icon 0.24.1` usa el backend Linux por
defecto y arrastra GTK 3/AppIndicator; los crates Rust `tray-icon`, `muda`, `gtk` y `libappindicator`
declaran MIT/Apache, pero eso **no determina la licencia de las bibliotecas nativas del sistema**.
El upstream documenta GTK 3 y AppIndicator como dependencias de compilación del backend por defecto
([tray-icon](https://github.com/tauri-apps/tray-icon)); GTK declara LGPL-2.1-or-later
([licencia upstream](https://github.com/GNOME/gtk)). En `tray-icon` el backend GTK-free KSNI aparece
en las notas de la versión 0.25.0 ([releases](https://github.com/tauri-apps/tray-icon/releases));
la dependencia del proyecto sigue fijada en 0.24.1 y este bloque no actualiza dependencias.

L-00 ya tiene decisiones de mínimo (Ubuntu 26.04 LTS x86_64), backend GLib y excepción GPL-3
Linux-only. Los avisos preliminares para Ayatana GLib y su procedencia están en
[`../../THIRD-PARTY-NOTICES-LINUX.md`](../../THIRD-PARTY-NOTICES-LINUX.md). L-00 sigue abierto para
validar el runtime y cerrar el inventario final en una instalación nativa, confirmar la fuente
correspondiente de la versión realmente usada y cerrar la matriz de GNOME/Wayland, X11 y derivado.
La inspección autenticada del paquete oficial no sustituye esas pruebas. No se instalaron paquetes
ni se afirma compilación o ejecución nativa Linux.

**Auditoría FFI L-03, 2026-10-18:** las funciones GLib/GObject/Gio se contrastaron con headers de
GLib 2.88 y AppIndicator GLib 2.0.1. Se ajustaron `GIOCondition`/`GConnectFlags` a enteros C con
signo y se fijó el callback de `g_unix_fd_add_full` al tipo `GUnixFDSourceFunc`. Pasaron el check y
Clippy cruzados de todos los targets Linux y el gate completo Windows (1.464 pruebas, 8 ignoradas).
El runtime, D-Bus y comportamiento de bandeja siguen requiriendo ejecución en Ubuntu nativo; la
evidencia está en [`../VALIDACION-L03-CROSS-2026-10-17.md`](../VALIDACION-L03-CROSS-2026-10-17.md).

## Arquitectura y contratos

- `core`: reglas/modelos/formatos, `PathBuf`/`OsString` nativos. Reglas de nombres por política
  explícita; nada de Win32, Slint, D-Bus, mountinfo o `stat` del SO dentro de la lógica pura.
- `platform`: fachada estable; implementaciones Windows/Linux por módulo y capacidades
  tipadas. Consultas físicas en workers; resultados `Unsupported`/errores reales, sin éxito vacío.
- `ui-slint`: mismo producto y modelos; adaptar comandos a capacidades de la sesión.
  Mantener hooks de teclado/render; integrar APIs winit sin repartir `cfg` por cada callback.
- Identidad de archivo y representación visual son diferentes. UTF-8 con sustituciones sólo
  sirve para mostrar; no convertir esa representación de vuelta a ruta para copiar/renombrar/borrar.
- Comparación de rutas distingue igualdad léxica, identidad física y pertenencia a directorio.
  Linux puede montar FAT/CIFS sin sensibilidad a caja y Windows puede tener directorios sensibles:
  no asumir que `cfg(windows)` resuelve todo. Detección física en workers; política conservadora
  para casos desconocidos y revalidación antes de operaciones destructivas.
- Capacidades del escritorio pueden depender de portales/servicios instalados y permisos.
  No habilitar una acción sólo porque `target_os = linux`; informar no disponible en esa sesión.

## Hitos y dependencias

| ID | Resultado verificable | Dependencias |
| --- | --- | --- |
| L-00 | Matriz, entorno y licencias cerrados (parcial; notices preliminares listos, fuente/derechos/runtime nativo pendientes) | W-00 |
| L-01A | Identidad/rutas y validación POSIX | L-00 |
| L-01B | Copia/move/rename/undo correctos | L-01A |
| L-02 | Config XDG, home, logging y recuperación | L-01A, W-05 |
| L-03 | Compilación nativa y capacidades explícitas | L-00, L-01A, L-02 |
| L-04 | Alfa que abre y navega por teclado | L-03, W-01, W-08, W-09 |
| L-05 | Abrir archivos/terminal y dialogs | L-04 |
| L-06 | Volúmenes, espacio, watchers y dispositivos | L-04 |
| L-07 | Papelera y restauración segura | L-01B, L-06 |
| L-08 | Instancia única e IPC por usuario | L-02, L-04 |
| L-09A | Copia/corte de archivos vía clipboard | L-01B, L-04 |
| L-09B | Drag interno y externo según capacidad | L-09A, L-03 |
| L-10A | Supervisor Linux para preview estático | L-03, W-07, W-11A/B/C |
| L-10B | Supervisor de animación y lifecycle | L-10A |
| L-11 | Integración de sesión y expulsión | L-06, L-08 |
| L-12 | CI Linux y `.deb` instalable | L-04; gate completo exige L-05–11 |
| L-13 | Beta aceptada en matriz y documentación | L-01–12, W-13 para regresión Windows |
| L-14 | AppImage si se confirma utilidad | L-13, decisión de empaquetado |

Hito A: L-00–04, **alfa de navegación**, operaciones no validadas/previews deshabilitados.
Hito B: L-05–09, **beta de navegación y operaciones** con límites explícitos.
Hito C: L-10–13, **candidato Linux** con preview protegido y paquetes verificados.
No anunciar «Linux disponible» por aprobar sólo `cargo check` o iniciar una ventana.

Usar el método de Luna del plan de mejoras: un ID por vez, archivos acotados, regresión
antes del fix, evidencias y aprobación de Nicolás antes del siguiente. L-01B/L-09B/L-10A
deben subdividirse si una decisión nueva aumenta el alcance; no ejecutar como un mega-refactor.

## L-00 — preparar la plataforma de pruebas

**Archivos:** planes, `Cargo.lock`, manifiestos, `THIRD-PARTY-NOTICES.md`, documentación de build.
Crear inventario de módulos/stubs, API/versiones, dependencias nativas por feature y licencias.
Probar opciones para limitar tray/GTK a capacidad opcional sin afectar Windows; documentar coste.

Confirmar VM o host Ubuntu con disco ext4 y usuario de prueba; WSL puede ayudar a compilar,
pero un checkout en `/mnt/d` no demuestra semántica de ext4 ni WSLg sustituye la matriz desktop.
No instalar SO, paquetes del sistema ni tocar servicios del usuario desde esta fase documental.
Durante implementación, preparar un entorno autorizado con configuración/corpus propios.

**Evaluación inicial de bandeja (2026-10-14; ruta GTK superada el 2026-10-15):** Nicolás aceptó
conservar AppIndicator en la primera alfa y pidió revisar licencias. `tray-icon 0.24.1` declara MIT/Apache; sus bindings `libappindicator` y
`libappindicator-sys` también declaran Apache-2.0 OR MIT. En Linux, el sys crate prueba en runtime
`libayatana-appindicator3.so.1` y después `libappindicator3.so.1` mediante `libloading`, por lo que
la biblioteca de bandeja no se resuelve como un enlace estático obligatorio. GTK 3.24 es
LGPL-2.1-or-later; Ubuntu 24.04 publica `libappindicator3-1` en Universe. La licencia exacta del
paquete/binarios puede abarcar fuentes distintas y debe asentarse desde su archivo `copyright`
cuando se fije la imagen de build/paquete. Añadir esos textos/notices al `.deb` y comprobar el
comportamiento si no existe AppIndicator son condiciones de cierre, no de diseño. Hallazgo de
código: `libappindicator-sys 0.9.0` termina en `panic!` si no logra cargar Ayatana ni AppIndicator;
si la bandeja debe degradarse como capacidad opcional, la app necesita detectar disponibilidad antes
de inicializarla. Si la biblioteca será obligatoria, el `.deb` debe declarar el paquete runtime.

**Excepción de licencia aprobada (2026-10-14):** Nicolás autoriza GTK LGPL-2.1-or-later sólo para
la distribución Linux, con enlace dinámico y notices. No altera la política de Windows ni amplía las
licencias aceptadas para otras dependencias. Antes de publicar, revisar el `copyright` del paquete
Ubuntu exacto, agregar los notices correspondientes y conservar cumplimiento de los términos de
GTK/AppIndicator. Esta revisión técnica no sustituye asesoría legal.

**Evaluación histórica del backend GTK, 2026-10-15:** el paquete que se identificó para la interfaz
actual que carga `tray-icon` es `libayatana-appindicator3-1`, no el proveedor legacy
`libappindicator3-1` de Universe. Esta opción quedó superada por la decisión posterior de migrar a GLib.
Ubuntu publica el paquete Ayatana en `main` para Noble 24.04 (`0.5.93-1build3`) y Resolute 26.04
(`0.5.94-1build1`); ambos incluyen GTK 3, GLib, libdbusmenu y `libayatana-indicator3-7` como
dependencias. La lista oficial de archivos de Noble contiene tanto
`libayatana-appindicator3.so.1` como `libappindicator3.so.1`, así que satisface el primer nombre
que prueba `libappindicator-sys` y conserva el alias legacy. Referencias: [paquete Ayatana Noble]
(https://packages.ubuntu.com/noble/libayatana-appindicator3-1), [paquete Ayatana Resolute]
(https://packages.ubuntu.com/resolute/libayatana-appindicator3-1), [archivos de Noble]
(https://packages.ubuntu.com/noble/amd64/libayatana-appindicator3-1/filelist) y [paquete legacy
Resolute](https://packages.ubuntu.com/resolute/libappindicator3-1).

Se descargó el [`debian.tar.xz` de la fuente exacta de Noble](https://archive.ubuntu.com/ubuntu/pool/main/liba/libayatana-appindicator/libayatana-appindicator_0.5.93-1build3.debian.tar.xz)
desde el archivo oficial Ubuntu y se verificó SHA-256
`90b6052050df827a82dfd7c875d933a828d67cf8c8712a924f415bf7b84e958a` contra su
[`.dsc`](https://archive.ubuntu.com/ubuntu/pool/main/liba/libayatana-appindicator/libayatana-appindicator_0.5.93-1build3.dsc).
`debian/copyright` identifica los fuentes de la biblioteca de runtime (`src/app-indicator.c/.h`)
como LGPL-2.1 o LGPL-3; también lista componentes distintos GPL-3 y helpers BSD. No debe resumirse
todo el paquete fuente con una sola licencia ni confundirse esa lista con las dependencias dinámicas
del ejecutable. El paquete binario instala `/usr/share/doc/libayatana-appindicator3-1/copyright`.
Para el `.deb` de Naygo, todavía se deben conservar los avisos de las bibliotecas realmente
declaradas/enlazadas y probar el cierre final con el paquete binario elegido.

**Evaluación de la alternativa GLib y GPL-3 (2026-10-15):** Nicolás pidió migrar antes de la alfa
porque upstream marca obsoleto el backend GTK y autorizó distribuir la edición Linux de la alfa bajo
GPL-3 si esa migración lo requiere, conservando Windows bajo MIT. El repositorio oficial
[`libayatana-appindicator-glib`](https://github.com/AyatanaIndicators/libayatana-appindicator-glib)
se identifica como GPL-3.0 y su archivo [`COPYING`](https://raw.githubusercontent.com/AyatanaIndicators/libayatana-appindicator-glib/main/COPYING)
contiene la GPL versión 3; en esos materiales no se encontró una excepción de enlace. Según la
interpretación publicada por la FSF, un programa que enlaza una biblioteca GPL forma una obra
combinada cuya distribución debe cumplir GPL; el enlace dinámico por sí solo no evita esa condición
([FAQ de GPL de GNU](https://www.gnu.org/licenses/gpl-faq.en.html#IfLibraryIsGPL)). La GPLv3 requiere
que el trabajo combinado se licencie como un todo bajo GPLv3 y, al distribuir binarios, que se
ofrezca su Corresponding Source por una vía permitida ([texto GPLv3, secciones 5–6](https://www.gnu.org/licenses/gpl.en.html)).

**Conclusión y decisión aprobada:** la excepción Linux-only para GTK LGPL no cubría esta biblioteca
GPL; Nicolás aprobó GPL-3 exclusivamente para la edición Linux de la alfa si la migración GLib lo
requiere. Windows y el proyecto general conservan MIT. Antes de distribuir esa variante se debe
confirmar que los derechos de todos los aportes permiten el alcance, proveer fuente correspondiente,
añadir notices y marcar las licencias por plataforma en repositorio/paquete/documentación. Si no puede
confirmarse alguno de estos requisitos, no enlazar esta biblioteca hasta resolverlo. La asesoría legal
puede aclarar obligaciones concretas. No se cambió la dependencia ni se inició la implementación;
Linux continúa pospuesto mientras se prioriza Windows.

**Disponibilidad por versión LTS (2026-10-15):** la consulta del catálogo oficial de paquetes encuentra
`libayatana-appindicator-glib2` y su paquete de desarrollo en Ubuntu 25.10 y 26.04; en Ubuntu 24.04/Noble
la fuente oficial encontrada es la API GTK antigua (`libayatana-appindicator3-1`) y no aparece el paquete
GLib nuevo. Fuentes: [runtime en Resolute 26.04](https://packages.ubuntu.com/resolute/amd64/libayatana-appindicator-glib2),
[paquete de desarrollo en Questing](https://packages.ubuntu.com/en/questing/libayatana-appindicator-glib-dev) y
[fuente AppIndicator de Noble 24.04](https://packages.ubuntu.com/source/noble/armhf/libayatana-appindicator).
Por decisión de Nicolás del 2026-10-15, Ubuntu 26.04 LTS x86_64 es el mínimo de la alfa; no se
mantendrá Noble 24.04 mediante backport en esta fase. No se debe asumir que el binding GLib pueda
construirse o instalarse en Noble.

**Inventario L-00 del paquete GLib (2026-10-15):** Ubuntu Resolute 26.04 publica el paquete amd64
`libayatana-appindicator-glib2` versión `2.0.1-1build1` en Universe. La ficha del runtime muestra
dependencias directas en `libc6` y `libglib2.0-0t64` (GLib >= 2.80), sin GTK ni dbusmenu; la lista de
archivos entrega `libayatana-appindicator-glib.so.2`. El paquete de desarrollo correspondiente
instala `ayatana-appindicator.h`, el enlace de desarrollo `.so`, `ayatana-appindicator-glib.pc` y
un ejemplo C. Referencias oficiales: [runtime y dependencias](https://packages.ubuntu.com/resolute/amd64/libs/libayatana-appindicator-glib2),
[archivos runtime](https://packages.ubuntu.com/resolute/amd64/libayatana-appindicator-glib2/filelist),
[desarrollo](https://packages.ubuntu.com/resolute/libayatana-appindicator-glib-dev) y
[archivos de desarrollo amd64](https://packages.ubuntu.com/resolute/amd64/libayatana-appindicator-glib-dev/filelist).

Se descargaron sólo para inspección en `target/agent-out/linux-glib-l00/` el `.dsc` y el
`debian.tar.xz` exactos de `2.0.1-1build1` ([tar](https://archive.ubuntu.com/ubuntu/pool/universe/liba/libayatana-appindicator-glib/libayatana-appindicator-glib_2.0.1-1build1.debian.tar.xz),
[`.dsc`](https://archive.ubuntu.com/ubuntu/pool/universe/liba/libayatana-appindicator-glib/libayatana-appindicator-glib_2.0.1-1build1.dsc)); el SHA-256 del tar es
`96a4f2c5622b49be6d5b215e676c54f34c6284d779b27066ca8bc4a070cc9114` y coincide con el declarado
por el `.dsc`. `debian/copyright` del paquete fuente atribuye GPL-3 a la biblioteca C runtime,
incluidos `src/ayatana-appindicator.c/.h`; un archivo de metadatos de documentación es CC0-1.0.
La ficha del paquete expone también el copyright instalado por el binario. Este inventario confirma
que la GPL-3 aprobada aplica al backend seleccionado, pero la revisión final de notices debe usar
el archivo `copyright` del paquete binario de la imagen de build elegida.

**Revalidación oficial del paquete GLib (2026-10-18):** la ficha de
[Ubuntu Resolute 26.04](https://packages.ubuntu.com/en/resolute/libayatana-appindicator-glib2)
confirma la versión `2.0.1-1build1` de Universe y dependencias runtime directas en `libc6` y
`libglib2.0-0t64 (>= 2.80.0)`; su [lista amd64](https://packages.ubuntu.com/en/resolute/amd64/libayatana-appindicator-glib2/filelist)
contiene el soname `libayatana-appindicator-glib.so.2` y el copyright instalado. El paquete binario
es de 70,0 kB instalados; no declara GTK ni dbusmenu como dependencia directa. Se bajaron el
`orig.tar.gz`, su `.asc`, el `.dsc`, el `debian.tar.xz` y el copyright desde las rutas oficiales.
Los SHA-256 del `orig.tar.gz`, `.asc` y `debian.tar.xz` coinciden con los tres valores declarados en
el `.dsc`; el archivo `copyright` descargado confirma GPL-3 para `src/ayatana-appindicator.c/.h`,
GPL-3 para los archivos de código inventariados y CC0-1.0 para `doc/AyatanaAppIndicatorGlib-2.0.toml.in`.
La ficha [publica ambos artefactos de firma](https://packages.ubuntu.com/en/resolute/libayatana-appindicator-glib2),
pero al corte inicial de esta revisión el host no tenía `gpg`/`gpgv`, así que todavía no se había
verificado criptográficamente la firma embebida del `.dsc` ni la firma upstream `.asc`. Los archivos
quedaron en `target/agent-out/linux-glib-l00/` para esa verificación posterior. También se descargó
el `.deb` amd64: su SHA-256
`2d9fbec8ebf70464db426bf339d72ed5f2b2b84416816a1a4672b01486a54a3e` coincide con la ficha oficial.
La inspección del paquete confirma que instala `libayatana-appindicator-glib.so.2.0.0` y el
`copyright` del runtime; este último declara GPL-3 para la biblioteca C. La extracción en Windows
no puede recrear el symlink `.so.2`, aunque el paquete oficial sí lo incluye. La autenticación APT
del `.deb` y la revisión de notices del artefacto Naygo estaban pendientes al corte de esa revisión;
el siguiente apartado registra que luego se cerró la cadena APT. La ejecución en una instalación
Linux sigue pendiente.

**Actualización de verificación criptográfica (2026-10-18):** se validó la firma del `.dsc` con la
clave de Sebastien Bacher; su huella completa coincide con el registro GPG de Launchpad. La firma
detached del `orig.tar.gz` valida con la clave de Mike Gabriel incluida en el keyring activo Debian.
También valida el `InRelease` de Resolute con la clave Ubuntu 2018; su SHA-256 cubre el índice
`Packages.xz`, cuyo stanza del paquete contiene el SHA-256 coincidente del `.deb`. Ubuntu firma
metadatos del repositorio y autentica desde ahí el paquete, en vez de añadir firma OpenPGP individual
al `.deb`. La verificación usó PGPy aislado en `target/agent-out`; el parser no verifica
autosignaturas ni revocaciones, limitación registrada en
[`VALIDACION-L00-AUDITORIA-2026-10-17.md`](../VALIDACION-L00-AUDITORIA-2026-10-17.md). Permanecen
pendientes source correspondiente/notices del artefacto final y la comprobación del runtime en
Ubuntu 26.04 nativo.

`git shortlog -sne --all` muestra 948 commits bajo `nicolasgroth <nicolas_groth@yahoo.com>`, mientras
que `Cargo.toml` y los headers identifican al autor como Nicolás Groth <ngroth@gmail.com>. Antes de
la confirmación no había `.mailmap` y `git check-mailmap` dejaba Yahoo sin mapear. El 2026-10-17
Nicolás confirmó que Yahoo era su alias histórico y que esos 948 commits son sus aportes; queda
resuelta la discrepancia de identidad declarada por el titular. Se añadió `.mailmap` para que Git
muestre esa identidad como Nicolás Groth <ngroth@gmail.com>, sin reescribir commits, y se verificó
con `git check-mailmap`. Esta confirmación no verifica por sí sola el origen/licencia de
código importado ni sustituye la preparación de fuente correspondiente y notices antes de distribuir
Linux bajo GPL-3. El barrido actual de 272 archivos `.rs`, `.slint`,
`.c` y `.h` encontró un identificador SPDX en todos; esto tampoco prueba el origen de código importado
ni sustituye revisar licencias antes de distribuir la variante GPL-3.

Hallazgo de mantenimiento: [el upstream](https://github.com/AyatanaIndicators/libayatana-appindicator)
etiqueta la implementación GTK de `libayatana-appindicator` como obsoleta y recomienda
`libayatana-appindicator-glib`; esa alternativa no ofrece los sonames/API que el binding actual de
`tray-icon 0.24.1` busca. Nicolás ya eligió migrar a GLib antes de la alfa y autorizó GPL-3 Linux-only
si el enlace lo requiere. Esta ruta sustituye la evaluación GTK anterior; todavía hay que implementar
y probar un binding compatible, verificar la dependencia runtime `libayatana-appindicator-glib2` en
Ubuntu 26.04 y asegurar degradación segura cuando la bandeja no esté disponible. No se debe reutilizar
el `libappindicator-sys` actual, que hace `panic!` si no carga los sonames GTK. La matriz confirma
disponibilidad del paquete en Ubuntu 26.04, no que todos los derivados ofrezcan la misma versión ni
que todos sus paneles muestren el indicador.

**Revisión de bindings upstream (2026-10-16):** se inspeccionó el árbol oficial
[`libayatana-appindicator-glib`](https://github.com/AyatanaIndicators/libayatana-appindicator-glib)
en `4fa65d8ce1a7f1a1f22cefe3d5bdd47667f188d0`. `bindings/` sólo contiene bindings Vala; sus
ejemplos C usan `GMenu`/`GSimpleActionGroup`, acordes con el contrato GLib/Gio. El crate Rust
[`libayatana-appindicator`](https://docs.rs/libayatana-appindicator/0.2.0/libayatana_appindicator/)
consultado documenta `GtkMenu` y pertenece a la API GTK obsoleta, por lo que no sirve para esta
migración. No se encontró un binding Rust upstream compatible en el árbol inspeccionado; L-03 deberá
mantener una capa FFI local mínima o generar bindings desde el GIR, con runtime detection y errores
tipados. Esta revisión no modifica dependencias ni habilita la bandeja sin entorno Ubuntu nativo.

**Interoperabilidad de menú GLib, hallazgo 2026-10-16:** el README del upstream dice que la
implementación nueva exporta menús/acciones Gio como `org.gtk.Menus` y `org.gtk.Actions`, en lugar
del protocolo dbusmenu previo. El [issue upstream #89](https://github.com/AyatanaIndicators/libayatana-appindicator-glib/issues/89)
sigue abierto; cuestiona que el menú sea compatible con hosts SNI genéricos y un contribuidor del
proyecto coincide en que los hosts no deberían adoptar ese protocolo alternativo. Es una discusión
del protocolo, no una prueba del comportamiento de Ubuntu 26.04 GNOME. Como GNOME/Wayland es el
escritorio prioritario, antes de cerrar L-03 hay que probar ícono, menú y acciones en una sesión real;
si no aparece, evaluar una implementación SNI estándar antes de declarar compatible la bandeja. La
decisión no cambia la autorización GLib/GPL-3 existente, pero sí agrega una puerta de aceptación.

**Revalidación contra SNI oficial, 2026-10-16:** el [README upstream de Ayatana GLib](https://github.com/AyatanaIndicators/libayatana-appindicator-glib)
describe la exportación de Gio como `org.gtk.Menus`/`org.gtk.Actions`; la [especificación StatusNotifierItem](https://specifications.freedesktop.org/status-notifier-item/latest/status-notifier-item.html)
define `Menu` como objeto `com.canonical.dbusmenu`. Esto confirma una diferencia entre interfaces,
pero no demuestra que una versión concreta de GNOME Shell o un host Ayatana rechace el menú. La
aceptación requiere verificar en Ubuntu 26.04 GNOME/Wayland que se muestren ícono, menú y acciones;
hasta entonces el tray no puede declararse compatible. Se recomendó hacer esa comprobación puerta de
L-03 y se pidió confirmación de alcance a Nicolás.

En la consulta actual, el [issue upstream #89](https://github.com/AyatanaIndicators/libayatana-appindicator-glib/issues/89)
sigue abierto. El autor cuestiona la conformidad con SNI por sustituir dbusmenu; un contribuidor
respondió que los hosts StatusNotifier no deberían adoptar el protocolo alternativo. Esto eleva el
riesgo de que el menú no aparezca en hosts estándar, pero no sustituye la prueba de Ubuntu 26.04.

**Hallazgo adicional L-00/L-03 (2026-10-17):** el [issue upstream #87](https://github.com/AyatanaIndicators/libayatana-appindicator-glib/issues/87)
continúa abierto. Un usuario informa que, con la implementación GLib nueva, Waybar muestra un ícono
de error y no abre menú; IceWM/LXDE no muestran el ícono; y Plasma 6 reserva el espacio, pero no
permite usarlo. Es un reporte individual sin reproducción independiente, por lo que no demuestra un
fallo general ni específicamente en GNOME/Wayland de Ubuntu 26.04. Sí confirma que no basta con
compilar/enlazar el binding: la aceptación de L-03 debe probar creación, visibilidad, menú y acciones
en GNOME/Wayland y al menos un derivado antes de considerar la bandeja compatible. Hasta esa prueba,
GLib sigue siendo la ruta aprobada a evaluar, no una integración validada.

**Decisión de Nicolás, 2026-10-17:** mantener la ruta GLib y probarla en entorno real. El reporte
upstream #87 no cambia por sí solo el backend elegido; icono, menú y ejecución de acciones en GNOME/
Wayland de Ubuntu 26.04 y en al menos un derivado quedan como aceptación funcional de L-03. Si falla
cualquiera de esos hosts, registrar el resultado y revisar el alcance del tray antes de declarar
compatible la alfa.

**Dependencias del backend GTK actual, revalidación 2026-10-16:** `cargo tree` muestra
`tray-icon → libappindicator → gtk → glib-sys/gtk-sys`; el `cargo check` Linux cruzado se detiene en
`glib-sys` por `pkg-config` sin sysroot. El soname AppIndicator puede cargarse dinámicamente, pero
eso no elimina el stack de bindings GTK del grafo de compilación actual. La migración GLib puede
retirarlo sólo si el binding nuevo no vuelve a introducir GTK; el target y paquete nativos deben
confirmarlo. Esto precisa la evaluación previa, que describía únicamente la carga dinámica del
soname AppIndicator.

**Avance local (2026-10-17):** se implementó un binding FFI mínimo a Ayatana AppIndicator GLib con
carga dinámica vía `libloading`, menú Gio y event loop en worker. `tray-icon` quedó condicionado a
Windows. El módulo Rust aislado pasó `cargo check --target x86_64-unknown-linux-gnu --tests --offline`
y Clippy `-D warnings` dentro de un harness temporal; el workspace Windows pasó 1.447 pruebas
(8 ignoradas), Clippy y formato. La UI Linux completa aún no cruza: después de retirar `tray-icon`
del grafo Linux, el check sigue bloqueado por `yeslogic-fontconfig-sys`/`pkg-config` sin sysroot.
El harness no valida ABI FFI, carga de bibliotecas ni comunicación con el host del escritorio.

**Revisión estática de AppIndicator (2026-10-18):** las firmas FFI de sus siete funciones usadas se
compararon con `ayatana-appindicator.h` upstream; también coincide el prefijo de acciones `indicator.`
con el ejemplo C upstream. Alcance y limitaciones en
[`VALIDACION-L03-CROSS-2026-10-17.md`](../VALIDACION-L03-CROSS-2026-10-17.md). Esto no valida los
prototipos GLib/Gio restantes, ABI/runtime, D-Bus ni integración del menú con el escritorio.

**Check cruzado de UI (2026-10-18):** `naygo-ui-slint` ya pasa `cargo check --all-targets` y Clippy
`-D warnings` para `x86_64-unknown-linux-gnu` desde Windows. Se habilitó `fontique-dlopen` solo para
Linux, evitando `pkg-config`/headers Fontconfig en compilación cruzada y permitiendo el fallback de
fuentes si falta `libfontconfig.so.1`. Sigue pendiente comprobar resolución de bibliotecas y
renderizado en Ubuntu nativo.

En la misma revisión se corrigió el cierre doble del source GLib: en HUP/EOF/error el callback pide
salir del loop, pero conserva el source para que su propietario lo quite una vez al volver de
`g_main_loop_run`. El 18 de octubre el harness aislado pasó check/Clippy Linux, y el gate Windows
pasó 1.453 pruebas (8 ignoradas), Clippy y formato. Portable/setup 0.5.2 se regeneraron bajo
supervisor y sus hashes se verificaron; no se instalaron ni firmaron. Sin entorno nativo no se valida
el comportamiento GLib en runtime.

**Cierre pendiente:** compilar la UI y el paquete de forma nativa en Ubuntu 26.04, probar degradación
segura e icono/menú/acciones en GNOME/Wayland y un derivado, completar notices del runtime real y
registrar baseline de memoria/CPU antes de portar integraciones. La ruta GLib y GPL-3 Linux-only
están autorizadas y el mínimo LTS ya está fijado; falta cerrar la validación legal/técnica y confirmar
las combinaciones de escritorio que se certificarán.

**Auditoría local de autoría y licencias (2026-10-17):** `git log --all` presenta un único autor y
committer normalizado por `.mailmap`, Nicolás Groth; la discrepancia Yahoo/Gmail está resuelta con su
confirmación previa, sin reescritura de commits. Un barrido de los 275 archivos de código `.rs`,
`.slint`, `.c`, `.h`, `.cpp` y `.cc` en el árbol encontró SPDX en todos; no encontró rutas
`vendor`, `third_party` o `external` entre los archivos versionados. Esto reduce el riesgo de aportes
de terceros sin atribución en esos archivos, pero no descarta fragmentos copiados, recursos binarios,
ni obligaciones de dependencias. `cargo license --json --avoid-build-deps --avoid-dev-deps
--filter-platform x86_64-unknown-linux-gnu` reporta 543 paquetes; el crate `option-ext 0.2.0`
(`MPL-2.0`) llega por `tray-icon → dirs → dirs-sys`, y Slint 1.16.1 conserva la opción
`LicenseRef-Slint-Royalty-free-2.0` escogida por el proyecto. Al reemplazar `tray-icon` solo en Linux,
se debe volver a generar el inventario para comprobar si MPL-2.0 deja de formar parte del grafo Linux;
si sigue presente, conservar su aviso y las obligaciones sobre modificaciones a archivos MPL. Este
grafo Cargo no incluye las bibliotecas runtime del sistema. La evidencia reproducible y sus límites
están en `docs/VALIDACION-L00-AUDITORIA-2026-10-17.md`. La fuente correspondiente GPL-3, notices
finales y revisión de runtime siguen abiertos antes de cualquier distribución Linux.

## L-01A — identidad y rutas

**Subpaso JSON de rutas (2026-10-17):** `core/path_wire.rs` conserva el JSON string para rutas
UTF-8 y codifica rutas POSIX no UTF-8 como `unix-bytes-v1` con hexadecimal reversible. Se aplica a
favoritos, cestas y referencias de espacios, búsquedas guardadas y puntos de comparación (raíz,
entradas, exclusiones, root resuelto y brechas). Lectores previos siguen aceptando strings; no
reconocen silenciosamente versiones de encoding futuras. El comparador usa identidad nativa también
en las brechas y el validador detecta segmentos `.`/`..` en los bytes originales. Windows conserva
su formato anterior. Los 944 tests core y 22 flujos pasaron en Windows; `cargo check --tests` y
Clippy Linux cruzados compilan las rutas/tests POSIX. El gate de workspace/Clippy pasó (pico 8,53 GiB)
y el portable/setup Windows se regeneraron bajo supervisor (pico 10,48 GiB); los detalles y hashes
están en `docs/VALIDACION-BUILD-2026-10-17-L01A-JSON.md`. Falta ejecución nativa Linux, por lo que
esto no cierra L-01A.

- [x] Subpaso names POSIX: validar sólo separadores/caracteres prohibidos en POSIX, tratar backslash
  como carácter literal, permitir espacios y puntos finales, y preservar bytes nativos al deduplicar.
  El plan de duplicados también conserva nombres no UTF-8 y desambigua colisiones sin reconstruir
  el nombre desde texto. Once tests nativos Windows pasan; los tests Unix compilan para
  `x86_64-unknown-linux-gnu` y falta ejecutarlos en Linux nativo. Esto no cierra L-01A ni habilita
  UI/operaciones Linux.
- [x] Subpaso ZIP: preservar backslash literal al formar nombres internos POSIX y rechazar
  componentes no UTF-8 antes de crear/truncar el ZIP. El gate Windows y check/Clippy cruzados Linux
  pasan; los dos tests Unix compilan, pero faltan ejecutarlos en Linux nativo.
- [x] Subpaso plantillas: captura fija conserva `PathBuf`, y el schema JSON de texto actual rechaza
  explícitamente rutas no UTF-8 antes de alterar el catálogo. La UI muestra un aviso traducido a los
  diez idiomas; rutas UTF-8 conservan el JSON anterior. Tests Windows, i18n y check/Clippy Linux
  pasan; falta la ejecución Linux nativa.
- [x] Subpaso CLI: el arranque usa `args_os()` y mantiene argumentos posicionales como `OsString`/
  `PathBuf`; sólo escapa bytes no UTF-8 al preparar avisos. Flags y valores de tema/plantilla siguen
  siendo UTF-8. Pasaron 19 tests CLI Windows, `cargo check` de UI y check/Clippy cruzados Linux del
  core con tests POSIX. Las rutas POSIX no UTF-8 compilan como test, pero aún requieren ejecución
  Linux nativa. El gate completo y la distribución Windows supervisada pasaron; hashes en
  `docs/VALIDACION-BUILD-2026-10-15.md`.
- [x] Subpaso sesión JSON: `FilePanePersist.current_dir` conserva la cadena histórica para rutas
  UTF-8 y codifica rutas POSIX no UTF-8 como objeto etiquetado `unix-bytes-v1` con bytes hexadecimales.
  El lector sigue aceptando strings antiguos; bytes mal formados o etiquetas desconocidas se rechazan.
  El fingerprint de sesión ahora usa bytes Unix/unidades UTF-16 de la ruta para evitar colisiones
  causadas por `to_string_lossy`. Pasaron 3 tests core Windows y `cargo check -p naygo-core --tests`
  cruzado a `x86_64-unknown-linux-gnu`; el round-trip POSIX y el test del fingerprint requieren
  ejecución nativa, pendiente por WSL. El gate Windows actualizado pasó 1.400 tests y Clippy;
  portable/setup se regeneraron bajo supervisor y el portable abrió/cerró con el backend software.
  Esto no valida round-trip nativo Linux ni cierra L-01A.
- [x] Subpaso presentación y búsqueda por nombre: el listado, el árbol y las dos búsquedas core
  convierten bytes POSIX inválidos a escapes `\\xNN`, duplican backslashes literales y filtran por
  la misma representación visible. La fila mantiene su `PathBuf` nativo y los escapes no se
  reinterpretan como rutas. Pasaron 909 tests core y 22 flujos Windows; `cargo check` y Clippy con
  `-D warnings` compilan los tests Unix para `x86_64-unknown-linux-gnu`. La prueba de búsqueda y los
  tests POSIX del helper aún requieren ejecución Linux nativa. El gate de workspace actualizado pasó
  1.404 tests (8 ignorados), Clippy y formato; la distribución Windows se regeneró bajo supervisor
  con pico Job 10,45 GiB. Se verificaron el manifiesto, el ZIP y la identidad del EXE, pero esta
  regeneración no se instaló. Ver `docs/VALIDACION-BUILD-2026-10-16.md`.

**Archivos:** `core/path_input.rs`, `fs_model.rs`, `ops/names.rs`, `ops/plan.rs`, `ops/undo.rs`,
`batch_rename.rs`, listados, config/formats de espacios/consultas/recetas y bridge.

- Inventariar normalización de separadores, drive letters, case-fold de claves, trimming/comillas,
  `to_string_lossy` operativo y límites UTF-16; contrastar semántica del SO.
- Linux: `/`, rutas relativas, espacios iniciales/finales y `\\`/comillas en nombres válidos
  no se modifican silenciosamente. Separar edición de rutas de sintaxis de listas multilínea;
  exponer claramente nombres que no puedan escribirse en el campo actual.
- Conservar bytes no UTF-8 en identidad/listing/operaciones. Auditar JSON, CLI, clipboard/URI
  y request de preview: elegir codificación etiquetada/versionada para formatos que lo requieran,
  o rechazo explícito de guardado/importación sin pérdida. No cambiar esquema sólo por conveniencia.
- Distinguir ocultos por dotfile, atributos de sistema inexistentes y filtros configurados;
  mantener compatibilidad Windows sin introducir flags de Linux en Win32.
- Política compartida de comparación sin claves lossy; pruebas puras más pruebas físicas.
  Unknown no autoriza sobrescribir un objeto que sólo parece el mismo.

**Avance parcial:** `path_input::normalize` conserva en POSIX espacios iniciales/finales, comillas y
backslash, sin aplicar el límite UTF-16 de Windows. Un helper de identidad léxica ahora conserva
case y bytes nativos en Unix en guards de rename/duplicate/undo, sync, task spaces, saved search,
retorno a selección, cesta, planificación de entregas y radar de destinos; Windows conserva el
criterio case-insensitive previo. La planificación de entregas también comprueba solapamiento por
componentes nativos. El preview de renombrado por lotes muestra los bytes inválidos escapados y
marca esos nombres como no editables para no convertirlos silenciosamente en `U+FFFD`; las rutas
UTF-8 válidas conservan su nombre exacto, incluyendo backslash POSIX. No resuelve identidad
física ante symlinks/cambios concurrentes ni activa operaciones/UI Linux. El 2026-10-15 la
planificación de copy/move y su guarda antes de borrar dejaron de aplicar case-fold global en Unix:
comparan igualdad y pertenencia por componentes nativos, mientras Windows conserva su criterio.
Un fixture POSIX comprueba una copia entre directorios `A` y `a`. Pasaron 19 tests dirigidos del
plan en Windows y el check cruzado compila tests Unix; estas pruebas aún requieren ejecución nativa.
El 15 de octubre, `ops::names` dejó de aplicar restricciones de nombres Windows al validar operaciones
POSIX: sólo rechaza vacío, `.`/`..`, `/` y NUL, y divide rutas relativas sólo por `/` para conservar
backslash literal. `dedup_name` ahora compone el sufijo con `OsStr` y conserva bytes no UTF-8. Pasaron
11 tests nativos Windows y `cargo check --target x86_64-unknown-linux-gnu -p naygo-core --tests
--locked`; los tres nuevos tests Unix compilan en ese target, pero no se pudieron ejecutar porque WSL
no inicia en este host. Esto sigue siendo core compartido: no habilita operaciones ni UI Linux.
La planificación de `Duplicate` ya usa `OsStr`/`OsString` para formar ` - copia` y resolver colisiones,
así que una fuente POSIX con bytes no UTF-8 conserva su nombre; un test Unix cubre el caso con una
colisión previa. El test Windows de colisiones existente y `cargo check`/Clippy cruzados de core
para Linux pasan. La prueba POSIX aún requiere ejecución nativa.

Las propuestas de nombres de grupo de `delivery::suggested_group_names` usan ahora
`path_display::display_os_str`: los bytes POSIX no UTF-8 se muestran como `\xNN` y los backslashes
literales se escapan por separado, sin sustituir ni confundir el nombre operativo. El test Unix
incluye ambos casos y pasa `cargo check`/Clippy cruzados Linux; su ejecución nativa sigue pendiente.
La guía de entregas explica que esta etiqueta es sólo una representación visible.

El 15 de octubre, `archive_ops::collect_entries` dejó de convertir nombres ZIP con `to_string_lossy`
y de reemplazar cada backslash por `/`. Ahora une componentes de ruta con `/`, preservando el
backslash que forma parte de un nombre POSIX; si un componente no es UTF-8, rechaza la compresión
durante el inventario, antes de crear o truncar el ZIP destino. Dos tests Unix verifican ambos
comportamientos y compilan con `cargo check --target x86_64-unknown-linux-gnu -p naygo-core --tests
--locked`; WSL no está disponible para ejecutarlos nativamente. Esto define una limitación explícita
de la API ZIP actual: nombres de miembros no UTF-8 no se sustituyen ni se comprimen. No habilita UI
ni operaciones Linux.

El 15 de octubre, la captura de plantillas dejó de convertir las rutas fijas del workspace a texto
lossy. `TemplateDir::Fixed` transporta `PathBuf` y mantiene la representación JSON de string para
rutas UTF-8; antes de guardar, `Workspace::to_template` rechaza con error tipado cualquier ruta que
no pueda representarse en UTF-8. El controlador no añade la plantilla al catálogo y la UI presenta
un mensaje localizado en sus diez idiomas. Pasó el test UI de guardar/aplicar/borrar plantilla,
110 tests workspace, `cargo check --target x86_64-unknown-linux-gnu -p naygo-core --tests --locked`,
Clippy cruzado y el checker i18n. La prueba POSIX compila por `cargo check`; `cargo test --no-run`
no pudo enlazar el target porque no existe `cc` en este host Windows. El gate completo y la
distribución Windows supervisada pasaron; no se cambió el esquema JSON ni se habilita UI/operaciones
Linux.

**Pruebas:** `a` y `A` en ext4, Unicode distinto sin normalización implícita, byte no UTF-8,
espacios/comillas/backslash, nombre con salto de línea, symlinks, `..`, carpeta padre parecida,
Windows UNC/prefijos extendidos sin regresión. Operaciones destructivas sobre los originales
no se habilitan hasta L-01B.

## L-01B — operaciones nativas

**Archivos:** `core/ops/plan.rs`, `engine.rs`, `undo.rs`, `fs_util.rs`, motores de archivos/entregas.
Dividir en entregas de copy/move, rename/batch y undo según evidencia.

- Confirmar overwrite/no-clobber, reserva atómica de nombres, renombrado sólo de caja, hardlinks,
  symlinks (copiar vínculo vs destino según contrato), reparse Windows y movimiento entre FS.
- Detectar EXDEV y usar copia + retirada de origen sólo tras confirmar éxito; cancelación limpia
  únicamente el parcial propio y conserva origen/destino previos. No reemplazar destino existente
  para simular `rename` Windows.
- Mantener permisos ejecutables cuando corresponda; documentar alcance de owner/ACL/xattrs,
  archivos especiales y sparse. No prometer clon completo de metadata por pasar tests básicos.
- Aplicar política de identidad de L-01A a rename/undo/anti-recursión; revalidar ante cambios
  externos entre revisión y ejecución. Undo no pisa un archivo nuevo del mismo nombre.

**Avance local, 2026-10-16 (parcial):** las copias de archivos ahora escriben a un temporal
creado junto al destino y publican al final. Cancelación/error elimina únicamente ese temporal,
conservando el origen y el destino previo; la publicación no-clobber detecta destinos creados
durante la copia. En movimientos cross-volume se retira el origen sólo tras publicar, Linux usa
`RENAME_NOREPLACE` para el movimiento rápido y se conserva el modo POSIX. Pasaron 916 tests del
core, los 22 flujos Windows, el gate de 1.408 tests, Clippy, formato y check Linux cruzado del
core con tests. La build Windows 0.5.2 fue regenerada el 16-10; hashes en
`docs/VALIDACION-BUILD-2026-10-16.md`.
Con autorización previa, esa build se instaló in-place con símbolos desmarcados; el PDB antiguo
se conservó y Naygo quedó abierto y respondiendo.

**Hallazgo adicional Windows, 2026-10-16:** la expansión preorder de Move copiaba/movía los
archivos, pero no retiraba las carpetas de origen que quedaban vacías. El plan de Move ahora coloca
los pasos de directorio en postorden y el motor quita cada origen sólo con `remove_dir`: carpetas
con contenido omitido/fallido se conservan y symlinks/junctions no se siguen ni se retiran. La
cancelación evita empezar la limpieza de directorios pendientes. La regresión de mover carpetas y
la de conservar carpetas con archivos omitidos pasan.

**Carrera de destino Windows, 2026-10-16:** Nicolás aprobó una interfaz mínima core↔platform para
inyectar el rename sin reemplazo. `core` conserva su independencia de Win32; Naygo UI provee el
adaptador, y `platform` llama `MoveFileExW` sin `MOVEFILE_REPLACE_EXISTING` ni
`MOVEFILE_COPY_ALLOWED`. Un destino ocupado después de planificar produce error y conserva ambos
archivos; el error de volumen distinto vuelve al fallback cancelable de copia/retirada. Pasan dos
tests Win32, dos regresiones de flujos UI y las pruebas del motor con destino concurrente y fallback
cross-volume. Gate workspace 1.417/8, Clippy, formato, diff check, check/Clippy cruzados Linux y
distribución supervisada 0.5.2 pasaron; hashes y alcance en `docs/VALIDACION-BUILD-2026-10-16.md`.
La build no se instaló.

**Recuperación de Replace Windows, 2026-10-16:** `Replace`
ahora escribe el árbol completo en un staging hermano del destino. Cancelar/fallar durante la copia
limpia sólo el temporal; la publicación aparta el destino anterior, publica con rename sin reemplazo
y trata de restaurarlo ante una colisión. Si no puede restaurar, conserva e informa el respaldo.
En Move el origen se mantiene durante staging, se publica primero y luego sólo se retiran los
archivos cuyo contenido coincide con lo publicado; las carpetas se quitan si quedan vacías. El
journal reinicia desde la raíz si la operación se interrumpió antes de publicar. Un registro
validado queda persistido antes del staging y un sentinel dentro del árbol publicado distingue
ambos lados de los renames. Antes de listar la carpeta o su padre, el worker restaura el destino
anterior si falta la ruta y el staging no se publicó; si la marca confirma publicación, limpia el
respaldo. La recuperación es cancelable, evita destinos concurrentes y valida nombres/rutas antes
de tocar el filesystem. Un proceso hijo terminó inmediatamente después de cada rename y se comprobó
la recuperación en el proceso padre; también se probaron caída durante staging, colisión y operación
activa. El gate protegido pasó con 1.433 pruebas, 8 ignoradas, Clippy, formato y diff check; los
checks y Clippy cruzados Linux de core/platform pasan. La distribución Windows 0.5.2 se regeneró bajo
supervisor (pico 10,45 GiB); hashes y verificación en `docs/VALIDACION-BUILD-2026-10-16.md`. No se instaló.
No se ha probado pérdida de energía. Windows no tiene un intercambio
atómico de directorios mediante estas APIs: puede haber una ventana breve sin la ruta entre los dos
renames; la recuperación se inicia al listar el padre después de reiniciar Naygo.

Esto no cierra L-01B: siguen faltando aceptación manual de Replace, casos de disco lleno/permisos,
movimiento entre volúmenes, recuperación tras pérdida de energía y la matriz Linux nativa.

**Pruebas:** dos archivos `a/A` preservados, renombrado a ocupado, cancelación de copia grande,
EXDEV, disco lleno, permisos, enlace roto/ciclo, destino cambiado antes de undo, nombres
no UTF-8 y modo ejecutable. Tests Windows del mismo motor siguen aprobados.

**Hallazgo Windows/Linux, 2026-10-18:** `expand()` descartaba errores individuales del iterador
`ReadDir` con `flatten()`, lo que podía producir una copia/movimiento incompleto sin indicar qué
carpeta no se pudo enumerar. Ahora cada error se devuelve como `PlanError::SourceUnreadable` con la
ruta de la carpeta y la planificación falla antes de ejecutar pasos; el borrado recursivo usa la
misma conversión. La prueba inyecta `PermissionDenied` en una entrada y verifica el error tipado.
Pasaron las 20 pruebas de `ops::plan`, el gate workspace (1.453 aprobadas, 8 ignoradas), Clippy,
formato y checks/Clippy cruzados Linux de core/platform. La distribución Windows se regeneró bajo
supervisor; ZIP/setup se verificaron y no se instalaron. Evidencia:
`docs/VALIDACION-READDIR-2026-10-18.md`. L-01B continúa parcial por pruebas manuales de permisos,
energía y volúmenes, además de aceptación Linux nativa.

**Cobertura adicional, 2026-10-18:** se añadió una regresión Unix para asegurar que reorientar un
symlink usado como carpeta destino invalida la revisión antes de reintentar. En Windows pasaron las
tres pruebas de reintento disponibles; `cargo check -p naygo-core --tests --target
x86_64-unknown-linux-gnu --locked` también pasó e incluyó la prueba Unix. La ejecución de esa prueba
requiere runtime Linux y queda pendiente; no cambia el alcance de la aceptación manual de L-01B.

**Carrera al apartar el destino, 2026-10-18:** «Renombrar antiguo» elegía una ruta libre y usaba
`std::fs::rename`; en POSIX ese rename reemplaza el destino si otro proceso lo crea entre la
comprobación y el rename. Ahora usa `NoReplaceRename`, incluida la implementación Win32 inyectada
por la UI y el rename `RENAME_NOREPLACE` de Linux. Un conflicto concurrente deja intactos el destino
original, la fuente nueva y el archivo externo. La regresión dirigida pasó en Windows; falta la
ejecución nativa Linux. El gate workspace del 2026-10-18 pasó con 1.456 pruebas aprobadas y 8
ignoradas, Clippy sin avisos y formato correcto. El build supervisado del mismo día terminó con pico
10,53 GiB y hashes verificados; no se instaló. Evidencia: `docs/VALIDACION-L01B-RENAME-2026-10-18.md`.

## L-02 — ubicación de datos

**Avance parcial L-02, 2026-10-16:** el arranque de producción ya resuelve `UserDirs` sin I/O:
Linux consume `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`, `XDG_STATE_HOME` y `HOME`; los
valores XDG relativos se ignoran y un `HOME` ausente/no absoluto detiene el inicio con un diálogo.
`settings.json`, workspace, favoritos, atajos y catálogos continúan bajo `config/naygo`; el logger
usa `state/naygo`. Crear directorios y preparar/truncar el log corre en sus workers. Windows conserva
los cuatro directorios junto al EXE portable y tiene test de regresión. `resolve_home_dir` usa `$HOME`
en Linux en vez de `USERPROFILE`. Revisión de consumidores encontró que `main`, reintentos de rutas
perdidas y varios paneles de respaldo aún usaban `USERPROFILE`/`C:/`; ahora llaman al resolver Home
compartido y respetan también la preferencia `home_dir`. Un test de controlador cubre reubicar una
carpeta perdida y crear paneles sin Files activo. Pasaron cuatro tests dirigidos de `UserDirs`, el
worker de logs y el test del controlador; `cargo check` y Clippy `-D warnings` cruzados de
core/platform pasaron antes de este delta. El intento de check de la UI
cruzada se detuvo en `glib-sys`: este Windows host carece de `pkg-config` con sysroot Linux. Falta
compilar y ejecutar en un Linux nativo. El delta pasó el gate Windows completo (1.399 tests, 7 ignorados,
Clippy, formato y `git diff --check`) y se incluyó en el portable/setup 0.5.2; el smoke portable creó
sus archivos aislados y cerró correctamente. Esto valida el comportamiento Windows, no el port Linux.
Los sets y PNG de usuario nuevos se guardan bajo `XDG_DATA_HOME/naygo/icons`; la configuración,
temas e idioma siguen bajo `XDG_CONFIG_HOME/naygo`. Catálogos y resolución leen primero desde data
y consultan config como ubicación heredada. Si el mismo set o PNG existe en ambos, gana data.
Importar un pack escribe sus JSON a config y sus íconos a data; exportar temas incorpora íconos
de ambas ubicaciones con la misma precedencia. No se mueven ni borran archivos antiguos. En Windows,
`data_dir == config_dir`, así que se conservan las rutas portables existentes. Tests dirigidos del
core (13) y packs UI (8) pasan; el check y Clippy cruzados del core Linux con `-D warnings` pasan.
Se regeneró la distribución Windows y se actualizó la instalación local; hashes y límites están en
`docs/VALIDACION-BUILD-2026-10-16.md`. Validación UI Linux nativa sigue pendiente. L-02 continúa
abierto hasta validar el arranque/flujo completo en Linux y revisar los demás consumidores XDG.

**Auditoría estática de consumidores, 2026-10-18:** la búsqueda en código de producción no halló
lecturas de `USERPROFILE` fuera del fallback condicionado a plataformas no Linux; los usos restantes
son comentarios. Las cachés de preview, íconos y datos de disco son estructuras en memoria: no hay
un consumidor de caché persistente. `UserDirs` resuelve `XDG_CACHE_HOME` para mantener el mapa XDG
completo, pero no crea ni escribe esa carpeta mientras no exista una caché de disco. Esto evita
introducir I/O de arranque o archivos que el producto no necesita.

**Importación de configuración, avance local 2026-10-18:** al importar un `.naygoconf`, el worker
filtra antes de escribir los ajustes cuya ruta no es absoluta para el sistema destino. En Linux
también desactiva autoinicio y atajo global si el pack los tenía activos, porque esas integraciones
aún no están disponibles; el modal enumera los campos omitidos en el idioma importado. Settings
comunes y keybindings se conservan. El formato de pack no incluye terminal. Los tests cubren
rutas Windows/POSIX, valores omitidos, preservación de atajos y filtrado previo a escritura. La
ejecución nativa Linux y la distribución pendiente quedan registradas en el backlog.

**Archivos:** `core/config/`, arranque/session, logging, stores y scripts de launch.
Instalación Linux usa XDG para config/datos/caché/estado según naturaleza; home desde entorno
nativo. No escribir junto a `/usr/bin/naygo` ni caer silenciosamente al cwd arbitrario.
API de ubicaciones injectable en tests; workers para crear/leer/escribir directorios.
[Especificación XDG](https://specifications.freedesktop.org/basedir/latest/).

Windows conserva comportamiento portable. Un modo Linux portable requiere selección explícita,
no detectar una carpeta escribible al azar. Import de config Windows revisa rutas/terminales/atajos
incompatibles y muestra qué se omite; nunca convierte `C:\\` a `/` como si fueran equivalentes.

**Pruebas:** EXE en ruta de sólo lectura, variables XDG ausentes/relativas inválidas, home no
escribible, config corrupta/futura, cierre/guardado concurrente y importación sin alteración original.

## L-03/L-04 — compilar y navegar

**L-03, archivos:** manifiestos, build.rs, `platform/src/*` stubs, `main.rs`, tray, win_helpers,
hooks de ventana y tests. Compilar nativamente con dependencias fijadas; registrar errores nuevos
desde esta base. Una feature opcional no debe enlazar GTK/AppIndicator sólo por compilar el core.
Introducir capacidades por operación (incluido restore trash, hotkey, tray, preview protegido),
defaults honestos y textos i18n. No exigir HWND en navegación ni despertar continuo por un stub.

**L-04, archivos:** controladores de navegación/listing/tree/input, rutas Slint y clipboard de texto.
Abrir `/home`, `/`, montajes y rutas relativas; teclado, Ctrl+L/F4, autocompletado cancelable,
selección/filtros/historial/paneles, fuentes de sustitución Linux y renderer software forzado.
Ventana sigue operable ante directorio inaccesible; iniciar sin bandeja disponible no la oculta.
Mantener acciones no implementadas deshabilitadas con explicación; no hacerlas no-op exitosas.

**Pruebas:** modelo/core en Ubuntu; UI X11 y Wayland reales donde disponibles, IME/teclado,
DPI/escalado fraccional, rutas Unicode/no UTF-8, permisos y árbol grande. Xvfb sirve para X11
automatizado, no valida portales ni interacción Wayland.
**Cierre:** binario alfa puede abrir/navegar; una prueba de ventana no habilita preview/operaciones.

**Revalidación del entorno y cross-check, 2026-10-16, actualización 2026-10-17:** `rustup` tiene instalado
`x86_64-unknown-linux-gnu`. `cargo check --target x86_64-unknown-linux-gnu -p naygo-core
-p naygo-platform --tests --locked --jobs 1` y Clippy cruzado con `-D warnings` pasan. El check
inicial de `naygo-ui-slint` fallaba en `glib-sys 0.18.1` porque `tray-icon` arrastraba GTK y
`pkg-config` no estaba configurado para cross-compilation. Tras condicionar `tray-icon` a Windows
y añadir el binding dinámico, el check completo avanza hasta `yeslogic-fontconfig-sys`, que requiere
`pkg-config`/sysroot Linux. Un harness aislado sí type-checkea el módulo GLib, pero no su ABI/runtime.
`wsl.exe -d Ubuntu` devuelve `HYPERV_NOT_INSTALLED` y pide habilitar Virtual Machine Platform/
virtualización; no se tocaron características del sistema. Estos checks no prueban ejecución POSIX
ni UI y no cierran L-01A/L-03.

El binding GLib local pasa typecheck y Clippy en un harness aislado, y el cross-check completo de la
UI pasa desde Windows mediante `fontique-dlopen`. La ejecución y el build nativos Ubuntu siguen
pendientes: el host actual no ofrece WSL funcional ni otra instalación Linux, y no se instalaron
paquetes ni se cambió su configuración.

**Revalidación cruzada después de W-06D, 2026-10-19:** `cargo check --workspace --tests --locked
--offline --target x86_64-unknown-linux-gnu --jobs 1` pasó. Clippy con `--all-targets -D warnings`
pasó en los tres crates al ejecutarlo por paquetes (`naygo-core`, `naygo-platform` y
`naygo-ui-slint`). Un intento agregado del workspace se quedó sin `rustc` ni salida y se canceló;
la repetición por paquetes terminó. Esto amplía la evidencia de compilación/lints cruzados de la
base W-06D, no certifica ABI, runtime GLib/AppIndicator, ext4, portal, tray ni interacción en Ubuntu.

## L-05 — abrir y seleccionar

**Archivos:** `platform/open.rs`, known_folders/locale/time, dialogs y callbacks.
Abrir archivo con programa por defecto mediante integración desktop/portal; construir argumentos
sin shell ni interpolación. Archivo que empieza por `-` no se interpreta como opción. «Abrir con»
requiere capacidad comprobada. Terminal Linux con comando configurado seguro, no un enum de
PowerShell convertido a `/bin/sh -c`. `run as administrator` no se sustituye por sudo silencioso.
[Portal OpenURI](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.OpenURI.html).

**Pruebas:** argv/URI con espacios, comillas, `%`, newline y Unicode; programa/portal ausente,
rechazo/cancelación del selector, terminal no instalado y TZ/locale. No ejecutar scripts del corpus.

**Subpaso L-05 implementado localmente (2026-10-18):** la acción Windows «Ejecutar como
administrador» queda disponible sólo en Windows. En otras plataformas, incluida Linux, la
comprobación de capacidad devuelve `false`, por lo que la UI no ofrece una elevación que luego
fallaría como `NotSupported`; no se intenta sustituirla por `sudo`. El test POSIX compila al cruzar
la plataforma. Esto no cierra L-05: «Abrir con», terminal seguro y validación interactiva
permanecen pendientes. La sesión Windows sólo puede validar el test Windows; no hubo ejecución
Linux nativa.

**Subpaso L-05 implementado localmente (2026-10-18):** Enter/doble clic en Linux lanza `xdg-open`
para que el escritorio use la aplicación predeterminada. Naygo crea argv directamente, convierte
rutas relativas en absolutas sin `canonicalize`, conserva los bytes `OsStr` y no pasa por shell;
la creación del proceso retorna sin esperar a la aplicación externa. Pruebas cubren nombres con guion
inicial, espacios, comillas, `%`, salto de línea, Unicode, rutas absolutas y bytes no UTF-8. Dos
pruebas de construcción de argv se ejecutaron en Windows; check/Clippy cruzados compilan todos los
targets Linux, incluido el caso no UTF-8, pero no ejecutan el binario. La integración real con
xdg-utils/portal, asociación ausente y errores posteriores al `spawn` requieren Ubuntu 26.04 nativo.
L-05 sigue abierto: «Abrir con», terminal seguro y aceptación interactiva permanecen pendientes.

## L-06 — montajes y vigilancia

**Archivos:** drives, drive_space, device_watch, dir_watch, dnd y footer/dispositivos de UI.
Representar volumen/mount point estable sin drive letter. Parsear mountinfo en worker, escapes,
bind mounts y prefijo más largo correcto; espacio disponible para usuario y filesystem real.
Determinar «mismo filesystem» físicamente en worker para sugerencia de mover; Unknown sugiere
conservadoramente y el motor maneja EXDEV. No escanear `/sys` ni mountinfo en cada frame.

Validar notify/inotify y reconciliación tras overflow/sin soporte. Cambios de montaje por servicio
del sistema o evento disponible; sin indexación/sondeo permanente de red. Red v1 usa ubicaciones
ya montadas, sin credenciales guardadas ni montaje automático. Detectar externo/removible sin
equiparar USB exclusivamente a `removable=1`.

**Pruebas:** mounts con espacios, mount anidado/bind, USB con varias particiones, lectura espacio
fallida, montaje perdido, watcher tardío y generaciones. Omitir pseudofs del listado de unidades
no implica prohibir navegación explícita hacia ellos.

Avance parcial 2026-10-19: `platform::drive_space::read_space` usa `statvfs` en Linux, consulta
`f_bavail` para el espacio disponible al usuario y pasa la carpeta actual desde el footer; así el
kernel selecciona el filesystem de volúmenes montados y no se consulta siempre `/`. El worker
existente conserva la regla de no bloquear el hilo UI; Linux limita la caché a 128 carpetas y a ocho
lecturas simultáneas. Los tests para esos límites pasan en Windows. El smoke test Linux compila con
`cargo check --workspace --tests --target x86_64-unknown-linux-gnu` y Clippy cruzado sin warnings;
falta ejecutarlo en Ubuntu 26.04. Esto no completa L-06: enumeración de montajes, decodificación de
`mountinfo`, watchers, dispositivos, invalidación al cambiar espacio y la matriz de mounts siguen
pendientes.

**Parser de mountinfo, preparación 2026-10-19:** se añadió `platform::mountinfo::parse_mountinfo`,
un parser de `BufRead` que conserva en bytes root, mount point y source, decodifica únicamente los
escapes documentados, conserva los desconocidos y omite líneas inválidas sin ocultar registros
válidos. Seis pruebas Windows cubren campos opcionales, bind mounts, escapes, bytes no UTF-8,
overflow y recuperación ante líneas corruptas; pasaron el gate workspace (1.474 tests, 8 ignorados),
Clippy/formato y check/Clippy cruzados Linux de platform. El parser es puro y aún no se conecta a
`drives()` ni a la interfaz: leer `/proc/self/mountinfo` debe hacerse desde un worker para respetar
la regla de no I/O en UI. No se afirma enumeración Linux disponible ni validación nativa. Registro:
[`VALIDACION-L06-MOUNTINFO-2026-10-19.md`](../VALIDACION-L06-MOUNTINFO-2026-10-19.md).

**Enumeración local L-06, 2026-10-19:** `platform::drives::drives()` lee y parsea
`/proc/self/mountinfo`; el refresco ya existente la ejecuta en un worker. Se omiten pseudofs de la
tira rápida, se incluyen filesystems de red conocidos, los puntos apilados se consolidan, y las
rutas se convierten desde bytes nativos sin perder nombres no UTF-8. Montajes desconocidos usan
`DriveKind::Unknown`; Linux no ofrece expulsión hasta que una sonda independiente confirme que un
medio es removible. Gate Windows: 1.477 tests aprobados/8 ignorados, Clippy y formato; política
probada en Windows, y check/Clippy cruzados Linux de platform aprobados. Esto no valida `/proc` ni el
comportamiento de la barra en Ubuntu: runtime, clasificación de removibles, watchers e invalidación
siguen pendientes. Detalle en
[`VALIDACION-L06-MOUNTINFO-2026-10-19.md`](../VALIDACION-L06-MOUNTINFO-2026-10-19.md).

## L-07 — papelera

**Archivos:** `platform/trash.rs`, `ops_ctrl/batch_delete.rs`, recibos y recuperación/undo.
Implementar/usar backend conforme a FreeDesktop Trash con licencia/API verificadas; recibo real
por archivo, preservación de ubicación original y comprobación de resultado. Papelera del FS
correspondiente y restore con conflicto. Permisos/symlinks de directorios trash se validan.
No hacer borrado permanente cuando papelera falla. Para lotes, reportar completados/fallidos
individuales y cancelación entre elementos; no prometer reversión de lo ya enviado.
[Especificación de Papelera](https://specifications.freedesktop.org/trash/latest/).

**Pruebas:** homónimos, otra partición, papelera no disponible, restore ocupado, errores parciales,
cancelación, permisos y nombre no UTF-8. Tests destructivos sólo en filesystem de prueba.

**Avance local 2026-10-19:** el backend Linux implementa `Trash/files` + `Trash/info`, `Path` con
percent-encoding de bytes nativos y fecha local. En el mismo filesystem mueve a `.Trash/$uid` si
el directorio compartido es real y tiene sticky bit; en caso contrario intenta `.Trash-$uid`.
Comprueba ownership/permisos de los directorios privados, evita symlinks en directorios finales,
resuelve `..` con el filesystem incluso tras symlinks, conserva el enlace final, usa publicación
`renameat2(RENAME_NOREPLACE)` para no sobrescribir y conserva `trashed_path` en el
historial de Deshacer con compatibilidad de lectura para entradas antiguas. Lotes producen recibos
y fallos individuales, atienden cancelación entre elementos y usan fallback `.Trash-$uid` si el
directorio compartido es un symlink. El check de tests y Clippy cruzados
Linux pasaron; el gate Windows pasó 1.478 tests, Clippy y formato. Los tests de filesystem temporal
compilan, pero el host Windows no puede ejecutarlos ni verificar runtime, otras particiones, permisos
efectivos o integración de escritorio. La especificación permite rechazar el envío cuando una
Papelera local no se puede usar; Naygo no borra permanentemente ni copia entre volúmenes como fallback.

## L-08 — instancia única

**Archivos:** `platform/single_instance.rs`, arranque y canales de solicitudes.
Lock/IPC por usuario en runtime dir seguro, mensajes de ruta acotados con identidad nativa.
Secundaria solicita abrir sin tocar la config; primaria despierta vía event loop. Comprobar peers
y permisos, socket obsoleto y muerte de primaria. Limitar comandos/longitud; no ejecutar strings.
Separar instancia de pruebas de sesión personal mediante directorio/identidad propia.

**Pruebas:** carreras de arranque, secundaria con ruta, proceso muerto, mensaje corrupto,
permiso/runtime dir ausente, dos usuarios y cierre con solicitudes pendientes.

**Avance local 2026-10-19:** Linux usa `XDG_RUNTIME_DIR` sólo si es absoluto, directorio real 0700
y propiedad del UID efectivo. `File::try_lock` arbitra carreras; la primaria limpia únicamente un
socket obsoleto, enlaza Unix socket 0600 y conserva el lock durante su vida. El listener vive en un
worker, comprueba `SO_PEERCRED`, limita el frame a 4.096 bytes y lleva rutas POSIX como bytes sin
convertirlas a texto ni consultar el filesystem en el hilo UI. El tick drena una cola acotada de
rutas; `--shutdown` llega por el mismo IPC. El worker se despierta y termina al soltar el guard.
Hay tests para permisos/runtime dir, lock exclusivo/socket stale, shutdown, ruta no UTF-8, mensaje
malformado y wake+ruta end-to-end. `cargo check --tests` y Clippy `-D warnings` cruzados para
Linux, además del gate Windows completo, pasan. WSL no inicia (`HCS_E_HYPER_V_NOT_INSTALLED`), por
lo que esos tests Linux sólo compilan y el runtime L-08 sigue sin aceptación; no se marca como
cerrado. Los mensajes pendientes se limitan a ocho y se descarta el más antiguo si llega una ráfaga.

**Cierre de trabajo local y paquete Windows, 2026-10-19:** gate Windows repetido: 1.480 tests
aprobados, 8 ignorados, Clippy y formato verdes. Checks cruzados Linux de `naygo-platform` y
`naygo-ui-slint` pasan. El release supervisado tomó 41m23s; Inno Setup, 199,766 s; pico Job
10,66 GiB, bajo el límite de 12 GiB. Se regeneraron ZIP y setup 0.5.3. La instalación autorizada
se actualizó y se dejó abierta/respondiendo como PID 50260; el EXE instalado coincide con Release.
Los cuatro JSON de usuario y `naygo.pdb` conservaron sus hashes. Checksums y límites en
[`../VALIDACION-L08-INSTANCE-2026-10-19.md`](../VALIDACION-L08-INSTANCE-2026-10-19.md). La ejecución
nativa de los tests y validación del IPC siguen pendientes; la compilación cruzada no las sustituye.

## L-09A/B — clipboard y arrastre

**L-09A, archivos:** platform clipboard y controlador de copia/corte. Interoperar con
`text/uri-list` y formatos desktop copy/cut que realmente soporte el backend elegido. Percent
encoding correcto y validación de esquemas; no ejecutar URIs ni tratarlas como rutas locales.
Propiedad de clipboard vive con sesión; copiar referencias internas puede funcionar aunque
no haya portapapeles de archivos externo, pero debe comunicarse. Corte se consume al éxito
real del motor; historial de Naygo sigue efímero.
**Pruebas:** ida/vuelta con Nautilus y Dolphin/derivado elegido, espacios/Unicode/bytes nativos,
clipboard vacío/ajeno, aplicación emisora cerrada y cancelación durante paste.

**Primer subpaso L-09A, 2026-10-19:** `core::clipboard::uri_list` ahora codifica y decodifica
`text/uri-list` como bytes, preserva nombres POSIX no UTF-8 y espacios/Unicode, admite sólo
autoridad local vacía/`localhost`, y rechaza esquemas no-file, URI remotas, escapes inválidos,
NUL, controles, delimitadores ambiguos y barras codificadas. Nueve pruebas unitarias pasan en
Windows. El codec es puro: aún no conecta el portapapeles del escritorio, ni Nautilus/Dolphin, ni
el estado copy/cut; L-09A/B sigue incompleto y no se anuncia como función disponible. WSL no permite
ejecutar las pruebas nativas aquí. Revalidación 2026-10-20: gate completo Windows **1.492 aprobadas,
8 ignoradas**, Clippy y formato verdes; `cargo check -p naygo-core --target
x86_64-unknown-linux-gnu --locked` pasa. Esto demuestra compilación cruzada del codec, no ejecución
nativa; evidencia en [`../VALIDACION-L09A-URI-LIST-2026-10-20.md`](../VALIDACION-L09A-URI-LIST-2026-10-20.md).

**Segundo subpaso local, 2026-10-20:** `core::clipboard::desktop_formats` añade los formatos
copy/cut de Nautilus (`copy|cut\n` + URI(s), sin LF final) y el estado de corte de Dolphin
(`application/x-kde-cutselection`, `0`/`1`). Se probaron rutas con espacios, ambos estados,
acciones inválidas y payloads ambiguos; el codec además limita la lista a 16 MiB, 100.000 rutas y
64 KiB por ruta. Pasan las 34 pruebas focalizadas, `cargo check --tests` y
Clippy cruzados Linux del core con `-D warnings`; no se ejecutaron en escritorio nativo. Evidencia
en [`../VALIDACION-L09A-FORMATOS-2026-10-20.md`](../VALIDACION-L09A-FORMATOS-2026-10-20.md).
El transporte, la vida del propietario del clipboard y el consumo de corte tras éxito siguen
pendientes; no se habilita todavía una función visible ni se cierra L-09A.

**L-09B, archivos:** drop_target/dnd, hooks winit, file-panel/tree/panes y workspace ops.
Separar drag interno entre paneles de DnD externo. Capturar panel destino, fuentes/modificadores
y staging; mantener confirmación/conflicto W-04. Verificar API de winit 0.30.13 antes de afirmar
posición disponible o soporte Wayland. Si un evento externo no identifica el panel bajo cursor,
no operar silenciosamente en otro panel: destino explícito/revisado o capacidad deshabilitada.
Drag saliente requiere soporte real, no sólo copiar clipboard. No actualizar Slint/winit dentro
del mismo bloque: evaluar upgrade separado con regresión Windows si fuera imprescindible.
**Pruebas:** drop sin foco, dos paneles, panel cerrado/movido, Ctrl/Shift, dentro de misma carpeta,
tray, virtual/URI remoto rechazado y sesión sin capacidad. Validar X11/Wayland por separado.

## L-10A — contención del preview

**Archivos:** `platform/preview_process.rs`, nuevo supervisor Linux, dispatcher aislado,
`preview_workers.rs`, protocolos y tests de proceso.

Diseñar y validar un backend Linux antes de leer el primer archivo. Preferencia: launcher
mínimo con límites de recursos, grupo de procesos y handshake de contención verificable.
No considerar `RLIMIT_AS` igual a RSS ni a `ProcessMemoryLimit`; medir margen para mappings,
stacks y allocator. Para CPU/descendientes investigar cgroup v2 delegado; no exigir root ni
modificar la sesión/cgroups del usuario para que el preview funcione.

La decisión de L-10A debe especificar: límites de memoria/CPU/tiempo/pipes/procesos, dónde se
aplican antes de input, cómo el hijo verifica, cleanup/reap, padre muerto, prioridad y errores.
`pre_exec` inseguro en proceso multihilo no se usa con asignaciones/locks; contrastar alternativas
del runtime o launcher antes de elegir. Grupo + kill no equivale por sí solo a contención completa
de descendientes; no declararlo sandbox ni protección total.

Cancelación independiente de IPC lleno, `kill`/reap fuera de UI y plazas RAII retenidas hasta
salida real. Job equivalente no disponible → preview deshabilitado/error terminal, nunca fallback
de lectura local. Preservar presupuestos de retención/scratch existentes en core.

**Pruebas:** hijo que no consulta token, pipe lleno, memoria/CPU excedida, respuesta hostil,
padre muerto, lector manual sin contención, timeout, reap repetido y ausencia de crecimiento
de procesos/FD. Ejecutar texto/imagen/PDF/ZIP/STL/3MF release con corpus de prueba.

## L-10B — animación

**Archivos:** `preview_animation_process.rs`, reader/wire/tick/Playback.
Reutilizar supervisor con una política explícita para CPU sostenida: límite acumulado de CPU
no reemplaza el hard-cap Windows para un proceso persistente. Si cgroup/rate cap no está
disponible, evaluar solución medida y aprobar degradación; no afirmar equivalencia de límites.
Conservar crédito único, pausa sin pedidos, timeout sólo con pedido pendiente, cancelación
fuera de cola, reloj monotónico y destrucción al ocultar/minimizar.

**Pruebas:** pausa con respuesta en vuelo, reanudar, repetición, loop finito, A → B → A,
frame corrupto/lento, hide/show, parent muerto y CPU/RSS principal + hijo en modo económico.
Sin audio/vídeo, colección de frames ni caché persistente.

## L-11 — sesión y expulsión

**Archivos:** autostart, global_hotkey, window/window_geometry, eject, tray y configuración.
Subdividir en bandeja/cierre, geometría/hotkey y eject.

- Bandeja sólo cuando el backend/event loop funciona; `close_to_tray` no oculta la única ventana
  si no hay vía de retorno. No exigir instalar extensiones GNOME para navegar.
- **Corregido localmente el 2026-10-17:** cerrar con `close_to_tray` solo conserva el proceso cuando
  la bandeja está visible o hay un hotkey global realmente registrado. Linux aún no tiene hotkey;
  si AppIndicator no está disponible, cerrar persiste y sale en vez de minimizar una ventana sin
  forma de recuperarla. En Windows se mantiene el tray o el hotkey como vías alternativas. La
  prueba unitaria, gate completo y distribución Windows pasan; esa build no se instaló. La prueba
  visual de visibilidad/retorno del tray requiere escritorio Linux nativo.
- Autostart mediante entrada de escritorio del usuario, activación/desactivación explícita.
  `--tray` sin tray sigue accesible. Respetar consentimiento para hotkey/portal.
- Geometría: restaurar tamaño/maximización; posición sólo con capacidad real. No prometer
  paridad Wayland a partir de APIs X11. Detectar soporte portal, no versión de GNOME asumida.
  [GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html),
  [API winit fijada](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html).
- Expulsión por servicio de dispositivos/Polkit, sin sudo ni shell strings. Distinguir desmontar
  filesystem de apagar dispositivo, particiones múltiples y «ocupado». Cancelación/timeout no
  anuncian éxito; releer resultado en worker y actualizar paneles coherentemente.
- Menú Shell Windows/Zone.Identifier quedan identificados como específicos de Windows;
  las acciones comunes de Naygo siguen disponibles. No inventar un «Shell menu Linux» universal.

**Pruebas:** portal/tray ausente, consentimiento rechazado, hotkey ocupado, cierre/retorno,
ventana fuera de pantalla, sesión sin posición, USB ocupado/multipartición y Polkit rechazado.

## L-12 — CI y empaquetado

**Archivos propuestos:** jobs Ubuntu en CI, `scripts/build-release-linux.sh`, `packaging/linux/`,
BUILD/DISTRIBUTION/THIRD-PARTY-NOTICES y tests de paquete.

**Preparación L-12, 2026-10-18:** `ci.yml` incorpora `linux-native`, ejecutable manualmente sobre
`ubuntu-26.04` x86_64. Instala dependencias de compilación, Fontconfig y X11/Wayland para Slint/Winit,
registra sus versiones, fija Rust 1.96.0 y corre formato/paridad i18n, tests nativos de core/platform,
Clippy y compilación/Clippy de la UI. Mantenerlo opt-in evita bloquear el gate Windows mientras Linux carece
de aceptación L-03/L-04. GitHub documenta `ubuntu-26.04` como runner hosted x64 y Slint lista las
dependencias X11 de Winit. `fontique-dlopen` evita exigir headers Fontconfig al compilar y deja que
el runner valide el backend runtime con las bibliotecas del sistema. El workflow pasó `actionlint` v1.7.12 tras declarar esa etiqueta oficial en
`.github/actionlint.yaml`, y `scripts/check_action_pins.py` confirmó sus SHAs. Es una preparación
estática: el job aún no se ejecutó, no compila artefacto instalable ni valida UI en escritorio.

- Desde L-03, check/tests core y platform nativos; desde L-04, UI/headless/X11 según entorno.
  Versiones de Ubuntu explícitas, `--locked`, toolchain validada, i18n, fmt/Clippy; verificar que
  los tests Windows no sean falsamente ejecutados contra stubs en Linux.
- Compilar paquetes en base mínima soportada, un job de build con supervisor de memoria
  adaptado y logs. Mantener Windows ThinLTO/jobs=1 con su supervisor; no ejecutar ambos builds
  pesados simultáneamente en el equipo de Nicolás.
- `.deb`: binario, desktop file, íconos, MIT completa, avisos, autoría de Nicolás/ISGroth y
  dependencias dinámicas reales obtenidas del artefacto. No instalar config en `/usr` ni ejecutar
  app como root en hooks. Actualización/desinstalación conservan datos personales.
- Verificar versión/build ID, hash e integridad, install/launch/remove en VM limpia y upgrade.
  Publicación/firma de repositorio queda fuera de ejecución automática del plan.
- L-14 AppImage es entrega adicional: verificar licencias de librerías incluidas, compatibilidad
  glibc y ejecución/extracción sin suponer FUSE disponible. No agregar Flatpak/Snap en v1.

## L-13 — matriz de aceptación

| Área | Casos mínimos | Evidencia requerida |
| --- | --- | --- |
| Escritorio | GNOME Wayland, X11 disponible y derivado elegido | Distro/DE/protocolo/versiones reales |
| Navegación | 10k/100k, filtros, teclado, history, docking, DPI | Primera fila, p95, CPU/RAM |
| FS | ext4, FS sin sensibilidad a caja si disponible, montajes USB/red | a/A, permisos, enlaces, EXDEV |
| Operaciones | copy/move/rename/batch/trash/restore/undo | Origen/destino íntegros tras fallos/cancelación |
| Integración | clipboard con otros exploradores, DnD, terminal, IPC | Capacidades disponibles/degradadas explícitas |
| Preview | corpus W-11, animación, ocultar, límites y lector hostil | Principal + auxiliares, FD/hilos/procesos |
| Datos | config XDG, migración/import, logging, update/uninstall | Preferencias conservadas, EXE sin escritura junto a él |
| Distribución | `.deb` limpio, checksums, autoría/MIT | Instalación/arranque/desinstalación y rollback |
| Windows | gates comunes + smoke de contratos tocados | Sin regresión en paquete Windows vigente |

Montajes de red caídos/permisos y cancelación se prueban tanto con fixtures como con entorno
real autorizado. No contar un test headless como aceptación del escritorio. Derivados no
probados se documentan como compatibles por comprobar, no certificados.

## Gate y entrega para Luna

Cada bloque comparte el gate de mejoras para código común. En Linux añadir tests nativos
`cargo test --workspace --no-fail-fast --locked --jobs 1`, Clippy/fmt/i18n y scripts pertinentes
al ID; tests de integración requieren capacidades/entorno reales o quedan explícitamente pendientes.
Si cambia producto Windows compartido, regenerar `dist/` Windows según AGENTS.md. Un cambio
sólo Linux sin impacto Windows requiere demostrarlo; no usarlo como excusa para omitir gate
cuando se tocan core/UI/manifiestos comunes. Desde empaquetado disponible, dejar también
paquete Linux local actualizado. Los paquetes de alfa indican claramente sus límites.

```text
Ejecuta únicamente <ID> de docs/plans/2026-10-13-linux-ubuntu-luna.md con GPT-6 Luna.
Lee AGENTS.md, BACKLOG, ESTADO-ACTUAL y ambos planes de octubre. Verifica dependencias,
entorno y decisiones del ID. Usa las APIs fijadas en Cargo.lock y preserva Windows.
No habilites stubs como funciones reales ni previews sin supervisor validado.
Prueba fallos/cancelación/identidad en corpus aislado, documenta evidencia y genera los
paquetes afectados con sus supervisores. No publiques ni modifiques mi sesión/config.
Termina ese ID, informa limitaciones y espera mi visto bueno antes del siguiente.
```

Todos los hitos permanecen abiertos. L-01A tiene implementados sus subpasos de nombres POSIX, ZIP,
plantillas y argumentos CLI, con checks cruzados; falta ejecutar sus pruebas Unix en Linux nativo y
completar los casos de identidad/bytes que detalla el backlog. L-00 tiene mínimo, ruta de bandeja,
licencia condicionada y paquete runtime inventariados, pero no está cerrado: faltan ambiente nativo,
binding GLib probado, degradación y notices del paquete final. Los demás hitos no se han iniciado.
No existe estimación calendarizada fiable hasta L-00/L-03; la estimación de julio no se hereda porque
el alcance del preview y la base cambiaron.
