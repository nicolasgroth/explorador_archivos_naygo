# Auditoría de portabilidad a Linux (Ubuntu y derivados)

> Fecha: 2026-07-01. Auditoría multi-agente (4 auditores en paralelo + verificación
> empírica de compilación cruzada). Target: Ubuntu LTS y derivados, X11 y Wayland.
> Objetivo: hoja de ruta accionable ANTES de escribir código. Licencias de todas las
> crates recomendadas verificadas contra crates.io (solo permisivas, regla del proyecto).

## Veredicto ejecutivo

**El port es factible y la arquitectura de 3 capas rinde exactamente como se diseñó.**
`core` y `platform` ya compilan para Linux (verificado empíricamente, no por lectura).
Los íconos —un riesgo asumido grande— resultaron ser **cero trabajo** (100% propios y
embebidos, nada del Shell de Windows). El grueso del esfuerzo está en 4 frentes:
implementar la fachada Linux de `platform` (~17 módulos, la mayoría S/M), corregir en
`core` un puñado de supuestos case-insensitive **que en Linux son riesgo de pérdida de
datos**, resolver el trío interactivo duro (clipboard de archivos / drops / drag interno
entre paneles), y aceptar 3 degradaciones honestas en Wayland (posición de ventana,
hotkey global en GNOME<48, drag saliente).

Estimación global: **proyecto de 5-6 lotes (~6-9 semanas de trabajo enfocado)**, con la
particularidad de que el **Lote 0 se puede hacer YA desde Windows** y mejora la
corrección del producto actual.

---

## 1. Evidencia empírica (compilación cruzada, ejecutada hoy)

| Crate | `cargo check --target x86_64-unknown-linux-gnu` | Nota |
|---|---|---|
| `naygo-core` | ✅ **Compila limpio** | 0 errores, 0 warnings |
| `naygo-platform` | ✅ **Compila** | 3 warnings `dead_code` en stubs (trivial) |
| `naygo-ui-slint` | ❌ Falla en `yeslogic-fontconfig-sys` | **Esperado**: lib C de sistema (fontconfig); no es error del código. Requiere entorno Linux real (CI/WSL/VM) con `libfontconfig1-dev` |

Conclusión: dos de tres capas **probadas** portables. La UI necesita un entorno Linux
para validarse (no se puede cross-compilar una GUI con deps C de sistema desde Windows).

---

## 2. Mapa por capa

### 2.1 `naygo-core` — compila, pero "compila ≠ correcto"

Hallazgos que requieren rama por plataforma (la mayoría corregibles YA, testeables):

| Sitio | Problema en Linux | Severidad |
|---|---|---|
| `ops/plan.rs:123` (clave batch-rename `to_lowercase`) | `a.txt` y `A.TXT` son archivos DISTINTOS; el rename puede **pisar el otro archivo** | **CRÍTICO (pérdida de datos)** |
| `ops/undo.rs:134-147` (validate case-insensitive) | Un deshacer puede sobrescribir un archivo distinto que solo difiere en caja | **CRÍTICO (pérdida de datos)** |
| `ops/plan.rs:311-340` (`paths_eq_ci`/`is_inside`) | Falsos positivos "misma ruta/está dentro" → desalinea la guarda anti-borrado de "reemplazar sobre sí mismo" | **ALTO** |
| `listing.rs:119-140` (`attrs_of`) | `hidden` siempre `false` → los toggles del menú "ojo" quedan muertos. Fix: `hidden = nombre.starts_with('.')` en no-Windows | ALTO (UX rota) |
| `dnd.rs:47-109` (`same_drive` por letra) | Siempre `false` → drag sin modificador **copia** en vez de mover en el mismo FS. Fix: `st_dev` (std puro) | MEDIO |
| `config/mod.rs:333` (`resolve_home_dir`) | `USERPROFILE`→`C:\`; falta rama `HOME`→`/` | MEDIO |
| `ops/names.rs:11-32` (`is_valid_name`) | Reglas Windows (prohíbe `:*?"<>|` etc.) impedirían renombrar nombres legales en Linux | DECISIÓN (sugerencia: validación por plataforma, reglas Windows como opción "compatibilidad") |
| `sizing.rs`/`search.rs` (`is_reparse_point`) | Fallback correcto (symlinks ya cubiertos) | ✅ OK |

**Concepto transversal "raíz = letra de unidad"** horneado también en ui-slint:
`path_is_on_drive` (`workspace_ctrl/mod.rs:631`), clave de caché del footer
(`mod.rs:604` — `ancestors().last()` = siempre `/` en Linux → mostraría el disco
equivocado), `drive_letter` pública de eject. Debe generalizarse a **mount point**
(prefijo más largo contra mountinfo) antes de que discos/eject/footer funcionen.

