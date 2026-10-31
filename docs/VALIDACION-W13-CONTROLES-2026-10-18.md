# Validación W-13 — filas y acciones accesibles

Fecha: 2026-10-18 (America/Santiago)

## Cambio

- «Subir carpeta», recientes, menús de contexto y opciones del menú de filtro declaran roles,
  nombres y acciones de accesibilidad. Las acciones llaman a los mismos callbacks que los clics;
  las opciones de tipos anuncian el estado marcado.

- Las filas del listado exponen el rol de elemento de lista, nombre, tipo, selección, posición y
  acción predeterminada. La fila especial `..` se anuncia como «Subir»; su activación reutiliza la
  ruta normal de la interfaz. Las filas en edición de nombre no anuncian una acción de apertura.
- Las acciones dibujadas a mano del Inspector para calcular tamaño, copiar procedencia y quitar la
  marca de procedencia exponen rol de botón, nombre localizado y la misma acción que el control visual.
- La guía de usuario explica el comportamiento y deja explícita la validación con lector de pantalla
  pendiente.

## Validación local

- `cargo check --locked -p naygo-ui-slint`: aprobado para los controles de navegación, recientes y
  menús. La suite completa se encuentra en ejecución; distribución y aceptación manual pendientes.

- `cargo check -p naygo-ui-slint --locked`: aprobado.
- `scripts/test-all.ps1`: 1.460 pruebas aprobadas, 8 ignoradas; Clippy sin avisos y formato correcto.
- `git diff --check`: aprobado; solo informa conversiones CRLF/LF ya presentes en archivos editados.
- `graphify update .`: completado; 9.332 nodos, 17.658 relaciones y 885 comunidades.
- `scripts/build-release-guarded.ps1`: aprobado. Límite del Job 12 GiB, umbral mínimo del equipo
  8 GiB, pico 10,64 GiB. `cargo build --release` terminó en 30m58s; Inno Setup en 168,719 s.
  No se ejecutó en paralelo otra compilación ni se instaló el resultado.

## Distribución

- `dist/Naygo-0.5.2-portable.zip` — SHA-256
  `2344a2fecdf6c6872ddb4ffa609603508d765078b2825d3781a8e329cc63cf3c`.
- `dist/Naygo-0.5.2-setup.exe` — SHA-256
  `493f3594cfdf228a66cca7478984e374430448e66a1f03783da742dad3f93086`.
- `target/release/naygo.exe` — SHA-256
  `7917a0ca63af08369f0b87cb7d3f48554ccebc11a4de29262448ffb78ca44aa5`.
- Ambos paquetes coinciden con `dist/SHA256SUMS.txt`. `zipfile.testzip()` devolvió `None`, el ZIP
  contiene `naygo.exe` con el mismo SHA-256 que el release y no contiene archivos `.pdb`.
- El setup no tiene firma Authenticode. La build quedó en `dist/` y no se instaló.

## Pendiente

La inspección interactiva con UI Automation y Narrador sobre esta distribución, incluida navegación,
anuncio del estado seleccionado y acciones del Inspector, requiere una sesión de Windows desbloqueada.
La prueba estructural no se considera aceptación de lector de pantalla.

## Controles adicionales y distribución actual — 2026-10-18 23:05

- «Subir carpeta», las filas de carpetas recientes, las opciones del filtro por tipo y los menús de
  contexto/filtro declaran roles, nombres y acciones accesibles. El estado marcado del filtro se
  publica como valor de casilla; las acciones reutilizan callbacks de los controles visuales.
- `cargo check --locked -p naygo-ui-slint`: aprobado. `scripts/test-all.ps1`: 1.464 pruebas aprobadas,
  8 ignoradas; Clippy sin avisos y formato aprobado.
- `graphify update .`: actualizado, 9.363 nodos y 17.716 relaciones.
- `scripts/build-release-guarded.ps1`: aprobado; pico del Job 10,64 GiB y margen libre mínimo del
  equipo superior a 27 GiB. Release compilado en 33m51s; Inno Setup en 141,828 s.
- Portable y setup coinciden con `dist/SHA256SUMS.txt`. El ZIP pasa `testzip()`, contiene el EXE
  cuyo hash coincide con Release y no contiene PDB. Ambos artefactos siguen sin firma pública.
