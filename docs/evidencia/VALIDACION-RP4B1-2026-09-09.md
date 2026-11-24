# Naygo — RP-4B1, lectores pesados aislados

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance y límites

Continuación autorizada después de RP-4A. El WebP aportado se inspeccionó sólo en lectura:
4.362.248 bytes, 800 × 450, 50 ANMF, 5.000 ms por ciclo y loop 0 (infinito). No se copió
al repositorio ni se conserva aquí su ruta. El preview sigue mostrando una imagen estática;
no se anuncia RP-5 completado ni se confunde la prueba del poster con animación.

RP-4B1 contiene imágenes/SVG/PDF/comprimidos en el mismo EXE, mediante modo interno anterior
a UI/configuración/logging/instancia única. Nada se instala ni se publica durante las pruebas.
No se modifican preferencias personales, esquema 4, atajos, cámara, scroll ni paquetes de íconos.
`serde` ya estaba fijado en Cargo.lock: sólo se declara su uso directo en UI. Se habilitan
JobObjects/Diagnostics_Debug de la misma versión de `windows`; no se actualizan dependencias.

192 MiB de memoria comprometida por hijo, 5 s CPU y 5 s de pared. Es una alternativa menor
que los 320 MiB propuestos por el intento recuperado, no una promesa de consumo total. Hasta
dos supervisores/lectores en la capacidad existente, incluidos retirados; cada uno añade un
hilo de pipe. El Job se aplica antes de enviar la petición, no después de decodificar. Un
fallo de ejecución no reactiva la tarea. El hijo usa prioridad inferior a normal y Rayon con un
hilo, sólo en su entorno; no cambia la prioridad ni el entorno del explorador.
Un fallo al asignar o comprobar el Job es terminal; no ejecuta el lector dentro de UI como fallback.
Muerte del padre cierra el Job; el proceso no puede crear descendientes dentro de él.
Job Object no es sandbox: no restringe todos los accesos del lector ni reemplaza validación.

IPC NGP1: petición ≤256 KiB; texto ≤1 MiB; imagen ≤2048 por lado y RGBA exacto validado
antes de reservar; tipo/versión/UTF-8, EOF y salida exitosa obligatorios. Cancel no atraviesa
ese pipe: el supervisor mata/recolecta y luego une el hilo, fuera de UI. Reintento explícito.
Imagen añade entrada real ≤20 MiB y límites del decoder de dimensiones/reserva best-effort;
SVG elimina el mínimo de escala que permitía exceder 2048 y referencias a imágenes externas.

Pendientes explícitos: interrupción de I/O bloqueado de texto/3D, presupuesto global que cuente
UI/cachés/escenas, optimización de PDF/TAR/texto, relojes/colas/composición/pausa de animación,
medición integral de navegación, aceptación instalada/VM/DPI y red real. RP-4 sigue parcial.

## Evidencia

**Validación automática final aprobada y distribución verificada.** Registros en
`target/agent-out/rp4b-*.log`, fuera de Git. Primera suite y repetición final:
**1.287 pruebas aprobadas, seis ignoradas preexistentes, cero fallos**
(`rp4b-full-tests.log`, `rp4b-verified-tests.log`). Incluyen reserva fallida real bajo límite
de memoria, proceso ocupado terminado por presupuesto CPU, cancelación/timeout con stdin
lleno, IPC truncado y muerte al cerrar Job. Clippy final sin advertencias
(`rp4b-clippy-final.log`, 1,98 s; análisis inicial `rp4b-clippy.log`).
Las diez traducciones pasaron paridad de claves/placeholders; formato y diff comprobados.
Grafo AST final: 7.845 nodos, 14.257 aristas, 809 comunidades (`rp4b-graphify-final.log`);
el extractor sigue informando 29 fuentes sin nodos, incluidos JSON: no es cobertura exhaustiva.

