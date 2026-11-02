# Onboarding — Trabajar con Calipso: motor, publicación, grabación y resumen

Guía de procedimiento para quien llega al proyecto. Explica la solución de
punta a punta y cómo **correrla, grabarla, verificarla y cambiarla**. Las
salidas de esta guía son de la **última verificación: 2026-10-11**, contra el
RTSP de `sample1`. Antes de confiar en una salida, volver a correr el comando.

> **Aviso (2026-11-01): este documento describe la estructura anterior al
> 2026-10-11.** Las secciones 3 (repositorio), 5 (compilar y verificar), 7 y 8
> (la grabación con `trama`) están en revisión: el crate `trama` no existe en
> ningún repo actual. Lo demás se corrigió mecánicamente; antes de seguir un
> comando, confirmarlo contra el checkout.

> Para el comportamiento en lenguaje del cuidado: [funcional](../../calipso-docs/funcional.md).
> Para las fronteras técnicas: [arquitectura C4](../arquitectura-c4.md). Para el
> diseño del núcleo: [`scene-core/DESIGN.md`](../crates/scene-core/DESIGN.md).

---

## 1. Qué hace Calipso, en una frase

> Mira una cámara de una habitación y dice **quién hay y quién sigue en la
> cama**, aunque deje de verlo (tapado, de espaldas), y publica cada cambio
> como un hecho.

```text
 cámara RTSP ──► calipso-detector ─────────────────────────────► NATS ──► consumidores
 (go2rtc)        ingest → sense → scene-core (el motor) → project        rerun-bridge (v3)
                 decodifica  YOLO    capa 0: la máquina      v3 y v4      trama → cinta
                                     de ocupación                         trama-resumen
```

- **El motor** (`scene-core`) es puro: no hace IO, decide con lo que ve y
  devuelve la transición. Todo lo demás observa.
- **El contrato** es lo que sale a NATS: `calipso.scene.v4`/`events.v4`, donde
  cada cambio de la capa 0 es una **transición** con su celda
  `antes:vimos:después`, y, al lado, `calipso.scene.v5`/`events.v5` cuando el
  perfil lo lista en `[publish] versions`. `v3` está retirado.
- **La trama** graba cada transición como una línea de texto: la fila de la
  tabla de la capa 0. **trama-resumen** la verifica y la resume.

## 2. El modelo en cinco minutos

La habitación tiene dos zonas: **afuera** (el piso) y **la cama**. El motor
lleva tres cuentas, la terna **RBH**:

| | Qué cuenta |
|---|---|
| **R** | ocupantes de la habitación |
| **B** | ocupantes de la cama (vistos + retenidos) |
| **H** | retenidos: están en la cama pero no se los ve |

Y mira dos cosas en cada muestra, el par **FV**: cuerpos vistos **f**uera y
en la cama (**v**istos). Una cuenta nueva tiene que **sostenerse** unos
segundos de fuente antes de creerla. Cuando se sostiene, la máquina cruza una
**celda** de la tabla:

```text
 100:01:110      antes RBH = 100 (uno afuera)
                 vimos FV  = 01  (nadie afuera, uno en la cama)
                 después   = 110 (uno, en la cama, visto)   → hecho: lay_down
```

Los estados con nombre (los de la tabla dorada):

| RBH | Estado |
|---|---|
| `000` | vacía |
| `100` | un solo ocupante, afuera |
| `110` | un solo ocupante, en la cama, visto |
| `111` | un solo ocupante, en la cama, **retenido** |
| `200`, `210`, `211`, … | acompañada |

Los hechos: `entered`, `left_room`, `lay_down`, `left_bed`, `held_in_bed`,
`reappeared`. Con causa `baseline` (`lay_down/baseline`), el motor lo
encontró en la cama al arrancar.

La tabla completa, generada desde el código y comparada en cada test, está en
[`tabla-capa-0.txt`](../contract/layer-0-table.txt)
(spec 1001). El fundamento, en el [libro 3](../../calipso-docs/libro-3/).

## 3. El repositorio

| Crate | Qué es | IO |
|---|---|---|
| `scene-core` | **El motor**: la máquina de la capa 0, la atención, la crónica y la proyección a v3/v4. El contrato (`contract::{common, v3, v4}`) y sus JSON Schema | ninguno (lo vigila `check.sh`) |
| `ingest` | RTSP → muestras decodificadas | red |
| `sense` | los modelos ONNX (YOLO: detección, refuerzo, caras, pose, profundidad) | archivos |
| `detector` | **la cáscara**: arma el lazo, el perfil (`config/*.toml`), publica a NATS, el archivero `--jsonl`. Binarios: `calipso-detector` y `calipso-detector-frames` | todo |

