# Mejoras: divisores de paneles, geometría de ventana, bandeja/autostart e instalador

> Spec de diseño. Fecha: 2026-07-01. Autor: Nicolás Groth / ISGroth.
> Estado: aprobado, pendiente de plan de implementación.

## Contexto

Lote de 7 mejoras pedidas por Nicolás tras probar el portable/instalador. El
grueso del trabajo es **una sola cosa** (el modelo de divisores de paneles); el
resto es conectar, exponer o pulir subsistemas que **ya existen** en el código
(`autostart.rs`, `tray.rs`, `close_to_tray`, persistencia de sesión de layout).

Mapa del código actual verificado antes de este diseño:

- **Config**: `crates/core/src/config/mod.rs` — struct `Settings` (serde_json,
  portable junto al `.exe`, tolerante a campos ausentes vía `#[serde(default)]`).
  Ya existen `tray_enabled` (default `true`), `close_to_tray` (default `false`),
  `autostart` (default `false`).
- **Geometría de ventana**: NO se persiste nada hoy (ni tamaño, ni posición, ni
  maximizado). Solo se lee `ui.window().size()` en runtime para logs.
- **Divisores**: `crates/core/src/workspace/layout.rs` — árbol binario
  `DockNode::Split { dir, fraction, first, second }`. `fraction` es la proporción
  del primer hijo. Cubierto por ~30 tests.
- **Sesión de layout**: `crates/ui-slint/src/workspace_ctrl/session.rs` — el árbol
  de layout COMPLETO ya se persiste (`session_persist` → `save_workspace`) y se
  restaura (`load_session`). `maybe_persist_session` lo guarda en cada tick si
  cambió.
- **Autostart**: `crates/platform/src/autostart.rs` — `HKCU\...\Run`,
  `is_enabled()` / `set_enabled()`.
- **Bandeja**: `crates/ui-slint/src/tray.rs` — ícono + menú (Abrir / Nuevo panel /
  Config / Centrar / Salir), crate `tray-icon`. `should_quit_on_close(close_to_tray,
  tray_active)` decide salir vs esconder.
- **Instalador**: `installer/naygo.iss` — Inno Setup, hoy `[Languages]` = ES + EN;
  `[Tasks]` = desktopicon, openwith, ctxmenu. Sin tarea de autostart.
- **Idiomas de la app**: 10 (`de, en, es, fr, hi, it, ja, ko, pt, zh`) en
  `crates/core/src/i18n/*.json`.

## Alcance

| # | Mejora | Estado hoy | Trabajo |
|---|--------|-----------|---------|
| A | Instalador: traducir wizard + elegir idioma de la app | Solo ES/EN | Medio |
| B1 | Iniciar con Windows desde el instalador | Autostart en app, no en instalador | Bajo |
| B2 | Autostart arranca minimizado en bandeja (opcional) | Tray + autostart no coordinados | Bajo-medio |
| B3 | La X esconde a bandeja (default) | `close_to_tray` existe, apagado | Bajo |
| C | Recordar tamaño/posición/maximizado de ventana | No existe | Medio |
| D | Divisores: solo vecinos, resto fijo + responsive | Árbol binario de fracciones | **Alto** |
| E | Extras: doble-clic 50/50, feedback visual, restaurar layout | Layout ya persiste | Bajo (sobre D) |

Fuera de alcance: reproducción de media, edición de archivos, refactors no
relacionados.

---

## Sección 1 — Divisores "solo vecinos, resto fijo" (D)

### Problema

Hoy 3 paneles en fila son el árbol `A | (B | C)`. Mover el divisor izquierdo
cambia la `fraction` del split raíz → el bloque `(B|C)` entero se re-escala y B y C
se mueven juntos ("efecto elástico proporcional"). El usuario quiere que mover el
divisor A|B toque **solo** A y B, dejando C fijo, **manteniendo** el responsive al
redimensionar la ventana completa.

### Modelo propuesto: split de N hijos con pesos

Un split deja de ser binario (`fraction` + 2 hijos) y pasa a ser una **fila/columna
de N hijos, cada uno con un peso (weight) relativo**. Los divisores viven entre
hijos consecutivos.

```rust
DockNode::Split {
    dir: SplitDir,           // Horizontal (fila) | Vertical (columna)
    children: Vec<DockNode>, // N ≥ 2 hijos en orden
    weights: Vec<f32>,       // len == children.len(); relativos, se normalizan al pintar
}
```

