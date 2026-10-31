# Naygo — validación de ruta superior RP-2

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance y conservación

Continuación de RP-1 sobre 0.5.1, rama local `codex/rutas-preview-051`.
Editor de ruta superior expandido en la franja de herramientas, con las mismas reglas
de edición nativa, sugerencias y validación asíncrona. ID de panel capturado al iniciar;
copiar/favorito usan la carpeta confirmada. La cancelación devuelve el foco al listado
activo. Un cambio de panel no confirma ni redirige el borrador; cerrar o navegar el
destino invalida la sesión. No cambia el esquema 4 ni añade dependencias o atajos.

Respaldo anterior: `target/agent-out/baseline-051-before-rp2/`, con setup/portable y
SHA256SUMS de **0.5.1+build.202609091710**. La evidencia de RP-1 permanece en
[su registro](VALIDACION-2026-09-09.md); sus hashes describen ese respaldo histórico.
No se instala sobre la sesión de Nicolás ni se leen o modifican sus preferencias.

## Validación automática

Suite final aprobada: **1.257 pruebas, cero fallos y seis ignoradas preexistentes**.
Desglose: 867 core, 22 integración core, 47 platform, 316 UI y cinco integración visual.
Registro: `target/agent-out/rp2-verified-tests.log`. Diez idiomas aprobados en
`rp2-i18n.log`; render sintético a 260 px revisado visualmente: campo, Esc, copiar y
favorito visibles, error bajo la barra.

Las primeras pasadas detectaron dos errores del fixture nuevo, no de la aplicación:
importación del tipo de píxel y falta de render previo al timer de foco de un hijo
condicional de Slint. Corregidos sin retirar aserciones: la prueba final usa el ciclo
render → timer que realiza la ventana nativa. El cableado real de destino/cancelación
pasó desde su primera ejecución.

Clippy de todo el workspace y todos los targets: **salida 0, sin advertencias**,
registro `rp2-clippy.log` (3 min 55 s). Formato y diff aprobados. Grafo AST actualizado:
7.691 nodos, 13.947 relaciones y 787 comunidades (`rp2-graphify-final.log`); sus límites
de extracción de JSON/documentos no se interpretan como cobertura de esos archivos.
La distribución se completa al terminar el empaquetado y comprobar su integridad.

Pruebas específicas nuevas:

- `callbacks_pathbar_tests.rs`: callbacks reales con AppWindow y directorios/configuración
  temporales. Cambio de panel antes de confirmar, ruta relativa Unicode, favorito sobre
  carpeta confirmada, error que conserva borrador, cancelación pendiente, cierre de destino
  y cambio entre editor de barra/panel.
- `tests/toolbar_path.rs`: componente Slint real con renderer software, clics a 260 px,
  acciones accesibles con ruta larga, edición, errores, sugerencias por teclado/ratón,
  Enter y Escape. Render sintético `target/agent-out/rp2-toolbar-260.png`.

Los tests no operan el portapapeles real. La prueba de favorito usa configuración temporal.
No equivalen a una revisión completa de ventanas nativas, Narrador o distribuciones físicas
del teclado.

## Distribución

**0.5.1+build.202609091819**, generado por `scripts/build-release.ps1` solo, con un
trabajo de compilación. Salida 0: release en 27 min 11 s; Inno Setup en 92,937 s.
Instalador terminado el 9 de septiembre a las 18:48 (America/Santiago).

| Artefacto | Bytes | SHA-256 |
| --- | ---: | --- |
| `dist/Naygo-0.5.1-setup.exe` | 52.145.519 | `15a99cf97a86a368a8d0e35c62d769b04ccbb412f37ef903570b77b4ffb19782` |
| `dist/Naygo-0.5.1-portable.zip` | 13.167.792 | `f11197f6ecfa2c8c896d706dfbbe8b57c5dfdc239081fa0c477c0fa84350117a` |
| `target/release/naygo.exe` | 36.221.952 | `b7698e4db6351ac0fa64432cac037207d6fae2383cd7cefc91004b566b0a9d14` |
| `target/release/naygo.pdb` | 339.701.760 | `05eabbd77c37ab0594686607864af0e5bc7bb86af5946e41a656036401e8e420` |

`SHA256SUMS.txt` coincide con ambos paquetes. `7z t` aprueba el portable; sus cuatro
entradas son LEEME, LICENSE, EXE y THIRD-PARTY-NOTICES. Los hashes de EXE, LICENSE y
notices dentro del ZIP coinciden con las fuentes locales. El PDB está incluido en
el instalador, no en el ZIP, conforme al formato de distribución vigente.
Metadatos del EXE comprobados: Naygo 0.5.1, Nicolás Groth/ISGroth y licencia MIT.
El EXE aumenta 296.960 bytes respecto de RP-1; sin dependencias nuevas. Esto no es
una medición de memoria o tiempo de navegación.

Registros: `rp2-release.log`, `rp2-artifacts.json` y `rp2-portable-test.log`, dentro de
`target/agent-out/`. Instalador **sin firma** (no hay certificado configurado).
No se instaló ni se ejecutó sobre la sesión del usuario. Los paquetes anteriores de
`dist` fueron reemplazados, pero se pueden recuperar desde el respaldo indicado arriba.
No se hicieron commits ni publicación en GitHub en este bloque.

## Aceptación instalada pendiente

1. Comprobar build `202609091819` en Acerca de. Abrir la edición superior con clic en
   la ruta o el lápiz: debe desplegarse una sola
   franja editable. Editar en medio, pegar, seleccionar y deshacer sin perder el cursor.
2. Mantener un borrador en el panel A, seleccionar B y volver al campo: confirmar debe
   navegar A. Copiar/favorito durante la edición deben usar la carpeta confirmada de A.
3. Probar carpeta inexistente y Esc; comprobar que se conserva la carpeta anterior y
   vuelve el teclado al listado activo. Probar cierre/navegación del destino pendiente.
4. Verificar que las sugerencias se eligen con flechas/Enter o clic, sin confirmar todavía
   la navegación. Comprobar rutas relativas, comillas, UNC y AltGr chileno.
5. Probar DPI 100/125/150/200 %, paneles dockables, ventanas estrechas, tooltips y Narrador.
   La matriz de red lenta/no disponible sigue pendiente; el timeout es lógico, no mata
   llamadas de Windows bloqueadas.

RP-3 (apariencia/configuración) y RP-4–RP-6 (preview/animación/lectores) no forman parte
de esta entrega y continúan pendientes en el plan aprobado.
