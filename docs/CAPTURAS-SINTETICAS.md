# Capturas de Naygo con datos sintéticos

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Generar datos de demostración con `./scripts/prepare-screenshot-fixture.ps1`. Se crea un árbol
nuevo bajo `target/agent-out/screenshots`, sin abrir aplicaciones ni acceder a datos personales.

Para imágenes representativas de una versión nueva se necesita una instancia real de ese build
con configuración y sesión aisladas. En Windows la exclusión de instancia de Naygo es por sesión,
no por ubicación del ejecutable: iniciar otra copia portable mientras el usuario ya tiene Naygo
abierto puede activar la instancia existente o navegarla. No hacerlo para obtener capturas sin
coordinar el cierre de la instancia o disponer de una sesión Windows aislada de prueba.

En la instancia aislada, usar los directorios sintéticos como paneles y desactivar o evitar
árboles, favoritos, historial, unidades y rutas reales. Antes de guardar una imagen, inspeccionar
la ventana completa, títulos, barra de ruta, pies, propiedades, menús y fondo para comprobar que no
hay nombres, rutas, correo, datos del equipo ni otras ventanas privadas. Si aparecen, descartar la
captura y corregir el entorno. No presentar capturas editadas como imágenes originales de la app.
Registrar versión, tema, funciones visibles y fecha; mantener en el backlog cualquier imagen nueva
que aún no tenga inspección visual. El fixture solo prepara datos: no captura ni valida la interfaz.

## Capturas actualizadas 2026-11-08

- `assets/screenshots/naygo-multipanel.png`: Naygo 0.5.4, tema `dark-blue`, dos paneles de
  archivos y Propiedades; barra compacta, 1020×780 px.
- `assets/screenshots/naygo-minimal.png`: Naygo 0.5.4, tema `paper`, disposición Minimalista;
  1020×780 px.

Ambas imágenes son capturas directas de la ventana usando el ejecutable del release, con perfil
aislado y los archivos sintéticos de `D:\NaygoDemo`. Se inspeccionaron antes de incorporarlas:
no contienen carpetas personales, nombres de cuenta ni capacidad real de discos. Las capturas no
validan la matriz interactiva de arrastre OLE.
