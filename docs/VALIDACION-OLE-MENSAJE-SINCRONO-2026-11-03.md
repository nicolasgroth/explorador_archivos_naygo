# Exploración de mensajes síncronos para fijar destino OLE

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

El receptor OLE en STA separado necesita conocer el destino que ve el usuario antes de abrir
Operaciones o esperar al proveedor. Una posible vía es solicitar un hit-test al hilo dueño de la
ventana mediante un mensaje Win32 con timeout. No está implementada ni aceptada para producción.

Una prueba local con ventana WinForms STA sintética y oculta, no la ventana Slint de Naygo,
instaló y retiró un subclass en su hilo propietario y produjo `ping=42; reentrancy=rejected;
timeout_ms=29; callback_completed_after_timeout=true; late_response_discarded=true`.
El callback lento duró 250 ms: el emisor recibió timeout antes de terminar el callback. La prueba
también ejecutó una llamada anidada en el mismo hilo y rechazó mediante un flag de ocupado.
El fixture reproducible está en `scripts/probe-drop-ui-rpc.ps1`, fuera de distribución.

La primera prueba no valida subclass de winit, selección de fila o panel, cambios de layout, rendimiento,
accesibilidad, ciclo de vida tras cerrar la ventana ni reentrancia de Slint. No basta para afirmar
que el destino capturado coincida con lo visible al usuario.

Un segundo fixture, `crates/ui-slint/examples/drop_subclass_probe.rs`, usa una ventana Slint con
backend winit/software real. El 2026-11-03 instaló el subclass en el hilo UI, procesó un mensaje
desde otro hilo que modificó una propiedad Slint, detectó reentrada simulada, observó timeout y
callback tardío, y retiró el subclass antes de cerrar. Resultado: `winit_subclass=ok;
slint_property=1; reentrancy=rejected; timeout_and_late_callback=ok; subclass_removed=ok`.
El request sintético usa un ID, registro `Mutex`, estado atómico y `Arc`: al vencer timeout se
marca abandonado y elimina la entrada; el callback conserva su `Arc` y no puede completar la
respuesta tardía. Se comprueba que su última referencia se libere. Es un escenario controlado,
no una auditoría de todas las intercalaciones concurrentes. Un primer intento de obtener el HWND antes del event loop produjo `NotSupported`; el fixture espera
la realización con un timer. La prueba no invoca la app, el `WorkspaceCtrl`, ni hit-test de filas;
no ejercita UI Automation, cierre durante callback ni todas las carreras de memoria tras timeout.

La lectura inmediata del callback `changed drag-client-y` no produjo valores al consultar desde
el mensaje nativo: su evaluación no es síncrona. Un cambio experimental en `FilePanel` extrae
la fórmula a una función pública consultable en el hilo UI y comprueba límites verticales del
viewport: dos filas, espacio vacío y fuera de lista produjeron máscara 15. Al restaurar el scroll
a la fila 30 de un modelo de 40 se detectó hasta la 39. La consulta 2D rechaza puntos fuera
de los límites horizontales del ListView; la ventana de prueba usó factor de escala 1, no prueba
otros DPI. Un intento previo con foco no desplazó
el fixture; no se concluyó un defecto de producto. Falta enlazar la instancia repetida correcta de
AppWindow, validar escalas DPI adicionales, estado de modelo y carpeta, cierre durante el callback y
probar con `WorkspaceCtrl` y cambios de sesión reales. Este experimento no fija el destino OLE.

Condiciones si se explora esta vía: instalar y retirar subclass en el hilo propietario; verificar
identidad de ventana y thread; rechazar reentrancia y `RefCell` ocupado; timeout breve sin llamar
IO ni invocar UI arbitraria; resolver fila y geometría exactas; mantener request/respuesta vivos
después de timeout y descartar resultados tardíos (un puntero a stack sería inseguro). Evitar
alterar los mensajes o accesibilidad de winit/Slint. Probar cambios de carpeta, filas, scroll,
resizing, cierre, timeout, callback posterior y origen colgado con fixtures; no integrar sin ello.

[SendMessageTimeout](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeouta)
indica que puede esperar a otro hilo o vencer, pero ignora timeout en la misma cola.
[SetWindowSubclass](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass)
no cuenta referencias del callback, por lo que el ciclo de vida debe gestionarse explícitamente.
