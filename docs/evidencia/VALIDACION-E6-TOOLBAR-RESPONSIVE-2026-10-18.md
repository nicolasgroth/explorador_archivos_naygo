# E-6: toolbar adaptable y Espacios separado de Disposiciones — 2026-10-18

## Cambio

La toolbar mantiene navegación, ruta, búsqueda, favoritos, Espacios y Disposiciones visibles en una
sola fila cuando la ventana queda angosta. Las acciones secundarias, unidades y Configuración se
agrupan en «Más»; la lista tiene scroll e incluye navegar a las unidades, refrescarlas y expulsar las
extraíbles con seguridad. Al ampliar la ventana, vuelven los botones directos. El umbral se ajusta a
la preferencia de iconos o etiquetas y se actualiza desde eventos de redimensionado/preferencias para
evitar ciclos en el layout de Slint.

Espacios ahora tiene botón propio y abre el selector con sus recientes. Disposiciones conserva las
plantillas; su menú ya no mezcla los recientes ni el gestor de espacios. Los ítems del menú muestran
semántica accesible de botón y acción por defecto.

## Verificación

- `scripts/test-all.ps1`: 1.459 pruebas aprobadas, 8 ignoradas; Clippy `-D warnings` y formato
  aprobados.
- Una prueba headless de `AppWindow` con renderer por software verifica el breakpoint en cuatro
  anchos: con etiquetas, toolbar amplia a 1600 px y compacta a 1300 px; solo iconos, amplia a
  1100 px y compacta a 1000 px. La prueba no necesita sesión de escritorio y queda en el gate.
- `cargo check -p naygo-ui-slint --locked`: aprobado, sin avisos Slint tras cambiar el breakpoint a
  actualización imperativa en resize/preferencias.
- Check cruzado Linux con todos los targets:
  `cargo check -p naygo-ui-slint --all-targets --target x86_64-unknown-linux-gnu --locked --offline --jobs 1`:
  aprobado. Clippy cruzado Linux con `-D warnings`: aprobado. El aviso de falta de `windres` al
  compilar desde Windows es esperado y solo afecta al icono PE.
- `graphify update .`: actualizado a 9.311 nodos, 17.630 aristas y 878 comunidades.
- Build supervisado Windows: pico contable Job 10,63 GiB (límite 12 GiB; mínimo libre 8 GiB),
  release 27m03s e Inno Setup 112,859 s.
- Portable `dist/Naygo-0.5.2-portable.zip`: 14.490.595 bytes, SHA-256
  `fdede0fada598a4a84e0c56fb42e4b919f6cf9194befc04cd57e44c692cc9e9d`.
- Setup `dist/Naygo-0.5.2-setup.exe`: 66.865.215 bytes, SHA-256
  `7c75a707360b221cb5aa79cf466df13378b235933be15630337b8a20e1f2d0cb`.
- EXE Release y el incluido en el ZIP coinciden: SHA-256
  `54fed15ac5585b6089a78ce7ebf6e14842b6934f7febd9f118c3bb3a14562a50`. Los dos artefactos
  coinciden con `dist/SHA256SUMS.txt`.
- El ZIP contiene `LEEME.txt`, `LICENSE`, `naygo.exe` y `THIRD-PARTY-NOTICES.md`; no contiene PDB.

## Instalación local y aceptación pendiente

El setup se instaló silenciosamente el 2026-10-18 con los hashes cotejados contra el manifiesto.
El instalador salió con código 0. El EXE instalado coincide con Release: SHA-256
`54fed15ac5585b6089a78ce7ebf6e14842b6934f7febd9f118c3bb3a14562a50`. La instancia iniciada
después de instalar es PID 41292 y responde. El setup permanece sin firma Authenticode; no se publicó.
Una inspección UIA pasiva posterior expuso 751 descendientes y 79/79 controles enfocables con
nombre, sin truncar el árbol; no abrió el menú «Más» ni sustituye Narrador. La captura de ventana
para aceptación visual expiró dos veces tras renovar la referencia de Computer Use, por lo que no
se enviaron acciones ni se considera hecha la revisión visual.
Falta revisar visualmente la toolbar en una sesión
desbloqueada, en ancho amplio y angosto, con distintos temas y varias unidades; también comprobar
con UI Automation/Narrador los comandos del menú «Más». El botón de Espacios usa el icono de paneles
clonados del set activo porque los packs aún no tienen un icono específico de espacios; valorar un
glifo semántico propio para los packs existentes.
