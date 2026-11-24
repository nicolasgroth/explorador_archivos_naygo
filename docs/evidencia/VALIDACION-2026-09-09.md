# Naygo — validación de edición de rutas RP-1

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Base 0.5.1 `abdef402babd2c136744c67ee9833c2f67bf0f16`, rama local
`codex/rutas-preview-051`. No modifica esquema 4, preferencias personales ni sesión instalada.

## Alcance

Editor de rutas de panel, acceso directo, validación y autocompletado asíncronos con plazas
acotadas, protección de borrador/panel/revisión, adaptación AltGr y documentación consolidada.
Sin dependencias nuevas. RP-2 (toolbar), temas/configuración y preview/animación NO integrados.

Respaldo previo de setup/portable/SHA256SUMS, con SHA-256 coincidentes:
`target/agent-out/baseline-051-before-rp1/`. Permite volver al build del día 7 sin usar Git.

## Comprobaciones

Validación automatizada del código final aprobada. El empaquetado se registra por separado;
no confundir los resultados anteriores de la base con esta ejecución.

| Comprobación | Resultado | Registro en `target/agent-out/` |
| --- | --- | --- |
| Suite completa `--workspace --no-fail-fast --locked` | 1.255 aprobadas, 0 fallidas, 6 ignoradas preexistentes | `rp1-final-tests.log` |
| Clippy `--workspace --all-targets --locked -- -D warnings` | Salida 0, sin advertencias | `rp1-final-clippy.log` |
| Formato y diff | Aprobados antes del empaquetado | Puertas de la misma ejecución |
| Diez idiomas: claves y placeholders | Todos aprobados | `rp1-i18n.log` |
| Editor real: teclado y render software | Aprobado a 260 px; Esc visible, rutas largas, selección, cursor y undo | `rp1-editor-260.png` y suite |
| Grafo AST | 7.656 nodos, 13.902 relaciones, 791 comunidades | `rp1-graphify.log` |

Desglose: 867 core, 22 integración core, 47 platform, 315 UI y 4 integración visual.
La suite añade nueve pruebas respecto de la base; no se añadieron exclusiones.

Pruebas añadidas: sintaxis (comillas, Unicode, UNC/prefijos, longitud), validación de carpeta
frente a archivo/ausente, timeout/cancelación sin liberar workers vivos, A → B → A y cancelación
del autocompletado; editor Slint real para caret, selección, undo, error/resize, Enter/Escape
y sesiones. La prueba del filtro de AltGr no simula la distribución física de teclado Windows.

Pendiente manual: AltGr chileno/IME, navegación y cierre con operación pendiente, UNC real
lenta/caída, DPI/Narrador y validación del instalador en otra máquina. Se usan directorios
temporales en pruebas; no se borran archivos personales ni se cierra Naygo del usuario.

Comprobación instalada sugerida (sin operaciones destructivas):

1. Comprobar en Acerca de el build `202609091710`.
2. Abrir el lápiz o Ctrl+L/F4 y editar en medio con flechas, selección, pegar y deshacer;
   probar `\` con la combinación habitual de AltGr del teclado chileno.
3. Confirmar una carpeta válida (también relativa o pegada entre comillas); después
   probar una inexistente y comprobar que conserva la carpeta anterior y el borrador.
4. Salir con Esc (tecla o botón): deben volver los breadcrumbs y el teclado del listado.
5. Repetir cambiando/cerrando el panel mientras se comprueba una ruta lenta; el resultado
   no debe redirigir otro panel. La prueba con red real sigue pendiente de aceptación.

## Distribución

Distribución **0.5.1+build.202609091710**, terminada el 9 de septiembre a las 17:39 local,
mediante `scripts/build-release.ps1`, solo y con un trabajo de compilación. Salida 0;
release optimizado en 28 min 52 s e Inno Setup en 93,25 s. Sin firma digital (no hay
certificado configurado). No se instaló ni se ejecutó sobre la sesión del usuario.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/Naygo-0.5.1-setup.exe` | 51.742.296 | `5e080e0412ffce4565e7347114dc8c6251c703cd5e99bc5f7254ccbc425ebf18` |
| `dist/Naygo-0.5.1-portable.zip` | 13.093.713 | `c1c201c5b05b32c71ac6ec978b7d7588b98ee1ef3bd3e0349bc81de5280f0fde` |
| `target/release/naygo.exe` | 35.924.992 | `3c5f95351295e21ef090a0a1d01953057ca1921e589ac86483371cfa76da8b39` |
| `target/release/naygo.pdb` | 336.277.504 | `ef07c6c34c13c3105048c06ed39ea2358b2be46014204b1e6a38cae6016a198e` |

`dist/SHA256SUMS.txt` coincide con ambos paquetes. `7z t` aprobó las cuatro entradas del
portable; SHA-256 del EXE, LICENSE y THIRD-PARTY-NOTICES coinciden con sus fuentes locales.
El portable mantiene su formato liviano sin PDB; el instalador incluye el PDB y éste también
queda en `target/release/`. Metadatos del EXE: Naygo 0.5.1, Nicolás Groth, ISGroth y MIT.
El EXE creció 105.984 bytes respecto de la base (sin nuevas dependencias).

Registros: `target/agent-out/rp1-release.log` y `rp1-portable-test.log`. El instalador anterior
fue reemplazado en `dist`, pero sigue recuperable en `baseline-051-before-rp1/`.

## Procedencia recuperada

Artefacto GitHub `10109221926` (`naygo-review-source`), run `34364248486`, referencia
`3489130d8ee7b7f34a11ebeb09b063b4377fdca0`. Árbol aislado local:
`target/agent-out/referencia-rp-20260909/source/`. Integridad del ZIP aprobada, 956 archivos;
SHA-256 `a2b88ea4898716650038936af28a03afd9be026da2209344b62ec150a5535ef6`.
No se ejecutaron scripts de reconstrucción ni se hizo merge masivo.

SHA-256 de archivos ORIGINALES de referencia (no del código final adaptado):

| Ruta relativa | SHA-256 |
| --- | --- |
| `crates/core/src/path_input.rs` | `a7715697bc17e3f702448eecfa1b2c96568c5f6db85fe5911fde992025c8e039` |
| `crates/ui-slint/src/callbacks_pathbar.rs` | `33e0b7eb16e5147565bd25119faf870e3362eb6b11511f8c14ad723e6c3a1d1e` |
| `crates/ui-slint/ui/path-editor.slint` | `ee53ffdf6055a5adb6216d26fad4fdb88dfcfa345340dcd774311c8d195aad8f` |
| `crates/ui-slint/src/path_window_events.rs` | `ff968847de4475d6d3f32ee67add7eab404431e68e9ea2588e865402f5429ae6` |

La validación fallida inicial del intento experimental no se considera evidencia del
producto. Se mantiene recuperable la referencia para los siguientes bloques del plan.
