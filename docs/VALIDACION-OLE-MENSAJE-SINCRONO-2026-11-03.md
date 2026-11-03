# Exploración de mensajes síncronos para fijar destino OLE

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

El receptor OLE en STA separado necesita conocer el destino que ve el usuario antes de abrir
Operaciones o esperar al proveedor. Una posible vía es solicitar un hit-test al hilo dueño de la
ventana mediante un mensaje Win32 con timeout. No está implementada ni aceptada para producción.

Una prueba local con ventana WinForms STA sintética y oculta, no la ventana Slint de Naygo,
produjo `ping=42; reentrancy=rejected; timeout_ms=29; callback_completed_after_timeout=true`.
El callback lento duró 250 ms: el emisor recibió timeout antes de terminar el callback. La prueba
también ejecutó una llamada anidada en el mismo hilo y rechazó mediante un flag de ocupado.
El fixture local está en `target/agent-out/probe-ole-timeout.ps1`, fuera de distribución.

La prueba no valida subclass de winit, selección de fila o panel, cambios de layout, rendimiento,
accesibilidad, ciclo de vida tras cerrar la ventana ni reentrancia de Slint. No basta para afirmar
que el destino capturado coincida con lo visible al usuario.

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
