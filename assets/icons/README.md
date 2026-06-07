# Íconos de Naygo

Los PNG aquí son un **set inicial propio** generado por
`crates/ui/src/bin/gen_icons.rs` (formas de color simples por categoría). Son
placeholders de licencia limpia (propios) para validar la infraestructura de
íconos. Los tres sets (flat/fluent/mono) comparten forma por ahora; el "mono" va en
gris para teñirse con el tema.

## Reemplazar por packs profesionales (a futuro)

Colocar un PNG por cada nombre de archivo de abajo en cada carpeta de set. Tamaño
recomendado 32×32 (o 16/32/48). Packs libres recomendados (verificar licencia):

- **Flat Color Icons** (icons8) — MIT — github.com/icons8/flat-color-icons
- **Fluent Emoji** (Microsoft) — MIT — github.com/microsoft/fluentui-emoji
- **Lucide** (ISC) / **Tabler** (MIT) — para el set mono — github.com/lucide-icons/lucide
- **VS Code Icons** — MIT — github.com/vscode-icons/vscode-icons (excelentes por tipo de archivo)

## Nombres de archivo esperados (uno por IconKey)

folder, file_image, file_video, file_audio, file_document, file_code,
file_archive, file_executable, file_model3d, file_font, file_generic, drive, unknown
