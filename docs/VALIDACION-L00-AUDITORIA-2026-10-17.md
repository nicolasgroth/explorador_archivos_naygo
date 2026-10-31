# Auditoría local L-00: autoría y licencias — 2026-10-17

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Resultado

- `git log --all --format='%aN <%aE>' | Sort-Object -Unique` y la consulta equivalente del
  committer presentan una sola identidad: `Nicolás Groth <ngroth@gmail.com>`. La identidad histórica
  Yahoo se normaliza mediante `.mailmap`, confirmado previamente por Nicolás; no se reescribió el
  historial. El shortlog atribuye 948 commits a esa identidad.
- El árbol contiene 275 archivos de código con extensiones `.rs`, `.slint`, `.c`, `.h`, `.cpp` y
  `.cc`. Un barrido de las primeras 35 líneas encontró `SPDX-License-Identifier` en los 275.
- La consulta de archivos versionados no encontró rutas `vendor`, `third_party` o `external`.
- `cargo license --json --avoid-build-deps --avoid-dev-deps --filter-platform
  x86_64-unknown-linux-gnu` produjo 543 paquetes del grafo Linux actual. No se encontraron
  dependencias Cargo con GPL/LGPL/AGPL como única licencia declarada; Slint enumera también GPL-3
  como alternativa, junto a la opción `LicenseRef-Slint-Royalty-free-2.0` elegida por el proyecto.
  `option-ext 0.2.0` es MPL-2.0 y llega por `tray-icon → dirs → dirs-sys`. Once paquetes Slint
  1.16.1 declaran esa opción triple.
- `cargo tree -i gtk --target x86_64-unknown-linux-gnu` muestra que el stack GTK actual llega por
  `tray-icon → libappindicator/muda → gtk`. La migración Linux aprobada reemplazará esa ruta por el
  backend GLib; el grafo debe reauditarse después del cambio.

## Alcance y pendientes

Este barrido verifica identidad de los commits, presencia de encabezados y metadatos de licencia del
grafo Cargo. No prueba que nunca se haya copiado un fragmento de terceros, no inspecciona
exhaustivamente recursos binarios ni reemplaza la revisión de licencias del paquete runtime Ubuntu.
La implementación de Ayatana AppIndicator GLib y su paquete pueden imponer condiciones GPL-3
adicionales; Nicolás autorizó GPL-3 solo para Linux. Antes de distribuir Linux faltan la fuente
correspondiente, notices finales basados en las dependencias realmente enlazadas/empacadas, y una
revisión del copyright del paquete binario elegido. Este cambio es documental y no altera la build
Windows.

## Revisión del source package de bandeja — 2026-10-18

Se inspeccionó la metadata local de `libayatana-appindicator-glib 2.0.1-1build1`. El changelog Debian
declara `resolute` y registra un rebuild de Ubuntu 26.04. El archivo `debian/copyright` asigna GPL-3
a la biblioteca y a la mayor parte de los archivos listados, además de CC0-1.0 para una plantilla de
documentación. `debian/control` identifica el runtime binario como `libayatana-appindicator-glib2`.
El SHA-256 del `debian.tar.xz` coincide con el checksum anotado por el `.dsc` local.

Esto confirma que la excepción GPL-3 Linux-only aprobada por Nicolás sí es relevante para el
runtime elegido y no introduce una licencia nueva respecto de esa decisión. La revisión no verifica
criptográficamente la firma del `.dsc`, no contiene el tar upstream original de la versión 2.0.1 ni
confirma qué paquete/DSONAME quedará enlazado y declarado por la alfa. Antes de empaquetar siguen
pendientes la fuente correspondiente completa, los notices finales y verificar dependencias runtime
desde una instalación nativa Ubuntu 26.04. Los archivos inspeccionados y extraídos permanecen bajo
`target/agent-out/linux-glib-l00/` y no se incluyeron en la distribución.

## Reauditoría del grafo Cargo Linux — 2026-10-18

Después de condicionar `tray-icon` a Windows y activar `fontique-dlopen` solo para Linux, se ejecutó
`cargo license --json --avoid-build-deps --avoid-dev-deps --filter-platform x86_64-unknown-linux-gnu`.
El inventario actual contiene 500 paquetes y no tiene expresiones de licencia desconocidas. No aparece
ninguna licencia MPL ni una dependencia GPL/LGPL/AGPL como opción única; 11 crates de Slint 1.16.1
declaran la alternativa `LicenseRef-Slint-Royalty-free-2.0` junto con GPL-3.0 y
`LicenseRef-Slint-Software-3.0`. La presencia de esas alternativas no demuestra por sí sola qué
obligación aplica: el proyecto conserva la selección documentada de la licencia royalty-free de
Slint.

