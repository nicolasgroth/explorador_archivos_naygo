# Distribución de Naygo

Naygo se distribuye de dos formas, generadas por `scripts\build-release.ps1`
(ver [BUILD.md](BUILD.md)).

## Portable (ZIP)

Última distribución comprobada y estado: [ESTADO-ACTUAL.md](ESTADO-ACTUAL.md).
Una actualización de documentación no implica que se hayan regenerado estos paquetes.

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
- **Símbolos de depuración (opcional, desmarcado)**: instala `naygo.pdb` solo si se va a
  investigar un crash. No es necesario para usar Naygo y se deja fuera del ZIP portable para
  mantener la descarga liviana.

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

## Probar mejoras sin afectar la instalación de trabajo

Conservar el instalador/portable validado y respaldar la configuración antes de migraciones.
Usar una copia portable en una carpeta de prueba con preferencias propias y datos ficticios.
La configuración portable se resuelve junto al ejecutable; no asumir que cambiar el directorio
de trabajo la aísla ni inventar un parámetro `--config-dir` no implementado.

La base 0.5.1 usa esquema 4. El intento de preview propone 5, aún no integrado: abrir una
configuración futura con el ejecutable anterior carga defaults y puede acabar sobrescribiendo
preferencias al guardar. No alternar ambos sobre la misma configuración. Evaluar primero si
el cambio de esquema es necesario y probar downgrade con copias, nunca con originales.

La instancia única puede redirigir un arranque al proceso ya abierto. Para una validación
instalada, coordinar con Nicolás salir desde la bandeja (la X normalmente sólo oculta),
verificar la versión/build efectiva y no cerrar ni modificar su sesión sin autorización.
Probar 7-Zip, Papelera, teclado, DPI y red usando escenarios controlados según
[PRUEBAS.md](PRUEBAS.md). No publicar configuraciones ni rutas personales.

## Advertencia de SmartScreen (importante)

La distribución 0.5.2 actualmente en `dist/` tiene el `.exe` y el instalador **sin firmar**;
SignPath aún no está configurado ni aprobado. Al ejecutar esos archivos, Windows SmartScreen puede
mostrar:

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

### Firma de código (mantenedores)

La firma Authenticode identifica al publicador y detecta cambios posteriores al firmado. No es una
firma manuscrita ni se inventa localmente: una CA valida la identidad y emite el certificado. Una
firma autofirmada sirve para pruebas controladas, pero Windows no la confía por defecto y no sirve
para distribuir Naygo públicamente.

El workflow de SignPath valida que las firmas del `naygo.exe` portable y del instalador estén
`Valid` y que el nombre simple del certificado firmante sea exactamente `SignPath Foundation`.
Registra sujeto, emisor y thumbprint para revisar el certificado recibido en la primera corrida real.

Cuando hay un certificado autorizado en el store de Windows, `build-release.ps1` firma/verifica
`naygo.exe` antes de armar el ZIP portable y el instalador; después firma/verifica `setup.exe` y
calcula los SHA-256 finales. Configura `NAYGO_SIGNTOOL` con `signtool.exe` del Windows SDK,
`NAYGO_SIGN_CERT_SHA1` con el thumbprint del certificado y, opcionalmente,
`NAYGO_SIGN_TIMESTAMP_URL` para RFC 3161. Los secretos no se escriben en el repositorio. Un ZIP no
recibe una firma Authenticode propia; el `.exe` que contiene sí queda firmado.
Sin certificado local, el empaquetador continúa únicamente si el artefacto está `NotSigned` o ya
tiene una firma `Valid`; cualquier otro estado Authenticode aborta el build para no distribuir ni
volver a firmar silenciosamente un archivo alterado o no confiable.

#### Preparación para SignPath Foundation

Nicolás eligió evaluar la modalidad OSS gratuita. Aún no se creó la organización/proyecto ni se
solicitó la aprobación; por tanto, los artefactos actuales siguen sin firma. El certificado sería
de SignPath Foundation y ese sería el publicador visible en Windows. La elegibilidad exige licencia
OSS aprobada por OSI para todos los componentes, proyecto ya publicado y documentado, sin código
propietario, repositorio/build verificables, MFA en GitHub y SignPath para los mantenedores, y
aprobación manual para cada release. SignPath debe aceptar el proyecto; el flujo local no puede
reemplazar esa aprobación. La postulación se inicia en [SignPath Foundation](https://signpath.org/apply).

El workflow de release admite SignPath una vez habilitado en la configuración del repositorio:

- Variable `NAYGO_SIGNPATH_ENABLED=true`.
- Secret `SIGNPATH_API_TOKEN`.
- Variables `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`,
  `SIGNPATH_SIGNING_POLICY_SLUG`, `SIGNPATH_EXE_ARTIFACT_CONFIGURATION` y
  `SIGNPATH_SETUP_ARTIFACT_CONFIGURATION`.
- En SignPath, importar `signpath/naygo-exe.xml` y `signpath/naygo-setup.xml` como configuraciones
  del proyecto; ambas restringen el nombre del producto a `Naygo` y exigen la versión del build.
  Vincular GitHub.com como Trusted Build System y configurar la política OSS con aprobación manual.
  [La Code signing policy](POLITICA-DE-FIRMA.md) está preparada y enlazada desde README y las
  notas que generará el workflow de Release;
  revisar los roles con el equipo real y publicarla junto con el aviso de privacidad antes de
  firmar releases.

Con SignPath habilitado, el workflow compila el EXE en runner GitHub-hosted, pasa la versión de
Cargo a las restricciones de metadata de ambos artefactos, lo envía para firma,
reconstruye ambos paquetes desde ese EXE firmado, firma el instalador, verifica las dos firmas y
calcula hashes después de firmar. La aprobación puede dejar el workflow esperando hasta una hora.
Si la variable está deshabilitada, el build conserva el modo sin firmar y el release lo declara.
Una firma válida no garantiza que SmartScreen elimine avisos en la primera descarga: la reputación
se acumula con las versiones y el uso.

### Integración continua

Aparte del release, el workflow `CI` corre el gate (formato + tests + clippy) en cada push
y pull request a `main`. Es el conjunto de checks natural para exigir en la protección de la
rama `main` (ver `docs/PROTEGER-RAMA-MAIN.md`).