Nuevas pruebas: dos del protocolo core, siete de procesos platform, dos de enrutamiento UI y
dos del ejecutable real. PNG exacto, SVG con canvas enorme y referencia externa desactivada,
ZIP, PDF con texto, imagen corrupta, petición inválida. No usan ni cierran la sesión instalada.
Sin prueba visual nueva de la app completa: no hubo cambios del layout de Slint en este bloque.

Prueba inicial con ejecutable debug (`rp4b-real-reader-tests.log`): dos tests aprobados.
Tres lecturas estáticas de la muestra privada, 849–1.164 ms y 18.333.696–18.366.464 bytes
comprometidos de pico por hijo. Son cifras sin optimización, con otras compilaciones activas;
no describen reproducción ni fluidez. La medida final sobre release se detalla a continuación.

## Distribución y prueba del ejecutable final

**0.5.1+build.202609092205**, America/Santiago. `scripts/build-release.ps1` solo, un job:
release **31 min 16 s**, Inno Setup **98,234 s** (`rp4b-release.log`, salida 0).
Portable generado a las 22:33:41; instalador a las 22:35:20 del 2026-09-09.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| dist/Naygo-0.5.1-setup.exe | 52.385.639 | `e372e656c880cda8e3ea4d4455c6a0d106d6634fe7e1821008d0b7860312aeea` |
| dist/Naygo-0.5.1-portable.zip | 13.223.741 | `778f97f66f14e7d0cac71ece20cca9f0569c63db314f3b3ba113a90a7547a85d` |
| target/release/naygo.exe | 36.333.056 | `b99e727b6baab3c76066f7810e563b35af020a6befb2160afd233f698b392f76` |
| target/release/naygo.pdb | 340.733.952 | `7b722bfab006c19027580a603cbefaeddfa9341ce7e380f2c016c283a51f32f9` |

Hashes comparados automáticamente con `SHA256SUMS.txt`; `7z t` aprobado
(`rp4b-portable-test.log`). ZIP con cuatro entradas exactas: LEEME.txt, LICENSE, naygo.exe
y THIRD-PARTY-NOTICES.md. Hashes de EXE/licencia/avisos internos iguales a sus fuentes locales.
Metadatos de EXE: Naygo 0.5.1, Nicolás Groth/ISGroth, MIT. Build ID comprobado en la salida
del build. Instalador `NotSigned`, sin certificado configurado; PDB opcional en instalador,
no en portable. Sin instalar sobre la sesión activa ni publicar a GitHub.

Se ejecutó el driver ya compilado `preview_process-449ab7b7e130c6ce.exe` con
`NAYGO_PREVIEW_EXE` apuntando al EXE release y `NAYGO_PREVIEW_SAMPLE` a la muestra privada,
sin compilaciones paralelas. **Dos pruebas aprobadas**, 1,56 s en total
(`rp4b-release-reader-tests.log`): petición inválida, PNG, SVG, ZIP, PDF con texto, corrupto
y tres lecturas del WebP. El parámetro de EXE alternativo existe sólo en el driver de tests.

WebP: **336 ms en la primera lectura observada, 81 y 87 ms en las siguientes**;
18.374.656–18.395.136 bytes comprometidos de pico por hijo. Incluye arranque/IPC/lectura;
no es tiempo por frame de animación ni benchmark controlado de caché/navegación.
Corpus sintético restante: 43–167 ms; pico máximo observado 27.160.576 bytes (SVG).
No extrapolar estas cifras a PDF comprimidos grandes, modelos 3D ni consumo global de Naygo.

Portable +46.969 bytes respecto de RP-4A (tamaño comprimido, no memoria en ejecución).
Distribución RP-4A reemplazada y recuperable desde
`target/agent-out/baseline-051-before-rp4b1/`; setup/portable/SHA256SUMS respaldados y
comparados por hash antes de reconstruir. Aceptación instalada/VM/DPI/red pendiente;
RP-4 sigue parcial y RP-5 todavía no implementa reproducción animada.
