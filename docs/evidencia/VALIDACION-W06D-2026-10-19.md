# Validación W-06D — unidades y expulsión fuera del hilo de UI

Fecha: 2026-10-19
Estado: implementación, gate, distribución supervisada e instalación completados.

## Cambio

- La consulta de unidades Win32 y la clasificación USB se ejecutan en un worker. La UI solicita y
  dibuja el último snapshot sin consultar discos ni ejecutar `DeviceIoControl` en callbacks.
- Las solicitudes repetidas durante una enumeración se coalescen en una única consulta posterior.
  La respuesta despierta el event loop y actualiza la barra de unidades.
- Construcción y restauración de árboles usan el snapshot disponible. Al llegar uno nuevo, las
  raíces se reconcilian conservando el estado de carpetas y las ramas expandidas de discos que
  continúan conectados.
- La expulsión libera primero los watchers propios de los paneles. Un worker reenumera unidades,
  comprueba que la raíz siga siendo extraíble y ejecuta la expulsión segura; el resultado vuelve a
  la UI para actualizar los paneles y notificar éxito o error. Ya no se consulta el dispositivo ni
  se expulsan unidades desde callbacks.

## Verificación

- `cargo test -p naygo-core --locked tree::tests::replace_drives_preserva_raices_fisicas_y_estado_de_unidades_conservadas -- --exact --nocapture`: 1/1.
- `cargo check -p naygo-ui-slint --tests --locked --jobs 1`: correcto.
- Revalidación cruzada Linux posterior a W-06D: `cargo check --workspace --tests --locked --offline
  --target x86_64-unknown-linux-gnu --jobs 1` pasó en 4m38s. Clippy `--all-targets -D warnings`
  pasó por paquetes (`naygo-core` + `naygo-platform`, y luego `naygo-ui-slint`); aparece únicamente
  el warning esperado de que el host Windows no puede embeber el recurso de icono `.exe`.
- El comando agregado `cargo clippy --workspace --all-targets` quedó sin proceso `rustc` ni salida
  varios minutos y se canceló; los tres paquetes pasaron al repetirlo por partes. No se afirma un
  cierre de ejecución nativa Linux.
- `scripts/test-all.ps1`: **1.476 aprobados, 8 ignorados**, Clippy sin avisos (`-D warnings`) y formato correcto.
- La distribución se regeneró con `scripts/build-release-guarded.ps1`: Release 37m32s, Inno Setup
  142,110 s, pico del Job 10,65 GiB bajo el límite de 12 GiB.
- SHA-256: portable `7f7f70368099e85332547770d3ed975723d23da94548545adf3705b04e8d5fa8`, setup
  `fa19ff5bdc9cb672a6688a56ceb02d2621eff1a29b9bca8d16bc15de09500540`, EXE
  `6015599ce33acd41b5763611cc4fbfd84a49ce6a81df0652cbc3394eef3a2c60`. Manifiesto y contenido
  del ZIP verificados; el ZIP incluye `naygo.exe` y no incluye PDB. Setup y EXE siguen sin firma
  Authenticode (W-14).
- Instalación in-place completada con código 0 en `%LOCALAPPDATA%\Programs\Naygo`;
  Naygo quedó abierto como PID 42728, ProductVersion 0.5.3, ventana principal presente y responde.
- UIA pasiva sobre el proceso instalado: 821 descendientes, 654 con nombre y 190/190 controles
  enfocables nombrados; árbol sin truncamiento. Esto no equivale a una prueba con Narrador.
- Los SHA-256 de `settings.json`, `workspace.json`, `favorites.json`, `templates.json` y
  `naygo.pdb` se mantuvieron iguales antes/después. Respaldo preventivo en
  `target/agent-out/install-preserve-053-20261019-w06d/`; los cuatro JSON parsean.

## Límites

- La selección visual de expulsión refleja el último snapshot recibido; el worker revalida antes de
  ejecutar. La comprobación sigue basada en la letra de unidad de Windows.
- No se ejecutó una expulsión real de un dispositivo conectado durante este gate. La verificación
  manual con un USB real continúa pendiente.
- Guardar y borrar temas personalizados aún tiene operaciones de archivo en callbacks; el hallazgo
  se registró como W-06E y espera decisión de alcance.
