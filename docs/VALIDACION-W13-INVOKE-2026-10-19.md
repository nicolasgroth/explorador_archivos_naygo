# W-13 — acción predeterminada accesible en la bandeja temporal

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Hallazgo

La inspección UI Automation pasiva del proceso instalado PID 42728 encontró que todos los controles
enfocables tenían nombre, pero 19 controles de tipo Button enfocables no anunciaban el patrón
`Invoke`. La instancia es anterior a la distribución W-06E. La revisión del componente compartido
`BasketIconButton` confirmó que declaraba rol, nombre y soporte de teclado, pero omitía
`accessible-action-default`; por eso un lector podía reconocerlo y enfocarlo sin poder activarlo
mediante el patrón de acción UIA.

## Cambio

`BasketIconButton` ahora publica una acción accesible que conserva el guard de `enabled`, enfoca el
control y llama al mismo callback que el clic. La inspección posterior de esa distribución encontró
74 patrones `Invoke` (antes 63) y redujo de 19 a 10 los botones enfocados sin `Invoke`.

La revisión estática del árbol de componentes también encontró que `OpButton` y `MissingOpButton`
dibujaban controles clickeables sin nombre/acción accesibles ni foco por teclado. Ambos declaran
rol, etiqueta y activación por acción accesible; Enter/Espacio ejecuta el mismo callback visual. Los
botones de operaciones sin texto toman una etiqueta localizada según el icono (por ejemplo,
Cancelar). Esta segunda corrección está incluida en la versión 0.5.3 que debe validarse a
continuación.

En una tercera revisión estática, los botones dibujados a mano para maximizar/cerrar panel y los
selectores de categoría de ajustes tampoco declaraban `Invoke`. Se añadió rol, nombre, acción y
foco de teclado donde faltaban; los callbacks originales permanecen como destino de la acción.

La distribución 0.5.3 se reconstruyó, validó e instaló localmente con autorización previa. El proceso
instalado responde (PID 44872) y su ejecutable coincide con `target/release/naygo.exe`. Una inspección
UIA pasiva de la ventana principal encontró 356 descendientes, 120/120 controles enfocables con
nombre, 69 patrones `Invoke`, cero botones sin `Invoke` y árbol sin truncamiento. Frente a los diez
botones sin patrón de la build anterior, los controles de botón de la ventana principal ahora
publican acción. No se invocó ningún patrón ni se abrieron diálogos.

El inspector ahora agrupa botones sin `Invoke` por clase, AutomationId, foco y presencia de nombre,
sin guardar el texto del nombre. En la ventana instalada, los diez botones restantes tenían nombre,
pero el proveedor no exponía clase ni AutomationId; esa lectura no permite atribuirlos con seguridad
a un componente. La revisión estática corrigió los demás controles personalizados identificados.

## Validación local

- `cargo test -p naygo-ui-slint --test layout_render --locked --offline`: 2/2 aprobadas.
- `scripts/test-all.ps1`: 1.480 pruebas aprobadas, 8 ignoradas; Clippy sin avisos y formato correcto,
  repetido después de añadir las acciones accesibles a operaciones, carpeta perdida, maximizar/cerrar
  panel y categorías de configuración.
- `graphify update .`: 9.527 nodos, 18.052 relaciones, 899 comunidades.
- `scripts/inspect-naygo-uia.ps1` ampliado para contar patrones de acción sin invocarlos ni leer
  nombres, valores, rutas o contenido. Dos lecturas pasivas del proceso anterior encontraron 19
  botones enfocables sin `Invoke`; se registran como línea base, no como verificación del cambio.
- `scripts/build-release-guarded.ps1`: Release en 34m39s; Inno Setup en 131,235s; pico Job de
  10,65 GiB (límite 12 GiB). Gate SHA-256 del portable, setup y ejecutable Release verificado; el
  EXE incluido coincide con Release, el ZIP tiene cuatro entradas legibles y no incluye PDB.
- Hashes SHA-256: portable `205c969d495f9b039784447e5715970798487f7287a068e7155e70481e53054d`,
  setup `ba892d36eb1f9687605acbd6daba5fc0b957119dce7055d24e7aadb6d117b9ce`, EXE
  `595bbc585a9035d6842cc0638ba318465917507c6a338ccb5fde6d3fb3111937`.
- EXE y setup siguen sin firma Authenticode; no se publicó ningún artefacto.

## Pendiente

La lectura UIA confirma la ventana principal del Release instalado, pero no sustituye la aceptación
de Narrador ni cubre diálogos que no se abrieron. Falta recorrer controles y diálogos (incluido
Ajustes) con Narrador en una sesión desbloqueada. No se considera cerrado W-13 hasta completar esas
comprobaciones manuales.

### Reintento pasivo sobre PID 50260 — 2026-10-19 23:53

El proceso respondía y conservaba un handle de ventana, pero `scripts/inspect-naygo-uia.ps1` no
encontró descendientes. Una lectura directa identificó la raíz como `Pane` (`Winit Thread Event
Target`) sin primer control hijo; había un proceso `LogonUI` en la misma sesión. Esto no confirma por
sí solo la causa ni permite concluir que haya una regresión de accesibilidad. No se enviaron acciones.
Repetir la inspección con la ventana visible en una sesión desbloqueada antes de evaluar el árbol; la
aceptación de Narrador y diálogos sigue pendiente.

### Reintento pasivo — 2026-10-20 00:40 (hora de Chile)

El proceso 50260 sigue respondiendo y conserva un handle de ventana, pero su título es vacío y la
inspección UIA vuelve a fallar al encontrar descendientes. En la misma sesión aparecen procesos
`LockApp` y `LogonUI`; por ello la sesión sigue bloqueada y este resultado no sirve para aceptar la
UIA del release. No se enviaron teclas, clics ni acciones. Repetir sólo cuando la sesión esté
desbloqueada; Narrador y diálogos continúan pendientes.
