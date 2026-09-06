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

## Distribución

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

## Recomendaciones — requieren confirmación

1. Modo opcional «ícono + etiqueta» y menú de acciones secundarias en barras compactas.
2. Espacios recientes accesibles sin catálogo/indexación global ni ejecución automática.
3. Filtro de fallos y reintento de fallidos con nueva revisión de fuentes, destinos y conflictos.

Referencia de UI: [barras de comandos de Microsoft](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/command-bar).
Referencia de términos: [MIT en Open Source Initiative](https://opensource.org/license/mit).
