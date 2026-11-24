# Revisión estructural de UI Automation — 2026-10-18

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Evidencia

Se ejecutó `scripts/inspect-naygo-uia.ps1 -ProcessId 25124` contra la instancia que ya estaba
abierta. El script recorre Control View y devuelve conteos; no lee ni registra nombres, rutas o
valores de controles. Se amplió para informar los tipos de controles enfocables sin nombre
accesible, y se volvió a ejecutar con éxito.

| Medida | Resultado |
| --- | ---: |
| Descendientes UIA | 732 |
| Elementos con nombre | 543 |
| Enfocables por teclado | 56 |
| Enfocables con nombre | 55 |
| Tipos enfocables sin nombre | `ControlType.Edit=1` |

La revisión técnica del único `Edit` confirmó que su nombre está vacío; su AutomationId y ClassName
también están vacíos y el framework reportado es Slint. No se leyó ningún valor ni texto del
control. La evidencia basta para marcar un candidato de accesibilidad, pero no para identificar qué
editor de Naygo representa.

## Mejora local posterior

El editor seleccionable que muestra texto en el panel de preview (`TextInput` de solo lectura)
ahora declara la etiqueta accesible localizada «Contenido de la vista previa de texto». La etiqueta
describe el propósito del control y no incorpora la ruta ni el contenido del archivo. Se verificó
la paridad de la nueva clave en los diez idiomas y la UI compila. Este editor es un candidato plausible
para el `Edit` observado, no una identificación confirmada: la inspección fue sobre un EXE anterior.

Como segunda medida defensiva, el `TextInput` compartido por crear/renombrar/comprimir ahora recibe
`accessible-label` desde el mismo propósito localizado que se presenta como título del diálogo. La
expresión conserva el nombre contextual al renombrar conflictos y usa etiquetas específicas al
comprimir o al crear. Este es otro candidato posible al `Edit` sin nombre; el binario inspeccionado
no permite identificarlo retroactivamente.

La auditoría local de controles Slint encontró más `LineEdit`/`TextEdit` cuyo propósito dependía de
placeholders o textos visuales vecinos, que no garantizan un nombre UIA. Se añadieron etiquetas
accesibles a búsqueda y categorías de configuración, límites y rutas configurables, alta de grupos
de favoritos, filtros de columnas, recetas, consultas guardadas, búsqueda de contenido, reintentos,
comparaciones, espacios y edición inline. Se reutilizaron etiquetas ya traducidas y no se incorporó
ningún valor, ruta o contenido escrito al nombre accesible. La UI compila y el gate completo pasa;
esto todavía no demuestra qué control produjo el `Edit` histórico.

## Reinspección del build actual

Con autorización previa se actualizó in-place desde `dist/Naygo-0.5.2-setup.exe`, código de salida
0, y se inició la build instalada como PID 23868. Su SHA-256 es
`21e97c3557ce5998429c83d0820e3212e5a8c2ebdca77a27eafdc45e1f4fd831`, igual a Release. Los cuatro
JSON de usuario (`settings`, `workspace`, `favorites`, `templates`) y el `naygo.pdb` anterior
conservaron exactamente su SHA-256 antes y después. La ventana principal respondió.

La inspección UIA pasiva de ese PID encontró 330 descendientes, 239 con nombre y 46 controles
enfocables, los 46 con nombre; no truncó el árbol y no vio controles enfocables sin nombre. La sesión
seguía bloqueada y el árbol visible no contenía `Edit`: esta lectura no demuestra el nombre de cada
campo de configuración, receta o diálogo, ni permite atribuir el `Edit` histórico. No se abrió ningún
diálogo ni se enviaron entradas.

## Instalación actualizada — 2026-10-18 21:33 local

Con autorización previa, se actualizó in-place desde el setup 0.5.2 verificado contra
`dist/SHA256SUMS.txt` (SHA-256 `fccfd7a82fe80a1e200a2989dcc042c32abde9d96d6e00f69abf869346570ee4`);
el instalador terminó con código 0. El EXE instalado quedó con SHA-256
`210901465840187a497dca452d56b0cbf843807c11c52079e0bbeca0ac1d4673`, idéntico al EXE de Release.
Los cuatro archivos JSON de usuario continuaron presentes y parsean correctamente; el PDB previo
también se conservó. Naygo se inició como PID 51452 y respondió.

La inspección UIA de solo lectura encontró 350 descendientes, 276 con nombre y 88 controles
enfocables; los 88 tienen nombre accesible y el árbol no se truncó. No se abrieron diálogos ni se
enviaron entradas. Esto valida la estructura accesible de la ventana principal actual; falta probar
con Narrador y recorrer los campos, menús y acciones en una sesión interactiva.

