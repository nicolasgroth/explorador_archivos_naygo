# Avisos de software de terceros — Linux

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Este archivo prepara los avisos de la bandeja Linux aprobada para la alfa. **No es aún el inventario final de dependencias de una distribución Linux de Naygo**: faltan la compilación y validación nativas en Ubuntu 26.04, confirmar el conjunto exacto de bibliotecas runtime y revisar la fuente correspondiente que se entregará con el binario. Windows mantiene MIT y publica su inventario separado en [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

## Ayatana AppIndicator GLib

La integración Linux de la bandeja carga dinámicamente el runtime del sistema `libayatana-appindicator-glib.so.2`. El paquete de runtime de referencia es `libayatana-appindicator-glib2` **2.0.1-1build1**, Ubuntu 26.04 LTS (Resolute), arquitectura amd64, componente Universe ([ficha de Ubuntu](https://packages.ubuntu.com/resolute/amd64/libs/libayatana-appindicator-glib2)). El proyecto upstream es [AyatanaIndicators/libayatana-appindicator-glib](https://github.com/AyatanaIndicators/libayatana-appindicator-glib/).

El runtime upstream declara **GNU General Public License, versión 3**. La distribución Linux alfa se prepara bajo la excepción GPL-3 Linux-only aprobada por el autor; esta decisión no cambia la licencia MIT de Windows. El texto completo GPL-3 recibido con la fuente upstream verificada está en [`licenses/linux/GPL-3.0.txt`](licenses/linux/GPL-3.0.txt). Las atribuciones y clasificación de archivos del paquete Debian/Ubuntu de referencia están en [`licenses/linux/libayatana-appindicator-glib2-copyright`](licenses/linux/libayatana-appindicator-glib2-copyright), cuyo contenido coincide con el [`copyright` publicado por Ubuntu](https://changelogs.ubuntu.com/changelogs/pool/universe/liba/libayatana-appindicator-glib/libayatana-appindicator-glib_2.0.1-1build1/copyright); ese archivo también identifica una plantilla de documentación CC0-1.0, sin atribuir esa licencia al runtime.

### Procedencia verificada

Los artefactos de referencia se obtuvieron del archivo oficial Ubuntu Resolute y del upstream. La cadena APT autenticó el índice y el hash del paquete; las firmas del `.dsc`, del tar upstream y del `InRelease` se verificaron contra las claves publicadas de sus firmantes. El `.deb` no tiene una firma OpenPGP individual.

- Paquete: `libayatana-appindicator-glib2_2.0.1-1build1_amd64.deb`; SHA-256 `2D9FBEC8EBF70464DB426BF339D72ED5F2B2B84416816A1A4672B01486A54A3E`.
- Licencia upstream `COPYING`; SHA-256 `0B383D5A63DA644F628D99C33976EA6487ED89AAA59F0B3257992DEAC1171E6B`.
- `debian/copyright` extraído del paquete binario; SHA-256 `E4165280122AE49BFE0CDFA991BCA660453F09F66E777E3E6A42527E52298782`.
- La revisión, método, fuentes y límites de la verificación están documentados en [`docs/VALIDACION-L00-AUDITORIA-2026-10-17.md`](docs/VALIDACION-L00-AUDITORIA-2026-10-17.md).

La fuente de referencia inspeccionada no se incluye aquí como fuente correspondiente de una entrega Naygo: antes de distribuir se debe confirmar que coincide exactamente con la biblioteca usada por la alfa, incluir la fuente y materiales de compilación requeridos, generar el inventario final del artefacto y completar la revisión de derechos de contribución.
