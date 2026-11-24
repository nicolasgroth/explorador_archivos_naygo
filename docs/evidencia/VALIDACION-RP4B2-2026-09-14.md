# Naygo 0.5.2 — texto/3D aislados y temporales acotados (RP-4B2)

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance implementado

- Texto y resaltado, poster 3D y carga de geometría real usan el lector supervisado.
  Cancelación y timeout terminan el proceso aunque la llamada al filesystem esté bloqueada.
- Protocolo de texto coloreado con límites antes de reservar, validación de reconstrucción
  y conservación de reglas/truncado/copia. Gramáticas y parser permanecen en el hijo.
- Escena binaria de hasta 48 MiB, transmisión/recepción directas con tubería bufferizada.
  Conteos, coordenadas finitas, índices y dimensiones se validan. La cámara reutiliza el Arc
  de escena retenida sin abrir archivos ni procesos nuevos por gesto.
- Margen de trabajo de 256 MiB por plaza / 512 MiB compartidos, sin preasignación de RAM;
  incluye hijo de 192 MiB y respuesta mayor de 48 MiB. Retirados conservan la reserva.
- Raster local con scratch máximo de 64 MiB y proyección transformada in-place.
  Presupuesto de retención previo de 64 MiB sin cambios.

## Verificación

**1.324 pruebas aprobadas, cero fallidas, seis ignoradas preexistentes**: 882 unitarias
core, 22 integración core, 54 platform, 350 unitarias UI y 16 integración UI.
La suite completa se ejecutó dos veces; evidencia final: `target/agent-out/rp4b2-tests-final.log`.
Clippy de todo el workspace/todos los targets, con advertencias como errores, aprobado:
`target/agent-out/rp4b2-clippy-final.log`. Se corrigieron dos aserciones constantes de una
prueba, moviéndolas a evaluación en compilación, y se repitió la suite completa.
El primer `cargo check` había detectado metadatos desfasados al editar constantes durante
la compilación; las ejecuciones finales se hicieron con las fuentes estabilizadas.

Formato y `git diff --check` aprobados; validación de claves/placeholders aprobada en
los diez idiomas (ES/EN/DE/FR/HI/IT/JA/KO/PT/ZH). El instalador/portable RP-5B y su lista
de hashes se respaldaron en `target/agent-out/baseline-052-before-rp4b2/`; cada copia
coincide por SHA-256 con el original.

Grafo AST actualizado sin llamadas semánticas: 8.091 nodos, 14.727 relaciones y 813
comunidades, incluyendo el supervisor de recuperación (`target/agent-out/rp4b2-recovery-graphify.log`). Conserva la advertencia conocida de
29 fuentes sin nodos (catálogos y similares); no se ejecutó etiquetado con LLM.

## Recuperación y primeros intentos de distribución (mañana)

El primer build quedó interrumpido por el reinicio forzado reportado por Nicolás.
Windows registra apagado inesperado a las 08:19:50 y arranque a las 08:31:59 (eventos
6008/41), sin causa concluyente en los eventos consultados. No atribuirlo sin evidencia
a falta de memoria ni a Naygo. El log acabó durante compilación, antes del empaquetado.

Después del reinicio se verificaron los hashes de setup/ZIP/checksums contra su respaldo;
los tres siguen intactos. EXE y PDB release también coinciden con RP-5B. No quedó ningún
compilador activo ni stage parcial. Git sin corrupción (objetos sueltos conservados),
metadata Cargo offline/locked, formato y diff correctos. No se limpió el workspace.

Se añadió `scripts/build-release-guarded.ps1`: ejecuta el empaquetador original bajo un
Job de 12 GiB, prioridad baja y parada si el margen físico/comprometido baja de 8 GiB.
No altera el perfil release ni los ajustes del SO. Prueba de límite/prioridad aprobada:
`guarded-20260914-094038-819.out.log`. El contador pico incluyó la solicitud rechazada;
no interpretarlo como RAM efectivamente usada.

