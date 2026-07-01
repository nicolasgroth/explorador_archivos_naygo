# Linux: pros, contras, licencias LGPL y la pregunta del "desde cero"

> Fecha: 2026-07-01. Documento de decisión, complementa la auditoría técnica
> (`2026-07-01-portabilidad-linux.md`, que tiene el mapa módulo a módulo y la hoja de
> ruta en 6 lotes). Este documento cubre lo que la auditoría no mide: si CONVIENE,
> qué implicaría flexibilizar la política de licencias, recomendaciones de proceso,
> y un análisis honesto de "¿mejor rehacer el proyecto desde cero?".

---

## 1. Pros y contras de portar Naygo a Linux

### Pros

1. **Audiencia nueva y receptiva.** El nicho "commander moderno, rápido, por teclado"
   está poco servido en Linux: las opciones son veteranas (Midnight Commander, Krusader,
   Double Commander) o abandonadas. Un explorador nuevo, rápido, MIT y bonito tiene
   espacio real. Además la comunidad Linux es la más propensa a probar, reportar y
   contribuir a proyectos open source — alineado con el objetivo del proyecto (dar a
   conocer el nombre de Nicolás Groth e ISGroth).
2. **El costo marginal es bajo gracias a la arquitectura.** No es una teoría: `core` y
   `platform` YA compilan para Linux (verificado con `cargo check` cruzado). La
   inversión de disciplina de las 3 capas se cobra aquí. El port es *aditivo* (una
   fachada nueva), no invasivo.
3. **El Lote 0 mejora Windows hoy.** Los bugs de case-sensitivity encontrados
   (batch-rename/undo pueden pisar el archivo equivocado) son latentes también en
   escenarios Windows (shares de red case-sensitive, WSL). Portar obligó a encontrarlos.
4. **CI y builds Linux son gratis** (GitHub Actions ya está montado; añadir un job
   `ubuntu-22.04` es una matriz más).
5. **Diversificación estratégica.** No depender de las decisiones de Microsoft
   (cambios de API, políticas de la tienda, telemetría del Explorer que empuja a la
   gente a buscar alternativas… en ambos mundos).
6. **Slint va en esa dirección.** El roadmap público de Slint ("desktop-ready", DnD
   cross-app vía winit upstream) significa que las lagunas actuales de Linux se cierran
   solas con el tiempo; portar ahora deja a Naygo listo para recibir esas mejoras.

### Contras

1. **Costo recurrente permanente (el contra más importante).** No es "6-9 semanas y
   listo": es mantener PARA SIEMPRE una matriz de prueba de 2 SO × 2 servidores
   gráficos (X11/Wayland) × 3+ escritorios (GNOME/KDE/XFCE). Cada bug report vendrá
   con la pregunta "¿qué distro, qué DE, X11 o Wayland?". Cada release son 4 artefactos
   (zip, setup, deb, AppImage) en vez de 2.
2. **Las features estrella degradan en el Ubuntu típico.** El usuario promedio de
   Ubuntu ≥22.04 usa GNOME + Wayland: ahí NO hay recordar-posición-de-ventana, NO hay
   hotkey global (hasta GNOME 48), NO hay drag hacia otras apps. Naygo-en-Linux v1 será
   objetivamente peor que Naygo-en-Windows, y las reviews no siempre distinguen "culpa
   de Wayland" de "culpa de la app".
3. **El diferencial competitivo es más débil.** En Windows, Naygo compite contra un
   Explorer lento y odiado. En Linux compite contra file managers nativos decentes
   (Nautilus/Dolphin) y commanders establecidos, con menos ventaja de velocidad
   percibida.
4. **Trabajo interactivo duro con API experimental.** El drag entre paneles necesita
   el DnD interno de Slint (experimental) — riesgo de rework cuando Slint lo estabilice.
5. **Costo de oportunidad.** 6-9 semanas en el port son 6-9 semanas sin features
   Windows (multi-ventana, miniaturas, visor, comprimir avanzado… el backlog existente).
6. **Infraestructura de pruebas personal.** Hoy Nicolás prueba en una VM Windows;
   necesitará al menos una VM Ubuntu (idealmente dos sesiones: Wayland y X11) y el
   hábito de validar en ambas.

### Balance

El port es técnicamente barato y estratégicamente razonable, PERO el costo real es el
mantenimiento perpetuo y la degradación Wayland visible. Recomendación al final (§3).

---

## 2. ¿Qué implicaría aceptar una licencia LGPL?

La pregunta surge porque los bindings oficiales de udisks2 para Rust son LGPL-2.1 (y
en el ecosistema Linux aparecerán más casos). Análisis honesto:

### Qué es LGPL (y qué NO es)

