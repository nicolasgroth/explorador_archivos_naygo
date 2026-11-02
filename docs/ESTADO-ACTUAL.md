# Naygo — estado local y punto de continuación

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Distribución local instalada — 2026-11-02

La compilación Windows 0.5.4 de `2319e51d` pasó el gate conjunto: 1553 pruebas,
9 ignoradas, Clippy y formato correctos. El build supervisado terminó con pico de 10,74 GiB.
Setup y ZIP están sin firma Authenticode. Setup terminó con código 0; el EXE instalado, el del
ZIP y `target/release/naygo.exe` coinciden. Settings, workspace, favoritos y plantillas se
conservaron byte a byte al instalar. Naygo se inició y respondió como PID 34208. Inspección
visual manual y arrastre experimental de carpetas de 7-Zip siguen pendientes.

- Setup: `df7f645a560a04cdf356fd07ad125c7ce03bbd4fd7d5c6a57283f2970b8e1daf`.
- ZIP: `58b91ec65f435f37d2bfdd4f3175abcb4887ed21908ee4034b4ceb12cd8b6d1b`.
- EXE: `8341510e8f285fbcd2a9f26dfe9ea297f2c977d483af23925f277580925a3793`.

## Registro histórico: distribución local de avance — 2026-11-01

La instalación abierta en este PC declara versión 0.5.4 y su ejecutable tiene SHA-256
`17c8c55c1e321428b8aafba33c5ffe12ad8934064342af469ce8313d29b74dd8`, coincidente con el release
de avance anterior. No es el ejecutable recién generado indicado más abajo ni incorpora los fixes
de recuento, efecto OLE o la rama experimental de carpetas 7-Zip.

El build supervisado posterior a `26847a17` terminó con pico de 10,72 GiB y generó setup y ZIP
0.5.4 sin firma. Incluye la presentación de errores fatales, validada localmente con 1.534 pruebas,
nueve ignoradas, Clippy y formato. No incluye aún el arrastre experimental de carpetas de 7-Zip ni
el recuento diferenciado; no se ha instalado esta compilación. SHA-256:

- Setup: `df8755ad2ef64b276f38c65c5470dfe2f7bf061f9488eac0478487589c307bd9`.
- ZIP: `37527931b27c58e1aa673660ee539d86e04b818cb9b86e98b25915016864466e`.
- El `naygo.exe` del ZIP coincide con `target/release/naygo.exe` (SHA-256
  `80fdd42b686cb7f33397789fd153c6011ba73c9e2aa4fe472ffda8468fac48e0`); el ZIP no contiene PDB.

## CI remota — primera evidencia nativa, 2026-10-31

La corrida [41204625942](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/41204625942)
se lanzó sobre `68d3fc38` después del respaldo remoto. En Ubuntu 26.04 el job `linux-native` falló
en las pruebas del core: 970 aprobadas y 27 fallidas. Los fallos incluyen supuestos de rutas Windows
y pruebas que comprueban persistencia de rutas no UTF-8; todavía no se ha atribuido cada uno a un
defecto de producto o de test. El gate Windows y el benchmark aún ejecutaban al registrar esta nota.
En el test de búsquedas guardadas se comprobó una causa de prueba: busca el fragmento JSON compacto
`"encoding":"unix-bytes-v1"`, mientras el serializador usa formato pretty con espacios; el test
falla antes de verificar el round-trip y no demuestra por sí mismo pérdida de datos.
Logs del job Linux: `target/agent-out/ci-linux-41204625942.log` (local, no versionado).
El gate Windows de esa misma corrida terminó fallando en dos tests de Papelera: faltó la identidad
directa de un archivo reciclado y no se encontró el junction para restaurarlo. El benchmark Windows
pasó; Clippy no se ejecutó en el gate. El código actual clasifica la desaparición del origen como
recibido aun sin identidad confirmada. La API documenta que un `psiNewlyCreated` nulo puede indicar
eliminación completa; el log disponible no distingue ese caso del fallback u otras causas. Se debe
diagnosticar y corregir sin afirmar éxito ni restaurabilidad sin pruebas.
Se añadió diagnóstico solo en pruebas, sin rutas ni nombres personales, para distinguir callbacks,
identidad, resultado HRESULT y uso del fallback. Las 11 pruebas de Papelera y Clippy de platform
pasaron localmente; falta ejecutar el diagnóstico en el runner.
La corrida diagnóstica [41208415922](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/41208415922)
confirmó objeto nuevo y nombre de parsing en ambos casos, sin fallback; falló la asociación con la
ruta original. La corrección local asocia el nombre Shell leído antes de borrar, preserva UTF-16 y
libera buffers COM. También deja de usar el fallback clásico y evita confirmar automáticamente una
posible eliminación permanente; un origen ausente sin identidad confirmada se comunica como fallo.
Faltan validación del runner, comportamiento de advertencia del Shell y build actualizado; no se
afirma que esta configuración impida el borrado permanente en todos los proveedores.
Validación local posterior: 1.533 pruebas aprobadas, nueve ignoradas y Clippy sin avisos; el
supervisor registró un pico de 8,77 GiB. La validación remota y el empaquetado siguen pendientes.
La corrida remota [41213425597](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/41213425597)
confirmó que pasan los dos casos originales de Papelera, pero falla el test nuevo de restauración
con alias 8.3: el origen se recicla y se identifica, pero no aparece con su nombre largo original.
La restauración usa el componente abreviado como nombre explícito de destino; falta corregirlo y
volver a ejecutar el gate. El job Linux vuelve a fallar y el benchmark Windows pasa.
El ajuste posterior consulta el nombre largo antes de reciclar y lo conserva en el recibo; si no
puede obtenerlo, aborta. Las 18 pruebas locales y Clippy de plataforma pasan, aunque el volumen
local no ofreció un alias 8.3 distinto. Falta repetir el runner y generar la distribución: se
interrumpió el build anterior antes de empaquetar al detectarse el fallo adicional.
La corrida [41217139110](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/41217139110)
pasó las pruebas de Papelera, incluida la de alias 8.3 y los dos casos originales. El gate Windows
falló después en dos tests de interfaz que esperan texto español, mientras el runner usa inglés;
Clippy remoto quedó omitido. El benchmark pasó; Linux mantiene sus fallos. El build supervisado
terminó (release 74m31s, pico 10,72 GiB), generó setup y ZIP sin firma y verificó que el EXE del
ZIP coincide con release y no incluye PDB. SHA-256 ZIP
`a975a4e1d54e4cd4907655a6808724f3f04e5d39a5754e7025aa50e8c4ad4e99`, setup
`a1847f9bf8705ca3ee3743d801f0d008a8102bd84a7abc814543661896866cd0`.
El paquete aún no incorpora la integración de carpetas 7-Zip y no se instaló.
Las dos pruebas de interfaz ahora seleccionan español explícitamente y pasan localmente; queda
repetir CI para comprobar el gate con el entorno remoto.
La corrida posterior [41223964469](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/41223964469)
terminó con el gate Windows y benchmark aprobados; `linux-native` sigue fallando. El receptor OLE
experimental está respaldado por separado en `codex/experimento-ole-worker-20261031`; no está
integrado en el paquete, ni validado con la interfaz o cancelación.

## Materiales 3MF — colores básicos y distribución local, 2026-10-31

El árbol local agrega una opción para mostrar el color de materiales base por objeto o triángulo,
con transporte de colores en la escena aislada. Pasaron pruebas iniciales del core; la verificación
completa pasó (1.518 pruebas, 9 ignoradas, Clippy y formato); el build de distribución supervisado
terminó (release 29m22s, pico 10,72 GiB). Logs del build
`target/agent-out/guarded-20261031-064903-791.{out,err}.log`. Logs de validación
`target/agent-out/guarded-20261031-062757-756.{out,err}.log`.
El EXE del ZIP coincide con release (`d629a8505aca187139a93178cfd0f1da4cf4bfd67ac83542b6b7927abc2f9a92`),
ZIP sin PDB (`2ef8989a880106fc3c75024ab0e2e63c5c13114aa932881d60234dbdd09b5b98`), setup
(`06fb588879f9c15a2a9fd42057a0759029816c82638151bf103822637126899c`); ambos artefactos sin firma.
No se instaló ni publicó. Medición supervisada: 96.800 triángulos a 1024×768, mediana azul
17,518 ms y color de archivo 18,249 ms (+4,2 %), siete rondas alternadas en una ejecución; no
incluye carga ni transporte. Ver `BENCHMARKS.md`. Pendiente aceptación visual. No se interpretan
todavía extensiones de materiales, texturas ni colores propietarios;
la transparencia se aproxima sobre el fondo y no muestra capas transparentes.

## Firma Windows — comprobación local, 2026-10-30

`scripts/test-signing.ps1` pasó con certificados sintéticos y rechazo de un archivo sin firma.
El helper validó nombre y versión 0.5.4 de `target/release/naygo.exe` y del setup en `dist/`;
ambos reportaron `NotSigned`. No se firmó ni publicó nada. Estas pruebas no sustituyen la
aprobación de SignPath, la ejecución remota ni la verificación de una firma real.

## Empaquetado — control de setup previo, 2026-10-30

El script podía incluir un setup previo en los checksums si faltaba Inno al generar un portable.
Ahora detecta ese caso antes de modificar dist y se detiene, conservando los archivos existentes;
el manifiesto incluye solo los artefactos creados en la ejecución. La prueba aislada con setup y ZIP
previos y manifiesto pasó. Build supervisado normal terminado (release 29m06s, pico 10,72 GiB), logs
`target/agent-out/guarded-20261030-150735-561.{out,err}.log`. El EXE del ZIP coincide con release
0.5.4 y el manifiesto coincide con ZIP y setup; ZIP sin PDB. SHA-256 ZIP
`e944d60567b0342e08dee407ca66791d1e3af7ea72d4d4d4d909a26cc2dc1bbd`, setup
`1e97f3f83be6f5742170efb6553dc2d7e72d068e4fcee51a1c8ad53c17efb0e7`. Sin firma ni instalación.

## W-12 — estado remoto comprobado 2026-10-30