`cargo tree -i tray-icon --target x86_64-unknown-linux-gnu --locked --offline` no encuentra el
crate en el grafo Linux. La salida JSON queda como evidencia de trabajo en
`target/agent-out/linux-license-inventory-2026-10-18.json`, fuera de la distribución. El inventario
Cargo no cubre Ayatana AppIndicator GLib, GLib/Gio, Fontconfig u otras bibliotecas dinámicas del SO;
Ayatana sigue bajo la excepción GPL-3 Linux-only autorizada. Los notices finales dependen del
paquete alfa real y de la auditoría del runtime nativo, por lo que L-00 permanece abierto.

## Verificación criptográfica del source y del paquete Ubuntu — 2026-10-18

La falta de `gpg`/`gpgv` se resolvió para esta auditoría con PGPy y `cryptography` en una ruta aislada
bajo `target/agent-out/`; no se instaló GnuPG ni se modificó el sistema. El reporte y el verificador
reproducible están en `target/agent-out/linux-glib-l00/signature-verification-2026-10-18.txt` y
`target/agent-out/verify_ayatana_signatures.py`.

- La firma clara del `.dsc` valida con la clave de Sebastien Bacher, cuya huella completa
  `92FE4F8B40CADC4ECCD60C1F5961FDBE9C619809` coincide con la clave publicada en su cuenta de
  Launchpad. El `.dsc` declara correctamente el SHA-256 y tamaño del tar upstream.
- La firma detached del tar upstream valida con la clave de Mike Gabriel,
  `9BFBAEE86C0AA5FFBF2207829AF46B3025771B31`; la clave obtenida del keyring HKP activo de Debian
  coincide con la huella completa del firmante. La identidad del keyring se contrastó con el registro
  oficial de Debian.
- La firma embebida del `InRelease` de Resolute valida con la clave de archivo Ubuntu 2018,
  `F6ECB3762474EDA9D21B7022871920D1991BC93C`, cuya identidad y rol corresponden a la documentación
  oficial. El SHA-256 y tamaño del `universe/binary-amd64/Packages.xz` coinciden con ese `InRelease`;
  el SHA-256 del `.deb` coincide con el stanza de `libayatana-appindicator-glib2 2.0.1-1build1` en
  dicho índice firmado.

El `.deb` no lleva una firma OpenPGP individual: su integridad queda autenticada por la cadena
`InRelease` firmado → índice `Packages` → hash del `.deb`, como documenta Ubuntu. PGPy 0.6.0 informa
que no implementa comprobación de autosignaturas/revocaciones de certificados; las huellas se
contrastaron por separado con Launchpad, el keyring Debian y documentación Ubuntu. Por ello queda
cerrada la verificación criptográfica de los tres artefactos y la cadena APT, pero este método no
equivale a una auditoría completa del trust database de GnuPG. Siguen pendientes la verificación
final del source correspondiente/notices del artefacto Naygo y comprobar el runtime desde Ubuntu
26.04 nativo.

## Avisos preliminares para el runtime GLib — 2026-10-18

Se preservaron byte a byte la licencia upstream `COPYING` y el `debian/copyright` obtenido del
paquete binario autenticado en `licenses/linux/`. Sus SHA-256 coinciden con los artefactos
verificados que se documentan arriba. Se creó
[`THIRD-PARTY-NOTICES-LINUX.md`](../THIRD-PARTY-NOTICES-LINUX.md) para atribuir el runtime
`libayatana-appindicator-glib2` 2.0.1-1build1 y dejar explícitos la decisión GPL-3 Linux-only y
los límites pendientes. El archivo general de avisos ahora se identifica como inventario Windows.

Esto completa la preparación documental de avisos para el runtime candidato, no L-00 entero: el
inventario final depende de comprobar la biblioteca cargada y todos los paquetes del sistema en
Ubuntu 26.04 nativo, distribuir la fuente correspondiente exacta y cerrar la revisión de derechos.

## Revalidación de la fuente de licencia Ubuntu 26.04 — 2026-10-19

Se volvió a consultar la [ficha oficial amd64 del runtime Resolute](https://packages.ubuntu.com/resolute/amd64/libs/libayatana-appindicator-glib2): sigue en Universe como `libayatana-appindicator-glib2` `2.0.1-1build1`, con dependencias directas en `libc6` y `libglib2.0-0t64 (>= 2.80.0)`. Se descargó sólo para lectura el `copyright` que esa ficha enlaza; su SHA-256 (`E4165280122AE49BFE0CDFA991BCA660453F09F66E777E3E6A42527E52298782`) coincide byte a byte con `licenses/linux/libayatana-appindicator-glib2-copyright`. Ese archivo atribuye GPL-3 a la biblioteca C runtime y CC0-1.0 a la plantilla de documentación indicada.

La evidencia refuerza la procedencia de los avisos y la licencia del paquete candidato; no prueba que una build Linux futura cargue exactamente ese runtime ni reemplaza distribuir su fuente correspondiente o revisar derechos de contribución. WSL continúa sin iniciar, por lo que la validación nativa permanece pendiente.