- Se instaló con la autorización previa y se dejó abierto Naygo PID 50496, respondiendo. El EXE
  instalado coincide con Release (SHA-256
  `1f93266279feae63839bbd125aaa30c181bf85b09f1c8457a2eb9e52c3c6db2f`); los cuatro JSON de usuario
  parsean correctamente y el PDB previo se conserva.
- UI Automation encontró 350 descendientes, 276 con nombre y 88 controles enfocables, los 88 con
  nombre; árbol completo. Esto es estructural, no aceptación con Narrador.

Distribución actual:

- `dist/Naygo-0.5.2-portable.zip` — SHA-256
  `85fac022818f2a02b93bba707aa156ce6e4200ccae1bf19b2e96d982feca4ddc`.
- `dist/Naygo-0.5.2-setup.exe` — SHA-256
  `028175df8265a2b68c06b703276b59aed069fb04f33daf4d4b8c73195a43ec36`.
- `target/release/naygo.exe` e instalado — SHA-256
  `1f93266279feae63839bbd125aaa30c181bf85b09f1c8457a2eb9e52c3c6db2f`.

Pendiente: probar navegación y acciones con Narrador sobre la instalación actual, incluidos menús,
filtros y diálogos, y completar el barrido de controles personalizados restantes.

## Acciones de preview, paleta y árbol — 2026-10-19

La revisión estática encontró acciones con clic sin una acción accesible equivalente. Se añadieron
roles y acciones predeterminadas ligadas a los callbacks ya existentes para cancelar/reintentar el
preview, copiar texto, alternar ajuste de línea/escala de imagen, restaurar la cámara 3D y abrir con
la aplicación predeterminada. Los toggles exponen su estado actual. Las filas del árbol ahora declaran
lista, nombre, posición, selección y navegación; el control de árbol enlazado anuncia su nombre y
estado. Los resultados de Ctrl+P anuncian la etiqueta visible y el atajo, y su acción accesible
reutiliza el mismo callback de selección. No se agrega contenido de archivo ni el payload de ruta al
nombre accesible de la paleta.

Validación local: prueba de UI focalizada y la suite de `naygo-ui-slint` (409 aprobadas, 2 ignoradas),
después `scripts/test-all.ps1` con **1.468 aprobadas, 8 ignoradas**, Clippy sin warnings y formato
correcto. Check y Clippy cruzados de todos los targets para `x86_64-unknown-linux-gnu` también pasan;
el único aviso señala que la compilación cruzada no puede embeber el recurso de icono `.exe` de
Windows. `graphify update .` actualizó el grafo a 9.393 nodos y 17.762 relaciones.

La reconstrucción supervisada 0.5.3 terminó con Release en 27m57s, Inno Setup en 120,094 s y pico
del Job de 10,65 GiB (límite de 12 GiB). SHA-256: portable `81db72979ab3eff99523798f5ca80aef55f79e7eda1b2c89453ab41a91e8c9b9`,
setup `ab80b50e4f065f7e2ea78dffdca2a7bf944cf9de7ffaf1bb638f6baaf7c15860`, EXE Release
`8b797c1fb6fd4028fa02727fdfae89f605126ae704735a7461260d08f23af199`. El manifiesto coincide,
el ZIP pasa integridad y excluye el PDB, y el EXE incluido coincide con Release. Setup y EXE siguen
sin Authenticode. Esta reconstrucción se instaló in-place el 2026-10-19 con autorización previa;
Narrador y el recorrido manual W-13 siguen pendientes.

**UI Automation del Release instalado, 2026-10-19:** PID 44896 usa el EXE Release por SHA-256 y
permanece abierto con ventana principal. La inspección pasiva obtuvo 375 descendientes, 302 con
nombre, 113/113 controles enfocables nombrados, ningún control enfocable anónimo y árbol sin truncar.
No registra rutas ni valores; es evidencia estructural y no sustituye Narrador ni activación real.
La actualización conservó los hashes de settings, workspace, favoritos, plantillas y PDB; los cuatro
JSON parsean. Detalle de instalación y backup de protección en
`target/agent-out/install-preserve-053-20261019-073712/`.