Invariantes: `children.len() == weights.len() >= 2`; todos los pesos `> 0`; un
split que quedaría con un solo hijo se colapsa a ese hijo (igual que hoy). Los
`Leaf` y `Tabs` no cambian.

### Comportamiento

- **Mover el divisor entre el hijo `i` e `i+1`**: solo se transfiere peso entre `i`
  e `i+1`; `weights[i] + weights[i+1]` se mantiene constante. El resto de hijos NO
  se mueve → "resto fijo". Se aplica un mínimo por hijo (equivalente al actual clamp
  0.05) para que ningún panel colapse a 0.
- **Redimensionar la ventana**: `ancho_i = weights[i] / Σweights × ancho_disponible`
  (descontando las barras). Como los pesos son relativos, el reparto es proporcional
  → responsive se mantiene.
- **Splits anidados**: un hijo puede ser a su vez un `Split` en el otro eje;
  recursivo, igual que hoy.

### Por qué pesos y no anchos absolutos en px

Anchos fijos en píxeles matan el responsive (al agrandar la ventana queda espacio
muerto o hay que elegir quién crece). Los pesos con reparto local entre vecinos dan
las dos propiedades pedidas a la vez (resto fijo + responsive). Es el modelo de VS
Code y de la mayoría de exploradores serios.

### Impacto técnico

