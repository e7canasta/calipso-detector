# Arquitectura: 4+1 vistas, con C4 y UML

Revisión del código al **2026-10-11**: contrato v4, protocolo `drive`/`cancel`.
Describe lo implementado; lo previsto se marca como tal. Si este documento y el
código no coinciden, uno de los dos tiene un error: la referencia del motor es
[`scene-core/DESIGN.md`](../calipso-detector/crates/scene-core/DESIGN.md), que
se compila con `cargo doc`.

El documento sigue las **4+1 vistas** de Kruchten (1995). Cada vista responde
una pregunta para alguien, y los niveles de **C4** (Brown) se ubican en la vista
a la que pertenecen. Los diagramas son Mermaid: `flowchart` para C4 (más estable
que el `C4Context` experimental) y los diagramas UML nativos (`classDiagram`,
`stateDiagram`, `sequenceDiagram`).

| Vista | Pregunta | Para quién | Diagramas |
| --- | --- | --- | --- |
| [Contexto](#0-contexto-c1) | ¿Qué es y con quién habla? | todos | C1 |
| [Lógica](#1-vista-lógica) | ¿Qué piezas tiene el motor y qué se dicen? | quien diseña o revisa | C3, clases UML, estados UML |
| [Procesos](#2-vista-de-procesos) | ¿Qué corre a la vez y cómo se apaga? | quien opera o integra | C2, secuencia UML |
| [Desarrollo](#3-vista-de-desarrollo) | ¿Cómo está partido el código y qué lo vigila? | quien contribuye | paquetes |
| [Física](#4-vista-física) | ¿Dónde corre? | quien despliega | despliegue |
| [Escenarios (+1)](#5-escenarios-1) | ¿Cómo se sostienen las cuatro vistas en un caso real? | todos | secuencia UML |

## Resumen

El motor es una **biblioteca Rust con estado, determinista y sin IO**,
`scene-core`: una función de `(Policy, SiteMap, Input*, Reading*)`. La conduce
un **proceso por cámara**, `calipso-detector`, que recibe RTSP, decodifica bajo
demanda, mira con modelos ONNX y publica `calipso.scene.v4` y
`calipso.events.v4` en NATS. Los consumidores leen solo el contrato publicado.

El motor hace una cosa: **publica la escena y los eventos de cada muestra**,
como un pipe. Suavizar en el tiempo, confirmar y avisar es de quien consume.

Una crate es una unidad de código, no un contenedor C4. Ingesta, sentidos,
núcleo y publicación corren **en un mismo proceso**, en hilos con un dueño
cada uno. No son microservicios.

## 0. Contexto (C1)

```mermaid
flowchart TB
    care["Personal de cuidado<br/>[persona]"]
    maint["Mantenedor<br/>[persona]"]
    camera["Cámara fija<br/>[sistema externo]<br/>RTSP H.264"]
    engine["Motor Calipso<br/>[sistema de software]<br/>cree quién hay y dónde; decide qué mirar"]
    nats["NATS<br/>[sistema externo]"]
    product["Producto Calipso<br/>[sistema externo, previsto]<br/>confirma y avisa"]
    tools["Herramientas<br/>[sistemas de software]<br/>rerun-bridge · shark · golden"]

    camera -->|"video"| engine
    maint -->|"perfil TOML por cámara"| engine
    engine -->|"calipso.scene.v4 · calipso.events.v4"| nats
    nats -->|"consumo"| tools
    nats -.->|"consumo"| product
    product -.->|"situación y alertas"| care
```

El residente es la persona observada; el motor no identifica a nadie y no
guarda imágenes (regla 7). Las alertas son del producto que consume.

## 1. Vista lógica

### Componentes del núcleo (C3)

```mermaid
flowchart TB
    shell["Cáscara<br/>detector · replay · pruebas"]

    subgraph core["scene-core · sin IO"]
        cycle["cycle<br/>Monitor, Turn: la vuelta y su orden (Course::next)"]
        attention["attention<br/>qué más mirar; solo lee"]
        evidence["evidence<br/>qué lectura cuenta; cuerpos por zona"]
        belief["belief<br/>capa 0 (occupancy::Machine), sujeto, latch"]
        mapm["map<br/>geometría del sitio y cama aprendida"]
        sensor["sensor<br/>estado de recepción"]
        refine["refine<br/>nivel 2: acostado de verdad, postura"]
        conclusion["conclusion<br/>Observation o qué republicar"]
        chronicle["chronicle<br/>desde cuándo: el pliegue de las vueltas"]
        project["project<br/>la única traducción al contrato"]
        contract["contract<br/>common + v4: el lenguaje publicado"]
    end

    shell -->|"handle(Input)"| cycle
    cycle -->|"&Probe"| shell
    shell -->|"(Reading | Failure, Spent)"| cycle
    cycle -->|"Output { at, scene, events }"| shell

    cycle -->|"Glimpse + Policy"| attention
    cycle -->|"Reading"| evidence
    cycle -->|"observe · observe_body · reinforced"| belief
    cycle -->|"gate · learn · state"| mapm
    cycle -->|"report"| sensor
    cycle --> refine
    cycle --> conclusion
    conclusion --> chronicle
    conclusion --> project
    chronicle -->|"Chronicled"| project
    project --> contract
```

Las flechas son las únicas conversaciones permitidas:

- **`cycle` solo secuencia** (regla 11). Le pasa la `Policy` entera a quien
  decide y nunca pregunta qué sentido está configurado; `check.sh` lo vigila.
- **`attention` lee y nunca escribe** la creencia: recibe un `Glimpse` y
  devuelve `Consideration<T>`, que es `Ask(T)` o `Skip(reason)` y siempre va al
  ledger (regla 12).
- **El dominio no importa el contrato.** Solo `project` traduce, y `cycle`
  nombra el contrato porque `Output` lo lleva. La proyección común
  (`project/mod.rs`) nombra solo `contract::common`, para que una versión nueva
  la comparta sin depender de v4 (C2). Las dos cosas las vigila `check.sh`.
- **El motor no lee lo publicado** (P4). `chronicle` recuerda el dominio fuera
  del motor y `project` lo traduce.

Lo público de la biblioteca es `Monitor`, `Turn`, `Input`, `Output`, `contract`
(con `contract::v4`), `look` (`Probe`, `Reading`, `Failure`, `Spent`), `policy`,
`map` (`SiteMap`, `FrameInfo`) y `geom`. Todo lo demás es `pub(crate)`, y
`#![warn(unreachable_pub)]` impide que un `pub` diga más de lo que alcanza.

### Las clases que sostienen la vuelta (UML)

```mermaid
classDiagram
    direction LR
    class Monitor {
        -Engine engine
        -Chronicle chronicle
        -last: (SampleRef, Arc~Scene~)
        +new(Policy, SiteMap) Monitor
        +handle(Input) Turn
    }
    class Engine {
        -Policy policy
        -Map map
        -Belief belief
        -SensorMemory sensor
        -frame: FrameInfo
    }
    class Turn {
        -Course course
        -Phase phase
        -Open open
        +drive(look: FnMut(&Probe)) Output
        +cancel() Output
    }
    class Course {
        -monitor: &mut Monitor
        -SampleRef at
        -Geometry geometry
        -Attention attention
        next(Reached) Phase
        advance(Continuation, &Probe, Outcome) Phase
    }
    class Belief {
        -Machine occupancy
        -SubjectMemory subject
        -LatchState latch
        observe(at, &Admission, &Policy) Believed
        observe_body(at, Believed, Enrichment) Believed
        reinforced()
    }
    class Chronicle {
        record(Entry) Chronicled
    }
    class Output {
        +SampleRef at
        +Arc~Scene~ scene
        +Vec~Event~ events
    }

    Monitor *-- Engine
    Monitor *-- Chronicle
    Engine *-- Belief
    Turn *-- Course
    Course --> Monitor : préstamo mutable exclusivo
    Turn ..> Output : devuelve
```

- `Turn` toma prestado `&mut Monitor`: el compilador impide abrir dos vueltas a
  la vez.
- `Open` es la obligación de cerrar: solo `drive` la descarga. Una vuelta
  soltada sin cerrar dispara la aserción de `Drop` (regla 10).
- El shell no tiene nada que usar mal: recibe una `&Probe` y devuelve lo que
  vio. Una lectura de otra clase que la sonda cuenta como un sentido que falla
  (`Failure::Model`), no como un pánico (regla 21).

### Los estados de una vuelta (UML)

```mermaid
stateDiagram-v2
    [*] --> Waiting: Sample, empieza por Primary
    [*] --> Republish: NoFrame o Sensor
    state Waiting {
        Primary --> Widen: ¿el foco no confirma?
        Widen --> Reinforce
        Primary --> Reinforce: ¿pérdida en la cama?
        Primary --> Faces
        Reinforce --> Faces
        Faces --> Calibrate: tras creer
        Calibrate --> Pose
        Pose --> Depth
    }
    Waiting --> Concluded: sin más miradas
    Concluded --> [*]: Observation → chronicle → project
    Republish --> [*]: lo último, declarado (regla 2)
```

Cada espera lleva lo que la vuelta juntó hasta ahí (`Continuation`), así que no
se puede seguir con datos de menos. Cada mirada es opcional salvo la primaria y
la decide `attention`. El orden vive en un solo lugar:

```text
Primary(foco | entero) → [Widen] → [Reinforce] → [Faces] → believe → [Calibrate] → [Pose → [Depth]] → conclude
```

### La capa 0 por dentro

```text
Bodies ──► Span ──(Before, Seen)──► settle ──[Movement]──► Machine::apply ──► Transition
            cuándo cambió             qué lo explica         retenidos, sin explicar,
            (dwell en tiempo de       (las seis reglas)      hechos en orden
             fuente)
```

La máquina devuelve la transición como valor y no la emite (P2). Quien la
recibe no necesita preguntarle nada (P6). La cuenta publicada satisface:

```text
room.occupants = floor.seen + bed.seen + len(bed.held)
rbh            = (occupants, bed.seen + held, held)
```

## 2. Vista de procesos

### Contenedores y ejecución (C2)

```mermaid
flowchart LR
    camera["Cámara RTSP"]
    subgraph det["Proceso calipso-detector · uno por cámara"]
        ingest["Hilo ingest<br/>[std::thread]<br/>retina · solo keyframes"]
        mailbox[("Mailbox&lt;Sample&gt;<br/>un lugar: lo más fresco gana")]
        engine["Motor<br/>[spawn_blocking]<br/>Driver · RtspSource (decode) · Senses (ONNX) · Monitor"]
        handle["PublisherHandle<br/>numera escenas y eventos"]
        actor["Actor NATS<br/>[tarea tokio]<br/>FIFO de eventos (1024) · escena más nueva"]
    end
    nats["NATS"]

    camera -->|"RTSP"| ingest
    ingest -->|"Sample"| mailbox
    mailbox -->|"take"| engine
    ingest -->|"StreamEvent (mpsc)"| engine
    engine -->|"Output + Timing"| handle
    handle -->|"Publication (mpsc)"| actor
    actor -->|"{prefix}.scene.v4 · {prefix}.events.v4"| nats
```

- **Cada recurso nace y muere en su hilo** (`detector/src/main.rs`). Nada se
  comparte salvo el buzón y dos canales.
- **El buzón no encola.** Lo que el motor no toma a tiempo lo reemplaza la
  muestra siguiente, y solo se decodifica lo que el motor va a mirar.
- **El driver no sabe de dónde vienen las muestras ni quién mira.** Se le
  inyectan una `SampleSource` y un `SensesPort`. En vivo son `RtspSource` y
  `sense::Senses`; en `calipso-replay`, una escena golden y su mundo guionado:
  el mismo driver, el mismo núcleo y el mismo publicador.
- **El transporte se vuelve dominio en un solo lugar.** El driver traduce cada
  `StreamEvent` a `Input::Sensor` (conectando, transmitiendo, sin muestras,
  reconectando) y vigila el plazo sin muestras.
- **La escena nunca espera.** El actor reintenta el frente de la cola de eventos
  con id de mensaje NATS, y de la escena publica siempre la más nueva.
- **Prefijo:** `vision.<site>.<camera>`, salvo que el perfil lo cambie.

### El apagado (UML)

```mermaid
sequenceDiagram
    participant OS as SIGINT / SIGTERM
    participant M as main
    participant E as motor
    participant I as ingest
    participant A as actor NATS

    OS->>M: señal
    M->>M: token.cancel() · mailbox.close()
    M->>E: espera
    E->>E: la vuelta en curso termina con drive<br/>(o cancel: Failure::Cancelled en lo pendiente)
    E-->>M: Ok
    M->>I: join
    M->>A: espera (gracia de 3 s para vaciar)
    A-->>M: Ok
```

Ninguna vuelta queda abierta: un apagado a mitad de una mirada es un sentido
que falla, y el dominio ya sabe interpretarlo.

## 3. Vista de desarrollo

### Paquetes y dependencias

```mermaid
flowchart BT
    subgraph detector_repo["calipso-detector"]
        core["scene-core<br/>serde · schemars"]
        sense["sense<br/>ONNX (ultralytics-inference) · image"]
        ingest["ingest<br/>retina · ffmpeg · tokio<br/>sin dominio"]
        detector["detector<br/>tokio · async-nats · clap"]
    end
    subgraph golden_repo["calipso-golden"]
        golden["golden<br/>lenguaje de escenas, mundo, destilado"]
        replay["replay<br/>calipso-replay"]
    end
    subgraph consumers["consumidores del contrato"]
        bridge["calipso-rerun-bridge"]
        shark["calipso-shark<br/>trama · medir"]
    end

    sense --> core
    detector --> core
    detector --> sense
    detector --> ingest
    golden --> core
    replay --> detector
    replay --> golden
    bridge --> core
    shark --> core
    core -.->|"dev: escenas en las pruebas"| golden
    detector -.->|"dev"| golden
```

- `scene-core` depende solo de `serde` y `schemars`. Clippy prohíbe relojes,
  archivos, red e hilos (`clippy.toml`). `check.sh` verifica en el árbol de
  dependencias que no entren tokio ni async-nats.
- `ingest` no conoce el dominio y `sense` no conoce el transporte asíncrono;
  `check.sh` vigila los dos.
- Los consumidores usan `scene_core::contract::v4` para leer el contrato.
  Depender de la crate entera es deuda: un crate de solo tipos del cable (como
  las cabeceras `uapi` del kernel) los dejaría depender solo del lenguaje
  publicado (DESIGN §11).
- La flecha punteada es una dependencia de desarrollo: las pruebas del núcleo
  corren escenas golden. No hay ciclo en el código que se compila para
  producción.

### Lo que vigila el diseño

Cada regla de [`RULES.md`](../calipso-detector/RULES.md) tiene un guardián, y
cada guardián tiene una fixture que lo rompe a propósito
(`scripts/check-guardians.sh`): si el guardián no falla, el gate falla.

| Guardián | Regla o principio | Fixture |
| --- | --- | --- |
| Árbol de dependencias del núcleo sin IO | 8 | `core-depends-on-tokio` |
| Clippy: sin relojes ni hilos | 8, 9 | `core-reads-the-clock` |
| `#![deny(...)]`: sin `unwrap`, `expect` ni indexación | 21 | `core-panics` |
| `missing_docs` en la API pública | 13 | `core-undocumented` |
| Snapshots del schema y los ejemplos | 13 | `contract-changes-without-snapshot` |
| `cycle` no pregunta por la política | 11 | `cycle-consults-a-policy` |
| La proyección común no nombra una versión | C2 | `common-projection-names-a-version` |
| Pasos Gherkin sin definir | 15 | `undefined-step`, `undefined-step-shell` |

## 4. Vista física

```mermaid
flowchart LR
    subgraph room["Habitación"]
        cam["Cámara IP<br/>RTSP"]
    end
    subgraph edge["Nodo de borde · Linux"]
        relay["go2rtc<br/>(opcional)"]
        d1["calipso-detector<br/>cámara 1"]
        d2["calipso-detector<br/>cámara N"]
        models[("modelos ONNX<br/>perfil TOML")]
    end
    subgraph bus["Mensajería"]
        n["NATS"]
    end
    subgraph apps["Consumidores"]
        r["rerun-bridge + visor Rerun"]
        s["shark: grabar y medir"]
        p["producto (previsto)"]
    end

    cam --> relay --> d1
    relay --> d2
    models --- d1
    models --- d2
    d1 --> n
    d2 --> n
    n --> r
    n --> s
    n -.-> p
```

N cámaras son N procesos, cada uno con su perfil. No hay estado compartido
entre cámaras ni persistencia: la creencia empieza de cero con el proceso, y ni
una reconexión ni un cambio de resolución sueltan a nadie. El repositorio **no
tiene manifiesto de despliegue**: «un proceso por cámara» se verifica en
`main.rs`, no en una topología operativa.

## 5. Escenarios (+1)

Cada escenario atraviesa las cuatro vistas. Están escritos en Gherkin en
`features/` y corren en el gate.

### Una muestra con alguien en la cama que se pierde

```mermaid
sequenceDiagram
    participant D as Driver (cáscara)
    participant T as Turn (cycle)
    participant A as attention
    participant E as evidence
    participant B as belief
    participant P as chronicle + project

    D->>T: handle(Input::Sample)
    T-->>D: &Probe::Detect (imagen entera)
    D->>T: Reading::Bodies([]) · Spent
    T->>E: admit: ningún cuerpo por zona
    T->>A: ¿pérdida en la cama? (latch armado)
    A-->>T: Ask(cama agrandada)
    T-->>D: &Probe::Reinforce
    D->>T: Reading::Bodies([caja en la cama])
    T->>E: consolidate
    T->>A: ¿caras?
    T->>B: observe: estabiliza, conserva, deriva el sujeto
    T->>A: ¿calibrar? ¿pose? ¿profundidad?
    T->>B: observe_body(enriquecimiento)
    T->>P: Observation → Chronicled → escena y eventos v4
    T-->>D: Output { at, scene, events }
```

En las vistas: lógica (el orden de `Course::next`), procesos (todo ocurre en el
hilo del motor; las miradas son llamadas síncronas a `SensesPort::look`),
desarrollo (los tipos que cruzan son `Probe` y `Reading`, de `look`) y física
(un proceso, un modelo por rol).

### El decoder no entrega imagen

`Input::NoFrame` abre una vuelta que no pide nada: republica la última escena,
**declarada** (`republished.reason = decode_failed`), y lo que era actual pasa a
retenido, porque nada es actual sin una observación (regla 2). Si el que falla
es el detector principal, la vuelta conserva la creencia y la escena se
declara con `detect_failed`.

### Apagado a mitad de una mirada

La señal cancela el token. La vuelta en curso termina: lo pendiente responde
`Failure::Cancelled`, se publica la escena de esa muestra y el proceso cierra
(ver el apagado en la vista de procesos).

### Una noche sin cámara

`calipso-replay` corre una escena golden por el **mismo** driver, con una
`SampleSource` de tramos y un `SensesPort` guionado. Como el núcleo es
determinista, la misma noche da las mismas escenas, y `calipso-distill` vuelve
escena una noche publicada.

## Hallazgos y límites

| Tema | Evidencia | Consecuencia |
| --- | --- | --- |
| Consumidores de la POC | perception-bridge y scene-analytics leían v3 a través de un adaptador (proyecto 22); v3 se retiró el 2026-10-11 | **Abierto:** hay que verificar que lean v4 antes de la próxima corrida en vivo |
| Consumidores atados al núcleo | rerun-bridge y shark dependen de `scene-core` para leer el contrato | Deuda: un crate de tipos del cable (DESIGN §11) |
| `Input::Sensor` | lleva un `SampleRef` que el driver toma de la última muestra | Deuda: requiere que `observed_at` sea opcional en la próxima versión del contrato |
| `last_event_seq` | lo escribe el actor NATS, no el núcleo | Deuda de contrato (DESIGN §11) |
| Profundidad | `meters` y `reference_m` llevan la escala relativa del modelo | Se renombran en la próxima versión del contrato |
| Despliegue | sin manifiesto en el repositorio | La topología se describe acá; no se verifica |

## Verificación de esta revisión

`scripts/check.sh` en verde al 2026-10-11:

- formato y clippy con `-D warnings`;
- árboles de dependencias del núcleo, ingest, detector y sense;
- el dominio sin contrato y `cycle` que solo secuencia;
- 131 tests de unidad e integración;
- **122 escenarios Gherkin** del núcleo y **54** de la cáscara;
- licencias y advisories.

`scripts/check-guardians.sh`: los nueve guardianes fallan cuando deben. Los
`cargo check` de calipso-golden, calipso-rerun-bridge y calipso-shark compilan
contra el núcleo de hoy. Las pruebas con cámara, modelo o NATS reales están
marcadas `#[ignore]` y se corren a mano.

## Fuentes

- Motor: [DESIGN.md](../calipso-detector/crates/scene-core/DESIGN.md) ·
  [RULES.md](../calipso-detector/RULES.md) ·
  [protocolo](../calipso-detector/crates/scene-core/src/cycle.rs) ·
  [orden de la vuelta](../calipso-detector/crates/scene-core/src/cycle/sequence.rs) ·
  [la máquina](../calipso-detector/crates/scene-core/src/belief/occupancy/machine.rs)
- Cáscara: [composición](../calipso-detector/crates/detector/src/main.rs) ·
  [driver](../calipso-detector/crates/detector/src/driver.rs) ·
  [ingest](../calipso-detector/crates/ingest/src/lib.rs) ·
  [sentidos](../calipso-detector/crates/sense/src/lib.rs)
- Decisiones: [ADR](adr/), en especial
  [una sola forma de recorrer la vuelta](adr/2026-10-11-una-sola-forma-de-recorrer-la-vuelta.md)
  y [retirar v3](adr/2026-10-11-retirar-v3.md).
- P. Kruchten, *The 4+1 View Model of Architecture*, IEEE Software (1995).
  S. Brown, *The C4 model for visualising software architecture*.