Fuera de este repo: `rerun-bridge` es `calipso-rerun-bridge` (visualización en
Rerun, consume v3 y v4); `golden` es `calipso-golden` (escenas golden); el
verificador es `calipso-verifier`.

Otras carpetas: `features/` (la especificación ejecutable, por capacidad),
`contract/` (schemas, ejemplos y tablas doradas), `config/` (perfiles),
`scripts/` (`check.sh`, `quick.sh`, `wip.sh`, `test-rtsp.sh`). Las specs y las
ADR están en `calipso-docs/specs/` y `calipso-docs/adr/`; las escenas golden,
en `calipso-golden/`.

## 4. Preparar el entorno

| Qué | Dónde / cómo |
|---|---|
| Rust (edition 2024) y `cargo` | el target compartido está en `~/.cache/cargo-target` (los binarios: `~/.cache/cargo-target/release/`) |
| NATS | `nats-server` escuchando en `127.0.0.1:4222` |
| La cámara de siempre | go2rtc sirve `rtsp://127.0.0.1:8554/sample1` (perfil `config/sample1.toml`) |
| Los modelos ONNX | en `calipso-dist/models`: exportar `CALIPSO_MODELS=/home/visiona/mana/calipso/calipso-dist/models` |
| Sin cámara | `scripts/test-rtsp.sh` (revisar su uso antes de correrlo) |

Compilar todo una vez:

```sh
cargo build --release -p detector
```

Comprobar que los servicios están: `ss -ltn | grep -E ':4222|:8554'`.

## 5. Procedimiento A — Verificar el repositorio

Antes de tocar nada y antes de **cada** commit:

```sh
LOG=/tmp/check.log; scripts/check.sh > "$LOG" 2>&1; echo "exit $?"   # 0 = commit verificado
```

`check.sh` es el guardián: formato, clippy, que el núcleo no dependa de IO,
que el dominio no hable en contrato, todos los tests (incluidos los Gherkin de
`scene-core` y `detector`) y licencias. Lo que está en curso (rojo a
propósito) se corre aparte con `scripts/wip.sh`.

## 6. Procedimiento B — Correr el motor en vivo

1. **El perfil.** `config/sample1.toml` describe la cámara, los sentidos, la
   política, el mapa (la cama dibujada) y NATS. Qué contratos se publican lo
   dice la sección `[publish]`; sin ella, v3 y v4:

   ```toml
   [publish]
   contracts = ["v3", "v4"]   # sin la sección, este es el default (R13-5)
   ```

   `[publish]` no cambia el hash del perfil: no es percepción. Para publicar
   solo una versión, `contracts = ["v3"]` o `["v4"]`.

2. **Validarlo** sin abrir cámara ni modelos:

   ```sh
   calipso-detector --config mi-perfil.toml --check
   ```

3. **Correrlo:**

   ```sh
   CALIPSO_MODELS=/home/visiona/mana/calipso/calipso-dist/models \
     calipso-detector --config mi-perfil.toml
   ```

   Publica en `vision.{site}.{camera}.{scene,events}.v4` siempre (para sample1:
   `vision.local.sample1.events.v4`), y en `.v5` cuando el perfil lo lista en
   `[publish] versions`. Cada versión tiene su propia numeración y su
   `last_event_seq`; los eventos llevan `Nats-Msg-Id` (`{run}-v4-{seq}` en v4,
   `{run}-v5-{seq}` en v5).

## 7. Procedimiento C — Grabar la noche (la trama)

**En vivo, desde NATS** (dejarlo corriendo al lado del detector):

```sh
trama --nats nats://127.0.0.1:4222 --prefix vision.local.sample1 >> noche.trama
```

**Desde el archivero** (una escena golden o una noche grabada en jsonl; las
líneas que no son `events.v4` se saltean):

```sh
calipso-detector --config perfil-con-v4.toml --escena golden/s1-personal-acuesta.md --rapido --jsonl s1.jsonl
trama < s1.jsonl > s1.trama
```

Una corrida real de sample1 (4 minutos), tal como la escribió `trama`
([`sample1-larga.trama`](../../calipso-docs/specs/r11/1104-la-trama/sample1-larga.trama)):

```text
# calipso.trama.v1 site=local camera=sample1 run=01M4MHQBX95J6YQ9PBV3MB79YH
~ 2026-10-11T03:57:23.905Z  1:0  0.0s  sensor connecting→streaming
2026-10-11T03:58:19.309Z  1:337  56.0s  000:10:100  de_paso  boot  entered
2026-10-11T03:58:46.302Z  1:499  83.0s  100:01:110  base  boot  lay_down
2026-10-11T04:00:00.628Z  1:943  157.0s  110:00:111  base  run  held_in_bed
2026-10-11T04:00:14.161Z  1:1021  170.0s  111:01:110  base  run  reappeared
…
```

