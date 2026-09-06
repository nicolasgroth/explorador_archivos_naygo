# Naygo 0.5.1 — ajustes de rutas, bandeja y licencia

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

- Versión de workspace/crates/metadata/notas: 0.5.1.
- Breadcrumbs por ancho natural, sin celdas de igual tamaño. El extremo de la carpeta actual
  permanece visible cuando falta espacio y el menú … mantiene todos los ancestros.
- Ocho acciones de bandeja arriba, en una fila si caben o más filas según el ancho disponible.
  Se conservan callbacks, tooltips, nombres accesibles y teclado. Vaciar queda separado abajo.
- Acerca de contiene un editor de sólo lectura con MIT completa, selección/copia y scroll.
  El texto se embebe desde LICENSE durante la compilación, sin I/O en UI ni traducciones que
  alteren los términos. LICENSE incorpora el correo del autor; el cuerpo MIT no se modifica.
- No se añadieron dependencias, indexación, servicios residentes ni acciones externas.

## Validación automática y visual

- `cargo test --workspace --locked`: 1.230 aprobados (859 core + 22 integración core +
  47 platform + 300 UI + 2 integración visual/documental); seis smoke interactivos ignorados.
  Registro: `target/agent-out/051-validated-tests.log`.
- Clippy `--workspace --all-targets --locked -- -D warnings` aprobado; `051-final-clippy.log`.
- Formato, diff y paridad de los diez catálogos aprobados. No se añadieron claves traducibles.
- Render real de los componentes Slint con MinimalSoftwareWindow, sin ventanas nativas ni
  cambiar la sesión del usuario: 900/360/200/110 px. Verifica tamaño real, presencia de ruta,
  ausencia de texto espaciado hasta el extremo de una barra ancha y clics de las ocho acciones
  en cada fila. Imágenes inspeccionadas: `target/agent-out/layout-051-*.png`.
  El fixture no carga el pack de íconos del usuario: las acciones con imagen muestran su marco.
- Graphify AST actualizado: 7.442 nodos / 13.526 aristas; `051-graphify.log`. Persisten los avisos
  conocidos de archivos sin nodos y etiquetas históricas; no se pidió extracción semántica/LLM.

La prueba headless no equivale a validar la aplicación instalada con Narrador, otros DPI o
temas personalizados. Probar instalado: ruta corta/larga al redimensionar, navegación y menú de
ancestros, bandeja ancha/estrecha, tooltips/teclado y selección/copia/scroll de MIT en Acerca de.

## Distribución inicial — 2026-09-05, 20:36

Regenerada con `scripts/build-release.ps1`, solo y sin otras tareas pesadas, el 2026-09-05.
Compilación Rust: 26 min 56 s; compilación del instalador: 98,172 s. Registro completo:
`target/agent-out/051-release.log`. Horas de artefactos en America/Santiago:

| Artefacto | Bytes | Hora |
| --- | ---: | --- |
| `dist/Naygo-0.5.1-setup.exe` | 50.907.343 | 20:36:18 |
| `dist/Naygo-0.5.1-portable.zip` | 12.456.618 | 20:34:38 |

SHA-256 recalculados y coincidentes con `dist/SHA256SUMS.txt`:

```text
17c464c7d541dbb3a76f4c07628053921c382e7bd6de74ad0cdc3504ed3ec165  Naygo-0.5.1-setup.exe
a567779e400717f4bc96b574f67c85cc8d9d81958b261a6016f2b3aeb8d404bf  Naygo-0.5.1-portable.zip
```

- 7-Zip verificó las cuatro entradas del portable sin errores.
- `target/release/naygo.exe`: 35.290.624 bytes, versión de producto 0.5.1, autoría
  Nicolás Groth / ISGroth y correo presentes. SHA-256:
  `49773d49b0d85b6d267d68af0fb47b14c057a9a238aaf9526f1b886fd9793648`.
  El EXE del ZIP coincide byte por byte mediante SHA-256.
- LICENSE del ZIP coincide con la fuente del repositorio; SHA-256:
  `6e0ce65e3ea9e55440f0e9c54a4bd9156a839e4786d86ec246cabab7147d9e43`.
  El registro de Inno Setup también confirma que incluye LICENSE.
- PDB generado: 329.658.368 bytes. Se conserva la política T-3: símbolos opcionales en el
  instalador, excluidos del ZIP portable.
- Instalador sin firma de código: certificado no configurado; no es un error de compilación.
- Build del working tree con cambios previos sin commit, base `6ea74b9f39da`; no se hizo commit
  ni push. No se cerró la instancia del usuario ni se instaló automáticamente.
