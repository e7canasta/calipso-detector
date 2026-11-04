Estado: vigente. Fecha: 2026-11-04. Dueño: master.

# `calipso.verify.v1` — el verificador lento, por NATS

El protocolo con el que la cáscara del detector le pide al verificador lento
(un modelo de lenguaje–visión local) una respuesta a una pregunta cerrada
sobre un recorte de la cama. Es request/reply de NATS: una petición, una
respuesta; la imagen no se escribe en ningún lado (regla 7) — el recorte va y
se suelta. El motor nunca ve el cable: entrega el pedido a su puerto de
verificación y después recibe `Verified`; las formas de la petición y de la
respuesta viven en `detector/src/verifier/verify_wire.rs`, con sus pruebas de
ida y vuelta.

## Quién es quién

- **El que pregunta**: la cáscara del detector, su actor de verificación
  (`NatsVerifier`). Corta la cama en memoria, arma la petición y sigue sin
  bloquear el turno; las respuestas vuelven al monitor antes de la muestra
  siguiente.
- **El que responde**: el servicio del verificador lento, detrás de NATS
  (`calipso-verifier`). Mira el recorte y contesta; puede no contestar.
- **El transporte**: NATS request/reply sobre el sujeto configurado
  (`[verify].subject`), con un pedido por vez.

## Cabeceras de la petición

Toda petición lleva:

| Cabecera | Qué dice |
| --- | --- |
| `Calipso-Camera` | la cámara que pregunta |
| `Calipso-Stream` | el prefijo del stream (la instalación) |
| `Calipso-Question` | la pregunta, con su id versionado: `held_in_bed@1`, `sitting_in_bed@1` o `leaving_bed@1` |
| `Calipso-Ticket` | el id del pedido; la respuesta se aplica a ese ticket |
| `Calipso-Bed` | las cuatro esquinas de la cama en píxeles del recorte, `x,y x,y x,y x,y` (cabecera izquierda, cabecera derecha, pie derecho, pie izquierdo); el verificador puede rectificar con ellas |
| `Calipso-Looked` | la muestra mirada, `sesión:secuencia:segundos` |

Con ráfaga (varios recortes de una duda), dos más:

| Cabecera | Qué dice |
| --- | --- |
| `Calipso-Frames` | cuántos JPEGs van |
| `Calipso-Frame-Lengths` | la longitud en bytes de cada uno, en su orden, separadas por comas |

Un solo recorte va solo y sin estas dos cabeceras, como antes de las ráfagas.

## Payload

El recorte de la región pedida sobre el cuadro mirado, un JPEG en memoria. En
ráfaga, los recortes guardados para el ticket, uno tras otro, en el orden en
que se guardaron; las longitudes de `Calipso-Frame-Lengths` parten esos bytes
en los N JPEGs.

## Respuesta

- El texto de la respuesta es JSON con un campo `answer`, o la palabra suelta.
- Se lee su **primera palabra alfabética**, en minúsculas: `yes`, `si` o `sí`
  es afirmativo; `no` es negativo; **todo lo demás es «no sé»** (`unsure`).
- **Sin respuesta** dentro del plazo no es un «no sé»: el pedido expira
  (`Verified { answer: None }`) y el motor lo toma como expiración del pedido,
  igual que su propio plazo de tiempo fuente. Un verificador que no sabe,
  responde.
