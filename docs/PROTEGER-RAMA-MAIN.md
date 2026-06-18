# Proteger la rama `main` en GitHub

GitHub muestra la advertencia **"Your main branch isn't protected"**. Proteger la rama
evita borrados y *force-push* accidentales sobre `main`, y permite exigir revisión o
estados verdes antes de mezclar.

Esta acción modifica la configuración de control de acceso del repositorio, por lo que la
realiza el dueño (admin) del repositorio: **Nicolás**. Aquí están las dos formas de hacerlo.

> Repositorio: `nicolasgroth/explorador_archivos_naygo`
> Rama a proteger: `main`

---

## Opción A — Por la interfaz web (recomendada, más visual)

1. Entra al repositorio en GitHub y abre **Settings** (Configuración), arriba a la derecha.
2. En el menú lateral izquierdo: **Branches** (Ramas).
3. En **Branch protection rules** pulsa **Add branch ruleset** o **Add rule**
   (según la versión de la interfaz; ambas llegan al mismo lugar).
   - Si usa el formato clásico ("Add rule"): en **Branch name pattern** escribe `main`.
   - Si usa **rulesets**: pon un nombre (p. ej. `Proteger main`), en **Target branches**
     elige **Include default branch** (o agrega el patrón `main`), y en **Enforcement
     status** déjalo en **Active**.
4. Marca, como mínimo, estas casillas:
   - **Require a pull request before merging** (exigir PR antes de mezclar).
     - Dentro de esto, **Require approvals** con al menos **1** aprobación es lo habitual.
       *Nota:* si trabajas en solitario, esto te obligará a abrir PRs y aprobarlas; si te
       resulta incómodo, puedes dejar el require-PR activado pero approvals en 0, o
       desmarcarlo y quedarte solo con las protecciones de abajo.
   - **Do not allow bypassing the above settings** queda a tu criterio. Si lo dejas
     activado, ni siquiera los admins pueden saltarse la regla (más estricto).
5. Recomendado adicional (siempre seguro, no estorba el trabajo en solitario):
   - **Block force pushes** (bloquear *force-push*).
   - **Restrict deletions** (impedir borrar la rama).
6. Si en el futuro agregas CI (el repo ya sugiere un workflow de Rust):
   - **Require status checks to pass before merging** y selecciona el check de build/test.
     Déjalo para cuando el workflow exista; no lo marques si aún no hay checks.
7. Pulsa **Create** / **Save changes**.

Al volver a la portada del repositorio, la advertencia debería desaparecer.

---

## Opción B — Con la CLI `gh` (rápida, copia y pega)

Requiere `gh` autenticado con scope `repo` (ya lo está en este equipo). Este comando crea
una protección razonable para un proyecto que de momento desarrollas tú: **bloquea
force-push y borrado**, y **exige PR** pero sin obligar aprobaciones de terceros.

```bash
gh api -X PUT repos/nicolasgroth/explorador_archivos_naygo/branches/main/protection \
  -H "Accept: application/vnd.github+json" \
  -F "required_status_checks=null" \
  -F "enforce_admins=false" \
  -F "required_pull_request_reviews[required_approving_review_count]=0" \
  -F "restrictions=null" \
  -F "allow_force_pushes=false" \
  -F "allow_deletions=false"
```

Notas sobre los campos:
- `required_status_checks=null` → todavía no exige CI (cámbialo cuando tengas workflow).
- `enforce_admins=false` → tú (admin) puedes saltarte la regla si hace falta; ponlo en
  `true` para máxima rigidez.
- `required_pull_request_reviews[...]=0` → exige abrir PR pero no aprobaciones externas
  (cómodo en solitario). Sube a `1` si quieres revisión obligatoria.
- `allow_force_pushes=false` y `allow_deletions=false` → las dos protecciones clave.

Si prefieres **solo** bloquear force-push/borrado sin exigir PR, usa esta variante mínima:

```bash
gh api -X PUT repos/nicolasgroth/explorador_archivos_naygo/branches/main/protection \
  -H "Accept: application/vnd.github+json" \
  -F "required_status_checks=null" \
  -F "enforce_admins=false" \
  -F "required_pull_request_reviews=null" \
  -F "restrictions=null" \
  -F "allow_force_pushes=false" \
  -F "allow_deletions=false"
```

Para **ver** la protección activa en cualquier momento:

```bash
gh api repos/nicolasgroth/explorador_archivos_naygo/branches/main/protection
```

Para **quitarla** (si te arrepientes):

```bash
gh api -X DELETE repos/nicolasgroth/explorador_archivos_naygo/branches/main/protection
```

---

## ¿Qué recomiendo para Naygo hoy?

Como por ahora desarrollas principalmente tú y empujas a mano, lo más práctico sin
estorbar es: **bloquear force-push + bloquear borrado** (Opción B variante mínima, o las
casillas *Block force pushes* + *Restrict deletions* de la Opción A). Eso ya hace
desaparecer la advertencia y te protege de los accidentes más graves. Cuando sumes CI o
más colaboradores, activa *Require PR* y *Require status checks*.
