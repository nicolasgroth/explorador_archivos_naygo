# Distribución de Naygo

Naygo se distribuye de dos formas, generadas por `scripts\build-release.ps1`
(ver [BUILD.md](BUILD.md)).

## Portable (ZIP)

`Naygo-<versión>-portable.zip` contiene `naygo.exe`, `LICENSE`, `LEEME.txt` y
`THIRD-PARTY-NOTICES.md`.
Descomprimir y ejecutar — no instala nada. **La configuración se guarda junto al
`.exe`** (modo portable): si mueves el ejecutable, lleva también los archivos de config que
crea a su lado. Ideal para probar rápido en una VM o llevar en un pendrive.

## Instalador (setup.exe)

`Naygo-<versión>-setup.exe` es un asistente (Inno Setup). Ofrece:

- **Modo de instalación**: "para mí" (sin permisos de administrador, instala en
  `%LocalAppData%\Programs\Naygo`) o "para todos" (requiere administrador, instala en
  `C:\Program Files\Naygo`). El asistente lo pregunta.
- **Accesos directos**: en el menú Inicio siempre; en el Escritorio si marcas la opción.
- **"Abrir con" (opcional)**: registra a Naygo en el menú "Abrir con" de carpetas, sin
  hacerlo predeterminado (no toca Win+E ni reemplaza el Explorador).
- **"Abrir en Naygo" (opcional)**: agrega una entrada al menú contextual (clic derecho)
  de carpetas y del fondo de carpeta, que abre esa carpeta en Naygo.
- **Idioma del asistente**: el instalador está disponible en **siete idiomas** (inglés,
  español, alemán, francés, italiano, portugués y japonés) y detecta el idioma del
  sistema para preseleccionarlo.
- **Idioma inicial de Naygo**: un paso adicional para elegir con qué idioma arranca
  Naygo entre los **diez disponibles**, preseleccionado según el idioma de Windows.
  Solo aplica en instalaciones **nuevas**: si ya existe un `settings.json` (una
  actualización), no se toca.
- **"Iniciar Naygo al arrancar Windows" (opcional)**: casilla que crea la entrada Run
  del usuario para que Naygo arranque con Windows (con el argumento `--tray`, minimizado
  en la bandeja).
- **Ejecutar al terminar**: opción en la última página.

### Qué escribe en el sistema

- Archivos: el `.exe`, `LICENSE`, `README.md` y `THIRD-PARTY-NOTICES.md` en la carpeta
  de instalación.
- Accesos directos: menú Inicio (y Escritorio si se eligió).
- Registro (solo si marcaste las opciones): claves bajo `Software\Classes` (en HKCU
  para "para mí", HKLM para "para todos") para "Abrir con" y el menú contextual; y la
  clave **Run** de `HKCU` (solo si marcaste "Iniciar Naygo al arrancar Windows").
- Un `settings.json` inicial con el idioma elegido en el paso de idioma (solo se crea
  si no existía uno previo).


### Desinstalar

Desde "Agregar o quitar programas" (o el acceso directo "Desinstalar Naygo"). Elimina
el ejecutable, los accesos directos y las claves de registro creadas. **No** borra la
configuración del usuario (queda en su ubicación; puedes borrarla a mano si quieres un
reinicio total).

## Advertencia de SmartScreen (importante)

El `.exe` y el instalador **no están firmados** (no hay certificado de firma de código
por ahora). La primera vez que los ejecutes, Windows SmartScreen puede mostrar:

> "Windows protegió tu PC — Editor desconocido"

Esto es normal en software open-source sin firma. Para continuar:

1. Haz clic en **"Más información"**.
2. Haz clic en **"Ejecutar de todos modos"**.

Naygo es open source (MIT); puedes revisar el código en
<https://github.com/nicolasgroth/explorador_archivos_naygo>.

---

## Publicar una versión (para mantenedores)

Naygo publica los releases con **GitHub Actions**: al crear un tag `vX.Y.Z`, un workflow
compila el portable y el instalador y los adjunta a un GitHub Release.

### Paso 1: subir la versión y el tag

Usa el script de versionado, que infiere el nivel (patch/minor/major) de los commits
convencionales desde el último tag, mueve el CHANGELOG y crea el commit y el tag:

```powershell
# Local (no publica): crea commit + tag
powershell -ExecutionPolicy Bypass -File scripts\bump.ps1

# Publicar directamente (crea commit + tag y hace push):
powershell -ExecutionPolicy Bypass -File scripts\bump.ps1 -Push
```

Opciones útiles: `-Level minor` fuerza el nivel; `-DryRun` muestra qué haría sin tocar nada.

Si corres `bump.ps1` sin `-Push`, publica el tag a mano cuando estés listo:

```powershell
git push ; git push --tags
```

### Paso 2: el workflow hace el resto

Al llegar el tag a GitHub, el workflow `Release`:

1. Valida que el tag coincida con la versión de `Cargo.toml`.
2. Corre los tests y clippy.
3. Instala Inno Setup y ejecuta `scripts\build-release.ps1` (portable + instalador).
4. Crea el GitHub Release y adjunta los cuatro archivos: los versionados
   (`Naygo-X.Y.Z-portable.zip`, `Naygo-X.Y.Z-setup.exe`) y las copias con nombre estable
   (`Naygo-portable.zip`, `Naygo-setup.exe`).

Puedes seguir el progreso en la pestaña **Actions** del repositorio.

### Link de descarga estable

Las copias con nombre fijo permiten un link que siempre apunta a la última versión:

```
https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip
https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-setup.exe
```

### Compilar sin publicar

Para generar los artefactos en `dist\` sin crear un release:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

### Integración continua

Aparte del release, el workflow `CI` corre el gate (formato + tests + clippy) en cada push
y pull request a `main`. Es el conjunto de checks natural para exigir en la protección de la
rama `main` (ver `docs/PROTEGER-RAMA-MAIN.md`).
