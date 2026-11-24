# Modificadores pegados al crear carpetas y navegar — 2026-10-18

## Hallazgo y corrección

El editor modal de nuevas carpetas toma el foco. Si Ctrl se usa para `Ctrl+Enter`, el evento de
liberación puede quedar dentro del modal y no llegar al panel de archivos. El controlador conservaba
ese estado y su ruta de doble clic podía interpretar como `Ctrl+doble clic` un gesto normal, abriendo
una carpeta en otro panel.

La apertura, confirmación y cancelación del modal ahora limpian Ctrl/Shift. Además, cada clic en una
fila sincroniza ambos modificadores desde el evento de puntero real antes de decidir si el doble clic
abre en el panel actual o en otro. Esto también evita heredar un Ctrl perdido después de operaciones
que dejan seleccionada una carpeta copiada o recién creada. La selección del elemento terminado se
mantiene intacta.

El comportamiento de entrada ya implementado se conserva: un `Enter` añade una línea, dos `Enter`
consecutivos confirman cuando existe al menos una carpeta válida y `Ctrl+Enter` confirma de inmediato.
Con el campo vacío no se encola una operación.

## Verificación

- Regresión headless: iniciar con Ctrl obsoleto, hacer dos clics sin modificadores sobre una carpeta
  y confirmar que navega en el panel actual.
- Regresión de integración: simular Ctrl/Shift obsoletos al abrir el diálogo y antes de aplicar; ambos
  quedan limpios, y la carpeta creada sigue seleccionándose al completar la operación.
- `scripts/test-all.ps1`: 1.460 pruebas aprobadas, 8 ignoradas; Clippy con `-D warnings` y formato
  aprobados.
- `git diff --check`: aprobado.
- `graphify update .`: actualizado a 9.327 nodos y 17.654 aristas.
- Build supervisado: pico 10,64 GiB, límite 12 GiB; Cargo release 29m44s e Inno Setup 125,250 s.
- Portable ZIP: SHA-256 `d2603deb61a2754363c0c91d34fd627355a6a68b63cbab4ee34175bdffdd0e6a`.
- Setup: SHA-256 `9aab4940bbf855b2518e330f38e9c442ec8d6759293c53ac2d118f8d924be0b2`.
- El ZIP pasa la prueba de integridad; el EXE incluido coincide con Release e instalado: SHA-256
  `94070d7b4dea91ebb9670cfb6f63abf63d8644f06556a4ff0a2c569c2fee3746`. Los artefactos coinciden con
  `dist/SHA256SUMS.txt`.
- Instalador sin Authenticode. Instalado con autorización previa; Naygo quedó abierto y respondiendo
  como PID 48792. No se publicó el artefacto.

## Límite de aceptación

Las regresiones cubren el estado del controlador y la navegación headless. Falta la comprobación
manual del gesto de teclado/ratón en la sesión Windows desbloqueada, en particular crear con `Enter`
doble y con `Ctrl+Enter`, y después navegar por doble clic. El ejecutable actualizado está instalado
y abierto para esa prueba.

## Regresión adicional de navegación, 2026-10-19

La integración `nav052_create_selects_after_completion_and_refreshes_tree_without_watcher` ahora
continúa después de crear y seleccionar la carpeta: dispara el doble clic nativo con Ctrl/Shift
desactivados y verifica que el mismo panel navega a ella. Resultado local: 1 prueba aprobada. Esto
refuerza el contrato del controlador; no sustituye la aceptación manual con el teclado y el mouse.
