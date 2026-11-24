# Naygo — RP-4A: cancelación, concurrencia y visibilidad del preview

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance y límites

Continuación local de RP-3 sobre versión 0.5.1; no modifica esquema 4, dependencias ni
preferencias personales. No instala sobre la instancia activa ni publica en GitHub.

- Cancelar desde debounce/capacidad y con miniatura 3D; estados terminales y Reintentar
  explícito. La etiqueta reutiliza una clave traducida en los diez idiomas.
- Dos plazas por controlador de preview, compartidas por lectura, carga de geometría
  y raster. El hilo retiene el permiso aun si se descarta su receptor o se cancela.
  Canal acotado a un resultado; última selección y cámara pendiente, sin colas ilimitadas.
- Canales exclusivos por solicitud: A → B → A no acepta resultados del primer A.
- Ocultar/minimizar, pestaña inactiva y otro panel maximizado cancelan y liberan caché
  de vista/escena. Mostrar de nuevo sólo carga el objetivo vigente; no reintenta fallos.
- Cámara 3D coalescida sin releer geometría en cada gesto; se presentan frames válidos de
  la escena actual mientras se prepara la última cámara, evitando congelar el arrastre.
  Relleno de triángulos consulta cancelación durante scanlines, además de por triángulo.

**No es el cierre total de RP-4.** RP-4B conserva aislamiento/terminación no cooperativa,
supervisión y presupuestos de memoria/CPU por lector. Un hilo bloqueado puede seguir vivo
y ocupar una plaza; si ambas están ocupadas, la última selección espera y puede cancelarse.
No se anuncia un límite global de MiB, timeout real ni sandbox. Animaciones siguen pendientes.

## Validación

Primera comprobación de compilación aprobada (`rp4a-check.log`). Suite final:
**1.274 aprobadas, cero fallidas y seis ignoradas preexistentes**, registro
`rp4a-verified-tests.log` (compilación 8 min 42 s incluyendo espera de bloqueo anterior).
Desglose: 871 core, 22 integración core, 47 platform, 326 UI y ocho integración visual.
Clippy `--workspace --all-targets --locked -- -D warnings` aprobado sin advertencias
(`rp4a-clippy.log`, 3 min 06 s). Nueva distribución generada y verificada.
Los registros se guardan bajo `target/agent-out/`, fuera de Git.

La primera suite no compiló por una construcción incorrecta del fixture de `Drop`
(`rp4a-tests.log`, E0509). Se corrigió inicializando el estado antes de asignar tokens;
no se retiró la prueba ni se rebajó la comprobación. Repetición completa aprobada en
`rp4a-verified-tests.log`. Paridad de claves/placeholders de diez idiomas aprobada.
Los eventos nativos de foco/tamaño/ocultación despiertan el tick una sola vez para
reanudar tras restauración; no se añadió polling permanente mientras el preview está oculto.

Render software inspeccionado a 420 × 240: `rp4a-preview-cancel.png` y
`rp4a-preview-retry.png`. Colores de prueba exclusivos para localizar los controles;
imagen sintética de 2 × 2 transparente, no un modelo real. Comprueba el estado de vista
de imagen ya cargada sin `loading`, botón visible y callbacks de clic, no calidad de un STL.
No hubo cambios de layout inesperados en estas capturas. Formato y diff aprobados.
Grafo AST: 7.772 nodos, 14.078 aristas y 802 comunidades (`rp4a-graphify-final.log`);
29 archivos sin nodos informados por el extractor (incluye JSON), no cobertura exhaustiva.

Pruebas nuevas: capacidad con hilos controlados, descarte de canales, estados terminales,
visibilidad, cierre, drop de tokens, cancelación antes de I/O, coalescencia 3D y Cancelar/
Reintentar sobre el componente Slint real. Render software con datos sintéticos; no usa
la sesión instalada. Aceptación de ventanas nativas/minimización, DPI, Narrador, red real
y modelos grandes continúa pendiente. Sin benchmarks globales de consumo en esta entrega.

## Distribución

**0.5.1+build.202609092050**, generado el 9 de septiembre de 2026, America/Santiago.
`scripts/build-release.ps1` ejecutado solo, un job: release 27 min 13 s; Inno Setup
93,203 s (`rp4a-release.log`). Instalador a las 21:18:15, portable a las 21:16:42.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| dist/Naygo-0.5.1-setup.exe | 52.164.726 | `d55eb5af7f5ebe099e3fc77d82b47a622dad7c23500c27638f8e5b9b1855a6a9` |
| dist/Naygo-0.5.1-portable.zip | 13.176.772 | `448f80a713873ad518952c7a76bff56a1e9c6788da339352bdc9de4321105e56` |
| target/release/naygo.exe | 36.233.216 | `05b6b2ce53b88949efb6ce1b88cd4f62fe802b3781cbc1972ed5e6edccd51726` |
| target/release/naygo.pdb | 339.734.528 | `bf67c826e3e987e2ee4b1b67e0aa7216fe4d2e16b44d97e5bb5b0342b5710642` |

Ambos paquetes coinciden con `SHA256SUMS.txt`; `7z t` aprobado (`rp4a-portable-test.log`).
ZIP con cuatro entradas exactas: LEEME.txt, LICENSE, naygo.exe y THIRD-PARTY-NOTICES.md.
Hashes internos del EXE/licencia/avisos coinciden con los archivos locales. Metadatos:
Naygo 0.5.1, Nicolás Groth/ISGroth, MIT; build ID confirmado en la salida de compilación.
Instalador `NotSigned`: no hay certificado configurado. PDB opcional en el instalador,
no incluido en el ZIP portable. No se ejecutó la instalación ni se publicó a GitHub.

Portable: +14.796 bytes respecto de RP-3, sin dependencias nuevas. Es tamaño comprimido,
no una medición del consumo de memoria al ejecutar Naygo.

Entrega RP-3 reemplazada y respaldada en `target/agent-out/baseline-051-before-rp4a/`.
Hashes de setup/portable comparados con su registro antes de reemplazarlos; se pueden
recuperar desde ese respaldo. RP-4B sigue pendiente junto con la aceptación instalada.
