# 🔧 Configuración necesaria antes del primer release

Este PR agrega un sistema automático de releases con GitHub Actions. Antes de hacer
el primer release, necesitás actualizar las siguientes URLs placeholder:

## 📝 Archivos a actualizar

✅ **Ya están actualizados** con el repositorio correcto:
- `README.md` (link de descarga)
- `docs/DISTRIBUTION.md` (link estable)

No se requiere acción adicional.

## ✅ Verificar después del merge

1. **Probar el workflow** con un tag de prueba:
   ```bash
   # Usar el script interactivo (recomendado)
   powershell -ExecutionPolicy Bypass -File scripts\create-release.ps1
   
   # O manualmente
   git tag v0.1.0-test
   git push origin v0.1.0-test
   ```
2. **Verificar en GitHub:**
   - Actions → Build and Release (debería ejecutarse)
   - Releases → debería aparecer el nuevo release con `Naygo-portable.zip`
3. **Probar el link de descarga:**
   ```
   https://github.com/nicolasgroth/explorador_archivos_naygo/releases/latest/download/Naygo-portable.zip
   ```

## 🎯 Qué hace este PR

- ✅ Workflow de GitHub Actions que se dispara al crear tags `v*.*.*`
- ✅ Ejecuta tests + lints antes de publicar
- ✅ Compila y empaqueta automáticamente
- ✅ Crea GitHub Release con el ZIP adjunto
- ✅ Script interactivo `create-release.ps1` para facilitar el proceso
- ✅ Documentación completa en `docs/DISTRIBUTION.md`
- ✅ Link de descarga permanente (sin versión en el nombre del archivo)

## 🚀 Uso después del merge

Para crear un nuevo release:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\create-release.ps1
```

El script te guiará por todo el proceso:
1. Verifica que el repo esté limpio
2. Te pide la nueva versión
3. Actualiza `Cargo.toml`
4. Crea el commit y tag
5. Hace push
6. Te da los links para seguir el progreso

---

**Nota:** Este sistema no requiere configuración adicional en GitHub. Los permisos
necesarios (`contents: write`) ya están incluidos en el workflow.
