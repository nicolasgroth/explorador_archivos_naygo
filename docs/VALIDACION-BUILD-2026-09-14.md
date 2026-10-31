# Naygo — auditoría de espacio y compilación de menor consumo

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Espacio local

Inventario de archivos del proyecto, sin seguir enlaces/junctions. Tamaños lógicos en GiB:

| Directorio | GiB | Decisión |
| --- | ---: | --- |
| `target/debug/incremental` | 144,61 | Caché regenerable. Eliminación bloqueada por la herramienta; **no eliminada**. |
| `target/debug/deps` | 33,43 | Conservado: bibliotecas y ejecutables de pruebas reutilizables. |
| `target/debug/build` | 3,14 | Conservado: salida de compiladores de UI/dependencias. |
| `target/release` | 6,58 | Conservado: artefactos, símbolos y dependencias release. |
| `target/tmp` | 1,75 | Conservado: reproducciones históricas y evidencias, no tratar todo como basura. |
| `target/agent-out` | 0,60 | Conservado: respaldos y registros de validación. |
| ZIP originales en `assets/icons` | 4,56 | Conservados: fuentes descargadas de íconos, no forman parte del instalador. |
| `tmp` | 0,18 | Conservado: muestras personales y fuentes de íconos. |

La caché incremental es el principal candidato: 155.269.196.316 bytes lógicos, no una
medición de espacio físico liberado. Ruta exacta:
`target\debug\incremental` dentro del checkout.
Puede eliminarse manualmente con todas las compilaciones detenidas; Cargo la regenera.
La siguiente recompilación de desarrollo puede tardar más. No borrar `target` completo:
contiene respaldos/evidencias y el ejecutable validado. No se limpió Temp global, caché
global de Cargo, Git, configuración personal ni originales. No se eludió el bloqueo
de la herramienta intentando otro mecanismo de eliminación.

## Dependencias y código

Revisados manifests/metadata Cargo y referencias en fuentes Rust, incluidos tests,
generadores y build scripts. Todas las dependencias directas efectivas tienen referencias;
esto no equivale a una prueba exhaustiva de ausencia de código muerto.
Se retiran sólo `tracing-subscriber` y `tracing-appender` de `workspace.dependencies`:
ningún crate las heredaba y no figuraban en el lock efectivo. El logging actual es propio
(`crates/ui-slint/src/logging.rs`). No reduce por sí mismo el EXE ni borra paquetes instalados.
Los generadores opcionales y fixtures permanecen: no confundirlos con funciones abandonadas.
Las dos versiones de lopdf siguen requeridas por APIs directas/pdf-extract; migrarlas queda
fuera de esta limpieza y no se eliminan a ciegas.

## Ajuste autorizado de distribución

Perfil release: `opt-level="s"`, `debug=true` y unwind/CRT estático conservados;
`lto="thin"`, `codegen-units=16`. El script de empaquetado fija `--locked --jobs 1`,
también fuera del supervisor. Mantener build solo y supervisor 12 GiB/margen 8 GiB.
No se aumenta el límite del intento anterior ni se desactiva la protección.
ThinLTO/unidades menores buscan reducir el trabajo monolítico; no prometer igual tamaño,
rendimiento ni un consumo concreto sin medir. Referencia:
[perfiles Cargo](https://doc.rust-lang.org/cargo/reference/profiles.html#lto).

## Verificación y entrega

- Suite completa: **1.324 aprobadas**, cero fallidas, seis ignoradas preexistentes.
  Clippy workspace/todos los targets sin advertencias. Supervisor: pico contable 5,86 GiB.
  Logs `target/agent-out/guarded-20260914-210630-995.{out,err}.log`.
- Formato, diff y metadata locked/offline correctos. Cargo.lock sin cambios en este bloque
  (SHA-256 `1712872d20ad057ed03c5b8b52ef82c9c7e4cde63382e3a50b4f6306e14e33be`;
  conserva cambios previos del workspace). No actualización de versiones de dependencias.
- Grafo AST: 8.097 nodos, 14.737 relaciones, 818 comunidades;
  `target/agent-out/build-thin-graphify.log`. Advertencia conocida de 29 catálogos sin nodos;
  sin etiquetado LLM/API.
- Build release completo: **44 min 40 s**, reconstruyendo dependencias para el perfil;
  no es medición comparable de tiempo incremental. Empaquetado Inno: 120,078 s.
  Supervisor 12 GiB/margen 8 GiB, pico contable de todo el Job **10,44 GiB**; no hubo corte.
  Logs `target/agent-out/guarded-20260914-211758-446.{out,err}.log`.
  El perfil anterior falló a 12/16 GiB; no se conoce su pico final y no se equipara este
  contador con RSS de Naygo ni se atribuye a memoria el reinicio previo.
- **Siete pruebas contra el EXE release aprobadas**: texto/resaltado/escenas STL/3MF,
  giro tras borrar la fixture, imágenes/SVG/PDF/ZIP, entradas inválidas, animaciones,
  pausa/economía/cancelación. Lectura de texto realmente bloqueada: cancelación en 21 ms
  y timeout en 4.999 ms. Log `target/agent-out/build-thin-release-readers.log`.
- Baseline: cinco pruebas compatibles contra el EXE RP-5B antes del build aprobadas,
  `target/agent-out/build-thin-baseline-readers.log`. Muestras pequeñas, una ejecución:
  imagen 45→39 ms; SVG 119→135 ms y 116→116 ms; PDF 47→49 ms. No es un benchmark
  estadístico de UI/listado ni una prueba de equivalencia global de rendimiento.
- ZIP íntegro según 7-Zip, exactamente cuatro entradas y SHA-256 de cada una igual al
  original (EXE, LICENSE, LEEME, THIRD-PARTY-NOTICES). Sin PDB en portable.
  SHA256SUMS comprobado contra los dos paquetes. Versión/autoría PE correctas.

**Entrega local 0.5.2+build.202609142135**, instalador terminado el 14 de septiembre
a las **22:04:44**, verificado a las 22:05. Sin firma Authenticode (sin certificado),
sin instalar sobre la sesión personal ni publicar a GitHub. RP-5B continúa respaldado.

| Artefacto | RP-5B bytes | Nuevo bytes | Nuevo SHA-256 |
| --- | ---: | ---: | --- |
| `dist/Naygo-0.5.2-setup.exe` | 52.703.896 | 64.209.471 | `b2c5d80bad554be68544f79de73c84a675651b425836767793b7ac0dc4b6c616` |
| `dist/Naygo-0.5.2-portable.zip` | 13.300.222 | 13.933.921 | `7a2d8d0f4a78d85b6b3c3cb8f777de17b745c96cd4f2f5fddaf303d4dd8d4b24` |
| `target/release/naygo.exe` | 36.567.040 | 42.060.800 | `c78828504e8f5908a0fb47c31f542d7c52d65a84f98469c3352079809d876f8e` |
| `target/release/naygo.pdb` | 342.831.104 | 438.808.576 | `8c43163734075d6766a71dfae6c746eac02d38fdb319df7e15cc64e16e6c7609` |

El portable aumenta 0,634 MB (4,8 %), el EXE 5,494 MB (15,0 %) y el instalador
11,506 MB (21,8 %, contiene símbolos opcionales). Esta comparación también incluye
RP-4B2: no atribuir cada byte al perfil ni afirmar ahorro de disco del producto.
El ahorro potencial de la caché **no se ejecutó**. Quedan aceptación instalada y mediciones
comparables de UI/listado/RSS; no se cierra RP-6 con estas comprobaciones.
