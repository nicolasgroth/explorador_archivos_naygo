# Rediseño de la ventana Configuración — opción A "Aire y jerarquía"

> Autor: Nicolás Groth / ISGroth — 2026-06-10. MIT License.
> Mockup A aprobado por Nicolás ("me gusta harto más el A").

## Qué cambia

Evolución de la estructura actual (sidebar de secciones + contenido), con pulido
visual:

1. **Sidebar** (ancho 176 px): cada sección con **ícono** (glifo) + texto. La activa
   lleva **barra de acento de 3 px** a la izquierda + fondo tintado con el acento del
   tema + ícono en color de acento. Hover sutil en las inactivas. Filas de 32 px.
   Glifos: Apariencia 🎨 · Paneles ▦ · Atajos ⌨ · Idioma 🌐 · Avanzado ⚙ ·
   Acerca de ℹ (mismos glifos embebidos que ya usa la toolbar).
2. **Encabezado de sección**: título (17 px, fuerte) + **subtítulo** (12 px, débil)
   que resume la sección. Claves i18n nuevas `settings.<sección>.sub` ES/EN.
3. **Etiquetas de grupo**: pequeñas (11 px), débiles, en MAYÚSCULAS (se derivan con
   `to_uppercase()` del texto i18n) — reemplazan los `ui.heading` internos.
4. **Separadores** entre grupos con aire arriba y abajo.

Helpers compartidos en `settings_window/mod.rs` (`section_header`, `group_label`,
`group_sep`) para que las 6 secciones queden consistentes y el estilo viva en UN
lugar. Los colores salen SIEMPRE del tema activo (`app.active_theme`) — nada
hardcodeado, los 4 temas se ven bien.

## No cambia
Estructura de secciones, controles existentes, comportamiento, atajos, "Acerca de"
(ya tiene su diseño centrado propio; solo consistencia de título si aplica).

## Verificación
Visual en vivo (computer-use): captura de cada sección comparada contra el mockup A;
glifos del sidebar renderizan (si alguno sale en blanco, se sustituye por un glifo
del set embebido); tema Light también legible.