### 2.2 `naygo-platform` — la fachada a implementar (17 módulos)

| Módulo | Equivalente Linux | Crate (licencia verificada) | Esfuerzo |
|---|---|---|---|
| `dir_watch` | inotify vía `notify` — **ya multiplataforma, 0 cambios** | ya en el árbol (CC0/MIT/Apache; inotify ISC) | **0** (solo validar) |
| `drive_space` | `statvfs` (usar `f_bavail`) | `rustix` o `nix` (MIT/Apache) | S |
| `trash` | Spec FreeDesktop Trash v1.0 | `trash` 5.2 (MIT) | S |
| `autostart` | `~/.config/autostart/naygo.desktop` (XDG), conserva `--tray` y semántica "el SO es fuente de verdad" | `auto-launch` 0.6 (MIT) o a mano | S |
| `locale` | `LC_ALL`>`LC_MESSAGES`>`LANG` (fallback parcial ya existe) | `sys-locale` (MIT/Apache) | S |
| `time` | `/etc/localtime` vía **chrono** (NO crate `time`: su offset local falla en multihilo Unix) | `chrono` (MIT/Apache) | S |
| `window` | `has_focus()`/`focus_window()` de winit; `screen_to_client` se elimina (drops Linux ya vienen en coords de ventana) | winit vía `i-slint-backend-winit` | S |
| `context_menu` | **Sin equivalente universal** → omitir; ya degrada solo (gate `naygo_hwnd` devuelve None en Linux) | — | S (0 código) |
| `window_geometry` | X11 completo; **Wayland: posición imposible** (confirmado docs winit) → tamaño+maximizado | winit | S/M |
| `open` | `xdg-open`/`gio open`; "Abrir con…" vía portal `OpenURI ask=true`; **rediseñar enum `Terminal`** (hoy PowerShell/Cmd/WT/Wsl) → cadena `x-terminal-emulator`→`$TERMINAL`→gnome-terminal/konsole | `open` 5.3 (MIT); opc. `ashpd` (MIT) | M |
| `drives` | `/proc/self/mountinfo` + `/sys/block` devpath para bus USB (⚠️ `removable` de sysfs miente para USB HDD — mismo gotcha que DRIVE_FIXED) + filtrar 30+ pseudo-mounts | sin crate (std) | M |
| `device_watch` | Señales D-Bus de udisks2 (ObjectManager + MountPoints; "enchufar ≠ montar"); fallback `POLLPRI` sobre mountinfo | `zbus` 5 (MIT) | M |
| `global_hotkey` | **X11: la crate actual ya lo soporta** (casi gratis). Wayland: portal GlobalShortcuts (KDE sí; GNOME ≥48; **LTS actuales NO**) con modelo de consentimiento (cambia UX de Config) | `global-hotkey` (ya) + `ashpd`/`zbus` (MIT) | M |
| `drop_target` (drops entrantes) | **Invertir estrategia**: reusar los eventos XDND de winit (`HoveredFile`/`DroppedFile` vía `on_winit_window_event`) en vez de pisar su target. winit 0.30: sin posición del cursor → drop al panel ACTIVO, sin Shift=mover. Se arregla cuando Slint suba a winit 0.31 (`DragEntered/Moved/Dropped` con posición). **Wayland: sin drops** (winit#1881 abierto) | `i-slint-backend-winit` (ya) | M |
| `eject` | udisks2 vía D-Bus (`Filesystem.Unmount`+`Drive.PowerOff`, polkit sin root). ⚠️ **crate `udisks2` = LGPL → DESCARTADA**; proxies a mano sobre zbus. Multi-partición; mapear "busy"→`InUse` fiel | `zbus` 5 (MIT); fallback `udisksctl` | L (M con fallback) |
| `clipboard` | Texto/imagen: `arboard` (y borra el parser DIB de 140 líneas). **Archivos: a mano** — `text/uri-list` + `x-special/gnome-copied-files` (cut/copy GNOME) + `application/x-kde-cutselection`; en X11 el dueño de la selección debe seguir vivo (hilo servidor) | `arboard`, `x11rb`, `wl-clipboard-rs` (todas MIT/Apache) | **L** |
| `dnd` (drag saliente) | **Sin camino maduro hoy**: la única crate (drag-rs) es GTK-only; XDND a mano = semanas; **Wayland bloqueado por diseño** (winit no expone serials) hasta que Slint/winit lo traigan upstream (está en su roadmap declarado). **Recomendación: degradar en v1** — copiar/pegar de archivos cubre el flujo | ninguna hoy | **L / bloqueado** |

### 2.3 `naygo-ui-slint`

- **~70 llamadas sin gate a `naygo_platform::*`** → estrategia correcta: implementar la
  fachada Linux EN platform (misma API), no sembrar `cfg` por la UI.
- **Tray** (`tray-icon`): soportado en Linux pero exige **un event loop GTK en un hilo
  dedicado** (Slint/winit no lo da) + `libayatana-appindicator`. Ubuntu trae la extensión
  de indicadores de fábrica; VERIFICAR Kubuntu/Xubuntu (StatusNotifier nativo, debería
  andar).
- **`rfd`**: ya usa portales XDG por defecto (no GTK) → cero trabajo.
- **Slint** (backend-winit + renderer-software): soporta X11 y Wayland; el forzado por
  código de "software" funciona igual.
- Sitios `cfg(windows)` de main.rs: la mayoría ya degradan con fallback; faltan ramas
  Linux para geometría, autostart-sync y terminal. Fallbacks `USERPROFILE`→`C:/` sin gate
  en 4 sitios (main.rs:229, listing.rs:129/157, templates.rs:16) → centralizar en
  `resolve_home_dir`.
- **build.rs/winresource**: ya gated, no rompe el build Linux.

### 2.4 Íconos — CERO riesgo (verificado)

Grep de `SHGetFileInfo|ExtractIcon|SHGetImageList|IShellItemImageFactory` en todo
`crates/`: **0 coincidencias**. Todos los íconos (archivos, acciones, tray) son propios y
embebidos (`include_bytes!` + tabla de extensiones propia). Lo único "Windows" es el
`.ico` del exe (build.rs, gated). En Linux: ícono hicolor del `.desktop` (empaquetado).

### 2.5 Drag ENTRE PANELES — deuda heredada a presupuestar aparte

**Corrección a la evaluación previa del proyecto**: el drag interno entre paneles viaja
hoy por el canal OLE (dnd.rs saliente + drop_target.rs entrante juntos, main.rs:1280).
En Linux, sin OLE, el drag entre paneles necesita reimplementarse con el **DnD interno de
Slint** (`DragArea`/`DropArea`, experimental; slint#11400 lo diseña "extensible a
cross-app"). Ítem propio, estimado M-L, imprescindible para paridad (el drag entre
paneles es feature central de un commander).

---

## 3. Config, logs y journal — showstopper de arreglo barato

TODO el estado (settings/workspace/keybindings/themes/lang/icons/journal de ops/log
diario) se escribe **junto al exe** (`portable_dir()`, config/mod.rs:752). En AppImage
(squashfs solo-lectura) o `.deb` (/usr/bin) eso **falla siempre y en silencio**.

**Propuesta (portable-first con fallback XDG)** — cambio quirúrgico en UNA función:
1. Si junto al exe hay `settings.json` o un marker `portable.txt` Y la carpeta es
   escribible → modo portable (Windows idéntico; pendrive Linux conservado).
2. Si no → `$XDG_CONFIG_HOME/naygo` (config) + `$XDG_STATE_HOME/naygo` (logs/journal).
3. Solo se toca `core::config::portable_dir` — logging y WorkspaceCtrl ya delegan ahí.

---

## 4. Degradaciones honestas en Wayland (documentar, no pelear)

| Feature | X11 | Wayland |
|---|---|---|
| Recordar posición de ventana | ✅ completa | ❌ imposible por diseño → tamaño+maximizado |
| Hotkey global | ✅ (crate actual) | Portal GlobalShortcuts: KDE ✅, GNOME ≥48 (LTS actuales ❌), con diálogo de consentimiento |
| Traer al frente | ✅ (EWMH) | Advisory (xdg_activation): KDE activa, GNOME puede solo "pedir atención" |
| Drops entrantes de otras apps | ✅ (sin posición hasta winit 0.31) | ❌ hasta upstream (winit#1881) |
| Drag saliente hacia otras apps | L (a mano) | ❌ bloqueado hasta Slint/winit upstream |

Ubuntu ≥22.04 usa Wayland por defecto → estas degradaciones afectan al usuario típico.
Ninguna es bloqueante para un explorador usable; todas tienen mitigación (clipboard de
archivos cubre el intercambio con otras apps).

## 5. Hallazgos de licencia

- ⚠️ **`udisks2` (bindings oficiales) = LGPL-2.1 → DESCARTADA** (regla del proyecto).
  Vía: proxies a mano sobre `zbus` (MIT).
- ⚠️ crate `udev` linkea libudev (LGPL como lib de sistema); evitable vía zbus/mountinfo.
- ✅ Todo lo demás recomendado es MIT / Apache-2.0 / ISC / CC0 (verificado una a una):
  open, trash, sys-locale, chrono, rustix/nix, zbus, ashpd, arboard, x11rb,
  wl-clipboard-rs, auto-launch, cargo-deb, sysinfo.

## 6. Empaquetado y CI

- **`.deb` principal** (cargo-deb en CI, `ubuntu-22.04` para compatibilidad glibc;
  `Depends: libgtk-3-0, libayatana-appindicator3-1, libxdo3`; instala .desktop + ícono).
- **AppImage secundaria** — el gemelo del ZIP portable (requiere el fix §3).
- **Flatpak descartado por ahora**: el sandbox pelea contra la esencia de un explorador
  (`--filesystem=host` mal visto en Flathub).
- CI: job Linux paralelo en `release.yml` + matrix os en `ci.yml` (cuando compile);
  script hermano `scripts/build-release-linux.sh`.
- Libs de build en runner: `libgtk-3-dev libxdo-dev libayatana-appindicator3-dev
  libfontconfig1-dev` (esta última la exige el renderer de Slint — confirmado por el
  check cruzado).

## 7. Hoja de ruta propuesta (lotes)

| Lote | Contenido | Esfuerzo | Nota |
|---|---|---|---|
| **0 — Correcciones pre-port (desde Windows, YA)** | Case-folding `#[cfg(windows)]` en ops/plan+undo (⚠️ pérdida de datos), `hidden=dotfile`, `same_drive` por `st_dev`, `resolve_home_dir`+centralizar fallbacks HOME, generalizar "letra"→mount point (path_is_on_drive, caché footer, drive_letter), `config_root` portable-first+XDG, dejar de ignorar el Result de trash (ops_ctrl:291) | ~1 semana | **Mejora la corrección en Windows también**; 100% testeable hoy |
| **1 — Platform fácil** | trash, autostart, locale, time, drive_space, open (con rediseño Terminal), validar dir_watch | ~1 semana | Crates verificadas, contratos 1:1 |
| **2 — Discos y dispositivos** | drives (mountinfo+sysfs), device_watch (zbus/udisks2), eject (zbus, o fallback udisksctl primero) | 1-2 semanas | El L del grupo es eject |
| **3 — Arrancar la app en Ubuntu** | window, window_geometry (X11+degradación Wayland), tray con hilo GTK, primer arranque, entorno de build/CI Linux, humo en VM Ubuntu | 1-2 semanas | Primer hito visible: **Naygo corre en Ubuntu** |
| **4 — Interacción dura** | clipboard de archivos (uri-list+convenciones GNOME/KDE), drops entrantes vía winit, **drag interno entre paneles con DnD de Slint** | 2-3 semanas | El lote más caro; drag saliente NO va (degradado) |
| **5 — Hotkey + pulido + release** | hotkey X11 (casi gratis) + portal Wayland (KDE/GNOME 48), i18n de avisos de degradación, .deb + AppImage + release.yml | 1-2 semanas | |

**Total estimado: 6-9 semanas** de trabajo enfocado. Los lotes 0-2 son de bajo riesgo;
el riesgo se concentra en 3 (tray GTK-thread) y 4 (clipboard/DnD).

## 8. VERIFICAR pendientes (al momento de implementar)

- Soporte `ext-data-control` en Mutter/GNOME (para clipboard sin foco en Wayland;
  mitigación: fallback XWayland funciona hoy).
- Estado del DnD entrante Wayland en winit 0.31 (winit#1881).
- Versión mínima de GNOME con portal GlobalShortcuts estable (48, con desviaciones).
- Tray en Kubuntu/Xubuntu (StatusNotifier nativo).
- Licencia de `inotify` (ISC) al regenerar THIRD-PARTY-NOTICES desde Linux.

## 9. Decisión de producto pendiente (para Nicolás)

CLAUDE.md declara "explorador para Windows 10/11" y el diferencial es velocidad. Ampliar
el alcance a Linux es una decisión de producto además de técnica: implica CI doble,
pruebas en 2+ entornos (X11/Wayland), y documentar degradaciones. La arquitectura lo
permite con esfuerzo razonable; el costo recurrente de mantener dos plataformas es la
variable que no mide esta auditoría.
