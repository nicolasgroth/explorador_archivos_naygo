# Distribución de Naygo

Naygo se distribuye de dos formas, generadas por `scripts\build-release.ps1`
(ver [BUILD.md](BUILD.md)).

## Portable (ZIP)

`Naygo-<versión>-portable.zip` contiene `naygo.exe`, `LICENSE` y `LEEME.txt`.
Descomprimir y ejecutar — no instala nada. **La configuración se guarda junto al
`.exe`** (modo portable): si movés el ejecutable, llevate los archivos de config que
crea a su lado. Ideal para probar rápido en una VM o llevar en un pendrive.

## Instalador (setup.exe)

`Naygo-<versión>-setup.exe` es un asistente (Inno Setup). Ofrece:

- **Modo de instalación**: "para mí" (sin permisos de administrador, instala en
  `%LocalAppData%\Programs\Naygo`) o "para todos" (requiere administrador, instala en
  `C:\Program Files\Naygo`). El asistente lo pregunta.
- **Accesos directos**: en el menú Inicio siempre; en el Escritorio si marcás la opción.
- **"Abrir con" (opcional)**: registra a Naygo en el menú "Abrir con" de carpetas, sin
  hacerlo predeterminado (no toca Win+E ni reemplaza el Explorador).
- **"Abrir en Naygo" (opcional)**: agrega una entrada al menú contextual (clic derecho)
  de carpetas y del fondo de carpeta, que abre esa carpeta en Naygo.
- **Ejecutar al terminar**: opción en la última página.

### Qué escribe en el sistema

- Archivos: el `.exe`, `LICENSE` y `README.md` en la carpeta de instalación.
- Accesos directos: menú Inicio (y Escritorio si se eligió).
- Registro (solo si marcaste las opciones): claves bajo `Software\Classes` (en HKCU
  para "para mí", HKLM para "para todos") para "Abrir con" y el menú contextual.

### Desinstalar

Desde "Agregar o quitar programas" (o el acceso directo "Desinstalar Naygo"). Elimina
el ejecutable, los accesos directos y las claves de registro creadas. **No** borra la
configuración del usuario (queda en su ubicación; podés borrarla a mano si querés un
reinicio total).

## Advertencia de SmartScreen (importante)

El `.exe` y el instalador **no están firmados** (no hay certificado de firma de código
por ahora). La primera vez que los ejecutés, Windows SmartScreen puede mostrar:

> "Windows protegió tu PC — Editor desconocido"

Esto es normal en software open-source sin firma. Para continuar:

1. Hacé clic en **"Más información"**.
2. Hacé clic en **"Ejecutar de todos modos"**.

Naygo es open source (MIT); podés revisar el código en
<https://github.com/nicolasgroth/explorador_archivos_naygo>.
