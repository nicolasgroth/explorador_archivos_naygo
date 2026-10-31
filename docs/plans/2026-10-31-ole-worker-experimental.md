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

Pendientes obligatorios: probar detención y cierre con proveedores lentos, arrastre desde Explorer, arrastre
interno, archivos virtuales, selecciones mixtas, colisiones y árboles grandes. Revisar el registro
asíncrono y sus reintentos; validar los límites en máquinas modestas; medir UI y memoria. No
mezclar ni publicar esta rama hasta completar esos controles y el build supervisado.