LGPL es *copyleft débil*: **usar una librería LGPL NO obliga a cambiar la licencia de
Naygo** (seguiría siendo MIT). Las obligaciones reales son: conservar avisos de
copyright, entregar el código fuente de la librería LGPL, y — la clave — **garantizar
que el usuario pueda sustituir la librería LGPL por una versión modificada** (el
requisito de "re-enlace", §6 de LGPL-2.1 / §4 de LGPL-3.0).

### El matiz crítico: enlace dinámico vs estático

- **Enlace DINÁMICO (librerías .so del sistema): cumplimiento trivial.** El usuario
  reemplaza el .so y listo. Así funciona TODA app Linux con glibc (LGPL) y GTK (LGPL).
  **Punto importante que la política actual no contempla: el port a Linux YA implica
  enlazar dinámicamente librerías LGPL de sistema** — glibc siempre, GTK3 +
  libappindicator si hay tray. Eso es universal, inevitable y no impone ninguna
  obligación práctica al código de Naygo. Nadie considera eso "usar LGPL".
- **Enlace ESTÁTICO (una crate Rust compilada dentro del binario): aquí está la
  incomodidad.** Rust enlaza crates estáticamente. Para cumplir el re-enlace habría
  que entregar los objetos compilados o el código fuente completo de la app. Como
  **Naygo es open source MIT con el código publicado, ese requisito se cumple de facto**
  — legalmente el riesgo es bajo.

### Entonces, ¿cuál es el problema real?

No es legal — es de **política y de terceros**:

1. **Rompe la promesa de simplicidad.** "Todas las dependencias permisivas, cero
   copyleft" es una regla que cualquiera entiende en 5 segundos. Con excepciones, cada
   dependencia nueva necesita análisis caso a caso, y los escáneres (cargo-license,
   cargo-deny) necesitan listas de excepciones.
2. **Complica la vida de quien reutilice Naygo.** El objetivo del proyecto es que la
   gente le saque provecho (incluso forks). Un fork CERRADO (empresa que adapte Naygo
   internamente) heredaría el problema LGPL de la crate estática y tendría que
   reemplazarla o cumplir re-enlace. MIT puro maximiza la reutilización sin fricción.
3. **Es innecesario en el caso concreto.** La alternativa MIT para udisks2 (proxies a
   mano sobre `zbus`) cuesta 1-2 días. No hay hoy NINGUNA dependencia LGPL estática
   que sea inevitable o cuyo reemplazo cueste más de unos días.

### Recomendación de política (para escribir en CLAUDE.md si se aprueba)

> **Crates Rust (enlace estático): solo licencias permisivas** (MIT/Apache-2.0/ISC/
> CC0/Zlib) — sin excepciones.
> **Librerías de sistema (enlace dinámico): LGPL aceptable** — es inevitable en Linux
> (glibc, GTK) y no impone obligaciones al código; se documentan en
> THIRD-PARTY-NOTICES como "system libraries".

Con esa formulación, la política actual se mantiene en espíritu, el port a Linux es
posible sin excepciones, y no se acepta ninguna crate LGPL.

---

## 3. Recomendaciones para hacer el port (si se decide hacerlo)

1. **Lote 0 primero, decida lo que se decida.** Los fixes de case-sensitivity,
   oculto=dotfile, mount-points y config XDG mejoran la corrección HOY y son
   pre-requisito de todo lo demás. Una semana bien invertida incluso si Linux nunca
   pasa del experimento.
2. **Fachada en `platform`, jamás `cfg` regados por la UI.** La UI llama la misma API;
   platform resuelve por SO. Es el patrón actual — mantenerlo a rajatabla (hay ~70
   call sites que lo agradecen).
3. **No pelear contra Wayland: degradar con dignidad y AVISAR.** Cada feature que
   Wayland no permite (posición, hotkey en GNOME<48, drag saliente) debe degradar con
   un aviso discreto e i18n-izado ("no disponible en esta sesión de escritorio"), no
   fallar en silencio. El usuario Linux entiende estas limitaciones si se le dicen.
4. **Hito visible temprano.** Priorizar llegar a "Naygo abre y navega en Ubuntu"
   (Lote 3) antes de pulir interacción (Lote 4). Un binario que corre motiva y permite
   feedback temprano de la comunidad.
5. **CI Linux desde el primer lote.** Job `ubuntu-22.04` en ci.yml apenas el workspace
   compile — evita regresiones silenciosas de portabilidad (como el `version: 1`
   hardcodeado que se cazó en el lote anterior: los gates automáticos encuentran esto
   gratis).
6. **VMs de prueba: GNOME+Wayland (el caso típico) y una sesión X11.** Kubuntu
   (KDE) como tercera si el tray/hotkey dan problemas.
