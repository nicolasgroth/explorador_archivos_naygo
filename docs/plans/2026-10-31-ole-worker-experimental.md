# Receptor OLE en STA separado: experimento

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
SPDX-License-Identifier: MIT

Este borrador no está listo para distribución. Registra el receptor de drop en un hilo STA,
captura árboles temporales de 7-Zip antes de volver de `Drop` y materializa en un worker.
La captura rechaza reparse points y limita la cantidad de entradas; el staging se limpia con
su guard al fallar o al soltar la última referencia. Las pruebas sintéticas no validan el ciclo
de vida de Slint/Winit ni sustituyen una prueba de arrastre real con el receptor integrado.

El 2026-11-01 Nicolás arrastró la carpeta sintética `E2 fixture` desde 7-Zip a la ventana
Slint/Winit del binario experimental y confirmó la copia. El destino se comparó con el árbol
original: 259 archivos con SHA-256 coincidente y cuatro subcarpetas, incluida una vacía; sin
entradas faltantes ni adicionales. Esta prueba cubre un árbol local concreto, no las demás
fuentes ni los casos de cancelación y conflictos enumerados abajo.

Después se adaptó el contrato OLE para no comunicar MOVE al origen antes de la confirmación.
Las pruebas focalizadas de plataforma pasaron: 25 aprobadas y una medición ignorada, incluyendo
Shift, fuentes que solo permiten copiar o mover y receptor cerrado. La prueba real anterior se
ejecutó antes de esta adaptación y debe repetirse en el binario actualizado; tampoco sustituye
la prueba real de cancelación de un movimiento desde Explorer. Clippy de plataforma para todos
los targets también pasó sin avisos tras este ajuste. El gate completo de esta rama pasó con
1.544 pruebas aprobadas, nueve ignoradas, Clippy y formato correctos. La validación con fuentes
reales posteriores al cambio de efecto OLE sigue pendiente.

La preparación se expone en Operaciones con un token cancelable; las pruebas sintéticas cubren
cancelación antes y durante `GetData` y el retiro de la operación transitoria. Si el proveedor
no retorna, la cancelación no garantiza interrumpir su llamada COM: se completa al regresar.
El cierre del registro usa un evento del sistema compartido y no un identificador de hilo que
podría reutilizarse. En la pasada local anterior se aprobaron 1.538 pruebas y Clippy; tras ese
cambio adicional pasaron 20 pruebas focalizadas del receptor y Clippy de plataforma.
Si el worker termina sin recibir la señal de cierre, marca el guard como fallido y despierta a la
UI para permitir registrar nuevamente el receptor; las 20 pruebas focalizadas y Clippy pasan.
Una prueba Win32 con ventana oculta comprobó también que el worker instala el target y lo revoca
al cerrar el guard. No simula un arrastre ni reemplaza la prueba real con Slint y 7-Zip.

Medición exploratoria local en debug, una ejecución con archivos sintéticos de un byte:
1.000 archivos, captura 363,90 ms y staging 866,70 ms; 10.000 archivos, captura 3.682,05 ms y
staging 7.633,05 ms. La prueba total tardó 288,02 s e incluye creación y limpieza de ambos árboles,
sin medición separada de esas fases. No incluye extracción de 7-Zip, UI ni unidades de red; no es
un benchmark comparativo. La limpieza de staging productivo ocurre en un worker.

Pendientes obligatorios: probar detención y cierre con proveedores lentos, arrastre desde Explorer, arrastre
interno, archivos virtuales, selecciones mixtas, colisiones y árboles grandes. Revisar el registro
asíncrono y sus reintentos; validar los límites en máquinas modestas; medir UI y memoria. No
mezclar ni publicar esta rama hasta completar esos controles y el build supervisado.

Hallazgo posterior: el payload final se enruta por coordenadas al terminar la preparación, no al
soltar. Si el usuario cambia distribución o carpeta durante el trabajo, puede apuntar a otro panel;
el fallback al panel activo aumenta el riesgo. Abrir Operaciones antes de resolver el destino
cambiaría también la geometría. El ajuste provisional de apertura se retiró; fijar y validar el
destino original antes de cambiar el layout, sin fallback inseguro, y hacer cancelable y visible la
preparación. La prueba integrada anterior se hizo sin cambiar la disposición.
# Nota experimental 2026-11-03: vida de objetos virtuales

La materialización virtual se efectúa durante `Drop` en el STA OLE dedicado para que el
proveedor permanezca activo hasta terminar, sin depender de `IDataObjectAsyncCapability`.
El origen espera durante la extracción; la cancelación se comprueba entre archivos y lecturas,
pero una llamada COM bloqueante solo termina cuando el proveedor retorna. El staging se limpia
al fallar o cancelar. Las 27 pruebas focalizadas de plataforma pasaron (una ignorada) y Clippy
de plataforma pasó en target aislado. Un IDataObject sintético sin soporte asíncrono entrega un
archivo cuyo origen se invalida justo después de Drop; la prueba comprueba el contenido preparado.
Otra prueba bloquea GetData, cancela y comprueba que solo se complete la cancelación tras el retorno
del proveedor, sin entregar rutas ni notificar copia. El conjunto ampliado tiene 28 pruebas aprobadas
y una ignorada; Clippy de plataforma pasó.
Esto no prueba aún con un proveedor virtual real, ni resuelve el destino asíncrono o la visibilidad
de la cancelación en UI. No integrar sin esas validaciones.

El prototipo `drop_snapshot` conserva destinos inmutables por PaneId y coordenadas físicas,
rechaza regiones ambiguas y publicaciones obsoletas. No está conectado a UI ni a Drop: no
demuestra que todos los cambios de viewport, layout, DPI y modelo invaliden a tiempo. Una
alternativa segura cuando no se pueda acreditar el destino es pedir al usuario elegirlo de forma
explícita, sin preselección ni fallback; esto añade interacción y no sustituye determinar el
destino original al soltar.

Prototipo posterior: los drops con staging solicitan elegir explícitamente un panel y confirman
la ruta completa antes de operar; no hay selección inicial. Un aviso FIFO abre Operaciones solo
tras identificar staging, para ofrecer cancelación sin mover paneles para `CF_HDROP` persistente.
Los drops de rutas persistentes conservan su enrutamiento anterior; la ventana entre soltar y
procesar sus coordenadas no queda demostrada segura. El callback `GetData` inicial puede bloquear
antes de que aparezca la preparación, y llamadas COM bloqueantes no se interrumpen. Esta nota no
acredita aceptación visual ni pruebas reales de selección, cierre o error en una app externa.
