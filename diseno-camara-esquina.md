# Diseño: capa 0 y capa 1 con el cuerpo en coordenadas de la cama

Rama `camara-esquina`. Parte de las fallas medidas en `notas-camara-esquina.md`
y en el loop de `dig1` (2026-10-28, `calipso/data/camara-esquina/`). La
pregunta: con la cama en diagonal, ¿con qué se decide ocupación (capa 0) y
postura en cama (capa 1)?

## El diagnóstico geométrico

Hoy las dos capas preguntan "¿este punto cae en este polígono?" con puntos que
no representan donde está el cuerpo:

1. **El centro de la caja no es el cuerpo.** La caja está alineada a la imagen;
   con el cuerpo en diagonal, su centro cae fuera del cuerpo (en el loop de
   `dig1`, una persona acostada en el borde contó como "en el piso").
2. **La homografía de la cama vale solo para el plano del colchón.**
   `BedPlane` lleva puntos de la imagen al plano del colchón. Un punto que no
   esta en ese plano se proyecta mal, por paralaje, a lo largo del rayo de la
   cámara:
   - la cabeza sobre la almohada (~15 cm sobre el colchón) cae más allá de la
     cabecera: los ~80 px de `live3`;
   - las caderas de alguien parado (~1 m sobre el piso) caen sobre el colchón
     en la imagen: la persona parada del lado cercano "está en la cama";
   - los pies están en el **piso**, ~50 cm debajo del colchón: tampoco son del
     plano del colchón.
   Con la cámara de frente y alta el error es chico y cae a lo largo de la cama;
   en esquina cae en diagonal, y cruza los costados.
3. **La retención no sabe salir por el costado.** Con alguien retenido en la
   cama, un cuerpo nuevo en el piso junto a la cama es "entró alguien"
   (`conservation::settle`, regla 4): una sola persona en el borde da
   `accompanied`, que en el Hive es personal presente y **no alerta**.
4. **El detector no ve el cuerpo acostado en diagonal** (t=36-43 s del loop):
   cuerpos rotados ~45° son raros en el entrenamiento.

## La idea: dos planos y el cuerpo por partes

Cada punto se juzga **en el plano donde está**, no en el de la imagen:

| Plano | Que puntos | Cómo se define |
|---|---|---|
| Colchón | hombros, caderas, cabeza acostada (con margen de paralaje) | `map.bed`, como hoy |
| Piso | tobillos (y el borde inferior de la caja si no hay tobillos) | `map.bed_floor`: los 4 puntos donde la cama toca el piso, dibujados como la cama |

`bed_floor` es la huella de la cama en el piso. Con las dos homografías
(`BedPlane` de cada una) un tobillo se lleva a `(u, v)` del piso y un hombro a
`(u, v)` del colchón, y los dos se comparan con el mismo cuadrado `[0, 1]²`.
Es geometría proyectiva exacta para puntos de cada plano; no hace falta
calibrar la cámara.

### Capa 0: ocupación por partes, no por caja

El cuerpo se lee por sus apoyos:

| Torso (hombros+caderas) en el colchón | Tobillos en la huella | Lectura |
|---|---|---|
| dentro | dentro o no visibles | en la cama |
| dentro (caderas en el borde, `u` cerca de 0 o 1) | fuera | sentado en el borde: **en la cama**, la capa 1 dice borde |
| fuera | fuera | en el piso |
| fuera | dentro | no fisico (parado sobre la huella): en el piso, cerca de la cama |

Sin keypoints (pose no disponible o cuerpo lejos), se cae a la regla de hoy:
el centro de la caja, con `bed_frame` si está.

Para tener keypoints de **todos** los cuerpos cerca de la cama, no solo del
sujeto: el modelo de pose ya es un detector de personas. En foco (residente
solo en la cama) el detector principal puede ser el de pose sobre el recorte
del foco: una pasada da cajas y keypoints. Medir si su recall de gente
acostada es comparable al de `yolo26s` antes de reemplazarlo; si no, pose
solo sobre las cajas cerca de la cama.

### Retención: salir por el costado

Regla nueva en `settle`: con alguien retenido, un cuerpo que aparece en el
piso **junto a la cama** (no desde la puerta: `from_door` ya lo distingue) es
el retenido que se levantó. Movimiento nuevo `HeldLeft` (retenido -> piso),
narrado como "se levantó". La regla 4 queda: un retenido no sale **sin ser
visto**; ahora puede ser visto saliendo al piso junto a la cama.

### Capa 1: postura en las coordenadas de la cama

Lo que hoy pregunta la capa 1, en `(u, v)` del colchón:

- **Cabecera derivada, no dibujada:** la franja `v >= 1 - h` (h ~ 0.2 del
  largo) con margen de paralaje hacia afuera de la cabecera (`v` hasta
  `1 + p`). `p` se mide una vez por cámara con alguien acostado. La cabecera
  dibujada queda como opción para camas raras.
- **Eje del cuerpo contra el eje de la cama:** el vector caderas -> hombros en
  `(u, v)`. Acostado: alineado con `v`, hombros hacia la cabecera. Atravesado
  en la cama: alineado con `u` (hoy se lee "intentando salir").
- **Sentado contra acostado:** el torso vertical se escorza en el plano del
  colchón y sus puntos quedan "levantados" (paralaje hacia la camara); se
  combina con la disparidad de profundidad que ya existe. Medir sobre el loop
  cuál separa mejor.
- **Hacia afuera:** tobillos fuera de la huella del piso con caderas en el
  colchón = borde. Brazos fuera del colchón no cuentan solos como salir.

### El cuerpo no visto en diagonal

Cuando hay alguien retenido en la cama, el refuerzo mira la cama
**enderezada**: el recorte se rectifica con `BedPlane` (el verificador ya
rectifica) para que la cama quede vertical; un cuerpo acostado en diagonal
llega acostado derecho. Y se reintenta cada N segundos mientras dure la
retención, no una vez por pérdida.

### Segmentación

Una máscara de persona (`yolo26*-seg`, una pasada da caja y máscara) mide la
fracción del cuerpo en el colchón sin depender de que se vean las
articulaciones. Pero tiene el mismo paralaje: una máscara de alguien parado
cae sobre el colchón. Útil como evidencia secundaria (cuerpo tapado por la
sábana, sin keypoints), no como base. Queda para después de medir lo anterior.

## Plan

0. **Oráculo del loop de `dig1`:** una línea por segundo con lo que pasa de
   verdad (en la cama / borde / piso / acompañado; acostado / sentado), sacada
   de los cuadros. Métrica: segundos bien leídos por capa. Base: la rama hoy.
1. **Retención por el costado** (`HeldLeft`). Chico, testeable con las reglas
   de conservación. Ataca la falla de seguridad.
2. **Huella en el piso y cuerpo por partes en capa 0** (`map.bed_floor`,
   apoyos con keypoints, caída a la regla de hoy sin keypoints).
3. **Capa 1 en `(u, v)`:** cabecera derivada con margen de paralaje, eje del
   cuerpo, borde por tobillos.
4. **Refuerzo enderezado y reintentado** mientras hay retención.
5. **Segmentación**, si los pasos anteriores dejan huecos medidos.

Cada paso se compara contra el oráculo, y la vista de frente contra
`calipso-golden` y `live4`/`live-cliente`: sin las claves nuevas, el
comportamiento de hoy.