- Cambio central en `crates/core/src/workspace/layout.rs`:
  - `DockNode::Split` (binario → N-ario con pesos).
  - `place` / `pane_rects`: reparte por pesos.
  - `split_handles`: emite un handle por cada divisor interno (N-1 por split).
  - `fraction_at` → equivalente que calcula, para el divisor `i`, la transferencia
    de peso local (mantiene la firma de "posición del puntero → geometría de la
    barra-fantasma"; el nombre puede cambiar a `divider_at`).
  - `set_fraction` → `set_divider(path, divider_index, ...)`.
  - `split_leaf`, `remove_leaf`, `stack_onto`, `swap_split_children`: adaptar a
    N-ario. Dividir una hoja crea un split de 2; quitar un hijo reduce el `Vec`
    (colapsa a hoja cuando queda 1). Insertar por drop lateral agrega un hijo al
    `Vec` en la posición correspondiente con un peso inicial = promedio.
- La ruta a un split (`Vec<SplitStep>`) deja de ser First/Second y pasa a
  `SplitStep(usize)` (índice del hijo). Un divisor se identifica por
  `(path_al_split, divider_index)`.
- UI (`app-window.slint`, handlers en `main.rs`): consume las mismas operaciones
  conceptuales (rects de paneles, handles de divisores, preview al arrastrar,
  commit al soltar). Se mantiene la forma de la interfaz para acotar el blast
  radius; cambia la identificación del divisor (índice en vez de First/Second).
- Tests de `layout.rs`: adaptar los ~30 existentes al modelo N-ario y añadir casos
  nuevos: mover un divisor central no altera hijos no adyacentes; suma de pesos de
  los dos vecinos constante; responsive proporcional; migración desde `fraction`.

### Migración de formato

`workspace.json` viejo usa `DockNode::Split { fraction, first, second }`. Al
deserializar, se convierte a `{ children: [first, second], weights: [fraction,
1-fraction] }`. Se hace con un deserializador tolerante (serde `untagged` o un
`From` explícito sobre una forma intermedia) para que un layout guardado previo se
lea sin romper y sin que el usuario pierda su disposición.

---

## Sección 2 — Recordar geometría de la ventana (C)

### Qué guardar

Nuevo campo `window` en `Settings` (`settings.json`):

```rust
pub struct WindowGeometry {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}
// Settings.window: Option<WindowGeometry>  (None = primera vez, usa default)
```

Cuando la ventana está maximizada, se guarda `maximized: true` **y** el
tamaño/posición "restaurado" (el que tendría des-maximizada), para que al
des-maximizar vuelva a un tamaño sensato en vez de a un default.

### Cuándo se guarda

Al cerrar la ventana y periódicamente en el tick (junto con
`maybe_persist_session`), leyendo tamaño/posición/estado actuales. Se persiste solo
si cambió (misma estrategia de huella que la sesión de paneles).

### Cómo se restaura

1. Leer `window`.
2. **Validación de visibilidad multi-monitor**: comprobar que el rectángulo de la
   ventana caiga (al menos parcialmente, con un margen mínimo visible) dentro de
   algún monitor conectado AHORA. Si cerró en un monitor que ya no está o fuera de
   pantalla → recolocar centrada en el monitor principal con el tamaño guardado.
   Evita el "abrió fuera de pantalla y no la encuentro".
3. Si `maximized` → aplicar tamaño/posición restaurado y luego maximizar.

### Decisión técnica y degradación

Slint no expone posición de ventana ni enumeración de monitores de forma uniforme
en todas las versiones. Tamaño y maximizado son directos. Para posición (x,y) y
monitores: verificar en implementación qué da la versión de Slint del proyecto; si
no es fiable, leer/fijar vía crate `windows` (ya es dependencia) usando el HWND
(`GetWindowPlacement` / `SetWindowPlacement`, `MonitorFromRect`).

**Degradación acordada**: si la posición resulta frágil en esta versión de Slint,
se degrada a "solo tamaño + maximizado, centrado en pantalla principal" y se avisa
a Nicolás antes de decidirlo. Objetivo primario: tamaño + posición + maximizado.

### Dónde vive

Dentro de `settings.json` (campo `window`), no un archivo aparte: es un dato de
configuración más, un solo lugar.

---

## Sección 3 — Bandeja y autostart (B1, B2, B3)

Todo montado sobre lo existente (`tray.rs`, `autostart.rs`, `close_to_tray`,
`should_quit_on_close`). Sin subsistemas nuevos.

### B3 — La X esconde a bandeja (comportamiento cotidiano)

- Default de `close_to_tray`: `false` → **`true`**.
- **Forzar en settings existentes vía migración**: subir `CONFIG_VERSION` y, al
  migrar un settings previo, fijar `close_to_tray = true` una vez. Así queda activo
  en instalaciones existentes y en la VM sin tocar nada a mano. (Decisión explícita
  de Nicolás; se acepta que a quien lo hubiera apagado a propósito se le reactiva,
  porque es el nuevo comportamiento por defecto.)
- Exponer visible en Config (Integración/General) con tooltip: *"Al cerrar la
  ventana, mantener Naygo en la bandeja del sistema en vez de salir."*
- Salir de verdad: menú de bandeja → Salir (ya existe).

### B2 — Autostart puede arrancar minimizado (opción del usuario)

- Campo nuevo `autostart_minimized: bool` (default **`true`**).
- Opción en Config, habilitada **solo** cuando `autostart` está activo: *"Al iniciar
  con Windows, arrancar minimizado en la bandeja."*
- Mecánica: al activar autostart, la app escribe en la entrada Run el comando
  `naygo.exe --tray` (arg nuevo, se parsea con el `core::cli` existente). Al
  arrancar, si el arg `--tray` está presente **y** `autostart_minimized` es true →
  la ventana no se muestra; solo el ícono de bandeja. La ventana aparece al hacer
  clic en el ícono / menú (ya existe).

### B1 — Iniciar con Windows desde el instalador

- Nueva tarea en `naygo.iss`:
  `[Tasks] Name: "startupwin"; Description: "{cm:StartupWin}"; Flags: unchecked`
  → "Iniciar Naygo cuando arranque Windows".
- Si se marca, el instalador crea la entrada `HKCU\...\Run` (mismo valor que usa la
  app, incluyendo `--tray` cuando corresponda).
- **Fuente de verdad = el registro**. Al arrancar, la app sincroniza
  `Settings.autostart` con `autostart::is_enabled()`, de modo que instalador y
  Config nunca se contradigan.

### Coherencia

Un solo lugar decide salir vs esconder: `should_quit_on_close(close_to_tray,
tray_active)` (ya existe). No se duplica la lógica.

---

## Sección 4 — Instalador: idiomas del wizard + idioma de la app (A)

### Idiomas del wizard

Inno Setup trae `.isl` oficiales para 7 de los 10 idiomas de Naygo:
`en, es, de, fr, it, pt, ja`. NO oficiales para `hi, ko, zh`.

`[Languages]` pasa de 2 → **7** (los oficiales). Para `hi/ko/zh` el wizard sale en
inglés (decisión de Nicolás: no incluir `.isl` de terceros, por licencia/calidad).
Esos 3 idiomas SÍ quedan seleccionables como idioma de la app.

### Elegir el idioma inicial de la app (los 10)

- Página/paso del instalador donde el usuario elige el idioma de arranque de Naygo
  entre los 10 completos.
- **Preselección = idioma de Windows detectado** (no el del wizard). Inno expone el
  LangID del sistema (`GetUILanguage`/`GetSystemDefaultLCID`); se mapea a uno de los
  10 `LangId` de Naygo; si el SO está en un idioma no soportado, cae a inglés.
  Reutiliza el mismo criterio de auto-detect de la app.
- Transmisión a la app: el instalador **escribe un `settings.json` mínimo**
  `{ "language": "xx" }` **solo si NO existe** settings.json (instalación nueva). La
  app completa el resto de defaults al arrancar (loader tolerante). En reinstalación
  (settings ya presente) **no lo toca**, para no pisar la config del usuario.
  - Alternativa descartada: pasar `--lang xx` en el acceso directo (no persiste,
    solo aplica a ese acceso directo).

### Portable

Sin cambios: no tiene wizard; el idioma se resuelve como hoy (settings o auto-detect
del SO).

---

## Sección 5 — Extras UX (E)

Montadas sobre el modelo de divisores nuevo.

### E1 — Doble-clic en un divisor = 50/50 entre sus dos vecinos

Con pesos: doble-clic sobre la barra iguala `weights[i]` y `weights[i+1]` de los dos
hijos adyacentes (los demás intactos). Se cablea detectando doble-clic sobre el
`TouchArea` de la barra en `main.rs`/`.slint`.

### E2 — Feedback visual al hover/arrastrar un divisor

- Cursor de redimensionar: `col-resize` (↔) para divisores de fila,
  `row-resize` (↕) para divisores de columna, vía `mouse-cursor` en el `TouchArea`
  de la barra.
- Resaltado sutil (color de acento del tema) en hover y durante el arrastre,
  cambiando el color del `Rectangle` de la barra según un estado de hover.
- Puramente visual, sin tocar la lógica de layout.

### E3 — Restaurar sesión de layout

El layout ya se persiste y restaura (`session_persist` / `load_session` guardan el
árbol completo con sus tamaños). Con el modelo de pesos —que sigue dentro de
`DockNode`— la persistencia sale por el mismo camino. Único trabajo real: la
migración de `fraction` → `weights` al deserializar (ya contemplada en la Sección
1). E3 = verificar que sigue funcionando + migración; sin código nuevo de
persistencia.

---

## Tabla de decisiones

| Punto | Decisión |
|-------|----------|
| D Divisores | Split N-ario con pesos; reparto local entre vecinos (resto fijo); responsive por proporción |
| C Ventana | Guardar tamaño+posición+maximizado en settings.json; validar visibilidad multi-monitor; degradar a tamaño+maximizado si Slint no da posición fiable (avisando) |
| B3 X→bandeja | `close_to_tray` default true, **forzado vía migración** de CONFIG_VERSION; visible en Config |
| B2 Autostart min. | Campo `autostart_minimized` (default true); opción en Config solo si autostart activo; arg `--tray` en la entrada Run |
| B1 Instalador autostart | Tarea `startupwin` en el .iss; el registro es la fuente de verdad; la app sincroniza `autostart` al arrancar |
| A Instalador idiomas | Wizard en 7 idiomas oficiales de Inno; paso de idioma de app (los 10) preseleccionado por idioma de Windows; escribe `settings.json` mínimo solo si no existe |
| E1 Doble-clic 50/50 | Iguala pesos de los dos vecinos |
| E2 Feedback visual | Cursor ↔/↕ + hover con color de acento |
| E3 Restaurar layout | Ya persiste; solo migración de formato |

## Orden de construcción

Por dependencia y riesgo:

1. **D** — modelo de layout N-ario con pesos (core) + migración + tests.
2. **E1 + E2** — doble-clic 50/50 y feedback visual (encima de D).
3. **C** — geometría de ventana (settings + restaurar + validación multi-monitor).
4. **B3** — `close_to_tray` default true + migración de CONFIG_VERSION + exponer en
   Config.
5. **B2** — `autostart_minimized` + arg `--tray` + arranque minimizado.
6. **B1 + A** — instalador: tarea de autostart + idiomas del wizard + paso de idioma
   de la app. Al final porque depende de que `language` y `autostart` estén estables.

## Principios respetados

- Core puro y testeable: el modelo de divisores vive en `core`, sin UI ni Windows.
- El filesystem/entorno es hostil: validación de monitores, tolerancia a settings
  ausentes, la app no cae si falta geometría o el registro difiere.
- i18n desde el día uno: claves nuevas para las opciones de Config y del instalador,
  en los 10 idiomas.
- Sin telemetría. Persistencia local (settings.json / workspace.json).
- Regenerar `dist/` (portable + instalador) tras los cambios, como es costumbre.
