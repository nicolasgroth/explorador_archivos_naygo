# Lote 3 de ajustes de Nicolás — Diseño (2026-06-12)

> Ajustes pedidos por Nicolás tras probar el lote 2 en vivo. Cinco puntos: un fix
> de regresión (íconos cuadrados que introduje al regenerar assets), dos mejoras de
> la vista previa, un fix de layout de la path-bar, glifos confiables + íconos
> propios para los botones de panel, y un botón para abrir otra ventana de Naygo.

## Contexto

El lote 2 dejó un **bug de regresión**: correr `cargo run -p naygo-ui --bin gen_icons`
sobrescribió los 25 íconos REALES de cada set (Flat/Fluent/Mono) con placeholders
(cuadrados de color sólido, ~230 bytes). Por eso la toolbar en modo "Pack de íconos"
se ve toda cuadrada. Los íconos buenos están en git en el commit `30a34d7` (anterior
al lote 2).

---

## 1. Fix de íconos cuadrados (regresión del lote 2) — URGENTE

**Causa:** `gen_icons` es destructivo (regenera TODOS los PNG) y se corrió sobre
assets curados, pisándolos.

**Fix:**
- Restaurar los 25 PNG reales de cada set desde `30a34d7`
  (`git checkout 30a34d7 -- assets/icons/`), salvo los 2 nuevos del lote 2
  (`action_swap_panes`, `action_clone_path`) que no existían entonces.
- Dibujar glifos reales (no cuadrados) para `swap_panes` (flecha doble ⇄) y
  `clone_path` en `gen_icons`, y generar SOLO esos dos.
- **Endurecer `gen_icons`**: no sobrescribir un PNG existente salvo `--force`
  (default: solo crea los que faltan). Comentario de cabecera advirtiendo que es
  destructivo con `--force`. Así el accidente no se repite.

## 2. Glifos confiables + íconos propios de los botones de panel

**Causa:** en modo Glifos, varios símbolos (➕, ⇄, ⎘, ▾ del menú) son emoji/símbolos
que la fuente default de egui no cubre → se ven como "tofu" (cuadrito).

**Fix:**
- Auditar los glifos de la toolbar y reemplazar los que no rendericen por
  caracteres seguros que la fuente sí tenga (ASCII/símbolos básicos), o ajustar.
  Candidatos a revisar: `➕` (add_pane), `⇄` (swap), `⎘` (clone), `▾` (menú
  plantillas), `⟳` (refresh). Se elige por carácter, conservadoramente.
- En modo Pack, los íconos reales (del punto 1 + los dibujados de swap/clone)
  cubren add_pane/swap/clone. El menú `▾` de plantillas es un `menu_button` de
  texto: usar un glifo confiable o el ícono del pack.

## 3. Íconos copiar-ruta/favorito de la path-bar tapados por rutas largas

**Causa:** en `pathbar::breadcrumb_mode`, los íconos (📋 copiar, ☆/★ favorito) y los
breadcrumbs comparten el MISMO contenedor `right_to_left`. Con una ruta larga, los
breadcrumbs comen el ancho y tapan/empujan los íconos.

**Fix (reservar espacio):** pintar los íconos SIEMPRE primero en el layout
`right_to_left` con su ancho reservado; los breadcrumbs van en el sub-layout
`left_to_right` con el ancho RESTANTE y se recortan/elipsan si no caben. Los íconos
nunca se tapan. (El layout ya es casi esto; el ajuste es asegurar que el sub-layout
de breadcrumbs no exceda el ancho disponible tras reservar los íconos.)

## 4. Previsualización: activar/desactivar por extensión + alias de extensión

**Modelo (core::config + core::preview):** reemplazar
`preview_text_exts: String` (CSV) por:

```rust
pub struct PreviewRule {
    pub ext: String,            // sin punto, minúscula: "sif", "txt", "png"
    pub enabled: bool,          // check on/off de la preview
    pub treat_as: Option<String>, // "xml" => clasificar el .sif como .xml; None => por sí misma
}
// Settings.preview_rules: Vec<PreviewRule>
```

- **Clasificación:** `classify(path, rules)` busca la regla de la extensión del
  archivo. Si `enabled == false` → `None` (sin preview). Si `treat_as = Some(x)`,
  resuelve recursivamente como si la extensión fuera `x` (tope de 1 salto para no
  ciclar; el alias apunta a una extensión "real" de texto/imagen). Luego decide
  Text/Image/None por la extensión efectiva (imágenes fijas, texto = las marcadas).
- **Defaults:** las extensiones de texto semilla (las de hoy) + las de imagen, todas
  `enabled:true`, `treat_as:None`.
- **Migración:** un settings.json con `preview_text_exts` (CSV) o sin `preview_rules`
  se migra: cada extensión de texto → regla enabled; + las de imagen. Serde
  `#[serde(default)]` + un `From`/normalización al cargar.

**UI (Configuración → Previsualización):** tabla editable, una fila por extensión:

```
[ext: TextEdit corto] [✓ enabled] [tratar como: combo] [🗑 quitar]
[+ agregar extensión]
```

- El combo "tratar como" ofrece *(propia)* + extensiones concretas (xml, txt, json,
  md, csv, log, ...). Mostrar extensiones concretas (más claro; el motor solo
  necesita saber si el target es texto/imagen, pero deja la puerta a resaltado por
  lenguaje futuro).
- Patrón de acción diferida: la tabla acumula cambios y se aplican tras pintar (no
  hace I/O). Persistencia por el watcher de settings existente.

## 5. Botón "abrir otra ventana de Naygo"

**Diseño:** `ActionIcon::NewWindow` nuevo en la toolbar. Al clic, lanza un NUEVO
proceso del propio ejecutable: `std::env::current_exe()` + `std::process::Command`
`.spawn()`, SIN argumentos. Arranca como un inicio normal → restaura el workspace
persistido (carpetas iniciales, layout). No clona los paneles vivos (más simple y
robusto; es lo que Nicolás pidió). Cada ventana = proceso independiente.
- Glifo confiable + ícono de pack `action_new_window` (dibujado en `gen_icons`).
- i18n ES/EN del tooltip.
- El spawn corre fuera del closure de egui (como los otros side-effects diferidos),
  vía el patrón `clicked = Some(ActionIcon::NewWindow)` → match que llama al método.

---

## Tests

- core::preview: `classify` con reglas (enabled/disabled, treat_as a texto, treat_as
  a imagen, alias a extensión desconocida → None, tope de salto). Migración del CSV
  viejo a `preview_rules`. Round-trip serde de `Vec<PreviewRule>` en settings.
- core::icon_kind: `ActionIcon::all()` incluye NewWindow con file_name único
  (actualizar el conteo).
- Verificación en vivo (Nicolás): íconos de toolbar reales (no cuadrados) en Pack y
  Glifos; íconos de path no tapados con ruta larga; tabla de preview (toggle, alias
  .sif→.xml); botón nueva ventana abre otra instancia.

## Fuera de alcance

- Resaltado de sintaxis por lenguaje (el `treat_as` deja la puerta abierta, pero el
  render sigue siendo monospace plano).
- Heredar la carpeta del panel activo en la nueva ventana (arranca del workspace
  persistido; suficiente para el pedido).
- Rediseñar el set de íconos completo (solo se restauran los reales + se dibujan los
  3-4 nuevos como placeholders honestos).
