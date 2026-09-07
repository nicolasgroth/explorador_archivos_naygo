# Naygo 0.5.1 — tipografía, actualización de carpetas y controles

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

- Fuente de sistema para la interfaz por defecto; opción independiente para nombres,
  fechas y tamaños en tablas, árbol y bandeja (hereda interfaz); Consolas por defecto
  para texto/código del preview. Se usan fuentes instaladas, sin paquetes adicionales.
- Apariencia incluye sugerencias, nombre libre, muestra y restauración por función.
  Una fuente inexistente utiliza el fallback del toolkit. Opciones traducidas a diez idiomas.
  El guardado de fuentes se hace en worker; revisiones ordenan esas escrituras con los
  guardados habituales/de cierre, impidiendo que un snapshot antiguo pierda otros cambios.
- Botón local de refresco sin cambiar el panel activo; botón global refresca todos los
  paneles Files abiertos (incluidas pestañas), ramas expandidas del árbol y unidades.
  No se recorre todo el disco ni se instala un indexador. F5 mantiene su semántica local.
  El árbol restaura ramas en orden padre/hijo conforme llegan los listados, sin perder
  expansiones ni mantener el timer vivo por carpetas borradas/ocultas. Colapsar una rama
  cancela su restauración pendiente. Se añadió una regresión específica de ese recorrido.
- Watchers: el relay publicaba después de que el productor ya hubiera despertado la UI.
  Ahora el wake ocurre después de publicar la metadata resuelta. Un número de generación
  impide aplicar resultados viejos al navegar/cerrar/rearmar. El alta y la liberación nativa
  se hacen en worker para no bloquear con rutas de red. Refrescar rearma watchers fallidos.
- Bandeja: `set_vec` recreaba TouchArea durante la activación y perdía el gesto down/up;
  ahora las filas existentes se actualizan in situ. Se conserva selección por teclado/ratón,
  arrastre OLE y el contexto de Preview/Propiedades.
- Íconos de operaciones/unidades dentro de cajas de altura completa; dibujo centrado.
- Compilador Slint en hilo de build con 8 MiB de stack para evitar el desbordamiento de
  la pila principal MSVC. No afecta la memoria de la aplicación. El ID se renueva al cambiar
  fuentes de código y por nonce en cada ejecución del empaquetador; pruebas sin cambios
  pueden reutilizar compilación sin invalidación artificial permanente.

## Validación

- `cargo test --workspace --locked`: **1.246 aprobados**, seis smoke ignorados en la suite
  habitual (865 core, 22 integración core, 47 platform, 309 UI y tres integración visual).
  Registro: `target/agent-out/fonts-refresh-complete-tests.log`.
- Smoke nativo `dir_watch::tests::dir_watch_smoke` ejecutado adicionalmente: **aprobado**,
  con archivo creado en un directorio temporal aislado. `fonts-refresh-native-watch.log`.
- Pruebas nuevas de wake posterior al lote, generaciones, refresco local/global,
  persistencia/herencia tipográfica y exclusión de escrituras antiguas.
- Gestos reales de puntero en Slint headless: down/up durante actualización de fila por
  foco conserva selección; arrastre dispara su callback y no un clic adicional al soltar.
- Render por software y comprobación de centrado por píxeles de pausa/salto/unidad fija;
  inspección del USB y selector compacto. Imágenes `controls-Segoe-UI.png`,
  `controls-Consolas.png` y `controls-Naygo-Missing-Font-Test.png` en target/agent-out.
- Grafo AST: 7.550 nodos, 13.697 aristas, 776 comunidades; `fonts-refresh-graphify-complete.log`.
  Persisten avisos conocidos de archivos sin nodos y etiquetas históricas, sin extracción LLM.
- Formato/diff y paridad de los diez catálogos aprobados.

- Clippy `--workspace --all-targets --locked -- -D warnings` aprobado;
  `fonts-refresh-complete-clippy.log`. La suite, Clippy y el empaquetador no se ejecutan juntos.

Estas pruebas no equivalen a validar la sesión instalada de Nicolás.

## Distribución verificada

Build `0.5.1+build.202609071600`. `scripts/build-release.ps1` completado en solitario;
registro `target/agent-out/fonts-refresh-complete-release.log`.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `Naygo-0.5.1-setup.exe` | 51.583.215 | `8055d1e0a06b420e96f6c02f76348adaa960ace9f80a5c112a48524742f22b36` |
| `Naygo-0.5.1-portable.zip` | 12.612.113 | `047dd2ba05b99e6207cd2db90dc83536bb5dbb3a0d6345a8c0de6931b327ad66` |
| `naygo.exe` | 35.819.008 | `7f312a197547b31b24ebc906c9d29ec07b4bd5817ba3691974db937862898b03` |

Instalador generado el 2026-09-07 a las 16:28:36; portable a las 16:27:08, hora de Chile.
SHA256SUMS coincide con ambos paquetes. `7z t` aprobado; ejecutable y LICENSE dentro del
ZIP coinciden byte a byte con los originales mediante SHA-256. Metadatos: producto 0.5.1,
autor Nicolás Groth, ISGroth, correo y licencia MIT. PDB: 335.097.856 bytes, opcional en el
instalador y fuera del ZIP según T-3. No se instaló sobre la sesión del usuario.

Código y documentación se integran en `main`. `dist/`, `target/`, `graphify-out/` y `tmp/`
mantienen su exclusión de Git; no se suben configuraciones ni datos personales.

## Comprobación instalada

1. Cambiar cada fuente, cerrar por completo y abrir; volver a sistema/heredar. Revisar
   TXT/XML, nombres largos, fechas, tamaños y temas claro/oscuro. Probar fuente inexistente.
2. Crear, modificar, renombrar y borrar un archivo desde otra aplicación con Naygo en reposo;
   verificar actualización sin mover el mouse. Repetir en unidad local y carpeta sincronizada.
3. Refrescar un panel secundario y comprobar que el foco no cambia; usar el botón global
   y revisar paneles abiertos/árbol. Una unidad desconectada debe poder reintentarse al volver.
4. Bandeja: clic, Ctrl+clic, Shift+clic, flechas, Ctrl+C, preview/propiedades y arrastre.
5. Copia pausada/reanudada y botones de unidades: alineación a DPI 100%, 125% y 150%.

El watcher depende de que Windows/proveedor de red entregue notificaciones. Un proveedor
sin soporte o una conexión interrumpida puede requerir refresco manual; esta revisión no
añade sondeo periódico costoso de carpetas. El instalador continúa sin firma digital.