El primer intento supervisado (`guarded-20260914-094521-002`) alcanzó el límite de
12 GiB y terminó por reserva rechazada antes del empaquetado. El último margen observado
del equipo superaba 16 GiB; esto comprueba el límite del nuevo intento, no explica el
reinicio anterior. Tras comprobar que no quedaban compiladores y que había más de 28 GiB
disponibles, se inició otro intento con `-MemoryGiB 16`, conservando margen mínimo de
8 GiB, prioridad baja, un solo trabajo y perfil original. Logs
`target/agent-out/guarded-20260914-095530-558.out.log` y `.err.log`.

El segundo intento también terminó con `Allocation failed` al alcanzar el límite de
16 GiB (último margen observado: 12,36 GiB), sin llegar al empaquetado. No se atribuye
el código de terminación del compilador a un fallo del producto ni al reinicio anterior.
No se hizo un tercer intento ni se alteraron LTO, símbolos, codegen-units o configuración
del equipo. Continuación: acordar más margen disponible para el build o evaluar un perfil
de compilación de menor consumo, con sus consecuencias de rendimiento/diagnóstico.

Comprobación final a las 10:21 locales: no quedan cargo/rustc/ISCC ni `portable-stage`.
Los cinco artefactos anteriores siguen coincidiendo exactamente con sus hashes RP-5B:

| Archivo anterior conservado | SHA-256 |
| --- | --- |
| `dist/Naygo-0.5.2-setup.exe` | `c56c8609d0fdef860621e8117320678a54bd1aa922a3a051d63d813ac45ca0c1` |
| `dist/Naygo-0.5.2-portable.zip` | `9f73a0dc0748cc99dc50624facc5b5e2476f3eb16122e770252963bad99ac18b` |
| `dist/SHA256SUMS.txt` | `8567e4c1069550f27e54f70808c3e108d3c66bb0c0d24d73d27d1baff7bfb010` |
| `target/release/naygo.exe` | `0211c9201c92b9b0fb6d0133d25d5d27f46b648ef49de2e57769f81b7427b90f` |
| `target/release/naygo.pdb` | `c4648b8f8b188053bf2d8b610a61070fff08fa3a7bb771a37282c96e2183acde` |

Revalidación tras el reinicio: **1.324 pruebas aprobadas, seis ignoradas preexistentes**
y Clippy sin advertencias, bajo el supervisor. Logs `target/agent-out/guarded-20260914-094040-951.out.log`
y `.err.log`; pico contable 0,70 GiB, margen observado mayor que 27 GiB. En aquel corte
los nuevos paquetes quedaron pendientes por límite de memoria de compilación.

## Distribución verificada (noche)

Tras la autorización de Nicolás se ajustó el perfil a ThinLTO/16 unidades conservando
símbolos y optimización de tamaño. Build supervisado a 12 GiB aprobado (pico 10,44 GiB).
Suite repetida: **1.324 pruebas**, seis ignoradas preexistentes; Clippy limpio.
**Siete pruebas del lector contra el EXE release aprobadas**, incluida lectura bloqueada:
cancelación 21 ms y timeout 4.999 ms. ZIP íntegro, entradas comparadas con originales,
hashes/versión/autoría comprobados. Instalador/portable **0.5.2+build.202609142135**
generados y verificados (14 de septiembre, 22:04–22:05 local). Sin instalación ni publicación.
Logs, tamaños, hashes y trade-offs en [validación de build](VALIDACION-BUILD-2026-09-14.md).

## Límites y continuación

No es un techo ni medición de RSS global: asignador, pilas, modelos Slint y resto de la
aplicación quedan fuera. Respuestas terminadas se acotan por capacidad uno/tamaño máximo,
separadas del trabajo vivo. No se cambian dependencias, versión 0.5.2 ni esquema de settings.

Pendientes: RP-6 (PDF/TAR/texto/SVG, métricas opt-in y listado medido), aceptación instalada
en red/DPI/VM/Narrador y mediciones comparables de consumo global. No se instala sobre la
sesión personal ni se publica a GitHub en este bloque.