7. **Publicar la intención y medir interés ANTES del Lote 3.** El repo es público:
   publicar la hoja de ruta (issue "Linux support" con la auditoría enlazada) y ver
   cuántos 👍/comentarios junta. Si la comunidad responde, es señal para invertir los
   lotes 3-5; si no, los lotes 0-1 ya valieron la pena por sí mismos.
8. **Drag saliente: feature-flag desde el día uno** (degradado en v1, se enciende
   cuando Slint/winit lo traigan). Drops entrantes: aceptar la limitación de winit
   0.30 (drop al panel activo) y saltar a 0.31 cuando Slint lo adopte.
9. **Releases: .deb primero, AppImage segundo, Flatpak nunca (por ahora).**
10. **Presupuestar el drag entre paneles como ítem propio** (DnD interno de Slint) —
    no está "incluido" en ningún módulo de platform y es paridad esencial.

---

## 4. ¿Sería mejor crear un proyecto desde cero "con todo lo aprendido"?

**Recomendación: NO. Con confianza alta.** Argumentos:

### Por qué no

1. **El argumento clásico para un rewrite no existe aquí.** Se reescribe cuando la
   arquitectura no da más de sí. La auditoría demostró empíricamente lo contrario:
   `core` compila para Linux sin tocar una línea, `platform` ya es una fachada con la
   frontera correcta, y la UI está desacoplada del SO. **El diseño actual ES el diseño
   que uno querría al empezar de nuevo.**
2. **El conocimiento está encarnado en el código, no en la memoria.** Naygo tiene ~880
   tests y decenas de bugs cazados y corregidos cuyo valor real es el código que los
   arregla: los footguns de RefCell, el render por software y sus casts i16, los
   watchers con eventos rezagados, el conflicto de copia que nunca pisa sin confirmar,
   el undo seguro, la saga completa del drag&drop, los splits degenerados… Un proyecto
   nuevo **tira esos arreglos y re-descubre los bugs uno a uno**. Lo aprendido no se
   transfiere por saberlo: se transfiere por tenerlo testeado.
3. **La economía no cierra.** El port son 6-9 semanas *aditivas* (lo existente sigue
   funcionando y mejorando). Un rewrite "con lo aprendido" costaría optimistamente
   el 60-70% del tiempo original (meses), con las features congeladas mientras tanto,
   y al final habría que portar a Linux IGUAL — el rewrite no se ahorra la fachada
   platform, que es donde está el trabajo del port.
4. **El "second-system effect" es real.** Los segundos sistemas nacen sobre-diseñados:
   la tentación de "esta vez lo hago genérico/multiplataforma/multi-ventana desde el
   día uno" produce arquitecturas especulativas. Naygo v1 es bueno precisamente porque
   creció feature a feature con tests.

### Lo que SÍ tiene sentido: refactors dirigidos

Todo lo que uno "haría distinto desde cero" es alcanzable como refactor acotado a ~10%
del costo de un rewrite, sin congelar nada:

| "Desde cero haría…" | Equivalente como refactor | Costo |
|---|---|---|
| Rutas case-correctas por plataforma | Lote 0 de la auditoría | días |
| Config en ubicación estándar | `config_root` portable-first+XDG (Lote 0) | horas |
| "Unidad" como mount point, no letra | Lote 0 | días |
| Multi-ventana desde el diseño | El refactor "proceso vs ventana" ya analizado (informe multi-ventana: 3-5 lotes) | semanas, por lotes |
| DnD interno sin OLE | Ítem del Lote 4 (DnD de Slint) | M-L |

### La única situación donde reconsideraría

Si algún día Slint dejara de servir (abandono del proyecto, cambio de licencia) y
hubiera que cambiar de framework UI, ESE sería el momento de evaluar cuánto se rescata
(core entero, platform entero) y cuánto se reescribe (solo ui-slint, que ya está
aislada). Nótese que incluso ese escenario catastrófico NO es un rewrite total gracias
a las 3 capas — otro argumento a favor de la arquitectura actual.

---

## 5. Síntesis para decidir

- **El port es viable, aditivo y de costo conocido** (auditoría con evidencia empírica).
- **El costo real es el mantenimiento perpetuo**, no las 6-9 semanas.
- **No hace falta aceptar ninguna crate LGPL** — con la política refinada de §2
  (permisivas para crates, LGPL dinámica de sistema aceptada como inevitable) el port
  entero se hace sin excepciones.
- **Rehacer desde cero sería un error**: la arquitectura ya es la correcta y lo
  aprendido vive en los tests, no en la memoria.
- **Camino sugerido de mínimo arrepentimiento**: Lote 0 ya (gana Windows), Lote 1 si
  hay ánimo (barato), publicar la intención y **medir interés de la comunidad antes de
  comprometer los lotes 3-5**.
