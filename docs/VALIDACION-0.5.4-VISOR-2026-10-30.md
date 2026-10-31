# Validación 0.5.4 de avance — visor 3D, 2026-10-30

Se conectaron al visor los controles de color uniforme azul/blanco/cálido y de presentación
sólida/bordes/malla, con etiquetas localizadas y rasterización de la escena retenida en worker.
Las elecciones se reinician al cambiar de archivo. La malla usa profundidad para ocultar bordes
tras superficies más cercanas. Los tonos no son materiales del archivo; los materiales 3MF siguen
pendientes, igual que la prioridad de geometría frente a la miniatura y la validación visual real.

- Gate Windows: **1.509 pruebas aprobadas, 8 ignoradas**, Clippy y formato sin avisos. Incluye
  17 pruebas de mesh, ciclo de vida de los cambios y clics en controles Slint headless. Los diez
  catálogos pasan las pruebas de claves y placeholders. No sustituye revisión visual ni Narrador.
- Benchmark W-07 supervisado: 96.800 triángulos a 1024×768, mediana de siete rondas **18,483 ms**
  para el modo sólido. Es una ejecución aislada, sin comparación controlada con el build previo;
  bordes y malla no se midieron. Detalle en `docs/BENCHMARKS.md`.
- Build supervisado: Release **28m59s**, Inno Setup **117,578 s**, pico del Job **10,72 GiB** con
  límite 12 GiB y margen mínimo 8 GiB. Logs `target/agent-out/guarded-20261030-013422-718.*.log`.
- El ZIP contiene `naygo.exe`, `LICENSE`, `LEEME.txt` y `THIRD-PARTY-NOTICES.md`, sin PDB; el EXE
  extraído coincide por SHA-256 con el Release. Metadatos del setup: Naygo 0.5.4, ISGroth. El
  ejecutable y el setup están `NotSigned`; no se instalaron, firmaron ni publicaron.

| Artefacto | SHA-256 |
| --- | --- |
| `dist/Naygo-0.5.4-portable.zip` | `ce4e1c62366cc4c74c02252a3c1cf3f942098332381fa148509d7f7849152db5` |
| `dist/Naygo-0.5.4-setup.exe` | `7ac5c100e62004f1e424e9747e1a4cd688d4c1772e4e179800a81bd999e4a122` |
| `target/release/naygo.exe` | `5c2dad042302e455437f26f64bb4be2b199467cfe2b1ba607dbb36035bef334f` |

Es una distribución de avance, no el cierre de 7-Zip, los materiales 3MF, la revisión de temas,
W-07/W-13 interactivos, W-12 remoto, W-14 o Linux. El arrastre de carpetas desde 7-Zip todavía no
está corregido. No se modificó la instalación existente.
