# Validación de ruta en el menú de Favoritos — 2026-10-17

## Cambio

El menú de favoritos de la barra muestra, a la derecha del alias, la ruta abreviada con texto
atenuado y de menor tamaño. Reutiliza `path-hint` del modelo compartido con el panel Favoritos:
los grupos no presentan ruta y las filas de favorito conservan su ruta completa para navegar.
Abrir el menú no realiza I/O.

## Verificación

- `cargo fmt --all -- --check`: aprobado.
- `cargo test -p naygo-ui-slint favoritos_arbol_se_aplana_con_grupos_y_respeta_colapso`: 1 aprobada.
  Verifica que los grupos no tengan `path_hint` y las hojas favoritas sí.
- `scripts/build-release-guarded.ps1 -Verify`: workspace completo, 1.447 aprobadas, 8 ignoradas,
  cero fallidas; Clippy con `-D warnings` sin avisos. Pico de Job 5,28 GiB, dentro del límite de
  12 GiB. Registro: `target/agent-out/guarded-20261017-192600-941.out.log` y `.err.log`.
- `graphify update .`: actualizado; grafo de 9.085 nodos, 17.218 aristas y 856 comunidades.
- Empaquetado por `scripts/build-release-guarded.ps1`: aprobado con un trabajo Cargo, límite 12 GiB,
  margen mínimo 8 GiB; pico del Job 10,49 GiB. Release: 29 min 57 s. Inno Setup: 129,672 s.
- Portable: `dist/Naygo-0.5.2-portable.zip`, 14.365.055 bytes,
  SHA-256 `18b3a867d4e37ec11e429df251392d7ce783f56a3cdbbef661b9a9a7057759aa`.
- Instalador: `dist/Naygo-0.5.2-setup.exe`, 66.444.697 bytes,
  SHA-256 `fdd6b1e6afe34ccebac0a77888ae35d9c3d72ba389b2dd48be6e39a6b11ca649`.
- Los dos hashes concuerdan con `dist/SHA256SUMS.txt`; el ZIP contiene `naygo.exe`, `LICENSE` y
  `THIRD-PARTY-NOTICES.md`. El ejecutable dentro del ZIP coincide con `target/release/naygo.exe`
  (SHA-256 `357ee82a0a27b2dbae5cecbe35d63a6394e97315fd384cee9db4dd951d2b1561`).
- El instalador actualizó la instalación existente con código de salida 0. La app quedó abierta y
  responde (PID 38744); el EXE instalado coincide con `target/release`.

La distribución local continúa sin firma Authenticode. No se ejecutó una aceptación visual manual
del menú; revisar la presentación en la siguiente sesión del usuario.
