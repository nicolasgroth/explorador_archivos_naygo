# Recursos Kenney integrados en Naygo

Naygo incorpora dos sets compactos de 39 PNG cada uno, bajo los IDs `kenney-game` y
`kenney-board`. Fueron seleccionados desde las colecciones locales **Game Icons** y **Board Game
Icons** de Kenney, no se redistribuyen las colecciones completas.

- Autor original: Kenney (Kenney.nl)
- Licencia: [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/)
- Origen local verificado: `tmp/Icons/Game Icons/License.txt` y
  `tmp/Icons/Board Game Icons/License.txt`
- Uso: acciones, carpetas y categorías genéricas de la app; el nombre de cada PNG sigue la clave
  estable de Naygo (`action_*`, `file_*`, `folder`, `drive`, `unknown`).

Las máscaras blancas se tiñen en tiempo de ejecución con el color de texto del tema. Así los dos
estilos mantienen contraste en temas claros y oscuros sin duplicar variantes de color ni cargar
activos adicionales.
