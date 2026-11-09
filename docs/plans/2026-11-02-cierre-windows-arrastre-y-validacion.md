# Plan de cierre Windows: arrastre interoperable y validación

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Objetivo y estado

Preparar una entrega Windows integrada, priorizando el arrastre entre aplicaciones, sin perder
datos ni congelar la interfaz. La entrega local 0.5.4 integra paneles auxiliares, cierre
automático configurable de Operaciones, recuentos, protección del efecto OLE y un receptor STA que
captura el destino antes de consultar el origen. La build 0.5.5 agrega la omisión reversible por
sesión o persistente de la confirmación externa, configurable desde el diálogo y Configuración.
El gate de 0.5.5 pasó 1.575 pruebas, 9 ignoradas, Clippy y formato; setup y portable se generaron
bajo supervisor, y la versión quedó instalada localmente. El setup no tiene firma. README y
capturas sintéticas fueron actualizados; el CI posterior al push (run `42512780530`) aprobó Windows
y benchmark; Linux fue omitido por diseño. La aceptación OLE general aún requiere completar la
matriz de clientes y condiciones. El gate final posterior de Windows (`42543957372`) también aprobó
el HEAD de producto `af2c64f3`: 1.575 pruebas, 9 ignoradas, Clippy, formato, pins e i18n; Linux y
benchmark se omitieron por diseño.

La prueba E2 anterior recibió desde 7-Zip 24.07 una carpeta con 259 archivos y cuatro
subcarpetas, una vacía, contrastados por SHA-256. El 2026-11-08 Nicolás confirmó una segunda
prueba con 7-Zip: soltó la fixture sintética `E2 fixture`, aceptó la copia y el manifiesto de 35
archivos y 4 carpetas coincidió en hashes y estructura en el destino elegido. La fixture se dejó
en ese destino sincronizado por OneDrive. Esto valida una copia de carpeta con esta build, pero no
cierra mover/cancelar, otras aplicaciones ni la matriz de condiciones adversas.

Revalidación local 2026-11-08: `cargo test -p naygo-platform --locked drop_target::` aprobó 31
pruebas y dejó una medición ignorada; la medición explícita midió la captura/materialización de
10.000 archivos pequeños en 4,77/12,09 s. La fixture ZIP sintética pasó `7z t` (35 archivos,
árbol anidado y carpeta vacía). La captura automática de la ventana expiró dos veces, pero Nicolás
completó manualmente la copia integrada en la build 0.5.4 y se verificó el manifiesto: 35 archivos,
4 carpetas, sin faltantes, extras ni hashes distintos. Esto no representa el tiempo end-to-end de
7-Zip ni cierra la matriz de otros clientes y condiciones adversas.

Revisión de cierre 2026-11-08: la build local 0.5.5 sigue instalada y el proceso Naygo responde; su
ejecutable coincide con el hash registrado en `docs/ESTADO-ACTUAL.md`. La rama de avance estaba limpia
y sincronizada con `origin` al iniciar esta revisión. Computer Use volvió a fallar al capturar la
ventana; Nicolás completó la comprobación y entregó dos capturas: la primera muestra la confirmación
con ambas casillas de «no volver a preguntar» sin marcar; la segunda muestra `E2 fixture` en el
destino. La comparación posterior del manifiesto sintético fuente/destino dio 35 archivos, 4
directorios, 39 entradas y cero diferencias SHA-256. Queda validada otra copia de carpeta en 0.5.5
y la presencia visual de las dos opciones, pero no el comportamiento al seleccionar cada casilla.
Directory Opus está instalado, pero su mera presencia no se cuenta como prueba. La evidencia previa
registrada en `docs/BACKLOG.md` cubre mover/cancelar desde Directory Opus; Explorer y la matriz
restante de condiciones adversas continúan abiertas.

## Fundamento y elección técnica

