# Agregar un idioma a Naygo

Naygo incluye **diez idiomas** en el ejecutable (es, en, pt, fr, de, it, zh, ja, ko, hi).
Agregar cualquier otro idioma **no requiere recompilar ni tocar código**: basta con dejar un
archivo de traducción junto al programa.

## Cómo funciona

- Los diez idiomas de fábrica están embebidos en `naygo.exe` (siempre disponibles).
- Al arrancar, Naygo busca además archivos `lang/<código>.json` en su carpeta de
  configuración (modo portable: junto al `.exe`). Cada uno se carga como un idioma más y
  aparece automáticamente en el selector de **Configuración → Idioma**.
- El **nombre del archivo es el código del idioma**: `pt.json` → portugués, `fr.json` →
  francés, `de.json` → alemán, etc. (códigos ISO 639-1 de dos letras).
- Si una traducción tiene claves incompletas, Naygo usa el español como respaldo para las que
  falten (nunca queda un texto en blanco).

## Pasos

1. Ubica la carpeta de configuración de Naygo:
   - **Portable:** la misma carpeta donde está `naygo.exe`.
   - **Instalado:** `%APPDATA%\Naygo` (pega esa ruta en el explorador).
   Dentro, crea una subcarpeta llamada **`lang`** si no existe.

2. Toma como base el catálogo en español del repositorio:
   [`crates/core/src/i18n/es.json`](../crates/core/src/i18n/es.json). Cópialo a
   `lang/<código>.json` (por ejemplo `lang/pt.json`).

3. Traduce **solo los valores** (el texto a la derecha de cada `:`), dejando las **claves**
   (lo de la izquierda) **exactamente igual**. Por ejemplo, para portugués:
   ```json
   "slint.toolbar.up": "Subir",          ← clave intacta, valor traducido
   "slint.history.label": "Histórico de pastas",
   ```
   - No cambies las claves ni la estructura del JSON.
   - Conserva los marcadores como `{drive}` o `{path}` tal cual (Naygo los reemplaza en
     tiempo de ejecución).
   - Guarda el archivo en **UTF-8** (sin BOM idealmente; si tu editor mete BOM, Naygo lo
     tolera).

4. Abre Naygo. En **Configuración → Idioma** aparecerá el nuevo idioma. Selecciónalo: el
   cambio se aplica al instante.

## Notas

- No hace falta traducir el 100% de las claves para probar: lo que falte se mostrará en
  español. Puedes ir completando el archivo y recargando.
- Para compartir un idioma con otros usuarios, basta con pasarles el `lang/<código>.json`,
  o exportarlo como pack **`.naygolang`** desde *Configuración → Importar/Exportar*.
- Si quieres que un idioma venga **incluido de fábrica** en el ejecutable (como los diez
  de fábrica), eso sí requiere agregarlo al código (`crates/core/src/i18n/mod.rs`) y
  recompilar; pero para uso normal, el archivo suelto en `lang/` es suficiente.
