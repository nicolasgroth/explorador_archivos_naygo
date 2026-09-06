# Recetas de operaciones — Naygo 0.5.0

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.

SPDX-License-Identifier: MIT

Una receta reúne archivos, aplica criterios y prepara una entrega nueva. No es un script,
una tarea programada ni una operación que se ejecute al abrir un documento.

## Ejemplo: PDF del mes

1. Abre **Disposiciones → Recetas de operaciones**, o busca «Recetas» en Ctrl+P.
2. Da un nombre, elige **Consulta en carpetas** e indica raíces absolutas, una por línea.
   También puedes importar los criterios de una consulta `.naygosearch` existente.
3. Usa `*.pdf`, activa comodines y selecciona **Este mes**. Puedes limitar tamaños en bytes
   o buscar texto; el contenido se busca como texto, no mediante extracción de PDF/OCR.
4. Indica la carpeta de destino y un nombre como `PDF-{date}`. Elige carpeta nueva o ZIP
   y estructura plana, por grupos o relativa a una sola raíz explícita.
5. **Preparar y revisar** evalúa los criterios y congela las fuentes y nombres propuestos.
   Revisa la correspondencia origen → destino. Si modificas cualquier opción, debes preparar
   nuevamente. **Ejecutar receta** inicia la copia y muestra el resultado en Operaciones.

`{date}` es el único token: fecha local `AAAA-MM-DD` congelada al preparar. «Este mes» comienza
el primer día del mes local, considerando el horario de verano de esa fecha. Las consultas
guardadas también tienen este filtro.

## Selección, persistencia y privacidad

- El modo selección captura los archivos marcados del panel o de la bandeja al abrir el editor.
  Sólo admite archivos normales; para reunir carpetas recursivamente usa una consulta.
- `.naygorecipe` es JSON versionado y acotado a 256 KiB. Contiene criterios y opciones, no las
  rutas raíz/destino ni el inventario de una ejecución. Los criterios sí pueden contener texto
  sensible: revísalos antes de compartir el archivo.
- Abrir/guardar no busca ni copia. Actualizar exige que la revisión del archivo no haya cambiado;
  Guardar como no sobrescribe documentos existentes. «Olvidar» quita la referencia, no el archivo.
- Los espacios `.naygospace` conservan referencias a recetas para Ctrl+P. Guardar el espacio es
  explícito; no existe catálogo residente ni autoguardado global de recetas.

## Límites y garantías

- Hasta 64 raíces y 5.000 archivos seleccionados. Las consultas reutilizan el límite del buscador:
  si alcanzan 5.000 resultados o 100.000 directorios, la preparación se rechaza como incompleta.
  También se bloquea ante raíces ilegibles o contenido omitido por límites/errores. No se siguen
  enlaces descubiertos durante el recorrido; las fuentes de entrega no admiten enlaces/reparse.
- Buscar contenido usa el lector existente, hasta 16 MiB por archivo. No añade OCR, indexación
  residente, nuevas dependencias ni ejecución de shell.
- La revisión muestra como máximo 500 correspondencias; el manifiesto final incluye el inventario
  completo. En modo plano los homónimos bloquean salvo que se pida proponer sufijos numerados.
- La ejecución sólo utiliza las fuentes revisadas; archivos aparecidos después no se incorporan.
  Se revalidan las fuentes y el destino. SHA-256 opcional permite verificar contenido además de
  tamaño/fecha. Ninguna receta mueve o borra originales.
- Se publica únicamente una carpeta o ZIP nuevo. Si ya existe, cambia el nombre y prepara otra vez;
  no hay fusión, reemplazo silencioso ni duplicación automática al reejecutar.
- Cancelar durante preparación no escribe una entrega. Durante copia se cancela desde Operaciones;
  el motor limpia su área temporal y no publica resultados parciales. Errores de limpieza se
  comunican por el motor existente. I/O del sistema puede tardar en responder a cancelación.
- El historial identifica la receta y su resultado. Deshacer se limita al destino nuevo mediante
  las comprobaciones del motor de entregas; no es una transacción entre múltiples volúmenes.

La validación visual instalada (teclado, tamaños de ventana, DPI, Narrador y red real) se mantiene
separada de las pruebas automáticas. ES/EN completos; ocho catálogos conservan provisionalmente
los nuevos textos en inglés hasta su revisión editorial.
