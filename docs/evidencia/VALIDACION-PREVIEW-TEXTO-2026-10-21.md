# Validación del límite configurable del preview de texto — 2026-10-21

## Cambio

Configuración → Previsualización permite escoger 64 KiB, 256 KiB, 512 KiB o 1 MiB de lectura
por archivo; 256 KiB es el valor inicial. La opción se guarda en settings v6 y las versiones
anteriores adoptan el valor por defecto sin perder sus preferencias. Al cambiarla, el preview de
texto activo vuelve a cargarse inmediatamente. La UI indica los bytes leídos y caracteres/líneas
presentados.

El lector permanece en un proceso aislado con los límites de memoria y tiempo existentes. Se
conservan los topes de 250 líneas y 1.000 caracteres por línea. Por eso un XML/SIF minificado en una
sola línea puede verse truncado aunque el usuario aumente los bytes; el formateo XML no forma parte
de este cambio. El archivo original nunca se modifica.

## Verificación

- `scripts/test-all.ps1`: **1.507 pruebas aprobadas, 8 ignoradas**; Clippy (`-D warnings`) y formato
  pasaron. La prueba dirigida que cambia el límite y recarga solo el preview de texto también pasó.
- `graphify update .`: grafo actualizado a 9.736 nodos y 18.485 relaciones. Graphify avisó que 29
  fuentes —principalmente catálogos JSON— no produjeron nodos AST; el grafo sí se reconstruyó.
- Build Windows 0.5.3 supervisado, con `scripts/build-release-guarded.ps1`: Release **46m41s**;
  Inno Setup **159,765 s**. El mayor pico observado durante el seguimiento fue **10,69 GiB** bajo
  el límite de 12 GiB; la memoria libre del equipo se mantuvo holgadamente por sobre 8 GiB. El
  supervisor y los logs del hijo reportaron compilación y empaquetado exitosos.
- El ZIP contiene `naygo.exe`, `LICENSE`, `LEEME.txt` y `THIRD-PARTY-NOTICES.md`; no incluye PDB.
  El hash del EXE dentro del ZIP coincide con `target/release/naygo.exe`.
- El setup y el ejecutable no tienen firma Authenticode (`NotSigned`). No se publicó ningún artefacto.

## Artefactos

- Portable: `dist/Naygo-0.5.3-portable.zip` — SHA-256
  `695cd80655845eded637dac4070174bacc65c59cea1fb4669e64c5b1d7ea56c4`.
- Instalador: `dist/Naygo-0.5.3-setup.exe` — SHA-256
  `d6098a4b990a812b1fadd3adac8d4f0b1079575bc589da2932cdb5c75aa8340c`.
- Ejecutable Release: `target/release/naygo.exe` — SHA-256
  `db69b70d8e30b793eaa497e92f8e72510cb42400353e4c03c8331ed45f7d5eea`.
- Los tres SHA-256 aparecen también en `dist/SHA256SUMS.txt` para los paquetes distribuidos.

El nuevo instalador quedó preparado para la validación manual de Nicolás; no se ejecutó ni se
instaló desde esta tarea. El proceso Naygo que ya estaba abierto responde, pero no fue reiniciado
con este build. La validación visual de la opción en equipos con 8 y 16 GiB queda pendiente.
