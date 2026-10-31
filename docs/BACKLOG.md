# Backlog — pendientes y mejoras de Naygo

> Documento de arranque para nuevas sesiones. Todo lo acordado que aún no se implementa,
> con contexto y punteros al código relevante. Actualizarlo al cerrar cada ítem.
>
> **Solicitudes Windows 0.5.4, 2026-10-29 — pendientes de implementación:**
> - [x] Empaquetado: corregida la mezcla de un setup anterior con un ZIP nuevo cuando falta Inno;
>   el preflight preserva dist y aborta si existe setup de la misma versión. La prueba aislada
>   pasa y preserva también el manifiesto. Empaquetado normal supervisado y hashes verificados.
> - [ ] Arrastrar carpetas anidadas desde 7-Zip al panel, preservando contenido y carpetas vacías.
>   El staging actual de `CF_HDROP` temporal rechaza explícitamente cualquier origen que no sea
>   archivo (`open_ephemeral_hdrop_files`). Debe retener los contenidos antes de que 7-Zip borre
>   el temporal, sin bloquear la interfaz innecesariamente; probar también el arrastre OLE real,
>   archivos mezclados, errores y estructuras grandes. La regresión anterior cubre solo archivos.
>   Revisión adicional: el callback espera como máximo dos segundos a que el worker abra los
>   handles y luego retorna aunque no estén listos; enumerar un árbol sin resolver esa carrera no
>   garantiza conservar árboles grandes. Investigar la captura de la raíz temporal completa y su
>   comportamiento real con 7-Zip antes de escoger el mecanismo; no seguir enlaces del filesystem.
>   Revisión 2026-10-30: se descartó trasladar carpetas solo por reconocer el nombre `7zE`, pues
>   ese nombre no acredita que la ruta sea propiedad de 7-Zip. El preparador de fixtures fallaba al
>   ejecutarse sin argumentos por evaluar `$PSScriptRoot` en el parámetro; se corrigió y se comprobó
>   la ejecución sin argumentos, el destino vacío y `7z t` del ZIP (dos archivos). Fixture:
>   `target/agent-out/e2-7zip-fixtures/20261030-002720-34ab8511/`. La ventana de 7-Zip se abrió,
>   pero dos capturas de Computer Use agotaron el tiempo; no se ejecutó el arrastre OLE real.
>   Investigación 2026-10-30 en [PanelDrag.cpp oficial de 7-Zip]
>   (https://github.com/ip7z/7zip/blob/main/CPP/7zip/UI/FileManager/PanelDrag.cpp): existe un
>   protocolo OLE propio (`7-Zip::GetTransfer`, `SetTransfer` y `SetTargetFolder`) con flags de
>   temporales y de espera; el código también distingue rutas preliminares y finales de `CF_HDROP`.
>   Es una vía a investigar para coordinar la extracción sin trasladar carpetas ajenas. No se ha
>   demostrado que resuelva cancelación, confirmación o fin asíncrono ni que coincida con la
>   versión instalada 24.07; no conectarlo al destino final sin pruebas de conflictos y fallos.
>   Contraste posterior con el [código 24.07 exacto](https://github.com/ip7z/7zip/blob/a7a1d4a241492e81f659a920f7379c193593ebc6/CPP/7zip/UI/FileManager/PanelDrag.cpp):
>   el tag oficial apunta a `a7a1d4a241492e81f659a920f7379c193593ebc6` y el archivo consultado
>   tiene SHA-256 `44724b49d6b78a2dc6e20806aa8f296c5d11b405e3052eadeb3d84fec4e6d67c`.
>   En 24.07, `SetTargetFolder` no basta para confirmar una operación: con destino no vacío la
>   fuente puede ejecutar `CopyTo` después del retorno de `DoDragDrop`, sin callback de resultado
>   hacia Naygo en ese flujo. Sin destino especial, `GetData` puede iniciar la extracción y la
>   limpieza del temporal ocurre después del retorno. Evitar un falso éxito y conservar cancelación,
>   colisiones e historial sigue siendo parte indispensable de la solución.
>   Revisión adicional del mismo código 24.07: `CDataObject` declara únicamente `IDataObject`
>   mediante `Z7_COM_UNKNOWN_IMP_1_MT`, sin `IDataObjectAsyncCapability`; la vida del directorio
>   temporal queda ligada al ámbito del arrastre. Por tanto no se puede asumir que retener el COM
>   object o usar el protocolo asíncrono del Shell evite la limpieza tras `DoDragDrop`.
>   Hallazgo de rendimiento: `Drop` llama `IDataObject::GetData(CF_HDROP)` en el hilo UI y el
>   `CDataObject::GetData` de 7-Zip 24.07 puede ejecutar `CopyFromPanelTo_Folder` antes de
>   responder. La espera de dos segundos cubre solo el snapshot posterior, no la extracción;
>   resolver la respuesta de UI exige abordar también esa llamada COM, no solo el recorrido del árbol.
>   El preparador admite ahora un árbol anidado, carpeta vacía y `-ExtraFiles` (0–10.000). Se generó
>   y comprobó con 7-Zip 24.07 un ZIP con 259 archivos, una carpeta vacía y destino vacío en
>   `target/agent-out/e2-7zip-fixtures/20261030-020929-91816ffe/`. Es una fixture; el arrastre real
>   y las pruebas de estructuras más grandes siguen pendientes.
>   Experimento local de handles (`scripts/measure-ephemeral-tree.rs`): tras abrir todos los
>   archivos se borraron las rutas sintéticas y se reconstruyeron y verificaron archivos y carpeta
>   vacía. Captura de 1.000 archivos: 260,071 ms; 10.000: 4.221,322 ms, una ejecución cada una.
>   Confirma que el timeout actual de dos segundos no basta para ese árbol. No prueba OLE, carreras,
>   redes, junctions ni límites de handles; el experimento no es una solución de producción.
>   El experimento se amplió con `OPEN_REPARSE_POINT`, verificación de atributos del handle y
>   handles de directorio sin compartir borrado. Con acceso cero o solo a atributos, Windows
>   permitió borrar un directorio abierto; al solicitar `FILE_LIST_DIRECTORY`, rechazó borrar y
>   renombrar una carpeta vacía retenida (error 32). Una junction sintética fue rechazada y su
>   destino permaneció intacto. Con este método: 1.000 archivos en 298,016 ms y 10.000 en
>   4.903,819 ms, una ejecución por tamaño (la de 10.000 precedió a la prueba de renombre).
>   Falta demostrar seguridad de antecesores, cancelación, límites y coordinación OLE; es solo
>   un prototipo aislado, sin comportamiento habilitado en la aplicación.
>   Estudio de espera COM: [COWAIT_DISPATCH_WINDOW_MESSAGES](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/ne-combaseapi-cowait_flags)
>   permite despachar mensajes de ventana durante `CoWaitForMultipleHandles`, pero el código local
>   de winit 0.30.13 en `platform_impl/windows/event_loop/runner.rs` encola eventos si el handler
>   está ocupado y trata `RedrawRequested` de forma especial. Despachar mensajes no demuestra que
>   Slint siga respondiendo ni descarta reentrada. Antes de usar esta vía se necesita una prueba
>   integrada con OLE real, entrada, repintado, cancelación y cargas grandes; otra opción es diseñar
>   un destino OLE dedicado sin depender de una espera reentrante en el hilo de la UI.
>   Prueba sintética STA/MTA posterior: en este Windows `CoWaitForMultipleHandles` con solo
>   `COWAIT_DISPATCH_WINDOW_MESSAGES` devolvió `CO_E_NOT_SUPPORTED` (0x80004021); con ese flag y
>   `COWAIT_DISPATCH_CALLS` el worker recuperó CF_HDROP. No se inyectaron mensajes de ventana ni se
>   usó Winit o 7-Zip: el resultado no demuestra que la UI siga respondiendo ni que el flag
>   combinado sea portable. La prueba incluye fallback COM sin exigir esos flags a otros equipos.
>   Un sondeo posterior de ventana Win32 mínima publicó un mensaje durante esa espera combinada;
>   en esta ejecución se procesó antes del evento de fin. Pasan ocho pruebas focalizadas y Clippy;
>   el caso de carpeta sigue ignorado. El sondeo no cubre Slint/Winit, entrada, repintado ni 7-Zip,
>   y no establece comportamiento para todos los equipos.
>   Preparado `crates/platform/examples/drop_probe.rs`: ventana OLE aislada que usa el receptor
>   actual y registra el payload sin copiar al destino ni modificar configuración. Compila y pasa
>   Clippy focalizado. Se abrió con el ZIP sintético: UI Automation identificó su carpeta de 259
>   archivos, pero la captura de ventana de 7-Zip agotó dos veces el tiempo; no hubo arrastre real.
>   Prueba OLE sintética agregada: el control con archivo pasa y el caso de carpeta con subcarpeta
>   vacía falla al ejecutar explícitamente la prueba ignorada porque se rechaza el directorio.
>   Queda ignorada hasta implementar el soporte; 6 pruebas focalizadas y Clippy de plataforma pasan.
>   Nuevo prototipo aislado `crates/platform/examples/drop_probe_async.rs`: marshaling de IDataObject
>   al worker, captura de handles durante una espera COM con timeout de 30 s, y verificación
>   posterior de 259 contenidos contra fixture y carpeta vacía. Dos pruebas del ejemplo y Clippy
>   pasan: una utiliza IDataObject sintético, borra el árbol y compara los 259 archivos retenidos.
>   Un intento previo que esperaba solo dos segundos observó que la limpieza aún no había terminado;
>   el verificador espera ahora la desaparición por hasta 30 s. Usa ventana Win32 mínima, límite de
>   5.000 archivos y solo acepta esa fixture. No está integrado en Naygo y en timeout el worker
>   aún podría estar usando el origen; se debe coordinar cancelación, finalización y limpieza.
>   Prueba posterior con tres hilos y fuente OLE sintética no ágil: el receptor en STA independiente
>   capturó 259 archivos, preservó la carpeta vacía y verificó sus contenidos tras eliminar el origen;
>   la ventana Win32 procesó temporizadores mientras la fuente simuló 800 ms de demora. Las tres
>   pruebas del ejemplo y Clippy pasan. No valida Slint/Winit ni reemplaza el arrastre real con 7-Zip;
>   sigue sin resolver cancelación, timeout y protección completa frente a carreras del filesystem.
>   El prototipo ahora señala cancelación al agotar la espera y la comprueba al volver de `GetData`
>   y durante el recorrido; al abortar, descarta los handles capturados. Una prueba sintética de
>   cancelación previa al recorrido comprueba la liberación de handles; cuatro pruebas y Clippy
>   pasan. No interrumpe `GetData` ni una llamada de filesystem bloqueada, ni prueba cancelación
>   durante una llamada de filesystem o con 7-Zip real; sigue sin ser una solución integrada.
>   Prueba posterior con proveedor COM sintético detenido dentro de `GetData`: cancelar no terminó
>   el worker mientras el proveedor siguió bloqueado; al liberarlo, el worker devolvió cancelación
>   antes de recorrer el origen. Las cinco pruebas del ejemplo y Clippy pasan. La prueba no demuestra
>   cancelación inmediata, comportamiento de 7-Zip ni limpieza si el proveedor nunca retorna.
>   Investigación de cancelación COM: la [documentación de Microsoft](https://learn.microsoft.com/en-us/windows/win32/com/canceling-method-calls)
>   indica que cancelar puede desbloquear al cliente sin detener al proveedor. En un experimento
>   aislado con proveedor COM sintético no ágil, `CoEnableCallCancellation` y `CoCancelCall`
>   devolvieron éxito; el worker retornó `0x80010002` mientras el proveedor aún estaba bloqueado.
>   Seis pruebas y Clippy del ejemplo pasan. Esto no prueba 7-Zip ni la limpieza segura y no se ha
>   conectado al prototipo operativo ni a Naygo.
>   Prueba adicional del ejemplo aislado: después de retener handles de una selección sintética
>   mixta (carpeta anidada, archivo independiente y subcarpeta vacía), se eliminó el origen y se
>   reconstruyó el árbol en un destino temporal propio; contenido y carpeta vacía coinciden. Otra
>   prueba rechaza `..` al materializar. Pasan ocho pruebas del ejemplo y Clippy. La materialización
>   existe solo en el test, no es el staging de producción ni prueba OLE con 7-Zip real.
>   La sesión volvió a bloquearse antes de la prueba real; se solicitó desbloqueo cuando sea posible.
>   Consulta WTS posterior reportó sesión 1 bloqueada (`SessionFlags=0`); el estado anterior es
>   desconocido. La siguiente prueba interactiva requiere escritorio desbloqueado.
>   Nicolás desbloqueó la sesión y WTS indicó `SessionFlags=1`, pero la captura de 7-Zip volvió a
>   agotar el tiempo. Receptor aislado PID 61976 y fixture PID 58132 abiertos; solicitado arrastre
>   manual, sin resultado al registrar. La prueba OLE sintética de carpeta reproduce el rechazo y
>   permanece ignorada; el control sintético con archivo pasa.
>   Nicolás realizó el arrastre real al receptor aislado: dos payloads devolvieron `paths=[]`,
>   `staged=false` y error de temporal que no es archivo para carpetas `Temp\7zE...\E2 fixture`.
>   Confirma el mismo fallo observado en el test sintético; no demuestra una solución ni cubre UI.
> - [ ] Visor STL/3MF: ofrecer colores y modos de superficie/malla que mejoren la percepción de
>   profundidad, con controles claros y costo acotado; evaluar colores y materiales incluidos en
>   3MF, informando honestamente qué propiedades se soportan.
>   El modelo anterior guardaba triángulos sin color por cara; admitir materiales requiere ampliar
>   el modelo, el transporte aislado y el renderizador. Un selector de tonos uniformes no equivale
>   a soporte de materiales del archivo.
>   Contraste con la [especificación 3MF Core](https://github.com/3MFConsortium/spec_core/blob/master/3MF%20Core%20Specification.md):
>   los materiales base definen `displaycolor`; `pid`/`pindex` a nivel de objeto y `pid`/`p1`
>   por triángulo determinan el color. La [extensión de materiales](https://github.com/3MFConsortium/spec_materials/blob/master/3MF%20Materials%20Extension.md)
>   agrega otros recursos, por lo que materiales base solos no equivalen a soporte completo de 3MF.
>   Avance 2026-10-31 en el árbol local: se agregó una opción de color de materiales base y el
>   transporte de color por cara. Pruebas focalizadas iniciales del núcleo aprobadas; la validación
>   completa pasó (1.518 pruebas, 9 ignoradas, Clippy y formato). Build 0.5.4 supervisado terminado;
>   ZIP y setup coinciden con su manifiesto, el EXE del ZIP coincide con release y no hay PDB en ZIP.
>   Sin firma ni instalación; ver hashes y límites en `docs/ESTADO-ACTUAL.md`.
>   Medición local supervisada sobre el mismo 3MF sintético: 17,518 ms uniforme y 18,249 ms color
>   del archivo (+4,2 %), una ejecución de siete rondas sin carga ni transporte; ver `BENCHMARKS.md`.
>   Falta la aceptación visual del visor; extensiones, texturas y
>   colores propietarios siguen pendientes. La transparencia es una aproximación sobre el fondo.
>   Avance 2026-10-30: el core incorpora opciones de color uniforme (azul/blanco/cálido) y modos
>   sólido, bordes y malla con profundidad, sin releer la escena. Los controles de UI están
>   conectados con etiquetas localizadas y reutilizan la escena; se reinician al cambiar archivo.
>   Pasan 17 pruebas focalizadas del core, prueba de ciclo de vida, clics en panel Slint y 20 de
>   i18n; Clippy del core y check UI pasan. Faltan medición comparativa y validación visual. La
>   opción predeterminada conserva el raster sólido anterior. Falta implementar
>   materiales 3MF. La geometría compatible tiene prioridad frente a la miniatura, que queda como
>   respaldo cuando el parser falla; 18 pruebas focalizadas y gate completo aprobados (1.510
>   pruebas, 8 ignoradas, Clippy y formato). Build supervisado generado y ZIP contrastado contra
>   el EXE release; falta aceptación nativa.
>   Corrección posterior: rechazar referencias externas evita confundir objectid externos con
>   locales; pasan 19 pruebas focalizadas y el gate completo (1.511 pruebas, 8 ignoradas, Clippy
>   y formato). Empaquetado supervisado completo; EXE del ZIP coincide con release, setup sin firma.
>   Pendiente aceptación nativa.
>   Comparación supervisada de modos (96.800 triángulos, 1024×768, siete rondas alternadas):
>   sólido 19,247 ms, bordes 20,354 ms, malla 20,074 ms; ver `docs/BENCHMARKS.md` y sus límites.
>   Gate 0.5.4 de avance: 1.509 pruebas aprobadas, 8 ignoradas, Clippy y formato verdes. Medición
>   puntual de sólido: mediana 18,483 ms (96.800 triángulos, 1024×768); sin comparación controlada
>   ni medición de bordes/malla. Build supervisado y hashes verificados; aceptación visual pendiente.
>   Evidencia: `docs/VALIDACION-0.5.4-VISOR-2026-10-30.md`.
> - [ ] Diferenciar visualmente título y pie de panel con colores coherentes en todos los temas;
>   revisar consistencia y legibilidad de todos los temas existentes y su compatibilidad.
>   Implementado el matiz del pie derivado del texto del tema, sin modificar el formato de los
>   temas personalizados. Renderizado automático de los 15 temas aprobado; inspección visual de
>   tres paletas representativas. Gate completo aprobado (1.509 pruebas, 8 ignoradas, Clippy y
>   formato); build posterior generado y verificado. Pendiente revisión nativa.
>   Auditoría de contraste posterior: el texto secundario resultaba inferior a 4,5:1 en Dracula,
>   Solarized Dark y Paper. Las leyendas del pie ahora usan texto principal: prueba focalizada de
>   los 15 temas pasa con mínimo 5,035:1. Gate completo aprobado: 1.511 pruebas, 8 ignoradas,
>   Clippy y formato. Distribución supervisada generada y ZIP verificado; falta revisión nativa;
>   no se garantiza contraste en temas personalizados arbitrarios ni en otros controles.
>   Auditoría posterior: texto secundario ajustado en Dark Blue, Dracula, Verde sobre azul, Neon
>   Retro, Paper y Solarized Dark; este último también ajusta texto principal. Prueba de los 15
>   temas exige 4,5:1 sobre tokens de panel, toolbar, fila, fila alterna y selección. Inspección
>   visual de capturas Dracula/Solarized/Paper; gate completo aprobado (1.511 pruebas, 8 ignoradas,
>   Clippy y formato). Distribución supervisada generada y ZIP verificado; falta aceptación nativa.
>   La comprobación no cubre todas las superficies, estados ni temas personalizados.
> - [ ] Subir la versión a 0.5.4 y generar/validar setup y portable tras las correcciones.
> **Instalación 2026-10-20:** con autorización previa, el setup 0.5.3 de W-06B se actualizó in-place.
> Naygo queda abierto/respondiente como PID 43624; EXE instalado idéntico a Release y ZIP. Los
> cuatro JSON de usuario conservaron hashes y parsean; el PDB previo conserva SHA-256
> `eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`. Muestra pasiva W-07 3×30 s:
> 57,0 MiB working set, 26,3–26,4 MiB privados, 370–375 handles, 25–26 hilos y 0/0,015/0 s CPU;
> 356 descendientes UIA, 120/120 controles enfocables nombrados, 69 Invoke, cero botones sin acción y árbol completo.
> Narrador/aceptación manual pendientes; sesión bloqueada, sin interacción, no cierra W-07/W-13.
> Setup/ZIP no firmados ni publicados. Evidencia:
> `docs/VALIDACION-INSTALACION-W06B-2026-10-20.md`.
> Hallazgo preview de Nicolás, 2026-10-20: `.sif` estaba en modo Auto pero no en las extensiones de
> texto conocidas; corregido en el checkout para previsualizarse automáticamente y migrar settings
> v4 sin pisar reglas existentes. `.sh` ya es texto y la captura muestra «No se pudo leer» (la ruta
> pertenece a `Z:\` en otro equipo); dejar pendiente reproducir allí el error de apertura/lectura.
> Gate completo: 1.503 aprobados, 8 ignorados, Clippy/formato verdes. La regeneración supervisada
> de distribución se detuvo al bajar de 8 GiB libres; los artefactos anteriores quedaron intactos.
> Evidencia: `docs/VALIDACION-PREVIEW-SIF-SH-2026-10-20.md`.
> **Cierre del preview de texto 2026-10-20:** implementada la elección de 64 KiB/256 KiB/512 KiB/1 MiB
> (default 256 KiB) con conteo visible de bytes leídos, caracteres y líneas. Configuración v6 migra
> aditivamente; lectura aislada y límites de 250 líneas/1.000 caracteres por línea se conservan.
> Gate completo: **1.507 aprobados, 8 ignorados**, Clippy y formato verdes. Build supervisado
> 0.5.3 regenerado y verificado el 2026-10-21; ZIP/setup no firmados ni publicados. XML/SIF
> minificados todavía quedan sujetos al límite por línea. Nicolás hará la instalación y pruebas
> manuales en este equipo y en sus máquinas de 8/16 GiB. Evidencia y hashes en
> `docs/VALIDACION-PREVIEW-TEXTO-2026-10-21.md`.
> **W-13, UI Automation pasiva 2026-10-20:** sobre PID 43624 (Release 0.5.3, SHA idéntico a
> `target/release/naygo.exe`) se observaron 356 descendientes, 120/120 enfocables con nombre,
> 69 botones con `Invoke`, cero botones enfocables sin acción y árbol completo. No comprueba
> Narrador, foco ni diálogos; W-13 continúa abierto. Evidencia: `docs/VALIDACION-W13-PASIVO-2026-10-20.md`.
> **W-07, comparación controlada 2026-10-20:** se midieron tres pasadas de listado local a 100k
> con el binario previo y el actual; medianas 261,693 y 295,669 ms (+13,0 %), bajo el umbral
> orientativo de 15 %. CPU total muestreada durante ambos runs 12–31 %, RAM libre 44,02–44,38 GiB.
> La repetición actual queda bajo el promedio variable anterior (330,138 ms); no hay base para
> atribuir una regresión ni optimizar por este delta. Fixtures limpiadas. W-07 sigue abierto para
> interacción, renderer y matriz local/UNC. Evidencia y JSONL en `docs/BENCHMARKS.md` y
> `target/agent-out/w07-{prior,current}-controlled-20261020*`.
> **W-06B, validación de empaquetado 2026-10-20:** todas las operaciones del portapapeles pasan
> por un worker serial; la distribución 0.5.3 se verificó, quedó sin firma y luego se instaló según
> la evidencia anterior. Hashes y estado más abajo. **Resumen L-08 (2026-10-19):** se implementó el backend Linux de instancia única y se
> regeneró la distribución Windows 0.5.3 bajo supervisor (Release 41m23s, Inno Setup 199,766 s,
> pico Job 10,66 GiB). Gate Windows: 1.480 tests aprobados, 8 ignorados, Clippy y formato verdes;
> check/Clippy Linux cruzados de platform y UI también pasan. WSL no inicia (`HCS_E_HYPER_V_NOT_INSTALLED`),
> por lo que los tests Linux compilan pero no corren en este host. El setup se instaló con autorización
> previa; Naygo está abierto y responde como PID 50260, EXE instalado idéntico a Release. Los cuatro
> JSON de usuario y `naygo.pdb` conservaron sus hashes durante la instalación. ZIP/setup no firmados
> ni publicados. ZIP SHA-256 `19f023b99e9a37656da36cba3d72e322586f3000e691225c33d99db5d3bf4cdf`,
> setup SHA-256 `c1d3ee78eafe71ca829f506c3a3c548f72369a23ee636987694cacd824cf0687`, EXE SHA-256
> `960ccee8a5bdfb85eaafcd8fa6112e736d4b050237e7fdfb592f41799671cadc`. Detalle en
> `docs/VALIDACION-L08-INSTANCE-2026-10-19.md`. L-08 sigue abierto hasta ejecución Linux nativa.
> **Follow-up de Papelera Windows, 2026-10-19:** además de comparar identidad sobre UTF-16 ordinal,
> el fallback de Deshacer ahora empareja `DeletedFrom` ignorando diferencias de caja y separadores
> que entrega el Shell, y valida también el nombre. Se añadieron regresiones para carpeta equivalente,
> carpeta incorrecta y nombre incorrecto. Las 11 pruebas de Papelera pasan; gate completo: 1.483
> aprobados, 8 ignorados, Clippy, formato y nueve idiomas verdes. Build supervisado concluido:
> Release 51m07s, Inno 199,000 s, pico 10,66 GiB. ZIP validado con cuatro entradas, sin PDB y con
> EXE idéntico al Release. ZIP SHA-256 `03755670fade2a1cc6adc36a008a916c222915d1f4550aeeb9f108372422d679`,
> setup `fa6fa2cb3f7542243a9076d060466cc0c7cc3c707a506cc0fa2233ff2651fd00`, EXE
> `cb8f28efcd9619463a5977ba6fb771e59656352bb4a49dfe76bb9fb3c0682229`. El 2026-10-20 se instaló
> in-place con autorización previa; settings, workspace, favoritos, plantillas y PDB conservaron
> sus hashes, los JSON parsean, y Naygo quedó abierto/respondiente como PID 31304 con ese EXE.
> ZIP/setup no firmados ni publicados. W-12 remoto pendiente. Ver
> `docs/VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md`.
> W-12/W-14, revalidación local 2026-10-20: `scripts/check_action_pins.py`, `scripts/test-signing.ps1`
> y `actionlint` v1.7.12 pasaron. El binario temporal se obtuvo del release oficial, su SHA-256
> `6e7241b51e6817ea6a047693d8e6fed13b31819c9a0dd6c5a726e1592d22f6e9` coincidió con el digest de
> GitHub, y lint de CI/release no reportó errores. No sustituye el runner Windows Server 2025 ni la
> firma real; no se disparó un workflow ni se publicó nada. `gh run list` confirma que el CI más
> reciente sigue siendo el run fallido 34155852541 sobre `abdef402` del 2026-09-07; no hay una
> ejecución del checkout local actualizado.
> W-06B, refinamiento Windows 2026-10-20: todas las lecturas/escrituras del clipboard de la UI
> ahora se serializan fuera del hilo interfaz. Incluye `op_paste`, Copiar/Cortar, pegar en Cesta y
> copiar rutas/texto/CSV/procedencia; las listas de rutas se comparten con el worker sin otra copia.
> Gate: **1.500 aprobados, 8 ignorados**, Clippy, formato y check UI cruzado Linux verdes; pruebas
> focalizadas 2/2. Build Windows 0.5.3 bajo supervisor completado (Release 46m08s, Inno Setup
> 176,562 s); ZIP/setup verificados, sin firma, instalación ni publicación. ZIP SHA-256
> `1b8212b02d18e71eb07f6d33e24c92fc63344615e44955d2d11ecb21518b4453`, setup
> `998c30daa7422d61e4187603462b575be7641549f189361c3658fd5771333000`, EXE
> `868a69dec0f6f79d0e91e54326eb9762cd745921467dd45a72fd5a4000bd2b5a`. Naygo sigue abierto como
> PID 31304 con el build anterior. Evidencia: `docs/VALIDACION-W06B-PASTE-CLIPBOARD-2026-10-20.md`.
> W-07, observación pasiva 2026-10-19 23:51: sobre PID 50260 se tomaron tres ventanas de 30 s;
> 62,0 MiB working set, 43,2 MiB privados, 368 handles, 22 hilos y CPU 0,031/0,032/0,015 s por
> ventana. Un solo proceso, sin SMB. No se controló inactividad ni se midió input; la instalación
> es anterior al Release recién empaquetado. No cierra W-07. JSON y límites en `docs/BENCHMARKS.md`.
> W-13, reintento UIA pasivo 23:53: proceso PID 50260 responde y tiene ventana, pero el inspector no
> halló descendientes; la raíz UIA era un `Pane` sin primer hijo y `LogonUI` estaba activo en la misma
> sesión. No se puede atribuir la causa ni concluir regresión; repetir con sesión desbloqueada. No se
> enviaron acciones. Evidencia: `docs/VALIDACION-W13-INVOKE-2026-10-19.md`.
> W-07, muestra pasiva del release instalado 0.5.3 el 2026-10-20 00:53–00:54: PID 31304 se observó
> en tres ventanas de 30 s (16 muestras cada una): pico estable de 57,7 MiB working set, 26,9 MiB
> privados, 367 handles y 25 hilos; CPU delta 0,015/0,047/0,032 s. Un proceso, sin SMB. La sesión
> seguía bloqueada; no hubo interacción ni control de inactividad, y esto no cierra W-07. JSON:
> `target/agent-out/w07-installed-053-20261020-passive.json`; análisis en `docs/BENCHMARKS.md`.
> Hallazgo nuevo al revisar Papelera Windows, 2026-10-19: `windows_paths_equal` usaba texto lossless-to-lossy,
> por lo que dos rutas con unidades UTF-16 inválidas distintas podían coincidir y asociar el recibo
> al origen equivocado. La regresión reprodujo el fallo antes del cambio; ahora compara por
> `CompareStringOrdinal`, preserva la distinción de unidades y la equivalencia de caja/separadores.
> En el mismo alcance se corrigió el fallback de recibos reconstruidos, que ahora empareja caja y
> separadores del `DeletedFrom` de Shell y valida nombre/carpeta. La regresión cubre equivalencias y
> rechazos por destino erróneo. Gate completo: **1.483 aprobados, 8 ignorados**, Clippy, formato e
> i18n verdes; build y hashes definitivos constan arriba.
> Ver
> `docs/VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md`.
> Reconstrucción anterior W-13 0.5.3 (reemplazada por la actualización L-08 arriba, 2026-10-19):
> gate completo **1.480 tests aprobados,
> 8 ignorados**, Clippy y formato verdes. El build supervisado de W-13 terminó en Release 34m39s,
> Inno Setup 131,235 s, pico Job 10,65 GiB (límite 12 GiB); ZIP/setup validados e instalados con
> autorización previa, sin firma ni publicación. Naygo quedó abierto y responde como PID 44872; su
> EXE coincide con Release. UIA pasiva: 356 descendientes, 120/120 controles enfocables con nombre,
> 69 `Invoke`, cero botones sin `Invoke`, árbol completo. Narrador/diálogos siguen pendientes; W-13
> no se cierra. Evidencia y hashes: `docs/VALIDACION-W13-INVOKE-2026-10-19.md`. La prueba USB real,
> W-07, W-12 remoto y W-14 también siguen pendientes.
> Revisión remota de solo lectura, 2026-10-19: `gh run list` no muestra ejecuciones del checkout;
> la CI más reciente sigue en `abdef402` del 2026-09-07 y los cinco runs siguientes son revisiones
> de `Preview implementation review` del 2026-09-09. No se publicó ni disparó un workflow. SignPath
> oficial aún no documenta tags en `Allowed branch names`; W-14 mantiene esa pregunta pendiente.
> Validación local W-12/W-14 repetida el 2026-10-19: pins SHA y digest de acciones, pruebas de firma
> y actionlint v1.7.12 pasaron; esto no reemplaza el runner Windows Server 2025 ni el run real de firma.
> UIA pasiva, 2026-10-19, sobre el Release W-13 recién instalado PID 44872: 356 descendientes,
> 120/120 controles enfocables nombrados, 69 acciones `Invoke`, cero botones sin `Invoke`, árbol
> completo. Aún falta Narrador y recorrido de diálogos; no cierra W-13. Ver
> `docs/VALIDACION-W13-INVOKE-2026-10-19.md`.
> La revisión ampliada detectó además 19 botones enfocables sin patrón UIA `Invoke`; se corrigió
> `BasketIconButton`, y la inspección posterior mostró 74 acciones `Invoke` (antes 63) y 10 botones
> sin el patrón. La revisión estática encontró también `OpButton`/`MissingOpButton` sin acciones
> accesibles: ya se corrigieron con acción y activación por Enter/Espacio. Otra revisión agregó
> acciones y foco a los botones personalizados de maximizar/cerrar panel y selectores de categoría de
> ajustes; gate completo repetido (1.480 tests, 8 ignorados, Clippy/formato) y Graphify 9.527 nodos /
> 18.052 relaciones pasan. La distribución se reconstruyó e instaló; UIA pasiva de la ventana
> principal ahora reporta cero botones sin `Invoke`. Narrador y recorrido de diálogos siguen
> pendientes. Ver
> `docs/VALIDACION-W13-INVOKE-2026-10-19.md`.
> Reconstrucción local posterior 0.5.3, 2026-10-19: gate Windows **1.477 tests aprobados/8
> ignorados**, Clippy y formato verdes; L-06 `drives()` ahora integra `/proc/self/mountinfo` en el
> worker compartido, con check/Clippy cruzados Linux aprobados. Build supervisado Release 34m58s,
> Inno Setup 145,172 s, pico 10,65 GiB. ZIP/setup verificados, sin firma; no instalado ni publicado.
> Hashes nuevos en `docs/VALIDACION-0.5.3-2026-10-19.md`; PID 42728 permanece en la build W-06D
> previa. Aceptación nativa Ubuntu y W-07/W-13/W-14 continúan pendientes.
> L-07 Linux: backend inicial de Trash FreeDesktop y recibo persistible para Deshacer implementados;
> check completo del workspace y Clippy cruzados de los tres crates, además del gate Windows, pasan;
> runtime y pruebas nativas de filesystem
> aislado/permisos pendientes. La ruta con `symlink/..` resuelve su padre real y no sigue el enlace
> final; se añadieron tests cruzados de cancelación y directorios inseguros.
> Primer build detenido a 7,87 GiB libres; el reintento terminó (Release 42m14s, Inno 159,329 s,
> pico 10,65 GiB). `dist/` contiene artefactos 0.5.3 actualizados y verificados; sin instalación,
> publicación ni firma. Hashes en `docs/VALIDACION-L07-TRASH-2026-10-19.md`.
> Rebuild local 0.5.3, 2026-10-19: gate Windows verde (1.476 tests, 8 ignorados, Clippy y
> formato). W-06D se incluyó en la distribución supervisada: Release 37m32s, Inno 142,110 s, pico
> 10,65 GiB (límite 12 GiB). ZIP/setup verificados; hashes y evidencia en
> `docs/VALIDACION-0.5.3-2026-10-19.md`. Instalado y abierto/respondiendo como PID 42728; EXE
> coincide con Release. Los cuatro JSON y el PDB previo conservaron sus hashes. UIA pasiva:
> 821 descendientes, 654 nombrados, 190/190 controles enfocables con nombre, árbol completo. Esto
> no reemplaza Narrador ni aceptación manual, ni una prueba de expulsión USB real. Authenticode
> (W-14) sigue pendiente; W-07 aún requiere interacción, medición de presentación y matriz local/UNC.
> Detalle en `docs/VALIDACION-0.5.3-2026-10-19.md`.
> L-06, integración de montajes Linux 2026-10-19: `drives()` lee `/proc/self/mountinfo` desde el
> worker existente, omite pseudofs, incluye puntos de montajes de red conocidos, deduplica montajes
> apilados y conserva rutas en bytes POSIX; unidades no clasificadas quedan como Unknown y no se
> pueden expulsar. Gate Windows: 1.477 tests, 8 ignorados, Clippy y formato verdes; check y Clippy
> cruzados Linux de platform pasan. Aún faltan runtime Ubuntu, mounts reales/red, clasificación de
> removibles y watchers; no se afirma soporte Linux nativo. Evidencia en
> `docs/VALIDACION-L06-MOUNTINFO-2026-10-19.md`.
> W-06D aprobado, implementado, validado, empaquetado e instalado el 2026-10-19: enumeración
> Win32/USB, actualización de raíces del árbol preservando las expansiones y revalidación/expulsión
> corren en workers. Gate completo: 1.476 tests, 8 ignorados, Clippy `-D warnings` y formato verdes.
> La prueba pasiva de UIA del proceso instalado encontró 190/190 controles enfocables nombrados.
> Una expulsión de dispositivo físico sigue pendiente.
> W-06E: guardar/borrar temas de usuario usa el worker de packs; el guardado escribe de forma
> atómica. Cuatro tests focalizados y gate completo (1.480 aprobados, 8 ignorados, Clippy y formato)
> pasan. Release 0.5.3 regenerado bajo supervisor, ZIP/setup verificados; no instalado/publicado.
> Evidencia en `docs/VALIDACION-W06E-2026-10-19.md`.
> Revalidación Linux posterior a W-06D: check de workspace/tests y Clippy `-D warnings` de los tres
> crates cruzados desde Windows pasaron (Clippy por paquetes; un intento agregado se atascó y se
> canceló). No demuestra ejecución nativa Linux; WSL sigue sin arrancar.
> L-00, revalidación 2026-10-19: ficha y `copyright` oficial de Ubuntu Resolute 26.04 para
> `libayatana-appindicator-glib2` 2.0.1-1build1 coinciden con el aviso archivado (SHA-256 verificado).
> GPL-3 del runtime y dependencia GLib confirmadas; fuente correspondiente para distribución y
> validación nativa Ubuntu siguen pendientes. WSL repitió `HCS_E_HYPERV_NOT_INSTALLED`.
> Repetición W-07 2026-10-19 con el harness release en fixtures locales de 100.000 archivos:
> medianas externas 241,865 / 210,594 / 232,889 ms; la variación no acredita ganancia ni regresión.
> Cancelación terminal observada y fixtures limpiadas. Logs y límites en `docs/BENCHMARKS.md`.
> Muestra pasiva del release 0.5.3 instalado con W-06D, 2026-10-19 10:09: working set pico
> 78,4–78,7 MiB, privado 46,6–46,8 MiB, 385 handles, 26 hilos, CPU delta 0,81–0,98 s por
> ventana de 30 s; un proceso y sin SMB. No se controló inactividad/entrada, por lo que no es
> baseline de reposo ni evidencia de latencia. Ver `docs/BENCHMARKS.md` y el JSON en `target/agent-out/`.
> Muestra W-07 adicional, 2026-10-19 10:32–10:34: 46 observaciones en 101,2 s, sin entrada física
> observada (`GetLastInputInfo` 302,9→404,1 s), Firefox en primer plano. Naygo en segundo plano
> acumuló 0,172 s CPU, 78,8 MiB working set máximo, 46,8 MiB privados, 381 handles y 24 hilos
> promedio. La carga global fue 16–67 %; no es reposo del equipo ni baseline de latencia. Ver
> `docs/BENCHMARKS.md` y el JSON de `target/agent-out/`.
> Repetición W-07 sobre la misma instancia 0.5.3, 2026-10-19 00:38–00:40: 76,9 MiB working set,
> 45,3 MiB privados, 385 handles, 26 hilos y CPU delta 0/0/0,063 s en 3×30 s; un proceso y sin
> SMB. Pasiva, sin control de inactividad, input ni presentación; no cierra W-07. WSL Ubuntu sigue
> registrado pero no arranca (`HYPERV_NOT_INSTALLED`). JSON y límites en `docs/BENCHMARKS.md`.
> Repetición W-07 local 2026-10-19: mediana externa 208,813 ms para enumerar 100.000 entradas
> (tres corridas, mismo host). Frente a 161,659 ms del 2026-10-15 se observa +29,2 %, pero no se
> aisló causa ni se controlaron los fingerprints/versiones; una microoptimización de cancelación
> no cambió la mediana y se revirtió. Fixtures limpiadas. Ver `docs/BENCHMARKS.md`; W-07 sigue abierto.
> Linux L-06, avance parcial 2026-10-19: el footer consulta con `statvfs` el volumen que contiene la
> carpeta abierta. Worker asíncrono, caché limitada a 128 carpetas y máximo de ocho consultas a la
> vez. Tests Windows 1.467/1.467; check y Clippy cruzados Linux pasan. No hay ejecución nativa porque
> WSL no arranca; enumeración/`mountinfo`, devices/watchers, invalidación y matriz Ubuntu siguen
> pendientes.
> W-07, ajuste local 2026-10-19: el worker reutiliza la única ruta materializada para filtrar
> temporales internos y construir la fila. Tests dirigidos y gate completo pasan. La medición de
> disco varió ampliamente y no demuestra una mejora de latencia; ver `docs/BENCHMARKS.md`. La
> distribución 0.5.3 ya se regeneró con el código final; no se instaló ni firmó.
> W-13, reinspección de Release actual 2026-10-18 21:33: instalación 0.5.2 autorizada; setup
> validado por SHA-256, código 0. El EXE instalado coincide con Release y se dejó abierto, PID
> 51452; cuatro JSON siguen parseando y el PDB se conservó. UIA: 350 descendientes, 88/88 controles
> enfocables con nombre, árbol completo. Falta Narrador y recorrer diálogos/acciones. Evidencia:
> `docs/VALIDACION-UIA-2026-10-18.md`.
> W-13, avance local 2026-10-18: «Subir carpeta», carpetas recientes, filas de tipos en el filtro,
> menús de contexto/filtro y sus acciones ahora tienen roles, nombres y callbacks accesibles. Gate
> completo: 1.464 pruebas aprobadas, 8 ignoradas, Clippy y formato correctos; build supervisado pasó
> (pico 10,64 GiB), ZIP/setup regenerados y verificados. Se actualizó Naygo instalado y quedó abierto
> PID 50496; UIA encuentra 88/88 controles enfocables nombrados, sin truncar. Narrador y recorrido
> manual de menús/filtros/diálogos siguen pendientes. Evidencia en
> `docs/VALIDACION-W13-CONTROLES-2026-10-18.md`.
> W-07, muestra pasiva sobre el Release instalado 2026-10-18 21:35–21:37: 60,6–60,7 MiB working
> set; 29,2–29,3 MiB privados; 387–392 handles; 28–29 hilos; CPU 0,06/0,09/0,05 s en tres
> ventanas. Un proceso, sin SMB. No hubo input ni control de inactividad; no cierra la latencia,
> presentación ni matriz de filesystem. JSON y límites en `docs/BENCHMARKS.md`.
> W-07, observación pasiva adicional 2026-10-18 21:28–21:29: el PID 48792 mantuvo 93,9 MiB
> working set, 53,6 MiB privados, 481 handles y 40 hilos; CPU delta 0,078/0/0,016 s. Se observó
> un solo proceso y ningún mapeo SMB. El EXE instalado no coincide con Release actual; no se
> controló inactividad ni hubo interacción, por lo que no cierra W-07. Registro y límites en
> `docs/BENCHMARKS.md` y `target/agent-out/w07-current-passive-20261018.json`.
> W-07, alcance de instrumentación: Slint 1.16.1 notifica `AfterRendering` antes de presentar;
> el renderer software no admite `set_rendering_notifier` y Winit-software presenta internamente.
> No se agregará una métrica que se interprete como latencia de presentación; esa medición requiere
> instrumentación externa del backend o del sistema. Detalle y fuentes en `docs/BENCHMARKS.md`.
> W-13, endurecimiento local 2026-10-19: el callback nativo de doble clic ahora usa Ctrl/Shift
> capturados del mouse-up del gesto, no el estado de teclado que puede quedar viejo si un modal
> pierde key-release. Regresión aprobada; gate completo: 1.465 tests, 8 ignorados, Clippy limpio,
> formato aprobado. Se regeneró la distribución 0.5.3: pico de 10,64 GiB, Release 27m49s, Inno
> 117,125s; ZIP/setup/EXE verificados. No se instaló: PID 48444 sigue ejecutando el binario previo
> de 0.5.3. Sigue pendiente aceptación manual W-13. Hashes actuales en `docs/VALIDACION-0.5.3-2026-10-19.md`.
> W-13, mejora local 2026-10-18: además del preview y el campo compartido crear/renombrar/comprimir,
> los campos editables de configuración, búsqueda, favoritos, recetas y consultas guardadas exponen
> nombres accesibles explícitos. Gate: 1.455 pruebas (8 ignoradas), Clippy y formato aprobados. Build
> supervisado actualizado: pico 10,53 GiB, release 28m32s e Inno Setup 128,218 s; ZIP/setup/EXE y
> hashes verificados. Se instaló con autorización previa y quedó abierto PID 23868; settings/workspace/
> favorites/templates y PDB previo preservados. UIA pasiva: 330 descendientes, 239 nombrados, 46/46
> enfocables nombrados; sesión bloqueada y diálogos cerrados limitan la evidencia. Falta Narrador.
> Evidencia: `docs/VALIDACION-UIA-2026-10-18.md`.
> Corrección 2026-10-18 (modificadores pegados): crear carpeta ahora limpia Ctrl/Shift al abrir y
> cerrar/aplicar el modal; cada clic de fila sincroniza esos estados desde el evento real antes de
> resolver un doble clic. Así Ctrl+Enter o un key-release tragado por el editor no hace que el
> siguiente doble clic normal abra una carpeta en otro panel. Gate completo: 1.460 tests aprobados,
> 8 ignorados, Clippy sin avisos y formato correcto. Build supervisado: pico 10,64 GiB, release
> 29m44s, Inno Setup 125,250 s. Portable/setup y EXE pasan hashes; el setup sigue sin firma. Se
> instaló y quedó abierto Naygo PID 48792; el EXE instalado coincide con Release. Evidencia y límites:
> `docs/VALIDACION-MODIFICADORES-2026-10-18.md`.
> W-13, avance local 2026-10-18: listas de archivos ahora anuncian filas seleccionables con nombre,
> tipo, estado e índice; la fila `..` informa «Subir» y la acción accesible reusa la activación normal.
> El Inspector publica acciones para calcular tamaño, copiar y quitar la marca de procedencia, con
> nombres localizados y los mismos callbacks que los controles visuales. Gate: 1.460 tests aprobados,
> 8 ignorados, Clippy y formato correctos. Build supervisado: pico 10,64 GiB, release 30m58s e Inno
> Setup 168,719 s; ZIP/setup/EXE y manifiesto verificados, sin instalar. Falta reinspección UIA/
> Narrador de la distribución nueva en sesión desbloqueada. Evidencia: `docs/VALIDACION-W13-CONTROLES-2026-10-18.md`.
> L-02, importación de configuración entre plataformas (avance local 2026-10-18): al importar
> `.naygoconf`, se omiten sin reescritura las rutas de inicio/recientes incompatibles; en Linux
> también se desactivan autoinicio y atajo global, aún no implementados. Un aviso localizado enumera
> los campos omitidos; keybindings y rutas nativas válidas se conservan. 4 pruebas dirigidas, gate
> completo (1.464/8), Clippy, formato, paridad i18n y check/Clippy cruzados de UI Linux pasan. ZIP/
> setup regenerados bajo supervisor (pico 10,63 GiB), hashes verificados, sin firma ni instalación.
> Falta prueba nativa Linux. El formato actual no incluye preferencia de terminal. Evidencia:
> `docs/VALIDACION-L02-IMPORT-CONFIG-2026-10-18.md`.
> Reinspección W-13 posterior, 2026-10-18: el escritorio ya estaba desbloqueado. UIA pasiva sobre
> PID 48792 halló 759 descendientes, 80 controles enfocables y los 80 con nombre; no truncó el árbol.
> Ese proceso tiene EXE anterior al release actual y la captura visual agotó el tiempo. Evidencia
> parcial solamente; no se abrió diálogo ni se envió entrada. Narrador/aceptación de la build actual
> sigue pendiente; ver `docs/VALIDACION-UIA-2026-10-18.md`.

> Cierre L-05 (subpaso xdg-open) del 2026-10-18: gate completo con 1.455 tests aprobados y 8
> ignorados, Clippy/formato y build supervisado aprobados (pico 10,53 GiB). Portable/setup 0.5.2
> regenerados; el EXE interno coincide con Release y el ZIP no incluye PDB. Setup unsigned, sin
> instalar. Evidencia: `docs/VALIDACION-L05-OPEN-2026-10-18.md`.
> Cierre adicional 2026-10-18: L-05 ahora oculta la capacidad de elevar ejecutables fuera de Windows;
> el gate pasó 1.453 pruebas (8 ignoradas), Clippy y formato. La UI completa pasó check/Clippy
> cruzados Linux desde Windows. El build supervisado regeneró ZIP/setup de 0.5.2 con pico 10,52 GiB;
> ambos hashes coinciden con `dist/SHA256SUMS.txt`, pero el setup sigue sin Authenticode y no se
> instaló. Evidencia: `docs/VALIDACION-L05-ADMIN-2026-10-18.md`.
> Hallazgo W-13 del 2026-10-18: la instancia abierta devolvió por UIA un control enfocable `Edit`
> sin nombre accesible. Su EXE no coincide con el release actual y la sesión está bloqueada; el
> control aún no se atribuye a un editor concreto. Se amplió el inspector para contar tipos
> anónimos sin registrar texto ni rutas. Ver `docs/VALIDACION-UIA-2026-10-18.md`.
> Hallazgo W-13 adicional, 2026-10-18: revisión estática detectó que los botones dibujados a mano
> de la toolbar y la tira de unidades eran clickeables pero no declaraban rol, nombre ni acción para
> UIA/AccessKit. Se añadieron esos metadatos a los botones y la acción de expulsar quedó anunciada
> separadamente. El gate pasó con 1.458 pruebas (8 ignoradas), Clippy y formato; build supervisado
> produjo ZIP/setup verificados (pico 10,54 GiB, release 26m05s, Inno 111,110 s), sin instalar.
> Aceptación con Narrador y UIA real quedan en W-13. Evidencia: `docs/VALIDACION-W13-TOOLBAR-2026-10-18.md`.
> Avance W-13 del 2026-10-18: se añadió nombre accesible localizado al editor de texto de solo
> lectura del preview, sin exponer contenido o rutas. Gate y distribución pasan; es un control
> plausible para el `Edit` anónimo, pero la build observada era anterior y falta confirmar con UIA/
> Narrador en sesión desbloqueada. Alcance ampliado con autorización del usuario: revisar también
> controles clickeables fuera de la toolbar (subir carpeta, acciones de Inspección y filas) y agregar
> regresiones acotadas cuando su rol/acción lo permita. Sigue pendiente la auditoría UIA/Narrador
> interactiva en una sesión desbloqueada.
> Avance L-05 del 2026-10-18: Linux ahora delega Enter/doble clic en `xdg-open` con argv directo,
> ruta absoluta y bytes nativos preservados. Pruebas de argv Windows y check/Clippy cruzados Linux
> pasan; gate completo Windows y build supervisado también pasan. Faltan runtime XDG/portal nativo,
> «Abrir con» y terminal segura. Evidencia: `docs/VALIDACION-L05-OPEN-2026-10-18.md`.
> Actualización local 2026-10-18: la UI completa ya pasa check/Clippy cruzados Linux con Fontconfig
> dinámico; Windows pasó 1.452 tests (8 ignorados), Clippy y formato. Portable/setup se regeneraron
> bajo supervisor (pico 10,53 GiB), hashes verificados, sin firma ni instalación. L-03 sigue abierto
> para runtime/escritorio nativos. Evidencia y hashes en `docs/VALIDACION-L03-CROSS-2026-10-17.md`.
> Última actualización: 2026-10-19. W-01–W-06D, M-01, W-08, W-09, W-10A–C, W-11A–C y la
> implementación local de W-12 están
> implementados, probados y empaquetados. La recuperación automática del Replace está implementada
> localmente, con gate y distribución regenerada (no instalada). La configuración local de W-12 pasa validaciones; falta
> el run remoto. W-07 tiene mediciones sintéticas, filesystem local y rasterizado headless en debug
> y release; siguen pendientes interacción real, presentación Winit y matriz local/UNC.
> El crash E-2 al iniciar drops desde aplicaciones como 7-Zip está corregido, probado y empaquetado;
> se preparó una fixture aislada y queda la reproducción manual de aceptación.
> E-3 (ruta en menú de Favoritos) quedó implementado, probado, empaquetado e instalado el 2026-10-17.
> E-5 actualizó la toolbar, pasó 1.450 pruebas (8 ignoradas), Clippy sin avisos y formato; el
> empaquetado supervisado alcanzó 10,49 GiB, release 33m07s e Inno 114,843s. ZIP/setup/EXE y hashes
> están verificados; la versión sigue sin firma Authenticode. Se instaló 0.5.2 y se dejó abierta
> (PID 51408); EXE instalado y release comparten SHA-256 `681b99ae…`. E-4: Favoritos por espacio implementados en JSON v2, migración v1 global,
> elección global/propia al guardar como y copia global confirmada; falta aceptación visual. Evidencia
> E-4 añadió el selector de recientes persistente; su gate pasó 1.452 pruebas (8 ignoradas), Clippy,
> formato y build supervisado. El setup se instaló in-place y dejó Naygo abierto, PID 25124; EXE
> instalado coincide con Release y settings/workspace/favoritos/plantillas/PDB conservaron hashes.
> UIA vio 726 descendientes, 539 con nombre y 55 enfocables; la inspección visual/manual y rutas
> remotas siguen pendientes. Evidencia: `docs/VALIDACION-E4-CATALOGO-2026-10-18.md`.
> Actualización local posterior: el fix de errores individuales `ReadDir` para Copy/Move pasó el gate
> (1.453 aprobadas, 8 ignoradas), Clippy, formato y checks/Clippy cruzados Linux de `core`/`platform`.
> Portable/setup regenerados y con SHA-256 verificado; sin instalar y sin Authenticode. Evidencia:
> `docs/VALIDACION-READDIR-2026-10-18.md`.
> Nueva muestra W-07 del PID actual 25124: tres ventanas de 30 s con el escritorio bloqueado,
> 78,5 MiB working set, 47,0 MiB privados, CPU 0,015/0/0,047 s y sin procesos auxiliares ni SMB.
> Mejora la evidencia de reposo, pero faltan interacción, presentación Winit y matriz local/UNC.
> JSON y límites en `docs/BENCHMARKS.md`.
> E-4 Favoritos por espacio implementados en JSON v2, migración v1 global,
> elección global/propia al guardar como y copia global confirmada; falta aceptación visual. Evidencia
> de esa entrega: `docs/VALIDACION-E4-FAVORITOS-2026-10-18.md`. E-3 recibió previamente su aceptación
> instalada. W-07 y W-13 mantienen sus verificaciones manuales pendientes. UIA sobre E-5 encontró
> 721 descendientes, 536 con nombre y 55 enfocables (todos con nombre); la muestra pasiva de tres
> ventanas de 30 s dio 76,7–77,9 MiB de working set y CPU delta 0,032/0,359/0 s. No se controló
> inactividad ni se midió input; no cierra W-07/W-13. Evidencia y límites en `docs/BENCHMARKS.md`.
> Consulta remota de solo lectura del 2026-10-18: sigue igual; el run más reciente es `Preview
> implementation review` del 9 de septiembre y el último `CI` visible es del 7 de septiembre. No hay
> run para los cambios actuales. Ejecutar CI sobre ellos requiere publicar la rama, lo que queda
> fuera del alcance autorizado.
> Cobertura L-01B añadida el 2026-10-18: regresión Unix asegura que redirigir por symlink una carpeta
> destino invalida la revisión de reintento. Pasaron 3 tests de reintento ejecutables en Windows y
> `cargo check` de `naygo-core --tests` cruzado a Linux; la regresión aún requiere ejecución nativa.
> Fix L-01B Windows/Linux del 2026-10-18: «Renombrar antiguo» ahora aparta el destino con el
> adaptador atómico sin reemplazo; un nombre de respaldo ocupado durante la carrera conserva ambos
> archivos y falla el paso. La prueba dirigida pasa en Windows; gate completo 1.456/8, Clippy y
> formato pasan. La distribución Windows se regeneró bajo supervisor (pico 10,53 GiB), hashes
> verificados y setup sin firma; no se instaló. Falta ejecución nativa Linux. Evidencia:
> `docs/VALIDACION-L01B-RENAME-2026-10-18.md`.
> Linux L-00 tiene resuelta la ruta GLib/GPL-3 Linux-only y el mínimo de Ubuntu 26.04 LTS x86_64.
> Revisión del source package Ayatana GLib 2.0.1-1build1 (resolute): `debian/copyright` confirma
> GPL-3 para la biblioteca y CC0-1.0 para una plantilla; coincide con la excepción Linux-only ya
> aprobada. El 18 de octubre se verificaron la firma OpenPGP del `.dsc`, la firma detached del tar
> upstream, y la cadena oficial Ubuntu `InRelease → Packages → .deb`. Ubuntu autentica el hash del
> `.deb` desde metadatos firmados; no firma cada `.deb` individualmente. PGPy detecta la firma pero
> no valida autosignaturas/revocaciones, y las identidades se contrastaron con Launchpad, Debian y
> Ubuntu. Evidencia: `docs/VALIDACION-L00-AUDITORIA-2026-10-17.md`. Siguen pendientes fuente
> correspondiente, notices finales y validar el runtime en Ubuntu nativo.
> L-03 ahora tiene un binding GLib/AppIndicator local con carga dinámica y worker; el 18 de octubre
> `core`/`platform` y la UI completa pasaron check y Clippy de todos sus targets para Linux desde
> Windows. La UI habilita `fontique-dlopen` solo en Linux. El binding pasó typecheck/Clippy aislados.
> El grafo Cargo Linux actual tiene 500 paquetes sin metadatos de licencia desconocidos y ya no
> incluye `tray-icon`; la auditoría de bibliotecas externas sigue abierta. Faltan
> build nativo, ejecución en GNOME/Wayland y derivado, y validación de notices/runtime.
> Hallazgo nuevo L-05, 2026-10-18: `can_run_as_administrator()` aceptaba `.exe`/`.com` sin filtrar
> plataforma, así la UI Linux ofrecía una acción que terminaba en `NotSupported`. Corregido para
> dejar la capacidad disponible sólo en Windows; Linux no sustituye elevación por `sudo`. Test
> específico añadido y compila al cruzar tests a Linux; la ejecución nativa sigue pendiente.
> El 17 de octubre el gate Windows volvió a pasar (1.447 pruebas, 8 ignoradas; Clippy y formato
> limpios) tras corregir el estado inicial del tray. `dist/` se regeneró bajo supervisor (pico
> 10,50 GiB); ZIP/setup/EXE concuerdan y siguen sin firma. No se instalaron los artefactos nuevos;
> la instancia previa PID 56616 continúa abierta y responde. Evidencia:
> `docs/VALIDACION-L03-CROSS-2026-10-17.md`.
> Revisión L-03 del 18 de octubre: se corrigió una doble retirada del source GLib al cerrar por HUP,
> EOF o error; el owner mantiene el source hasta salir del loop y lo quita una sola vez. Las firmas
> AppIndicator y el namespace de acciones coinciden con el header/ejemplo upstream. El harness Linux
> y la UI completa pasaron check/Clippy cruzados. Windows pasó 1.452 pruebas (8 ignoradas), Clippy
> y formato; portable/setup 0.5.2 se regeneraron bajo supervisor (pico 10,53 GiB), con hashes
> verificados, sin firma ni instalación. La ejecución nativa sigue pendiente; ver el informe L-03.
> Auditoría FFI del 2026-10-18 contra headers AppIndicator 2.0.1 y GLib/Gio 2.88: los tipos Rust
> de `GIOCondition`/`GConnectFlags` se ajustaron a `c_int` y el callback se tipó como
> `GUnixFDSourceFunc`. Pasaron el check y Clippy cruzado de todos los targets Linux y el gate
> Windows (1.464 pruebas, 8 ignoradas, Clippy y formato limpios); distribución regenerada bajo
> supervisor (pico 10,64 GiB), hashes ZIP/setup verificados, ZIP íntegro y EXE interno coincidente;
> setup/EXE siguen sin firma. No se instaló. El ABI/runtime aún requiere Ubuntu 26.04 nativo.
> Revalidación L-00 del 16 de octubre: Ayatana GLib declara exportar `org.gtk.Menus`/`org.gtk.Actions`
> en vez del `com.canonical.dbusmenu` que exige la propiedad `Menu` de SNI. Esto no prueba por sí
> solo que GNOME/Wayland rechace el menú; deja la compatibilidad sin demostrar. Se recomendó a
> Nicolás hacer la prueba nativa de menú una puerta de L-03.
> Hallazgo L-00/L-03 adicional, 2026-10-17: el issue upstream #87 sigue abierto y reporta ícono/menú
> inutilizables en Waybar, IceWM/LXDE y Plasma 6. Es un reporte individual, no una reproducción; refuerza
> probar ícono, menú y acciones en Ubuntu 26.04 GNOME/Wayland y un derivado antes de aceptar el tray.
> Decisión de Nicolás del 2026-10-17: mantener GLib y probarla; el comportamiento de icono, menú y
> acciones en esos entornos queda como aceptación L-03, sin cambiar aún el backend.
> L-00, revisión de autoría (2026-10-17): Nicolás confirmó que `nicolas_groth@yahoo.com` era su
> alias histórico y que los 948 commits asociados corresponden a sus aportes. Esto resuelve la
> discrepancia de identidad del historial; no sustituye la revisión de código importado ni la
> preparación de fuente correspondiente y notices antes de una distribución Linux GPL-3. Se añadió
> `.mailmap` para mostrar ese alias como `Nicolás Groth <ngroth@gmail.com>` sin reescribir commits;
> `git check-mailmap` confirma el mapeo. El barrido
> actual encontró SPDX en los 272 archivos `.rs`, `.slint`, `.c` y `.h`.
> Revisión de autoría/licencias del 2026-10-17: `git log --all` muestra un único autor/committer
> tras aplicar `.mailmap` (Nicolás); los 275 archivos de código `.rs`, `.slint`, `.c`, `.h`, `.cpp`
> y `.cc` presentes tienen SPDX, y el barrido no encontró rutas `vendor`, `third_party` o `external`
> en los archivos versionados. El grafo Cargo Linux x86_64 tiene 543 paquetes: Slint conserva su
> opción royalty-free ya elegida y aparece `option-ext` MPL-2.0 como dependencia transitiva de
> `tray-icon → dirs → dirs-sys`; esta combinación exige conservar sus avisos/fuentes MPL si se
> distribuye ese código, y debe revalidarse después de migrar tray. El grafo no sustituye el
> inventario de bibliotecas del sistema ni define el paquete final. Evidencia y límites en
> `docs/VALIDACION-L00-AUDITORIA-2026-10-17.md`.
> Hallazgo L-11/Windows del 2026-10-17: «cerrar a bandeja» podía dejar el proceso oculto cuando
> fallaban el tray y el hotkey (en Linux este último es stub). Corregido para exigir una vía de
> retorno real antes de ocultar; guía, changelog y test unitario actualizados. Falta gate/build de
> distribución y validar la bandeja Linux en entorno nativo.
> El grafo actual de dependencias confirma que `tray-icon` trae bindings GTK/GObject y el check Linux
> requiere `pkg-config`/sysroot aunque cargue AppIndicator por soname; esto se precisará en L-00.
> El 2026-10-16 se revalidó el check/Clippy cruzados de core y platform; UI Linux falla en `glib-sys`
> por no tener `pkg-config`/sysroot. WSL devuelve `HYPERV_NOT_INSTALLED` y `Win32_ComputerSystem`
> informa `HypervisorPresent=False` (CPU expone virtualización/SVAT). Habilitar la característica
> requiere elevación; no se modificó Windows. La UI Linux no puede comprobarse
> sin entorno Linux. El 16 de octubre se conectó
> parcialmente L-02: arranque/config XDG y logs en XDG state; Windows conserva modo portable.
> Check cruzado de core/platform y Clippy Linux pasan; check de UI y compilación nativa siguen
> pendientes. El build previo al cierre W-10C pasó 1.400 tests/7 ignorados; su ZIP anterior arrancó
> y cerró con `--shutdown` y tuvo una captura pasiva opt-in, sin interacción. W-10C luego pasó el
> gate actualizado (1.402 aprobados, 8 ignorados, Clippy limpio; pico de gate 9,07 GiB) y regeneró
> la distribución con pico 10,42 GiB. El 16 de octubre se actualizó la instalación existente con
> setup 0.5.2 W-10C y se dejó la app corriendo; ese EXE fue reemplazado después por una primera build
> AccessKit y finalmente por W-13 con roles de pestañas, cuyo hash y estado están registrados abajo.
> Los ZIP no se ejecutaron en paralelo porque la app instalada debe permanecer abierta. W-07, W-12 remoto,
> W-13 manual y W-14 SignPath siguen abiertos. W-15 quedó validado localmente el 16 de octubre en
> setup real y sandbox aislado. La revisión UI Automation encontró que la ventana
> no exponía controles; se habilitó `accessibility` de Slint y se añadió un inspector de árbol para
> verificarlo en la instalación actualizada. Esto no cierra Narrador/DPI/teclado manual. Evidencia en
> `docs/VALIDACION-BUILD-2026-10-16.md`.
> Tras el build se regeneró `THIRD-PARTY-NOTICES.md` de 387 a 404 paquetes, incluyendo AccessKit;
> el ZIP/setup se reempaquetó y la instalación se actualizó con ese inventario. Hashes finales en
> `docs/VALIDACION-BUILD-2026-10-16.md`.
> W-16 (iconos Shell opcionales en Windows) pasó gate el 17 de octubre: 1.438 tests aprobados,
> 8 ignorados, Clippy sin avisos y formato correcto. Distribución 0.5.2 regenerada e instalada;
> Naygo quedó abierto y responde. Se preservaron settings, workspace, favoritos, plantillas y PDB.
> Hashes y alcance de la verificación en `docs/VALIDACION-W16-2026-10-17.md`.
> La captura pasiva de la primera instalación AccessKit (3×30 s; 60,8 MiB working set y
> 29,3–29,4 MiB privados) quedó registrada como evidencia parcial; no confirma inactividad ni
> interacción y no cierra W-07.
> Hallazgo local W-13: la barra de pestañas personalizada no se exponía como `tab-list` ni permitía
> recorrer pestañas por teclado. Se implementaron roles/selección/acciones AccessKit y navegación
> ←/→ + Enter/Espacio; una prueba headless cubre foco, activación y límite del grupo. El gate completo
> pasó con pico 5,97 GiB y la distribución 0.5.2 se regeneró bajo supervisor (pico 10,45 GiB).
> ZIP/setup actualizados; hashes y logs en `docs/VALIDACION-BUILD-2026-10-16.md`. Narrador/teclado
> manual sigue pendiente. La nueva build se instaló y quedó abierta/respondiendo; los cuatro JSON
> de configuración/sesión coinciden byte a byte con el respaldo previo.
> Un probe temporal de Windows con el `TabBar` de producción confirmó UIA `Tab=1`, `TabItem=3`, tres
> elementos enfocables y selección `true/false/false`; esto no reemplaza Narrador ni la aceptación
> manual de la aplicación completa.
> El ensayo adicional de clic/teclado en el probe quedó inconcluso: UIA observó el clic en Panel dos,
> pero Enter no produjo una selección legible por la aserción. La prueba dirigida headless pasó 1/1;
> W-13 mantiene pendiente la aceptación interactiva con Narrador y en la aplicación.
> La repetición con estado textual no envió teclas porque Windows no dejó activar el probe en primer
> plano; se conservó la guarda de seguridad. UIA confirmó selección inicial Panel uno. No prueba un
> defecto de la barra.
> Reinspección de solo lectura de la instalación vigente, PID 41724 y EXE SHA-256 `bfb03176…`:
> 731 descendientes UIA, 541 con nombre y 55 enfocables; proceso receptivo, sin grupos apilados de
> pestañas en la sesión. Confirma el árbol general, no la barra de pestañas ni Narrador. Evidencia en
> `docs/VALIDACION-BUILD-2026-10-16.md`.
> L-01A: el core conserva el `PathBuf` operativo y muestra bytes POSIX no UTF-8 con escapes `\\xNN`
> en listado, árbol y búsqueda. Windows pasó 909 tests core y 22 flujos; check y Clippy cruzados
> compilaron tests Linux. El gate de workspace pasó 1.404 tests (8 ignorados), Clippy y formato;
> distribución 0.5.2 regenerada con pico release 10,45 GiB. Después, con autorización previa, se
> instaló in-place con código 0 y se dejó abierta la nueva versión (EXE `bfb031…`). Cuatro JSON y el
> PDB antiguo conservaron sus hashes pre/post. Una muestra pasiva del PID nuevo registró 62,8 MiB
> working set, 31,1 MiB privados, 394 handles, 27 hilos y 0,359/0,110/0,047 s CPU; no cierra W-07.
> Hashes y límites en `docs/VALIDACION-BUILD-2026-10-16.md`. Linux nativo aún pendiente.
> El portable actual pasó smoke aislado de 10 s con `NAYGO_PERF_METRICS=1`, respondió y cerró con
> `--shutdown`; escribió 16 resúmenes internos (13 spans y tres contadores de workers). Sin input
> real ni medición Winit, no cierra W-07. Los cuatro JSON y el PDB instalado siguieron intactos, y
> el release se volvió a abrir y responde como PID 41724. Evidencia detallada en `docs/BENCHMARKS.md`.
> El intento de observar el probe UIA aislado agotó el tiempo de captura dos veces; no se enviaron
> teclas y W-13 manual sigue pendiente.
> Captura pasiva 3×30 s sobre esa build: 60,5–60,6 MiB de working set, 29,2 MiB privados, 385–387
> handles, 26–27 hilos y 0–0,05 s CPU; sin SMB. Evidencia limitada, no cierra W-07.
> Repetición pasiva 3×30 s sobre el proceso instalado (13:07–13:09): un proceso, 60,9 MiB working
> set, 29,2 MiB privados, 385 handles, 24 hilos y 0,047–0,141 s CPU por ventana; sin SMB. Sigue
> siendo observación pasiva, no cierra W-07. JSON en `docs/BENCHMARKS.md`.
> Repetición más reciente sobre PID 41724 (14:43–14:44): 16 muestras por ventana, proceso único,
> cero SMB, 78,7 MiB working set, 47,7 MiB privados, 390 handles, 26 hilos y CPU 0,125/0,093/0,032 s.
> Respondía al terminar; no se controló inactividad y no cierra W-07. JSON en `docs/BENCHMARKS.md`.
> Portable aislado con 10.000 archivos sintéticos: instrumentó 256 conversiones de filas y 249 ticks;
> el pico pasivo fue 82,1 MiB/51,4 MiB privados, 421 handles, 22 hilos y 0,172 s CPU en 12 s.
> La captura UI agotó el tiempo dos veces, sin entradas ni muestras `ui.input.*`; W-07 permanece abierto.
> Detalle y límites en `docs/BENCHMARKS.md`.
> Repetición con portable recién extraído y fixture limpia de 100 archivos: UIA vio 25 filas y confirmó
> la ruta CLI sintética. Tab/Abajo no movieron el foco desde la región raíz ni generaron spans input;
> UIA no pudo hacer clic por falta de geometría y la captura gráfica agotó el tiempo. En un segundo
> arranque, otro argumento CLI reemplazó la sesión sintética guardada (25 filas del segundo fixture,
> cero del anterior). No hubo spans `ui.input.*`; se restauró el release instalado, PID 18080. W-07 sigue abierto.
> Tras corregir el crash de drop y actualizar la instalación autorizada, captura pasiva 3×30 s del
> PID 23400: 83,7–83,8 MiB working set, 52,7 MiB privados, 370 handles, 24 hilos y
> 0,063/0,015/0,047 s CPU; un proceso, cero SMB. No se controló reposo ni se midió interacción;
> no cierra W-07. JSON y limitaciones en `docs/BENCHMARKS.md`.
> Actualización E-1 posterior: con autorización previa se instaló el setup local; código 0, EXE instalado
> igual al release (SHA-256 `97bd204b…`), PID 12232 iniciado y receptivo. UIA expuso 729 descendientes
> sin truncar; esto no reemplaza Narrador ni las pruebas manuales de drop/Extensión. El PDB antiguo
> quedó presente. Evidencia en `docs/VALIDACION-BUILD-2026-10-17-E1.md` y la hoja Windows.
> El generador W-13 ahora produce fixtures E-1 (`1.The Buddha`, `2.3DBenchy`, STL y archivo sin
> extensión) y un ZIP pequeño para reproducir el drop 7-Zip. Smoke con 50 archivos validó manifest,
> carpetas, contenido y entradas del ZIP; además rechazó una salida existente y una ruta fuera del
> área desechable sin escribir allí. La interacción real sigue pendiente.
> L-02: se añadió `XDG_DATA_HOME/naygo/icons` como destino de nuevos sets, PNG de usuario e íconos
> importados. Lectura/catálogos/exportación prefieren data y conservan fallback en la carpeta config
> antigua; no se migran ni borran archivos. En Windows data y config siguen siendo el mismo directorio.
> Pasaron 13 pruebas focales core, 8 de packs UI, check/Clippy cruzados del core Linux y el gate
> completo (1.408 pruebas, 8 ignoradas, Clippy y formato). ZIP/setup se regeneraron con pico 10,46 GiB;
> el check de UI Linux sigue bloqueado por falta de sysroot/pkg-config y no hubo ejecución Linux nativa.
> Se actualizó la instalación in-place, quedó abierta y responde como PID 14640; hashes en la validación.
> Pendiente Windows aprobado el 16 de octubre: la carpeta de instalación conserva un `naygo.pdb`
> antiguo (fecha 22 de agosto; ≈290 MB), distinto de los símbolos actuales de `target/release`
> (≈446 MB). Nicolás decidió el 2026-10-16: reemplazarlo sólo si se eligen los símbolos; si se
> desmarcan, conservarlo intacto. La entrada opcional `debugsymbols` con `ignoreversion` ya coincide
> con esta política. La actualización local del 16 de octubre con la tarea desmarcada terminó en código
> 0 y conservó el SHA-256 del PDB (289.722.368 bytes). El sandbox aislado confirmó reemplazo al
> marcar símbolos y eliminación del PDB al desinstalar; la app no ofrece reparación (`NoRepair=1`).
> W-15 cerrado; evidencia detallada abajo y en `docs/VALIDACION-BUILD-2026-10-16.md`.
> W-10C ya pasó gate y benchmark supervisado y se regeneró la distribución 0.5.2; append a 100k:
> 21,279 ms de mediana frente a 561,367 ms previos. El portable contiene EXE y notices, no PDB;
> ZIP/setup/exe coinciden con hashes registrados en `docs/VALIDACION-BUILD-2026-10-16.md`.
> La medición anterior confirma 256 invocaciones del callback Slint de teclado en release
> (p50 12,900 µs, máximo 183,700 µs); no cubre eventos Windows/Winit. El gate actual está registrado
> en la validación W-10C; el release anterior de 1.399 pruebas/8 ignoradas no incluye ese delta.
> L-01B (2026-10-16): subpunto Windows de carrera de destino **implementado, probado y empaquetado**.
> `core` expone una interfaz mínima core↔platform; el motor inyecta el rename y Windows usa
> `MoveFileExW` sin flags de reemplazo ni copia entre volúmenes. La revalidación local añadió una
> tercera prueba Win32: 32 contenciones
> reales entre el rename y `create_dir` sobre la misma ruta, comprobando que sólo gana una operación
> y que el origen/destino ganador conserva su contenido. Una cuarta prueba fija el mapeo de
> `HRESULT_FROM_WIN32(ERROR_NOT_SAME_DEVICE)` a `CrossesDevices`, necesario para el fallback
> cancelable cross-volume. Pasan los cuatro tests Win32, dos regresiones
> UI para rutas anidadas y las pruebas core de colisión/fallback; el gate previo de 1.417 pruebas
> (8 ignoradas), Clippy, formato, diff check y check/Clippy cruzados Linux de core/platform también
> pasan. La distribución Windows se regeneró bajo supervisor (pico 10,45 GiB).
> ZIP/setup/EXE y límites están en `docs/VALIDACION-BUILD-2026-10-16.md`; esta build no se instaló.
> Avance: Replace de carpeta prepara el árbol completo junto al destino, conserva
> el anterior ante cancelación/fallo de staging, revierte el rename si la publicación falla y
> conserva el respaldo cuando una colisión impide restaurarlo. Move borra fuentes sólo tras publicar
> y comparar contenido. La regresión cubre cancelación, fallo de staging, rollback, colisión tardía,
> Move y reanudación del journal. El gate workspace pasó con 1.423 tests (8 ignorados), Clippy,
> formato, diff check y check/Clippy cruzados Linux de core/platform. El portable y setup se
> regeneraron bajo supervisor (pico 10,45 GiB), hashes en la validación. No se instaló esta build.
> El resto de L-01B sigue parcial: recuperación de Replace tras terminar el proceso está implementada,
> pero quedan aceptación manual, pruebas de pérdida de energía/permisos y ejecución Linux nativa.
> Revisión 2026-10-18: se corrigió un error dentro de L-01B que descartaba con `flatten()` fallos
> individuales de `ReadDir` al planificar Copy/Move. Ahora el plan falla con `SourceUnreadable` y la
> ruta de carpeta antes de ejecutar cambios; el test inyectado de `PermissionDenied` y las 20 pruebas
> de `ops::plan` pasan. Guía, changelog y plan Linux actualizados; gate completo (1.453 aprobados,
> 8 ignorados), Clippy, formato, checks cruzados de `core`/`platform` y build supervisado pasaron.
> Portable y setup locales verificados, pero no instalados. Evidencia en
> `docs/VALIDACION-READDIR-2026-10-18.md`. L-01B queda parcial por aceptación manual, pruebas de
> energía/permisos y ejecución Linux nativa.
> Avance adicional Windows (2026-10-16): Replace persiste un manifiesto
> validado y marca el staging antes de renombrar. El worker de listado reanuda antes de emitir filas:
> restaura el árbol anterior si el proceso terminó entre renames, o termina la limpieza si el nuevo
> árbol ya fue publicado. Un proceso hijo terminó en ambas fronteras y el proceso padre recuperó los
> estados; también pasan casos de colisión, Replace interrumpido durante staging, activo concurrente
> y nombres reservados. La consulta corre fuera de UI, propaga cancelación y no pisa un destino nuevo.
> Gate protegido: 1.433 tests aprobados, 8 ignorados; Clippy, formato, diff check y check/Clippy
> cruzados Linux de core/platform pasan. La distribución 0.5.2 se regeneró bajo supervisor (pico 10,45 GiB);
> no se instaló. La prueba cubre terminación del
> proceso, no pérdida de energía. Si otra aplicación ocupa la ruta, se preserva el respaldo y se informa.
> L-01B sigue parcial: una ventana
> breve sin ruta es posible entre los dos renames Windows; la recuperación automática corre al listar
> el destino o su padre. Queda aceptación manual, pruebas de energía/permisos y ejecución Linux nativa.
> Ver plan Linux y la
> validación 2026-10-16. En la actualización previa se instalaron símbolos desmarcados bajo autorización;
> Naygo quedó abierto con EXE SHA-256 `c9db8752…` y el PDB anterior se conservó. El smoke
> portable aislado de 100 archivos registró spans de listado/modelo; la captura UI agotó tiempo
> antes de enviar entradas, se restauraron los cuatro JSON y la instalación quedó abierta como PID 9204.
> La muestra pasiva previa (PID 12532) dio 62,9–63,1 MiB y 2,359–2,781 s CPU por ventana;
> tras restaurar los JSON y relanzar PID 9204, otra muestra (3×30 s) dio 79,1 MiB y CPU
> 0,016/0,031/0,016 s. Ninguna confirma inactividad o interacción. El portable aislado sí produjo
> spans internos; captura visual falló dos veces y no se enviaron entradas.
> Actualización in-place posterior, 23:37: el EXE instalado coincide con la distribución local más
> reciente (`05eee5e4…`); PID 52608 responde. Los cuatro JSON y el PDB antiguo conservaron sus hashes.
> Actionlint v1.7.12, con checksum del release oficial verificado, no encontró errores en ambos
> workflows. W-07 interactivo, W-12 remoto y W-13 manual siguen pendientes; ver
> `docs/VALIDACION-BUILD-2026-10-16.md`.
> Captura pasiva adicional del PID 52608 (23:42–23:43): 71,4 MiB working set, 41,1 MiB privados,
> 368–370 handles, 24 hilos y CPU 0/0/0,078 s; un proceso, sin SMB. No se confirmó inactividad ni
> interacción; no cierra W-07. JSON y límites en `docs/BENCHMARKS.md`.
> W-14: la documentación oficial de SignPath sólo describe `Allowed branch names`; no resuelve cómo
> autorizar el trigger local `push.tags: v*.*.*`. No inferir compatibilidad tag/rama. Confirmar con
> SignPath antes de habilitar la firma; evidencia enlazada en `docs/POLITICA-DE-FIRMA.md`.

## Planificación de octubre — implementación pendiente

Nicolás solicitó un plan ejecutable con GPT-6 Luna para los hallazgos de la revisión y
una versión Linux. Eligió Ubuntu LTS y derivados, x86_64 como objetivo inicial.
El 2026-10-14 Nicolás indicó priorizar todos los temas Windows antes de cualquier otro; Linux
queda en espera hasta avanzar los bloques Windows.
Nicolás autorizó continuar sin detenerse por validación entre bloques; si surge alcance no
previsto, se informará para decidir si se agrega. La distribución previa y los resultados de
septiembre siguen siendo históricos hasta cerrar la nueva validación.

- [x] W-00 — inventario/respaldo y documentación vigente. Respaldo local de 983 archivos
  versionables verificado; hashes existentes de distribución coinciden con `dist/SHA256SUMS.txt`.
  Registro: `target/agent-out/w00-baseline-20261013.txt`.
- [x] W-01 — resultados tipados, parcialidad, canal desconectado y snapshots válidos; pruebas
  dirigidas y build portable/instalador verificados. La aceptación instalada queda en W-13.
  Detalle y pruebas en el
  [plan de mejoras](plans/2026-10-13-mejoras-y-ejecucion-luna.md).
- [x] W-02 — resaltado indexado por ruta completa; tests, benchmark y distribución aprobados.
  Ver el [plan](plans/2026-10-13-mejoras-y-ejecucion-luna.md).
- [x] W-03 — snapshots compactos con LRU y pruebas core; valor provisional de 32 MiB /
  100.000 entradas pendiente de medición W-07. Distribución verificada; ver el
  [plan](plans/2026-10-13-mejoras-y-ejecucion-luna.md).
- [x] W-04–W-06A/B: conflictos, persistencia e I/O de interfaz asíncronos; 359 pruebas UI, 891
  pruebas core y 22 flujos de operaciones aprobados. Distribución 0.5.2 supervisada y verificada.
  Detalles en [el plan](plans/2026-10-13-mejoras-y-ejecucion-luna.md).
- [x] W-06C — carga inicial de configuración, sesión, favoritos y plantillas en workers; suites
  completas aprobadas y distribución supervisada verificada. Alcance y evidencia
  en [el plan](plans/2026-10-13-mejoras-y-ejecucion-luna.md).
- [x] W-06D — enumeración Win32/USB y expulsión segura revalidada desde workers; conserva las
  ramas abiertas del árbol para unidades conectadas. Gate completo, distribución 0.5.3 regenerada
  bajo supervisor e instalada. Prueba pasiva UIA correcta; falta expulsión manual con USB real.
  Evidencia: `docs/VALIDACION-W06D-2026-10-19.md`.
- [x] W-06E — guardar y borrar temas de usuario fuera de callbacks; el guardado usa escritura
  atómica, los resultados actualizan catálogo/settings al volver al hilo UI. Los cuatro tests
  focalizados y el gate completo (1.480 aprobados, 8 ignorados, Clippy y formato) pasan. Falta
  aceptación visual del editor, sin I/O en callbacks comprobado y distribución supervisada verificada.
  Evidencia: `docs/VALIDACION-W06E-2026-10-19.md`.
- [~] L-07 — backend Linux FreeDesktop implementado de forma inicial: almacena por filesystem,
  crea `.trashinfo` con codificación de bytes, conserva la ruta exacta en el recibo de Deshacer,
  evita sobrescrituras en traslados/restauración y rechaza directorios de Papelera inseguros. Falta
  ejecutar los tests Linux agregados en Ubuntu, probar permisos y montajes reales, y aceptar UI;
  el host Windows confirmó check/clippy cruzados, Windows 1.478 tests y formato. Referencia oficial:
  https://specifications.freedesktop.org/trash/latest/.
- [x] M-01 — 16 ocurrencias de Clippy agrupadas en siete clases en la corrida global del 13 de
  octubre. Corregirlas sin supresiones generales y cerrar con `-D warnings`; aislado del alcance
  funcional W-06C. Clippy, workspace y build release posteriores pasaron. Ver el bloque M-01 en
  el [plan](plans/2026-10-13-mejoras-y-ejecucion-luna.md).
- [ ] W-07, W-12 remoto y W-13–W-14: [mejoras y ejecución con Luna](plans/2026-10-13-mejoras-y-ejecucion-luna.md).
  W-14, revalidación local 2026-10-30: `scripts/test-signing.ps1` pasó con datos sintéticos; los
  metadatos del EXE y setup 0.5.4 son correctos y ambos están `NotSigned`. No prueba firma real.
  W-07 requiere mediciones interactivas; W-12 un run del workflow actualizado en Windows Server
  2025; W-13 aceptación manual instalada; W-14 configuración/aprobación externa de SignPath y firma
  verificada de EXE y setup. W-08–W-11 y M-01 están cerrados localmente; sus criterios de aceptación
  manual que aún falten se agrupan en W-07/W-13.
  W-07 ya instrumenta trabajos enviados, iniciados, terminados, activos, pico y rechazados de
  preview, validación de rutas y packs, además de espera/ejecución de packs. La prueba de apagado
  integrado del colector, suite del workspace, Clippy, formato, catálogos y build supervisado pasan.
  Las métricas instrumentadas ya cubren los tres tipos de worker; ahora también miden la duración
  opt-in de callbacks de teclado, filas y sort, sin datos de entrada ni rutas. Aún falta la matriz
  real de rendimiento y la presentación del renderer. El 15 de octubre se regeneró y verificó el
  release con esos spans; el portable actual arrancó con el opt-in y escribió métricas internas de
  listado/modelo/tick al cerrarse. La automatización de entrada no generó muestras `ui.input.*`; luego
  se midió por separado el callback Slint generado de teclado en headless. Esa prueba confirma el
  callback, pero no sustituye la interacción del SO/Winit, la matriz ni el renderer. Hashes y evidencia
  en `docs/VALIDACION-BUILD-2026-10-15.md`. Repetición supervisada del 2026-10-17: cuatro pasadas
  de 256 invocaciones `DownArrow` sobre el callback Slint en headless; las tres p50 warm fueron
  11,300/7,800/8,000 µs (mediana 8,000 µs), máximos 64,300/50,700/55,800 µs. No mide Windows/Winit
  ni presentación y no cierra W-07;
  detalle en `docs/BENCHMARKS.md`. Repetición Windows del 14 de octubre: tres ejecuciones sintéticas en el mismo equipo
  quedaron 1,0–9,4 % sobre las medianas de 100.000 filas de la base, bajo el umbral orientativo de
  15 %; no mide la UI completa ni cierra W-07. JSONL y detalle en `docs/BENCHMARKS.md`.
  El 15 de octubre se repitió el harness tres veces con fingerprint completo y logs JSONL. Las
  medianas externas para 100.000 filas fueron 21,3892 ms vista inicial, 2,4636 ms sort tamaño,
  24,1554 ms filtro+sort y 19,7888 ms índice por ruta. Son mucho menores que las publicadas el día
  anterior; se registran sin atribuir causa, porque no se controlaron diferencias de checkout,
  artefacto o estado CPU. La captura sigue siendo sintética y no cierra W-07.
  Además, el 15 de octubre el nuevo modo opt-in de filesystem midió con el worker real del core
  primera entrada/lote de 1.024/total: 1,124/15,963/15,969 ms para 10k y
  1,170/161,651/161,659 ms para 100k en Windows local. Fixture temporal de archivos vacíos, tres
  pasadas por tamaño y borrado automático. También se canceló tras 1.024 filas en seis pasadas;
  los terminales fueron Cancelled y el worker terminó/join en mediana 0,0712 ms (10k) y 0,0876 ms
  (100k). Fingerprint: Ryzen 7 1700X, 63,94 GiB RAM, Rust 1.96.0, Windows 10. No incluye
  modelo/renderer Slint ni red. JSONL y límites en `docs/BENCHMARKS.md`; esto
  aporta evidencia parcial, no cierra W-07.
  La UI agrega spans opt-in por firma (`ui.rows_signature`), construcción desde estado (`ui.rows_build`),
  conversión a `RowData` (`ui.rows_convert`) y mutación del modelo (`ui.rows_model_apply`), sin leer
  reloj con la captura apagada. El 15 de octubre, una ventana pasiva de 20 s y tres de 30 s del
  proceso instalado se mantuvieron en 72,4–72,8 MiB de working set, 54,8–54,9 MiB privados,
  430–432 handles y 33–35 threads; los deltas CPU de 30 s fueron 0,14/0,03/0,16 s. No había SMB
  mapeado; no se midió interacción ni auxiliares, y esto no cierra W-07. Se añadió
  `scripts/capture-w07-process.ps1` para repetir las muestras pasivas en JSON; su smoke de dos
  segundos pasó. Repetición pasiva posterior: working set 72,8 MiB, privados 54,8 MiB, 430 handles,
  33 threads y deltas CPU de 0,234/0,156/0,156 s; sin mapeos SMB. Evidencia en
  `target/agent-out/w07-process-20261015-034602.json` y `docs/BENCHMARKS.md`. Nueva repetición
  después del reinicio en tres ventanas de 30 s: 41,6 MiB working set, 40,1 MiB privados, 349
  handles, 17 threads y 0,031/0,047/0,016 s CPU; tampoco había SMB. No se atribuye la diferencia
  entre sesiones porque no se controló la actividad de fondo. JSON en
  `target/agent-out/w07-process-20261015-080019.json`; sigue sin incluir auxiliares ni medir
  interacción. Nueva muestra de la instancia abierta (PID 23856), 3×30 s: 70,2 MiB working set,
  46,9 MiB privados, 439 handles, 19 threads y CPU 0/0,047/0 s; sin SMB. No hubo interacción ni
  confirmación de inactividad, y la muestra no incluye auxiliares. Diferencia no atribuida frente
  a la captura posterior al reinicio. JSON en `target/agent-out/w07-process-20261015-125432.json`.
  La instancia no inició con `NAYGO_PERF_METRICS=1`; el mutex fijo `NaygoSingleInstance` hace que
  una copia adicional avise a la ventana existente y salga. Para capturar spans internos se requiere
  un cierre limpio y relanzamiento opt-in; no se intervino sobre la sesión abierta.
  Repetición pasiva a las 14:22 del mismo proceso: 72,9/46,9 MiB, 441 handles, 19 threads y CPU
  0,094/0,047/0 s en tres ventanas; sin SMB. Sin interacción ni auxiliares. JSON en
  `target/agent-out/w07-process-20261015-142335.json`; se registra también en `docs/BENCHMARKS.md`.
  Nueva repetición 19:04–19:06 del mismo proceso instalado: 46,6/47,5 MiB working/private set,
  437 handles, 17 threads y CPU 0 en cada ventana; sin SMB. No se confirma inactividad ni se miden
  renderer, auxiliares o latencia; diferencia no atribuida. JSON en
  `target/agent-out/w07-process-20261015-190632.json`, documentado en `docs/BENCHMARKS.md`.
  La instancia anterior usaba el EXE instalado el 14 de septiembre, no el release recién generado:
  tamaños y hashes diferían. El 16 de octubre Nicolás autorizó actualizar la instalación al W-10C;
  se verificó versión 0.5.2 y coincidencia del hash del EXE con el release.
  El 16 de octubre el portable actual se extrajo en una carpeta aislada, arrancó, creó archivos
  portables y cerró al recibir `--shutdown`. El log opt-in conserva métricas de listado/modelo/tick,
  pero no muestras `ui.input.*`; el smoke no cierra W-07 ni sustituye aceptación manual W-13. Hashes
  y evidencia en `docs/VALIDACION-BUILD-2026-10-16.md`.
  Reintento automatizado posterior: `SendKeys` falló al enviar Ctrl+L, `PostMessage` no activó los
  callbacks de teclado, fila o sort aunque encoló los mensajes, y Windows negó el foco mientras otra ventana estaba activa.
  Se dejó intacta esa ventana y se cerraron los Naygo aislados con `--shutdown`; falta medir
  con Naygo en primer plano. Detalle y logs en `docs/BENCHMARKS.md`.
  Captura adicional del release: tres ventanas pasivas de 30 s (intervalo 2 s) registraron pico
  55,8 MiB working set, 26,7 MiB privados, 344 handles y 20 hilos; la primera ventana incluye
  arranque (1,953 s CPU) y las dos siguientes dieron 0 s. No hubo descendientes ni SMB. Es evidencia
  parcial de reposo, no cubre interacción, UNC o preview; JSON y límites en `docs/BENCHMARKS.md`.
  Después de actualizar la instalación se capturó W-10C en 3×30 s (intervalo 2 s): picos de
  72,1 MiB working set, 41,3 MiB privados, 357 handles y 25 threads; CPU 0,062/0,047/0,047 s.
  Solo apareció el proceso raíz, sin SMB. No hubo interacción ni inactividad confirmada, por lo que
  es evidencia pasiva parcial y no cierra W-07. JSON: `target/agent-out/w07-installed-w10c-20261016-0924.json`.
  E-6 instalado el 18 de octubre: PID 41292, EXE idéntico al release. Tres ventanas pasivas de 30 s
  se mantuvieron en 78,2/47,1 MiB working/private set, 388 handles, 26 hilos y CPU 0 s; sólo se
  observó Naygo, sin mapeos SMB. No se controló inactividad y no cierra W-07. JSON:
  `target/agent-out/w07-e6-passive-20261018.json`; análisis en `docs/BENCHMARKS.md`.
  Hallazgo de carga en el release: tres aperturas estables por CLI de 100k registraron p50 de
  `ui.sync_rows` 118,962 ms y `ui.tick` 139,940 ms; con 10k fueron 15,087/26,091 ms. Durante una
  captura de 90 s sobre 100k, `--shutdown` no cerró en 20 s; la instancia aislada tuvo que detenerse
  forzosamente. `sync_rows` reconstruye y convierte el vector completo durante cada revisión de
  streaming, probable fuente del costo. Evidencia y límites en `docs/BENCHMARKS.md`;
  Nicolás autorizó W-10C el 2026-10-16 para renderizar lotes añadidos sin reconstruir las filas previas.
  El 15 de octubre se mejoró `scripts/capture-w07-process.ps1` a schema v3 para muestrear el árbol
  durante cada ventana, retener descendientes observados aunque terminen antes del cierre y reportar
  picos de recursos. Un smoke verificó un hijo que termina antes de cerrar la ventana. No es evidencia
  de rendimiento del release; procesos más breves que el intervalo pueden escapar y working set
  agregado puede duplicar páginas compartidas.
  [x] Capturador v3 y prueba de detección de hijo transitorio; [x] observación pasiva del release W-10C;
  [ ] medición W-07 de interacción. Evidencia de herramienta en
  `target/agent-out/w07-process-sampling-smoke.json` y `docs/BENCHMARKS.md`.
  Preparé la [hoja de aceptación del release Windows 0.5.2](VALIDACION-WINDOWS-0.5.2-2026-10-15.md)
  con hashes, entorno, matriz manual, mediciones W-07 y estados pendientes. El 16 de octubre se
  actualizó la instalación de usuario y la ventana del release W-10C quedó respondiendo; el ZIP
  portable vigente sigue sin smoke.
  La distribución de recuperación de Replace también se dejó sin abrir: PID 31084 conserva el mutex
  único de la sesión. Abrir el portable avisaría a la instancia instalada y `--shutdown` podría
  cerrarla; W-07 interactivo queda para una sesión aislada o tras cerrar Naygo manualmente.
  W-10B ya tiene
  igualdad semántica de filas/celdas Slint y sync incremental medido en headless; el append de una
  fila a 100k bajó de 44,032 s a 366,762 ms. La medición no incluye interacción ni renderer.
  El 16 de octubre se extendió el harness manual para rasterizar cinco frames de 1280×800 con
  `MinimalSoftwareWindow`, tras 10k/100k filas. En tres pasadas con perfil de test no optimizado,
  las medianas de renderer fueron 72,409/78,535 ms; sync inicial 233,274/1.823,465 ms y append
  263,609/2.398,735 ms. La virtualización reduce la diferencia de raster entre tamaños; estas cifras
  no son tiempos de release ni presentación Winit. Logs y límites en `docs/BENCHMARKS.md`.
  Se repitió el harness en perfil release optimizado, tres veces: mediana a 100k de sync inicial
  453,691 ms, una fila 47,235 ms, append 561,367 ms y render headless 3,086 ms. El append sigue
  siendo costoso; W-10C ya implementa rangos append-only y `sync_rows` agrega solo filas nuevas cuando
  la vista conserva un prefijo idéntico, con fallback si se intercala el orden. Tests focalizados,
  gate completo, Clippy y tres benchmarks release pasan; append a 100k midió 21,279 ms de mediana
  frente a 561,367 ms (~26,4×, comparación headless). La distribución 0.5.2 ya se regeneró bajo
  supervisor y sus hashes están verificados; el smoke del ZIP W-10C queda pendiente por la instancia
  instalada que se dejó abierta, y la aceptación interactiva sigue en W-07/W-13.
  W-07
  ahora tiene evidencia headless
  optimizada, pero falta medir UI interactiva y presentación Winit. Detalle/logs en
  `docs/BENCHMARKS.md`; gate workspace anterior: 1.399 tests, 7 ignorados, Clippy limpio.
  Medición separada del callback de teclado: 256 invocaciones al callback generado `AppWindow::invoke_key`,
  conectado a `wire_nav_keys`, en release dieron p50 12,900 µs y máximo 183,700 µs. El log confirma
  256 muestras `ui.input.keyboard_callback`; pico supervisado 8,60 GiB. Es una invocación directa del
  callback Slint, no un evento de teclado del sistema ni una medición Winit. W-07 sigue abierto;
  log: `target/agent-out/guarded-20261016-060837-057.*.log`.
  El gate Windows completo posterior pasó: 904 core, 22 flujos, 54 platform (6 ignorados), 388 UI
  y 16 integración/render; Clippy limpio. La distribución 0.5.2 se regeneró bajo supervisor con
  pico 10,41 GiB; hashes en `docs/VALIDACION-BUILD-2026-10-14.md`.
  W-08 implementó recogida por poll limitada
  a 1.024 filas/3 ms, canal acotado con backpressure cancelable/terminal separado y presupuesto
  global rotativo entre paneles y árbol; suite completa, Clippy, build supervisado y hashes de
  distribución verificados. Sigue pendiente medir primera fila, p95 de interacción y tiempo total
  frente a la base anterior; por eso W-08 no se cierra como resultado de rendimiento.
  W-09 implementó admisión común de ocho workers para listados/probes, pendientes coalescidas
  por panel/rama, cancelación/revisión de probes y reintento en el tick; suite, Clippy y distribución
  supervisada pasaron. Aún se puede decidir si conviene mostrar explícitamente la espera por capacidad.
  W-10A comparte el `Arc<[usize]>` cacheado con render y comparación: el acceso cacheado a 100.000
  índices bajó de 0,021924 ms al clonar el Vec a 0,000061 ms al compartirlo en benchmark release.
  W-10B compara semánticamente las filas y sus celdas y actualiza solo filas afectadas por eventos
  `Modified` estables del watcher. En tres corridas headless a 100.000 filas, modificar una fila medió
  31,874 ms frente a 353,568 ms (~91 % menos) en la muestra previa. Carga inicial y append requieren
  comparación controlada: sus medianas fueron 333,959 ms y 403,996 ms frente a muestras previas
  únicas de 282,260 ms y 366,762 ms. El gate completo pasó (1.393 tests, 7 ignorados, Clippy sin
  warnings y formato); prueba focalizada confirma que el watcher entrega el delta estable al sync.
  ZIP/setup 0.5.2 regenerados bajo supervisor con hashes verificados. W-07
  mantiene pendiente la medición interactiva y W-13 la aceptación instalada.
  W-11A comparte la carga del PDF entre el conteo de páginas y `pdf-extract`, limita el texto a
  8.000 caracteres durante la extracción y el contenido a 500 páginas. Las cuatro pruebas PDF, la
  suite completa, Clippy y build supervisado pasan; los hashes están verificados. W-11B limita el
  preflight ZIP32/ZIP64, las lecturas/expansión TAR/GZIP, las rutas internas y la suma de tamaños;
  28 pruebas de preview, suite workspace, Clippy y build supervisado pasan; los hashes están
  verificados. Ambos lectores
  conservan timeout/cancelación y el límite de memoria del proceso; no se agregó render de páginas.
  W-11C preserva caracteres cuando el tope de texto corta UTF-8/UTF-16 y elimina las copias
  completas al inspeccionar el canvas SVG. Cobertura sintética: Unicode truncado, línea larga,
  4.000 elementos SVG, canvas declarado, imagen con dimensión extrema, animación pausada/cancelada y
  cambio rápido. Suite completa y Clippy pasan; el build supervisado y hashes están en la bitácora.
  El corpus de uso real y las mediciones comparativas siguen pendientes con W-07.
  W-12 fija Rust 1.96.0, `--locked`, un solo job y el runner explícito `windows-2025` para CI,
  benchmark y release; CI/release comprueban el caption del SO y benchmark registra versión/build,
  modelo del CPU y RAM física total; añadió un artifact
  semanal/manual de benchmark JSONL y un smoke headless del ZIP en el runner de release. El último
  run remoto visible (SHA `abdef402`, 7 de septiembre) fallaba al enumerar el ítem reciclado durante
  restore en Windows Server 2025. Se corrigió la causa: guardar el `IShellItem` recién creado que
  entrega `PostDeleteItem` y restaurar mediante `IFileOperation::MoveItem`, sin depender del refresh
  de Explorer ni de su verbo contextual `undelete`. El test de ida/vuelta dejó de estar ignorado,
  exige que el callback entregue el ítem real; suite completa (1.381 tests, 6 ignorados), Clippy y
  distribución supervisada pasan localmente. W-12 aún requiere ejecutar el workflow actualizado
  remotamente. Se encontró y corrigió un hueco adicional: el restore exigía enumerar la carpeta
  virtual antes de usar el recibo cacheado. Ahora la enumeración de Papelera es diferida y solo se
  ejecuta para recibos reconstruidos; el test de ida/vuelta ahora inyecta un enumerador que falla y
  verifica cero llamadas para el recibo directo. El gate Windows y Clippy pasaron. Además, el caché
  acotado de 1.024 identidades expulsa FIFO y refresca antigüedad al reinsertar una ruta. El gate y
  la distribución 0.5.2 final pasaron; ZIP/setup y hashes se verificaron. Queda la ejecución remota
  del workflow en Windows Server 2025. Un test adicional cubre recibos reconstruidos: si falta el
  enumerador, se intenta una vez y se conserva un error explícito.
  Revalidación local 2026-10-15: `cargo test -p naygo-platform --locked
  trash::tests::enviar_un_archivo_a_papelera -- --exact --nocapture` pasó (1/1) en Windows; el
  fixture temporal volvió a su ruta original y restauró sin enumerar la Papelera. No sustituye el
  runner Windows Server 2025.
  Refuerzo Windows autorizado el 15 de octubre: resuelto el path final de Papelera sin
  `canonicalize`, para que un junction/symlink no redirija el borrado al destino; el fallback
  clásico trabaja por elemento y no se ejecuta tras cancelar. Los callbacks `PostDeleteItem` se
  asocian a su origen, índice y HRESULT; los resultados parciales conservan recibos de los éxitos
  mientras UI muestra fallos por ruta aunque la ruta de entrada sea relativa. Pasaron 5 tests
  dirigidos, incluido un junction creado, reciclado y
  restaurado con el destino intacto; `cargo check --tests` Windows para platform/UI pasó. También
  pasó `scripts/test-all.ps1`: 1.389 tests, seis ignorados, Clippy `-D warnings` y formato. Falta
  confirmación remota Windows Server 2025. El build 0.5.2 se regeneró bajo supervisor con pico
  10,42 GiB; ZIP/setup, firma no aplicada y hashes constan en
  [validación 2026-10-15](VALIDACION-BUILD-2026-10-15.md).
  Revisión adicional Windows: todas las comprobaciones de ocupación al restaurar y al verificar el
  fallback inspeccionan la entrada con `symlink_metadata`. Sólo `NotFound` significa ausencia;
  `PermissionDenied` y otros errores se propagan y hacen fallar la operación de forma segura, sin
  emitir un recibo falso ni ejecutar el fallback. Las pruebas cubren el junction colgante, el rechazo
  del destino ocupado antes de enumerar y el error de permisos. Pasaron las 8 pruebas
  `trash::tests`, el gate supervisado completo (1.396 aprobados, 7 ignorados), Clippy, formato,
  `git diff --check` y el check cruzado Linux de platform. La distribución 0.5.2 se regeneró bajo
  guard (pico gate 8,39 GiB; pico release 10,41 GiB); hashes actuales en la bitácora. No se instaló
  ni se cerró la instancia interactiva. W-07/W-13 manual y el run remoto de W-12 siguen pendientes.
  Para facilitar la aceptación manual sin reutilizar archivos personales, está disponible
  `scripts/prepare-w13-filesystem-fixtures.ps1`: genera listados sintéticos de 10k/100k y casos de
  operaciones bajo `target/agent-out/`, sin sobrescritura ni limpieza automática. El 16-10 se generó
  el corpus completo en `target/agent-out/w13-filesystem-20261016-014835/`; se verificaron 10.000 y
  100.000 archivos y los fixtures de operaciones. Aceptación W-13 y mediciones interactivas W-07
  siguen pendientes.
- [x] W-10C — append incremental de lotes al modelo Slint con verificación exacta del prefijo y
  fallback seguro ante inserción intermedia; gate completo, benchmark release y distribución
  0.5.2 verificados el 2026-10-16. Evidencia en `docs/BENCHMARKS.md` y
  `docs/VALIDACION-BUILD-2026-10-16.md`.
- [x] W-12 — corrección y verificación local del restore de Papelera, incluido test que prohíbe
  invocar el enumerador al restaurar el recibo directo; gate, Clippy y distribución final verificados.
- [x] W-12 — acciones CI/release fijadas a cinco SHAs verificados por API; YAML, bloques PowerShell
  `pwsh` y `scripts/check_action_pins.py` pasan localmente.
- [ ] W-12 — confirmar el workflow actualizado en un runner Windows Server 2025.
  Reconsulta remota 2026-10-30: la última CI es la corrida `34155852541` del 7 de septiembre,
  commit `abdef402babd2c136744c67ee9833c2f67bf0f16`, fallida al restaurar Papelera; Clippy se
  omitió. El workflow y `trash.rs` locales difieren de ese commit, así que no valida el código actual.
  Consulta de solo lectura del 15 de octubre con `gh run list`: no hay una corrida CI/release que
  incluya este checkout; la última CI pública sigue siendo `main`/`abdef402` del 7 de septiembre
  ([run](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/34155852541)).
  Revisión directa del log remoto el 16 de octubre: falló el paso `Tests`, específicamente
  `trash::tests::enviar_un_archivo_a_papelera`; el test esperaba restaurar el recibo, pero el código
  entonces reportó `el elemento ya no está en Papelera`. Los otros 46 tests de platform pasaron y
  hubo seis ignorados. El fallo concuerda con el restore basado en enumeración que se reemplazó por
  la identidad `IShellItem` capturada en `PostDeleteItem`; el código corregido aún necesita un run
  remoto.
  Nota W-12 aprobada el 15 de octubre: las cinco corridas visibles de `Preview implementation review`
  del 9 de septiembre pertenecen a `feat/responsive-previews-and-paths`, sin PR abierto. La última
  (`34364248486`) falló al aplicar cambios revisados porque la base de `docs/PREVIEW_AND_PATHS.md`
  no coincidía con la esperada; se detuvo antes de compilar o probar. Es una incidencia del flujo
  experimental de parches y no evidencia un fallo del código ni del workflow W-12 actual. El script
  remoto `scripts/preview-fixes-3.py` tampoco existe en este checkout; si se retoma esa automatización,
  habrá que reconciliarla con la documentación vigente antes de aplicar el parche.
  Hallazgo nuevo aprobado el 15 de octubre: CI y release fijan ahora todas sus acciones externas a
  SHAs completos verificados por GitHub API (`checkout`, `upload-artifact`, `rust-cache`,
  `rust-toolchain` y SignPath). Las cinco referencias exactas se resolvieron contra la API de
  commits de GitHub; el verificador local pasó. Falta el run remoto Windows Server 2025, que no se
  dispara ni publica desde este checkout.
  `scripts/check_action_pins.py` quedó integrado al gate para impedir que vuelvan a entrar tags
  mutables; también exige digest SHA-256 para acciones Docker.
  CI declara explícitamente `contents: read`, que cubre checkout, tests y artifacts; release conserva
  sus permisos de escritura para publicar. YAML y gate local verificados; la ejecución real sigue
  pendiente.
  Revalidación local, 2026-10-16: `cargo test -p naygo-platform --locked trash::tests` pasó las ocho
  pruebas, incluidas Papelera directa, recibos reconstruidos, resultados parciales y junctions;
  `scripts/check_action_pins.py` y `scripts/test-signing.ps1` también pasaron. En la consulta de
  solo lectura `gh run list`, las cinco ejecuciones más recientes son revisiones del 9 de septiembre
  en una rama experimental, y el último CI de `main` sigue fallido el 7 de septiembre en `abdef402`.
  Ese SHA coincide con `HEAD` remoto y local, pero los cambios W-12 actuales siguen sin commit ni
  publicación. Ningún run remoto ha probado los workflows actualizados; no se disparó uno contra el
  checkout antiguo.
  Revalidación local 2026-10-17: `scripts/check_action_pins.py`, pruebas `scripts/test-signing.ps1`
  y las ocho pruebas `cargo test -p naygo-platform --locked trash::tests` pasan. Los dos workflows
  pasan PyYAML y `actionlint` v1.7.12; el ejecutable temporal se descargó en `target/agent-out/` y
  su SHA-256 coincidió con el digest publicado por [la release oficial](https://github.com/rhysd/actionlint/releases/tag/v1.7.12).
  Estas validaciones estáticas no sustituyen la ejecución en GitHub Actions. `gh run list --workflow CI` aún muestra como último CI el
  run 34155852541 de `main` (`abdef402`, 2026-09-07), sin ejecución del checkout actualizado.
  Revalidación de solo lectura 2026-10-18: las ocho corridas visibles más recientes siguen siendo
  revisiones `Preview implementation review` del 9 de septiembre y el último CI de `main` es el run
  34155852541 fallido del 7 de septiembre; no hay ejecución del workflow actualizado. En el checkout
  actual pasaron `python scripts/check_action_pins.py` y `scripts/test-signing.ps1` (incluye aceptación
  de SignPath Foundation, rechazo de identidad alterada y rechazo de artefactos unsigned). No se
  publicó ni disparó una ejecución. W-12 aún necesita runner Windows Server 2025; W-14 sigue sujeto a
  configuración/aprobación externas de SignPath y a la primera firma real.
  W-14 prepara
  firma local de EXE + instalador y dos solicitudes SignPath Foundation en el workflow; las
  configuraciones validan el nombre y la versión del producto, y el workflow exige que ambas firmas
  Authenticode sean válidas y que el publicador sea exactamente `SignPath Foundation`. El helper
  registra sujeto, emisor y thumbprint; `scripts/test-signing.ps1` prueba certificados sintéticos,
  incluido rechazo sensible a mayúsculas, y se ejecuta en CI. Hay un borrador de política enlazado
  desde README, pendiente de revisión/publicación y confirmación de responsabilidades. La etiqueta
  oficial `v3` de la acción SignPath
  resuelve a `f6d04783b4569d051e0c80105fe66e82819d0092`; se verificó por GitHub API el 15 de octubre.
  El `action.yml` de ese commit se leyó por API y los inputs de ambas solicitudes coinciden con el
  contrato. El workflow quedó fijado al commit inmutable
  [`f6d04783b4569d051e0c80105fe66e82819d0092`](https://github.com/SignPath/github-action-submit-signing-request/commit/f6d04783b4569d051e0c80105fe66e82819d0092).
  La configuración del proyecto, el certificado y la aprobación manual de firma siguen pendientes.
  Faltan aceptación/configuración del proyecto y una ejecución real aprobada. Inno Setup 6.7.3
  mantiene padding de espacios ASCII al final en metadatos PE incluso con directivas de versión
  explícitas. El helper local valida nombre/versión recortando sólo ese padding y rechaza espacios
  iniciales o valores distintos; el build local y el workflow comprueban los metadatos antes de
  enviar, después de firmar y en el paquete final. Tests usan valores sintéticos y el EXE/setup 0.5.2
  actuales pasan. Esto no demuestra que el comparador remoto SignPath recorte igual; no se relajaron
  sus restricciones. La
  distribución 0.5.2 regenerada el 14 de octubre continúa sin firma; hashes y límites en
  [validación del build](VALIDACION-BUILD-2026-10-14.md).
  La revisión de los [términos oficiales de SignPath Foundation](https://signpath.org/terms.html)
  confirmó que el proyecto debe estar publicado en el formato que se firmará; la release v0.3.0 ya
  documenta portable ZIP e instalador, aunque la elegibilidad final depende de SignPath.
  El enlace ya usa el rótulo “Code signing policy” en README y en las notas de releases futuras.
  La única release pública consultada, `v0.3.0`, no incluye el enlace; queda pendiente decidir si
  se actualiza esa página histórica o se cubre al publicar la próxima versión. Los términos también
  exigen MFA para el acceso al repositorio y a SignPath; verificarlo es un requisito operativo antes
  de postular y no puede confirmarse desde el código local.
  Preparación W-14: la política ahora enumera la configuración SignPath y las siete variables/secretos
  que consume el workflow. Hallazgo por confirmar antes de activar firma: release parte de tags
  `v*.*.*`, mientras la restricción de origin verification se documenta como `Allowed branch names`;
  confirmar con SignPath cómo autorizar esos refs antes de habilitar la política o modificar el
  trigger. Se documentó en `docs/POLITICA-DE-FIRMA.md`; no se cambió el workflow.
  Revalidación local 2026-10-18: `scripts/test-signing.ps1` volvió a pasar identidad del firmante,
  rechazo de metadatos/publicadores incorrectos y artefactos sin firma; `scripts/check_action_pins.py`
  confirma referencias inmutables. `actionlint` no está instalado en este host; el último lint
  registrado de los workflows fue el 17 de octubre con v1.7.12; la misma copia temporal v1.7.12 volvió
  a validar ambos YAML el 18 de octubre. No se solicitaron ni usaron
  credenciales SignPath; tampoco hubo una corrida remota nueva.
  Revalidación local 2026-10-18: `cargo test -p naygo-platform --locked trash::tests` pasó 8/8;
  `scripts/check_action_pins.py` y `scripts/test-signing.ps1` también pasaron. La documentación
  oficial consultada sigue sin especificar cómo se representa `push.tags` para `Allowed branch names`;
  el workflow de demostración oficial reserva `release-signing` para `refs/heads/main` y
  `refs/heads/release/*`, sin documentar tags. Se dejó formulada la pregunta exacta en
  `docs/POLITICA-DE-FIRMA.md`. Runner Windows Server 2025,
  configuración/credenciales SignPath y firma real siguen pendientes.
  Revalidación local 2026-10-20: `scripts/check_action_pins.py` y `scripts/test-signing.ps1` pasan;
  el helper verifica metadatos Inno con padding y rechaza publicadores/metadatos inválidos y EXE
  sin firma. El instalador 0.5.3 actual reporta `ProductName=Naygo`, `ProductVersion=0.5.3` y
  estado Authenticode `NotSigned`. `actionlint` no está instalado en el host, por lo que no se
  repitió aquí el lint; su última validación registrada es v1.7.12 el 18 de octubre. No se
  solicitaron credenciales ni se firmó, publicó o disparó un workflow.
  Consulta remota adicional, 2026-10-18 04:41: `actionlint` v1.7.12 pasó en ambos workflows;
  `gh run list` sigue mostrando el último CI de `main` en `abdef402` (2026-09-07) y no encuentra
  corridas de los cambios locales. No se publicó el checkout.
- [x] W-15 — definir y probar la política de `naygo.pdb` obsoleto en upgrades. Nicolás decidió
  reemplazarlo sólo al elegir símbolos y conservarlo al desmarcar. El update real in-place con tarea
  desmarcada conservó el PDB antiguo; un setup de sandbox con AppId independiente confirmó que
  desmarcar conserva un PDB marcador y marcar `debugsymbols` lo reemplaza por el PDB release exacto.
  El uninstaller del sandbox lo retiró correctamente. El registro instalado indica `NoRepair=1`, por
  lo que no se ofrece un flujo de reparación. El PDB real conserva su hash y Naygo está abierto.
  Evidencia completa: `docs/VALIDACION-BUILD-2026-10-16.md`. Referencias Inno: [sección Files]
  (https://jrsoftware.org/ishelp/topic_filessection.htm), [log de desinstalación en upgrades]
  (https://jrsoftware.org/ishelp/topic_appendnotes.htm).
- [x] W-16 — opción Windows para iconos de archivos asociados por extensión. Desactivada por
  defecto; consulta `SHGetFileInfoW` con `SHGFI_USEFILEATTRIBUTES` en worker, nunca abre la ruta,
  limita el cache a 512 tipos y mantiene los iconos de Naygo como fallback. La interfaz sólo
  muestra el control en Windows. Gate completo, build release supervisado, setup, portable e
  instalación local verificados; la instancia quedó abierta y respondiendo. Evidencia: `docs/
  VALIDACION-W16-2026-10-17.md`.
- [x] W-17 — renombrar simple y por lotes sin reemplazar un destino que aparezca después de
  planificar. La publicación usa el adaptador atómico no-clobber compartido con movimientos; en
  Windows se conserva la ruta especial para cambios de solo mayúsculas/minúsculas. Regresión de
  carrera y prueba Windows de case-only pasan. Gate completo: 1.458 tests, 8 ignorados, Clippy y
  formato. Distribución supervisada regenerada, hashes verificados y no instalada. Ver
  `docs/VALIDACION-W17-RENAME-2026-10-18.md`.
- [ ] L-00–L-14: [Linux Ubuntu con Luna](plans/2026-10-13-linux-ubuntu-luna.md).
  Revalidar auditoría histórica, semántica POSIX/identidad, XDG, capacidades, integraciones,
  supervisor Linux de preview, CI y empaquetado. Matriz concreta, dependencias nativas y
  licencias requieren decisiones en L-00; no habilitar lectores sin contención validada.
  L-08 avance local 2026-10-19: instancia única Linux con lock `try_lock`, Unix socket 0600,
  `SO_PEERCRED`, frames limitados y rutas POSIX en bytes; listener fuera de UI y cola acotada de
  ocho solicitudes. Tests de runtime dir/lock/shutdown/ruta no UTF-8/IPC corrupto/wake se compilan
  para Linux; platform y UI pasan check/Clippy cruzados; gate Windows 1.480/8 y formato pasan.
  La build Windows se regenerará bajo supervisor. WSL está detenido por
  `HCS_E_HYPER_V_NOT_INSTALLED`, así que runtime y esos tests Linux no están aceptados todavía.
  L-09A primer subpaso local: codec puro para `text/uri-list` en
  `core::clipboard::uri_list`, con nueve pruebas Windows de round-trip, bytes POSIX no UTF-8 y
  rechazos seguros. Aún no accede al portapapeles del escritorio ni implementa copy/cut; no se
  habilitó una función visible y la validación nativa Ubuntu/Wayland/X11 sigue pendiente. El 2026-10-20
  gate Windows pasó (1.492 aprobadas, 8 ignoradas, Clippy y formato) y `cargo check` cruzado de
  `naygo-core` para `x86_64-unknown-linux-gnu` pasó. Evidencia en
  `docs/VALIDACION-L09A-URI-LIST-2026-10-20.md` y plan `docs/plans/2026-10-13-linux-ubuntu-luna.md`.
  Avance adicional 2026-10-20: `core::clipboard::desktop_formats` añade metadata copy/cut para
  Nautilus y el indicador de corte de Dolphin; el codec acota listas a 16 MiB, 100.000 elementos
  y 64 KiB por ruta. Pasan 34 tests focalizados, y `cargo check --tests`
  y Clippy cruzados Linux del core pasan con `-D warnings`. Graphify actualizado (9.639 nodos,
  18.286 relaciones, 896 comunidades). Aún no se implementa el transporte ni la propiedad del
  portapapeles, consumo de cut tras éxito ni prueba nativa; no cerrar L-09A ni iniciar L-09B.
  Evidencia: `docs/VALIDACION-L09A-FORMATOS-2026-10-20.md`.
  Evidencia de 2026-10-14: `cargo check --tests` y Clippy cruzados con `-D warnings` para core y
  platform pasan sin warnings. Se limpiaron 10 warnings de imports/helpers Windows-only y tres
  `drop_non_drop` causados por tipos con destructores sólo en Windows; tests explícitos fijan los
  stubs de espacio, carpetas conocidas y Papelera. WSL Ubuntu no
  inicia (`HCS_E_HYPERV_NOT_INSTALLED`): `wsl --status` reporta Ubuntu como distro por defecto,
  pero su arranque pide habilitar Virtual Machine Platform y `HypervisorPresent=False` pese a
  que el CPU expone virtualización, monitor VM y SLAT. Consultar el estado de la característica
  exige elevación; no se cambió configuración ni se reinició. Tampoco hay Docker/Podman/QEMU/
  VirtualBox. Slint no puede
  verificarse sin sysroot/pkg-config Linux. La evaluación inicial GTK/AppIndicator examinó
  `libayatana-appindicator3-1` de `main`: está disponible para Noble 24.04 y Resolute 26.04, y Noble
  proporciona ambos sonames que prueba `libappindicator-sys`; esa ruta quedó superada por la decisión
  posterior de migrar a GLib antes de la alfa. Se verificó el `debian/copyright` de
  la versión Noble exacta desde el source tar oficial: la biblioteca de runtime permite LGPL-2.1 o
  LGPL-3; el paquete fuente también contiene partes GPL-3/BSD, que no deben atribuirse
  indiscriminadamente al binario. SHA-256 del `debian.tar.xz`:
  `90b6052050df827a82dfd7c875d933a828d67cf8c8712a924f415bf7b84e958a`. El upstream marca obsoleta
  la implementación GTK y recomienda otra API incompatible con los sonames actuales. Nicolás eligió
  migrar a GLib antes de la alfa y fijó Ubuntu 26.04 LTS x86_64 como mínimo, sin backport a Noble.
  Pendientes L-00/L-03: compilar y ejecutar el binding GLib en Ubuntu 26.04, validar degradación
  segura y compatibilidad de icono/menú/acciones en GNOME/Wayland y un derivado, y completar notices
  según dependencias reales del `.deb` y del ZIP.
  Evaluación GPL-3 del backend GLib solicitada el 15 de octubre: su upstream lo publica bajo GPL-3.0
  y `COPYING` contiene GPL v3 sin excepción de enlace visible. La guía FSF indica que enlazar una
  biblioteca GPL conduce a una obra combinada distribuida bajo GPL; el enlace dinámico no elimina
  esa obligación. La excepción GTK LGPL aprobada no cubre esta biblioteca. El 15 de octubre Nicolás
  autorizó que la edición Linux de la alfa se distribuya bajo GPL-3 si la migración GLib lo requiere,
  manteniendo Windows bajo MIT. Aún hay que auditar derechos de todos los colaboradores, definir
  fuente correspondiente y notices, y validar paquete; no se cambió dependencia ni licencia. Detalles
  y fuentes están en el plan Linux. La implementación y entrega Linux siguen detrás de las
  validaciones Windows pendientes. Preparación L-12 limitada: `ci.yml` tiene un job `linux-native`
  manual en `ubuntu-26.04` para tests core/platform nativos y check/Clippy de UI; pasó actionlint y
  el verificador de pins, pero no se ejecutó. Se conserva opt-in para no bloquear entregas Windows
  antes de cerrar L-03/L-04.
  Hallazgo L-00 del 15 de octubre: el catálogo oficial ofrece `libayatana-appindicator-glib2` en
  Ubuntu 26.04/Resolute, mientras que la fuente de Ubuntu 24.04/Noble encontrada corresponde a la
  API GTK antigua y no muestra el paquete GLib nuevo. Nicolás fijó 26.04 LTS x86_64 como mínimo de
  la alfa; no se planifica backport para Noble en esta fase. Falta validar en entorno nativo la
  integración y la matriz de derivados/escritorios. Evidencia y enlaces oficiales en el
  [plan Linux](plans/2026-10-13-linux-ubuntu-luna.md).
  Avance documental L-00: el candidato Resolute amd64 es `libayatana-appindicator-glib2` 2.0.1-1build1
  (Universe), con dependencias runtime directas en libc y GLib >= 2.80 y soname
  `libayatana-appindicator-glib.so.2`; el paquete de desarrollo incluye header, `.pc` y enlace `.so`.
  Se extrajo `debian/copyright` del source package tras comprobar el SHA-256 del tar contra el `.dsc`;
  confirma GPL-3 para el C runtime. La revisión del copyright binario y notices finales sigue pendiente.
  El 2026-10-17 Nicolás confirmó que la identidad histórica `nicolas_groth@yahoo.com` es su alias y
  que los 948 commits son suyos; queda resuelta la discrepancia con `ngroth@gmail.com`. `.mailmap`
  normaliza cómo Git los presenta sin alterar los commits, comprobado con `git check-mailmap`. Esto no
  reemplaza la revisión de origen y licencias de archivos importados. La implementación Linux se mantiene detrás de las validaciones
  Windows pendientes; este inventario no modifica el host ni habilita la bandeja.
  Actualización de evidencia L-00 del 2026-10-18: la firma del `.dsc` verifica con la clave de
  Sebastien Bacher vinculada a su huella Launchpad; la firma upstream `.asc` verifica con la clave
  Mike Gabriel del keyring Debian. El `InRelease` Ubuntu Resolute verifica con la clave de archivo
  oficial, su hash cubre el índice `Packages.xz` y el hash del `.deb` coincide con el stanza firmado.
  El `.deb` no tiene firma individual; Ubuntu usa la cadena de metadatos APT firmados. El verificador
  local PGPy no comprueba autosignaturas ni revocaciones; fuentes, hashes, resultados y límites están
  en `docs/VALIDACION-L00-AUDITORIA-2026-10-17.md`. Avance documental 2026-10-18: se preservó el
  texto `COPYING` y el `debian/copyright` autenticado, y se creó
  `THIRD-PARTY-NOTICES-LINUX.md` con procedencia y hashes. Esto completa solo el aviso preliminar
  del runtime candidato, no el inventario de la alfa. Siguen pendientes fuente correspondiente
  exacta del artefacto final, revisión de derechos, inventario de dependencias efectivas y validación
  runtime nativa.
  Revisión adicional L-00/L-03 (2026-10-16): el árbol oficial de Ayatana GLib en
  `4fa65d8ce1a7f1a1f22cefe3d5bdd47667f188d0` sólo aporta bindings Vala; el crate Rust existente
  documenta la API GTK obsoleta. Avance local L-03 del 2026-10-17: se añadió FFI GLib/AppIndicator
  mínima con `libloading`, ejecutada en worker y conectada a la UI por canal; `tray-icon` quedó
  condicionado a Windows. El módulo pasó `cargo check --target x86_64-unknown-linux-gnu --tests`
  y Clippy `-D warnings` dentro de un harness aislado, no del crate completo. El gate Windows del
  workspace pasó 1.447 tests (8 ignorados), Clippy y formato después de corregir una referencia al
  estado inicial de la bandeja y limitar `TrayMsg::Availability` a Linux. Esto verifica la
  separación Windows y tipos Rust, no el ABI/runtime ni el menú real. WSL sigue indisponible con
  `HYPERV_NOT_INSTALLED`; no se modificó configuración del host. Detalle y límites en el
  [plan Linux](plans/2026-10-13-linux-ubuntu-luna.md).
  L-02 parcial, 2026-10-16: producción resuelve `UserDirs` antes de crear la UI; Linux usa XDG
  config para settings/workspace y XDG state para logs; workers crean los directorios y preparan
  el límite de tamaño del log. Windows conserva sus cuatro ubicaciones junto al EXE. `$HOME`
  sustituye `USERPROFILE` para navegación Home Linux, y HOME inválido aborta el arranque con aviso.
  Hallazgo adicional corregido: el inicio y varios respaldos de panel todavía usaban
  `USERPROFILE`/`C:/`, así que en Linux podían apuntar a una ruta inexistente. Se reemplazaron por el
  resolver Home compartido, que respeta la preferencia del usuario; el test de recuperación y paneles
  de respaldo pasó en Windows. Pasaron también tests de rutas XDG, regresión portable Windows y worker
  de logs. La regeneración portable del 16-10 con el fix también abrió, creó sus archivos aislados y
  cerró con código 0. Pasaron los 1.399 tests del gate completo.
  `cargo check` y Clippy `-D warnings` cruzados de core/platform pasan. El check de UI cruzado se
  detuvo en `glib-sys` porque falta `pkg-config` con sysroot Linux. Un helper Windows-only de Papelera
  se condicionó a Windows/tests para que Clippy Linux no lo reporte como dead code. L-02 aún debe
  separar consumidores de data/cache,
  cerrar importación/recuperación y probar el flujo con filesystem nativo.
  Subpaso L-01A, 16 de octubre: listado, árbol y búsqueda de core escapan bytes POSIX no UTF-8 como
  `\\xNN` y duplican backslashes literales; ambas búsquedas usan la misma forma visible, mientras
  las filas conservan el `PathBuf` original. Pasaron 909 tests core y 22 flujos Windows; check/Clippy
  cruzados Linux compilan los tests POSIX. Falta ejecutar en Linux nativo. Guía y evidencia en el plan.
  Subpasos L-01A implementados: `path_input` conserva en POSIX espacios iniciales/finales, comillas
  y backslash sin límite UTF-16; guards de rename/duplicate/undo, sync, task spaces, búsquedas
  guardadas, retorno a selección, cesta, planificación de entregas y radar de destinos usan identidad
  nativa en Unix en vez de texto lossy/case-fold. Entregas comprueba solapamiento por componentes.
  El renombrado por lotes escapa bytes no UTF-8 en la etiqueta y bloquea esa fila; los nombres UTF-8,
  incluido backslash válido en POSIX, se mantienen intactos. Windows mantiene el criterio previo.
  Copy/move ahora usan identidad de ruta por plataforma en la detección de destinos anidados y no-op:
  Windows conserva case-insensitive, Unix distingue case/bytes. Un test de planificación con `A` y
  `a` verifica que POSIX permite copiar entre esos directorios. Pasaron 19 tests dirigidos de plan en
  Windows y `cargo check -p naygo-core --tests --target x86_64-unknown-linux-gnu --locked`; los tests
  Unix todavía requieren ejecución nativa. L-01A también corrige `is_valid_name` y la división de
  rutas relativas según POSIX (backslash literal, espacios/puntos finales permitidos) y `dedup_name`
  conserva bytes no UTF-8 al agregar sufijos. Once tests nativos Windows pasan; `cargo check` Linux
  con tests pasa, pero los tres tests Unix no pueden ejecutarse en este host sin WSL/VM Linux.
  La planificación de Duplicate ahora usa componentes `OsStr`/`OsString` al formar el nombre
  ` - copia`, preservando bytes inválidos y resolviendo colisiones sin descartar el origen. Pasó el
  test de colisión Windows; el caso Unix compila en el check cruzado y necesita ejecución nativa.
  Hallazgo adicional autorizado el 15 de octubre e incorporado a L-01A: `archive_ops::collect_entries`
  ya no convierte rutas con `to_string_lossy` ni reemplaza backslash globalmente. Conserva `\\`
  literal como carácter de nombre POSIX y rechaza nombres internos no UTF-8 antes de crear o truncar
  el ZIP. Dos pruebas Unix cubren ambos casos; compilan en el check cruzado, pero aún necesitan
  ejecución Linux nativa. La corrección también queda dentro de la distribución Windows regenerada.
  El subpaso de implementación ZIP queda marcado; su aceptación Unix nativa permanece pendiente.
  El gate volvió a pasar (1.393 tests, 7 ignorados, Clippy y formato); el check y Clippy cruzados de
  core para Linux pasan. Se regeneró el portable/setup 0.5.2 con pico de 10,42 GiB y hashes
  verificados en [la validación 2026-10-15](VALIDACION-BUILD-2026-10-15.md). Sólo falta ejecutar
  los dos tests nuevos en Linux nativo junto al resto de la matriz L-01A.
  Segundo subpaso L-01A autorizado: `Workspace::to_template` y `TemplateDir::Fixed` ya conservan
  `PathBuf`; las rutas no UTF-8 se rechazan antes de añadir/guardar la plantilla, con aviso
  localizado en los diez idiomas. Para rutas UTF-8, el JSON conserva la forma de texto anterior.
  El test de plantilla UI pasó (1/1), core workspace (110/110), i18n y checks/Clippy cruzados Linux;
  el test POSIX compila en `cargo check --tests`. La prueba no pudo enlazarse con `cargo test` cross
  porque el host no tiene linker `cc`; requiere ejecución Linux nativa. El gate completo pasó
  (1.393 tests, 7 ignorados, Clippy y formato) y se regeneró la distribución Windows bajo supervisor;
  hashes en la validación 2026-10-15.
  Hallazgo nuevo aprobado el 15 de octubre: el arranque cambia a `args_os()` y conserva rutas
  posicionales como `OsString`/`PathBuf`, evitando el panic de `std::env::args()` con bytes POSIX no
  UTF-8. La ruta se escapa sólo al formar avisos; flags y valores de tema/plantilla permanecen
  UTF-8. Pasaron 19 tests CLI Windows, check UI y Clippy/check cruzados Linux con los tests POSIX;
  éstos compilan pero aún requieren ejecución Linux nativa. El gate completo pasó y el portable/setup
  se regeneró bajo supervisor (pico 10,42 GiB); ZIP, EXE, hashes y logs constan en la
  [validación 2026-10-15](VALIDACION-BUILD-2026-10-15.md).
  El gate completo posterior aprobó 1.393 tests, 7 ignorados, Clippy y formato; la distribución se
  regeneró bajo supervisor (pico 10,41 GiB). ZIP, setup, EXE del portable y hashes están verificados
  en [la validación 2026-10-15](VALIDACION-BUILD-2026-10-15.md).
  Subpaso nuevo L-01A, 16 de octubre: `FilePanePersist.current_dir` conserva strings JSON históricos
  para rutas UTF-8 y usa `{encoding:"unix-bytes-v1",bytes_hex:"…"}` para bytes POSIX no UTF-8; el
  lector rechaza versiones/codificaciones mal formadas. El fingerprint de sesión ahora incluye la
  identidad nativa reversible, no texto lossy. Pasaron 3 tests core Windows, el gate de 1.400 tests
  y Clippy, además del check cruzado `cargo check -p naygo-core --tests --target
  x86_64-unknown-linux-gnu --locked`. Se regeneró portable/setup bajo supervisor (pico 10,42 GiB);
  hashes y captura opt-in constan en `docs/VALIDACION-BUILD-2026-10-16.md`. El round-trip de bytes
  POSIX y fingerprint exacto compilan como tests Unix, pero WSL no permite ejecutarlos aquí.
  Quedan la ruta física ante symlinks/cambios externos,
  transporte de bytes no UTF-8 por UI/clipboard y otros campos JSON, otros usos lossy y el resto de
  operaciones; esto no habilita UI ni operaciones Linux.
  Hallazgo de L-01A: las propuestas de nombres de grupo para entregas ahora usan el helper de
  presentación escapada POSIX, evitando `U+FFFD` y distinguiendo bytes no UTF-8 de backslashes
  literales. El test Unix pasa el check y Clippy cruzados para Linux; su ejecución nativa permanece
  pendiente. Se actualizó también la guía de entregas; el gate Windows completo y la regeneración
  supervisada de portable/setup 0.5.2 pasaron (pico 10,48 GiB). No se instaló. Ver
  `docs/VALIDACION-BUILD-2026-10-17-L01A.md`.
  Subpaso aprobado L-01A: `path_wire` extiende el JSON reversible a favoritos, cestas/referencias de
  espacios, búsquedas guardadas y puntos de comparación (raíces, registros, exclusiones y brechas).
  UTF-8 mantiene el formato string histórico; POSIX no UTF-8 usa objeto etiquetado con bytes hex.
  `comparison_point` también compara claves nativas y valida segmentos `.`/`..` sin convertir bytes
  inválidos a texto sustituto. Pasaron 944 tests core + 22 flujos Windows, `cargo check --tests` y
  Clippy cruzados Linux. Los tests Unix compilan, pero requieren ejecución Linux nativa. El gate de
  workspace y Clippy pasó (pico Job 8,53 GiB); portable/setup se regeneraron bajo supervisor (pico
  Job 10,48 GiB, ThinLTO 30m37s, setup 126,25s). Hashes y actualización instalada están en
  `docs/VALIDACION-BUILD-2026-10-17-L01A-JSON.md`. El drop manual desde 7-Zip sigue pendiente: el
  archivo de destino ya existía y la captura de ventana del helper de escritorio agotó timeout en
  dos intentos; no se tocó el fixture.

W-04 está implementado, probado y empaquetado. La sugerencia del modal Renombrar no consulta el
filesystem desde UI; la preconsulta del drop corre en un worker acotado, conserva errores de
acceso y el motor revalida conflictos al ejecutar. La suite completa UI (354 tests) y la
distribución regenerada pasaron; queda aceptación instalada en W-13.

Hallazgos nuevos revisados con Nicolás y agregados a W-06B: `op_paste` resuelve colisiones con
`exists()` en UI; reintentar/subir desde una carpeta perdida hace consultas de filesystem
síncronas; y `maybe_persist_session()` persistía `workspace.json` sincrónicamente desde tick.
Este último se corrigió en W-05 y permanece en W-06B como alcance aprobado de auditoría para
verificar persistencias restantes. El autocompletado de ruta y la importación de `.naygolist` ya
trabajan en workers, así que no se agregan como defectos.

Refinamiento de W-06B, 2026-10-20: las lecturas y escrituras del clipboard del sistema ahora usan
un worker único con cola acotada. Se incluye el lector de `op_paste` y de la Cesta, escritura de
Copiar/Cortar, rutas/texto/CSV/procedencia y texto del preview. El orden FIFO evita que una lectura
de pegar adelante una escritura de Copiar/Cortar; corte e historial se confirman sólo tras el éxito.
La prueba focalizada (2/2), gate (1.500 aprobadas, 8 ignoradas), Clippy, formato y check cruzado UI
Linux pasan. Distribución 0.5.3 supervisada regenerada y verificada; hashes y límites constan en la
evidencia de validación.

Auditoría de alcance autorizada por Nicolás, 2026-10-20: también se verificó la acción de copiar la
ruta del resultado de Entregas, que llama a `OpsCtrl::write_clipboard_text` y comparte el mismo
worker. No hay llamadas directas a `naygo_platform::clipboard` desde callbacks/controladores: el
único acceso al backend queda dentro de `ops_ctrl/clipboard_io.rs`. Se agregó una prueba que asegura
que la lectura asíncrona corre fuera del hilo solicitante; suite focalizada actual: 3/3.

El build nuevo está registrado en [VALIDACION-BUILD-2026-10-13.md](VALIDACION-BUILD-2026-10-13.md);
falta aceptación instalada en una máquina externa.

W-06A/B están implementados, probados y empaquetados. Los PNG externos, packs, logs ordinarios, y guardado o
borrado de temas usan workers acotados; el arranque pinta con catálogos embebidos y descubre los
catálogos del usuario de forma asíncrona, sin escanear `icons/` dentro de la carga de settings.
También se movieron a workers la colisión de pegado y las consultas de carpeta perdida. Las suites
actuales dieron 359 UI + 891 core + 22 flujos de operaciones aprobados. Artefactos y límites en
el [registro del build](VALIDACION-BUILD-2026-10-13.md).

W-05: `settings.json`, `keybindings.json` y `workspace.json` comparten un worker único con una
revisión pendiente coalescida por archivo. Configuración y guardado de sesión desde UI no hacen
I/O de disco; al cerrar, un coordinador fuera del event loop espera los tickets hasta 3 s y deja
que el watchdog cubra el desmontaje. Tres pruebas core del writer y los 354 tests UI pasaron; la
distribución se regeneró y verificó. Los errores del worker quedan en el log y la espera final
comunica el fallo. El instalador aún requiere aceptación manual W-13.

W-06C se aprobó para sacar del hilo UI la carga inicial de settings, idioma, atajos,
favoritos, plantillas y sesión, incluida la elección/persistencia del locale de primer uso; ya está
implementado y probado. La configuración embebida aparece al primer frame y la carga desde worker
merge de cambios durante la espera, cierre diferido y persistencias de favoritos/plantillas
asíncronas. Hallazgo adicional: la escritura atómica no creaba el directorio de configuración;
se corrigió dentro del worker y se agregó prueba. Suites: 365 UI, 893 core y 22 flujos. El build
supervisado de esta revisión y todos los tests del workspace pasaron. Clippy dejó 16 ocurrencias en
siete clases; Nicolás aprobó registrarlas como mantenimiento M-01. Siguiente: completar mediciones
W-07. RP-6 y aceptación instalada siguen abiertos; los planes de octubre no reactivan planes antiguos.

Preparación W-07: el benchmark sintético `large_directory` se ejecutó tres veces y registró medianas
en `docs/BENCHMARKS.md`. La instrumentación opt-in de tick, sincronización UI, listado y preview pasó
cuatro pruebas del colector y la suite UI (369 unitarias + 16 integración/render); ya está en el
build 0.5.2 supervisado y sus hashes están verificados en `VALIDACION-BUILD-2026-10-13.md`. La matriz
de UI, preview, memoria/CPU y filesystem local/UNC sigue abierta; los tiempos sintéticos no describen
el producto completo.

El 14 de octubre se agregaron contadores opt-in de ciclo de vida para workers reales de preview y
validación de rutas: iniciados, terminados, activos al cierre, pico simultáneo y rechazos por límite.
La instrumentación toma los mismos permisos que gobiernan los hilos y no registra rutas. Falta
instrumentar la cola serial de packs y validar el cierre con métricas habilitadas. Este delta pasó
Clippy y la suite UI (370 + 16), y quedó en la distribución supervisada con hashes verificados.

M-01: corregidas las 16 ocurrencias de Clippy en siete clases sin `allow` global. El gate con
`-D warnings`, todas las suites del workspace y el build supervisado de distribución pasaron.
El bloque quedó completado el 14 de octubre.

## Estado de partida

- Continuación autorizada del 14 de septiembre: auditoría de espacio (144,61 GiB de
  caché incremental identificada, eliminación bloqueada por la herramienta; no liberada).
  Dos declaraciones sin uso retiradas; nuevo perfil ThinLTO/16 unidades, símbolos y
  optimización de tamaño conservados, empaquetado con un solo trabajo. **1.324 pruebas,
  Clippy y siete pruebas del EXE release aprobadas**. Instalador/portable
  **0.5.2+build.202609142135** generados a las **22:04**, verificados; pico de build 10,44 GiB
  bajo límite 12 GiB. Sin instalar/publicar. [Auditoría/build](VALIDACION-BUILD-2026-09-14.md).

- Recuperación del 14 de septiembre: reinicio forzado durante el build, sin causa
  concluyente en eventos Windows. Código y distribución RP-5B preservados/verificados;
  1.324 pruebas y Clippy nuevamente aprobados. Los intentos protegidos de distribución
  iniciales alcanzaron límites de 12 y 16 GiB; se detuvieron antes del empaquetado.
  Ese corte quedó superado por la entrega ThinLTO indicada arriba. No se cambiaron ajustes
  del equipo ni se identificó la causa del reinicio original.
  Ver [registro RP-4B2](VALIDACION-RP4B2-2026-09-14.md) y [build protegido](BUILD.md).

- RP-4B2 **entregado localmente**: texto/resaltado y carga STL/3MF aislados,
  geometría transferida sin buffers duplicados, admisión temporal y scratch de raster
  acotados. Conserva giro 3D en memoria y copia parcial/resaltado. Pruebas y distribución
  se registran en [RP-4B2](VALIDACION-RP4B2-2026-09-14.md), con entrega 202609142135.
  RP-6, medición global y aceptación instalada siguen pendientes.

- RP-5B entregado localmente: modo económico configurable, imagen Slint compartida y
  presupuesto de retención imagen/texto/escenas. **1.317 pruebas**, Clippy, diez idiomas,
  formato/diff/grafo aprobados. Instalador/portable **0.5.2+build.202609100757**
  generados y verificados el 10 de septiembre a las **08:26**; cinco pruebas adicionales
  del lector release aprobadas. Entrega anterior respaldada; sin instalar ni publicar.
  Aquel corte no cerraba I/O bloqueado texto/3D ni temporales; RP-4B2 los continúa arriba.
  RP-6 conserva las mediciones globales y optimizaciones pendientes.
  [Registro RP-5B](VALIDACION-RP5B-2026-09-10.md).

- Entregado localmente **0.5.2**: selección al subir/crear, actualización de ramas, limpieza
  del modal, resaltado continuo y segundos/nombres únicos al pegar. Reconciliación de eventos
  durante listados y al finalizar operaciones. **1.308 pruebas**, Clippy, diez idiomas y
  formato/diff aprobados; render Slint inspeccionado. Instalador/portable **202609100054**
  generados y verificados (2026-09-10 01:19); cuatro pruebas adicionales del lector release
  aprobadas. Paquetes anteriores respaldados; sin instalación ni publicación remota.
  [Registro 0.5.2](VALIDACION-0.5.2.md).
  Este bloque corrige reportes del 10 de septiembre; no cierra presupuestos/modo económico RP-4/RP-5.

**Punto de continuación:** [estado local vigente](ESTADO-ACTUAL.md),
[índice documental](README.md) y [arquitectura](ARQUITECTURA.md).

- RP-5A entregado localmente: WebP/GIF/APNG animados, pausa/reanudación/repetición,
  reloj exclusivo del preview y flujo de crédito único. **1.301 pruebas** y Clippy aprobados;
  diez idiomas/formato/diff correctos. Instalador/portable **0.5.1+build.202609092333**
  generados y verificados (2026-09-10 00:04 local). Lector release: cuatro pruebas aprobadas,
  incluidos 55 frames de la muestra privada y cancelación en 30 ms. Sin instalación ni publicación remota.
  [Registro actual](VALIDACION-RP5A-2026-09-09.md).
  En ese corte (2026-09-10), RP-4 global/texto/3D, la política económica y RP-6 estaban pendientes;
  RP-4B2/RP-5B los continuaron después, según el registro vigente al inicio de este backlog.

- Entrega anterior RP-4B1: lectores imagen/SVG/PDF/comprimidos aislados, protocolo acotado y
  límites Windows. **1.287 pruebas**, Clippy, diez idiomas y prueba del EXE release aprobados.
  Instalador/portable **0.5.1+build.202609092205** generados y verificados (22:35); RP-4A respaldado.
  WebP de Nicolás es animado (50 frames, ciclo 5 s) pero en ese build seguía estático;
  En ese corte de RP-4B1 (2026-09-09), WebP seguía estático y texto/3D bloqueados; los cierres
  posteriores RP-5A, RP-4B2 y RP-5B se describen arriba.
  [Registro de continuación](VALIDACION-RP4B1-2026-09-09.md).

- RP-4A entregado: cancelación inmediata/reintento, dos trabajos de preview incluidos
  retirados, cámara pendiente única y suspensión por visibilidad. **1.274 pruebas**,
  Clippy, idiomas y controles Slint aprobados. Instalador/portable **0.5.1+build.202609092050**
  generados y verificados; RP-3 respaldado. [Validación RP-4A](VALIDACION-RP4A-2026-09-09.md).
  Siguiente: RP-4B (aislamiento/terminación no cooperativa y presupuestos completos).
  Aceptación instalada/minimización/DPI/Narrador/red pendiente; RP-4 no se cierra completo.

- RP-2: ruta superior implementada; 1.257 pruebas, Clippy, idiomas y render sintético
  aprobados. Instalador/portable **0.5.1+build.202609091819** generados y verificados.
  [Validación RP-2](VALIDACION-RP2-2026-09-09.md). Entrega anterior a RP-3.

- RP-3 entregado: seis paletas originales integradas, énfasis independiente y galería
  de íconos única. 1.263 pruebas, Clippy, diez idiomas y render software de quince temas
  aprobados. Instalador/portable **0.5.1+build.202609091919** generados y verificados;
  distribución anterior respaldada. [Validación RP-3](VALIDACION-RP3-2026-09-09.md).
  Pendiente aceptación instalada/DPI/Narrador. Entrega anterior a RP-4A.

- Revisión inicial local 2026-09-09: base 0.5.1 `abdef402` conservada; 1.246 pruebas aprobadas,
  seis ignoradas, Clippy/formato/diez idiomas correctos. Hashes de setup/portable/EXE
  coinciden con el build `202609071600`; ZIP íntegro. No se consultó GitHub ni se modificó
  la sesión instalada. Esta consolidación sólo cambia documentación, no la distribución.
- Nuevo alcance aprobado: [rutas, preview responsable y configuración](plans/2026-09-09-rutas-preview-y-configuracion.md).
  RP-0 cerrado: referencia recuperada y verificada en carpeta aislada. RP-1 implementado
  (editor/validación/autocompletado); 1.255 pruebas aprobadas, Clippy limpio, instalador y
  portable **0.5.1+build.202609091710** regenerados y verificados (entrega anterior a RP-2).
  RP-4B–RP-6 y aceptación instalada/DPI/AltGr/red pendientes.
  Se reutilizó selectivamente el editor/AltGr, sin merge del intento ni cambios de preferencias.
  [Evidencia y límites](VALIDACION-2026-09-09.md).
- El traspaso temporal se consolidó y se retiró; original en
  `target/agent-out/traspaso-original-20260909.zip`, fuera de Git. Identificadores y fallos
  comunicados se conservan en el plan, junto con la recuperación remota posterior autorizada.

## Historial de entregas (contexto, no lista de trabajo por rehacer)

Las menciones a working tree, commits pendientes o versión 0.5.0 describen el momento
de cada entrega. La integración posterior del 7 de septiembre las supera. Los pendientes
de aceptación manual siguen abiertos aunque la implementación esté terminada.

- Revisión 2026-09-07 implementada y validada: fuentes por función en Apariencia;
  refresco local/global; reparación del wake prematuro y descarte de lotes de watchers
  antiguos; selección de bandeja estable; íconos de operaciones/unidades centrados.
  Refresco global conserva ramas abiertas del árbol, también si desaparecen carpetas.
  1.246 pruebas aprobadas + smoke nativo del watcher; Clippy sin advertencias, formato/diff
  y diez idiomas correctos. Grafo: 7.550 nodos / 13.697 aristas.
  Se mantiene 0.5.1, build `202609071600`. Instalador y portable regenerados el 2026-09-07
  a las 16:28 / 16:27 (America/Santiago), con hashes y contenido verificados. Sin nuevas
  fuentes/dependencias. Esta entrega integra el trabajo anterior y esta revisión en `main`
  por fast-forward, sin reescribir historial. Pendiente validación instalada de Nicolás.
  Ver [validación de esta revisión](VALIDACION-2026-09-07.md).

- Respaldo solicitado publicado y comprobado en GitHub: `dcca05b1`, rama
  `fix/single-instance-y-bandeja`, 131 archivos. Incluye código, documentación y galería de demo;
  excluye tmp/ personal, configuraciones y artefactos generados. En ese primer respaldo
  todavía no se modificó main.
- Revisión posterior de 0.5.1 implementada y validada: etiquetas/acciones secundarias de bandeja,
  espacios recientes de sesión y filtro/reintento revisado de fallidos. 1.238 pruebas aprobadas,
  seis smoke ignorados; Clippy/formato/diff/diez idiomas correctos. Grafo: 7.498 nodos / 13.618
  aristas. Registros `post-github-verified-tests.log`, `post-github-clippy-final.log` y
  `post-github-graphify-final.log` en target/agent-out. Distribución regenerada y verificada:
  instalador del 2026-09-05 a las 22:31 y portable a las 22:29 (America/Santiago); hashes en
  [validación 0.5.1](VALIDACION-0.5.1.md). Publicada como `d3d492a5` en la misma rama.
  Pendiente sólo validación instalada de Nicolás para estos ajustes.

- Entregada **0.5.1** (petición 2026-09-05): rutas con ancho natural y extremo actual
  visible, acciones de bandeja arriba con ajuste de filas según ancho, vaciar separado abajo,
  MIT completa y seleccionable en Acerca de desde LICENSE. Sin nuevas dependencias.
  Validación: 1.230 pruebas aprobadas, 6 smoke interactivos ignorados; Clippy sin advertencias,
  formato/diff y paridad de diez catálogos correctos. Logs 051-validated-tests.log y
  051-final-clippy.log en target/agent-out. Render software inspeccionado a 900/360/200/110 px,
  con comprobación de ancho real, ruta compacta y clics de las ocho acciones al envolver.
  Grafo actualizado: 7.442 nodos / 13.526 aristas (051-graphify.log).
  Instalador regenerado a las 20:36 y portable a las 20:34, hora America/Santiago;
  checksums, integridad ZIP, versión del EXE y LICENSE empaquetada verificados.
  Registro: target/agent-out/051-release.log. Detalle y hashes: [validación 0.5.1](VALIDACION-0.5.1.md).
  Pendiente prueba instalada de Nicolás.
- Distribución histórica: **0.5.0**; instalador y portable regenerados el 2026-09-05.
  Sus cambios entonces sin commit quedaron incluidos en la integración del 7 de septiembre.
- Suite de recetas verificada 2026-09-05: **1.228 tests verdes** (más 6 smoke tests de Windows
  ignorados por requerir interacción). Registro final: `target/agent-out/recipes-final-tests.log`.
  Clippy aprobado con `--all-targets --locked -- -D warnings` (`recipes-final-clippy.log`).
  Séptima entrega regenerada y verificada: instalador del 2026-09-05 a las 18:32,
  portable de las 18:30, hora America/Santiago. Detalle y hashes en el plan.
- i18n: 10 idiomas con paridad verificada por `scripts/check_i18n_lang.py` (en CI).
- **Fix histórico ya integrado:** las 7 claves
  `slint.preview.{loading,cancel,cancel_tip,copy_tip,wrap_on_tip,wrap_off_tip,reset_view_tip}`
  se agregaron a los 8 idiomas que las perdían (pt/de/fr/hi/it/ja/ko/zh). Sin esto, el test
  `i18n::tests::todos_los_idiomas_tienen_las_claves_de_es` falla.
- **Cierre operativo 2026-08-28:** `scripts/build-release.ps1` ya tiene fallback SHA-256 con
  .NET para runtimes PowerShell sin `Get-FileHash`. El release del 2026-08-22 sí produjo ZIP e
  instalador; antes de este fix el script fallaba solo al escribir `SHA256SUMS.txt`, después de
  terminar ambos artefactos.
- **Cierre técnico 2026-08-28 (T-1):** `ops_ctrl` ahora es un módulo con raíz en
  `ops_ctrl/mod.rs`; los workers de archivo comprimido, borrado por lotes/Papelera y recuperación
  de journal viven respectivamente en `archive.rs`, `batch_delete.rs` y `resume.rs`. El wizard de
  sincronización y la bandeja ya estaban correctamente aislados en `workspace_ctrl/`.
- **Cierre técnico 2026-08-28 (T-2):** `scripts/smoke-ui.ps1` ejecuta los drivers interactivos de
  editor de ruta y scroll, exige capturas y rechaza panics nuevos en el log. Quedó documentado como
  gate manual post-build porque los runners alojados no ofrecen un escritorio Windows fiable.
- **Cierre técnico 2026-08-28 (T-3):** el ZIP portable ya no incluye `naygo.pdb` (bajó de ~70 MB a
  ~11 MB). El instalador ofrece los símbolos como tarea opcional y desmarcada; el PDB se sigue
  generando junto al binario para diagnósticos.
- **Cierre funcional 2026-08-29 (C-2):** el diálogo «nueva carpeta» mantiene `Enter` como salto
  de línea y confirma con dos `Enter` consecutivos dentro de 500 ms; `Ctrl+Enter` continúa como
  confirmación inmediata.
- **Cierre funcional 2026-08-29 (F-3):** la mini-barra del filtro por tipeo ahora alterna entre
  resaltar coincidencias y mostrar solo estas. El filtro actúa sobre la vista real del panel,
  conserva foco/selección por ruta y no se persiste ni altera los filtros por columna.
- **Cierre funcional 2026-08-29 (F-2):** `Ctrl+D` duplica la selección en su misma carpeta con
  nombres seguros (` - copia`, ` - copia (2)`, …), planificación asíncrona, journal y deshacer.
  Las configuraciones heredadas liberan el antiguo `Ctrl+D` de Favoritos solo cuando conservan
  exactamente ese valor histórico; los atajos personalizados distintos se respetan.
- **Cierre funcional 2026-08-29 (C-1):** `Ctrl+N` crea carpeta y `Ctrl+Shift+N` crea archivo.
  La carga del keymap migra únicamente el par de defaults anterior, sin alterar reasignaciones.
- **Corrección 2026-09-01 (C-1):** `Ctrl+N` vuelve a abrir el editor multilínea de
  «Nueva(s) carpeta(s)», idéntico al botón y al menú contextual. Así el estado vacío explica que
  falta una carpeta en vez de marcar un nombre inexistente como inválido, y se recupera la
  creación simultánea (incluidas rutas anidadas con `\\`).
- **Corrección 2026-09-01 (Papelera + preview):** la capa Shell de Papelera ahora valida el
  resultado de cada `DeleteItem` y que las rutas hayan desaparecido; si `IFileOperation` informa
  éxito sin retirar el archivo, reintenta mediante `SHFileOperationW` y nunca muestra una
  operación falsa como completada. El preview de texto reinicia viewport, selección y caret cada
  vez que llega contenido nuevo, por lo que abre desde el inicio y no mantiene una selección
  invisible del archivo anterior.
- **Ajuste 2026-09-01 (Papelera + preview):** el callback de resultado se corrigió a
  `PostDeleteItem` (no `PostMoveItem`) y un rechazo ahora queda registrado por archivo como
  fallido, en vez de aparecer engañosamente como «hecho: 0». Para lectura, la vista previa pasa
  de 100 a 250 líneas dentro del mismo máximo de 64 KiB y remata el contenido recortado con un
  borde vectorial de papel rasgado; ambos cambios mantienen el costo de memoria acotado.
- **Ajuste 2026-09-01 (Papelera + preview, segunda validación):** las rutas internas de Naygo
  del tipo `D:carpeta\\archivo` son válidas para `std::fs`, pero Shell las rechaza como
  `E_INVALIDARG`; antes de invocar la Papelera se resuelven ahora a rutas físicas con formato
  Shell (sin el prefijo extendido `\\?\\`). El preview usa un único editor multilínea de sólo
  lectura también para XML/código, con viewport explícito: permite rueda, barra de scroll,
  selección parcial y `Ctrl+C` en cualquier texto que se despliegue.
- **Cierre funcional 2026-08-31 (F-4):** `Ctrl+Shift+V` abre el historial interno de los últimos
  10 conjuntos copiados/cortados dentro de Naygo. Es efímero, deduplica entradas, no guarda rutas
  privadas en disco y pega mediante el mismo motor cancelable de operaciones.
- **Cierre funcional 2026-08-31 (F-1):** el menú contextual exporta la selección —o toda la vista
  filtrada si no hay selección— al portapapeles o a CSV. Respeta columnas visibles y su orden,
  agrega ruta completa, permite separador `;` o `,` y escribe BOM UTF-8 en archivos para Excel.

---

## A. Mejoras técnicas pendientes (importante aplicarlas)

### Mejoras de usabilidad posteriores a 0.5.1 — aprobadas el 2026-09-05

Nicolás pidió respaldar primero todos los cambios del proyecto en GitHub y después continuar
con estas recomendaciones. No incluye publicar muestras personales de tmp/ ni binarios en Git.

- [x] Opción «ícono + etiqueta» y menú de acciones secundarias en barras con íconos poco familiares.
- Acceso a espacios recientes sin indexación global ni abrirlos/ejecutarlos automáticamente.
- Filtrar fallos del historial y preparar un reintento sólo de fallidos, con revisión de fuentes,
  destinos y conflictos; no repetir a ciegas una operación completa ni los pasos ya completados.

Implementada previamente en la barra general: `icon_only` permite alternar entre iconos y etiquetas;
los botones con acciones secundarias muestran el chevrón de menú. Implementadas también: etiquetas
de bandeja reutilizan la preferencia persistida
«Solo íconos»; recientes limitados a diez referencias de sesión (con limpiar, sin I/O al renderizar);
reintentos sólo de archivos regulares fallidos de copiar/mover con origen conocido en el plan.
No se reintentan borrados, ZIP, carpetas, saltados, éxitos ni operaciones sin request recuperable.
Se conservan destinos exactos, se revalidan tamaño/mtime/rutas físicas al confirmar y el motor
consulta conflictos. No registra undo; el diálogo lo advierte. Los fallos no compatibles siguen
accesibles en el detalle. Referencia: [reintentos](REINTENTOS.md).

Referencia de interfaz: https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/command-bar

### T-4. Firma de código del instalador
**Preparación completada; activación externa pendiente.** `scripts/build-release.ps1` ya acepta
`SignToolPath` + `CertificateThumbprint` (o sus variables de entorno), firma el `setup.exe` con
SHA-256, timestamp y luego ejecuta `signtool verify`. Falta que Nicolás/ISGroth cree y autorice
un certificado (p. ej. SignPath.io) y entregue la identidad de firma; no se puede fabricar ni
usar una sin esa autorización. Mientras tanto, el bypass de SmartScreen está documentado en
`docs/DISTRIBUTION.md`.

---

## B. Funcionalidades aprobadas para implementar

### Plan 2026-09-09 — rutas, preview responsable y configuración

[Plan y matriz de aceptación](plans/2026-09-09-rutas-preview-y-configuracion.md).
Conservar la base local, recuperar el paquete sólo como referencia e integrar por bloques.
Prioridad: rutas de panel/superior → temas/configuración → ciclo de vida/límites →
animaciones → lectores/métricas → aceptación y distribución. No probar un esquema futuro
sobre preferencias del usuario. No aceptar límites de memoria provisionales sin medirlos.

- [x] RP-0 documental: auditoría local, requisitos consolidados y registro de evidencia ausente.
- [x] RP-0 fuentes: paquete localizado y verificado en carpeta aislada; comparación y uso
  selectivo como referencia, sin validar ni aplicar en bloque el intento experimental.
- [x] RP-1/RP-2: implementación de rutas editables coherentes, teclado/caret y validación fuera de UI.
- [ ] RP-1/RP-2 aceptación nativa: AltGr/IME, DPI/Narrador y UNC lenta/caída; paquetes RP-2 ya verificados.
- [x] RP-3: seis temas claros y configuración sin duplicación de íconos; build 202609091919 verificado. Aceptación nativa pendiente.
- [x] RP-4A/RP-4B1: cancelación/capacidad/visibilidad y aislamiento de lectores pesados; build 202609092205 verificado. No cierra RP-4 completo.
- [x] RP-5A: animación WebP/GIF/APNG, pausa/reanudación/repetición; build 202609092333 verificado.
- [x] RP-4/RP-5 implementación: RP-4B2 aisló texto/3D y acotó temporales; RP-5B entregó modo económico y retención acotada. Build 202609142135 verificado.
- [x] Mejora del visor STL/3MF (2026-10-17): órbita relativa a la cámara mediante orientación 3D
  completa; raster incremental sin divisiones por píxel; benchmark release supervisado mostró una
  mediana de 20,85 ms frente a 29,77 ms antes (-30,0 %) en fixture sintética. Gate completo
  (1.440 pruebas), distribución Windows 0.5.2 regenerada e instalada. La aceptación manual del gesto
  sigue pendiente.
- [ ] RP-4/RP-5 aceptación: instalación, medición de RSS global y escenarios de escritorio/red.
- [ ] RP-6/RP-7 lectores e instrumentación pendientes; verificar la siguiente distribución afectada.

### Plan 2026-09-04 — usabilidad y flujos de trabajo (aceptación parcial pendiente)

Plan detallado: [Usabilidad y flujos de trabajo](plans/2026-09-04-usabilidad-y-flujos-de-trabajo.md).
Solicitado por Nicolás; séptima entrega implementada y empaquetada (recetas declarativas).
El alcance completo de las siete etapas NO está cerrado.

Implementado en el working tree del 2026-09-04:

- Tinte tenue en la columna ordenada solo del panel activo, con prioridad de selección/alertas.
- Ruta acotada con ancestros accesibles y botones laterales reservados; comparación en segunda
  fila cuando falta ancho.
- Bandeja con Inicio/Fin/Página, scroll siguiendo foco y metadata básica asíncrona por ruta.
- Radar desplazable con flechas/Enter y fuentes capturadas al abrir; mover no vacía la bandeja
  antes de conocer el resultado.
- Maximizar/restaurar un panel con botón/paleta/Ctrl+Shift+M sin alterar el layout persistido.

Segunda entrega implementada: selección múltiple por identidad en bandeja (Ctrl/Shift/Ctrl+A,
Ctrl+flechas y arrastre del grupo), marcas persistentes entre paneles y alcance explícito de
acciones. Borrar confirma los originales; quitar/vaciar solo retira referencias. Los controles
están agrupados y tienen foco de teclado. Ctrl+V agrega referencias, no pega en otro panel.

Asistente de entregas implementado: carpeta nueva o ZIP desde marcados/Ctrl+P; grupos por fuente,
plano o raíz relativa explícita, revisión y revalidación, SHA-256 opcional, manifiesto/lista sin
rutas privadas, temporales propios, cancelación y deshacer únicamente del destino. Se reutilizan
los motores existentes; ZIP usa inventario congelado. [Guía y límites](ENTREGAS.md).

Tercera entrega implementada (2026-09-05): nombres de grupo editables, revisión conjunta de
homónimos y propuesta numerada explícita en modo plano, conservando todas las fuentes. Tarjeta
de resultado no modal con abrir carpeta de salida, copiar ubicación y recuperar las fuentes
marcadas en bandeja sin vaciar otras referencias. El historial conserva los errores de una
entrega fallida, en vez de perder el motivo y aparentar «hecho: 0». La detección de solapamientos
entre muchas fuentes pasa de comparar todas las parejas a búsquedas ordenadas O(n log n).

Configuración: búsqueda por etiquetas de ajustes, no solo por nombre de categoría; navegación
lateral con foco visible, Enter/Espacio y estado sin coincidencias. No consulta disco.
Quedan controles/configuración restantes (incluida restauración por sección), validación
visual/DPI/accesibilidad y mediciones. Etapa 4 implementada con validación instalada pendiente;
etapas 5–7 implementadas con validación instalada pendiente. El alcance completo NO está cerrado.

Cuarta entrega (2026-09-05): espacios por tarea `.naygospace` desde Disposiciones y Ctrl+P,
guardado/actualización explícitos, revisión antes de cambiar y raíz portable opcional. Conserva
layout, panel activo, columnas/filtros, tipeo y bandeja sin serializar listados ni preview.
La cola de operaciones conserva sus destinos capturados al cambiar. Archivos corruptos,
incompatibles o editados externamente no se sobrescriben; staging efímero se rechaza al guardar.
Conversión de layouts existentes aplicándolos y usando Guardar como. [Guía y límites](ESPACIOS.md).
El indicador de cambios se recalcula en el gestor, no en cada frame; abrir tarea usa el selector
nativo, sin catálogo residente ni exploración al arranque. ES/EN; nuevas claves usan inglés
provisional en los otros ocho idiomas (paridad no significa traducción editorial terminada).

Validación de cuarta entrega: 815 core + 22 integración + 40 platform + 285 UI = 1.162 pruebas,
6 ignoradas. Grafo AST: 7.046 nodos / 12.702 aristas; registro `spaces-graphify.log`.
Formato/diff y paridad de los diez idiomas verificados. Prueba de firma de filas desacoplada del
portapapeles global para que un bloqueo externo del SO no produzca fallos ajenos a su contrato.

Distribución de cuarta entrega terminada con `scripts/build-release.ps1`, ejecutado solo,
salida 0 (`target/agent-out/spaces-release.log`): release 25 min 40 s, Inno 85,5 s.
Instalador `dist/Naygo-0.5.0-setup.exe` de las 12:40:27 (49.512.070 bytes), portable de
las 12:39:01 (12.470.322 bytes). Hashes verificados contra `dist/SHA256SUMS.txt`; `7z t`
aprobado y ejecutable del ZIP idéntico al release (34.059.776 bytes, versión/autoría verificadas).
PDB de diagnóstico: 320.688.128 bytes en `target/release`, opcional en instalador y fuera del ZIP.
Sin dependencias nuevas; portable +145.762 bytes frente a la tercera entrega. Sin firma externa
y sin cerrar la instancia instalada de Nicolás: validación visual/DPI/Narrador pendiente.
Origen: working tree sin commit sobre `6ea74b9f39da`, rama `fix/single-instance-y-bandeja`.

Validación de la segunda entrega: `cargo test --workspace --locked` (1.142 aprobados, 6 ignorados),
`cargo fmt --all -- --check` y paridad de los 10 idiomas aprobadas.
`cargo clippy --workspace --all-targets --locked -- -D warnings` aprobado; grafo actualizado por AST
(6.924 nodos, 12.393 aristas). Logs: `delivery-validated-tests.log` y `delivery-final-clippy.log`
dentro de `target/agent-out`. Distribución de esta segunda entrega regenerada con
`scripts/build-release.ps1` (salida 0): instalador del 2026-09-05 a las 00:13
(48.621.974 bytes), portable de las 00:11 (12.233.915 bytes), hora America/Santiago. Ambos SHA-256
verificados contra `dist/SHA256SUMS.txt`; `naygo.exe` del ZIP coincide con `target/release`.
Versión y autoría del exe verificadas. Origen: working tree sin commit sobre `6ea74b9f39da`,
rama `fix/single-instance-y-bandeja`. Hashes completos en el plan enlazado arriba.
Registro: `target/agent-out/delivery-release.log`. Validación visual pendiente: no se cerró
la instancia instalada del usuario para forzar el smoke. Firma externa T-4 sigue pendiente.

Tercera entrega: suite completa con 1.147 aprobados (808 core + 22 integración + 37 platform +
280 UI), 6 ignorados; formato, diff y paridad i18n verificados. Grafo AST actualizado:
6.943 nodos / 12.467 aristas. Logs `target/agent-out/refinements-tests.log` y
`refinements-graphify.log`. Clippy final con `--all-targets --locked -- -D warnings` aprobado,
registro `refinements-final-clippy.log`. Distribución regenerada con `scripts/build-release.ps1`
(salida 0): instalador del 2026-09-05 a las 07:32 (48.987.054 bytes), portable de las 07:30
(12.324.560 bytes), hora America/Santiago. Ambos hashes contrastados con `SHA256SUMS.txt`,
ZIP legible y ejecutable idéntico al release (33.639.936 bytes); versión/autoría verificadas.
Log: `target/agent-out/refinements-release.log` (compilación: 23 min 48 s).
Hashes completos en el plan. Sigue sin firma externa y sin validación visual/DPI instalada;
no se cerró la sesión del usuario. Cambios aún sin commit sobre `6ea74b9f39da`.

- [ ] Etapa 0: base 0.5.0 reproducible e inventario de validación.
- [ ] Etapa 1: ruta adaptable, foco/teclado, propiedades completas de bandeja, accesibilidad,
  controles/diálogos adaptables y configuración fácil de encontrar.
- [ ] Etapa 2: maximización temporal implementada y probada en controlador; validar
  interactivamente restauración visual, pestañas y DPI con el nuevo instalador.
- [ ] Etapa 3: carpeta/ZIP, manifiesto, SHA-256 y refinamientos de grupos/homónimos/posentrega
  implementados. Falta validación manual de fallos/cancelación de archivos grandes. Los demás
  errores de I/O/ruta todavía se informan al encontrarlos, sin resolución masiva ni omisiones.
- [ ] Etapa 4: guardado/actualización/importación de espacios implementados y probados;
  falta validar instalado (diálogo/teclado/DPI/Narrador y red caída real). Referencias a búsquedas
  implementadas con etapa 5; recetas añadidas en etapa 7. Sin catálogo ni autoguardado de espacios.
- [ ] Etapa 5: consultas .naygosearch multiraíz implementadas, con tamaños/fechas relativas,
  ejecución explícita, cobertura parcial, selección/preview/propiedades/bandeja y referencias
  Ctrl+P/espacios. Validación automática aprobada: 1.181 tests, 6 ignorados, Clippy limpio.
  Logs queries-validated-tests.log y queries-validated-clippy.log en target/agent-out.
  Instalador/portable regenerados y verificados (2026-09-05, 14:44/14:42); build salida 0,
  ZIP íntegro, ejecutable idéntico al release y SHA-256 contrastados. Falta validación instalada/DPI/red real.
  Guía: docs/BUSQUEDAS-GUARDADAS.md. No hay índice residente ni ejecución al cargar o arrancar.
- [ ] Etapa 6: puntos .naygopoint implementados; captura/lectura/comparación/exportación/papelera
  en workers, SHA-256 opcional, exclusiones y cobertura desconocida explícitas, límites 50.000
  entradas / 32 MiB, 5.000 filas de cambios y envío sólo de archivos presentes a bandeja.
  Validación automática aprobada: 1.207 tests, 6 ignorados; Clippy limpio. Instalador y portable
  regenerados/verificados: 2026-09-05 a las 17:24/17:22 (build salida 0, ZIP íntegro, exe idéntico
  al release, versión 0.5.0 y SHA-256 contrastados). Log: target/agent-out/points-release.log.
  Pendientes aceptación instalada/DPI/Narrador,
  permisos/red reales y traducción editorial de los ocho catálogos con respaldo EN.
  Guía: docs/PUNTOS-DE-COMPARACION.md. No es respaldo ni incluye catálogo/indexador residente.
- [ ] Etapa 7: recetas .naygorecipe implementadas en working tree; editor de selección/consulta,
  filtros y fecha «este mes», parámetros de raíces/destino separados, revisión congelada,
  entrega carpeta/ZIP con nombres fechados, guardado protegido por revisión y referencias en
  espacios/Ctrl+P. Sin scripts, borrado de originales ni servicios residentes. Guía: docs/RECETAS.md.
  Validación final: 1.228 pruebas aprobadas (859 core + 22 integración + 47 platform + 300 UI),
  6 smoke tests interactivos ignorados; Clippy sin advertencias, formato/diff y paridad de diez
  catálogos aprobados. Logs recipes-final-tests.log y recipes-final-clippy.log en target/agent-out.
  Distribución regenerada/verificada: 2026-09-05 a las 18:32/18:30 (build salida 0, ZIP íntegro,
  exe idéntico al release, versión 0.5.0 y SHA-256 contrastados). Log recipes-release.log.
  Grafo AST actualizado: 7.426 nodos / 13.508 aristas (recipes-graphify.log).
  Pendiente aceptación instalada: teclado/DPI/Narrador, red/permisos y cancelación real.
  Ocho catálogos usan respaldo EN para las claves nuevas; traducción editorial pendiente.

Cada entrega que modifique la app requiere pruebas e instalador/portable regenerados mediante
`scripts/build-release.ps1`, ejecutado solo, y validación de Nicolás antes de la siguiente feature.

### F-5. Historial de navegación con memoria de contexto («Atrás de verdad»)
Al volver Atrás/Adelante, restaurar no solo la ruta sino también el archivo enfocado, selección,
posición de scroll y filtros activos de esa visita. El objetivo es retomar exactamente donde se
estaba sin reconstruir visualmente el contexto. Extender `core::workspace::NavHistory` con una
entrada de navegación compacta; no debe persistir listados ni provocar I/O adicional. Definir qué
parte del contexto sobrevive al reinicio y cómo degradar si los archivos ya no existen.

- **Cierre funcional 2026-08-31:** cada entrada conserva durante la sesión foco, selección,
  primera fila visible, filtros de columna y filtro visual. Se referencia por ruta (no por índice):
  al volver se reubica tras ordenar y se omiten archivos ausentes. Tras reiniciar solo sobrevive
  el estado de tabla/ruta ya persistido; no se guardan listados ni contexto efímero adicional.

### F-6. Radar de destinos para copiar/mover
Overlay navegable íntegramente por teclado que reúna paneles abiertos numerados, favoritos,
carpetas recientes/frecuentes y últimos destinos de operaciones. Elegir un destino con una tecla
y ejecutar copiar/mover sin navegar primero hasta él. Reutilizar el selector numérico de paneles,
`RecentDirs`, Favoritos y el historial de operaciones; el ranking debe ser local, determinista y
sin escaneo ni servicio residente.

- **Cierre funcional 2026-08-31:** F5/F6 (o las acciones configuradas de copiar/mover al otro
  panel) muestran un radar con hasta nueve destinos. Prioriza paneles abiertos, últimos destinos
  de operación, favoritos, frecuentes y recientes; deduplica rutas sin consultar disco. Se elige
  con 1–9, clic o Escape y reutiliza el motor cancelable de operaciones.

### F-7. Vista «qué cambió desde mi última visita»
Comparar el último listado completo conservado en memoria con el listado actual al volver a una
carpeta y marcar elementos nuevos, modificados y desaparecidos. Primera versión solo durante la
sesión, usando metadatos ya listados (nombre, tamaño y fecha), sin indexador ni vigilancia global.
Los desaparecidos se presentan como información, no como filas operables. Evaluar persistencia
opt-in únicamente después de medir memoria y utilidad real.

- **Cierre funcional 2026-08-31:** Naygo conserva el último snapshot completo de cada carpeta
  solo en memoria y compara nombre, tamaño, fecha y tipo al volver/listar otra vez. Las filas
  nuevas o modificadas se resaltan; el footer indica `+n ~m -d`, donde los ausentes (`-d`) son
  únicamente información y nunca se convierten en filas operables. No hay indexador, watcher
  global ni persistencia entre reinicios.

### F-8. Comparación entre paneles accionable
Extender la comparación rápida actual con acciones para ocultar iguales, seleccionar «solo aquí»
o «distintos» y copiar/mover ese subconjunto al otro panel. Agregar navegación relativa enlazada
opcional entre dos raíces: entrar a `sub/a` en un lado intenta abrir `sub/a` en el otro y muestra
discretamente si no existe. La comparación superficial debe seguir usando solo las entradas ya
cargadas; cualquier comparación recursiva pertenece al asistente cancelable de sincronización.

**Cierre funcional 2026-09-01:** permite seleccionar distintos/exclusivos, ocultar iguales de
la vista real y copiar/mover la selección desde el panel que originó la acción. El botón ↔ activa
opcionalmente el enlace relativo; sus marcas se invalidan al recargar, pero la relación persiste.

### F-9. Lente de procedencia de archivos de Windows
Mostrar bajo demanda los datos de `Zone.Identifier` (zona, URL de origen y referente cuando
existan) en Propiedades/Inspector, con acciones explícitas para copiar la información y desbloquear
el archivo. Implementar la lectura/escritura de ADS exclusivamente en `naygo-platform`, nunca al
listar carpetas: solo para la selección activa y en worker. Informar las limitaciones en volúmenes
sin ADS y exigir confirmación antes de quitar la marca de procedencia.

**Cierre funcional 2026-08-31 (F-9):** Inspector carga Zone.Identifier solo para el archivo
enfocado y desde worker. Permite copiar los datos resueltos y quitar explícitamente la marca tras
confirmación; la escritura ADS también ocurre en worker y luego refresca la metadata.

**Ajuste 2026-09-01:** un ADS presente sin ZoneId/URLs legibles igual mantiene habilitado el
desbloqueo; la confirmación nativa quedó localizada.

### F-10. Bandejas guardables como conjuntos de trabajo (`.naygolist`)
Guardar y abrir una bandeja como lista portable de referencias a archivos dispersos, sin copiar
los datos. Debe admitir rutas absolutas y, cuando haya una raíz común declarada, rutas relativas;
al cargar, conservar entradas ausentes marcadas como tales en vez de descartarlas silenciosamente.
Integrar con búsqueda, filtros, exportación y operaciones por lote. Definir primero un formato
JSON versionado, pequeño, inspeccionable y sin metadatos privados innecesarios.

**Cierre funcional 2026-08-31 (F-10):** `.naygolist` usa JSON versionado, admite raíz con rutas
relativas y conserva referencias ausentes. Al importar, un worker verifica disponibilidad y la
bandeja marca visualmente las rutas faltantes sin eliminarlas ni bloquear la UI.

**Ajuste 2026-09-01:** al guardar, la raíz se selecciona explícitamente y ya no se infiere del
panel activo.

### Preview de texto seleccionable

**Cierre funcional 2026-09-01:** el texto plano de Vista previa es solo lectura, pero permite
seleccionar fragmentos y copiarlos con `Ctrl+C`; el botón de la cabecera conserva copiar todo.

### Cierre funcional 2026-09-02 (árbol, preview, apariencia e íconos)

- El árbol común ahora sigue al panel Files activo y revela su carpeta cuando estaba fuera del
  viewport, sin modificar la navegación ni expandir ramas ajenas.
- La rueda permite recorrer la Vista previa de texto seleccionable (incluye TXT, XML y otros
  formatos de texto); se conserva el límite liviano de lectura y la señal visual de contenido
  recortado.
- Los sets Fluent y de color conservan su identificador al reiniciar. El editor de temas incorpora
  un token independiente para el fondo de la barra de herramientas, compatible con temas previos.
- Se incorporaron los sets compactos tintables Outline y Tiles (IDs estables `kenney-game` y
  `kenney-board`): 39 íconos por set, con licencia CC0 documentada en
  `assets/icons/KENNEY-SOURCES.md`; la colección fuente de `tmp/` no se empaqueta.

### Cierre funcional 2026-09-02 (SVG, imágenes y ZIP)

- Los SVG con `viewBox` o dimensiones declaradas respetan su canvas original. Si un export carece
  de ambos —como algunos assets de Flash/Animate con coordenadas negativas— el preview encuadra
  sus límites reales y evita el recorte. Al ser vectoriales se rasterizan hasta 2048 px para que
  no se pixelicen al ajustar el panel; ese mismo tope limita la textura a ~16 MiB.
- Las imágenes estáticas disponen de dos controles locales: ajustar al panel (inicial) y `1:1`.
  El modo natural no amplía fotos pequeñas, las centra y habilita scroll solamente cuando exceden
  el área visible. No hay I/O ni decodificación en el hilo de UI.
- El árbol de ZIP/TAR alinea los tamaños en una columna monoespaciada a la derecha, calculada a
  partir de todas las rutas mostradas. Se mantuvo el árbol ASCII y se descartaron puntos de relleno
  para conservar una lectura limpia.

### Cierre funcional 2026-09-02 (ajustes de controles y configuración)

- El control de escala del preview de imagen es ahora un conmutador único: muestra ↔ cuando la
  imagen está ajustada y `1:1` cuando se ve a tamaño natural; el tooltip anuncia el modo que se
  activará al pulsarlo.
- El editor de temas agrupa los tokens por superficies y paneles, filas y tablas, texto,
  interacción/foco y alertas. La representación no altera los índices internos ni los temas ya
  guardados.
- Los sets antes rotulados por su fuente se presentan como Outline y Tiles, manteniendo sus IDs
  persistentes. Se añadieron Vivid y Pastel: dos sets CC0 de 39 íconos multicolor, con paleta
  semántica por tipo de archivo o acción y sin tintado por tema.

### Cierre funcional 2026-09-02 (productividad y bandeja temporal)

- Se verificaron y conservaron los tres flujos de productividad ya presentes: paleta de comandos
  con `Ctrl+P`, ayuda contextual de atajos con `F1` y disposiciones de workspace guardables desde
  el menú Layouts. No se duplicaron con una segunda implementación.
- Copiar/Mover desde Bandeja abre el radar de destinos y ofrece primero los paneles Files visibles;
  mantiene «Elegir otra carpeta…» como alternativa explícita. Arrastrar una fila de la bandeja a
  un panel abierto usa el mismo OLE drag seguro que el resto de la aplicación.
- La fila elegida se destaca y alimenta Vista previa, Propiedades y metadata asíncrona. Las acciones
  de la bandeja son ahora compactas, basadas en íconos y tienen tooltip, incluido quitar una fila.

### Cierre funcional 2026-09-02 (0.5.0: continuidad de preview y bandeja)

- El selector de escala de imágenes mantiene el último modo elegido —ajustar o 1:1— al navegar
  entre archivos, sin afectar el comportamiento de scroll del tamaño natural.
- Con la Bandeja temporal activa, las flechas ↑/↓ recorren sus ítems y conservan su contexto como
  fuente de Preview, Propiedades y metadata; no recuperan la selección del último panel Files.
- La versión de distribución pasa a **0.5.0**. Todo cambio de la aplicación debe cerrar con un
  nuevo `setup.exe` y ZIP portable para validación manual.

---

---

## D. Bugs/mejoras de arrastrar y soltar

### D-1. Arrastrar dentro del MISMO panel sobre una carpeta (mejora futura)
Hoy el drop intra-panel no mueve/copia a subcarpetas. Implementar con **las mismas reglas que
el drop entre paneles** (copiar por defecto, mover con Shift/mismo volumen, menú con clic
derecho si aplica). Punteros: el drop entre paneles está resuelto en
`crates/ui-slint/src/workspace_ctrl/ops.rs` (`drop_at` y siguientes); extender el hit-test de
`body-touch` en `file-panel.slint` para detectar «fila carpeta destino» dentro del mismo panel.

**Cierre funcional 2026-08-31 (D-1):** el hover OLE ahora cruza su coordenada real con el
`ListView` del `FilePanel`, incluyendo scroll, y reporta la fila de vista bajo el cursor. Si es
una carpeta, `drop_at` la resuelve como destino antes de evaluar las reglas existentes; por tanto
Ctrl/Shift, mover por mismo volumen, confirmación y conflictos se comportan igual que entre
paneles. Se rechaza una carpeta sobre sí misma o dentro de su propio árbol.

---

## E. Hallazgos nuevos pendientes de priorización

### [x] E-1. La columna «Extensión» interpreta como extensión el sufijo de carpetas con punto
**Corregido y en distribución local 0.5.2.** La tabla y el CSV muestran la etiqueta localizada
«Carpeta» para cualquier `Directory`; el filtro y sus conteos agrupan todas las carpetas bajo una
clave interna reservada imposible en una extensión válida. La ordenación usa esa misma categoría,
independiente del sufijo del nombre, tanto ascendente como descendente. Archivos sin extensión siguen
en la categoría vacía. `Other` (por ejemplo, enlaces o dispositivos) conserva la extensión si la
tiene y cae en «sin extensión» si no; no se interpreta como carpeta. Se añadieron regresiones para
carpetas con uno o varios puntos, archivos con/sin extensión, filtro/conteo, orden bidireccional y
exportación CSV. La guía y el changelog quedaron actualizados; gate y empaquetado constan en
`docs/VALIDACION-BUILD-2026-10-17-E1.md`. Revalidación focalizada 2026-10-20:
`bridge::tests::extension_cell_does_not_treat_dotted_folder_as_file_type` pasó 1/1.

### [x] E-2. El drop desde 7-Zip puede cerrar Naygo al iniciar la copia
**Corregido y empaquetado el 2026-10-17.** El registro muestra que el archivo llegó a `drop_at` y
al inicio de la copia, pero Naygo panicó con `RefCell already borrowed` en
`crates/ui-slint/src/sync.rs:174`. El tick llamaba a `sync_rows` desde el brazo `Started` mientras
seguía vivo el préstamo mutable de `pump_drop_collision_probe()`. También se separó la limpieza del
resaltado de arrastre de su sincronización, que tenía el mismo riesgo de reentrada. Ahora se liberan
ambos préstamos antes de sincronizar. Pasaron las pruebas del workspace, Clippy y el build
supervisado; la distribución 0.5.2 se regeneró y se instaló con autorización previa. En la revisión
del 2026-10-17, el EXE instalado coincidía con el release (SHA-256 `97bd204b…`), PID 12232
respondía y el único pánico de ese log era el de las 12:58, anterior al arranque corregido de las
16:45. La carpeta destino contiene un OBJ con fecha anterior al incidente, lo que no demuestra que
esa operación lo copiara. Pendiente: aceptación manual con el fixture ZIP y comprobación del
contenido copiado. Revalidación del 2026-10-17: las 7 pruebas de `drop_at` pasan; el EXE instalado
coincidía entonces con el EXE instalado de E-3 (`357ee82a…`); el proceso PID 38744 seguía en esa
versión. La nueva distribución 0.5.2 lleva el fix L-11 (`ed1f3491…`). **Actualización posterior,
2026-10-17 21:16:** se cerró PID 38744 limpiamente, se ejecutó el setup con salida 0 y se dejó L-11
abierta como PID 56616. El EXE instalado coincide byte por byte con `target/release/naygo.exe`.
`settings.json`, `workspace.json`, `favorites.json`, `templates.json` y el PDB conservaron sus
SHA-256; el PDB preexistente se respaldó en
`target/agent-out/l11-install-preserve-20261017-211618-8d2bd046` y no se reemplazó. UI Automation
leyó 691 descendientes, 514 con nombre y 55 enfocables; el proceso tiene ventana principal. El único
`PANIC` del log del día fue el de las 12:58, anterior a la instalación; no hubo marcadores
`error`/`fatal` posteriores. Esto aún no sustituye el arrastre real desde 7-Zip, la comprobación del
contenido/hash ni Narrador. Gate y hashes de L-11 en
`docs/VALIDACION-CIERRE-SEGURO-2026-10-17.md`.
Para hacer repetible la aceptación manual se añadió `scripts/prepare-e2-7zip-fixture.ps1`; crea
un ZIP inocuo, destino vacío y hash esperado bajo `target/agent-out/`. La fixture del 2026-10-17
contiene `E2 fixture/sample.obj` + `readme.txt`; entradas, hash y destino vacío se verificaron.
7-Zip está instalado en `C:\Program Files\7-Zip\7zFM.exe`. Aún falta arrastrar tanto la carpeta
como el OBJ desde esa app y comprobar contenido, hash, historial y que Naygo siga respondiendo;
la fixture no prueba por sí sola el flujo OLE efímero del archivador. Revalidación local del
2026-10-17: `7z.exe t` validó el ZIP (2 entradas, 403 bytes) y `7z.exe x` extrajo los dos archivos
al destino desechable; el SHA-256 del OBJ coincidió con el esperado y Naygo siguió en PID 56616.
Esto valida el fixture y la extracción con 7-Zip CLI, no el drag & drop de su ventana.
`cargo test -p naygo-ui-slint --locked drop_at` pasó 7/7 y
`cargo test -p naygo-platform --locked trash::tests` pasó 8/8, incluidos resultados parciales,
recibos por origen y junctions; son pruebas automatizadas, no sustituyen la aceptación OLE manual.
Revalidación 2026-10-18: las mismas ocho pruebas de Papelera y las siete de `drop_at_` pasan; la
regresión `hdrop_temporal_de_7zip_se_detecta_y_se_snapshottea` pasó 1/1. La instancia PID 25124
responde, pero `LogonUI` sigue activo, así que el arrastre GUI, W-07 y W-13 continúan pendientes.
Detalle en `docs/VALIDACION-WINDOWS-0.5.2-2026-10-15.md`. Tras instalar E-6 el 2026-10-18,
la instancia vigente es PID 41292, responde y su EXE coincide con Release (`54fed15a…`). UIA
pasiva expuso 751 descendientes y 79/79 controles enfocables con nombre. El intento de captura de
ventana con Computer Use expiró dos veces tras renovar la referencia; no se enviaron clics ni
arrastres. La fixture de octubre 17 ya tenía su destino poblado por la extracción CLI anterior, así
que se generó una nueva fixture el 18 (`target/agent-out/e2-7zip-fixtures/20261018-170257-2906b077`):
el archivo ZIP pasó `7z t`, tiene dos entradas y el destino quedó vacío. Sigue pendiente reproducir
el drag real desde la ventana de 7-Zip, comprobar ambos archivos y el hash, y confirmar que Naygo
permanezca respondiendo. Revalidación automatizada posterior: las ocho pruebas Windows de
`naygo-platform::trash::tests` y las siete regresiones `naygo-ui-slint` filtradas por `drop_at`
pasaron en el checkout actual; esto no sustituye la prueba OLE real.
Revalidación focalizada 2026-10-20: las siete pruebas `drop_at` pasaron; y
`drop_target::windows_impl::tests::hdrop_temporal_de_7zip_se_detecta_y_se_snapshottea` pasó 1/1.
La instancia instalada PID 43624 sigue en la sesión bloqueada; por ello, el arrastre OLE real,
W-07 interactivo y W-13 manual continúan pendientes.

---

### [x] E-3. El menú rápido de favoritos no muestra la carpeta destino
**Corregido, empaquetado e instalado el 2026-10-17.** El menú de la estrella de toolbar muestra ahora a la
derecha del alias la ruta abreviada, con tipografía menor y atenuada, igual que el panel Favoritos.
Reutiliza `path-hint` ya calculado para el árbol; abrir el menú no hace I/O. La ruta completa sigue
siendo la usada para navegar. Prueba de datos cubre rutas de hojas y grupos sin ruta; el gate
completo pasó 1.447 pruebas (8 ignoradas), Clippy `-D warnings`, formato y empaquetado supervisado.
ZIP/setup verificados e instalación 0.5.2 abierta/respondiendo. La validación manual de aspecto en
pantalla y la aceptación del usuario siguen pendientes. Evidencia:
`docs/VALIDACION-FAVORITOS-2026-10-17.md`.

### [x] E-4. Selector rápido de espacios/perfiles (Trabajo, Vacaciones, etc.) — implementado localmente; aceptación manual pendiente
Nicolás propone guardar la disposición visible completa bajo un nombre y poder cambiarla desde un
ícono o panel: paneles y pestañas abiertos, rutas de cada panel, proporciones, vista/columnas,
filtros, preview y referencias de la bandeja. Es un buen caso de uso para los `.naygospace`
existentes, que ya conservan la estructura, rutas, columnas, filtros y bandeja; no conviene crear un
segundo formato ni duplicar el gestor. La lista rápida y el historial local persistente ya están
implementados; E-6 les da un acceso propio en la toolbar, separado de **Disposiciones**. La transición aplica
directamente si la sesión está limpia y conserva la revisión guardar/descartar/cancelar ante cambios.
El catálogo guarda hasta diez referencias explícitas en `settings.json` y muestra cinco en el selector
de Espacios; cargarlo no
abre los documentos ni explora carpetas al iniciar. Limpiar la lista no borra los `.naygospace`.

Nicolás aprobó elegir al crear cada espacio entre Favoritos globales o una lista propia. Implementado
en JSON v2: los archivos v1 se leen como globales; la lista propia se inicializa como copia del árbol
global y se guarda dentro del `.naygospace` al actualizarlo. El cambio de Favoritos privados marca el
espacio como pendiente y no modifica `favorites.json`. El selector de Espacios muestra
hasta cinco espacios recientes con nombre y carpeta, y conserva diez referencias MRU en
`settings.json`. La carga del catálogo ocurre en el worker inicial; no escanea carpetas ni abre los
documentos. Si la sesión actual está limpia aplica al terminar de leer; ante cambios abre la revisión
con guardar/descartar/cancelar. Limpiar recientes quita referencias sin borrar archivos. No agrega
búsqueda de carpetas al inicio. El gate completo y la distribución Windows quedaron registrados en
`docs/VALIDACION-E4-CATALOGO-2026-10-18.md`; la build se instaló in-place, los datos se preservaron y
la app quedó abierta (PID 25124). Aceptación visual del diálogo y del menú y prueba con rutas ausentes
o remotas siguen pendientes (se conservan y cada panel presenta
su error normal). Preview e información derivada se vuelven a cargar; no se guardan bytes de preview
ni listados en el perfil. Guía actual: `docs/ESPACIOS.md`; evidencia:
`docs/VALIDACION-E4-FAVORITOS-2026-10-18.md` y `docs/VALIDACION-E4-SELECTOR-2026-10-18.md`.

### [x] E-5. Agrupar iconos de la barra para mejorar descubrimiento y orientación
**Implementado, gate completo, regenerado e instalado el 2026-10-18; aceptación visual del usuario pendiente.** La toolbar ya tenía los
controles, tooltips y separadores; el problema era la jerarquía y posición. Ahora se lee de
izquierda a derecha: navegación → ubicación → búsqueda/visibilidad/recientes/favoritos → paneles y
disposiciones → acciones → unidades → Configuración. La ruta flexible va junto a navegación y
Configuración al extremo derecho. Los separadores conservan el color del tema y ganan dos píxeles de
aire, sin añadir operaciones, lecturas ni coste de CPU. La guía explica el orden y recuerda que los
tooltips identifican controles en modo solo iconos. Pendiente revisar el resultado en pantalla a
distintos anchos/temas con la distribución E-4 abierta (PID 25124); la automatización no sustituye
esa aceptación visual.
Evidencia: `docs/VALIDACION-TOOLBAR-2026-10-18.md`.

### [x] E-6. Toolbar adaptable y accesos separados para Espacios/Disposiciones
**Implementado, gate completo, distribución regenerada e instalada el 2026-10-18; aceptación visual/UIA pendiente.** En ventanas angostas la barra
conserva visibles navegación, ruta, búsqueda, favoritos, Espacios y Disposiciones, sin crear una
segunda fila. Las acciones menos usadas, unidades y Configuración pasan a «Más»; su lista permite
navegar a unidades, refrescarlas y expulsar las extraíbles. Espacios tiene botón propio y el menú de
Disposiciones queda dedicado a plantillas, recetas y puntos de comparación. El panel «Más» puede
recorrer sus opciones con scroll. El gate pasó con 1.458 pruebas (8 ignoradas), Clippy y formato; el
check y Clippy cruzados Linux también pasaron. Build supervisado: pico 10,63 GiB, release 27m03s e
Inno Setup 112,859 s; portable/setup y EXE verificados. El setup se instaló silenciosamente tras
comprobar sus hashes con el manifiesto; el EXE instalado coincide con Release (`54fed15a…`) y la
instancia PID 41292 responde. El setup permanece sin Authenticode. Falta aceptar visualmente en
ancho compacto/amplio, con varias unidades y temas, y revisar el menú con UIA/Narrador desde sesión
desbloqueada. Hallazgo nuevo relacionado: el botón de Espacios usa el icono de paneles clonados del
set activo, ya que aún no existe un glifo propio; valorar añadir uno semántico a los packs.
Evidencia: `docs/VALIDACION-E6-TOOLBAR-RESPONSIVE-2026-10-18.md`.

> Cobertura de regresión añadida el 2026-10-18: test headless del `AppWindow` verifica los breakpoints
> de toolbar a 1600/1300 px en modo con etiquetas y 1100/1000 px en solo iconos. Gate actualizado:
> 1.459 tests aprobados, 8 ignorados; Clippy y formato aprobados. Como el cambio añade solo código
> bajo `cfg(test)`, no altera el artefacto de distribución ni requiere regenerarlo.

## Notas de operación para la próxima sesión

- Verificación estándar: `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets` (cero warnings), `cargo test --workspace` (todo verde), paridad i18n con
  `scripts/check_i18n_lang.py` para los 10 idiomas al tocar claves.
- Tras cada fix que afecte la app: rebuild de `dist` con `scripts/build-release.ps1` (el
  usuario prueba instalando desde `dist/`; ver el punto «Cómo trabajar» en AGENTS.md).
- **Corrección 2026-09-01 (D-2):** 7-Zip en esta máquina entrega `CF_HDROP` con archivos
  temporales `Temp\\7zE…`, no contenido virtual; borra esa ruta al salir de `Drop`. Naygo ahora
  abre esos handles antes de retornar al archivador y un worker los copia a staging propio, por
  lo que la planificación asíncrona no depende de una ruta efímera. Si una fuente ofrece contenido
  virtual también se prioriza y materializa. Se corrigió además un préstamo `RefCell` en el toast
  de error que podía cerrar Naygo al informar `SourceUnreadable`. Revalidar manualmente archivo +
  carpeta desde 7-Zip y WinRAR, conflicto/cancelación y Explorer (`CF_HDROP`) con el build de
  distribución resultante.
- La máquina del usuario estuvo bajo presión de RAM (~700 MB libres): correr builds pesados
  (release con LTO) SOLOS, sin otros trabajos en paralelo.
- **Material web 2026-09-05:** generadas y revisadas 20 capturas reales de Naygo 0.5.0
  en seis temas, con datos ficticios y TXT de explicaciones/alt. Entrega local:
  `%USERPROFILE%\Pictures\Screenshots\Naygo-web-20-capturas.zip`.
  Corpus y copia portable separados en `D:\Naygo-Demo`; sesión instalada restaurada
  en segundo plano. No se modificó la aplicación ni se requirió rebuild de distribución.
  Ampliación: material ubicado por el usuario en `assets/screenshots`; agregadas
  variantes multipanel 21–22 (oscura y clara), anonimizadas mediante edición generativa
  de su referencia. Guía y notas distinguen estas ilustraciones de las 20 capturas
  reales. Revisadas visualmente; referencia privada excluida de la entrega.
- `scripts/run-logged.sh` envuelve comandos pesados (deja salida + curva de RAM en
  `target/agent-out/` y bitácora en `logs/agent-bitacora.md`); útil ante caídas del entorno.
> W-12 revalidado en solo lectura el 2026-10-16: pins locales pasan; Actionlint v1.7.12 del release
> oficial pasó ambos workflows tras verificar SHA-256 del ZIP contra el checksum publicado. Actions
> sigue sin corridas nuevas (última Preview implementation review 9-09, failure; último CI 9-07,
> failure). No se publicó nada; runner Windows Server 2025 continúa pendiente.