### Cómo leer una línea

```text
2026-10-11T03:58:46.302Z  1:499  83.0s  100:01:110  base  boot  lay_down
└─ reloj (UTC) ─────────┘ └muestra┘ └fuente┘ └ celda ──┘ └ctx┘ └fase┘ └hechos┘
```

| Columna | Qué dice |
|---|---|
| reloj | cuándo se publicó (pared, UTC, ms) |
| muestra | `sesión:secuencia`; la sesión cambia si se cortó el stream |
| fuente | segundos del video dentro de la sesión |
| celda | `antes:vimos:después`; con `+u` si quedan cuerpos sin explicar en la cama |
| contexto | `base`, `lejos` (alguien lejos de la cama), `de_paso` (se vio a uno más de paso), o `lejos+de_paso` |
| fase | `boot` hasta la primera subida a la cama (lo que aparece en la cama se adopta); después `run` |
| hechos | separados por coma; `/causa` si no es la de siempre |

Las líneas `~` son cortes y vueltas del sensor. La cabecera `#` se repite por
cada corrida del detector. Las columnas no tienen espacios adentro: `awk` y
`grep` funcionan directo.

```sh
grep ':111 ' noche.trama          # cada vez que alguien quedó retenido en la cama
grep -c held_in_bed noche.trama   # cuántas veces
grep '^~' noche.trama             # los cortes del stream
```

## 8. Procedimiento D — Verificar y resumir la cinta

```sh
trama-resumen noche.trama        # o: trama-resumen < noche.trama
```

La salida real de la corrida de arriba:

```text
corrida calipso.trama.v1 site=local camera=sample1 run=01M4MHQBX95J6YQ9PBV3MB79YH
reloj 2026-10-11T03:57:23.905Z → 2026-10-11T04:01:38.714Z
10 transiciones · 1 corte de sensor

tiempo por estado (segundos de fuente)
  000  vacía                                    56.0 s
  100  un solo ocupante, afuera                 27.0 s
  110  un solo ocupante, en la cama, visto     131.0 s + sigue desde 1:1531
  111  un solo ocupante, en la cama, retenido   41.0 s

hechos: entered 1 · lay_down 1 · held_in_bed 4 · reappeared 4

verificación
  ✓ cadena sin huecos
  ✓ tabla: 9 de 9 filas verificables están en la tabla dorada
    1 no verificable: 1:337 (de_paso en boot)
```

**Cómo se lee:** la habitación estuvo vacía 56 s; entró una persona y a los
27 s se acostó. En la cama estuvo 172 s: 131 vista y 41 retenida, en cuatro
idas y vueltas `held_in_bed`/`reappeared` (se tapa y se destapa). Termina en
la cama, vista.

**Qué verifica**, como si cada línea fuera un datagrama:

1. **La cadena** (como un número de secuencia): el `después` de cada fila es
   el `antes` de la siguiente. Toda corrida empieza en `000`. Un corte del
   sensor **no** rompe la cadena (el motor conserva la cuenta entre
   sesiones). Si no encadena, falta al menos una fila:
   `✗ hueco entre 1:37 y 1:109: dice antes 100, se esperaba 000`.
2. **La tabla** (como un checksum): cada fila existe en la tabla dorada con el
   mismo después y los mismos hechos, en la columna de su contexto (`run`+
   `base` → `base`, `boot`+`base` → `arranque`, `run`+`lejos`, `run`+
   `de_paso`). Los contextos compuestos y más de dos personas no tienen
   columna: **no verificables**, sin veredicto.

`trama-resumen` sale con **1** si hay huecos, filas fuera de la tabla o
líneas ilegibles: sirve en un script o en un cron de la mañana.

## 8.1 Procedimiento D′ — Mirar la capa 0 en vivo (la occupancy-tui)

La mesa de trabajo de la capa 0, hermana de `oreo/resident-tui` y con su
mismo design system ([spec 1103](../../calipso-docs/specs/r11/1103-occupancy-tui/spec.md)):

```sh
cd tui/occupancy-tui && go run . --prefix vision.local.sample1
```

A la izquierda, la habitación según la última `scene.v4` y el tiempo por
estado; a la derecha, el log de `events.v4`, una vuelta por bloque:

```text
  hora     muestra #   capa   transición                               hechos         duró    verif.
▸ 01:27:56 1:1021   11 TRANS  EN CAMA·RETENIDO ▸ F0 V1 ▸ EN CAMA·VISTO reappeared      13.0 s ✓ tabla ✓ cadena
                       ┊  room_changed alone_in_bed_held → alone_in_bed_seen  ·  bed_changed held → seen
  01:27:43 1:943     8 TRANS  EN CAMA·VISTO ▸ F0 V0 ▸ EN CAMA·RETENIDO held_in_bed     74.0 s ✓ tabla ✓ cadena
```