## Límites y siguiente comprobación

La instancia PID 25124 es anterior al build actual: su SHA-256 instalado es
`feba7919f7f308bacb8198b4abf5eb61c1d8fbfafbe5ff1171916caf92376293`; el EXE recién generado tiene
SHA-256 `21e97c3557ce5998429c83d0820e3212e5a8c2ebdca77a27eafdc45e1f4fd831`. `LockApp` y `LogonUI`
estaban activos durante la reinspección; por eso no se hizo una revisión visual ni se recorrió la UI
con Narrador, y no se enviaron entradas. La inspección estructural del build actual sí quedó hecha,
pero no cierra W-13.

Al contar con una sesión desbloqueada, abrir las áreas de configuración, búsqueda, recetas y
consultas; recorrerlas con UIA/Narrador y comprobar que cada campo anuncia su propósito. Repetir en
el preview y revisar la barra de herramientas en distintos anchos/temas. El inspector informa tipos
sin mostrar datos del usuario.

## Gate y distribución de la mejora

El gate actualizado pasó con 1.455 pruebas aprobadas y 8 ignoradas; Clippy no encontró avisos y el
formato está correcto. La build supervisada de 0.5.2 terminó con pico de 10,53 GiB, release de
28m32s e Inno Setup de 128,218 s. El EXE del ZIP coincide con Release y el portable no incluye PDB.

| Artefacto | SHA-256 |
| --- | --- |
| `dist/Naygo-0.5.2-portable.zip` | `a3667816eee192b6252e53c71fbeeba59f34e12ce83d041a1339e462e04c0656` |
| `dist/Naygo-0.5.2-setup.exe` | `34816a4f1e43b969cb5720611b0355a794930b1f279e2c8a5815a6cdf97cef0f` |
| `target/release/naygo.exe` | `21e97c3557ce5998429c83d0820e3212e5a8c2ebdca77a27eafdc45e1f4fd831` |

Los hashes concuerdan con `dist/SHA256SUMS.txt`. El ZIP contiene exactamente `LEEME.txt`, `LICENSE`,
`naygo.exe` y `THIRD-PARTY-NOTICES.md`; no incluye PDB. El hash del EXE extraído coincide con Release.
El setup sigue sin firma Authenticode. La distribución se instaló in-place y quedó abierta como PID
23868; los datos de usuario y PDB anterior se conservaron. W-13 permanece abierto hasta validar los
campos con Narrador y la UI en una sesión desbloqueada.

## Revisión posterior de la instancia activa, 2026-10-18

La sesión de Windows quedó desbloqueada. `scripts/inspect-naygo-uia.ps1` inspeccionó en solo lectura
el PID 48792: 759 descendientes, 571 con nombre, 80 controles enfocables y 80/80 con nombre; árbol
sin truncar y cero controles enfocables anónimos. El árbol de accesibilidad se pudo leer, pero la
captura de pantalla agotó el tiempo incluso tras renovar el identificador de ventana. El proceso
ejecuta el EXE instalado con SHA-256 `94070d7b4dea91ebb9670cfb6f63abf63d8644f06556a4ff0a2c569c2fee3746`,
que no coincide con el release actual `210901465840187a497dca452d56b0cbf843807c11c52079e0bbeca0ac1d4673`.
Por eso, esta evidencia es parcial y pertenece a un build anterior; no valida los controles del
release actual ni sustituye la navegación manual con Narrador. No se activó la ventana, abrió ningún
diálogo ni se enviaron entradas.

## Reinspección pasiva — 2026-10-19 06:03 local

Se volvió a ejecutar `scripts/inspect-naygo-uia.ps1 -MaxDepth 20 -MaxElements 5000` contra la
ventana que ya estaba abierta, PID 48444. El proceso sigue usando el ejecutable instalado con SHA-256
`c428cae8eba6cf0fd6949f737917eb7479757cb79ebc79a8b917aabab3436cf2`, anterior al paquete vigente
en `dist/`.

| Medida | Resultado |
|---|---:|
| Descendientes UIA | 768 |
| Elementos con nombre | 600 |
| Enfocables por teclado | 140 |
| Enfocables con nombre | 140 |
| Tipos enfocables sin nombre | Ninguno |
| Árbol truncado | No |

La inspección fue de solo lectura: no se activó la ventana, no se abrieron diálogos y no se enviaron
entradas. Confirma la estructura accesible de esa instancia previa, no la build final de `dist/` ni
el uso real con Narrador; W-13 sigue pendiente de aceptación interactiva.