- Pendiente validación instalada de Nicolás, incluida la interacción manual indicada arriba.

## Revisión posterior al respaldo en GitHub

Nicolás aprobó continuar el 2026-09-05 después de publicar los cambios. El respaldo inicial es
`dcca05b1799ec5781ed0a00eab5a3202e0b34008`, verificado en origin/fix/single-instance-y-bandeja.
Los binarios se regeneran con el mismo número 0.5.1 y un identificador de build nuevo.

- Bandeja: desactivar «Solo íconos» muestra etiquetas y un desplegable de acciones secundarias.
  Mantiene el modo compacto anterior por defecto; se reutiliza la preferencia persistida.
- Espacios: diez recientes de la sesión, deduplicados, con limpiar y revisión antes de aplicar.
  No se persiste este MRU ni se exploran las rutas al renderizar.
- Operaciones: filtro de fallidos tanto en historial como en detalle, señal de error y acceso al
  detalle aunque haya cero éxitos. Revisión/reintento de archivos fallidos de copiar/mover con
  origen conocido, destinos exactos, comprobación de cambios y consulta de conflictos. No undo,
  explícito en el diálogo. Limitaciones detalladas en [REINTENTOS.md](REINTENTOS.md).
- Tests nuevos: selección de candidatos, cambios de origen/destino, cancelación sin ejecutar,
  copia/movimiento confirmado conservando otros archivos y recientes sin aplicación implícita.
  El render por software cubre también bandeja con etiquetas a 900/200 px.
- Paridad de los diez idiomas validada; no se añadieron dependencias.

Validación final de esta revisión: **1.238 pruebas aprobadas** (862 core + 22 integración core +
47 platform + 305 UI + 2 integración visual), seis smoke interactivos ignorados. Logs:
`target/agent-out/post-github-verified-tests.log` y `post-github-clippy-final.log` (Clippy sin
advertencias, 1 min 27 s). Formato/diff y diez catálogos correctos. Capturas de etiquetas a
900/200 px y desplegable a 200 px inspeccionados; clic en acción secundaria verificado. El fixture
no carga todos los íconos ni todo el catálogo de producción. Grafo final: 7.498 nodos / 13.618
aristas / 775 comunidades, `post-github-graphify-final.log`.

Distribución de la revisión regenerada con `scripts/build-release.ps1` solo: Rust 27 min 27 s;
Inno Setup 87 s, ambos correctos. Registro: `target/agent-out/post-github-release.log`.

| Artefacto actual | Bytes | Hora del 2026-09-05 (America/Santiago) |
| --- | ---: | --- |
| `dist/Naygo-0.5.1-setup.exe` | 51.410.586 | 22:31:11 |
| `dist/Naygo-0.5.1-portable.zip` | 12.558.380 | 22:29:44 |

SHA-256 recalculados y coincidentes con `dist/SHA256SUMS.txt`:

```text
16ae35d59ca31da542521b759b42948e0ae69369d4cb4c86ef58fd3ea5a0a988  Naygo-0.5.1-setup.exe
2451194893dab550bed959c6dc386b9ba4a584470c53043fe0bea7ba3a18e920  Naygo-0.5.1-portable.zip
```

7-Zip verificó las cuatro entradas. EXE del ZIP idéntico al release: 35.668.992 bytes,
versión 0.5.1 y autoría ISGroth / Nicolás Groth; SHA-256
`c31140fd3ea8e1f5d907ce992d1d10ebb4e0763bbe9e8bd989236ac3074fd0f3`.
LICENSE vuelve a coincidir con la fuente canónica (hash indicado en la distribución inicial).
PDB: 334.016.512 bytes, opcional en el instalador y excluido del ZIP. Firma de código sigue
pendiente del certificado externo; no se instaló ni se cerró la instancia del usuario.

Esta revisión se prepara para publicar en `fix/single-instance-y-bandeja`, sobre `dcca05b1`.
Pendiente prueba instalada de Nicolás: alternar etiquetas, desplegar acciones con teclado,
abrir/limpiar recientes, filtrar fallidos y revisar/cancelar/reintentar una copia de prueba.

## Recomendaciones aprobadas

1. Modo opcional «ícono + etiqueta» y menú de acciones secundarias en barras compactas.
2. Espacios recientes accesibles sin catálogo/indexación global ni ejecución automática.
3. Filtro de fallos y reintento de fallidos con nueva revisión de fuentes, destinos y conflictos.

Referencia de UI: [barras de comandos de Microsoft](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/command-bar).
Referencia de términos: [MIT en Open Source Initiative](https://opensource.org/license/mit).
