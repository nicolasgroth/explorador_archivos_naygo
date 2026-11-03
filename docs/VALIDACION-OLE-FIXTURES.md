# Validación manual OLE con datos sintéticos

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Preparar un fixture nuevo desde el checkout que se probará:

```powershell
./scripts/prepare-e2-7zip-fixture.ps1 -ExtraFiles 256
```

La salida indica el ZIP, `archive-content` y `destination`, todos bajo `target/agent-out`.
No contiene datos personales y no instala ni modifica configuraciones de Naygo. Usar un perfil y
una instancia aislados de prueba; documentar versión exacta de Naygo, Windows y fuente.

1. Desde 7-Zip, abrir el ZIP, arrastrar `E2 fixture` al panel que muestra `destination` y confirmar
   copia. Ejecutar `./scripts/verify-e2-drop-fixture.ps1 -FixtureDirectory <carpeta-del-fixture>`.
   Comprueba todos los archivos por SHA-256, nombres, estructura y subcarpetas vacías.
2. Repetir con fixtures nuevos desde Explorer y Directory Opus, arrastrando la carpeta de
   `archive-content` y eligiendo copiar. No reutilizar un destino ya ocupado sin probar conflictos.
3. Para mover o cancelar, crear otra copia sintética independiente de `E2 fixture` dentro de la
   carpeta del fixture y arrastrarla al destino; conservar `archive-content` como referencia para
   el verificador. Tras cancelar, comprobar origen intacto y destino vacío; tras confirmar mover,
   comprobar origen ausente y destino idéntico. Usar fixtures distintos para cada caso.
4. Repetir con Ctrl/Shift y con origen/ventana navegados o cerrados durante la preparación, además
   de conflictos, errores y cancelación. Registrar qué se probó y qué no, sin rutas privadas.

El verificador solo lee rutas dentro de `target/agent-out` del checkout, rechaza enlaces dentro de
los árboles comparados y no limpia ni modifica archivos. Su éxito demuestra la copia concreta,
no certifica compatibilidad con otros programas, grandes volúmenes o proveedores virtuales.
