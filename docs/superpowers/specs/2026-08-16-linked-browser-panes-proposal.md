# Paneles Árbol + Archivos enlazados — diseño implementado

Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.  
SPDX-License-Identifier: MIT

## Decisión

No crear un cuarto tipo de panel que duplique Árbol y Archivos. Mantener ambos paneles actuales y
agregar una relación opcional entre ellos:

- **Árbol común:** conserva el comportamiento actual y sigue al último panel Files activo.
- **Árbol dedicado:** apunta a un `PaneId` Files concreto y no cambia al activar otros paneles.
- **Explorador enlazado:** una acción crea Árbol + Files dentro de un grupo visual común y fija la
  relación entre ambos. Se pueden desacoplar sin destruir ninguno.

## Modelo mínimo

Persistir en el workspace una colección ordenada `tree_pane → files_pane`. Un
árbol sin enlace usa `LastActiveFiles`; uno enlazado usa `Fixed(PaneId)`. El layout sigue siendo el
dock existente: el enlace es semántico y no introduce un contenedor de negocio nuevo.

## Comportamiento

- Navegar desde un árbol dedicado solo modifica su Files asociado.
- Navegar en el Files asociado revela la ruta en ese árbol.
- El árbol común continúa siguiendo el Files usado más recientemente.
- Cerrar un lado conserva el otro ya desacoplado; nunca cierra dos paneles por sorpresa.
- Duplicar un grupo crea IDs nuevos; no comparte accidentalmente el enlace original.
- La carga de ramas sigue siendo lazy y cancelable, por lo que un árbol dedicado no penaliza a
  quienes no lo usan.

## Tratamiento visual

Usar una delgada franja de acento compartida y una insignia discreta en el árbol. El indicador
`○ Árbol común` / `● ↔ <panel>` funciona además como alternador: enlaza al último Files activo o
desenlaza. La acción “Explorador enlazado (Árbol + Archivos)” crea un split interno 28/72 que no
se aplana con la fila exterior, por lo que la pareja conserva su agrupación visual.

## Entrega

Implementado en 0.4.0 con migración por `#[serde(default)]` para sesiones y plantillas anteriores,
limpieza automática de enlaces al cerrar paneles y pruebas de navegación, persistencia, plantillas
y geometría del grupo visual.
