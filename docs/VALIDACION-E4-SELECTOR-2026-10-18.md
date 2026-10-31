# Validación E-4 — acceso rápido a espacios por tarea

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Fecha: 2026-10-18 (America/Santiago). Base: Naygo 0.5.2.

Este informe registra la primera versión, que conservaba recientes durante una sola sesión. El
catálogo persistente implementado después, con su gate y build Windows, está en
`docs/VALIDACION-E4-CATALOGO-2026-10-18.md`.

## Cambio

El menú de Disposiciones muestra hasta cinco espacios recientes de la sesión, con nombre y carpeta
del archivo `.naygospace`. La selección abre el documento en un worker. Si el espacio actual está
limpio, se aplica al terminar la lectura; si hay cambios, queda en revisión para guardar, descartar
o cancelar. No se inspeccionan carpetas ni se exploran proyectos al arrancar. El menú reutiliza el
grupo de Disposiciones y no añade un botón permanente a la toolbar.

## Validación

- Prueba específica: `quick_recent_applies_when_clean_and_requires_review_when_dirty` aprobada; cubre
  aplicación desde un estado limpio y conservación del estado actual cuando requiere revisión.
- `scripts/test-all.ps1`: 1.451 pruebas aprobadas, 8 ignoradas; Clippy con `-D warnings` y formato
  aprobados.
- `graphify update .`: 9.219 nodos, 17.490 aristas y 850 comunidades.
- `git diff --check`: aprobado; Git emite únicamente avisos informativos de normalización CRLF/LF
  en archivos previos del workspace.

## Distribución Windows

Build Release 0.5.2 supervisado, sin instalar:

- Pico contable: 10,53 GiB; compilación release: 30m01s; Inno Setup: 151,735 s.
- Portable: 14.385.870 bytes, SHA-256 `cfb7cdeb6f91a6e03846f6f6603575ec7b7d4c500693c99dd81c90e15e55da98`.
- Setup: 66.426.168 bytes, SHA-256 `eb6f2fdb345336b1514685897c37e1abe7614a6e0f1e7350940c9ee9e9b8c645`.
- `target/release/naygo.exe`: 43.517.952 bytes, SHA-256
  `fd33470f141fd1f272a2ba1201010f41118094405793448ac798d03f65947312`; coincide con el ejecutable
  leído del ZIP.
- 7-Zip verificó las cuatro entradas del portable. El instalador sigue sin firma Authenticode.

## Pendiente

El catálogo persistente entre sesiones, la aceptación visual del menú y del gestor, y la prueba de
rutas ausentes o remotas siguen abiertos. Esta entrega no se instaló y no sustituye la aceptación
manual del usuario.
