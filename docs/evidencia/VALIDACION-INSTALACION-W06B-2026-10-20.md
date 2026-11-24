# Validación de instalación Windows W-06B — 2026-10-20

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Resultado

Con la autorización previa de Nicolás, se actualizó in-place la instalación de Naygo desde
`dist/Naygo-0.5.3-setup.exe`, que incluye W-06B. El instalador terminó con código 0. Después se
abrió la aplicación instalada; quedó un único proceso, PID 43624, y Windows reportó
`Responding=True`.

El SHA-256 del EXE instalado coincide con `target/release/naygo.exe` y con el ejecutable del ZIP:

| Artefacto | SHA-256 |
| --- | --- |
| `Naygo-0.5.3-portable.zip` | `1b8212b02d18e71eb07f6d33e24c92fc63344615e44955d2d11ecb21518b4453` |
| `Naygo-0.5.3-setup.exe` | `998c30daa7422d61e4187603462b575be7641549f189361c3658fd5771333000` |
| `naygo.exe` Release e instalado | `868a69dec0f6f79d0e91e54326eb9762cd745921467dd45a72fd5a4000bd2b5a` |

Authenticode informa `NotSigned` para el setup; los artefactos no se publicaron.

## UI Automation pasiva

El inspector de solo lectura recorrió la ventana del PID 43624 sin truncar el árbol: 356 descendientes,
295 elementos nombrados, 120/120 controles enfocables nombrados, 69 acciones `Invoke` y cero botones
sin ese patrón. El inspector consulta disponibilidad de patrones y no invoca controles ni registra
nombres, rutas o contenido. Es una captura del árbol de la ventana, no una prueba de Narrador, foco,
teclado o flujos manuales; W-13 continúa abierto.

## Preservación de datos

Antes de instalar se copiaron y calcularon los hashes de los cuatro JSON que ya existían en la
carpeta de instalación. Los mismos hashes se observaron después; cada archivo volvió a parsearse
como JSON válido:

| Archivo | Bytes | SHA-256 antes y después |
| --- | ---: | --- |
| `settings.json` | 5.027 | `5eae204a5c83bd856a8ad139a99b73bb15b25171e0072f62e3e13710553d1737` |
| `workspace.json` | 7.770 | `51b72850cc5944cf68c54138cf5e2c4277e0fd81a08d8f216e0dc68710b8eda7` |
| `favorites.json` | 860 | `0cf87ca76089e1fecff18433f9697783f6751724b6008c693a0d3106ee6d52cf` |
| `templates.json` | 160 | `7557750482a01e650db278b14e55d56d656004a775b40b6e7e29d93bb1da43e0` |

El `naygo.pdb` de 289.722.368 bytes quedó con SHA-256
`eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`, igual al registrado en la
validación del 19 de octubre. El respaldo de los cuatro JSON y su manifiesto previo están en
`target/agent-out/install-w06b-20261020/`.

## Muestra W-07

Sobre ese PID y EXE se ejecutó `scripts/capture-w07-process.ps1` durante 3×30 s, con 16 muestras por
ventana. Se observó un solo proceso y ningún mapeo SMB. Working set máximo: 57,0 MiB; memoria privada:
26,3–26,4 MiB; handles: 370–375; hilos: 25–26; CPU por ventana: 0 / 0,015 / 0 s.

Es una observación pasiva, sin entrada de usuario. `LockApp` y `LogonUI` estaban activos y la sesión
seguía bloqueada. Por eso estos datos no confirman reposo controlado, latencia de navegación ni
presentación y **no cierran W-07 ni W-13**. El registro completo está en
`target/agent-out/w07-installed-053-w06b-20261020-passive.json`.
