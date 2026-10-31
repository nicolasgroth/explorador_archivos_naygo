# Receptor OLE en STA separado: experimento

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
SPDX-License-Identifier: MIT

Este borrador no está listo para distribución. Registra el receptor de drop en un hilo STA,
captura árboles temporales de 7-Zip antes de volver de `Drop` y materializa en un worker.
La captura rechaza reparse points y limita la cantidad de entradas; el staging se limpia con
su guard al fallar o al soltar la última referencia. Las pruebas sintéticas no validan el ciclo
de vida de Slint/Winit ni sustituyen una prueba de arrastre real con el receptor integrado.

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
