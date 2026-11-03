# Plan de cierre Windows: arrastre interoperable y validación

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

## Objetivo y estado

Preparar una entrega Windows integrada, priorizando el arrastre entre aplicaciones, sin perder
datos ni congelar la interfaz. La base instalada es 0.5.4 de avance: paneles auxiliares, cierre
automático configurable de Operaciones, recuentos y protección del efecto OLE ya están integrados.
El gate conjunto pasó 1553 pruebas, 9 ignoradas, Clippy y formato; el instalador no tiene firma.
No volver a implementar estos puntos: verificar su comportamiento real y corregir regresiones.

Una rama experimental recibió una carpeta real desde 7-Zip 24.07 y conservó 259 archivos y cuatro
subcarpetas, una vacía, contrastados por SHA-256. No está en la instalación: resolver el destino
asíncrono y la cancelación visible, además de ampliar las pruebas, antes de integrarla.

## Fundamento y elección técnica

El receptor debe admitir formatos anunciados por cada origen, no suponer que son equivalentes.
Windows ofrece `CF_HDROP` para rutas y formatos `FILEDESCRIPTOR`/`FILECONTENTS` para elementos
virtuales. `IDataObjectAsyncCapability` es opcional y solo debe usarse tras comprobar y negociar
su soporte. 7-Zip explica que al arrastrar a Explorer puede extraer primero a temporales; su
[código 24.07](https://github.com/ip7z/7zip/blob/a7a1d4a241492e81f659a920f7379c193593ebc6/CPP/7zip/UI/FileManager/PanelDrag.cpp)
no permite asumir soporte asíncrono del Shell. No escribir directamente en el destino mediante un
protocolo específico de 7-Zip sin demostrar confirmación, resultado, conflictos y cancelación.
Fuentes: [formatos del Shell](https://learn.microsoft.com/en-us/windows/win32/shell/clipboard),
[transferencia asíncrona](https://learn.microsoft.com/en-us/windows/win32/api/shldisp/nn-shldisp-idataobjectasynccapability),
[FAQ oficial de 7-Zip](https://d.7-zip.org/faq.html).

Clippy analiza código Rust; no implementa portapapeles ni arrastre. Mantenerlo en el gate, y
auditar el portapapeles como flujo relacionado pero independiente, sin prometer que una biblioteca
existente resuelva la duración de los temporales. Evaluar implementaciones existentes y licencias
compatibles; incorporar dependencias solo con mejora demostrada y sin regresiones de rendimiento.

## Bloques de ejecución, en orden

### 1. Fijar el destino y proteger datos

- [ ] Diseñar una instantánea inmutable del destino asociado al momento de soltar: panel, ruta o
  carpeta bajo el cursor, geometría y versión de la vista. Debe cubrir cambios entre el evento
  OLE y su procesamiento por UI, además de cambios durante la extracción; no basta con resolver
  coordenadas cuando termina el staging.
- [ ] El destino puede conservar la ruta elegida o rechazar la operación si ya no es válido; no
  redirigir silenciosamente al panel activo. Confirmar siempre la ruta real antes de escribir.
- [ ] Evitar el efecto `MOVE` prematuro y respetar efectos permitidos, modificadores y confirmación;
  mantener origen y destino intactos en cancelaciones, errores y conflictos no resueltos.
- [ ] Probar cambios de carpeta, cierre/reordenamiento de paneles y ventanas, redimensionamiento,
  carpetas bajo cursor y dos drops simultáneos. Cubrir que cancelar no deje parciales ni staging.

### 2. Recepción interoperable

- [ ] Integrar selectivamente el receptor experimental con COM/OLE y el modelo de ventanas de
  Slint; validar vida de objetos, marshalling y salida limpia del worker, sin E/S en UI.
- [ ] Para rutas persistentes, usar el motor habitual; para temporales de 7-Zip, asegurar la vida
  de todo el árbol antes de que el origen lo retire. Preservar carpetas vacías y rechazar rutas o
  enlaces inseguros. No depender del nombre del directorio temporal como prueba de propiedad.
- [ ] Para objetos virtuales negociar capacidades reales y materializar de forma acotada. Mantener
  cancelación, progreso, colisiones e historial; si un origen no ofrece garantías suficientes,
  fallar de forma explícita y segura en vez de anunciar éxito o copiar parcialmente en silencio.
- [ ] Hacer visible la preparación cancelable sin modificar la geometría usada para seleccionar
  el destino. Medir tiempo de respuesta, memoria, descriptores y coste de staging en árboles
  pequeños y grandes, incluidos tiempos de `GetData` bloqueante y cancelación durante COM.
- [ ] Revisar arrastres internos de Naygo y portapapeles copiar/cortar/pegar para no degradarlos;
  no convertir el arrastre en portapapeles ni aplicar supuestos entre ambos mecanismos.

### 3. Matriz de aceptación Windows

- [ ] Probar realmente con Explorer, Directory Opus y 7-Zip instalado; incluir otra versión de
  7-Zip si está disponible. Identificar versiones y formatos realmente ofrecidos, sin registrar
  rutas ni contenido privado. No afirmar compatibilidad universal con programas no probados.
- [ ] Casos: archivos, carpetas, mezcla, vacías/anidadas, Unicode, archivos grandes y muchos
  pequeños; mismo disco y distinto disco, USB/red disponible, ruta larga, acceso denegado y destino
  que desaparece. Cubrir archivos virtuales con origen sintético y aplicaciones disponibles.
- [ ] Copiar/mover, Ctrl/Shift, fuentes copy-only/move-only, cancelar antes y durante, conflictos,
  repeticiones, caída del origen y actividad simultánea. Verificar contenido y estructura con
  hashes, ausencia de pérdidas/duplicados y limpieza; no usar archivos personales.
- [ ] Pruebas automatizadas representativas, gate aislado, CI Windows actualizado y evidencia de
  pruebas reales. Si no hay sesión interactiva disponible, dejar la aceptación explícitamente
  pendiente; no repetir ciclos manuales sin instrumentación ni pedir datos reales al usuario.

### 4. Cierre visual y documentación

- [ ] Validar en la instalación real paneles únicos y duplicados heredados, ubicación tras cerrar,
  reiniciar y cambiar el layout, autocierre y errores; revisar foco y apariencia en temas claros,
  oscuros y contrastantes. No eliminar disposiciones o preferencias existentes.
- [ ] Evaluar visualmente órbita respecto a cámara, colores, modos sólido/bordes/malla y colores
  base 3MF ya implementados; medir en equipos modestos. Texturas y materiales extendidos son una
  capacidad adicional, no declarar soporte integral; no agregarlos sin evaluar coste y alcance.
- [ ] Actualizar README y capturas reales con datos sintéticos y sin rutas privadas; registrar qué
  fue probado y mantener en backlog lo no aceptado. Consolidar entradas históricas contradictorias
  sin borrar evidencia. Comprobar preview de texto y límites configurables en las otras máquinas
  cuando exista acceso, sin asumir que se validó un sistema remoto.

### 5. Entrega y pendientes externos

- [ ] Unificar en la rama de avance, actualizar grafo, ejecutar pruebas, Clippy y formato; repetir
  CI Windows. Ejecutar el build de distribución solo, con supervisor, verificar hashes, contenido,
  PDB opcional y que ZIP/EXE instalado coincidan. Respaldar configuración, instalar, arrancar y
  verificar respuesta y datos preservados; dejar setup en `dist/` y subir commits/documentación.
- [ ] Firma: continuar preparación y verificar requisitos de SignPath OSS; la aprobación de la
  organización, credenciales y la primera firma real dependen del servicio y de Nicolás. No
  afirmar firma ni publicación antes de verificarlas.
- [ ] Linux Ubuntu 26.04 LTS x86_64, bandeja GLib y sus implicancias de licencia: conservar el
  plan y la evaluación aprobada, pero continuar tras Windows; corregir CI Linux y probar escritorio
  real antes de anunciar una alfa. La política GPL Linux aprobada no cambia la licencia Windows.

## Criterio de cierre

La entrega queda lista cuando la matriz Windows acordada pasa, la aplicación permanece sensible,
no hay pérdida ni destino incorrecto en los escenarios de fallo, los resultados se reflejan con
honestidad en la UI y el instalador verificado queda disponible. Registrar limitaciones concretas;
si alguna fuente no puede garantizar una transferencia segura, conservar el rechazo explícito y
no declarar compatibilidad completa. Estimación en bloques, no promesa de calendario: captura de
destino y seguridad 1–3 sesiones, integración y matriz 2–5 sesiones, cierre visual y distribución
1–2 sesiones; pruebas interactivas, rendimiento y compilación pueden extender esos rangos.
