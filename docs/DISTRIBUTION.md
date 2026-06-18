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

Naygo es open source (MIT); el código fuente está disponible en el repositorio
de GitHub del proyecto.

---

## Crear un release (para mantenedores)

Naygo utiliza **GitHub Actions** para automatizar la compilación y publicación de
nuevas versiones. El workflow se dispara automáticamente al crear un tag semántico.

### Proceso de release

#### Método recomendado: Script interactivo

Ejecutá el script de release que automatiza todo el proceso:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\create-release.ps1
```

El script:
- ✅ Verifica que el repositorio esté limpio y sincronizado
- ✅ Te pide la nueva versión (valida formato SemVer)
- ✅ Actualiza automáticamente `Cargo.toml`
- ✅ Crea el commit y el tag
- ✅ Hace push y te muestra los links para seguir el progreso

#### Método manual

Si preferís hacer el proceso a mano:

1. **Actualizar la versión en `Cargo.toml`**:
   ```toml
   [workspace.package]
   version = "1.0.0"
   ```

2. **Crear commit y tag**:
   ```bash
   git add Cargo.toml
   git commit -m "chore: bump version to v1.0.0"
   git tag -a v1.0.0 -m "Release v1.0.0"
   ```

3. **Publicar**:
   ```bash
   git push && git push origin v1.0.0
   ```

#### GitHub Actions se encarga del resto

Una vez que el tag se publique:
- Ejecuta los tests (`cargo test --workspace`)
- Ejecuta lints (`cargo clippy`)
- Compila el proyecto en modo release
- Empaqueta `Naygo-portable.zip` con el ejecutable, LICENSE y LEEME.txt
- Crea un GitHub Release con el tag
- Adjunta el ZIP como asset descargable

### Link estable de descarga

El workflow genera un archivo con nombre fijo (`Naygo-portable.zip`, sin versión),
lo que permite usar un **link estable** que siempre apunta a la última versión:

```
https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip
```

Este link está publicado en el README.md para que los usuarios siempre descarguen
la versión más reciente.

### Verificar el release

Después de hacer push del tag:

1. Ir a **Actions** en GitHub para ver el progreso del workflow `Build and Release`
2. Una vez completado (✅), ir a **Releases** para ver el nuevo release
3. Verificar que el asset `Naygo-portable.zip` está disponible
4. Descargar y probar el ejecutable localmente

### Deshacer un release erróneo

Si necesitás eliminar un release:

```bash
# Borrar el release en GitHub
gh release delete v1.0.0

# Borrar el tag local y remoto
git tag -d v1.0.0
git push origin :refs/tags/v1.0.0
```

### Versionado semántico

Seguir [SemVer](https://semver.org/) para los tags:

- **v1.0.0**: release inicial o cambios mayores incompatibles
- **v1.1.0**: nuevas características (compatible hacia atrás)
- **v1.0.1**: correcciones de bugs (compatible hacia atrás)

**Importante:** La versión en `Cargo.toml` debería coincidir con el tag creado
para mantener consistencia en los metadatos del ejecutable.

### Compilación manual (sin GitHub Actions)

Si necesitás compilar y empaquetar localmente:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

Este script genera los artefactos en la carpeta `dist\`, pero **no los publica
automáticamente** en GitHub. Para publicar manualmente:

```bash
gh release create v1.0.0 dist\Naygo-portable.zip --title "Release v1.0.0"
```
