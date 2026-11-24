# Espacios con Favoritos globales o propios — 2026-10-18

## Cambio

Al crear un `.naygospace`, el usuario puede elegir Favoritos globales compartidos o una lista propia.
Por decisión de Nicolás del 2026-10-18, la lista propia parte como una copia del árbol global actual
y queda dentro del documento de espacio;
editarla marca el espacio como pendiente de guardar y no modifica `favorites.json`. Los espacios v1
se leen como globales. Rutas privadas también participan en el mapeo de espacios portables y en la
detección de referencias bajo una raíz.

El selector rápido/catálogo de perfiles (Trabajo, Vacaciones, etc.) sigue pendiente. La selección se
ofrece dentro del diálogo **Guardar como / exportar…**; no agrega un acceso directo de cambio rápido.

## Verificación

- `cargo test -p naygo-core --locked task_space::tests`: 11 aprobadas; cubre migración v1, scope,
  validación de payload, límite y rutas portables.
- `cargo test -p naygo-ui-slint --locked per_space_favorites_do_not_change_global_tree_and_return_with_workspace`:
  1 aprobada; valida aislamiento, guardado, reapertura y cambio de vuelta a Favoritos globales.
- `scripts/check_i18n_lang.py` para de/en/es/fr/hi/it/ja/ko/pt/zh: todos OK.
- Las etiquetas nuevas están traducidas en los diez catálogos, no usan el texto inglés como fallback.
- `scripts/test-all.ps1`: 1.450 aprobadas, 8 ignoradas; Clippy `-D warnings` y formato aprobados.
- `graphify update .`: grafo actualizado con 9.207 nodos, 17.471 aristas y 866 comunidades.
- `scripts/build-release-guarded.ps1`: aprobado; pico del Job 10,49 GiB, release 29m59s, Inno Setup
  117,578 s. Sin firma Authenticode.
- Portable `dist/Naygo-0.5.2-portable.zip`, 14.374.488 bytes, SHA-256
  `e26dfe32f8cae5b6a46cc5f77263d54a4d88418993dbf9a423a2d27e4e739600`.
- Setup `dist/Naygo-0.5.2-setup.exe`, 66.006.818 bytes, SHA-256
  `46323eb921f9eaebeea60f7bdaebcb3549c87cd76aa2d1468b6f0437200bef77`.
- Los hashes concuerdan con `dist/SHA256SUMS.txt`; el ZIP incluye `naygo.exe`, `LICENSE` y
  `THIRD-PARTY-NOTICES.md`; su `naygo.exe` coincide con el release, SHA-256
  `eeb84a4cc2cac575bd85328b3c235dba0173f7c19afcc3faaff5a00720622514`.

No se instaló esta compilación ni se hizo aceptación visual manual del diálogo. Los cambios de
Favoritos propios se persisten solo al actualizar/guardar el `.naygospace`; rutas ausentes/remotas
conservan el manejo normal de cada panel.
