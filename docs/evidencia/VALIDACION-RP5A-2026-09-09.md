# RP-5A — preview de imágenes animadas

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Alcance

WebP, GIF y APNG animados; pausa/reanudación, final natural y repetición explícita.
Decodificación secuencial/composición en proceso aislado, crédito único y reloj del
preview separado del bombeo de listados. Mantiene el modo ajuste/1:1, reglas habilitadas,
aliases forzados a imagen y cancelación por generación. `.apng` se añade a formatos
reconocidos sin migrar preferencias ni cambiar esquema 4. No se habilita `.wepb` por
defecto ni audio/vídeo. Las animaciones decorativas de íconos siguen siendo independientes.

Límites explícitos: entrada 20 MiB, canvas animado 4 Mpx, salida 768 px, como máximo
15 presentaciones/s y delay mínimo 10 ms. No se amplían imágenes animadas pequeñas.
La composición original no se omite al descartar presentaciones. Un frame de salida
ocupa como máximo 768 × 768 × 4 bytes; el Job limita a 192 MiB la memoria comprometida
del hijo. Estos números NO son límites globales ni consumo habitual medido de Naygo.

El proceso persistente mantiene prioridad inferior y Rayon de un hilo. Tasa CPU hard-cap
del 20 % del sistema; cinco segundos de espera sólo por pedido sin respuesta, no durante
pausa ni delays válidos. No hay corte de 60 segundos ni caché de todos los frames.
Pausar deja terminar como máximo el pedido en curso y no emite nuevos créditos. Cancelar
no queda detrás de una cola llena; el supervisor hace kill/reap/join fuera del hilo UI.

## Evidencia de esta iteración

Fuentes y distribución verificadas; entrega local completada el 2026-09-10.
**1.301 pruebas aprobadas**, seis ignoradas preexistentes; Clippy de todo el workspace y
todos los targets sin advertencias. Formato/diff y claves/placeholders de diez idiomas correctos.
Grafo actualizado por AST: 7.946 nodos, 14.461 aristas y 809 comunidades. Se conservan los
29 avisos conocidos de fuentes JSON/otros sin nodos; no se ejecutó extracción semántica externa.
Desglose: 874 core + 22 integración core + 54 platform + 339 UI + 12 integración/visual UI.
Logs definitivos: `target/agent-out/rp5-final-tests.log` y `rp5-clippy-final.log`.
Una suite anterior de 1.300 pruebas también pasó; el conteo definitivo incluye el caso de
deshabilitación del lector activo. No se añadieron exclusiones ni supresiones.

Incidencias resueltas, sin excluir pruebas ni rebajar advertencias:

- El localizador por color del test Slint encontraba hover al sustituir botones bajo el cursor.
  Se mueve el cursor fuera antes de verificar el color normal; también se conservan las capturas.
- Clippy pidió `is_multiple_of` en el generador de fixtures; corregido sin supresiones.
- Una decodificación más lenta que el archivo podía descartar todas las presentaciones atrasadas.
  Se mantiene el reloj pero se garantiza avance visible al tener un frame y llevar 250 ms sin
  presentar; siguen descartándose resultados atrasados durante la recuperación del ritmo.
- Deshabilitar/cambiar el tipo de regla del archivo activo retira inmediatamente su generación.
  Editar otra regla no reinicia la animación ni reactiva un error cancelado.

Muestra privada en depuración: 55 frames en 22.884 ms, cancelación en 30 ms. Es decodificación
sin esperas de presentación, no un tiempo de reproducción de la UI ni un benchmark release.
Los ejemplos sintéticos del proceso hijo alcanzaron unos 6,7 MB comprometidos; esto no mide
el padre ni el pico de la muestra privada. Logs en `target/agent-out/rp5-private-sample-debug.log`.

Pruebas añadidas:

- Protocolo NGA1: tipos/longitudes/dimensiones, truncamiento, reserva acotada antes del cuerpo.
- GIF/APNG: parciales, alpha, Keep/Previous/Background; GIF sin extensión de repetición.
- WebP: alpha, ciclos finitos e infinitos; canvas excesivo y truncado.
- Reloj: delays, pausa/reanudación, créditos, descarte atrasado, estáticos sin reloj,
  cancelación con cola llena, ocultación y A → B → A.
