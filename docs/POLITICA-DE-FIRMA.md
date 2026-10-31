# Política de firma de código de Naygo

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Estado

No hay constancia local de que la postulación de Naygo al programa OSS de SignPath Foundation se
haya enviado. El ejecutable release 0.5.4 y el setup local 0.5.4 se comprobaron como `NotSigned` el
2026-10-30; el EXE del ZIP coincide con el release. La instalación previa 0.5.3 tampoco está firmada.
No se afirmará que una versión está firmada hasta que SignPath
Foundation acepte el proyecto y se verifiquen las firmas del ejecutable portable y del instalador.

La [release pública v0.3.0](https://github.com/nicolasgroth/explorador_archivos_naygo/releases/tag/v0.3.0)
ya ofrece el formato portable ZIP y el instalador que se propone firmar. Esto parece cubrir el
requisito de que el proyecto ya esté publicado en la forma que se firmará, pero SignPath conserva la
decisión de elegibilidad. Sus [condiciones vigentes para proyectos OSS](https://signpath.org/terms.html)
también exigen que el código esté mantenido, que las funciones estén documentadas en la página de
descarga, que la política de firma esté visible y que cada release tenga aprobación manual.

El README del repositorio enlaza esta política y el workflow añade el enlace con la etiqueta exacta
“Code signing policy” a las nuevas notas de release. La release pública histórica `v0.3.0` aún
no incluye ese enlace; las notas ya publicadas no se actualizan desde este repositorio.
Antes de enviar la postulación hay que confirmar MFA en la cuenta GitHub con acceso al repositorio y
en la cuenta SignPath, requisito explícito de los términos OSS. El estado de esas cuentas no se puede
verificar desde este repositorio.

Si el proyecto es aceptado, se aplicará esta política:

> Free code signing provided by SignPath.io, certificate by SignPath Foundation.

## Alcance

La firma cubre únicamente artefactos construidos desde el repositorio público oficial de Naygo,
por el workflow de release alojado en GitHub Actions. Se firmarán `naygo.exe` incluido en el ZIP
portable y el instalador `setup.exe`. La configuración valida el nombre `Naygo` y la versión del
producto contra la versión declarada en Cargo.toml. Los SHA-256 publicados se calculan después de
firmar.

Cada solicitud de firma de release requiere aprobación manual en SignPath. Los logs del workflow,
la solicitud aprobada y las firmas Authenticode se conservarán como evidencia de la versión.
Un ZIP no recibe firma Authenticode propia; la firma se verifica en el ejecutable que contiene.
El workflow aborta si un artefacto firmado no tiene estado Authenticode `Valid` o si el nombre simple
del firmante no es exactamente `SignPath Foundation`; registra sujeto, emisor y thumbprint para
revisión. Esa comprobación local aún debe confirmarse contra el certificado de la primera firma real.

## Configuración pendiente de SignPath

Después de la aceptación OSS, configurar el Trusted Build System `GitHub.com`, vincular el proyecto
al repositorio público, exigir verificación de origen y limitar la política release a referencias de
release. Open Source Code Signing requiere trusted build system y origin verification; todos los
jobs que preceden a la solicitud deben usar agentes alojados por GitHub. La acción ya carga cada
artefacto en GitHub antes de solicitar la firma.

Los XML locales [`naygo-exe.xml`](../signpath/naygo-exe.xml) y
[`naygo-setup.xml`](../signpath/naygo-setup.xml) configuran EXE dentro del ZIP de entrada y el
`setup.exe` preparado por el workflow. Crear en SignPath una configuración de artefacto para cada
XML, con Authenticode y restricciones `ProductName=Naygo` y `ProductVersion=${version}`. La acción
envía la versión de Cargo.toml como parámetro.

El workflow consume estas variables/secretos; los identificadores concretos se obtienen al crear el
proyecto y sus configuraciones, y el token nunca se guarda en el repositorio:

| Nombre | Tipo en GitHub | Origen |
| --- | --- | --- |
| `NAYGO_SIGNPATH_ENABLED` | Variable | Habilitar solo después de configurar y revisar todo el flujo |
| `SIGNPATH_ORGANIZATION_ID` | Variable | Identificador de la organización SignPath |
| `SIGNPATH_PROJECT_SLUG` | Variable | Slug del proyecto Naygo |
| `SIGNPATH_SIGNING_POLICY_SLUG` | Variable | Política release con aprobación manual |
| `SIGNPATH_EXE_ARTIFACT_CONFIGURATION` | Variable | Configuración basada en `naygo-exe.xml` |
| `SIGNPATH_SETUP_ARTIFACT_CONFIGURATION` | Variable | Configuración basada en `naygo-setup.xml` |
| `SIGNPATH_API_TOKEN` | Secreto | Token con permiso para enviar solicitudes a ambas configuraciones |

**Compatibilidad pendiente:** `.github/workflows/release.yml` se dispara desde tags `v*.*.*`.
GitHub define `GITHUB_REF` para ese evento como `refs/tags/<tag>`; la documentación de SignPath
habla de `Allowed branch names` para origin verification y no especifica en esa sección cómo se
admiten tags. Confirmar con SignPath qué patrón/referencia autoriza la política antes de habilitarla.
No cambiar el trigger ni firmar hasta resolver esta compatibilidad y probar ambos artefactos en un
run aprobado.

**Revalidación documental, 2026-10-16:** la guía oficial define origin verification como revisión
del repositorio, rama, commit y job; la restricción release se configura mediante `Allowed branch
names` (ejemplos `main` y `release/*`). La guía del conector GitHub confirma que el origen se obtiene
de GitHub y que el artefacto debe estar cargado como workflow artifact antes de solicitar firma,
pero no describe cómo se representa o autoriza un evento `push` de tag. Como este workflow usa
`push.tags: v*.*.*`, sigue faltando confirmación explícita de SignPath sobre la referencia que verá
la política; no se infiere que un tag sea una rama ni se modifica el trigger. Fuentes oficiales:
[Origin Verification](https://docs.signpath.io/origin-verification/),
[GitHub Trusted Build System](https://docs.signpath.io/trusted-build-systems/github) y
[Setting up Projects](https://docs.signpath.io/projects).

**Revalidación oficial, 2026-10-18:** las páginas actuales de Origin Verification y Setting up
Projects siguen expresando la restricción como `Allowed branch names`; la integración GitHub afirma
que el origen lo aporta GitHub, pero no especifica el valor que recibe el conector para un workflow
disparado por `push.tags`. No cambiaré el trigger ni asumiré que el tag se trata como rama. Pregunta
pendiente para SignPath antes de activar: para `push.tags: v*.*.*`, ¿qué `BranchName` verifica GitHub
en la solicitud de firma, qué patrón debe configurarse en `Allowed branch names` y puede la política
de release autorizar el tag conservando la verificación del repositorio, commit y workflow? Fuentes:
[Origin Verification](https://docs.signpath.io/origin-verification/),
[GitHub Trusted Build System](https://docs.signpath.io/trusted-build-systems/github) y
[Setting up Projects](https://docs.signpath.io/projects).

**Revisión adicional de ejemplo oficial, 2026-10-18:** el workflow de demostración de SignPath
selecciona `release-signing` únicamente para `refs/heads/main` y `refs/heads/release/*`; los demás
refs seleccionan `test-signing`. Esto confirma el modelo de ramas protegidas que describe la guía,
pero no dice si un tag puede usarse con verificación de origen ni qué nombre recibe. No se cambió el
trigger `push.tags` de Naygo. Fuente oficial: [demo-github-actions de SignPath](https://github.com/SignPath/demo-github-actions/blob/main/.github/workflows/build-and-sign.yml).

**Revalidación oficial, 2026-10-19:** se revisaron de nuevo Origin Verification, Setting up Projects
y el conector GitHub. Siguen documentando políticas con nombres de ramas como `main`/`release/*`;
ninguna página menciona `push.tags` ni especifica qué `BranchName` entrega GitHub para ese evento.
El bloqueo se mantiene hasta obtener confirmación del proyecto SignPath o hacer una prueba autorizada
con origin verification. No se habilitaron secretos ni se cambió el trigger. Fuentes oficiales:
[Origin Verification](https://docs.signpath.io/origin-verification/),
[Setting up Projects](https://docs.signpath.io/projects) y
[GitHub Trusted Build System](https://docs.signpath.io/trusted-build-systems/github).

**Revalidación oficial, 2026-10-20:** las páginas siguen limitando su lenguaje de política a nombres
de ramas (`main`, `release/*`); la referencia de origen indica `BranchName` y requiere que coincida
con la política, sin definir el valor para un workflow GitHub `push.tags`. No hay una regla pública
que permita concluir que el patrón `v*.*.*` funciona en `Allowed branch names`. El release workflow
permanece sin activar y la configuración/signing request requieren confirmación del proyecto o una
prueba aprobada con origin verification. Fuentes: [Origin Verification](https://docs.signpath.io/origin-verification/),
[Setting up Projects](https://docs.signpath.io/projects) y
[Submit-SigningRequest](https://docs.signpath.io/powershell/Submit-SigningRequest).

## Responsabilidades

- Autor y mantenedor: [Nicolás Groth](https://github.com/nicolasgroth), responsable del repositorio,
  código y scripts de build.
- Revisión: [Nicolás Groth](https://github.com/nicolasgroth) revisa los cambios incorporados al branch de release, incluidos los
  workflows y scripts que intervienen en la compilación.
- Aprobación de firma: [Nicolás Groth](https://github.com/nicolasgroth) revisa el origen, versión y resultado de CI antes de aprobar
  cada solicitud de firma.

Estas responsabilidades reflejan el mantenimiento individual actual del proyecto. Si se agregan
mantenedores, se actualizarán los responsables antes de concederles acceso de firma.

## Privacidad

Naygo no incluye telemetría. No transfiere información a otros sistemas en red salvo cuando el
usuario solicita explícitamente una operación que lo requiere, como acceder a una carpeta de red.
El proceso de distribución comunica los artefactos del build a GitHub Actions y SignPath para
construcción, firma y publicación; no envía datos de uso de la aplicación.

English statement for the SignPath Foundation application:

> This program will not transfer any information to other networked systems unless specifically
> requested by the user or the person installing or operating it.

## Descargas y verificación

La página de releases publica `SHA256SUMS.txt` para comprobar la integridad de los archivos
descargados. Una firma válida identifica a SignPath Foundation como publicador y permite comprobar
si el ejecutable fue alterado después de firmarse; no garantiza que Windows SmartScreen elimine
inmediatamente sus advertencias de reputación.

Las futuras páginas de descarga incluirán un enlace visible a esta política bajo el rótulo “Code
signing policy”. La release histórica `v0.3.0` no lo incluye; no se modificará una release publicada
sin una decisión explícita de mantenimiento.
