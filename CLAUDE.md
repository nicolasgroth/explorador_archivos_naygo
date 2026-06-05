# Explorador de archivos — Contexto del proyecto

> Proyecto **independiente**. No tiene relación con WinShelf ni con
> `organiza_escritorio_ng`. No mezclar contexto, convenciones ni código entre
> ambos.

## Qué es

Explorador de archivos para Windows 10/11, estilo **Commander** (inspirado en
Directory Opus): paneles dinámicos dockables, navegación ultra-rápida por teclado,
operaciones de archivo entre paneles. Gratuito, open source, propio.

**Prioridad absoluta:** velocidad de navegación y bajo consumo. Lo visual importa,
pero está **subordinado a la velocidad**.

**Hace bien una cosa:** navegar, ver y operar archivos rápido. NO reproduce media,
NO es editor, NO hace de todo. Abre el archivo con su programa por defecto cuando
el usuario lo pide.

## Autoría y licencia

- Autor: **Nicolás Groth** (Chile)
- Empresa: **ISGroth**
- Año: 2026
- Licencia: **MIT**

Marcar la autoría visiblemente en: metadatos del `.exe`, ventana "About", headers
de archivos clave, `README.md` y `LICENSE`. El objetivo del proyecto es que la
gente le saque provecho dando a conocer el nombre de Nicolás Groth y de ISGroth.

## Stack

- Lenguaje: **Rust**
- UI: **egui / eframe** (UI inmediata, render GPU)
- Docking: **egui_dock** (paneles dinámicos)
- Interop Windows: crate **`windows`** (oficial de Microsoft) para Shell32 / COM /
  OLE
- Serialización: **serde / serde_json**
- Todas las dependencias **libres** (MIT/Apache/ISC/CC0). Cero regalías.
- Build: `cargo` desde terminal.

## Arquitectura (3 capas — ver spec)

- **`core`**: lógica pura, sin UI ni Windows. Testeable al 100%. Contiene
  `fs_model`, `listing` (streaming incremental), `ops`, `sizing`, `i18n`, `theme`,
  `config`.
- **`platform`**: TODO lo que toca Windows, aislado. `shell` (íconos, ShellExecute,
  papelera, discos), `dnd` (drag&drop COM/OLE), `watcher` (futuro).
- **`ui`**: egui, sin lógica de negocio. `app`, `docking`, paneles, `theme_apply`,
  `input`, `icons`, `progress`.

**Regla de oro:** el hilo de UI **nunca** hace I/O de disco. Todo lo pesado corre
en workers async que se comunican por canales. `core` no conoce egui ni Windows.

## Principios de diseño (críticos)

- **Streaming incremental**: listar una carpeta nunca congela; los resultados
  aparecen en vivo.
- **Cancelación universal**: TODA operación larga (listar, copiar, mover, calcular
  tamaño) es cancelable por el usuario, siempre. Cada una recibe un
  `CancellationToken` y aborta limpio (una copia cancelada borra el parcial).
- **El filesystem es hostil**: discos de red caídos, permisos denegados, rutas que
  desaparecen son normales. La app NUNCA cae por eso: `Result` tipado, errores
  comunicados de forma discreta, timeouts en I/O de red, panic handler.
- **i18n y temas desde el día uno**: ningún texto hardcoded (todo por clave). Temas
  + color sets intercambiables en caliente. ES + EN incluidos; agregar idioma =
  soltar un archivo.

## Convenciones de código

- Nombres en inglés en el código. Comentarios y commits pueden ser en español.
- Cada archivo lleva header:
  ```
  // Explorador de archivos — <descripción breve del archivo>
  // Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
  ```
- Privilegiar legibilidad sobre brevedad. Un tercero debe poder leer y mantener.
- Modular: una responsabilidad por módulo. Si un archivo crece demasiado, split.
- Async para todo I/O. El hilo de UI no bloquea.
- Logging básico a archivo desde el inicio. Sin telemetría.
- Build limpio + tests pasando antes de cada commit.

## Estrategia de construcción — faseado

El **Build 1 (núcleo)** está especificado en
`docs/superpowers/specs/2026-06-05-explorador-nucleo-design.md`. Empezar por ahí.

**Capas posteriores** (cada una con su propio brainstorm → spec → build, NO en
Build 1): miniaturas, visor de contenido (imágenes/texto/PDF), comprimir/
descomprimir, batch-rename avanzado, caché de carpetas visitadas, paleta de
comandos (Ctrl+P), animaciones de íconos, personalización fina de toolbar.

**Nunca**: reproducción de media, edición de archivos.

## Cómo trabajar conmigo (el usuario)

- Soy Nicolás. Hablo español chileno, tuteo. Inglés técnico OK.
- Ingeniero, base técnica fuerte. No me expliques lo obvio, pero si algo es
  ambiguo, pregúntame antes de avanzar con supuestos.
- Si una decisión técnica tiene trade-offs reales, explícamelos brevemente antes
  de elegir.
- Si no sabes algo o necesitas verificar, dilo. No inventes.
- Trabajamos feature por feature. No saltes adelante sin que yo confirme.
- Al terminar cada feature, sugiere el siguiente paso y espera mi visto bueno.