**Los eventos permiten reaccionar; la escena permite reconciliar.** Si la
última `scene.v4` contradice lo que armaron los eventos, o falta un evento
(`seq` o `last_event_seq`), o una fila no encadena, aparece un `GAP` en el log
y en la línea inferior; la pantalla no completa nada a mano. `j/k` + `enter`
abre el inspector de una transición; `f` deja solo las transiciones.

## 8.2 Procedimiento D″ — Ver la noche en Rerun

El puente dibuja `scene.v5`/`events.v5` (y convierte las grabaciones `v4`) en Rerun **0.37.2** (el visor tiene que
ser de la misma versión que el SDK: `uv run --no-project --with rerun-sdk==0.37.2 rerun`).
Con el perfil por defecto, el detector ya publica v4:

```sh
rerun &                                    # el visor 0.37.2, gRPC en 9876
NATS_SUBJECT=vision.local.sample1.scene.v3 \
  cargo run --locked --manifest-path crates/rerun-bridge/Cargo.toml
```

Se ve la escena 2D, el carril de la habitación con el modo de la máquina, la
fase, la última celda, una línea por transición y la anatomía de la última
([README del puente](../../calipso-rerun-bridge/README.md), sección v4).

## 9. Procedimiento E — Cambiar algo (la forma de trabajo)

El orden de siempre: **primero el núcleo** (el motor, con specs y Gherkin),
**después la cáscara** (perfil, NATS, eventos reales), **después los
consumidores**.

1. **Una spec por cambio**: `specs/rNN/NNNN-nombre/spec.md` (historia, caso
   de uso, reglas) y su `.feature` en español. Lo que no está listo, `@wip`.
2. **Diseño mínimo antes de codificar**, mostrado y aprobado por la pareja.
   Las decisiones que duran van a un ADR en `docs/adr/`.
3. **Un escenario por vez**: el paso Gherkin en rojo **por la razón
   correcta**, el código mínimo para el verde, y una **prueba de mutación en
   pasos visibles**: mutar → correr (tiene que caer) → restaurar → correr.
4. **`scripts/check.sh` en verde antes de cada commit.**
5. **Commits por ruta explícita** (`git add <archivos>`, nunca `-A`), con el
   formato `<módulo>: <qué> (spec o ronda)`, y **push** de la rama. Nada queda
   solo en el disco.
6. Los arneses Gherkin se reparten por tag: sin tag, el núcleo
   (`scene-core`); `@cascara`, el detector; `@trama`, la grabación.

Reglas que no se rompen sin la pareja: v3 no cambia un byte con el perfil por
defecto (lo comparan los ejemplos S1–S8, las escenas golden y la tabla
dorada); el núcleo no importa IO; el dominio no habla en contrato.

## 10. Problemas conocidos

| Síntoma | Causa y arreglo |
|---|---|
| `Model file not found: face/yolov12l-face.onnx` | los modelos están en `calipso-dist/models`: `CALIPSO_MODELS=…` |
| `./target/release/…: no such file` | el target es `~/.cache/cargo-target` |
| Al cortar con Ctrl-C: `WARN canal de publicación lleno o cerrado … dropped_events=0` | es la última escena en vuelo al cerrar; no se pierden eventos |
| La cinta no tiene filas | ¿el perfil tiene `[publish] contracts = [..., "v4"]`? ¿`--prefix` es `vision.{site}.{camera}`? |
| `git push` pide usuario | las credenciales de Gitea están en `~/.git-credentials` (`credential.helper store`) |
| Primera fila `de_paso`/`lejos` en `boot` | es real (alguien se vio de paso antes de que la cama tuviera historia); la tabla no la cubre: no verificable |

## 11. Para seguir leyendo

- [Memoria funcional R4](../../calipso-docs/specs/r4/00-memoria-funcional.md): las seis reglas y lo que se publica (§8, §8.1 v4).
- [ADR del contrato v4](../../calipso-docs/adr/2026-10-10-contrato-v4-en-paralelo.md) y [spec 1301](../../calipso-docs/specs/r13/1301-contrato-v4/spec.md).
- [Spec 1104, la trama](../../calipso-docs/specs/r11/1104-la-trama/spec.md): el formato y la verificación.
- [Spec 1001, la tabla como red](../../calipso-docs/specs/r10/1001-la-tabla-como-red/spec.md).
- [Escenas golden](../../calipso-golden/README.md): correr una escena con `--escena`.
- [`scene-core/DESIGN.md`](../crates/scene-core/DESIGN.md): el mapa de módulos y la deuda.