La última corrida remota de CI es la [34155852541](https://github.com/nicolasgroth/explorador_archivos_naygo/actions/runs/34155852541),
del 7 de septiembre, sobre `abdef402babd2c136744c67ee9833c2f67bf0f16`. Falló en
`trash::tests::enviar_un_archivo_a_papelera` al no encontrar el elemento al restaurar; Clippy no
se ejecutó. El workflow y `trash.rs` locales están modificados respecto de ese commit, por lo que
esa corrida no valida la corrección actual ni el runner `windows-2025`. La ejecución remota del
workflow actualizado sigue pendiente; esta consulta fue de solo lectura.

## Paneles y temas — avance 2026-10-30

Se ajustó el texto secundario de seis temas y el principal de Solarized Dark para mejorar su
contraste. La prueba focalizada exige 4,5:1 sobre tokens de panel, toolbar, filas y selección en
los 15 temas; se inspeccionaron capturas de Dracula, Solarized Dark y Paper. No cubre todos los
controles, estados ni temas personalizados. Gate completo aprobado: 1.511 pruebas, 8 ignoradas,
Clippy y formato. Build anterior supervisado completado (release 28m26s, pico 10,72 GiB), logs
`target/agent-out/guarded-20261030-143329-910.{out,err}.log`. EXE de ZIP idéntico al release
0.5.4; ZIP sin PDB. SHA-256 ZIP `1b97669cf08846e28bb2aa7db180b9016b2817963e4ebaf59bb453cbf131a778`,
setup `1e6eb705ee6fd525255319345c49f06bd57f208609a6cff1b4718b551b87d08f`.
Sin firma ni instalación; pendiente aceptación nativa.

La auditoría de contraste del pie detectó relaciones inferiores a 4,5:1 en Dracula (2,629),
Solarized Dark (2,502) y Paper (3,965) con texto secundario. El pie usa ahora el texto principal;
la prueba de renderizado de 15 temas pasa con mínimo 5,035:1. Gate completo: 1.511 pruebas,
8 ignoradas, Clippy y formato correctos. Build anterior supervisado completado (release 32m33s, pico
10,72 GiB), logs `target/agent-out/guarded-20261030-133938-907.{out,err}.log`. EXE del ZIP igual
al release 0.5.4, ZIP sin PDB; SHA-256 ZIP
`8c484b263f6db4d2b53d0726ac61f699dc231d54b25e2ba80bdbcf51dc4577ab`, setup
`f0e74edb9afe26c4c83e6a79e63f6de5b221c2da5aa0ecef4a8c4730b28fc229`. Setup sin firma, no instalado.
Pendiente aceptación visual nativa; los temas personalizados no tienen garantía automática de contraste.

El pie del panel de archivos tiene un matiz derivado del color de texto del tema para distinguirlo
del título, sin nuevos campos obligatorios en temas existentes. La prueba de renderizado en Slint
pasó para los 15 temas de fábrica y verifica separación y conservación relativa del contraste;
se inspeccionaron además capturas de Dark Blue, Arena y High Contrast. Gate completo aprobado:
1.509 pruebas, 8 ignoradas, Clippy y formato. El build supervisado del 2026-10-30 fue detenido al
bajar el margen disponible de 8 GiB; pico del proceso 10,73 GiB. Logs
`target/agent-out/guarded-20261030-070415-552.{out,err}.log`. El build posterior, descrito abajo,
completó el empaquetado de este ajuste; queda la aceptación en ventana nativa.

## 7-Zip — investigación de carpetas, 2026-10-30

Consulta WTS posterior a los timeouts de captura: sesión 1, nivel 1, `SessionFlags=0`
(bloqueada en Windows 10 según la documentación de Microsoft). No permite inferir el estado al
momento de los intentos anteriores. La siguiente prueba visual requiere sesión desbloqueada.
Posteriormente Nicolás desbloqueó la sesión; WTS indicó `SessionFlags=1`. Se abrió el receptor
aislado PID 61976 y la fixture en 7-Zip PID 58132, pero la captura volvió a agotar el tiempo incluso
tras reactivar la ventana. Se solicitó arrastre manual de la carpeta sintética; al registrar esta
nota aún no había payload. Si se cierra el proceso, volver a abrir el receptor y la fixture.
Nicolás arrastró manualmente la carpeta de la fixture desde 7-Zip hacia el receptor aislado. El
receptor registró dos eventos con `paths=[]`, `staged=false` y error «el temporal de 7-Zip no es un
archivo», cada uno con una ruta `Temp\7zE...\E2 fixture`. Se confirma el rechazo real, no una
copia correcta ni la interacción en Naygo completo. Se cerraron únicamente los procesos de prueba.

Experimento sintético `scripts/measure-ephemeral-tree.rs`: 1.000 archivos se abrieron en 260,071 ms
y 10.000 en 4.221,322 ms; después de borrar las rutas sintéticas se copiaron desde los handles y se
verificaron contenidos y carpeta vacía. Una ejecución por tamaño, datos pequeños en disco local.
El segundo caso excede la espera actual de dos segundos. Esa primera versión no probó OLE real, cancelación, junctions,
permisos, red ni límites de recursos y no habilita la función; resultados en `target/agent-out`.
El prototipo se amplió con apertura sin seguir puntos de reanálisis y handles de directorios que no
comparten borrado. Una junction sintética se rechazó. En este equipo abrir el directorio con acceso
cero o solo a atributos no impidió borrarlo; con `FILE_LIST_DIRECTORY` se observaron errores de
compartición al intentar borrar y renombrar una carpeta retenida. Con listado, 1.000 archivos se
capturaron en 298,016 ms y 10.000 en 4.903,819 ms (una ejecución; la prueba de renombre se añadió
después de medir 10.000). No valida seguridad de antecesores, cancelación, OLE o recursos ilimitados.
La espera COM con `COWAIT_DISPATCH_WINDOW_MESSAGES` permite despachar mensajes, según
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/ne-combaseapi-cowait_flags),
pero winit 0.30.13 encola eventos reentrantes cuando el handler está ocupado y trata los repintados
por separado. No se ha demostrado que una espera anidada mantenga Slint utilizable o sea segura;
requiere prueba integrada, o un destino OLE separado. No se cambió el comportamiento de la app.
Prueba sintética STA/MTA: `CoWaitForMultipleHandles` con solo despacho de ventana devolvió
`CO_E_NOT_SUPPORTED` (0x80004021) en este equipo; combinando despacho de ventana y llamadas COM,
el worker recuperó CF_HDROP del objeto sintético. No se inyectaron mensajes de ventana ni se probó
Winit o 7-Zip en ese experimento. Siete pruebas focalizadas y Clippy de plataforma pasaron; el caso
de carpeta sigue ignorado. El flag combinado no demuestra respuesta de la interfaz ni portabilidad.
Un sondeo posterior publicó un mensaje a una ventana Win32 mínima durante la espera combinada; se
despachó antes del evento de fin en esta ejecución. El control verificó que el mensaje llegó y que
no se duplicó. Ocho pruebas focalizadas y Clippy pasan; no demuestra entrada, repintado o seguridad
reentrante de Slint/Winit, ni el comportamiento con 7-Zip real.
Se preparó el ejemplo aislado `crates/platform/examples/drop_probe_async.rs`: obtiene `CF_HDROP` en
un worker COM y conserva handles mientras una ventana Win32 mínima espera hasta 30 s; luego compara
los 259 contenidos con la fixture y comprueba una carpeta vacía. Dos pruebas y Clippy del ejemplo
pasan: una usa IDataObject sintético, elimina el árbol y compara todos los archivos retenidos. Un
intento anterior reveló que la limpieza podía tardar más de dos segundos; el verificador espera ahora
hasta 30 s la desaparición. Limita a 5.000 archivos, no copia a destino, no valida seguridad de
antecesores, cancelación, Slint/Winit ni OLE real y no está integrado en Naygo. La sesión se bloqueó
de nuevo antes de probarlo con 7-Zip; se solicitó desbloqueo cuando sea posible.
Si la espera agota su timeout, el worker puede seguir operando sobre el origen; falta resolver ese
ciclo de vida antes de cualquier integración en producción.
Una tercera prueba sintética registra el receptor en un STA distinto al de la ventana y ejecuta la
fuente COM en un tercer hilo: conserva los 259 archivos y la carpeta vacía tras eliminar el origen.
Con una demora simulada de 800 ms, la ventana Win32 procesó temporizadores durante la espera. Las
tres pruebas y Clippy pasan; esto aún no prueba la integración Slint/Winit ni el nuevo prototipo con
7-Zip real. La sesión de Windows se encuentra bloqueada para esa prueba interactiva.
El prototipo señala cancelación al agotar la espera y la comprueba después de `GetData` y en cada
entrada del árbol. Una cuarta prueba comprueba que una cancelación previa detiene el recorrido y
que los handles se liberan al descartar el snapshot; pasan las cuatro pruebas y Clippy. Una llamada
`GetData` o de filesystem bloqueada todavía no se puede interrumpir con esta señal; no se ha probado
cancelación durante una llamada de filesystem ni con 7-Zip real.
Una prueba posterior bloqueó un `GetData` sintético, señaló cancelación y constató que el worker
siguió esperando; al liberar el proveedor devolvió cancelación antes de recorrer la ruta. Las cinco
pruebas del ejemplo y Clippy pasan. No prueba 7-Zip ni resuelve un proveedor que nunca retorne.
Otro experimento sintético habilitó la cancelación COM y ejecutó `CoCancelCall`: en esta máquina
la solicitud devolvió éxito y el cliente retornó `0x80010002` antes de liberar al proveedor.
Las seis pruebas del ejemplo y Clippy pasan. Según [Microsoft](https://learn.microsoft.com/en-us/windows/win32/com/canceling-method-calls),
desbloquear al cliente no garantiza que el servidor deje de trabajar; falta probar 7-Zip y coordinar
la vida del temporal. Esta vía no está conectada al receptor experimental de la app ni a producción.
Se agregó una prueba de materialización en un destino temporal propio: retiene una carpeta con
contenido y subcarpeta vacía junto a un archivo independiente, elimina el origen sintético y
reconstruye ambos desde los handles. También se rechaza una ruta ascendente `..`. Pasan ocho
pruebas del ejemplo y Clippy; el helper es de prueba y no resuelve todavía el staging de Naygo ni
demuestra comportamiento de 7-Zip real.
Prueba real posterior con Nicolás: desde 7-Zip 24.07 se arrastró la carpeta sintética de 259
archivos al receptor experimental con STA separado. Se capturaron cinco directorios en 615 ms y
la ventana Win32 procesó seis temporizadores. Tras la limpieza del temporal, los 259 contenidos
(3.229 bytes) y la carpeta vacía coincidieron con la fixture. Es una sola prueba local; no valida
Slint/Winit, selección mixta, árboles grandes, errores, cancelación ni la integración en Naygo.
Una medición exploratoria de enlaces físicos sintéticos dio 1.918,591 ms para 1.000 y 19.513,499 ms
para 10.000 archivos (una ejecución, sin creación ni limpieza). Un test demostró que comparten los
cambios del origen, así que no ofrecen un snapshot inmutable. Una alternativa de handle permite
compartir lectura y borrado, rechaza escritores existentes o nuevos y conserva el contenido tras
eliminar el nombre; pasa en una fixture local. El prototipo se cambió para usarla; 11 pruebas y
Clippy pasan. Una segunda prueba real con esa variante capturó 259 archivos y cinco directorios
en 598 ms, atendió seis temporizadores y conservó los contenidos y la carpeta vacía tras la
limpieza de 7-Zip. El módulo reutilizable tiene siete pruebas focalizadas y todavía no está
conectado al receptor de producción.
Se preparó el ejemplo `crates/platform/examples/drop_probe.rs`: ventana aislada con el receptor OLE
actual, sin copiar a un destino ni modificar la configuración; check y Clippy focalizados pasan.
La ventana y la fixture ZIP se abrieron. UI Automation detectó la carpeta y 259 archivos, pero la
captura de 7-Zip agotó el tiempo dos veces; no se realizó arrastre. Se cerraron los procesos de prueba.
Se agregó además un `IDataObject` sintético en pruebas: el drop de archivo temporal pasa por la
interfaz OLE; el caso de carpeta y subcarpeta vacía reproduce el error «no es un archivo». La
prueba de carpeta permanece ignorada hasta corregirlo. Seis pruebas focalizadas y Clippy de
plataforma pasan; el objeto sintético no reproduce extracción ni limpieza real de 7-Zip.

Se contrastó `PanelDrag.cpp` del tag oficial 24.07 (`a7a1d4a241492e81f659a920f7379c193593ebc6`)
con la versión instalada. El protocolo de destino especial puede hacer que la fuente extraiga
después de `DoDragDrop`, sin proporcionar a Naygo una confirmación de resultado en ese flujo;
no debe enviarse el destino final sin resolver conflictos, cancelación e historial. Se amplió el
preparador con árbol anidado, carpeta vacía y tamaño parametrizable; un ZIP de 259 archivos y una
carpeta pasó `7z t` con destino vacío. El arrastre OLE real y la corrección siguen pendientes.
La clase fuente de 7-Zip 24.07 expone `IDataObject` como única interfaz COM declarada y el
directorio temporal vive en el ámbito de la operación de arrastre. Retener una referencia al
objeto no demuestra que se retrase la limpieza y no hay soporte declarado para
`IDataObjectAsyncCapability`; la solución debe controlar la retención antes del retorno o
demostrar otra coordinación con el proveedor.
Además, el `Drop` actual llama `GetData(CF_HDROP)` desde la UI; el código 24.07 del proveedor
puede invocar `CopyFromPanelTo_Folder` antes de devolver los datos. El límite de dos segundos del
snapshot no limita esa extracción previa. La solución debe cubrir latencia COM además de la vida
de los temporales; aún no existe medición del arrastre real en este equipo.

## Visor 3D — base de presentación en curso, 2026-10-30

Benchmark supervisado con modos alternados y calentados: medianas sólido 19,247 ms, bordes
20,354 ms y malla 20,074 ms para 96.800 triángulos a 1024×768; solo raster, una ejecución.
Detalles y límites en `BENCHMARKS.md`. No reemplaza la validación visual ni mediciones en UI.

El parser ahora rechaza referencias a componentes externos antes de resolver identificadores
locales, evitando mostrar una malla incorrecta por coincidencia de IDs; se conserva la miniatura de
respaldo. Pasan 19 pruebas focalizadas y el gate completo (1.511 pruebas, 8 ignoradas, Clippy y
formato). Build anterior supervisado completado (release 32m58s, pico 10,72 GiB), logs
`target/agent-out/guarded-20261030-123102-244.{out,err}.log`; el ZIP contiene el EXE release
0.5.4 idéntico y no incluye PDB. SHA-256 ZIP
`b07c4d38df37fa1700e8629ceb96d07b0f19592c50052235354b9990d1b98f05`, setup
`37e0e74409519b9289919071370cfb38bc25013e84b4338b00c87ff1823d3504`; setup sin firma.
No instalado ni publicado; aceptación nativa pendiente.

La lectura inicial 3MF ahora prioriza geometría compatible y usa la miniatura como respaldo ante
errores de parseo, sin convertir cancelación en éxito. Pasaron 18 pruebas focalizadas, incluidas
prioridad, respaldo y cancelación, y el gate completo (1.510 pruebas, 8 ignoradas, Clippy y formato).
Build anterior supervisado completado: release 40m08s, pico 10,72 GiB; logs
`target/agent-out/guarded-20261030-112629-250.{out,err}.log`. El EXE del ZIP coincide con release,
versión 0.5.4; ZIP sin PDB. SHA-256 ZIP `3208064f2de8c769fc98c279cdf069b91f673e62ca930e36195be93797910d9d`,
setup `6af36bc942123d10fdeba2aee5604d13eb722b82ebf15d7fc11a9e7ee118e43c`. Sin firma ni instalación.
Falta validación nativa. La lectura previa
de geometría puede demorar más que mostrar una miniatura, y sigue acotada por el lector aislado.

El gate 0.5.4 pasó (1.509 pruebas, 8 ignoradas, Clippy y formato). El benchmark supervisado del
modo sólido reportó mediana 18,483 ms para 96.800 triángulos a 1024×768, sin comparación
controlada. Build supervisado completado (Release 28m59s, Inno 117,578 s, pico 10,72 GiB); ZIP y
setup verificados, sin firma, instalación ni publicación. Los hashes y límites están en
[`VALIDACION-0.5.4-VISOR-2026-10-30.md`](VALIDACION-0.5.4-VISOR-2026-10-30.md). Es una
distribución de avance, no el cierre de 7-Zip, materiales 3MF ni revisión completa de temas.

El visor permite elegir tonos uniformes azul, blanco y cálido, y modos sólido, bordes o malla con
oclusión por profundidad. Los controles de UI están conectados y utilizan escenas retenidas; aún no
se leen materiales 3MF y la miniatura se usa como respaldo de la geometría. El render sólido por defecto
mantiene su cobertura y colores anteriores. Pasan pruebas del core, ciclo de vida, clics en Slint,
20 pruebas i18n, Clippy y gate completo. Faltan mediciones de bordes/malla y validación visual.

## Preview configurable Windows 0.5.3 — paquete listo, 2026-10-21

Se completó la lectura máxima configurable para archivos de texto: 64 KiB, 256 KiB, 512 KiB o
1 MiB, con 256 KiB por defecto. La lectura ocurre en el proceso aislado existente; el panel muestra
bytes leídos y caracteres/líneas. El gate completo pasó (**1.507 pruebas aprobadas, 8 ignoradas**,
Clippy y formato verdes). El build supervisado regeneró ZIP y setup 0.5.3; el EXE del ZIP coincide
con `target/release/naygo.exe`. Los artefactos no están firmados ni publicados y no se instalaron;
el proceso que ya estaba abierto quedó intacto. Nicolás validará manualmente la configuración y el
comportamiento en este equipo y en máquinas de 8/16 GiB. Los límites de 250 líneas y 1.000
caracteres por línea siguen activos; XML/SIF minificado puede requerir una opción futura de
formateo. Detalle y hashes en
[`VALIDACION-PREVIEW-TEXTO-2026-10-21.md`](VALIDACION-PREVIEW-TEXTO-2026-10-21.md).

## Continuación 2026-10-19 — L-08 y release local instalado

Se completó la implementación local de instancia única Linux y sus checks cruzados; el gate Windows
pasó **1.480 tests, 8 ignorados**, Clippy y formato. La distribución 0.5.3 se reconstruyó bajo
supervisor (Release 41m23s, Inno Setup 199,766 s, pico Job 10,66 GiB), se verificó y se instaló con
autorización previa. Naygo quedó abierto y responde como PID 50260; el EXE instalado coincide con
Release (SHA-256 `960ccee8a5bdfb85eaafcd8fa6112e736d4b050237e7fdfb592f41799671cadc`). Los cuatro
JSON de usuario y el PDB existente conservaron sus hashes. ZIP y setup siguen sin firma y no se han
publicado. Los tests Linux compilan, pero no se ejecutaron: WSL informa
`HCS_E_HYPER_V_NOT_INSTALLED`. L-08 permanece abierto hasta validar runtime en Ubuntu nativo. Ver
[`VALIDACION-L08-INSTANCE-2026-10-19.md`](VALIDACION-L08-INSTANCE-2026-10-19.md).

## Instalación Windows 0.5.3 — 2026-10-20, actualización W-06B

Con la autorización previa de Nicolás, se actualizó in-place desde el setup 0.5.3 que contiene
W-06B y los arreglos Windows de Papelera. Inno terminó con código 0. Naygo quedó abierto y responde
como PID 43624; el EXE instalado coincide byte a byte con `target/release/naygo.exe` y el EXE del
ZIP (SHA-256 `868a69dec0f6f79d0e91e54326eb9762cd745921467dd45a72fd5a4000bd2b5a`). Settings,
workspace, favoritos y plantillas mantuvieron sus hashes y siguen siendo JSON válidos. El PDB
previo de 289.722.368 bytes conservó su SHA-256 `eaad3dd91e67bec98b00b16051c037a982590b3cee6adc1c20ccca4c6a0e2d07`.
Se guardó respaldo de los cuatro JSON en `target/agent-out/install-w06b-20261020/`.

Una muestra pasiva 3×30 s del proceso exacto instalado registró 57,0 MiB de working set, 26,3–26,4
MiB privados, 370–375 handles, 25–26 hilos y 0/0,015/0 s de CPU; un solo proceso, sin SMB. `Responding`
fue `True`. UI Automation de solo lectura encontró 356 descendientes; 120/120 enfocables con nombre,
69 acciones `Invoke`, cero botones sin ese patrón y árbol completo. La sesión seguía bloqueada: esto
no mide interacción ni latencia; Narrador y aceptación manual siguen pendientes, así que W-07 y W-13
no se cierran. Setup y ZIP siguen `NotSigned`; no se publicaron. Ver
[`VALIDACION-INSTALACION-W06B-2026-10-20.md`](VALIDACION-INSTALACION-W06B-2026-10-20.md).

## W-13 — UI Automation pasiva, 2026-10-20

La inspección del PID 43624 expuso 356 descendientes; los 120 controles enfocables tienen nombre,
los 69 botones anuncian `Invoke` y el árbol quedó completo. El SHA del EXE instalado coincide con
`target/release/naygo.exe` (0.5.3). No se activaron controles ni se leyeron nombres/rutas. Narrador,
calidad de anuncios, foco real y diálogos siguen pendientes. Evidencia:
[`VALIDACION-W13-PASIVO-2026-10-20.md`](VALIDACION-W13-PASIVO-2026-10-20.md).

## Vista previa Windows — `.sif` y `.sh`, 2026-10-20

`.sif` en Auto antes no clasificaba como texto. Se incorporó a la lista textual y la configuración
v4 migra añadiendo la regla faltante, sin reemplazar reglas existentes; `.sh` permanece en texto
por defecto y su lectura se prueba en fixture local. En la captura externa, `.sh` llega al lector
pero recibe un error de apertura/lectura («No se pudo leer»); el archivo está en `Z:\`, inaccesible
desde este equipo para reproducir permisos o estado de la unidad. Verificación remota pendiente.
El gate completo pasó (1.503 aprobados, 8 ignorados; Clippy y formato verdes). El build supervisado
se detuvo al quedar 7,91 GiB libres, bajo el umbral de 8 GiB; por eso no hay una distribución nueva
ni instalación de esta corrección. Los artefactos previos permanecen intactos. Evidencia:
[`VALIDACION-PREVIEW-SIF-SH-2026-10-20.md`](VALIDACION-PREVIEW-SIF-SH-2026-10-20.md).

## Límite configurable del preview de texto — implementado y empaquetado

Se añadió a Configuración una elección de 64 KiB, 256 KiB, 512 KiB o 1 MiB (valor por defecto:
256 KiB), persistida en `settings.json` v6 con migración aditiva. El worker aislado recibe el límite;
el preview muestra bytes leídos y caracteres/líneas presentados. Los límites de 250 líneas y 1.000
caracteres por línea siguen protegiendo el renderizado. Gate y build 0.5.3 pasan; el paquete aún
requiere aceptación manual en equipos con recursos menores. XML/SIF minificados en una línea aún
pueden quedar recortados por el límite por línea; formateado XML es un ajuste separado por evaluar.

## W-07 — repetición de listado local 100k, 2026-10-20

Dos ejecuciones iniciales habían dado medianas de 351,502 y 308,775 ms para 100.000 archivos
vacíos; sus consultas puntuales de CPU fueron posteriores y no alineadas. La comparación controlada
del 2026-10-20 midió tres pasadas por cada uno de dos binarios: 261,693 ms para el release previo y
295,669 ms para el actual (+13,0 %). CPU total durante ambos runs: 12–31 %; RAM libre: 44,02–44,38
GiB. La nueva mediana queda bajo el umbral orientativo de 15 % y bajo el promedio previo de 330,138
ms. Builds distintos y actividad de fondo impiden atribuir el delta al código. Fixtures temporales
limpiadas; no se justifica optimización con esta evidencia. W-07 sigue abierto para interacción real,
presentación y matriz local/UNC; resultados y JSONL en [`BENCHMARKS.md`](BENCHMARKS.md) y
`target/agent-out/`.

## Revalidación Windows focalizada — 2026-10-20

La prueba de regresión de la columna Extensión para carpetas con puntos pasó 1/1; las siete
pruebas de planificación de `drop_at` pasaron; y la prueba de `CF_HDROP` temporal de 7-Zip pasó
1/1. La instancia instalada coincide con el release 0.5.3 local (PID 43624), pero la sesión sigue
bloqueada, así que no se intentó el arrastre real ni se pudo completar W-07/W-13 manuales. E-1 y
E-2 siguen implementados; su aceptación visual/OLE manual permanece pendiente.

## Refuerzo Windows de Papelera — 2026-10-19

La asociación del Shell ya compara rutas sobre UTF-16 nativo, sin fusionar nombres inválidos
distintos, y el fallback de Deshacer acepta `DeletedFrom` con mayúsculas o separadores equivalentes
manteniendo la comprobación de carpeta y nombre. La prueba del recibo reconstruido fallaba con la
comparación estricta anterior y ahora acepta el caso equivalente, rechazando destinos incorrectos.
Las 11 pruebas de Papelera pasan; gate completo: **1.483 aprobadas, 8 ignoradas**, Clippy, formato y
nueve catálogos correctos. El paquete supervisado terminó (Release 51m07s, Inno 199,000 s, pico
10,66 GiB); ZIP y setup verificados, ZIP con cuatro entradas y EXE idéntico al Release. No se
instaló, firmó ni publicó; Naygo continúa abierto como PID 50260 con la versión previa. Hashes y
límites están en
[`VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md`](VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md).

## W-07 — observación pasiva reciente

El 2026-10-19 23:51–23:53 se midió la instancia ya abierta PID 50260 durante tres ventanas de 30 s:
62,0 MiB working set, 43,2 MiB privados, 368 handles, 22 hilos y CPU 0,031/0,032/0,015 s por
ventana; un proceso y sin SMB. La captura no controla reposo ni mide interacción, y corresponde al
EXE instalado anterior (SHA-256 `960ccee8a5bdfb85eaafcd8fa6112e736d4b050237e7fdfb592f41799671cadc`),
no al paquete recién reconstruido. W-07 continúa abierto. Evidencia en [`BENCHMARKS.md`](BENCHMARKS.md)
y JSON local `target/agent-out/w07-process-20261019-pid50260-passive.json`.

## W-13 — intento pasivo de UI Automation

A las 23:53 del 2026-10-19, PID 50260 respondía y tenía handle de ventana, pero UI Automation no
expuso controles descendientes; la raíz era un `Pane` sin hijo y `LogonUI` estaba activo en la misma
sesión. Esto no prueba la causa ni demuestra una regresión. No se enviaron acciones. Repetir con la
ventana visible en una sesión desbloqueada; Narrador y diálogos siguen pendientes. Detalle en
[`VALIDACION-W13-INVOKE-2026-10-19.md`](VALIDACION-W13-INVOKE-2026-10-19.md).

Reintento pasivo 2026-10-20 00:40 Chile: PID 50260 responde, pero el título de ventana está vacío y
UIA vuelve a no exponer descendientes. `LockApp` y `LogonUI` siguen activos en la sesión; no se
interactuó con la aplicación. Repetir cuando esté desbloqueada; Narrador y diálogos no están
aceptados.

## W-12/W-14 — verificaciones locales 2026-10-20

Los pines SHA de Actions y `scripts/test-signing.ps1` pasan localmente. El ejecutable temporal
oficial `actionlint` v1.7.12 se verificó con el SHA-256 publicado y ambos workflows (`ci.yml` y
`release.yml`) pasaron el lint. El instalador 0.5.3 actual tiene metadatos `Naygo`/`0.5.3`, pero sigue
`NotSigned`. W-12 aún requiere runner Windows Server 2025. W-14 sigue pendiente de configurar y
aprobar SignPath, resolver cómo autoriza refs de tag, y verificar la primera firma real; no se usaron
credenciales ni se publicaron artefactos.

Consulta remota de solo lectura 2026-10-20: GitHub sigue mostrando como último CI el run fallido
34155852541 sobre `abdef402` del 2026-09-07; no hay ejecución del checkout local actualizado.

## Instalación Windows 0.5.3 — 2026-10-20

Con autorización previa de Nicolás, se actualizó in-place desde `dist/Naygo-0.5.3-setup.exe`
(SHA-256 del setup `fa6fa2cb3f7542243a9076d060466cc0c7cc3c707a506cc0fa2233ff2651fd00`). Inno terminó
con código 0; los hashes de settings, workspace, favoritos, plantillas y PDB quedaron idénticos y
los JSON parsean. El EXE instalado coincide con Release (`cb8f28efcd9619463a5977ba6fb771e59656352bb4a49dfe76bb9fb3c0682229`); Naygo quedó abierto/respondiente como PID 31304, versión 0.5.3. No se firmó ni publicó. La sesión está bloqueada, por lo que la aceptación visual sigue pendiente. Evidencia en
[`VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md`](VALIDACION-WINDOWS-TRASH-UTF16-2026-10-19.md) y el respaldo de datos en `target/agent-out/install-preserve-053-20261020-wtrash/`.

## W-07 — muestra pasiva del ejecutable instalado, 2026-10-20

PID 31304 se observó en tres ventanas de 30 s, con 16 muestras por ventana: 57,7 MiB working set,
26,9 MiB privados, 367 handles y 25 hilos; CPU delta 0,015/0,047/0,032 s. Se detectó un proceso,
sin SMB. La sesión estaba bloqueada; no se midieron interacción, inactividad confirmada, latencia,
presentación ni filesystem. La muestra es evidencia parcial y W-07 permanece abierto. JSON:
`target/agent-out/w07-installed-053-20261020-passive.json`; detalle en [`BENCHMARKS.md`](BENCHMARKS.md).

## W-06B — lectura asíncrona del portapapeles al pegar, 2026-10-20

Las lecturas/escrituras del clipboard ahora pasan por un worker serial con cola acotada. Incluye
`op_paste`, pegar en Cesta, Copiar/Cortar, exportar CSV y copiar rutas/texto/procedencia/preview;
así las operaciones rápidas mantienen orden y no bloquean callbacks. Corte e historial se aplican
tras confirmar la escritura; fallos se muestran en un toast. Test focalizado: 2/2; gate completo:
**1.500 tests aprobados, 8 ignorados**, Clippy y formato. El check cruzado `x86_64-unknown-linux-gnu`
de UI pasó con el aviso esperado de que el recurso PE de Windows no aplica al target Linux. La
distribución 0.5.3 se regeneró bajo supervisor (Release 46m08s, Inno 176,562 s), ZIP y setup
verificados sin firma; PID 31304 continúa ejecutando el build anterior. Evidencia:
[`VALIDACION-W06B-PASTE-CLIPBOARD-2026-10-20.md`](VALIDACION-W06B-PASTE-CLIPBOARD-2026-10-20.md).

Auditoría adicional de W-06B autorizada por Nicolás: la copia de la ruta del resultado de Entregas
también pasa por `OpsCtrl::write_clipboard_text`. No se hallaron accesos al backend de plataforma
fuera del worker serial. La regresión `async_read_is_processed_off_caller_thread` pasa; suite
focalizada vigente: 3/3. El cambio sólo añade cobertura y no modifica el comportamiento del producto.

## Linux L-09A — codec inicial de `text/uri-list`

Se agregó en `naygo-core` un encoder/decoder puro para rutas `file:` locales; preserva bytes POSIX
no UTF-8 y rechaza authorities remotas, esquemas ajenos y entradas ambiguas. Pasaron 9 pruebas
focalizadas, el gate completo (1.492 aprobadas, 8 ignoradas, Clippy/formato) y el check cruzado del
crate core para `x86_64-unknown-linux-gnu`. La función aún no está conectada al clipboard del
escritorio; Nautilus/Dolphin, copy/cut, Wayland/X11 y tests nativos siguen pendientes. No se
regeneró distribución porque el codec todavía no tiene consumidores de runtime. Evidencia:
[`VALIDACION-L09A-URI-LIST-2026-10-20.md`](VALIDACION-L09A-URI-LIST-2026-10-20.md).

### Avance posterior — formatos copy/cut Nautilus y Dolphin

`naygo-core::clipboard::desktop_formats` ahora codifica/decodifica el prefijo `copy`/`cut` de
Nautilus junto a las URIs y el byte de estado `0`/`1` de Dolphin. El codec limita listas a 16 MiB,
100.000 elementos y 64 KiB por ruta. Pasan 34 pruebas focalizadas de `clipboard`; `cargo check --tests` y Clippy cruzados del core para Linux
también pasan. Se actualizó Graphify. Esto aún no es integración de runtime: L-09A sigue abierto,
no hay transporte del clipboard ni confirmación de copy/cut con aplicaciones Linux nativas, y no se
regeneró distribución. Ver
[`VALIDACION-L09A-FORMATOS-2026-10-20.md`](VALIDACION-L09A-FORMATOS-2026-10-20.md).

## Registro histórico 2026-10-19 — W-13 instalado y reinspeccionado (reemplazado)

En esa etapa, la distribución 0.5.3 con las correcciones de acción accesible se reconstruyó bajo supervisor
(Release 34m39s, Inno Setup 131,235s, pico Job 10,65 GiB), se verificó e instaló con la autorización
previa. Naygo quedó abierto y responde como PID 44872; el EXE instalado coincide con Release. UIA
pasiva de la ventana principal reportó 356 descendientes, 120/120 controles enfocables con nombre,
69 acciones `Invoke`, cero botones sin `Invoke` y árbol completo. No se ejecutaron acciones ni se
abrieron diálogos. La aceptación con Narrador y el recorrido manual de Ajustes/diálogos siguen
pendientes, por lo que W-13 permanece abierto. Checksums y detalles en
[`VALIDACION-W13-INVOKE-2026-10-19.md`](VALIDACION-W13-INVOKE-2026-10-19.md).
Una muestra W-07 pasiva de 3×30 s en esa instalación registró 61,7 MiB working set, 30,1 MiB
privados, 387 handles y 26 hilos máximos; no controló actividad ni mide interacción/presentación.
W-07 sigue abierto. Detalle en [`BENCHMARKS.md`](BENCHMARKS.md).

## Implementación local L-08 — evidencia complementaria

El backend Linux de instancia única ya está implementado con runtime dir privado, lock de proceso,
Unix socket, validación de peer, mensajes acotados y rutas en bytes; la cola UI es de memoria y
capacidad limitada. Pasaron check/Clippy cruzados Linux de platform y UI, el gate Windows completo
(1.480 tests, 8 ignorados, Clippy y formato) y `graphify update`. WSL sigue sin poder iniciar por
`HCS_E_HYPER_V_NOT_INSTALLED`, así que los tests Linux compilan pero no se ejecutaron y L-08 queda
abierto para runtime nativo. El release Windows posterior ya se instaló; el estado y hashes actuales
están en la sección inicial y en [`VALIDACION-L08-INSTANCE-2026-10-19.md`](VALIDACION-L08-INSTANCE-2026-10-19.md).

## Continuación 2026-10-19 — reinspección UIA pasiva

La instancia ya abierta (PID 42728, versión de archivo 0.5.3) respondió a una lectura UIA de solo
lectura: 980 descendientes y 219/219 controles enfocables con nombre; el árbol no se truncó. No se
enviaron entradas ni se abrieron diálogos. El EXE instalado no coincide con la distribución W-06E
reconstruida; este resultado no cierra Narrador ni la aceptación de W-13. Evidencia y hashes en
[`VALIDACION-UIA-2026-10-19.md`](VALIDACION-UIA-2026-10-19.md).

## Continuación 2026-10-19 — acciones UIA de controles personalizados

La revisión UIA ampliada mostró 19 botones enfocables sin patrón `Invoke`; el componente
`BasketIconButton` publicaba rol/nombre y respuesta al teclado, pero no la acción accesible; se
añadió su `Invoke` guardado y enlazado al callback. La UIA de esa versión reportó 74 `Invoke` frente
a 63 y bajó de 19 a 10 los botones enfocados sin patrón. El análisis estático mostró que `OpButton`
y `MissingOpButton` también requerían rol, etiqueta, acción y foco/Enter/Espacio; se corrigieron.
Después del cambio, el gate completo volvió a pasar (1.480 tests, 8 ignorados), Clippy/formato y
Graphify (9.527 nodos, 18.052 relaciones, 899 comunidades). Otra inspección de fuente detectó que los
botones custom de maximizar/cerrar panel y los selectores de categoría de configuración tampoco
exponían `Invoke`; ya se corrigieron con nombre y foco/acción de teclado donde faltaban. Este cambio
pasó el gate completo repetido; falta regenerar la distribución e inspeccionar UIA sobre la nueva
instalación. Evidencia:
[`VALIDACION-W13-INVOKE-2026-10-19.md`](VALIDACION-W13-INVOKE-2026-10-19.md).

## Continuación 2026-10-19 — L-07 Papelera Linux

W-06D ya está implementado, validado, empaquetado e instalado; la aprobación recibida para ese
bloque no requiere cambios adicionales. L-07 avanzó con backend FreeDesktop, recibos de deshacer
persistibles y pruebas Linux para homónimos, conflictos, symlinks y nombres no UTF-8. Gate Windows:
1.478 tests, 8 ignorados, Clippy y formato; check y Clippy cruzados Linux pasan. Los tests Linux se
compilaron, pero el host Windows/WSL no puede ejecutarlos. El primer build se detuvo al marcar 7,87
GiB libres; el reintento terminó en 42m14s, Inno Setup en 159,329 s y pico 10,65 GiB. `dist/` se
actualizó y verificó, sin instalar/publicar/firmar. Naygo PID 42728 sigue abierto con la versión
previamente instalada. Evidencia:
[`VALIDACION-L07-TRASH-2026-10-19.md`](VALIDACION-L07-TRASH-2026-10-19.md).

## Continuación 2026-10-19 — W-06E temas

La escritura y eliminación de temas personalizados ya se ejecutaban desde el worker de packs; el
guardado ahora usa escritura atómica para no dejar JSON truncados si falla la operación. Se agregaron
pruebas del worker para guardar/renombrar y borrar (incluida la repetición idempotente). Los cuatro
tests focalizados pasaron; `scripts/test-all.ps1` pasó con 1.480 tests aprobados, 8 ignorados, Clippy
sin warnings y formato correcto. El build supervisado finalizó (Release 40m08s, Inno Setup 195,625 s,
pico Job 10,65 GiB bajo el límite de 12 GiB). ZIP, setup y versión se verificaron; no se instaló,
publicó ni firmó. Los hashes nuevos están en la validación detallada. Evidencia:
[`VALIDACION-W06E-2026-10-19.md`](VALIDACION-W06E-2026-10-19.md).

## Revalidación remota de workflows — 2026-10-19

`gh run list` de solo lectura no encuentra ejecuciones del checkout actualizado. La CI más reciente
es `abdef402` del 2026-09-07; los cinco runs más recientes pertenecen a `Preview implementation
review` del 2026-09-09. No se publicó ni disparó un workflow. En W-14, la documentación oficial
revisada nuevamente sigue sin decir cómo una ejecución `push.tags` se representa en `Allowed branch
names`; la política no debe activarse hasta que SignPath confirme el `BranchName` y el patrón. Detalle
en [`POLITICA-DE-FIRMA.md`](POLITICA-DE-FIRMA.md).

Punto de control anterior a la reinspección W-13: W-06D ejecuta enumeración Win32/USB y expulsión segura en workers; los
árboles reconcilian unidades sin perder las expansiones de discos conectados. El gate de fuente
actualizado pasó **1.476 tests (8 ignorados)**, Clippy y formato. El release 0.5.3 con W-06D se
regeneró bajo supervisor e instaló; Naygo permanece abierto y responde como PID 42728. W-07
reutiliza la única `PathBuf` de cada entrada para filtrar temporales
internos y construir `Entry`, sin atribuir una mejora de latencia. W-13 agregó acciones accesibles
para preview, árbol y Ctrl+P. El gate completo previo al empaquetado pasó **1.468 tests
(8 ignorados)**. La corrida de fuente anterior, con el parser preparatorio L-06, pasó **1.474 tests
(8 ignorados)**, Clippy y formato; también pasaron `cargo check` y Clippy cruzados para Linux. La
distribución 0.5.3 en `dist/` se regeneró tras añadir la enumeración Linux L-06 al worker compartido:
build supervisado Release **34m58s**, Inno Setup **145,172 s**, pico del Job **10,65 GiB** (límite
12 GiB). Desde el último
gate, `drives()` integra el parser L-06 con `/proc/self/mountinfo` en el worker de refresco; la
compilación cruzada y el gate Windows pasaron (1.477 tests, 8 ignorados). La tira y los mount points
reales aún requieren validación en Ubuntu; esta fuente no está en la distribución Windows 0.5.3
instalada. Hashes de esa build:

- Portable: `df10b324d2d4ce39ff279937d9d6e50b2048591a0eeb542fd2a441a93b6bb5f9`.
- Setup: `7eca3b54f92e82a3684f54cefb42d112cfb57452ac14155c7591e7e28670e731`.
- `naygo.exe`: `cf73acd1917a48057bd971da7af1e9efb2d835d2acabeae653c32543197004ec`.

El manifiesto de SHA-256 coincide, el EXE del ZIP coincide con Release y el PDB no se incluye en el
ZIP. Setup y EXE no tienen Authenticode. Esta regeneración no se instaló ni publicó. El proceso
instalado PID 42728 continúa abierto y responde con la build W-06D anterior, SHA-256
`6015599ce33acd41b5763611cc4fbfd84a49ce6a81df0652cbc3394eef3a2c60`.
Los cuatro JSON de usuario y el PDB conservaron sus hashes durante la actualización. UIA pasiva
encontró 821 descendientes, 654 nombrados y 190/190 controles enfocables nombrados, sin truncamiento;
la aceptación manual W-13 con Narrador aún está pendiente.

Repetición W-07 posterior sobre el harness release: las medianas externas del listado de 100.000
archivos fueron 241,865 / 210,594 / 232,889 ms; hay variación amplia y no se atribuye al cambio de
ruta. La cancelación llegó al estado terminal en las tres series y las fixtures se limpiaron. W-07
sigue pendiente de medición interactiva, presentación del renderer y matriz local/UNC. Logs y límites
en [`BENCHMARKS.md`](BENCHMARKS.md).

En ese punto de control, la instancia instalada era PID 42728, SHA-256
`6015599ce33acd41b5763611cc4fbfd84a49ce6a81df0652cbc3394eef3a2c60`, y permanece abierta. UIA
pasiva del proceso actual encontró 821 descendientes, 654 con nombre y 190/190 controles
enfocables nombrados, árbol completo. Esto no sustituye Narrador ni el recorrido manual. Los cuatro
JSON parsean; sus hashes y el PDB previo (289.722.368 bytes) se conservaron. La validación completa está en
[`VALIDACION-0.5.3-2026-10-19.md`](VALIDACION-0.5.3-2026-10-19.md).

Muestra pasiva W-07 sobre la instancia actual PID 42728, 2026-10-19 10:09: 78,4–78,7 MiB working
set, 46,6–46,8 MiB privados, 385 handles, 26 hilos y CPU delta 0,81–0,98 s en ventanas de 30 s;
un proceso y sin mapeos SMB. No se controló la actividad ni se midió input/presentación, así que no
es una línea base de reposo ni cierra W-07. Evidencia en [`BENCHMARKS.md`](BENCHMARKS.md).

Captura W-07 adicional, 10:32–10:34: durante 101,2 s `GetLastInputInfo` avanzó de 302,9 a 404,1 s
y Firefox permaneció en primer plano; Naygo en segundo plano consumió 0,172 s CPU y promedió 78,8
MiB working set / 46,8 MiB privados. La carga total reportada fue 16–67 %, así que no se certifica
reposo de la máquina. Evidencia y límites en [`BENCHMARKS.md`](BENCHMARKS.md).

La inspección anterior del PID 48444 (hash previo) sigue como evidencia histórica y no debe
confundirse con la inspección del proceso actual. Los dos resultados son estructurales y no
reemplazan la aceptación con Narrador. Registro: [`VALIDACION-UIA-2026-10-18.md`](VALIDACION-UIA-2026-10-18.md).

Revalidación Linux 2026-10-19: WSL2 registra Ubuntu pero no puede iniciar porque la plataforma de
máquina virtual está deshabilitada (`HYPERV_NOT_INSTALLED`). No se modificó la configuración de
Windows. El check y Clippy cruzados compilan todos los targets Linux, incluido el smoke test de
`statvfs`; la consulta real del filesystem sigue requiriendo Ubuntu 26.04 nativo.

Reintento WSL 2026-10-19: `wsl --list --verbose` sigue mostrando Ubuntu como stopped y `wsl -d Ubuntu`
falla con `HCS_E_HYPERV_NOT_INSTALLED`, indicando que Virtual Machine Platform/virtualización no está
habilitada. No se intentó habilitar características ni reiniciar el PC. El copyright oficial del
runtime Ayatana GLib 2.0.1-1build1 de Ubuntu Resolute coincide por SHA-256 con el aviso preservado en
`licenses/linux/`; evidencia ampliada en
[`VALIDACION-L00-AUDITORIA-2026-10-17.md`](VALIDACION-L00-AUDITORIA-2026-10-17.md).

Revalidación cruzada posterior a W-06D, 2026-10-19: `cargo check --workspace --tests --locked
--offline --target x86_64-unknown-linux-gnu --jobs 1` pasó; Clippy `--all-targets -D warnings`
pasó por separado en `naygo-core`, `naygo-platform` y `naygo-ui-slint`. El intento agregado de
Clippy workspace se canceló tras quedar sin `rustc` ni salida; no se afirma que corra el binario ni
que esté validado el runtime en Ubuntu.

Repetición W-07 local del 2026-10-19: tres corridas dieron mediana externa de 208,813 ms para
enumerar 100.000 entradas; tres corridas tras evaluar una microoptimización dieron 208,955 ms.
La diferencia frente a los 161,659 ms observados el 15 de octubre es una señal sin causa aislada;
la microoptimización se revirtió y no cambió el release. Fixtures temporales eliminadas. W-07
sigue abierto hasta repetir con fingerprint y estado controlados y completar la matriz UI/UNC.
Detalle y archivos no versionados: [`BENCHMARKS.md`](BENCHMARKS.md).

Evaluación intermedia W-07 del 2026-10-19 (superada por la reconstrucción vigente): primero se probó
una variante `file_name()` que evitaba una asignación temporal, pero se reemplazó por reutilización
de `PathBuf`; sus resultados de disco fueron variables. La nota histórica que decía que 0.5.3 estaba
pendiente de regeneración ya no aplica: la distribución final está verificada arriba. No hubo
instalación ni firma.

Registro previo: 2026-10-18 21:37: se actualizó 0.5.2 desde el setup verificado y se dejó Naygo
abierto como PID 51452, respondiendo. El EXE instalado coincide con Release
(`210901465840187a497dca452d56b0cbf843807c11c52079e0bbeca0ac1d4673`); los cuatro JSON de usuario
parsean correctamente y el PDB previo se conserva. UIA de solo lectura encontró 88/88 controles
enfocables con nombre, sin truncar el árbol; falta Narrador y validación interactiva. Captura W-07
pasiva sobre este Release: 60,6–60,7 MiB working set y 0,06/0,09/0,05 s CPU en tres ventanas de
30 s; no mide input o presentación y no cierra W-07. Detalles en [`BENCHMARKS.md`](BENCHMARKS.md)
y [`VALIDACION-UIA-2026-10-18.md`](VALIDACION-UIA-2026-10-18.md).

Actualización 2026-10-18 23:05: tras incorporar roles/nombres/acciones UIA en «Subir carpeta»,
recientes y menús de contexto/filtro, pasó el gate completo (1.464 pruebas, 8 ignoradas, Clippy y
formato). `dist/` se regeneró bajo supervisor (pico 10,64 GiB), hashes verificados y ZIP íntegro.
El setup se instaló con autorización previa; Naygo quedó abierto y respondiendo como PID 50496. El
EXE instalado coincide con Release (`1f93266279feae63839bbd125aaa30c181bf85b09f1c8457a2eb9e52c3c6db2f`),
los cuatro JSON de usuario parsean y el PDB anterior se conserva. UIA: 350 descendientes, 88/88
enfocables con nombre, árbol completo. W-13 sigue pendiente de Narrador y recorrido manual; W-14
firma Authenticode también sigue abierta. Evidencia: [`VALIDACION-W13-CONTROLES-2026-10-18.md`](VALIDACION-W13-CONTROLES-2026-10-18.md).

Muestra W-07 adicional del 2026-10-18, 21:28–21:29: el PID instalado 48792 tuvo 93,9 MiB de
working set, 53,6 MiB privados, 481 handles, 40 hilos y CPU delta 0,078/0/0,016 s en tres
ventanas de 30 s. Se observó un proceso y no había mapeos SMB. El EXE instalado no coincide con
el Release local; la muestra fue pasiva, sin control de inactividad, y no valida latencia ni la
build actual. W-07 sigue abierto. Evidencia: [`BENCHMARKS.md`](BENCHMARKS.md).

Revisión L-03 del 2026-10-18: se compararon las funciones FFI del tray GLib con headers
AppIndicator 2.0.1 y GLib/Gio 2.88; `GIOCondition`/`GConnectFlags` y el callback ahora usan los
tipos C correspondientes. Check y Clippy Linux cruzados de todos los targets pasaron. El gate
Windows pasó 1.464 pruebas, 8 ignoradas, Clippy y formato. `dist/` se regeneró bajo supervisor
(pico 10,64 GiB); ZIP íntegro, hashes verificados y EXE interno coincidente con release. Setup/EXE
sin firma; no se instaló. La ejecución de bandeja aún requiere Ubuntu 26.04 nativo. Evidencia:
[`VALIDACION-L03-CROSS-2026-10-17.md`](VALIDACION-L03-CROSS-2026-10-17.md).
UIA pasiva posterior del PID 48792 (EXE anterior al release actual): 759 controles, 80/80
enfocables con nombre, árbol completo y sin truncar; no valida Narrador ni la build nueva. El
escritorio está desbloqueado, pero la captura visual agotó el tiempo. Evidencia:
[`VALIDACION-UIA-2026-10-18.md`](VALIDACION-UIA-2026-10-18.md).

Actualizado: 2026-10-18 (America/Santiago). L-02, importación de configuración: el importador
omite rutas incompatibles y capacidades Linux aún no implementadas antes de escribir; presenta un
aviso traducido. Gate: 1.464 pruebas aprobadas, 8 ignoradas, Clippy/formato correctos y paridad i18n
en diez idiomas. UI Linux cruzada: check de todos los targets y Clippy -D warnings aprobados.
Build supervisado: pico 10,63 GiB; release 28m35s e Inno Setup 116,890 s. ZIP
`dist/Naygo-0.5.2-portable.zip`, SHA-256
`3f919371d9452a1b60041e159ea41664599b360c0480335617f7c1b15740ed5b`; setup
`dist/Naygo-0.5.2-setup.exe`, SHA-256
`955b5aa5d822024ff129af7d686c74090d3bea61be39cee119de723308a19564`. Manifiesto y ZIP verificados;
ZIP sin PDB, EXE interno coincide con Release; el setup sigue sin Authenticode. La build quedó en
`dist/`, no se instaló ni publicó. Evidencia:
[`VALIDACION-L02-IMPORT-CONFIG-2026-10-18.md`](VALIDACION-L02-IMPORT-CONFIG-2026-10-18.md).
W-13: las filas de archivos y las acciones del Inspector exponen roles, nombres, estados y acciones
accesibles; aceptación UIA/Narrador sobre la build nueva sigue pendiente.
Fix de modificadores: el modal de nuevas carpetas limpia Ctrl/Shift al abrir y cerrar/aplicar, y los
clics sincronizan su modificador desde el gesto real antes de interpretar doble clic. Gate: 1.460
pruebas aprobadas, 8 ignoradas, Clippy y formato correctos. Build supervisado previo: pico 10,64 GiB;
release 29m44s e Inno Setup 125,250 s. Esa build se instaló con autorización previa y quedó abierta,
PID 48792 respondía; settings/workspace/favorites/templates y PDB se preservaron. Evidencia:
[`VALIDACION-MODIFICADORES-2026-10-18.md`](VALIDACION-MODIFICADORES-2026-10-18.md).
W-13 da nombres accesibles localizados al editor del preview, campos de nombre, búsqueda,
configuración, favoritos, recetas, consultas, toolbar y unidades; aceptación UIA/Narrador pendiente. L-05,
capacidad de elevación y subpaso Linux `xdg-open` pasaron el gate Windows;
la UI Linux completa además pasó check/Clippy cruzados para `x86_64-unknown-linux-gnu` desde Windows
con Fontconfig dinámico, incluido el test POSIX al compilar todos los targets. La ejecución nativa
Linux sigue pendiente. Evidencia: [`VALIDACION-L05-ADMIN-2026-10-18.md`](VALIDACION-L05-ADMIN-2026-10-18.md)
y [`VALIDACION-L03-CROSS-2026-10-17.md`](VALIDACION-L03-CROSS-2026-10-17.md).
La instancia instalada PID 48792 sigue activa; su EXE conserva la build anterior, SHA-256 `94070d7b4dea91ebb9670cfb6f63abf63d8644f06556a4ff0a2c569c2fee3746`. La distribución L-02 actual está solo en `dist/` y no se instaló; settings/workspace/favorites/templates no se tocaron en esta entrega.
UIA pasiva de E-6 vio 751 descendientes y 79/79 controles enfocables con nombre, sin truncar el árbol.
El menú «Más» no se abrió; la captura de ventana expiró dos veces, así que aún falta revisar la barra
visualmente, usar Narrador y comprobar sus acciones. L-03 aún
requiere runtime y escritorio Ubuntu nativos. Evidencia:
[`VALIDACION-L03-CROSS-2026-10-17.md`](VALIDACION-L03-CROSS-2026-10-17.md) y
[`VALIDACION-READDIR-2026-10-18.md`](VALIDACION-READDIR-2026-10-18.md).
La inspección UIA anterior del PID 25124 encontró un `Edit` enfocable sin nombre, sin identificarlo;
esa instancia era de una distribución previa. Se añadieron nombres accesibles a los campos Slint y
se inspeccionó el árbol del PID 41292: no mostró controles enfocables sin nombre, aunque el menú
«Más» y los diálogos estaban cerrados. La captura de escritorio falló antes de poder abrirlos; falta
comprobarlos con Narrador y revisar la interfaz visualmente. Ver
[`VALIDACION-UIA-2026-10-18.md`](VALIDACION-UIA-2026-10-18.md).
La muestra pasiva de E-6, PID 41292, dio 78,2 MiB working set, 47,1 MiB privados, 388 handles,
26 hilos y CPU 0/0/0 s en tres ventanas de 30 s; no observó procesos auxiliares ni mapeos SMB.
No se controló inactividad ni mide input, latencia o presentación, por lo que no cierra W-07.
Registro y límites en `docs/BENCHMARKS.md`.
Evidencia: [`docs/VALIDACION-E4-CATALOGO-2026-10-18.md`](VALIDACION-E4-CATALOGO-2026-10-18.md),
[`docs/VALIDACION-TOOLBAR-2026-10-18.md`](VALIDACION-TOOLBAR-2026-10-18.md).
Favoritos globales o propios se eligen al crear un espacio; v1 migra a global y las diez traducciones
están incluidas. El gate completo pasó 1.450 pruebas (8 ignoradas), Clippy `-D warnings` y formato.
Evidencia E-4: [`docs/VALIDACION-E4-FAVORITOS-2026-10-18.md`](VALIDACION-E4-FAVORITOS-2026-10-18.md).
El PDB no se incluye en el ZIP. Evidencia de cierre e instalación en
[`docs/VALIDACION-CIERRE-SEGURO-2026-10-17.md`](VALIDACION-CIERRE-SEGURO-2026-10-17.md).
La muestra pasiva E-5 del proceso recién instalado midió 76,7–77,9 MiB de working set, 45,5–46,7 MiB
privados, 385–387 handles, 26–27 hilos y CPU 0,032/0,359/0 s en tres ventanas de 30 s; comenzó 79 s
después del arranque y no controló inactividad, así que no cierra W-07. UIA expuso 721 descendientes,
536 con nombre y 55 enfocables; todos los enfocables tenían nombre accesible. No sustituye Narrador
ni aceptación manual. Evidencia en
[`docs/VALIDACION-FAVORITOS-2026-10-17.md`](VALIDACION-FAVORITOS-2026-10-17.md),
[`docs/BENCHMARKS.md`](BENCHMARKS.md) y
[`docs/VALIDACION-WINDOWS-0.5.2-2026-10-15.md`](VALIDACION-WINDOWS-0.5.2-2026-10-15.md).
Evidencia histórica anterior:
[E-1](VALIDACION-BUILD-2026-10-17-E1.md), [L-01A / build](VALIDACION-BUILD-2026-10-17-L01A.md),
[L-01A JSON y distribución](VALIDACION-BUILD-2026-10-17-L01A-JSON.md),
[W-16](VALIDACION-W16-2026-10-17.md) y
[visor 3D](VALIDACION-VISOR-3D-2026-10-17.md).

W-01–W-06D, W-08–W-11C, W-15, W-16 y E-2 (crash en drop desde 7-Zip) están implementados, probados y empaquetados. La L-11 está ahora instalada, abierta y responde; en su log del 17-10 el único panic es el evento viejo de las 12:58, anterior a la actualización. El registro confirma que el cierre E-2 ocurrió en la build anterior al procesar el resultado del drop; no confirma que el OBJ se copiara. Queda pendiente repetir el drop con el fixture de aceptación. W-12 y la corrección Windows de la
Papelera pasan validaciones locales; falta ejecutar el workflow actualizado en GitHub sobre
Windows Server 2025. La interfaz core↔platform para evitar la carrera de destinos Windows, el
reemplazo transaccional y la recuperación de Replace tras terminar el proceso están implementados,
probados y empaquetados; siguen pendientes las pruebas manuales de aceptación, permisos/disco lleno
y pérdida de energía. W-07 necesita mediciones interactivas reales; el callback Slint de teclado tuvo
una medición inicial de 256 muestras headless (p50 7,6 ms, máximo 151,3 ms) y tres repeticiones
posteriores con mediana p50 de 8,0 ms y máximo 64,3 ms. Mide el callback, no entrada física,
Windows/Winit ni presentación (ver `docs/BENCHMARKS.md`). W-13 necesita aceptación manual
de Narrador, foco/teclado, DPI y flujos completos. W-14 depende de SignPath, aprobación y credenciales
externas; exe y setup continúan sin firmar.

Linux está pospuesto por la prioridad Windows. L-00 tiene elegidos Ubuntu 26.04 LTS x86_64, GLib y
GPL-3 Linux-only, y Nicolás confirmó el alias histórico de autoría; `.mailmap` lo normaliza sin
reescribir commits. El binding FFI GLib ahora está implementado localmente y pasa typecheck/Clippy
en un harness cruzado aislado. El 18 de octubre `core` y `platform` pasaron `cargo check` y Clippy
para `x86_64-unknown-linux-gnu` con todos sus targets desde Windows. El código Linux sigue parcial:
faltan build y tests nativos, validación de ABI/runtime e integración de escritorio, supervisor de
preview y empaquetado. Evidencia: [`docs/VALIDACION-L03-CROSS-2026-10-17.md`](VALIDACION-L03-CROSS-2026-10-17.md).
Revalidación 2026-10-18: WSL muestra Ubuntu registrado, pero `wsl -d Ubuntu` falla con
`HYPERV_NOT_INSTALLED`; la capa de virtualización no está disponible y no se cambió Windows. Por
ahora no se puede ejecutar la validación nativa desde este host.
La auditoría local de licencias/autoría del 2026-10-17 está en
[`docs/VALIDACION-L00-AUDITORIA-2026-10-17.md`](VALIDACION-L00-AUDITORIA-2026-10-17.md): historial
con un solo autor normalizado y 275/275 fuentes con SPDX; quedan pendientes revisión de fragmentos
importados, fuente correspondiente y notices finales. La revisión local del source package Ayatana
GLib 2.0.1-1build1 confirmó GPL-3 para la biblioteca y CC0-1.0 para una plantilla, coherente con la
excepción Linux-only aprobada. El 18 de octubre se verificaron la firma del `.dsc`, la firma upstream
del tar y la cadena Ubuntu `InRelease → Packages → .deb`; Ubuntu autentica el hash del `.deb` desde
metadatos firmados, no con una firma individual. La verificación aislada con PGPy no revisa
autosignaturas/revocaciones. Siguen pendientes fuente correspondiente, notices finales y confirmar
runtime en Ubuntu nativo.
El 18 de octubre se prepararon avisos preliminares separados para Linux con GPL-3, atribución y
hashes del runtime Ayatana de referencia; aún no son el inventario final ni sustituyen la fuente
correspondiente de la versión que distribuya Naygo. Detalle en
[`THIRD-PARTY-NOTICES-LINUX.md`](../THIRD-PARTY-NOTICES-LINUX.md).

## Historial de implementación

Las anotaciones siguientes registran avances por fecha. Si indican que una distribución no se había
instalado o que Naygo seguía en una build anterior, describen ese momento histórico; el estado vigente
de instalación y pendientes es el resumen fechado de arriba.

Revalidación Linux del 2026-10-16: core/platform pasan `cargo check --tests` y Clippy cruzados para
`x86_64-unknown-linux-gnu`; la UI falla al resolver `glib-sys` por falta de `pkg-config` configurado
y sysroot. `wsl.exe -d Ubuntu` devuelve `HYPERV_NOT_INSTALLED`; no se cambió la configuración de
Windows. El menú GLib añade además un riesgo de compatibilidad SNI/GNOME documentado en el plan Linux.
En el trabajo actual se implementó localmente el cleanup de directorios vacíos al mover una carpeta
y se cerró la carrera de destino en el movimiento rápido Windows mediante un adaptador inyectado
core↔platform (`MoveFileExW` sin flags de reemplazo ni copia entre volúmenes). Las dos pruebas de
plataforma y las dos regresiones de flujo UI pasan. El gate workspace terminó con 1.417 pruebas,
8 ignoradas y Clippy limpio; check/Clippy cruzados de core/platform para Linux también pasan. La
distribución 0.5.2 se regeneró bajo supervisor con pico 10,45 GiB y está en `dist/`; esta compilación
no se ha instalado y la instancia abierta no se modificó. Hashes y alcance en
`docs/VALIDACION-BUILD-2026-10-16.md`.
También se implementó localmente el reemplazo de árboles con staging hermano, restauración del
destino anterior si el rename final falla, conservación del origen de Move hasta publicar y
revalidación de contenido antes de retirarlo. Pasaron las siete pruebas focales y el gate workspace
(1.423 tests, 8 ignorados), Clippy, formato, diff check y check/Clippy cruzados Linux de core y
platform. La regeneración supervisada de distribución terminó con pico 10,45 GiB; ZIP/setup y
hashes están registrados en `docs/VALIDACION-BUILD-2026-10-16.md`. No se instaló esta compilación.
Windows requiere dos renames para sustituir una carpeta, así que aún puede haber un intervalo corto
sin ruta. El delta local de L-01B registra la transacción antes del staging y recupera al listar el
padre: revierte si el proceso terminó entre renames y completa limpieza si el árbol nuevo se publicó.
El gate protegido pasó 1.433 tests (8 ignorados), Clippy, formato y diff check; check/Clippy
cruzados Linux de core/platform también pasan. La distribución 0.5.2 se regeneró bajo supervisor
(pico 10,45 GiB), sin instalarla. Se probó terminación abrupta de proceso en ambos límites, no pérdida
de energía. Hashes y evidencia en `docs/VALIDACION-BUILD-2026-10-16.md`.
El smoke del portable con 100 archivos produjo spans internos de listado/modelo, pero no de entrada:
UIA encontró 23 filas visibles y la captura gráfica falló dos veces, sin enviar teclas/clics. Tras
restaurar los cuatro JSON y relanzar la instalación, el PID 9204 respondió; 3×30 s pasivos midieron
79,1 MiB working set, 47,5 MiB privados y CPU 0,016/0,031/0,016 s. No se controló inactividad ni
latencia. Una lectura anterior del PID 12532 dio CPU mayor tras inspección UIA; causa no aislada.
La matriz interactiva sigue pendiente.
W-07: portable aislado con fixture sintética de 10.000 archivos produjo spans internos de carga y
actualización; una ventana de 12 s midió 82,1 MiB working set, 51,4 MiB privados y 0,172 s CPU.
Dos capturas de ventana agotaron el tiempo y no se enviaron teclas; no hubo `ui.input.*`, por lo que
la interacción real sigue pendiente. La build instalada volvió a abrirse y permanece en ejecución.
Una fixture limpia de 100 archivos confirmó que el argumento CLI llega al árbol UIA (25 filas visibles),
pero Tab/Abajo no cambiaron el foco ni produjeron callbacks; el helper careció de geometría para clic
y su captura gráfica agotó el tiempo. No se cuenta como interacción validada.
Un segundo inicio con otro argumento CLI confirmó en UIA que la carpeta solicitada reemplaza la sesión
sintética anterior (25 filas nuevas, ninguna anterior). Esto valida el override CLI, no la navegación
por teclado ni la latencia interactiva.
W-15 cerrado: el upgrade in-place sin símbolos conservó el PDB real; en sandbox, la tarea marcada
lo reemplazó por los símbolos release y la desinstalación los eliminó. No hay un flujo de reparación.
Repetición pasiva sobre PID 41724, 14:43–14:44: un proceso, cero SMB, picos de 78,7 MiB working
set, 47,7 MiB privados, 390 handles y 26 hilos; CPU por ventana 0,125/0,093/0,032 s. Respondía
al terminar. No se controló inactividad ni hubo interacción; no cierra W-07. JSON en `BENCHMARKS.md`.
W-07 tiene benchmark sintético e instrumentación opt-in de tick, fases UI, callbacks de
teclado/filas/sort, listados, preview, validación de rutas y cola de packs. Las suites del workspace,
Clippy, formato, diez idiomas y el build supervisado del último delta pasan. Sigue pendiente medir rendimiento real (interacción,
CPU/RAM, filesystem local/UNC y escenarios de preview); el build no sustituye esas mediciones.
W-13: la primera inspección UI Automation sólo veía la ventana raíz; se habilitó `accessibility`
de Slint/AccessKit y se completaron roles y navegación de la barra de pestañas. El gate W-13 pasó
1.403 tests (8 ignorados), Clippy y formato. La primera instalación del delta tuvo EXE SHA-256
`c492982e5f70d6a8d1a2e78982035554e6a1065a3a5fbbf3fdd05eb82540bdca`; fue reemplazada por la
regeneración L-01A registrada arriba. La primera instalación reportó 312 descendientes UIA (228 con
nombre, 46 enfocables).
Los cuatro JSON de configuración/sesión coinciden byte a byte con el respaldo tras el cierre limpio.
Un probe UIA aislado del componente de pestañas confirmó `Tab=1`, tres `TabItem` enfocables y selección
correcta. La prueba real con Narrador, foco y orden de teclado sigue manual. El setup permanece sin
firma hasta W-14. La instalación conserva un PDB antiguo opcional (≈290 MB) con hash `eaad3dd9…`.
W-15 está cerrado: upgrade sin símbolos conservó el PDB real; el sandbox confirmó conservación al
desmarcar, reemplazo al marcar y retiro al desinstalar. El producto no ofrece reparación
(`NoRepair=1`). Reinspección de solo lectura del PID 25520: 731 descendientes UIA, 541 con nombre y
55 enfocables; el proceso responde y el EXE coincide con el release actual. La sesión no contiene
grupos de pestañas, así que esta captura no valida Narrador ni la navegación de la barra. Evidencia
detallada en `VALIDACION-BUILD-2026-10-16.md`.
Un ensayo aislado adicional con clic y teclado no fue concluyente: UIA observó el clic, pero la
aserción tras Enter no encontró selección; no se cuenta como interacción aprobada. La prueba dirigida
headless `tab_bar_accessibility` pasó 1/1.
Una segunda repetición se detuvo antes de enviar teclas porque Windows no permitió activar el probe
en primer plano; UIA confirmó sólo la selección inicial. No demuestra un defecto del componente.
Una captura pasiva de esta build (3×30 s) registró 60,5–60,6 MiB de working set, 29,2 MiB privados,
385–387 handles, 26–27 hilos y 0–0,05 s CPU, sin SMB. No confirma inactividad, interacción ni trabajo
de UI y no cierra W-07. Detalle en `docs/BENCHMARKS.md`.
Una repetición pasiva posterior (3×30 s) sobre la misma instalación observó un solo proceso, 60,9 MiB
de working set, 29,2 MiB privados, 385 handles, hasta 24 hilos y 0,047–0,141 s de CPU por ventana;
sin SMB. No confirma reposo ni mide UI y tampoco cierra W-07.
El 16 de octubre L-02 avanzó parcialmente: Linux resuelve XDG para configuración, datos y estado, usa
`$HOME` para navegación y aborta con aviso si falta un home absoluto; workers crean directorios.
Windows conserva la ubicación portable junto al ejecutable. Se corrigieron además fallbacks de UI
que usaban `USERPROFILE`/`C:/` al iniciar o reubicar paneles, y respetan el Home configurado. El test
de recuperación de carpeta/paneles pasó en Windows. El host no ofrece ejecución Linux nativa ni
verificación de la UI Linux.
Sets de íconos y PNG de usuario nuevos usan XDG data; las lecturas mantienen fallback a la ubicación
XDG config anterior, y data gana duplicados. Import/export de packs conserva esa distribución. No hay
migración ni borrado automático. Windows sigue usando un solo directorio portable. Pasaron pruebas
del core, packs UI y check/Clippy cruzados del core; falta el check de UI Linux y ejecución nativa.
L-01A avanzó en la representación: listado, árbol y búsquedas core escapan bytes POSIX no UTF-8 como
`\\xNN` y duplican backslashes literales; las acciones conservan la ruta `PathBuf`. Pasaron 909 tests
core y 22 flujos Windows; el check y Clippy Linux cruzados compilan los tests POSIX. La regeneración
Windows ya está instalada; falta ejecutar los tests Unix en Linux nativo.
Nuevo subpaso L-01A: los paths guardados en favoritos, cestas/espacios, búsquedas y puntos de
comparación preservan bytes POSIX no UTF-8 mediante `unix-bytes-v1`; las rutas UTF-8 mantienen su
JSON histórico. La comparación utiliza claves nativas y valida segmentos con los bytes originales.
Pasaron 944 tests core + 22 flujos, check y Clippy Linux cruzados. El gate de workspace/Clippy pasó
(pico 8,53 GiB) y la distribución se regeneró bajo supervisor (pico 10,48 GiB), se instaló y quedó
abierta. Falta ejecutar las pruebas Unix en Linux nativo; la automatización para repetir el drop desde
7-Zip agotó timeout al capturar la ventana y el archivo destino del fixture preexistía.
La última repetición sintética Windows de W-07 quedó bajo el umbral comparativo orientativo. El build
previo al cierre W-10C pasó 1.400 tests/7 ignorados y el portable de ese build arrancó/cerró con
`--shutdown`; su captura pasiva 3×30 s registró 55,6 MiB working set y 26,5 MiB privados. Tras W-10C,
el gate actualizado pasó 1.402 tests/8 ignorados, Clippy, formato y `git diff --check`; se regeneró
0.5.2 bajo supervisor con pico release 10,42 GiB. ZIP/setup/EXE, checksums y contenido portable se
verificaron; EXE y setup siguen `NotSigned` hasta configurar SignPath. El 16 de octubre Nicolás
autorizó actualizar la instalación de usuario: Inno Setup terminó con código 0, la versión registrada
es 0.5.2 y el EXE instalado coincide con SHA-256
`493ad821665db26384464e87e0537e5afa2dff6de65ce529bb7f3e4d9c940f04`. La ventana Naygo quedó
visible y respondiendo en Windows 10 Home 19045. Esa primera instalación era el artefacto W-10C
anterior; después AccessKit y la barra accesible de pestañas se instalaron como actualizaciones
sucesivas. El EXE instalado vigente es
`c492982e5f70d6a8d1a2e78982035554e6a1065a3a5fbbf3fdd05eb82540bdca`; detalles e inventario UIA en
`VALIDACION-BUILD-2026-10-16.md`. El ZIP vigente se verificó como archivo, pero no se abrió en
paralelo para evitar el mutex de instancia única; su ejecución portable y la aceptación interactiva
siguen pendientes. Inno Setup no modifica configuraciones; para la última actualización, los cuatro
JSON se compararon con el respaldo tras el cierre limpio y coinciden byte a byte.
Un harness de software headless en perfil release, tres pasadas, midió a 100k una mediana de
453,691 ms para sync inicial, 47,235 ms para modificar una fila, 561,367 ms para append y 3,086 ms
para render. No mide presentación Winit ni interacción; W-07 sigue abierto. Detalle en
`docs/BENCHMARKS.md`.
La revalidación release posterior, en una sola pasada (no agregada a las medianas anteriores), midió
100k en 269,938 ms inicial, 1,479 ms no-op, 27,949 ms al modificar una fila, 21,101 ms append y
2,301 ms render p50; el Job alcanzó 8,67 GiB durante la compilación supervisada. No incluye Winit,
presentación física ni entrada, así que no cierra W-07. Detalle y log en `docs/BENCHMARKS.md` y
`docs/VALIDACION-BUILD-2026-10-16.md`.
El intento local de generar teclado con automatización tampoco produjo callbacks: Windows no permitió
activar la ventana desde el proceso de prueba y se evitó enviar teclas a la app que estaba en primer
plano. Después, un test headless invocó directamente 256 veces el callback Slint real de `wire_nav_keys`;
en release registró p50 12,900 µs y máximo 183,700 µs, con 256 muestras `ui.input.keyboard_callback`.
Esto mide el handler y su trabajo directo, no foco, entrada del sistema, Winit ni presentación; la
medición interactiva todavía requiere Naygo enfocado. Detalle en `docs/BENCHMARKS.md`.
La apertura por CLI de corpus local mostró un costo relevante: en tres ejecuciones con 100k, la mediana
de `ui.sync_rows` fue 118,962 ms y de `ui.tick` 139,940 ms, frente a 15,087/26,091 ms con 10k. Una
captura de 90 s sobre 100k midió hasta 265,7 MiB working set; `--shutdown` no respondió dentro de
20 s y se cerró forzando sólo el proceso aislado. El hallazgo apunta al costo de reconstruir filas
durante el streaming. W-10C implementado y empaquetado: el modelo agrega solo el lote nuevo si el
prefijo visible permanece intacto; tres corridas release midieron 21,279 ms de mediana al añadir una
fila a 100k, frente a 561,367 ms previos (~26,4×, headless). Gate completo y Clippy pasan. W-07 sigue
abierto. Evidencia en `docs/BENCHMARKS.md` y `docs/VALIDACION-BUILD-2026-10-16.md`.
W-12 local revalida pins, sintaxis YAML y pruebas sintéticas de firma; el último CI remoto sigue
fallido en `abdef402`. `HEAD` remoto coincide con la base local, pero los cambios W-12 están sin
publicar, así que aún no existe un run que los pruebe en Windows Server 2025.
L-02 también sigue parcial porque falta entorno Linux nativo. Hashes y evidencia en
`VALIDACION-BUILD-2026-10-16.md`. La prioridad vigente es cerrar primero todos los bloques Windows.
El 14 de octubre se repitió tres veces el benchmark sintético Windows en el mismo Ryzen 7 1700X:
las medianas de 100.000 filas variaron +1,0–9,4 % frente a la base del 13 de octubre, dentro del
umbral orientativo de 15 %. Esto no mide la UI completa ni cierra W-07. Nicolás pidió priorizar
todos los bloques Windows antes de retomar Linux.
W-07 instrumenta firma, construcción, conversión y aplicación de filas Slint por fase. W-10B compara
filas por contenido visible, incluidas sus celdas anidadas, y evita notificaciones masivas al insertar
una fila. El watcher propaga deltas para `Modified` estable y el sync actualiza solo esas filas: mediana
31,874 ms a 100k frente a 353,568 ms antes (~91 % menos), en tres corridas headless. La carga inicial
y el append requieren repetición contra baseline controlado por la dispersión observada. Falta medir
interacción real con `NAYGO_PERF_METRICS=1`.
El 15 de octubre se tomó una muestra pasiva de 20 s y tres ventanas de 30 s del proceso instalado:
72,4–72,8 MiB de working set, 54,8–54,9 MiB privados, 430–432 handles, 33–35 threads y deltas de
CPU de 0,14/0,03/0,16 s en las ventanas de 30 s. No se capturaron auxiliares ni interacción; no
había unidades SMB mapeadas. Esta observación no cierra W-07.
El script `scripts/capture-w07-process.ps1` permite repetir la captura pasiva del proceso y guardar
JSON en `target/agent-out/`; fue probado con una ventana corta. No inicia la aplicación ni habilita
las métricas internas. Nueva observación tras el reinicio: tres ventanas de 30 s sobre el proceso
instalado, working set/privada de 41,6/40,1 MiB, 349 handles, 17 threads, CPU
0,031/0,047/0,016 s y sin SMB. La sesión/actividad de fondo no se controló, no se midió interacción
ni auxiliares, así que la diferencia frente a muestras anteriores no se atribuye a una causa y W-07
sigue abierto. JSON: `target/agent-out/w07-process-20261015-080019.json`.
Nueva muestra pasiva a las 14:22 del mismo PID 23856: 72,9 MiB working set, 46,9 MiB privados,
441 handles, 19 threads, deltas CPU 0,094/0,047/0 s y sin SMB. Sin interacción, inactividad
confirmada ni procesos auxiliares; JSON en `target/agent-out/w07-process-20261015-142335.json`.
Nueva captura pasiva 19:04–19:06 del mismo PID: 46,6/47,5 MiB working/private set, 437 handles,
17 threads y CPU sin incremento en tres ventanas de 30 s; sin SMB. La instancia seguía abierta
desde las 07:08; no se confirmó inactividad, no se midió interacción ni procesos auxiliares y la
variación de memoria queda sin atribuir. JSON en
`target/agent-out/w07-process-20261015-190632.json`; W-07 permanece abierto.
El ejecutable de esa instancia era el build del 14 de septiembre (hash `c7882850…`), distinto al
release 0.5.2 que ahora se probó como portable (`a2810673…`); las capturas anteriores no describen
la entrega actual. La prueba opt-in reciente sólo aporta telemetría parcial; faltan las muestras de
callback y la matriz de aceptación.
La matriz y hoja para probar esta entrega con resultados por entorno está en
[`VALIDACION-WINDOWS-0.5.2-2026-10-15.md`](VALIDACION-WINDOWS-0.5.2-2026-10-15.md); el smoke portable
de arranque/cierre está anotado, y el resto de casos manuales sigue pendiente.
El capturador pasivo W-07 pasó a schema v3: muestrea los procesos descendientes cada 2 s por defecto,
reporta identidades vistas al menos una vez y picos de recursos por ventana. Un intervalo menor puede
detectar lectores auxiliares más breves, a cambio de más consultas y sobrecarga; procesos más cortos
que el intervalo aún pueden perderse. Se validó con un hijo que termina antes del fin de ventana.
Esto no reemplaza la medición interactiva de W-07 ni caracteriza el release 0.5.2; limitaciones en
`BENCHMARKS.md`.
Los JSON de prueba están en `target/agent-out/w07-process-tree-child-smoke.json` y
`target/agent-out/w07-process-tree-root-smoke.json`.
La distribución fue regenerada tras este delta; ZIP y setup están verificados y siguen sin firma.
W-08 tiene recogida limitada, canal acotado y presupuesto global rotativo probados y empaquetados.
La comparación de primera fila, p95 de interacción y tiempo total sigue pendiente.
W-09 incorpora admisión compartida de ocho hilos para Files, árbol y probes; pruebas, Clippy y
distribución supervisada pasaron. Falta aceptación en filesystem de red realmente lento.
W-10A comparte los índices cacheados con bridge/comparación; suite, Clippy y microbenchmark pasaron,
y la distribución supervisada 0.5.2 está regenerada con hashes verificados. El hit compartido mide
0,000061 ms frente a 0,021924 ms con copia para 100.000 filas. W-10B implementó igualdad semántica
de `RowData`/celdas, aplicación incremental y actualización selectiva con deltas estables del watcher.
Tres mediciones headless a 100.000 filas dan una mediana de 31,874 ms para modificar una fila, frente
a 353,568 ms en la muestra anterior. El append mantiene aplicación estructural incremental; sus cifras
nuevas deben compararse con una línea base controlada. El gate completo pasó y la distribución 0.5.2
se regeneró bajo supervisor con hashes verificados. La sesión interactiva completa queda pendiente.
Evidencia en `BENCHMARKS.md` y `VALIDACION-BUILD-2026-10-15.md`.
W-11A ya reutiliza el parse del preview PDF, limita el texto extraído a 8.000 caracteres y el
contenido recorrido a 500 páginas; sus pruebas dirigidas, suite, Clippy y build supervisado pasaron,
con hashes de distribución verificados. Falta aceptación instalada y medición con corpus PDF.
W-11B limita el índice ZIP a 10.000 entradas/6 MiB, lecturas a 8 MiB, TAR a 32 MiB y TAR.GZ a
16 MiB comprimidos/32 MiB expandidos, además de rutas y sumas. Sus 28 pruebas de preview, workspace
completo, Clippy global y empaquetado supervisado pasan; los hashes de distribución están
verificados. La aceptación instalada en un equipo externo sigue pendiente.
W-11C preserva el último carácter válido al truncar texto UTF-8/UTF-16 y evita copiar completo el
SVG al detectar el canvas. Pruebas sintéticas de 4.000 elementos, dimensiones extremas de imagen,
líneas grandes, Unicode y animación pasan junto con workspace/Clippy; build y hashes constan en la
bitácora. Falta medir con carpetas/documentos reales y comparar la latencia, dentro de W-07.
W-12 fija Rust 1.96.0 y hace `--locked` el gate de CI/release, con un solo job. Hay informe benchmark
JSONL periódico/manual y smoke headless sobre el ZIP en runners efímeros. El último run remoto visible
es `main`/`abdef402` del 7 de septiembre: fallaba la restauración desde Papelera en Windows Server
2025 porque el runner no refrescaba su enumerador virtual y no exponía el verbo contextual `undelete`.
`PostDeleteItem` ahora guarda el nombre de parsing del `IShellItem` recién reciclado y el restore lo
abre directamente; prioriza `IFileOperation::MoveItem` y conserva el menú contextual como respaldo.
La enumeración de la carpeta virtual ahora se difiere hasta usar un recibo reconstruido, así que un
runner sin proveedor de enumeración puede restaurar el recibo recién creado.
El caché de identidades tiene capacidad fija de 1.024, expulsa por FIFO y al reemplazar una ruta la
marca como reciente; una prueba dirigida cubre ese orden.
El round-trip de Papelera ahora inyecta un enumerador que devuelve error y comprueba cero llamadas
al restaurar el recibo directo; el test local pasó en Windows. La distribución 0.5.2 final incluye
este refuerzo, con ZIP, setup y hashes verificados. Cargo release tardó 26m28s, Inno Setup 111,860 s
y el pico del Job fue 10,41 GiB. Solo falta el run remoto del workflow en Windows Server 2025.
Un test adicional para recibos reconstruidos verifica que, si el enumerador no está disponible, el
respaldo se intenta una vez y se comunica el fallo sin panic. El gate de verificación del 15 de
octubre pasó suite workspace y Clippy global (`-D warnings`), con 56 tests platform aprobados,
seis ignorados y pico 5,89 GiB; el log está en `target/agent-out/guarded-20261015-001236-148.out.log`.
La consulta remota del 15 de octubre encontró además cinco fallos del workflow experimental
`Preview implementation review` del 9 de septiembre en otra rama, sin PR: el último se detuvo por
divergencia de contenido en `docs/PREVIEW_AND_PATHS.md`, antes de compilar o probar. Se registró en
W-12 como incidencia histórica si se retoma ese aplicador; no corresponde a la CI actual. El run
remoto con el workflow actual sigue pendiente.
El 15 de octubre, por aprobación de Nicolás, CI y release fijaron checkout, rust-toolchain,
rust-cache y upload-artifact a commits verificados por GitHub API; SignPath ya estaba en SHA.
`scripts/check_action_pins.py` quedó en el gate y rechaza tags mutables y acciones Docker sin digest.
Los cinco SHAs exactos de las acciones se confirmaron por GitHub API; ambos YAML y todos los bloques
PowerShell `pwsh` embebidos pasan el análisis local. Falta ejecutar el workflow actualizado en
Windows Server 2025.
Los YAML pasan parseo y las 14 referencias actuales coinciden con sus SHAs. Una nueva consulta
`gh run list` no encontró ejecución para este checkout: la CI remota más reciente sigue siendo
`main`/`abdef402` del 7 de septiembre. No se publicó el checkout ni se disparó una release.
El 15 de octubre también se corrigieron dos casos aprobados: los resultados por lote ahora conservan
recibos de elementos reciclados aunque otro origen falle, y asocian cada error al origen correspondiente;
las rutas se vuelven absolutas sin seguir el reparse point final. El fallback de Papelera se limita a
cada origen que sigue presente y respeta la cancelación. La prueba Windows creó un junction, lo envió a
Papelera, verificó que el destino y su archivo siguieran intactos y restauró el junction; los cinco
tests dirigidos y los checks cruzados Windows de platform/UI pasaron. El gate completo quedó verde:
1.389 tests, seis ignorados, Clippy `-D warnings` y formato. La distribución de este último delta
sigue vigente en 0.5.2: ZIP y setup se regeneraron bajo supervisor con pico 10,41 GiB; hashes,
tamaño y estado Authenticode `NotSigned` están en [la validación del 15 de octubre](VALIDACION-BUILD-2026-10-15.md).
El test de ida/vuelta ya no está ignorado, confirma que Shell entregó el ítem y pasó en el gate local
completo (1.381 tests, 6 ignorados; Clippy limpio). El portable y el instalador se regeneraron bajo
supervisor y verificaron; detalle y hashes en [la validación](VALIDACION-BUILD-2026-10-14.md). W-12
sigue pendiente del run remoto con este cambio adicional. El gate local supervisado del 14 de
octubre pasó suite completa y Clippy (`-D warnings`), con pico 5,93 GiB. La distribución 0.5.2 se
regeneró con el caché FIFO: Cargo release tardó 31m11s, Inno Setup 114,485 s y el pico Job fue
10,41 GiB. ZIP, setup, EXE incluido y `SHA256SUMS.txt` se verificaron; detalle y hashes en
[la bitácora](VALIDACION-BUILD-2026-10-14.md). W-12 aún espera la validación remota.
Faltan mediciones reales de UI, memoria/CPU y red. Falta aceptación instalada en un equipo externo antes
de considerar la entrega validada.
Refuerzo de Papelera del 15 de octubre: el restore y el fallback inspeccionan la entrada con
`symlink_metadata`. Sólo `NotFound` significa ausencia; `PermissionDenied` y otros errores se
propagan para impedir recibos falsos o fallback ante estado incierto. Los tests confirman que un
junction colgante cuenta como ocupado, que restore lo rechaza antes de enumerar y que los errores de
permisos no se tratan como ausencia. Suite supervisada: 1.396 tests, 7 ignorados, Clippy y formato
limpios; las 8 pruebas de Papelera y el check cruzado Linux de platform también pasaron. ZIP/setup
regenerados bajo supervisor (pico gate 8,39 GiB; release 10,41 GiB); el EXE del ZIP coincide con el
release y hashes/tamaños actuales están en la validación del build. Authenticode sigue `NotSigned`;
no se instaló el artefacto y la aceptación W-07/W-13 continúa pendiente.
W-14 prepara firma Authenticode del ejecutable que entra al portable/setup y del instalador final,
con verificación y hashes posteriores. El empaquetador aborta ante firmas presentes inválidas;
pruebas de estados `NotSigned`, `Valid` y `HashMismatch` pasaron. Nicolás eligió explorar SignPath
Foundation; el proyecto no se ha solicitado/aprobado ni hay credenciales configuradas, por lo que
0.5.2 sigue sin firma. Los artefactos SignPath restringen `ProductName=Naygo` y `ProductVersion`
contra la versión del build. Se preparó una política de firma con responsabilidades y privacidad,
enlazada desde README y desde las notas de futuras releases con el rótulo “Code signing policy”. La
release pública `v0.3.0` aún no tiene el enlace; una actualización de esa página requiere una decisión
explícita y la política local aún no se publica desde el repositorio remoto. El tag oficial `v3` de
`signpath/github-action-submit-signing-request` resuelve al commit
`f6d04783b4569d051e0c80105fe66e82819d0092` (verificado por GitHub API el 15 de octubre); el workflow
ya está fijado a ese SHA inmutable para ambas solicitudes.
Inno Setup guarda espacios finales en campos de texto del recurso PE; la validación local los recorta
para comparar `Naygo`/`0.5.2`. La primera solicitud SignPath debe confirmar que su comprobación de
metadata normaliza ese padding; si lo rechaza, W-14 requiere resolverlo antes de firmar.
La distribución local se regeneró el 14 de octubre tras el ajuste de identidad de rutas y la limpieza
de warnings cruzados Linux; gate, Clippy, formato, diff y Graphify pasaron. Sus hashes/validaciones están en
[VALIDACION-BUILD-2026-10-14.md](VALIDACION-BUILD-2026-10-14.md).
Este documento distingue
producto disponible, trabajo aprobado y evidencia histórica. No convierte un plan ni un
resultado de CI en una entrega instalada.

L-00 sigue abierto. La evaluación GTK/AppIndicator de la bandeja fue superada por la decisión de
Nicolás del 15 de octubre de migrar a GLib antes de la alfa; autorizó GPL-3 sólo para Linux si el
enlace lo requiere y Windows conserva MIT. El catálogo oficial ofrece `libayatana-appindicator-glib2`
en Ubuntu 26.04/Resolute, pero no en Noble 24.04; Nicolás fijó Ubuntu 26.04 LTS x86_64 como mínimo
de la alfa y no se hará backport a Noble en esta fase. El binding GLib compatible, degradación segura,
entorno nativo, matriz de derivados/escritorios, revisión de licencias, fuente correspondiente y
notices siguen pendientes. La evaluación histórica del paquete GTK Ayatana y su copyright consta en
el [plan Linux](plans/2026-10-13-linux-ubuntu-luna.md).
Inventario documental L-00: el runtime Resolute amd64 es `libayatana-appindicator-glib2` 2.0.1-1build1
de Universe, depende directamente de libc y GLib >= 2.80 y entrega el soname `.so.2`; la fuente
exacta confirma GPL-3 para la biblioteca C. El desarrollo publica header, `.pc` y enlace `.so`.
El source tar quedó hash-verificado contra su `.dsc` en `target/agent-out/linux-glib-l00/`; faltan
revisar el `copyright` del binario y generar notices finales. Detalle y URLs en el plan Linux.
Evidencia enlazada en el [plan Linux](plans/2026-10-13-linux-ubuntu-luna.md).
`cargo check --tests` y
Clippy `-D warnings` cruzados de core/platform para `x86_64-unknown-linux-gnu` pasan sin avisos;
se limpiaron warnings de imports/helpers Windows-only y drops sin destructor en stubs Linux.
`cargo test -p naygo-platform --lib` pasó 54 tests (6 ignorados). Revisión WSL del 16 de octubre:
Ubuntu está registrada en WSL 2, pero el inicio falla con `HCS_E_HYPERV_NOT_INSTALLED`;
`HypervisorPresent=False` aunque el CPU reporta virtualización firmware activa. La lectura de
features opcionales exige elevación, así que la feature faltante no se confirmó. Tampoco hay
Docker/Podman/QEMU/VirtualBox o sysroot/pkg-config Linux para comprobar Slint. No se modificó la
configuración del equipo ni se instaló software; los checks cruzados no certifican build nativo.
Nicolás autorizó una excepción de licencia exclusivamente Linux para GTK/AppIndicator con enlace
dinámico y notices; no altera la política de Windows ni otras dependencias. El `.deb` deberá llevar
los avisos de su cierre real de dependencias.
Subpasos de L-01A implementados: el parser POSIX conserva espacios extremos, comillas y backslash;
las claves de rename/duplicate/undo, sync, task spaces, búsquedas guardadas, retorno a selección,
cesta, planificación de entregas y radar de destinos comparan case/bytes nativos en Unix. Entregas
valida el solapamiento por componentes. Renombrado por lotes muestra nombres no UTF-8 con bytes
escapados y bloquea su aplicación; los nombres UTF-8 válidos conservan backslash POSIX. Windows
conserva el criterio anterior. Copy/move ya distinguen case y componentes nativos en Unix al detectar
destinos anidados y no-op; un test permite planificar copy entre carpetas `A` y `a`. Pasaron 19 tests
dirigidos Windows y el check cruzado con tests para Linux; su ejecución nativa sigue pendiente. Esto
no habilita UI ni operaciones Linux ni resuelve transporte general de bytes de ruta.
El 15 de octubre se corrigió además la validación POSIX de nombres (backslash, espacios/puntos finales
y signos permitidos) y la generación de nombres de conflicto preserva bytes no UTF-8. Pasaron 11 tests
nativos Windows y el check cruzado de `naygo-core` con tests Linux; los tests Unix requieren ejecución
nativa, todavía no disponible. El planificador de Duplicate ahora genera ` - copia` con `OsStr`/
`OsString`, conservando nombres no UTF-8 y resolviendo colisiones; pasó su prueba nativa Windows y
`cargo check` y Clippy cruzados de core para Linux con los tests Unix incluidos.

La compresión ZIP deriva nombres internos por componentes nativos y une componentes con `/`;
conserva backslash literal de POSIX. Si un componente no puede representarse como UTF-8, devuelve
error durante el inventario y deja intacto el destino, evitando sustituciones o colisiones
silenciosas. Dos tests Unix cubren backslash y byte inválido; el check Linux cruzado compila ambos,
pero falta ejecutarlos en un entorno Linux nativo. La corrección no habilita por sí sola operaciones
ni UI Linux.

El guardado de plantillas de layout ya mantiene las rutas fijas como `PathBuf` durante la captura.
Si alguna ruta no es UTF-8, el controlador rechaza el guardado antes de modificar el catálogo y la
UI informa el motivo en los diez idiomas; rutas UTF-8 mantienen el formato JSON string existente.
Pasaron la prueba UI de plantillas, 110 tests de workspace, i18n y check/Clippy cruzados de core
para Linux. El test Unix compila por check, pero el enlace de su binario de test cross falla porque
falta `cc`; necesita ejecución Linux nativa. El gate completo pasó (1.393 tests, 7 ignorados, Clippy
y formato) y se regeneró la distribución Windows bajo supervisor; hashes en la validación 2026-10-15.

El parser de inicio usa ahora `args_os()` y conserva la ruta posicional no UTF-8 en `OsString`/
`PathBuf`; así un argumento POSIX válido ya no provoca panic antes de abrir la app. Sólo la
representación para avisos escapa bytes no válidos. Los flags y valores de tema/plantilla mantienen
el contrato UTF-8 existente. El check/test Windows pasa; los tests POSIX compilan cruzados, pero
la ejecución requiere un host Linux nativo. El gate completo pasó (1.393 tests, 7 ignorados, Clippy y
formato); después del ajuste de visualización pasaron los tests CLI focales, Clippy Linux cruzado y
check UI. Se regeneró el release Windows con pico 10,42 GiB; hashes y limitaciones en
[la validación 2026-10-15](VALIDACION-BUILD-2026-10-15.md).

**Nota de planificación, 2026-10-13:** Nicolás solicitó [un plan de mejoras para GPT-6 Luna](plans/2026-10-13-mejoras-y-ejecucion-luna.md)
y [un plan Linux](plans/2026-10-13-linux-ubuntu-luna.md), con Ubuntu LTS y derivados x86_64
como prioridad elegida.

W-01 clasifica la salida de listados como completa, cancelada, fallida o parcial. Los resultados
parciales y los canales desconectados no actualizan snapshots ni recientes; se muestra un aviso
y se conserva el snapshot válido anterior. El probe sólo diagnostica ausencia ante `NotFound` /
`NotADirectory`. Tests dirigidos del core/UI y los diez catálogos pasaron. W-02 usa un conjunto de
rutas para resaltar cambios y lo invalida al navegar; sus tests dirigidos y benchmark sintético
pasaron. W-03 reemplaza `Vec<Entry>` por metadatos compactos y LRU. El límite de 32 MiB estimados /
100.000 entradas es provisional; falta medición W-07. Sus cuatro pruebas core pasaron. La
distribución 0.5.2 se regeneró y sus hashes se verificaron; detalles en
[el registro del build](VALIDACION-BUILD-2026-10-13.md).
W-04 eliminó consultas al filesystem desde UI al sugerir nombres y detectar colisiones de Copy/Move.
La preconsulta del drop corre en un worker acotado, conserva errores de acceso y deja el staging vivo
hasta decidir entre el diálogo genérico o el de conflicto; el motor revalida antes de actuar. Los
354 tests UI pasaron y la distribución 0.5.2 se regeneró con este bloque.

Nicolás aprobó añadir a W-06B tres hallazgos de la auditoría: colisiones síncronas al pegar archivos,
`read_dir`/búsqueda de ancestro desde los controles de carpeta perdida y la revisión de la
persistencia incremental de workspace que antes escribía desde tick. Este último ya se movió a un
worker en W-05. El autocompletado de ruta y la importación `.naygolist` ya son asíncronos.

W-05 mueve settings, keymap y sesión a un worker JSON coalescido, y coordina el cierre fuera de UI
con plazo de 3 s. Tres pruebas dirigidas del writer y los 354 tests UI pasaron; la distribución
actualizada está generada y sus checksums verificados.

W-06A/B mueve a workers acotados la carga de PNG externos y catálogos de temas/sets al iniciar,
packs, copia de PNG de usuario, guardado/borrado de temas, logging ordinario, colisión de pegado y
consultas de carpeta perdida. Durante la carga inicial se usan assets embebidos; el catálogo del
usuario se aplica cuando el worker termina. Verificación actual: 359 tests UI, 891 core y 22 flujos
de operaciones aprobados. El build supervisado está registrado en
[la validación](VALIDACION-BUILD-2026-10-13.md).

W-06C mueve a un worker la lectura inicial de settings, idioma, keymap, sesión, favoritos y
plantillas; el primer frame usa defaults/recursos embebidos. La aplicación del snapshot conserva
ediciones de preferencias, atajos, favoritos/plantillas y navegación ocurridas mientras carga.
También difiere cierre y escrituras de favoritos/plantillas; ante fallo del worker no guarda los
defaults sobre archivos existentes. Hallazgo adicional: la escritura atómica no creaba el directorio
padre, lo que impedía guardar favoritos/plantillas en una configuración nueva; ahora lo crea en el
worker. Suites completas: 365 UI, 893 core, 22 flujos de operaciones y 54 platform (6 ignorados).
`cargo check`, formato y diff check pasan. La distribución supervisada de W-06C está en
[la validación](VALIDACION-BUILD-2026-10-13.md). Clippy encontró 16 ocurrencias agrupadas en siete
clases y Nicolás aprobó registrarlas como mantenimiento M-01.

W-07 capturó tres ejecuciones de benchmark sintético y agregó métricas opt-in de tick, sincronización
Slint, listado y fases de preview. Suite UI aprobada tras instrumentar; la distribución supervisada
está generada y sus hashes se verificaron. No se han medido CPU en reposo, memoria principal/
auxiliares, latencia de interacción ni UNC. Ver [validación](VALIDACION-BUILD-2026-10-13.md).
M-01 limpió las 16 ocurrencias Clippy en siete clases. El gate global con `-D warnings`, suites
completas y nueva distribución supervisada pasaron; evidencia en el plan y validación de build.

## Entrega actual — 0.5.2, RP-4B2 y compilación de menor consumo

Nicolás autorizó auditar temporales/dependencias y ajustar la compilación. Se identificó
caché regenerable (144,61 GiB, no eliminada por restricción de herramienta) y se validó
ThinLTO/16 unidades bajo el mismo supervisor: [registro](VALIDACION-BUILD-2026-09-14.md).

Texto/resaltado y STL/3MF aislados; protocolo de geometría y temporales de raster acotados.
Se conservan giro 3D sin I/O, reglas de lenguaje y copia parcial. **1.324 pruebas y Clippy
aprobados**, siete pruebas adicionales del EXE release. Instalador/portable
**0.5.2+build.202609142135**, generados a las **22:04** y verificados el 14 de septiembre.
Build completo con pico contable 10,44 GiB (límite 12 GiB). ZIP/entradas/checksums/autoría
verificados; PDB opcional en instalador, fuera del ZIP. Sin instalar ni publicar a GitHub.
[Registro RP-4B2](VALIDACION-RP4B2-2026-09-14.md).

Recuperación tras reinicio forzado: fuentes y distribución anterior intactas; Git sin
corrupción, **1.324 pruebas aprobadas**, seis ignoradas preexistentes y Clippy limpio.
Los intentos iniciales con fat LTO se detuvieron a 12/16 GiB antes de empaquetar.
La continuación autorizada resolvió el build con el nuevo perfil, sin aumentar el límite.
RP-5B quedó respaldado. No se determinó la causa del reinicio original.
Siguiente: aceptación instalada y RP-6; no afirmar equivalencia global de rendimiento/RSS.

## Entrega anterior — 0.5.2, RP-5B

Modo económico persistente, imagen Slint compartida y presupuesto de retención
imagen/texto/escenas implementados. **1.317 pruebas aprobadas**, seis ignoradas preexistentes,
Clippy sin advertencias, diez idiomas/formato/diff correctos y grafo actualizado.
Instalador/portable **0.5.2+build.202609100757** generados y verificados a las **08:26**
del 10 de septiembre. Cinco pruebas adicionales del lector release aprobadas.
La entrega anterior está respaldada; sin instalación sobre la sesión personal ni GitHub.
[Evidencia y límites RP-5B](VALIDACION-RP5B-2026-09-10.md).

Continuación de aquel corte: I/O bloqueado de texto/3D y presupuestos temporales;
implementados ahora en RP-4B2, cuya validación figura al inicio de este documento.
RP-6 y aceptación instalada continúan pendientes. La calidad económica ya está disponible;
el presupuesto de retención no equivale a un límite de RSS global.

## Entrega anterior — 0.5.2, navegación y refresco

Correcciones solicitadas de selección al subir/crear, árbol, modal de carpetas, nombres de
pegado con segundos y resaltado sin espacios. Reconciliación del refresco por operaciones y
watchers implementada. **1.308 pruebas aprobadas**, seis ignoradas preexistentes, Clippy
sin advertencias, formato/diff/diez idiomas correctos. Captura Slint inspeccionada y prueba
del modal aprobada. Instalador/portable **0.5.2+build.202609100054** generados y verificados
el 2026-09-10 a las 01:19 local; cuatro pruebas adicionales del lector release aprobadas.
RP-5A respaldado; sin instalar sobre la sesión personal ni publicar a GitHub.
Siguiente: aceptación de estos gestos en la instalación del usuario; los presupuestos globales,
modo económico y demás pendientes RP-4/RP-5/RP-6 conservan su alcance anterior.
[Registro 0.5.2](VALIDACION-0.5.2.md).

## Entrega anterior — RP-5A

Animación WebP/GIF/APNG con pausa/reanudación/repetición, decodificación aislada secuencial
y reloj sólo de preview implementados. **1.301 pruebas aprobadas**, seis ignoradas preexistentes,
Clippy limpio, diez idiomas/formato/diff correctos. Instalador/portable
**0.5.1+build.202609092333** generados y verificados (2026-09-10 00:04 local), RP-4B1 respaldado.
Cuatro pruebas del lector release aprobadas: la muestra privada recorrió 55 frames en 1.942 ms
de decodificación/IPC sin esperas de presentación; cancelación en 30 ms. No mide reproducción UI.
Sin instalar sobre la sesión personal ni publicar a GitHub. Siguiente: presupuestos globales,
calidad económica configurable y aceptación instalada; RP-4/RP-5/RP-6 no están cerrados completos.
[Registro RP-5A](VALIDACION-RP5A-2026-09-09.md).

## Entrega anterior — RP-4B1

Lectores de imagen/SVG/PDF/comprimidos aislados en el mismo EXE, Job Windows y protocolo
acotado implementados. **1.287 pruebas aprobadas, seis ignoradas preexistentes, Clippy
sin advertencias**, idiomas/formato/diff correctos. Instalador/portable
**0.5.1+build.202609092205** generados y verificados a las 22:35 local; distribución RP-4A
respaldada. Prueba adicional del ejecutable release aprobada con PNG/SVG/ZIP/PDF y la muestra
WebP de Nicolás, sólo en lectura. No se instaló ni se publicó a GitHub.
WebP confirmado animado (50 frames, 800 × 450, ciclo 5 s), pero **en ese build el preview seguía estático**.
RP-4 continúa parcial: I/O bloqueado de texto/3D y presupuesto global pendientes;
RP-5 animación y RP-6 lectores/métricas también. [Registro RP-4B1](VALIDACION-RP4B1-2026-09-09.md).

## Entrega anterior — RP-4A

Cancelación inmediata y reintento explícito, plazas de worker retenidas hasta su salida,
coalescencia de cámara 3D y suspensión por visibilidad implementados. **1.274 pruebas
aprobadas, seis ignoradas preexistentes, Clippy limpio, idiomas/formato/diff correctos**.
Controles Slint renderizados y probados con datos sintéticos. Instalador/portable
**0.5.1+build.202609092050** generados y verificados (21:18 local), entrega anterior
respaldada. Sin instalar sobre la sesión activa ni publicar a GitHub.
[Validación RP-4A](VALIDACION-RP4A-2026-09-09.md). Siguiente: RP-4B, aislamiento/terminación
no cooperativa y presupuestos completos. RP-4 no está cerrado; aceptación nativa pendiente.

## Entrega anterior — RP-3

Integradas seis paletas originales, color independiente de énfasis y catálogo único de
íconos. Se conservan preferencias, nueve temas anteriores y esquema 4. **1.263 pruebas
aprobadas, seis ignoradas preexistentes, Clippy limpio y diez idiomas correctos**.
Quince temas renderizados e inspeccionados con componentes Slint reales y datos sintéticos.
Instalador/portable **0.5.1+build.202609091919** generados y verificados (19:44 local).
Entrega anterior respaldada; sin instalación sobre la sesión personal ni publicación remota.
[Registro RP-3](VALIDACION-RP3-2026-09-09.md). Entrega anterior a RP-4A.

## Entrega anterior — RP-2

Ruta superior implementada sobre RP-1: clic/lápiz abre el editor compartido expandido,
con panel destino capturado, acciones sobre ruta confirmada y retorno de foco al cancelar.
Suite aprobada: 1.257 pruebas y seis ignoradas preexistentes; Clippy limpio.
Instalador y portable **0.5.1+build.202609091819** generados y verificados (18:48 local).
Distribución anterior respaldada; aceptación instalada/DPI/AltGr/red pendiente.
[Registro de este bloque](VALIDACION-RP2-2026-09-09.md).

## Entrega anterior — RP-1

Edición de rutas de panel implementada sobre 0.5.1, en la rama `codex/rutas-preview-051`:
lápiz, cursor/selección/undo nativos, validación asíncrona con error inline, consultas acotadas
y destino capturado. Configuración y dependencias conservadas. **1.255 pruebas aprobadas,
seis ignoradas preexistentes, Clippy sin advertencias**. Instalador y portable regenerados
como **0.5.1+build.202609091710**, con integridad y hashes comprobados;
ver [registro de entrega](VALIDACION-2026-09-09.md).

Fuentes experimentales recuperadas de GitHub en una carpeta separada, con ZIP/hashes
verificados; no se aplicaron en bloque. RP-3/RP-4A entregados; RP-4B–RP-6 siguen pendientes. La documentación que
sigue identifica la base previa y su evidencia para no perder trazabilidad.

## Base conservada (antes de RP-1)

- Código de aplicación: **0.5.1**, revisión base `abdef402babd2c136744c67ee9833c2f67bf0f16`.
- Distribución local: **0.5.1+build.202609071600**, generada el 7 de septiembre.
- Arquitectura: Rust, tres crates, Slint/winit con renderer por software; sin sustitución
  de UI ni incorporación de reproductor multimedia general.
- La revisión inicial local del 9 de septiembre no encontró cambios en las fuentes respecto de
  esa base. El único archivo nuevo al iniciar la revisión era el Markdown de traspaso.
- La primera consolidación sólo modificó documentación. RP-1 es la siguiente entrega
  funcional y regenera la distribución con esos documentos, sin cambiar versión ni esquema.

Se conservan navegación/docking, 7-Zip/OLE, Papelera, selección/scroll de texto, SVG,
escala de imagen, fuentes por función, bandeja, refresco local/global y los flujos de
entregas, espacios, consultas, puntos de comparación y recetas. Conservación del código
no significa que todos los escenarios de escritorio/red hayan recibido aceptación manual.

## Evidencia de la auditoría inicial del 9 de septiembre (antes de RP-1)

| Comprobación | Resultado | Registro local en `target/agent-out/` |
| --- | --- | --- |
| `cargo test --workspace --no-fail-fast --locked` | 1.246 aprobados; 0 fallidos; 6 ignorados | `local-audit-20260909-tests.log` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Sin advertencias, salida 0 | `local-audit-20260909-clippy.log` |
| Formato y diff | Aprobados | Salida de la revisión local |
| Catálogos ES/EN/PT/DE/FR/HI/IT/JA/KO/ZH | Claves y placeholders correctos | `scripts/check_i18n_lang.py` |
| ZIP portable | Integridad aprobada por `7z t` | Salida de la revisión local |
| SHA-256 del setup, ZIP y EXE | Coinciden con la entrega del día 7 | [Registro de distribución](VALIDACION-2026-09-07.md) |

Desglose: 865 core, 22 integración core, 47 platform, 309 UI y 3 integración visual.
Los seis tests ignorados no se ejecutaron en esta revisión. El smoke nativo del watcher
sí se había ejecutado el día 7; no presentar ese resultado anterior como una ejecución nueva.
La paridad de idiomas no certifica traducción editorial: quedan claves con respaldo inglés.
Los logs y la distribución son archivos locales ignorados por Git, no adjuntos publicados.

## Pendientes reales

1. [Rutas, configuración y preview responsable](plans/2026-09-09-rutas-preview-y-configuracion.md):
   RP-0–RP-3, RP-4A/RP-4B1 y RP-5A/RP-5B implementados/validados automáticamente.
   RP-4B2 implementa I/O aislado texto/3D y temporales acotados; entrega 202609142135 verificada.
   Quedan RP-6, medición global y aceptación instalada.
   RP-7 se aplica por entrega: paquetes hasta RP-4B2 verificados; aceptación nativa pendiente.
2. [Plan de usabilidad y flujos](plans/2026-09-04-usabilidad-y-flujos-de-trabajo.md):
   conserva lo implementado; faltan aceptación instalada/DPI/Narrador/red/permisos,
   mediciones, restauración por sección y trabajo editorial indicado en el backlog.
3. Firma digital del instalador: preparación disponible; falta certificado autorizado.

No rehacer flujos ya implementados porque un registro antiguo diga «working tree» o
«sin commit». [BACKLOG.md](BACKLOG.md) conserva la cronología con su contexto temporal.

## Límites de la base y avance

- La ruta superior ya se edita por clic/lápiz (RP-2). RP-1 conserva el lápiz del panel,
  `… → Editar ruta` y Ctrl+L/F4 con el mismo editor; una única sesión en una ubicación.
- RP-1 retira `complete_path`/`is_dir` síncronos de los callbacks de rutas y conserva
  el borrador ante errores. La llamada bloqueada del SO no se mata: mantiene una plaza
  hasta terminar, con timeout lógico para dejar libre la UI.
- RP-1 elimina la concatenación especial de `\`: la edición nativa respeta caret/selección.
- Desde RP-5A, el preview presenta WebP, GIF y APNG animados de forma secuencial, con pausa,
  reanudación y repetición. La decodificación y el reloj tienen límites y ciclo de vida propios;
  consultar [RP-5A](VALIDACION-RP5A-2026-09-09.md) y [la arquitectura](ARQUITECTURA.md).
- RP-4A ofrece Cancelar desde el debounce y sobre miniaturas con trabajo pendiente;
  un token no interrumpe una llamada bloqueante del SO o un decoder que no lo consulta.
- Los límites de salida visible no garantizan un presupuesto total de memoria/CPU;
  PDF se carga para metadatos y se vuelve a leer para extraer texto.
- RP-3 conserva un catálogo de sets en Íconos y un enlace desde Apariencia. Las seis
  paletas recuperadas amplían el catálogo a quince; `icon_accent` conserva compatibilidad
  con temas antiguos y separa el énfasis del fondo de filas recientes.

Punteros y contratos: [Arquitectura](ARQUITECTURA.md). Son hallazgos de inspección, no
una nueva reproducción interactiva de cada problema.

## Continuar sin perder preferencias ni trabajo

1. Leer este estado, `BACKLOG.md` y el bloque concreto del plan. Inventariar cambios
   locales antes de editar; no hacer reset, sobrescritura masiva ni merge de código sin validar.
2. Usar el paquete ya recuperado sólo como referencia. Su ubicación, hashes y fallos
   conocidos están en el plan; no ejecutar aplicadores opacos ni aplicar el árbol completo.
3. Trabajar en una rama de continuación y una copia de prueba con configuración propia.
   Respaldar preferencias y datos de prueba; no usar originales personales en pruebas destructivas.
4. Preservar la configuración de esquema 4: un ejecutable experimental que escriba esquema 5
   no debe compartirla con 0.5.1. La versión anterior usa defaults si recibe una versión futura.
5. Cerrar cada bloque con código comentado, pruebas, documentación y distribución si cambia
   la app. Registrar aparte compilación, tests, revisión visual y generación del instalador.
6. La publicación o ejecución de pruebas sobre la sesión instalada requiere el alcance que
   indique Nicolás; no cerrar su instancia ni modificar preferencias en nombre de la validación.

El Markdown temporal fue consolidado por petición de Nicolás. Su original se conserva en
`target/agent-out/traspaso-original-20260909.zip` (respaldo local, no versionado). Los requisitos,
identificadores de recuperación y advertencias útiles permanecen en el plan versionable.

L-01B avanzó parcialmente en Windows: las copias usan staging en un temporal del directorio de destino y publicación al final. Cancelar/fallar conserva el archivo anterior; los movimientos cross-volume borran el origen sólo tras publicar. Pasaron 916 tests core, 22 flujos de operaciones, gate completo de 1.408 pruebas (8 ignoradas), Clippy, formato y check cruzado Linux del core. La distribución de esa etapa `0.5.2` se regeneró bajo supervisor con pico 10,45 GiB; ZIP SHA-256 `46687762…`, setup `106e34c3…`, EXE `c9db8752…`. Se instaló con autorización previa y se dejó abierta/respondiendo; tras el smoke aislado y la restauración exacta de los cuatro JSON, el proceso de esa actualización fue PID 9204. El PDB antiguo quedó intacto. En la regeneración posterior descrita al inicio de este documento se cerró la carrera de overwrite de la ruta rápida Windows. El reemplazo de árboles ya tiene staging y rollback local; el gate y la distribución regenerada se registran al inicio de este documento. L-01B permanece parcial por aceptación manual, recuperación ante pérdida de energía, pruebas de disco/permisos, movimiento entre volúmenes y ejecución Linux nativa; la recuperación tras terminación de proceso se documenta en el avance posterior. W-07 sigue pendiente: el fixture aislado produjo métricas internas, pero la captura de ventana agotó tiempo y no se enviaron entradas. Detalle de la nueva evidencia en `docs/VALIDACION-BUILD-2026-10-16.md` y el plan Linux.