El receptor debe admitir formatos anunciados por cada origen, no suponer que son equivalentes.
Windows ofrece `CF_HDROP` para rutas y formatos `FILEDESCRIPTOR`/`FILECONTENTS` para elementos
virtuales. `IDataObjectAsyncCapability` es opcional y solo debe usarse tras comprobar y negociar
su soporte. 7-Zip explica que al arrastrar a Explorer puede extraer primero a temporales; su
[código 24.07](https://github.com/ip7z/7zip/blob/a7a1d4a241492e81f659a920f7379c193593ebc6/CPP/7zip/UI/FileManager/PanelDrag.cpp)
no permite asumir soporte asíncrono del Shell. No escribir directamente en el destino mediante un
protocolo específico de 7-Zip sin demostrar confirmación, resultado, conflictos y cancelación.
Fuentes: [formatos del Shell](https://learn.microsoft.com/en-us/windows/win32/shell/clipboard),
[transferencia asíncrona](https://learn.microsoft.com/en-us/windows/win32/api/shldisp/nn-shldisp-idataobjectasynccapability),
[FAQ oficial de 7-Zip](https://d.7-zip.org/faq.html).

Clippy analiza código Rust; no implementa portapapeles ni arrastre. Mantenerlo en el gate, y
auditar el portapapeles como flujo relacionado pero independiente, sin prometer que una biblioteca
existente resuelva la duración de los temporales. Evaluar implementaciones existentes y licencias
compatibles; incorporar dependencias solo con mejora demostrada y sin regresiones de rendimiento.

## Bloques de ejecución, en orden

### 1. Fijar el destino y proteger datos

- [x] Antes de `GetData`, el STA solicita al hilo UI el panel Files bajo el punto del drop y captura
  su id y ruta. Un destino inválido, ocupado por otro tipo de panel o que no se puede consultar se
  rechaza; no hay fallback al panel activo. Si la ruta ya no corresponde al panel tras la
  preparación, la UI pide elegir explícitamente otro panel abierto. La subcarpeta se elige
  navegando a ella antes del drop.
- [x] Confirmar con la ruta completa antes de escribir. La confirmación permite envolver rutas
  largas y cancelar sin ejecutar la operación.
- [x] Negociar los efectos permitidos y diferir la operación de Naygo hasta después de la
  confirmación; respetar los modificadores de copia/movimiento recibidos y resolver conflictos por
  el flujo habitual.
- [x] En drops externos, permitir desactivar la confirmación por sesión o persistentemente. Ambos
  controles están disponibles también en Configuración; omitir el modal genérico conserva la
  validación de destino y la decisión independiente de los conflictos. Build local 0.5.5 instalada.
- [ ] La consulta del destino ocurre antes de `GetData`, pero no sincroniza atómicamente todos los
  mensajes de ratón ya encolados por Windows. Completar la matriz de cambios de carpeta, cierre y
  reordenamiento de paneles, dos drops simultáneos y cancelación; verificar ausencia de parciales.
- [x] Omitir la confirmación externa por sesión o de forma permanente desde el diálogo, con ambos
  controles modificables en Configuración. Incluso al omitirla deben seguir activos el probe del
  destino y el diálogo propio de los conflictos. Implementado y probado en el gate 0.5.5; la captura
  de Nicolás confirma que las dos casillas aparecen en el diálogo. No se probó manualmente elegir
  cada casilla ni revisar su estado en Configuración después de reiniciar.

### 2. Recepción interoperable

- [x] Receptor OLE STA separado del hilo UI, con referencias COM conservadas durante el ciclo de
  drop y preparación comunicada por canales. El hit-test se hace antes de pedir datos al origen.
- [x] Rutas persistentes y árboles temporales `CF_HDROP`: copia acotada fuera de la UI, conserva
  carpetas vacías, valida rutas/reparse points y limita archivos, carpetas y profundidad.
- [x] Objetos virtuales `FILEDESCRIPTOR`/`FILECONTENTS`: materialización en staging antes del
  motor habitual de operaciones. Se rechaza explícitamente un formato sin destino válido.
- [x] Preparación visible y cancelable antes de completar la consulta de datos; el historial
  diferencia archivos y carpetas y evita reconstruir listas completas de nombres no mostrados.
  Una llamada COM bloqueante a `GetData` no se puede interrumpir: la solicitud de cancelación se
  aplica cuando esa llamada retorna.
- [x] Se conservaron las rutas de arrastre interno y el portapapeles como flujos separados; el
  gate cubre regresiones automatizadas. La validación interactiva final sigue pendiente.

### 3. Matriz de aceptación Windows

- [x] 7-Zip 24.07: se aceptaron dos copias manuales de carpetas sintéticas, una de 259 archivos y
  otra de 35 archivos con cuatro carpetas (incluida una vacía). Los manifiestos, hashes y estructura
  coincidieron. La segunda copia se repitió durante este cierre desde la build 0.5.5: 35 archivos,
  4 directorios, 39 entradas y cero diferencias SHA-256. Esto acredita copia de carpetas en esa
  versión; no acredita movimiento ni cancelación.
- [x] Directory Opus: la evidencia previa registrada en `docs/BACKLOG.md` cubre cancelar y mover
  una fixture con verificación SHA-256 en la integración anterior a 0.5.5. La versión 0.5.5 no cambió
  la negociación de efectos OLE; no se vuelve a contar esto como prueba en esa build.
- [ ] Explorer: probar archivos y carpetas desde una ventana real, y registrar el resultado. La
  instalación de Explorer es parte de Windows, pero no hubo una sesión de drop con Naygo durante
  esta revisión. No afirmar compatibilidad universal con programas no probados.
- [ ] Casos: archivos, carpetas, mezcla, vacías/anidadas, Unicode, archivos grandes y muchos
  pequeños; mismo disco y distinto disco, USB/red disponible, ruta larga, acceso denegado y destino
  que desaparece. Cubrir archivos virtuales con origen sintético y aplicaciones disponibles.
- [ ] Copiar/mover, Ctrl/Shift, fuentes copy-only/move-only, cancelar antes y durante, conflictos,
  repeticiones, caída del origen y actividad simultánea. Verificar contenido y estructura con
  hashes, ausencia de pérdidas/duplicados y limpieza; no usar archivos personales.
- [ ] Pruebas automatizadas representativas, gate aislado, CI Windows actualizado y evidencia de
  pruebas reales. Si no hay sesión interactiva disponible, dejar la aceptación explícitamente
  pendiente; no repetir ciclos manuales sin instrumentación ni pedir datos reales al usuario.

### 4. Cierre visual y documentación

- [x] Marcar el panel Files o la Bandeja que está bajo el cursor durante un arrastre, sin mover
  el foco de teclado. Implementado en `715904d4`; falta validar visualmente en la instalación.
- [ ] Validar en la instalación real paneles únicos y duplicados heredados, ubicación tras cerrar,
  reiniciar y cambiar el layout, autocierre y errores; revisar foco y apariencia en temas claros,
  oscuros y contrastantes. No eliminar disposiciones o preferencias existentes.
- [ ] Evaluar visualmente órbita respecto a cámara, colores, modos sólido/bordes/malla y colores
  base 3MF ya implementados; medir en equipos modestos. Texturas y materiales extendidos son una
  capacidad adicional, no declarar soporte integral; no agregarlos sin evaluar coste y alcance.
- [x] Actualizar README y dos capturas de Naygo 0.5.4 con perfil aislado, dos temas y datos
  sintéticos; inspeccionadas sin rutas personales ni detalles de discos. Ver
  `docs/CAPTURAS-SINTETICAS.md`.
- [ ] Consolidar entradas históricas contradictorias sin borrar evidencia. Comprobar preview de
  texto y límites configurables en las otras máquinas cuando exista acceso, sin asumir que se
  validó un sistema remoto.

### 5. Entrega y pendientes externos

- [x] Integrar en la rama de avance y actualizar Graphify. Gate local: 1.572 pruebas aprobadas,
  9 ignoradas, Clippy sin warnings y formato limpio.
- [x] Build supervisado de distribución: pico 10,78 GiB; ZIP y setup 0.5.4 creados. ZIP sin PDB,
  EXE del ZIP igual a release; instalación terminó con código 0 y preservó settings, sesión,
  favoritos y plantillas. El README y las capturas quedaron actualizados; hashes registrados en
  `docs/ESTADO-ACTUAL.md`.
- [x] Cierre de producto 0.5.5, 2026-11-08: gate local supervisado (1.575 pruebas aprobadas,
  9 ignoradas, Clippy sin warnings, formato e i18n correctos), setup y ZIP generados, setup instalado
  y ejecutable abierto con hash coincidente. Los hashes y el pico de memoria constan en
  `docs/ESTADO-ACTUAL.md`. La distribución Windows sigue sin firma Authenticode y no se publicó un
  GitHub Release.
- [x] CI final sobre `af2c64f3` (run
  [42543957372](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/42543957372)):
  Windows gate aprobado: 1.575 tests, 9 ignorados, Clippy sin warnings, formato, pins e i18n.
  Linux y benchmark omitidos por diseño. El commit documental de este cierre no modifica código.
- [x] CI manual sobre `a28d2810` (run `42504789478`): gate Windows aprobado (1.571 tests, 9
  ignorados, Clippy, formato, pins, prueba SignPath e i18n); benchmark aprobado. El workflow
  global quedó fallido por Linux, con 972 aprobadas y 27 fallidas en `core/platform`; esa incidencia
  se sigue aparte. Completar la aceptación OLE real con Explorer, Directory Opus y 7-Zip antes de
  declarar compatibilidad cerrada.
- [ ] Firma: continuar preparación y verificar requisitos de SignPath OSS; la aprobación de la
  organización, credenciales y la primera firma real dependen del servicio y de Nicolás. No
  afirmar firma ni publicación antes de verificarlas.
- [ ] Linux Ubuntu 26.04 LTS x86_64, bandeja GLib y sus implicancias de licencia: conservar el
  plan y la evaluación aprobada, pero continuar tras Windows; corregir CI Linux y probar escritorio
  real antes de anunciar una alfa. La política GPL Linux aprobada no cambia la licencia Windows.

## Criterio de cierre

La entrega queda lista cuando la matriz Windows acordada pasa, la aplicación permanece sensible,
no hay pérdida ni destino incorrecto en los escenarios de fallo, los resultados se reflejan con
honestidad en la UI y el instalador verificado queda disponible. Registrar limitaciones concretas;
si alguna fuente no puede garantizar una transferencia segura, conservar el rechazo explícito y
no declarar compatibilidad completa. Estimación en bloques, no promesa de calendario: captura de
destino y seguridad 1–3 sesiones, integración y matriz 2–5 sesiones, cierre visual y distribución
1–2 sesiones; pruebas interactivas, rendimiento y compilación pueden extender esos rangos.
