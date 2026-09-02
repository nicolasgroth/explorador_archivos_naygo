# Recursos Kenney integrados en Naygo

Naygo incorpora cuatro sets compactos de 39 PNG cada uno. Los IDs estables
`kenney-game` y `kenney-board` corresponden a los estilos visibles **Outline** y **Tiles**.
Los IDs `vivid` y `pastel` son variantes multicolor derivadas de las mismas máscaras; se muestran
como **Vivid** y **Pastel**. Fueron seleccionados desde las colecciones locales **Game Icons** y
**Board Game Icons** de Kenney; no se redistribuyen las colecciones completas.

- Autor original: Kenney (Kenney.nl)
- Licencia: [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/)
- Origen local verificado: `tmp/Icons/Game Icons/License.txt` y
  `tmp/Icons/Board Game Icons/License.txt`
- Uso: acciones, carpetas y categorías genéricas de la app; el nombre de cada PNG sigue la clave
  estable de Naygo (`action_*`, `file_*`, `folder`, `drive`, `unknown`).

Las máscaras blancas se tiñen en tiempo de ejecución con el color de texto del tema en los estilos
Outline y Tiles. Vivid y Pastel conservan una paleta semántica por tipo de elemento (por ejemplo,
carpetas, documentos y acciones) para ampliar la oferta de sets con color sin perder legibilidad.
