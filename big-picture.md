# Calipso — Big picture

Actualizado al **2026-10-11**. Una página para ubicarse: qué es el sistema, cómo
está partido, qué está verificado y dónde mirar. La arquitectura completa, en
4+1 vistas con C4 y UML, está en [arquitectura-c4.md](arquitectura-c4.md); el
detalle, en los documentos enlazados.

## Qué es

Un **gemelo digital de la habitación** para el monitoreo de cama. Una cámara fija
mira la habitación; el motor mantiene una **creencia** sobre quién hay y dónde,
y la publica muestra a muestra con su procedencia. No ve la habitación entera ni
siempre: por eso decide **qué mirar** según lo que cree. Es un **lazo de
percepción activa** ([libro 1](libro/00-prologo.md)) y es **determinista**: con
las mismas lecturas da las mismas escenas, así que una noche se repite sin
cámara.

## El lazo, en dos niveles

```mermaid
flowchart TB
    subgraph N1["Nivel 1 · dónde está (fundamentos)"]
        D["detector principal"] --> R["refuerzo, una vez por pérdida"]
        R --> F["caras en la cabecera"]
        F --> C["creencia por zonas: cama y afuera\nvacía · acompañado · acostado · junto a la cama · retenido"]
    end
    C -->|"solo lectura"| N2
    subgraph N2["Nivel 2 · cómo está (refinamiento)"]
        P["pose cuando se mueve o vence"] --> Q["profundidad en cada punto"]
        Q --> L["acostado de verdad\n(plano de la cama)"]
        Q --> S["postura: acostado o erguido\n(disparidad relativa)"]
    end
    C --> E["scene.v4 · events.v4"]
    L --> E
    S --> E
```

- **Nivel 1** es lo que no se negocia: la ocupación por zonas con seis reglas de
  conservación. Un cero no es una ausencia, la cama solo gana a quien vino de
  afuera y un retenido no sale sin reaparecer ([memoria funcional](../specs/r4/00-memoria-funcional.md),
  R4–R6).
- **Nivel 2** lee al primero y nunca lo escribe: si está acostado de verdad, en
  qué postura está y la profundidad de cada punto del cuerpo
  ([libro 2](docs/libro-2/00-prologo.md), R8–R9).
- **El motor hace una cosa y bien** (R9-5): publica hechos por muestra. Suavizar
  en el tiempo, confirmar y avisar es de los consumidores.

## Núcleo y cáscara

| | Núcleo (`scene-core`) | Cáscara (`ingest`, `sense`, `detector`) |
| --- | --- | --- |
| Qué | Creer y decidir qué mirar | Recibir video, mirar con modelos, publicar |
| IO | Ninguno (regla 8) | RTSP, ONNX, NATS |
| Prueba | Gherkin con sentidos guionados y mutaciones; escenas golden | Gherkin `@cascara` con modelos guionados; corridas medidas |
| Costura | `handle(Input)` y `drive`: el núcleo pide con `&Probe`, la cáscara responde con una `Reading` | |

Fuera de los dos quedan las herramientas, que leen solo el contrato publicado:
`calipso-rerun-bridge` (dibujar), `calipso-shark` (grabar y medir) y
`calipso-golden` (escenas y replay sin cámara).

Se trabaja siempre en ese orden: primero la política en el núcleo, con su
escenario en rojo; después la capacidad en la cáscara.

## La cadena de la POC

```mermaid
flowchart LR
    CAM["cámara RTSP\n(go2rtc)"] --> CD["calipso-detector\nscene.v4 · events.v4"]
    CD -.->|"adaptador por migrar"| PB["perception-bridge\n(confirma y avisa)"]
    PB --> HV["perception.observation.v1.{bed}\nSTANDING · IN_BED · OUT_OF_ROOM · …"]
    CD -.->|"adaptador por migrar"| SA["scene-analytics\n(análisis por consenso)"]
    SA --> OB["observations.v1"]
    CD --> RB["rerun-bridge"]
    OB -.-> RB
```

Reemplaza a la cadena anterior (`rtsp-person-detector` + `scene.v1`) con los
mismos consumidores, que leían `scene.v3` a través de un adaptador
(`scenev3`, proyecto 22). La recibe el que **avisa**, perception-bridge, que lee
la escena directamente. scene-analytics queda como capa de análisis y
comparación.

**Abierto:** el motor publica solo v4 desde el 2026-10-11
([retirar v3](adr/2026-10-11-retirar-v3.md)). Antes de la próxima corrida en
vivo hay que llevar a v4 los adaptadores de perception-bridge y
scene-analytics; rerun-bridge ya lee v4.

## Qué está verificado

| Qué | Cómo |
| --- | --- |
| Nivel 1 | 122 escenarios Gherkin del núcleo en `scripts/check.sh`, cada regla con un guardián que se prueba rompiéndolo (`scripts/check-guardians.sh`), mutaciones, propiedades `proptest`, escenas golden S1–S8 y la noche real de sample1 repetida por el detector |
| Nivel 2 | 901–904: escenarios con figuras del cuerpo y mutaciones; medido en vivo sobre sample1 (912) |
| Cáscara | 54 escenarios `@cascara`; latencia medida con los cinco sentidos (p90 de la vuelta de 235 a 477 ms contra 1000) |
| Cadena de la POC | Replay de la noche de sample1: 327 escenas → 327 observaciones; perception-bridge da un `STANDING` y un `IN_BED`. En vivo: **pendiente** |

## El mapa de proyectos (calipso-roadmap)

| Proyecto | Qué dejó |
| --- | --- |
| 15 | El lazo y el motor R3 (el libro 1) |
| 16–17 | La infraestructura y la escena v3 en vivo (RTSP → NATS → Rerun) |
| 18 | R5: la creencia no inventa a nadie |
| 19 | R6: refuerzo y caras (política y capacidad) |
| R7 | El motor de replay: escenas golden y destilado de noches |
| 20 | R8: pose, profundidad y calibración (política y capacidad) |
| 21 | R9: el segundo nivel (acostado de verdad, postura, Rerun, medición) |
| 22 | La escena v3 hacia afuera: los consumidores de la POC leen `scene.v3` |
| — | El contrato v4 y el retiro de v3; la vuelta en una sola llamada (ADR del 2026-10-11) |

## Dónde mirar

- **Para trabajar con la dirección médica:** el [workshop](workshop-director/00-guia.md).
- **Qué hace el sistema, para el cuidado:** [documento funcional de la POC v3](funcional-poc-v3.md)
  y [escenarios para el directorio](directorio-escenarios.md).
- **Cómo está construido:** [arquitectura en 4+1 vistas](arquitectura-c4.md) y el
  [DESIGN del motor](../calipso-detector/crates/scene-core/DESIGN.md).
- **Por qué es así:** [libro 1](libro/00-prologo.md) y [libro 2](docs/libro-2/00-prologo.md).
- **Qué está hecho y las decisiones:** [specs](../specs/README.md) (R4–R9) y los ADR en [`docs/adr/`](adr).
- **Cómo levantar la POC:** `docs/RUNBOOK-SCENE-V3.md` en nats-event-recorder (rama `feat/scene-v3`).
- **Dónde quedamos:** `calipso-roadmap/HANDOFF-2026-10-10.md`.
