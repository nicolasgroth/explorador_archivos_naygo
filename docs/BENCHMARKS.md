# Benchmarks de rendimiento

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.  
SPDX-License-Identifier: MIT

La línea base reproducible del modelo se ejecuta con:

```powershell
cargo bench -p naygo-core --bench large_directory
```

Genera en memoria directorios de 10.000 y 100.000 entradas y mide construcción de vista,
aciertos del caché, orden por tamaño/fecha y filtro por nombre. No depende del disco, por lo que
sirve para detectar regresiones entre commits y comparar CPU/VM.

Para cada medición de interfaz anotar además: commit, Windows, CPU/vCPU, RAM, tipo de disco,
resolución/DPI y si es VM. Usar una carpeta local y una ruta UNC con 10.000 entradas; registrar
tiempo hasta la primera fila, tiempo hasta completar, pico de memoria y CPU en reposo durante
30 segundos. La ruta UNC debe probarse también desconectada: la ventana debe seguir respondiendo
y permitir cancelar. Ejecutar tres veces y conservar la mediana.

El benchmark no establece umbrales universales entre equipos. En CI debe compararse contra la
mediana de la misma máquina y marcar regresión si aumenta más de 15 % en dos ejecuciones seguidas.
