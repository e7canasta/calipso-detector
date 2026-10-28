# Camara de esquina: la cama en diagonal (2026-10-28)

Estado: rama `camara-esquina` (d1c3753). **No es para produccion.** La primera
prueba con una persona acostada mostro problemas que no son de zonas solamente.

## Lo que hace la rama

- `map.headboard` acepta cuatro puntos, como la cama. El motor (lectura de
  partes y lugar de la cabeza, capa 1) decide contra el poligono; la busqueda de
  caras y lo publicado en la escena siguen con su rectangulo.
- `[policy] bed_frame = <parte>`: cerca de la cama medido en las coordenadas de
  la cama, y la caja de la persona "en cama" por la parte de ella que cae en la
  cama (las esquinas de la caja de un cuerpo acostado en diagonal caen fuera).
- Sin esas claves el comportamiento es el de antes, cuenta por cuenta: 163
  tests y calipso-golden verdes.

## Lo que se probo

- `dig1` (camara de esquina real, 2 corridas de ~50 s, perfil
  `calipso-dist/config/live_dig1.toml`): parada junto a la cabecera, del lado
  lejano, quedo fuera de la cama; sentarse en el borde lejano leyo "sentada en
  la cama", no "en el borde".
- `live` con la cama girada a 45 grados (perfil `calipso-dist/config/live3.toml`),
  acostado y levantandose, 16:23-16:27 UTC. Grabacion:
  `calipso/data/camara-esquina/live3-2026-10-28.rrd`.

## Lo que fallo (live3-2026-10-28.rrd)

1. **Held tonto.** Al acostarse en diagonal el detector perdio el cuerpo y lo
   sostuvo (`held in bed`) 1 min 37 s, 16:23:34-16:25:11, sin verlo. El held
   tiene que ser mas inteligente: no puede sostener un minuto y medio a ciegas
   una postura que nadie ve. Revisar por que yolo26s con el foco no encuentra a
   alguien acostado en diagonal (evidencia `scene/evidence/bodies` en ese tramo).
2. **La cabeza cae fuera de la cama.** Acostado, la nariz estaba en ~(767, 202)
   y los ojos y orejas en y ~174; el borde de la cabecera de la cama dibujada
   pasa por y ~286 en esa x. La cabeza queda ~80 px mas alla de la cama y de la
   cabecera (que solo se estira un 5%). Con la camara alta, la cabeza sobre la
   almohada se proyecta mas alla del colchon. Partes: `face out of bed · head out
   of bed`.
3. **Acostado se lee "trying to leave".** De 4 min, solo 14 s leyeron `lying`
   (16:25:13-16:25:27). El resto: `trying to leave` con cabeza/cara fuera
   (punto 2) o con los brazos fuera de la cama (`0010100` en el grupo fuera).
   Con la cama en diagonal, los brazos a los costados pasan las barandas en la
   imagen.

## Para retomar

- Zonas: la cabecera tiene que cubrir donde se proyecta la cabeza acostada, no
  el colchon: medirla con una persona acostada, no con la cama vacia. Lo mismo
  el margen de los costados para los brazos.
- Held: acotar cuanto se sostiene sin ver, o pedir la vista completa/refuerzo
  antes de sostener.
- Lo que ya se habia visto con un solo frame y falta probar: una persona parada
  del lado cercano tapa el colchon y el centro de su caja cae en la cama; para
  alguien parado manda donde apoya los pies.
- Contrato: publicar el poligono de la cabecera (`headboard_area`) para que
  Rerun lo dibuje.
- El laboratorio: binario de la rama fuera de Docker, prefijo propio, frames de
  go2rtc con la escena del momento y lo inferido dibujado encima. Scripts y
  cuadros de esa sesion: `calipso/data/camara-esquina/`.
