# Naygo — validación RP-3: temas y configuración

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

Sobre RP-2 local, versión 0.5.1 y esquema de configuración 4. Sin nuevas dependencias,
sin instalación sobre la sesión personal, sin cambiar preferencias ni publicar en GitHub.
Se incorporan seis paletas originales al catálogo de quince temas. El tema predeterminado
y los nueve anteriores permanecen. Apariencia enlaza a la única galería de Íconos;
abrirla no aplica otro set. Se conservan IDs de categorías, sets y callbacks.

`icon_accent` separa énfasis de íconos/texto del fondo `highlight` de archivos recientes.
En JSON antiguos hereda `highlight`, sin cambio visual. El editor añade el índice 15 y
preserva 0–14. Las seis paletas nuevas usan su `accent` original como `icon_accent`;
no se recolorearon sus demás valores. Colores hexadecimales con Unicode inválido se
rechazan antes de indexar bytes, evitando un panic durante importación.

## Procedencia de las paletas

Fuente aislada: `target/agent-out/referencia-rp-20260909/source/crates/core/src/theme/builtin/`.
Paquete del artefacto 10109221926, run 34364248486, commit
`3489130d8ee7b7f34a11ebeb09b063b4377fdca0`. Recuperación documentada en
[RP-1](VALIDACION-2026-09-09.md); no se aplicó el árbol experimental completo.
SHA-256 de los JSON **originales**, antes de añadir `icon_accent`:

| Archivo | SHA-256 original |
| --- | --- |
| isgroth-arena.json | `5D93E655947417A1DBFC5482646A56C1B46F40779835512B72BCBAF6E25FE723` |
| isgroth-bruma.json | `A8502E735181106AC48E6D288E8E20AD8259D7EC86D2D3BA851B3084D8F024A7` |
| isgroth-grafito-claro.json | `C27318DB3C51A05F2244EF62F37002EFC688FE3319311E5E2EA72F3C44DB12E2` |
| isgroth-nieve.json | `5B8762317744CFB36E017886E3EEB33141E5B5C695420E0E2F62013C81F74BCC` |
| isgroth-perla.json | `B652AB69702214BCF2F8FB93D315B06561467D6B50D5952F89577775E41A8CFD` |
| isgroth-salvia.json | `2B9B69988F6D38400B6EE64168B7EB4C09B9C80492A247649B6CB3BB4E796D95` |

## Validación automática

Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)`. Las 31 pruebas focalizadas de temas
están aprobadas (`target/agent-out/rp3-core-themes.log`), igual que claves/placeholders
de diez idiomas. Suite completa: **1.263 aprobadas, cero fallidas y seis ignoradas
preexistentes** (`rp3-tests.log`): 870 core, 22 integración core, 47 platform,
317 UI y siete integración visual. Compilación inicial de pruebas: 11 min 10 s.
Suite repetida con fixture corregido: mismo resultado (`rp3-verified-tests.log`).
Clippy `--workspace --all-targets --locked -- -D warnings`: salida 0, sin advertencias
(`rp3-clippy.log`, 3 min 07 s). Formato y diff correctos. Empaquetado aprobado.

Grafo AST final: 7.718 nodos, 13.987 aristas y 794 comunidades (`rp3-graphify-final.log`).
El extractor informa 29 archivos sin nodos (incluye JSON); no equivale a cobertura completa.

Las pruebas numéricas de las seis paletas exigen contraste 4,5 para texto principal/secundario
y 3 para énfasis sobre superficies de panel, filas, selección y toolbar. No equivalen a una
certificación de accesibilidad. La prueba de configuración usa un directorio temporal para
guardar, recargar y restaurar ambos tokens sin interferencia.

La galería de prueba usa paneles, ruta y botones Slint reales con renderer software y datos
sintéticos; no HTML ni archivos personales. La comprobación estática del enlace a Íconos
no sustituye una prueba de toda Configuración en Windows.
La primera captura reveló datos sintéticos sin el nombre en `cells[0]`; se corrigió el
fixture para representar las columnas reales, sin modificar el listado de producción.
Se inspeccionaron las quince capturas finales a 800 × 400: nombres, fechas y tamaños,
panel activo/inactivo, selección, fila reciente/alternada/cortada, ruta, favoritos,
botones e indicador de error. Las seis nuevas paletas son legibles en ese escenario.
Los temas previos conservan limitaciones existentes (p. ej., énfasis claro en Windows XP
y texto secundario tenue en Solarized/Dracula); no se recolorearon silenciosamente ni se
afirma que los quince temas cumplan un estándar de contraste.

## Distribución

**0.5.1+build.202609091919**, generado el 9 de septiembre de 2026, hora America/Santiago.
`scripts/build-release.ps1` ejecutado solo, con un job: release 24 min 41 s e Inno Setup
82,859 s (`target/agent-out/rp3-release.log`). Instalador final a las 19:44:19;
portable a las 19:42:55. Artefactos locales, no publicados ni instalados sobre la sesión activa.

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| dist/Naygo-0.5.1-setup.exe | 52.078.892 | `dfd156cd4d56731ee35d126972c80e3515cf6eaa8fde5bd8969dfeb6cca7eeda` |
| dist/Naygo-0.5.1-portable.zip | 13.161.976 | `e3929aab08927f9c901f941611bbf2475da70c7d5a01fe8e51963f5a097a9f31` |
| target/release/naygo.exe | 36.186.624 | `17b8f4138141be2d8fcceb3b7c0b18fb0144aa27e5c1bd04d1c0457e2c908c77` |
| target/release/naygo.pdb | 339.226.624 | `d102249e3f81555b67359798e877839417f3d63673919502bb7cced818f3b4a2` |

`SHA256SUMS.txt` coincide con ambos paquetes. `7z t` aprobado (`rp3-portable-test.log`);
cuatro entradas exactas: LEEME.txt, LICENSE, naygo.exe y THIRD-PARTY-NOTICES.md.
Hashes del EXE, licencia y avisos dentro del ZIP coinciden con los archivos locales.
Metadatos del EXE: Naygo 0.5.1, Nicolás Groth/ISGroth, MIT. Build ID confirmado en la
salida de compilación. Instalador **NotSigned**: falta certificado, no es un error del build.
PDB generado e incluido como componente opcional del instalador, no en el portable.

El instalador/portable RP-2 reemplazados y sus checksums se respaldaron y verificaron en
`target/agent-out/baseline-051-before-rp3/`; se pueden recuperar desde esa carpeta.

## Límites y continuación

Pendientes aceptación instalada de Nicolás, DPI 100/125/150/200 %, teclado/Narrador,
tooltips/campos/scrollbars y combinaciones de preferencias en escritorio real. No se afirma
haber certificado toda la matriz T01–T02/D01–D02. RP-4–RP-6 siguen pendientes: ciclo de vida
y cancelación del preview, animaciones, lectores e instrumentación. No se añadieron controles
de animación ni se alteró la política de recursos del preview en este bloque.