- Proceso real: PNG estático, GIF/APNG/WebP, pausa superior a cinco segundos, final y cancelación.
- Controles Slint: Pausar/Reanudar/Repetir sobre componente real con render software.

La muestra privada de Nicolás se utiliza sólo en lectura. No se copia a fixtures ni se
publican ruta/nombre/contenido. La prueba opcional recorre 55 frames para cruzar el ciclo
de 50, verifica tiempos de 100 ms, cambios de imagen y cancelación. Esa prueba mide lector
aislado y protocolo, no equivale a observar toda la sesión instalada de Windows.

## Distribución

Build **0.5.1+build.202609092333** completado mediante `scripts/build-release.ps1`, solo,
con una tarea de compilación: Rust release 30 min 11 s e Inno 102,453 s. Finalizado
el 2026-09-10 a las 00:04:18 local. Log: `target/agent-out/rp5-release.log`.
La distribución RP-4B1 está respaldada y sus hashes verificados en
`target/agent-out/baseline-051-before-rp5a/`. No se ha instalado sobre la sesión personal
ni se han publicado cambios remotos en esta iteración.

| Artefacto | Bytes | SHA-256 |
|---|---:|---|
| `dist/Naygo-0.5.1-setup.exe` | 52.597.769 | `79ff56427a6262c4294814b09c11027c63fce862d5528416de1879c28e8753b6` |
| `dist/Naygo-0.5.1-portable.zip` | 13.271.192 | `5152d1f00813e84f1b09643aaaf2e71ab8739a7d4daf74b949f47e2e1030b39b` |
| `target/release/naygo.exe` | 36.472.832 | `86c6dd12cab262bffcf62b4f6ec6a281131df739656721a9c8cf16ba9668dc96` |
| `target/release/naygo.pdb` | 342.028.288 | `1bec28011e3988d51afe07d9a2c1316bab1472cfadfa9a76a3c511278d5545b6` |

`SHA256SUMS.txt` coincide con setup/ZIP. `7z t` aprobó el ZIP y sus cuatro entradas
(EXE, LEEME, LICENSE, THIRD-PARTY-NOTICES); EXE/licencia/avisos cotejados por hash con
los originales. PDB como componente opcional del instalador, no dentro del portable.
Metadatos del EXE correctos; instalador sin firma digital, igual que las entregas anteriores.
No se extrajo ni instaló el setup. Portable: 47.451 bytes adicionales frente a RP-4B1.

## Verificación del lector optimizado

Cuatro pruebas aprobadas, cero fallos, 9,16 s; log
`target/agent-out/rp5-release-reader-tests.log`. Incluye PNG/SVG/ZIP/PDF, entradas corruptas
y ausentes, solicitud inválida antes de iniciar UI, cancelación con cola llena y fin estático.
WebP finito conserva una pausa de 5,2 s; GIF/APNG componen cuatro frames parciales.
WebP infinito sintético: ocho frames y cancelación en 29 ms.

La muestra privada recorrió **55 frames en 1.942 ms** de decodificación/IPC sin esperas de
presentación, cruzando su ciclo de 50 frames, con timestamps de 100 ms y cambios de píxeles.
Cancelación en **30 ms**. No es duración de reproducción en UI ni benchmark con caché fría.
Picos comprometidos de hijos sintéticos: aproximadamente 4,7 MB. La lectura estática de la
muestra tuvo picos de unos 18,4 MB; no se midió el pico animado privado ni la memoria global.

## Pendientes que no se cierran

RP-4: filesystem bloqueado en texto/3D y presupuesto global de UI/cachés/escenas.
RP-5: política/calidad económica configurable y aceptación instalada completa (minimización,
pestañas, DPI, VM, Narrador y archivos de otras procedencias). Los tests no cubren todas
las variantes de cada codec. RP-6: PDF/TAR/texto extremo y métricas locales/comparativas.
No se promete consumo global ni mejora de navegación sin una medición comparable.
